mod guarded_io;
mod pair_source;
mod pair_source_chain2;
mod pair_source_json;
pub mod process_runner;
pub mod scope_resolver;

use guarded_io::{CacheWriteGuard, atomic_write_guarded_file, read_guarded_text};

use crate::agent_profiles::{AgentProfiles, RoundType};
use crate::tddd::semantic_verify_codec::{
    CatalogueSpecVerifyCacheDocumentCodec, SpecAdrVerifyCacheDocumentCodec,
};
use domain::tddd::semantic_verify::{
    CatalogueSpecVerifyCacheDocument, ModelTier, SemanticVerdict, SemanticVerifyEntry,
    SpecAdrVerifyCacheDocument,
};
use pair_source::{extract_json_object_parsed, render_prompt_template, validate_template_path};
pub use process_runner::{
    build_claude_ref_verifier_args, build_codex_ref_verifier_args, build_gemini_ref_verifier_args,
    make_ref_verifier_process_runner,
};
pub use scope_resolver::{RefVerifyScopeResolver, RefVerifyScopeResolverError};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use usecase::ref_verify::{
    RefVerifierPort, RefVerifyCachePort, RefVerifyCacheScope, RefVerifyCommand, RefVerifyConfig,
    RefVerifyError, RefVerifyPair, RefVerifyPairSourcePort,
};

pub type AgentExecutionRunner = dyn Fn(crate::agent_profiles::ResolvedExecution, String) -> Result<String, RefVerifyError>
    + Send
    + Sync;

fn track_dir(project_root: &Path, track_id: &str) -> PathBuf {
    project_root.join("track").join("items").join(track_id)
}
#[derive(Debug)]
pub struct RefVerifyPairSourceAdapter {
    project_root: PathBuf,
}
impl RefVerifyPairSourceAdapter {
    #[must_use]
    pub fn new(project_root: PathBuf) -> Self {
        Self { project_root }
    }
}
impl RefVerifyPairSourcePort for RefVerifyPairSourceAdapter {
    fn load_pairs(
        &self,
        cmd: &RefVerifyCommand,
        config: &RefVerifyConfig,
    ) -> Result<Vec<RefVerifyPair>, RefVerifyError> {
        let track_id = cmd.track_id.as_ref();
        let dir = track_dir(&self.project_root, track_id);
        let mut pairs: Vec<RefVerifyPair> = Vec::new();
        let include_chain1 = matches!(
            &cmd.scope,
            usecase::ref_verify::RefVerifyScope::Chain1 | usecase::ref_verify::RefVerifyScope::All
        );
        if include_chain1 {
            pairs.extend(pair_source::enumerate_chain1_pairs(&dir, &self.project_root)?);
        }
        match &cmd.scope {
            usecase::ref_verify::RefVerifyScope::Chain2 { layer } => {
                pairs.extend(pair_source_chain2::enumerate_chain2_pairs_for_layer(
                    &dir,
                    &self.project_root,
                    layer.clone(),
                )?);
            }
            usecase::ref_verify::RefVerifyScope::All => {
                pairs.extend(pair_source_chain2::enumerate_chain2_all_layers(
                    &dir,
                    &self.project_root,
                )?);
            }
            usecase::ref_verify::RefVerifyScope::Chain1 => {}
        }
        let injection_rate = config.known_bad_injection_rate_percent.as_u8();
        let probe_count = pair_source::calculate_probe_count(pairs.len(), injection_rate);

        // For All-scope runs, alternate Chain1/Chain2 probes only when Chain-2 pairs were
        // actually produced (catalogues present). In the pre-Phase-2 path (all catalogues
        // absent), `enumerate_chain2_all_layers` correctly returned zero pairs, so we must
        // not inject Chain-2 probes — the chain2 capability/template may not be configured
        // yet, and injecting them would break the run.
        let has_chain1_pairs =
            pairs.iter().any(|p| matches!(p.cache_scope, RefVerifyCacheScope::SpecAdr));
        let has_chain2_pairs = pairs
            .iter()
            .any(|p| matches!(p.cache_scope, RefVerifyCacheScope::CatalogueSpec { .. }));

        // AC-09 / D5: every verifier capability exercised by this run must receive at
        // least one known-bad calibration probe. With the alternating assignment below,
        // a single probe would reach only one chain, leaving the other chain's verifier
        // uncalibrated while its production pairs are still trusted at the fast tier.
        let probe_count = if matches!(cmd.scope, usecase::ref_verify::RefVerifyScope::All)
            && has_chain1_pairs
            && has_chain2_pairs
        {
            probe_count.max(2)
        } else {
            probe_count
        };

        for i in 0..probe_count {
            // Route known-bad probes through the same chain capability as the real pairs.
            // For Chain1-only runs: SpecAdr (Chain1 verifier).
            // For Chain2-only runs: CatalogueSpec (Chain2 verifier).
            // For All-scope runs: alternate between both chains when Chain-2 pairs exist;
            // fall back to SpecAdr-only when no Chain-2 pairs are present (pre-Phase-2 path).
            let probe_scope = match &cmd.scope {
                usecase::ref_verify::RefVerifyScope::Chain1 => RefVerifyCacheScope::SpecAdr,
                usecase::ref_verify::RefVerifyScope::Chain2 { layer } => {
                    RefVerifyCacheScope::CatalogueSpec { layer: layer.clone() }
                }
                usecase::ref_verify::RefVerifyScope::All => {
                    if has_chain2_pairs && i % 2 == 0 {
                        // Even indices → Chain2 probe first, so that a single probe (i == 0)
                        // always exercises the Chain2 verifier when Chain-2 pairs are present.
                        // Propagate errors: a rules file that exists but fails to load/parse
                        // must not silently fall back to SpecAdr and leave Chain-2 uncalibrated.
                        match pair_source_chain2::first_tddd_layer_scope(&self.project_root)? {
                            Some(scope) => scope,
                            None => RefVerifyCacheScope::SpecAdr,
                        }
                    } else {
                        RefVerifyCacheScope::SpecAdr
                    }
                }
            };
            pairs.push(pair_source::make_known_bad_probe(i, probe_scope)?);
        }
        Ok(pairs)
    }
}

#[derive(Debug)]
pub struct RefVerifyCacheAdapter {
    project_root: PathBuf,
}

impl RefVerifyCacheAdapter {
    #[must_use]
    pub fn new(project_root: PathBuf) -> Self {
        Self { project_root }
    }

    fn cache_file_path(&self, cmd: &RefVerifyCommand, scope: &RefVerifyCacheScope) -> PathBuf {
        let dir = track_dir(&self.project_root, cmd.track_id.as_ref());
        match scope {
            RefVerifyCacheScope::SpecAdr => dir.join("spec-adr-verify-cache.json"),
            RefVerifyCacheScope::CatalogueSpec { layer } => {
                dir.join(format!("{}-catalogue-spec-verify-cache.json", layer.as_ref()))
            }
        }
    }
}

impl RefVerifyCachePort for RefVerifyCacheAdapter {
    fn load_entries(
        &self,
        cmd: &RefVerifyCommand,
        scope: &RefVerifyCacheScope,
    ) -> Result<Vec<SemanticVerifyEntry>, RefVerifyError> {
        let path = self.cache_file_path(cmd, scope);

        let exists = path.try_exists().map_err(|e| RefVerifyError::CachePersistence {
            message: format!("cannot inspect verify-cache path '{}': {e}", path.display()),
        })?;
        if !exists {
            return Ok(Vec::new());
        }

        let text = read_guarded_text(&path, &self.project_root).map_err(|e| {
            RefVerifyError::CachePersistence {
                message: format!("cannot read verify-cache at '{}': {e}", path.display()),
            }
        })?;

        match scope {
            RefVerifyCacheScope::SpecAdr => {
                let doc = SpecAdrVerifyCacheDocumentCodec::decode(&text).map_err(|e| {
                    RefVerifyError::CachePersistence {
                        message: format!(
                            "cannot decode spec-adr-verify-cache at '{}': {e}",
                            path.display()
                        ),
                    }
                })?;
                Ok(doc.entries)
            }
            RefVerifyCacheScope::CatalogueSpec { layer } => {
                let doc = CatalogueSpecVerifyCacheDocumentCodec::decode(&text).map_err(|e| {
                    RefVerifyError::CachePersistence {
                        message: format!(
                            "cannot decode catalogue-spec-verify-cache at '{}': {e}",
                            path.display()
                        ),
                    }
                })?;
                if &doc.layer != layer {
                    return Err(RefVerifyError::CachePersistence {
                        message: format!(
                            "catalogue-spec verify-cache layer mismatch at '{}': expected '{}', got '{}'",
                            path.display(),
                            layer.as_ref(),
                            doc.layer.as_ref()
                        ),
                    });
                }
                Ok(doc.entries)
            }
        }
    }

    fn save_entries(
        &self,
        cmd: &RefVerifyCommand,
        scope: &RefVerifyCacheScope,
        entries: Vec<SemanticVerifyEntry>,
    ) -> Result<(), RefVerifyError> {
        let path = self.cache_file_path(cmd, scope);

        let _guard = CacheWriteGuard::acquire(&path, &self.project_root)?;

        let json = match scope {
            RefVerifyCacheScope::SpecAdr => {
                let doc = SpecAdrVerifyCacheDocument::new(entries);
                SpecAdrVerifyCacheDocumentCodec::encode(&doc).map_err(|e| {
                    RefVerifyError::CachePersistence {
                        message: format!("encode error for spec-adr-verify-cache: {e}"),
                    }
                })?
            }
            RefVerifyCacheScope::CatalogueSpec { layer } => {
                let doc = CatalogueSpecVerifyCacheDocument::new(layer.clone(), entries);
                CatalogueSpecVerifyCacheDocumentCodec::encode(&doc).map_err(|e| {
                    RefVerifyError::CachePersistence {
                        message: format!(
                            "encode error for {}-catalogue-spec-verify-cache: {e}",
                            layer.as_ref()
                        ),
                    }
                })?
            }
        };

        #[cfg(unix)]
        atomic_write_guarded_file(&path, &_guard.parent_dir, json.as_bytes()).map_err(|e| {
            RefVerifyError::CachePersistence {
                message: format!("cannot write verify-cache at '{}': {e}", path.display()),
            }
        })?;

        #[cfg(not(unix))]
        atomic_write_guarded_file(&path, &self.project_root, json.as_bytes()).map_err(|e| {
            RefVerifyError::CachePersistence {
                message: format!("cannot write verify-cache at '{}': {e}", path.display()),
            }
        })?;

        Ok(())
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum VerdictKindDto {
    Pass,
    Fail,
    Pending,
}

/// Flat verdict response DTO compatible with OpenAI structured-output (`--output-schema`).
///
/// OpenAI rejects `oneOf` in structured output schemas and requires every property
/// to appear in `required`, so we model the verdict as a flat struct with a `kind`
/// discriminator plus nullable `citation` / `reason` strings.
///
/// `deny_unknown_fields` enforces the fail-closed boundary: any extra field in the
/// verifier response causes deserialization to fail rather than being silently accepted.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct VerdictResponseDto {
    kind: VerdictKindDto,
    citation: Option<String>,
    reason: Option<String>,
}

pub struct AgentRefVerifierAdapter {
    profiles: Arc<AgentProfiles>,
    runner: Arc<AgentExecutionRunner>,
    project_root: PathBuf,
}

impl std::fmt::Debug for AgentRefVerifierAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentRefVerifierAdapter")
            .field("profiles", &self.profiles)
            .field("runner", &"<AgentExecutionRunner>")
            .field("project_root", &self.project_root)
            .finish()
    }
}

impl AgentRefVerifierAdapter {
    #[must_use]
    pub fn new(
        profiles: Arc<AgentProfiles>,
        runner: Arc<AgentExecutionRunner>,
        project_root: PathBuf,
    ) -> Self {
        Self { profiles, runner, project_root }
    }

    fn tier_to_round_type(tier: &ModelTier) -> RoundType {
        match tier {
            ModelTier::Fast => RoundType::Fast,
            ModelTier::Final => RoundType::Final,
        }
    }
}

impl RefVerifierPort for AgentRefVerifierAdapter {
    fn verify_pair(
        &self,
        claim: String,
        evidence: String,
        cache_scope: &RefVerifyCacheScope,
        tier: ModelTier,
    ) -> Result<SemanticVerdict, RefVerifyError> {
        // D11: Chain-specific capabilities + prompt templates.
        // SpecAdr   → ref-verifier-chain1 (intent grounding, strict)
        // Catalogue → ref-verifier-chain2 (translation-gap allowance per D10)
        let capability: &str = match cache_scope {
            RefVerifyCacheScope::SpecAdr => "ref-verifier-chain1",
            RefVerifyCacheScope::CatalogueSpec { .. } => "ref-verifier-chain2",
        };

        let round_type = Self::tier_to_round_type(&tier);

        let resolved =
            self.profiles.resolve_execution(capability, round_type).ok_or_else(|| {
                RefVerifyError::VerifierPort {
                    message: format!(
                        "capability '{capability}' is not defined in agent-profiles.json"
                    ),
                }
            })?;

        let template_path = self.profiles.resolve_prompt_template_path(capability).ok_or_else(
            || RefVerifyError::VerifierPort {
                message: format!(
                    "capability '{capability}' has no prompt_template_path in agent-profiles.json"
                ),
            },
        )?;

        let validated_template_path = validate_template_path(&template_path, &self.project_root)
            .map_err(|e| RefVerifyError::VerifierPort {
                message: format!("prompt template path validation failed: {e}"),
            })?;

        let template_text = read_guarded_text(&validated_template_path, &self.project_root)
            .map_err(|e| RefVerifyError::VerifierPort {
                message: format!(
                    "cannot read prompt template at '{}': {e}",
                    validated_template_path.display()
                ),
            })?;

        let tier_str = match tier {
            ModelTier::Fast => "fast",
            ModelTier::Final => "final",
        };
        let prompt = render_prompt_template(&template_text, &claim, &evidence, tier_str);

        let raw_output = (self.runner)(resolved, prompt)?;

        let dto: VerdictResponseDto =
            extract_json_object_parsed(&raw_output).map_err(|e| RefVerifyError::VerifierPort {
                message: format!(
                    "ref-verifier response JSON parse error: {e}; raw model output redacted"
                ),
            })?;

        // Verify that all required schema fields are present in the JSON payload
        // (even if their values are null). The OpenAI structured-output contract
        // requires every property to appear in `required`, so a payload missing
        // `citation` or `reason` keys entirely is schema-invalid and must be
        // rejected fail-closed — regardless of the `kind` discriminator.
        let raw_value: serde_json::Value =
            extract_json_object_parsed(&raw_output).map_err(|e| RefVerifyError::VerifierPort {
                message: format!("verdict schema re-parse error: {e}; raw model output redacted"),
            })?;
        for required_key in ["citation", "reason"] {
            if raw_value.get(required_key).is_none() {
                return Err(RefVerifyError::VerifierPort {
                    message: format!(
                        "verdict missing required field '{required_key}' — \
                         rejected at codec boundary"
                    ),
                });
            }
        }

        match dto.kind {
            VerdictKindDto::Pass => {
                let citation_text = dto.citation.unwrap_or_default();
                let evidence_citation =
                    domain::tddd::semantic_verify::EvidenceCitation::try_new(citation_text)
                        .map_err(|e| RefVerifyError::VerifierPort {
                            message: format!(
                                "citation-absent or empty pass rejected by codec boundary: {e}"
                            ),
                        })?;
                Ok(SemanticVerdict::Pass { citation: evidence_citation })
            }
            VerdictKindDto::Fail => {
                let reason = dto.reason.filter(|r| !r.is_empty()).ok_or_else(|| {
                    RefVerifyError::VerifierPort {
                        message:
                            "fail verdict missing required reason field — rejected at codec boundary"
                                .to_owned(),
                    }
                })?;
                Ok(SemanticVerdict::Fail { reason })
            }
            VerdictKindDto::Pending => Ok(SemanticVerdict::Pending),
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::too_many_lines
)]
mod tests {
    use std::io::Write as _;
    use std::sync::Arc;

    use domain::ContentHash;
    use domain::tddd::LayerId;
    use domain::tddd::semantic_verify::{EvidenceCitation, SemanticVerdict, SemanticVerifyEntry};

    use usecase::ref_verify::{
        RefVerifyCachePort, RefVerifyCacheScope, RefVerifyCommand, RefVerifyScope,
    };

    use super::*;

    // ── helpers ───────────────────────────────────────────────────────────────

    fn hash(byte: u8) -> ContentHash {
        ContentHash::from_bytes([byte; 32])
    }

    fn pass_entry(claim: u8, evidence: u8) -> SemanticVerifyEntry {
        SemanticVerifyEntry::new(
            hash(claim),
            hash(evidence),
            SemanticVerdict::Pass {
                citation: EvidenceCitation::try_new("the spec states X".to_owned()).unwrap(),
            },
        )
    }

    fn track_cmd(track_id: &str) -> RefVerifyCommand {
        RefVerifyCommand {
            track_id: domain::TrackId::try_new(track_id).unwrap(),
            scope: RefVerifyScope::All,
            current_branch: format!("track/{track_id}"),
        }
    }

    // ── extract_json_object_parsed ────────────────────────────────────────────

    #[test]
    fn extract_json_object_parsed_finds_pass_in_plain_json() {
        let raw = r#"{"kind": "pass", "citation": "the text"}"#;
        let dto: VerdictResponseDto = extract_json_object_parsed(raw).unwrap();
        assert!(matches!(dto.kind, VerdictKindDto::Pass));
    }

    #[test]
    fn extract_json_object_parsed_trims_outer_whitespace() {
        let raw = "\n  {\"kind\": \"fail\", \"reason\": \"no match\"}\n";
        let dto: VerdictResponseDto = extract_json_object_parsed(raw).unwrap();
        assert!(matches!(dto.kind, VerdictKindDto::Fail));
    }

    #[test]
    fn extract_json_object_parsed_rejects_object_embedded_in_prose() {
        let raw = r#"Here is my verdict: {"kind": "fail", "reason": "no match"} end."#;
        let result: Result<VerdictResponseDto, _> = extract_json_object_parsed(raw);
        assert!(matches!(result, Err(message) if message.contains("exactly one")));
    }

    #[test]
    fn extract_json_object_parsed_rejects_invalid_brace_prose_before_verdict() {
        let raw =
            r#"Example placeholder: {not json}. Final: {"kind": "pass", "citation": "the text"}"#;
        let result: Result<VerdictResponseDto, _> = extract_json_object_parsed(raw);
        assert!(matches!(result, Err(message) if message.contains("exactly one")));
    }

    #[test]
    fn extract_json_object_parsed_handles_citation_with_braces_in_string() {
        // A citation containing `{` or `}` must not confuse the parser.
        let raw = r#"{"kind": "pass", "citation": "text with {braces} in citation"}"#;
        let dto: VerdictResponseDto = extract_json_object_parsed(raw).unwrap();
        assert!(matches!(dto.kind, VerdictKindDto::Pass));
        assert!(dto.citation.as_deref().unwrap_or("").contains("{braces}"));
    }

    #[test]
    fn extract_json_object_parsed_rejects_multiple_top_level_verdicts() {
        let raw = r#"Example: {"kind": "fail", "reason": "sample"} Final: {"kind": "pass", "citation": "real verdict"}"#;
        let result: Result<VerdictResponseDto, _> = extract_json_object_parsed(raw);
        assert!(matches!(result, Err(message) if message.contains("exactly one")));
    }

    #[test]
    fn extract_json_object_parsed_rejects_valid_example_before_malformed_final() {
        let raw = r#"Example: {"kind": "fail", "reason": "sample"} Final: {not json}"#;
        let result: Result<VerdictResponseDto, _> = extract_json_object_parsed(raw);
        assert!(matches!(result, Err(message) if message.contains("exactly one")));
    }

    #[test]
    fn extract_json_object_parsed_rejects_valid_example_before_malformed_final_with_prose() {
        let raw = r#"Example: {"kind": "fail", "reason": "sample"} Final: here is the JSON verdict: {not json}"#;
        let result: Result<VerdictResponseDto, _> = extract_json_object_parsed(raw);
        assert!(matches!(result, Err(message) if message.contains("exactly one")));
    }

    #[test]
    fn extract_json_object_parsed_rejects_valid_example_before_unmarked_malformed_trailing_object()
    {
        let raw =
            r#"Example: {"kind": "fail", "reason": "sample"} Actual result follows: {not json}"#;
        let result: Result<VerdictResponseDto, _> = extract_json_object_parsed(raw);
        assert!(matches!(result, Err(message) if message.contains("exactly one")));
    }

    #[test]
    fn extract_json_object_parsed_rejects_trailing_malformed_brace_after_verdict() {
        let raw = r#"Final: {"kind": "pass", "citation": "real verdict"} trailing note {not json}"#;
        let result: Result<VerdictResponseDto, _> = extract_json_object_parsed(raw);
        assert!(matches!(result, Err(message) if message.contains("exactly one")));
    }

    #[test]
    fn extract_json_object_parsed_returns_error_when_no_object() {
        let result: Result<VerdictResponseDto, _> = extract_json_object_parsed("no braces here");
        assert!(result.is_err());
    }

    #[test]
    fn extract_json_object_parsed_rejects_nested_verdict_in_wrapper() {
        let raw = r#"{"meta":{"kind":"pass","citation":"nested verdict"}}"#;
        let result: Result<VerdictResponseDto, _> = extract_json_object_parsed(raw);
        assert!(result.is_err());
    }

    #[test]
    fn extract_json_object_parsed_rejects_verdict_nested_in_array() {
        let raw = r#"[{"kind":"pass","citation":"nested verdict"}]"#;
        let result: Result<VerdictResponseDto, _> = extract_json_object_parsed(raw);
        assert!(result.is_err());
    }

    #[test]
    fn render_prompt_template_does_not_rescan_inserted_values() {
        let rendered = render_prompt_template(
            "{{claim}}|{{evidence}}|{{tier}}",
            "{{evidence}}",
            "{{tier}}",
            "fast",
        );
        assert_eq!(rendered, "{{evidence}}|{{tier}}|fast");
    }

    #[test]
    fn hash_git_blob_text_matches_sha256_git_blob_preimage() {
        let blob_hash = pair_source::hash_git_blob_text("hello\n");
        let plain_hash = pair_source::hash_text("hello\n").unwrap();

        assert_eq!(
            blob_hash.to_string(),
            "2cf8d83d9ee29543b34a87727421fdecb7e3f3a183d337639025de576db9ebb4"
        );
        assert_ne!(blob_hash, plain_hash);
    }

    #[test]
    fn test_read_adr_anchor_text_with_frontmatter_decision_without_heading_returns_decision_evidence()
     {
        let dir = tempfile::tempdir().unwrap();
        let adr_path = dir.path().join("adr.md");
        std::fs::write(
            &adr_path,
            r#"---
adr_id: frontmatter-only-adr
decisions:
  - id: D1
    status: proposed
    candidate_selection: "choose semantic gate"
---
# ADR

## Context
No decision heading exists here.
"#,
        )
        .unwrap();

        let (evidence, raw) =
            pair_source::read_adr_anchor_text(&adr_path, dir.path(), "D1").unwrap();

        assert!(raw.contains("frontmatter-only-adr"));
        assert!(evidence.contains("ADR decision [D1]"));
        assert!(evidence.contains("status: proposed"));
        assert!(evidence.contains("candidate_selection: choose semantic gate"));
        assert!(!evidence.contains("No decision heading exists here."));
    }

    #[test]
    fn test_read_adr_anchor_text_with_heading_only_anchor_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let adr_path = dir.path().join("adr.md");
        std::fs::write(
            &adr_path,
            r#"---
adr_id: heading-only-adr
decisions:
  - id: D1
    status: proposed
---
# ADR

### D2: Heading without front-matter decision
This section must not make D2 a valid ADR ref.
"#,
        )
        .unwrap();

        let err = pair_source::read_adr_anchor_text(&adr_path, dir.path(), "D2").unwrap_err();

        assert!(err.contains("decisions[].id"));
    }

    // ── pair_source::calculate_probe_count ───────────────────────────────────

    #[test]
    fn probe_count_is_zero_when_no_pairs() {
        assert_eq!(pair_source::calculate_probe_count(0, 10), 0);
    }

    #[test]
    fn probe_count_rounds_up() {
        // 3 pairs × 10% → 0.3 → rounds up to 1.
        assert_eq!(pair_source::calculate_probe_count(3, 10), 1);
    }

    #[test]
    fn probe_count_at_100_percent() {
        assert_eq!(pair_source::calculate_probe_count(5, 100), 5);
    }

    // ── known_bad probe flag ──────────────────────────────────────────────────

    #[test]
    fn make_known_bad_probe_sets_known_bad_true() {
        let probe = pair_source::make_known_bad_probe(0, RefVerifyCacheScope::SpecAdr).unwrap();
        assert!(probe.known_bad);
    }

    #[test]
    fn make_known_bad_probe_claim_starts_with_known_bad() {
        let probe = pair_source::make_known_bad_probe(0, RefVerifyCacheScope::SpecAdr).unwrap();
        assert!(probe.claim.starts_with("known-bad"));
    }

    // ── AgentRefVerifierAdapter — profiles resolution ─────────────────────────

    fn write_profiles(dir: &std::path::Path, json: &str) -> std::path::PathBuf {
        let path = dir.join("agent-profiles.json");
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(json.as_bytes()).unwrap();
        path
    }

    const REF_VERIFIER_CONFIG: &str = r#"{
        "schema_version": 1,
        "providers": {
            "claude": { "label": "Claude Code" }
        },
        "capabilities": {
            "ref-verifier-chain1": {
                "provider": "claude",
                "model": "claude-opus-4-8",
                "fast_provider": "claude",
                "fast_model": "claude-haiku-4-5",
                "prompt_template_path": ".harness/prompts/ref-verifier-chain1.md"
            },
            "ref-verifier-chain2": {
                "provider": "claude",
                "model": "claude-opus-4-8",
                "fast_provider": "claude",
                "fast_model": "claude-haiku-4-5",
                "prompt_template_path": ".harness/prompts/ref-verifier-chain2.md"
            }
        }
    }"#;

    #[test]
    fn agent_ref_verifier_adapter_constructs_with_profiles_and_runner() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_profiles(dir.path(), REF_VERIFIER_CONFIG);
        let profiles = Arc::new(AgentProfiles::load(&path).unwrap());
        let runner: Arc<AgentExecutionRunner> = Arc::new(|_resolved, _prompt| {
            Ok(r#"{"kind": "pass", "citation": "the spec states X"}"#.to_owned())
        });
        let _adapter = AgentRefVerifierAdapter::new(profiles, runner, dir.path().to_path_buf());
    }

    #[test]
    fn tier_to_round_type_fast_maps_to_fast() {
        assert_eq!(AgentRefVerifierAdapter::tier_to_round_type(&ModelTier::Fast), RoundType::Fast);
    }

    #[test]
    fn tier_to_round_type_final_maps_to_final() {
        assert_eq!(
            AgentRefVerifierAdapter::tier_to_round_type(&ModelTier::Final),
            RoundType::Final
        );
    }

    #[test]
    fn verify_pair_resolves_fast_execution_via_profiles_when_tier_is_fast() {
        let dir = tempfile::tempdir().unwrap();
        let _path = write_profiles(dir.path(), REF_VERIFIER_CONFIG);
        // profiles from REF_VERIFIER_CONFIG uses relative prompt_template_path; re-create below.

        // Create the prompt template file.
        let prompt_dir = dir.path().join(".harness").join("prompts");
        std::fs::create_dir_all(&prompt_dir).unwrap();
        std::fs::write(
            prompt_dir.join("ref-verifier-chain1.md"),
            "Claim: {{claim}}\nEvidence: {{evidence}}\nTier: {{tier}}",
        )
        .unwrap();

        let called_with: Arc<std::sync::Mutex<Option<crate::agent_profiles::ResolvedExecution>>> =
            Arc::new(std::sync::Mutex::new(None));
        let called_with_clone = Arc::clone(&called_with);

        let runner: Arc<AgentExecutionRunner> = Arc::new(move |resolved, _prompt| {
            *called_with_clone.lock().unwrap() = Some(resolved);
            Ok(r#"{"kind": "pass", "citation": "spec states X", "reason": null}"#.to_owned())
        });

        // Point profiles at dir.path() for the prompt template path.
        let profiles_json = r#"{
                "schema_version": 1,
                "providers": { "claude": { "label": "Claude" } },
                "capabilities": {
                    "ref-verifier-chain1": {
                        "provider": "claude",
                        "model": "claude-opus-4-8",
                        "fast_provider": "claude",
                        "fast_model": "claude-haiku-4-5",
                        "prompt_template_path": ".harness/prompts/ref-verifier-chain1.md"
                    }
                }
            }"#
        .to_owned();
        let path2 = write_profiles(dir.path(), &profiles_json);
        let profiles2 = Arc::new(AgentProfiles::load(&path2).unwrap());

        let adapter = AgentRefVerifierAdapter::new(profiles2, runner, dir.path().to_path_buf());
        let verdict = adapter
            .verify_pair(
                "claim text".to_owned(),
                "evidence text".to_owned(),
                &RefVerifyCacheScope::SpecAdr,
                ModelTier::Fast,
            )
            .unwrap();

        assert!(matches!(verdict, SemanticVerdict::Pass { .. }));
        let called = called_with.lock().unwrap();
        let resolved = called.as_ref().unwrap();
        // Fast tier → fast_model
        assert_eq!(resolved.model.as_deref(), Some("claude-haiku-4-5"));
    }

    #[test]
    fn verify_pair_returns_verifier_port_error_when_capability_missing() {
        let json = r#"{
            "schema_version": 1,
            "providers": { "claude": { "label": "Claude" } },
            "capabilities": {}
        }"#;
        let dir = tempfile::tempdir().unwrap();
        let path = write_profiles(dir.path(), json);
        let profiles = Arc::new(AgentProfiles::load(&path).unwrap());
        let runner: Arc<AgentExecutionRunner> = Arc::new(|_, _| Ok(String::new()));
        let adapter = AgentRefVerifierAdapter::new(profiles, runner, dir.path().to_path_buf());
        let err = adapter
            .verify_pair(
                "claim".to_owned(),
                "evidence".to_owned(),
                &RefVerifyCacheScope::SpecAdr,
                ModelTier::Fast,
            )
            .unwrap_err();
        assert!(
            matches!(err, usecase::ref_verify::RefVerifyError::VerifierPort { .. }),
            "expected VerifierPort, got {err:?}"
        );
    }

    #[test]
    fn verify_pair_returns_pass_verdict_when_runner_returns_pass_json() {
        let dir = tempfile::tempdir().unwrap();
        let prompt_dir = dir.path().join(".harness").join("prompts");
        std::fs::create_dir_all(&prompt_dir).unwrap();
        std::fs::write(prompt_dir.join("ref-verifier-chain1.md"), "{{claim}} {{evidence}}")
            .unwrap();
        let profiles_json = r#"{
                "schema_version": 1,
                "providers": { "claude": { "label": "Claude" } },
                "capabilities": {
                    "ref-verifier-chain1": {
                        "provider": "claude",
                        "model": "claude-opus-4-8",
                        "fast_provider": "claude",
                        "fast_model": "claude-haiku-4-5",
                        "prompt_template_path": ".harness/prompts/ref-verifier-chain1.md"
                    }
                }
            }"#
        .to_owned();
        let path = write_profiles(dir.path(), &profiles_json);
        let profiles = Arc::new(AgentProfiles::load(&path).unwrap());
        let runner: Arc<AgentExecutionRunner> = Arc::new(|_, _| {
            Ok(r#"{"kind": "pass", "citation": "the spec states X explicitly", "reason": null}"#
                .to_owned())
        });
        let adapter = AgentRefVerifierAdapter::new(profiles, runner, dir.path().to_path_buf());
        let verdict = adapter
            .verify_pair(
                "c".to_owned(),
                "e".to_owned(),
                &RefVerifyCacheScope::SpecAdr,
                ModelTier::Final,
            )
            .unwrap();
        assert!(matches!(verdict, SemanticVerdict::Pass { .. }));
    }

    #[test]
    fn verify_pair_fail_closed_on_malformed_json() {
        let dir = tempfile::tempdir().unwrap();
        let prompt_dir = dir.path().join(".harness").join("prompts");
        std::fs::create_dir_all(&prompt_dir).unwrap();
        std::fs::write(prompt_dir.join("ref-verifier-chain1.md"), "{{claim}}").unwrap();
        let profiles_json = r#"{
                "schema_version": 1,
                "providers": { "claude": { "label": "Claude" } },
                "capabilities": {
                    "ref-verifier-chain1": {
                        "provider": "claude",
                        "model": "claude-opus-4-8",
                        "prompt_template_path": ".harness/prompts/ref-verifier-chain1.md"
                    }
                }
            }"#
        .to_owned();
        let path = write_profiles(dir.path(), &profiles_json);
        let profiles = Arc::new(AgentProfiles::load(&path).unwrap());
        let runner: Arc<AgentExecutionRunner> = Arc::new(|_, _| Ok("not json at all".to_owned()));
        let adapter = AgentRefVerifierAdapter::new(profiles, runner, dir.path().to_path_buf());
        let err = adapter
            .verify_pair(
                "c".to_owned(),
                "e".to_owned(),
                &RefVerifyCacheScope::SpecAdr,
                ModelTier::Fast,
            )
            .unwrap_err();
        assert!(matches!(err, usecase::ref_verify::RefVerifyError::VerifierPort { .. }));
    }

    #[test]
    fn verify_pair_fail_closed_on_citation_absent_pass() {
        let dir = tempfile::tempdir().unwrap();
        let prompt_dir = dir.path().join(".harness").join("prompts");
        std::fs::create_dir_all(&prompt_dir).unwrap();
        std::fs::write(prompt_dir.join("ref-verifier-chain1.md"), "{{claim}}").unwrap();
        let profiles_json = r#"{
                "schema_version": 1,
                "providers": { "claude": { "label": "Claude" } },
                "capabilities": {
                    "ref-verifier-chain1": {
                        "provider": "claude",
                        "model": "claude-opus-4-8",
                        "prompt_template_path": ".harness/prompts/ref-verifier-chain1.md"
                    }
                }
            }"#
        .to_owned();
        let path = write_profiles(dir.path(), &profiles_json);
        let profiles = Arc::new(AgentProfiles::load(&path).unwrap());
        // Pass with empty citation — should fail-closed.
        let runner: Arc<AgentExecutionRunner> =
            Arc::new(|_, _| Ok(r#"{"kind": "pass", "citation": "", "reason": null}"#.to_owned()));
        let adapter = AgentRefVerifierAdapter::new(profiles, runner, dir.path().to_path_buf());
        let err = adapter
            .verify_pair(
                "c".to_owned(),
                "e".to_owned(),
                &RefVerifyCacheScope::SpecAdr,
                ModelTier::Fast,
            )
            .unwrap_err();
        assert!(matches!(err, usecase::ref_verify::RefVerifyError::VerifierPort { .. }));
    }

    #[test]
    fn verify_pair_catalogue_spec_scope_uses_chain2_capability() {
        // Verifies that a CatalogueSpec-scoped pair is dispatched to `ref-verifier-chain2`
        // rather than `ref-verifier-chain1`. We assert this by (a) only registering chain2 in
        // agent-profiles.json and verifying the call succeeds, and (b) capturing the capability
        // name passed to the runner and asserting it corresponds to the chain2 model.
        let dir = tempfile::tempdir().unwrap();
        let prompt_dir = dir.path().join(".harness").join("prompts");
        std::fs::create_dir_all(&prompt_dir).unwrap();
        std::fs::write(
            prompt_dir.join("ref-verifier-chain2.md"),
            "Claim: {{claim}}\nEvidence: {{evidence}}\nTier: {{tier}}",
        )
        .unwrap();

        let called_with: Arc<std::sync::Mutex<Option<crate::agent_profiles::ResolvedExecution>>> =
            Arc::new(std::sync::Mutex::new(None));
        let called_with_clone = Arc::clone(&called_with);

        let runner: Arc<AgentExecutionRunner> = Arc::new(move |resolved, _prompt| {
            *called_with_clone.lock().unwrap() = Some(resolved);
            Ok(r#"{"kind": "pass", "citation": "catalogue entry matches spec section", "reason": null}"#.to_owned())
        });

        let profiles_json = r#"{
                "schema_version": 1,
                "providers": { "claude": { "label": "Claude" } },
                "capabilities": {
                    "ref-verifier-chain2": {
                        "provider": "claude",
                        "model": "claude-opus-4-8-chain2",
                        "fast_provider": "claude",
                        "fast_model": "claude-haiku-4-5-chain2",
                        "prompt_template_path": ".harness/prompts/ref-verifier-chain2.md"
                    }
                }
            }"#
        .to_owned();
        let path = write_profiles(dir.path(), &profiles_json);
        let profiles = Arc::new(AgentProfiles::load(&path).unwrap());

        let layer = LayerId::try_new("domain".to_owned()).unwrap();
        let adapter = AgentRefVerifierAdapter::new(profiles, runner, dir.path().to_path_buf());
        let verdict = adapter
            .verify_pair(
                "catalogue claim".to_owned(),
                "spec evidence".to_owned(),
                &RefVerifyCacheScope::CatalogueSpec { layer },
                ModelTier::Fast,
            )
            .unwrap();

        assert!(matches!(verdict, SemanticVerdict::Pass { .. }));
        // Fast tier of chain2 must have been selected.
        let called = called_with.lock().unwrap();
        let resolved = called.as_ref().unwrap();
        assert_eq!(resolved.model.as_deref(), Some("claude-haiku-4-5-chain2"));
    }

    // ── validate_template_path ────────────────────────────────────────────────

    #[test]
    fn validate_template_path_accepts_relative_path_within_root() {
        let dir = tempfile::tempdir().unwrap();
        let prompt_dir = dir.path().join(".harness").join("prompts");
        std::fs::create_dir_all(&prompt_dir).unwrap();
        std::fs::write(prompt_dir.join("tmpl.md"), "hello").unwrap();
        let result = validate_template_path(
            &std::path::PathBuf::from(".harness/prompts/tmpl.md"),
            dir.path(),
        );
        assert!(result.is_ok(), "expected Ok, got {result:?}");
    }

    #[test]
    fn validate_template_path_accepts_root_with_dot_segment() {
        let dir = tempfile::tempdir().unwrap();
        let prompt_dir = dir.path().join(".harness").join("prompts");
        std::fs::create_dir_all(&prompt_dir).unwrap();
        std::fs::write(prompt_dir.join("tmpl.md"), "hello").unwrap();
        let result = validate_template_path(
            &std::path::PathBuf::from(".harness/prompts/tmpl.md"),
            &dir.path().join("."),
        );
        let path = result.unwrap();
        assert_eq!(path, prompt_dir.join("tmpl.md").canonicalize().unwrap());
    }

    #[test]
    fn validate_template_path_rejects_absolute_path() {
        let dir = tempfile::tempdir().unwrap();
        let result = validate_template_path(&std::path::PathBuf::from("/etc/passwd"), dir.path());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("absolute"));
    }

    #[test]
    fn validate_template_path_rejects_path_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let result =
            validate_template_path(&std::path::PathBuf::from("../outside/file.md"), dir.path());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("'..'"));
    }

    // ── resolve_and_guard_path ───────────────────────────────────────────────

    #[test]
    fn resolve_and_guard_path_accepts_root_with_dot_segment() {
        let dir = tempfile::tempdir().unwrap();
        let adr_dir = dir.path().join("knowledge").join("adr");
        std::fs::create_dir_all(&adr_dir).unwrap();
        std::fs::write(adr_dir.join("decision.md"), "decision").unwrap();
        let result = guarded_io::resolve_and_guard_path(
            &dir.path().join("."),
            &std::path::PathBuf::from("knowledge/adr/decision.md"),
            "test",
        );
        let path = result.unwrap();
        assert_eq!(path, adr_dir.join("decision.md").canonicalize().unwrap());
    }

    // ── RefVerifyCacheAdapter ─────────────────────────────────────────────────

    /// Shared roundtrip helper: creates a temp track-items directory, constructs
    /// a [`RefVerifyCacheAdapter`], saves `entries` under `scope`, reloads them,
    /// and asserts equality.  Each call uses a distinct `track_id` so parallel
    /// test runs do not collide.
    fn assert_cache_adapter_roundtrip(
        track_id: &str,
        scope: RefVerifyCacheScope,
        entries: Vec<SemanticVerifyEntry>,
    ) {
        let tmp = tempfile::tempdir().unwrap();
        let items_dir = tmp.path().join("track").join("items").join(track_id);
        std::fs::create_dir_all(&items_dir).unwrap();

        let adapter = RefVerifyCacheAdapter::new(tmp.path().to_path_buf());
        let cmd = track_cmd(track_id);

        adapter.save_entries(&cmd, &scope, entries.clone()).unwrap();
        let loaded = adapter.load_entries(&cmd, &scope).unwrap();
        assert_eq!(loaded, entries);
    }

    #[test]
    fn cache_adapter_load_returns_empty_when_file_absent() {
        let tmp = tempfile::tempdir().unwrap();
        let track_id = "my-track";
        // Create track/items/<track_id>/ directory.
        let items_dir = tmp.path().join("track").join("items").join(track_id);
        std::fs::create_dir_all(&items_dir).unwrap();

        let adapter = RefVerifyCacheAdapter::new(tmp.path().to_path_buf());
        let cmd = track_cmd(track_id);
        let entries = adapter.load_entries(&cmd, &RefVerifyCacheScope::SpecAdr).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn cache_adapter_load_rejects_malformed_spec_adr_cache_json() {
        let tmp = tempfile::tempdir().unwrap();
        let track_id = "my-track-malformed-spec-adr-cache";
        let items_dir = tmp.path().join("track").join("items").join(track_id);
        std::fs::create_dir_all(&items_dir).unwrap();
        std::fs::write(items_dir.join("spec-adr-verify-cache.json"), r#"{"entries":["#).unwrap();

        let adapter = RefVerifyCacheAdapter::new(tmp.path().to_path_buf());
        let cmd = track_cmd(track_id);
        let err = adapter.load_entries(&cmd, &RefVerifyCacheScope::SpecAdr).unwrap_err();

        let usecase::ref_verify::RefVerifyError::CachePersistence { message } = err else {
            panic!("expected CachePersistence for malformed spec-adr cache JSON");
        };
        assert!(message.contains("cannot decode spec-adr-verify-cache"), "{message}");
    }

    #[test]
    fn cache_adapter_save_then_load_roundtrip_for_spec_adr() {
        assert_cache_adapter_roundtrip(
            "my-track-2",
            RefVerifyCacheScope::SpecAdr,
            vec![pass_entry(0x01, 0x02), pass_entry(0x03, 0x04)],
        );
    }

    #[test]
    fn cache_adapter_save_then_load_roundtrip_for_catalogue_spec() {
        let layer = LayerId::try_new("domain".to_owned()).unwrap();
        assert_cache_adapter_roundtrip(
            "my-track-3",
            RefVerifyCacheScope::CatalogueSpec { layer },
            vec![pass_entry(0x0a, 0x0b)],
        );
    }

    #[test]
    fn cache_adapter_spec_adr_and_catalogue_spec_use_separate_files() {
        let tmp = tempfile::tempdir().unwrap();
        let track_id = "my-track-4";
        let items_dir = tmp.path().join("track").join("items").join(track_id);
        std::fs::create_dir_all(&items_dir).unwrap();

        let adapter = RefVerifyCacheAdapter::new(tmp.path().to_path_buf());
        let cmd = track_cmd(track_id);

        let spec_adr_entries = vec![pass_entry(0x01, 0x02)];
        let layer = LayerId::try_new("usecase".to_owned()).unwrap();
        let cat_entries = vec![pass_entry(0x0a, 0x0b)];

        adapter
            .save_entries(&cmd, &RefVerifyCacheScope::SpecAdr, spec_adr_entries.clone())
            .unwrap();
        adapter
            .save_entries(
                &cmd,
                &RefVerifyCacheScope::CatalogueSpec { layer: layer.clone() },
                cat_entries.clone(),
            )
            .unwrap();

        // Verify files are separate.
        assert!(items_dir.join("spec-adr-verify-cache.json").exists());
        assert!(items_dir.join("usecase-catalogue-spec-verify-cache.json").exists());

        // Verify contents are independent.
        let spec_adr_loaded = adapter.load_entries(&cmd, &RefVerifyCacheScope::SpecAdr).unwrap();
        assert_eq!(spec_adr_loaded, spec_adr_entries);
        let cat_loaded =
            adapter.load_entries(&cmd, &RefVerifyCacheScope::CatalogueSpec { layer }).unwrap();
        assert_eq!(cat_loaded, cat_entries);
    }

    #[test]
    fn cache_adapter_load_rejects_malformed_catalogue_spec_cache_json() {
        let tmp = tempfile::tempdir().unwrap();
        let track_id = "my-track-malformed-catalogue-cache";
        let items_dir = tmp.path().join("track").join("items").join(track_id);
        std::fs::create_dir_all(&items_dir).unwrap();
        std::fs::write(
            items_dir.join("domain-catalogue-spec-verify-cache.json"),
            r#"{"layer":"domain","entries":["#,
        )
        .unwrap();
        let layer = LayerId::try_new("domain".to_owned()).unwrap();

        let adapter = RefVerifyCacheAdapter::new(tmp.path().to_path_buf());
        let cmd = track_cmd(track_id);
        let err =
            adapter.load_entries(&cmd, &RefVerifyCacheScope::CatalogueSpec { layer }).unwrap_err();

        let usecase::ref_verify::RefVerifyError::CachePersistence { message } = err else {
            panic!("expected CachePersistence for malformed catalogue-spec cache JSON");
        };
        assert!(message.contains("cannot decode catalogue-spec-verify-cache"), "{message}");
    }

    #[test]
    fn cache_adapter_load_rejects_catalogue_spec_cache_layer_mismatch() {
        let tmp = tempfile::tempdir().unwrap();
        let track_id = "my-track-layer-mismatch";
        let items_dir = tmp.path().join("track").join("items").join(track_id);
        std::fs::create_dir_all(&items_dir).unwrap();
        let stored_layer = LayerId::try_new("domain".to_owned()).unwrap();
        let requested_layer = LayerId::try_new("usecase".to_owned()).unwrap();
        let doc = CatalogueSpecVerifyCacheDocument::new(stored_layer, vec![pass_entry(0x01, 0x02)]);
        let json = CatalogueSpecVerifyCacheDocumentCodec::encode(&doc).unwrap();
        std::fs::write(items_dir.join("usecase-catalogue-spec-verify-cache.json"), json).unwrap();

        let adapter = RefVerifyCacheAdapter::new(tmp.path().to_path_buf());
        let cmd = track_cmd(track_id);
        let err = adapter
            .load_entries(&cmd, &RefVerifyCacheScope::CatalogueSpec { layer: requested_layer })
            .unwrap_err();

        let usecase::ref_verify::RefVerifyError::CachePersistence { message } = err else {
            panic!("expected CachePersistence for catalogue-spec cache layer mismatch");
        };
        assert!(message.contains("layer mismatch"), "{message}");
    }

    #[test]
    fn cache_adapter_save_uses_lock_and_atomic_write() {
        let tmp = tempfile::tempdir().unwrap();
        let track_id = "my-track-5";
        let items_dir = tmp.path().join("track").join("items").join(track_id);
        std::fs::create_dir_all(&items_dir).unwrap();

        let adapter = RefVerifyCacheAdapter::new(tmp.path().to_path_buf());
        let cmd = track_cmd(track_id);

        adapter
            .save_entries(&cmd, &RefVerifyCacheScope::SpecAdr, vec![pass_entry(0x01, 0x02)])
            .unwrap();

        assert!(items_dir.join("spec-adr-verify-cache.json").exists());
        assert!(items_dir.join("spec-adr-verify-cache.json.lock").exists());
        let tmp_entries: Vec<_> = std::fs::read_dir(&items_dir)
            .unwrap()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_name().to_string_lossy().starts_with(".tmp-"))
            .collect();
        assert!(tmp_entries.is_empty(), "unexpected temp files left behind: {tmp_entries:?}");
    }

    // ── RefVerifyPairSourceAdapter::load_pairs — end-to-end ──────────────────

    /// Minimal valid spec.json (schema_version 2, no adr_refs) for use in
    /// `load_pairs` tests that exercise the Chain-1 code path without needing
    /// real ADR files.
    const MINIMAL_SPEC_JSON: &str = r#"{
        "schema_version": 2,
        "version": "0.1",
        "title": "Test spec",
        "goal": [],
        "scope": { "in_scope": [], "out_of_scope": [] },
        "constraints": [],
        "acceptance_criteria": []
    }"#;

    const ARCHITECTURE_RULES_DOMAIN_TDDD: &str = r#"{
        "layers": [
            {
                "crate": "domain",
                "path": "libs/domain",
                "tddd": {
                    "enabled": true,
                    "catalogue_file": "domain-types.json"
                }
            }
        ]
    }"#;

    const ARCHITECTURE_RULES_DOMAIN_CUSTOM_TDDD: &str = r#"{
        "layers": [
            {
                "crate": "domain",
                "path": "libs/domain",
                "tddd": {
                    "enabled": true,
                    "catalogue_file": "semantic-domain-types.json"
                }
            }
        ]
    }"#;

    const ARCHITECTURE_RULES_NO_TDDD: &str = r#"{
        "layers": [
            {
                "crate": "domain",
                "path": "libs/domain",
                "tddd": {
                    "enabled": false
                }
            }
        ]
    }"#;

    fn spec_json_with_adr_ref(file: &str, anchor: &str) -> String {
        serde_json::json!({
            "schema_version": 2,
            "version": "0.1",
            "title": "Test spec",
            "goal": [{
                "id": "GO-01",
                "text": "ADR-backed requirement",
                "adr_refs": [{ "file": file, "anchor": anchor }]
            }],
            "scope": { "in_scope": [], "out_of_scope": [] },
            "constraints": [],
            "acceptance_criteria": []
        })
        .to_string()
    }

    fn write_architecture_rules(tmp: &tempfile::TempDir, architecture_rules: &str) {
        std::fs::write(tmp.path().join("architecture-rules.json"), architecture_rules).unwrap();
    }

    fn ref_verify_adapter_and_cmd(
        tmp: &tempfile::TempDir,
        track_id: &str,
        scope: RefVerifyScope,
    ) -> (RefVerifyPairSourceAdapter, RefVerifyCommand) {
        let adapter = RefVerifyPairSourceAdapter::new(tmp.path().to_path_buf());
        let cmd = RefVerifyCommand {
            track_id: domain::TrackId::try_new(track_id).unwrap(),
            scope,
            current_branch: format!("track/{track_id}"),
        };
        (adapter, cmd)
    }

    fn domain_chain2_scope() -> RefVerifyScope {
        RefVerifyScope::Chain2 { layer: LayerId::try_new("domain".to_owned()).unwrap() }
    }

    fn write_adr_ref_fixture(tmp: &tempfile::TempDir, track_id: &str) {
        let items_dir = track_dir(tmp.path(), track_id);
        let adr_dir = tmp.path().join("knowledge").join("adr");
        std::fs::create_dir_all(&items_dir).unwrap();
        std::fs::create_dir_all(&adr_dir).unwrap();
        std::fs::write(
            items_dir.join("spec.json"),
            spec_json_with_adr_ref("knowledge/adr/decision.md", "D1"),
        )
        .unwrap();
        std::fs::write(adr_dir.join("decision.md"), ADR_WITH_D1).unwrap();
    }

    fn adr_ref_fixture(
        track_id: &str,
        scope: RefVerifyScope,
        architecture_rules: Option<&str>,
    ) -> (tempfile::TempDir, RefVerifyPairSourceAdapter, RefVerifyCommand) {
        let tmp = tempfile::tempdir().unwrap();
        write_adr_ref_fixture(&tmp, track_id);
        if let Some(rules) = architecture_rules {
            write_architecture_rules(&tmp, rules);
        }
        let (adapter, cmd) = ref_verify_adapter_and_cmd(&tmp, track_id, scope);
        (tmp, adapter, cmd)
    }

    fn semantic_thing_spec_json_with_text(text: &str) -> serde_json::Value {
        serde_json::json!({
            "schema_version": 2,
            "version": "0.1",
            "title": "Test spec",
            "goal": [{
                "id": "IN-01",
                "text": text,
                "adr_refs": []
            }],
            "scope": { "in_scope": [], "out_of_scope": [] },
            "constraints": [],
            "acceptance_criteria": []
        })
    }

    fn semantic_thing_spec_json() -> serde_json::Value {
        semantic_thing_spec_json_with_text("The semantic thing preserves catalogue entry meaning")
    }

    fn semantic_thing_catalogue_json_with_spec_file(
        spec_file: &str,
        docs: &str,
    ) -> serde_json::Value {
        serde_json::json!({
            "schema_version": 3,
            "crate_name": "domain",
            "layer": "domain",
            "types": {
                "SemanticThing": {
                    "action": "add",
                    "role": "ValueObject",
                    "kind": { "kind": "struct", "shape": { "kind": "unit" } },
                    "methods": [],
                    "module_path": "",
                    "docs": docs,
                    "spec_refs": [{
                        "file": spec_file,
                        "anchor": "IN-01",
                        "hash": "0000000000000000000000000000000000000000000000000000000000000000"
                    }],
                    "informal_grounds": []
                }
            },
            "traits": {},
            "functions": {}
        })
    }

    fn semantic_thing_catalogue_json(track_id: &str, docs: &str) -> serde_json::Value {
        semantic_thing_catalogue_json_with_spec_file(
            &format!("track/items/{track_id}/spec.json"),
            docs,
        )
    }

    fn load_chain2_semantic_thing_pair(
        track_id: &str,
        catalogue_file: &str,
        architecture_rules: &str,
        docs: &str,
    ) -> RefVerifyPair {
        let tmp = tempfile::tempdir().unwrap();
        let items_dir = track_dir(tmp.path(), track_id);
        std::fs::create_dir_all(&items_dir).unwrap();
        std::fs::write(items_dir.join("spec.json"), semantic_thing_spec_json().to_string())
            .unwrap();
        std::fs::write(
            items_dir.join(catalogue_file),
            semantic_thing_catalogue_json(track_id, docs).to_string(),
        )
        .unwrap();
        write_architecture_rules(&tmp, architecture_rules);

        let (adapter, cmd) = ref_verify_adapter_and_cmd(&tmp, track_id, domain_chain2_scope());
        let pairs = adapter.load_pairs(&cmd, &RefVerifyConfig::default()).unwrap();
        pairs.into_iter().find(|pair| !pair.known_bad).unwrap()
    }

    const ADR_WITH_D1: &str = r#"---
adr_id: test-adr
decisions:
  - id: D1
    status: proposed
    candidate_selection: "choose the guarded path"
---
# ADR

### D1: Guarded path decision
The guarded path must stay inside the trusted repository root.
"#;

    #[test]
    fn load_pairs_chain1_valid_ref_produces_pair_and_known_bad_probe() {
        let track_id = "test-load-pairs-chain1-valid";
        let (_tmp, adapter, cmd) = adr_ref_fixture(track_id, RefVerifyScope::Chain1, None);
        let config = usecase::ref_verify::RefVerifyConfig::try_new(100, 90, 1).unwrap();

        let pairs = adapter.load_pairs(&cmd, &config).unwrap();

        assert_eq!(pairs.len(), 2);
        let real_pair = pairs.iter().find(|pair| !pair.known_bad).unwrap();
        let probe = pairs.iter().find(|pair| pair.known_bad).unwrap();

        assert_eq!(real_pair.claim, "[goal GO-01] ADR-backed requirement");
        assert!(real_pair.evidence.contains("ADR decision [D1]"));
        assert!(real_pair.evidence.contains("Guarded path decision"));
        assert_eq!(
            real_pair.claim_hash,
            pair_source::hash_text(
                r#"{"adr_refs":[{"anchor":"D1","file":"knowledge/adr/decision.md"}],"id":"GO-01","text":"ADR-backed requirement"}"#,
            )
            .unwrap()
        );
        assert_eq!(real_pair.evidence_hash, pair_source::hash_git_blob_text(ADR_WITH_D1));
        assert_eq!(real_pair.cache_scope, RefVerifyCacheScope::SpecAdr);
        assert_eq!(probe.cache_scope, RefVerifyCacheScope::SpecAdr);
    }

    #[test]
    fn load_pairs_chain1_with_no_adr_refs_returns_empty_no_probes() {
        let tmp = tempfile::tempdir().unwrap();
        let track_id = "test-load-pairs";
        let items_dir = tmp.path().join("track").join("items").join(track_id);
        std::fs::create_dir_all(&items_dir).unwrap();
        std::fs::write(items_dir.join("spec.json"), MINIMAL_SPEC_JSON).unwrap();

        let adapter = RefVerifyPairSourceAdapter::new(tmp.path().to_path_buf());
        let cmd = RefVerifyCommand {
            track_id: domain::TrackId::try_new(track_id).unwrap(),
            scope: RefVerifyScope::Chain1,
            current_branch: format!("track/{track_id}"),
        };
        // Default config: 10% injection rate — with 0 real pairs, probe_count=0.
        let config = usecase::ref_verify::RefVerifyConfig::default();
        let pairs = adapter.load_pairs(&cmd, &config).unwrap();
        // No adr_refs in spec → no Chain-1 pairs; 0 pairs × 10% → 0 probes.
        assert!(pairs.is_empty(), "expected empty pairs, got {}", pairs.len());
    }

    #[test]
    fn load_pairs_all_scope_with_missing_architecture_rules_returns_chain1_only() {
        // No architecture-rules.json → no TDDD layers → zero Chain-2 pairs.
        // The scope resolver returns All (empty TDDD set), and the pair source must
        // mirror that behaviour by contributing only Chain-1 pairs rather than erroring.
        let (_tmp, adapter, cmd) =
            adr_ref_fixture("test-load-pairs-all-no-rules", RefVerifyScope::All, None);
        let pairs = adapter.load_pairs(&cmd, &RefVerifyConfig::default()).unwrap();

        // Chain-1 pairs from the ADR ref fixture must be present.
        assert!(pairs.iter().any(|p| !p.known_bad && p.claim.contains("[goal GO-01]")));
        // No Chain-2 pairs because no architecture-rules.json was provided.
        assert!(
            !pairs
                .iter()
                .any(|p| matches!(p.cache_scope, RefVerifyCacheScope::CatalogueSpec { .. }))
        );
    }

    #[test]
    fn load_pairs_all_scope_with_no_tddd_layers_preserves_chain1_pairs() {
        let (_tmp, adapter, cmd) = adr_ref_fixture(
            "test-load-pairs-all-chain1-only",
            RefVerifyScope::All,
            Some(ARCHITECTURE_RULES_NO_TDDD),
        );
        let pairs = adapter.load_pairs(&cmd, &RefVerifyConfig::default()).unwrap();

        assert!(pairs.iter().any(|pair| !pair.known_bad && pair.claim.contains("[goal GO-01]")));
    }

    #[test]
    fn load_pairs_all_scope_with_absent_catalogue_returns_chain1_only_pre_phase2_path() {
        // Pre-Phase-2 path: architecture-rules.json declares a TDDD layer but the
        // catalogue file does not exist yet. The scope resolver permits this as an
        // all-absent (pre-Phase-2) run; the pair source must contribute zero Chain-2
        // pairs rather than erroring. Chain-1 pairs from spec/ADR refs are still
        // produced.
        let (_tmp, adapter, cmd) = adr_ref_fixture(
            "test-load-pairs-chain2",
            RefVerifyScope::All,
            Some(ARCHITECTURE_RULES_DOMAIN_TDDD),
        );
        let config = usecase::ref_verify::RefVerifyConfig::default();
        let pairs = adapter.load_pairs(&cmd, &config).unwrap();

        // Chain-1 pairs from the ADR ref fixture must be present.
        assert!(pairs.iter().any(|p| !p.known_bad && p.claim.contains("[goal GO-01]")));
        // No Chain-2 pairs: domain catalogue was absent (pre-Phase-2).
        assert!(
            !pairs
                .iter()
                .any(|p| matches!(p.cache_scope, RefVerifyCacheScope::CatalogueSpec { .. }))
        );
    }

    #[test]
    fn load_pairs_all_scope_with_absent_spec_returns_zero_chain1_pairs_phase0_path() {
        // Phase 0 path (AC-01 / AC-02): spec.json does not exist yet, so Chain-1
        // contributes zero pairs instead of erroring. With no catalogue either,
        // the whole pair set is empty and no known-bad probe is injected.
        let tmp = tempfile::tempdir().unwrap();
        let track_id = "test-load-pairs-phase0-no-spec";
        let items_dir = track_dir(tmp.path(), track_id);
        std::fs::create_dir_all(&items_dir).unwrap();
        write_architecture_rules(&tmp, ARCHITECTURE_RULES_NO_TDDD);

        let (adapter, cmd) = ref_verify_adapter_and_cmd(&tmp, track_id, RefVerifyScope::All);
        let pairs = adapter.load_pairs(&cmd, &RefVerifyConfig::default()).unwrap();

        assert!(pairs.is_empty(), "Phase 0 must produce zero pairs, got: {pairs:?}");
    }

    #[test]
    fn load_pairs_with_regular_file_at_track_dir_path_fails_closed() {
        // The track directory guard must reject a regular file at the track path.
        // Before the is_dir() fix, a regular file at the track path would pass the
        // symlink check (Ok(true)), fail to find spec.json (not a directory), and
        // silently return zero pairs. Now it must fail with a VerifierPort error.
        let tmp = tempfile::tempdir().unwrap();
        let track_id = "test-load-pairs-track-dir-is-file";
        let items_dir = tmp.path().join("track").join("items");
        std::fs::create_dir_all(&items_dir).unwrap();
        // Write a regular file at the path that should be a directory.
        std::fs::write(items_dir.join(track_id), "not a directory").unwrap();
        write_architecture_rules(&tmp, ARCHITECTURE_RULES_NO_TDDD);

        let (adapter, cmd) = ref_verify_adapter_and_cmd(&tmp, track_id, RefVerifyScope::All);
        let err = adapter.load_pairs(&cmd, &RefVerifyConfig::default()).unwrap_err();

        assert!(
            matches!(err, RefVerifyError::VerifierPort { ref message } if message.contains("not a directory")),
            "regular file at track path must fail closed via VerifierPort, got: {err:?}"
        );
    }

    #[test]
    fn load_pairs_all_scope_with_malformed_spec_fails_closed() {
        // Present-but-broken spec.json (IN-07 / AC-10): the absence skip applies
        // only to a missing file; a malformed file still fails closed.
        let tmp = tempfile::tempdir().unwrap();
        let track_id = "test-load-pairs-malformed-spec";
        let items_dir = track_dir(tmp.path(), track_id);
        std::fs::create_dir_all(&items_dir).unwrap();
        std::fs::write(items_dir.join("spec.json"), "{not valid json").unwrap();
        write_architecture_rules(&tmp, ARCHITECTURE_RULES_NO_TDDD);

        let (adapter, cmd) = ref_verify_adapter_and_cmd(&tmp, track_id, RefVerifyScope::All);
        let err = adapter.load_pairs(&cmd, &RefVerifyConfig::default()).unwrap_err();

        assert!(
            matches!(err, RefVerifyError::VerifierPort { ref message } if message.contains("spec.json")),
            "malformed spec.json must fail closed via VerifierPort, got: {err:?}"
        );
    }

    #[test]
    fn load_pairs_chain2_with_missing_architecture_rules_fails_closed() {
        let tmp = tempfile::tempdir().unwrap();
        let track_id = "test-load-pairs-chain2-no-rules";
        let items_dir = track_dir(tmp.path(), track_id);
        std::fs::create_dir_all(&items_dir).unwrap();
        std::fs::write(items_dir.join("spec.json"), MINIMAL_SPEC_JSON).unwrap();

        let (adapter, cmd) = ref_verify_adapter_and_cmd(&tmp, track_id, domain_chain2_scope());
        let err = adapter.load_pairs(&cmd, &RefVerifyConfig::default()).unwrap_err();

        assert!(
            matches!(err, RefVerifyError::VerifierPort { message } if message.contains("architecture-rules.json"))
        );
    }

    #[test]
    fn load_pairs_chain2_with_configured_catalogue_file_uses_custom_catalogue() {
        let real_pair = load_chain2_semantic_thing_pair(
            "test-load-pairs-chain2-custom-catalogue",
            "semantic-domain-types.json",
            ARCHITECTURE_RULES_DOMAIN_CUSTOM_TDDD,
            "custom catalogue docs that model must evaluate",
        );

        assert!(real_pair.claim.contains("[types:SemanticThing] SemanticThing"));
        assert!(real_pair.claim.contains("custom catalogue docs that model must evaluate"));
    }

    #[test]
    fn load_pairs_chain2_claim_includes_catalogue_entry_semantics() {
        let real_pair = load_chain2_semantic_thing_pair(
            "test-load-pairs-chain2-claim",
            "domain-types.json",
            ARCHITECTURE_RULES_DOMAIN_TDDD,
            "semantic docs that model must evaluate",
        );

        assert!(real_pair.claim.contains("[types:SemanticThing] SemanticThing"));
        assert!(real_pair.claim.contains("Catalogue entry canonical JSON:"));
        assert!(real_pair.claim.contains("\"docs\":\"semantic docs that model must evaluate\""));
        assert!(real_pair.claim.contains("\"role\":\"ValueObject\""));
    }

    #[test]
    fn load_pairs_chain2_uses_each_spec_ref_file_as_evidence_source() {
        let tmp = tempfile::tempdir().unwrap();
        let track_id = "test-load-pairs-chain2-spec-ref-file";
        let items_dir = track_dir(tmp.path(), track_id);
        let referenced_spec = format!("track/items/{track_id}/referenced-spec.json");
        std::fs::create_dir_all(&items_dir).unwrap();
        std::fs::write(
            items_dir.join("spec.json"),
            semantic_thing_spec_json_with_text("wrong active track spec text").to_string(),
        )
        .unwrap();
        std::fs::write(
            items_dir.join("referenced-spec.json"),
            semantic_thing_spec_json_with_text("referenced spec evidence text").to_string(),
        )
        .unwrap();
        std::fs::write(
            items_dir.join("domain-types.json"),
            semantic_thing_catalogue_json_with_spec_file(
                &referenced_spec,
                "semantic docs that model must evaluate",
            )
            .to_string(),
        )
        .unwrap();
        write_architecture_rules(&tmp, ARCHITECTURE_RULES_DOMAIN_TDDD);

        let (adapter, cmd) = ref_verify_adapter_and_cmd(&tmp, track_id, domain_chain2_scope());
        let pairs = adapter.load_pairs(&cmd, &RefVerifyConfig::default()).unwrap();
        let real_pair = pairs.into_iter().find(|pair| !pair.known_bad).unwrap();

        assert!(real_pair.evidence.contains("referenced spec evidence text"));
        assert!(!real_pair.evidence.contains("wrong active track spec text"));
        assert_eq!(
            real_pair.evidence_hash,
            pair_source::hash_text(
                r#"{"adr_refs":[],"id":"IN-01","text":"referenced spec evidence text"}"#
            )
            .unwrap()
        );
    }

    #[test]
    fn load_pairs_injects_probes_proportional_to_pair_count() {
        // With 0 real pairs, probe injection is 0.  With non-zero injection rate
        // and more pairs (not easy to create without full ADR fixtures), we at
        // least verify calculate_probe_count integration via a direct call.
        assert_eq!(pair_source::calculate_probe_count(10, 10), 1);
        assert_eq!(pair_source::calculate_probe_count(10, 20), 2);
        assert_eq!(pair_source::calculate_probe_count(10, 100), 10);
        // 0 pairs → always 0 probes regardless of rate.
        assert_eq!(pair_source::calculate_probe_count(0, 100), 0);
    }

    #[test]
    fn load_pairs_all_scope_with_both_chains_injects_probe_per_exercised_verifier() {
        // AC-09 / D5: when an All-scope run exercises BOTH chain verifiers
        // (Chain1 and Chain2 production pairs present), each chain capability
        // must receive at least one known-bad calibration probe — a single
        // probe routed to only one chain would leave the other chain's
        // verifier uncalibrated while its production pairs are still trusted.
        let track_id = "test-load-pairs-all-both-chain-probes";
        let tmp = tempfile::tempdir().unwrap();
        // Chain1 fixture: spec.json with an adr_ref + the referenced ADR.
        write_adr_ref_fixture(&tmp, track_id);
        // Chain2 fixture: a TDDD domain catalogue referencing the same spec.
        // The adr_ref fixture's spec lacks the IN-01 anchor, so point the
        // catalogue at a dedicated referenced spec file instead.
        let items_dir = track_dir(tmp.path(), track_id);
        let referenced_spec = format!("track/items/{track_id}/referenced-spec.json");
        std::fs::write(
            items_dir.join("referenced-spec.json"),
            semantic_thing_spec_json().to_string(),
        )
        .unwrap();
        std::fs::write(
            items_dir.join("domain-types.json"),
            semantic_thing_catalogue_json_with_spec_file(
                &referenced_spec,
                "semantic docs that model must evaluate",
            )
            .to_string(),
        )
        .unwrap();
        write_architecture_rules(&tmp, ARCHITECTURE_RULES_DOMAIN_TDDD);

        let (adapter, cmd) = ref_verify_adapter_and_cmd(&tmp, track_id, RefVerifyScope::All);
        // Default config: 10% injection over 2 production pairs would yield a
        // single probe without the per-verifier guarantee.
        let pairs = adapter.load_pairs(&cmd, &RefVerifyConfig::default()).unwrap();

        let probes: Vec<_> = pairs.iter().filter(|p| p.known_bad).collect();
        assert!(
            probes.iter().any(|p| matches!(p.cache_scope, RefVerifyCacheScope::SpecAdr)),
            "Chain1 verifier must receive at least one calibration probe; probes: {:?}",
            probes.iter().map(|p| &p.cache_scope).collect::<Vec<_>>()
        );
        assert!(
            probes
                .iter()
                .any(|p| matches!(p.cache_scope, RefVerifyCacheScope::CatalogueSpec { .. })),
            "Chain2 verifier must receive at least one calibration probe; probes: {:?}",
            probes.iter().map(|p| &p.cache_scope).collect::<Vec<_>>()
        );
    }

    #[cfg(unix)]
    #[test]
    fn enumerate_chain2_all_layers_rejects_symlinked_track_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let track_id = "test-all-layers-symlinked-track-dir";
        let items_parent = tmp.path().join("track").join("items");
        std::fs::create_dir_all(&items_parent).unwrap();
        std::fs::write(outside.path().join("domain-types.json"), "[]").unwrap();
        let track_dir = items_parent.join(track_id);
        std::os::unix::fs::symlink(outside.path(), &track_dir).unwrap();

        let err =
            pair_source_chain2::enumerate_chain2_all_layers(&track_dir, tmp.path()).unwrap_err();
        let usecase::ref_verify::RefVerifyError::VerifierPort { message } = err else {
            panic!("expected VerifierPort for symlinked track directory");
        };
        assert!(message.contains("cannot open track directory"), "{message}");
    }

    #[test]
    fn load_pairs_rejects_adr_ref_path_traversal_at_adapter_boundary() {
        let tmp = tempfile::tempdir().unwrap();
        let track_id = "test-load-pairs-path-traversal";
        let items_dir = tmp.path().join("track").join("items").join(track_id);
        std::fs::create_dir_all(&items_dir).unwrap();
        std::fs::write(items_dir.join("spec.json"), spec_json_with_adr_ref("../outside.md", "D1"))
            .unwrap();

        let adapter = RefVerifyPairSourceAdapter::new(tmp.path().to_path_buf());
        let cmd = RefVerifyCommand {
            track_id: domain::TrackId::try_new(track_id).unwrap(),
            scope: RefVerifyScope::Chain1,
            current_branch: format!("track/{track_id}"),
        };
        let config = usecase::ref_verify::RefVerifyConfig::default();

        let err = adapter.load_pairs(&cmd, &config).unwrap_err();
        let usecase::ref_verify::RefVerifyError::VerifierPort { message } = err else {
            panic!("expected VerifierPort for rejected adr_ref path");
        };
        assert!(
            message.contains("invalid path") && message.contains("path-traversal"),
            "{message}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn load_pairs_rejects_symlinked_adr_artifact_at_adapter_boundary() {
        let tmp = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let track_id = "test-load-pairs-symlinked-adr";
        let items_dir = tmp.path().join("track").join("items").join(track_id);
        let adr_dir = tmp.path().join("knowledge").join("adr");
        std::fs::create_dir_all(&items_dir).unwrap();
        std::fs::create_dir_all(&adr_dir).unwrap();
        let outside_adr = outside.path().join("outside.md");
        std::fs::write(&outside_adr, ADR_WITH_D1).unwrap();
        std::os::unix::fs::symlink(&outside_adr, adr_dir.join("linked.md")).unwrap();
        std::fs::write(
            items_dir.join("spec.json"),
            spec_json_with_adr_ref("knowledge/adr/linked.md", "D1"),
        )
        .unwrap();

        let adapter = RefVerifyPairSourceAdapter::new(tmp.path().to_path_buf());
        let cmd = RefVerifyCommand {
            track_id: domain::TrackId::try_new(track_id).unwrap(),
            scope: RefVerifyScope::Chain1,
            current_branch: format!("track/{track_id}"),
        };
        let config = usecase::ref_verify::RefVerifyConfig::default();

        let err = adapter.load_pairs(&cmd, &config).unwrap_err();
        let usecase::ref_verify::RefVerifyError::VerifierPort { message } = err else {
            panic!("expected VerifierPort for rejected symlinked ADR");
        };
        assert!(
            message.contains("cannot read ADR") || message.contains("cannot open"),
            "{message}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn cache_adapter_save_rejects_symlinked_cache_directory_at_adapter_boundary() {
        let tmp = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let track_id = "test-cache-symlinked-dir";
        let items_parent = tmp.path().join("track").join("items");
        std::fs::create_dir_all(&items_parent).unwrap();
        std::os::unix::fs::symlink(outside.path(), items_parent.join(track_id)).unwrap();

        let adapter = RefVerifyCacheAdapter::new(tmp.path().to_path_buf());
        let cmd = track_cmd(track_id);
        let err = adapter
            .save_entries(&cmd, &RefVerifyCacheScope::SpecAdr, vec![pass_entry(0x01, 0x02)])
            .unwrap_err();

        assert!(
            matches!(err, usecase::ref_verify::RefVerifyError::CachePersistence { .. }),
            "expected CachePersistence for rejected symlinked cache directory, got {err:?}"
        );
        assert!(!outside.path().join("spec-adr-verify-cache.json").exists());
    }
}
