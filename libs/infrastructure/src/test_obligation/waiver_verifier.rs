//! Capability adapter for the waiver semantic verifier.
//!
//! Judges whether an implementer-authored waiver reason justifies leaving an
//! obligation edge untested — a natural language → natural language comparison,
//! separated from obligation-fulfillment into its own capability and prompt
//! (ADR D8). The provider is resolved from the `waiver-verifier` capability in
//! `agent-profiles.json` (IN-09 / IN-15 / CN-08); the judgement is edge-local
//! (the reason must be self-contained for this edge, ADR D6 / OS-04).
//!
//! Fail-closed decoding: a `pass` without a citation and a `fail` without a
//! reason are both rejected at the codec boundary (OS-01). A `pending` verdict
//! is preserved and treated as fail at the gate by the caller.

use std::sync::Arc;

use domain::EvidenceCitation;
use domain::ModelTier;
use domain::tddd::test_obligation::errors::SemanticVerifierError;
use domain::tddd::test_obligation::hashes::VerifierPromptFingerprint;
use domain::tddd::test_obligation::ids::DiagnosticMessage;
use domain::tddd::test_obligation::ports::WaiverVerifierPort;
use domain::tddd::test_obligation::verdict::WaiverVerdict;
use usecase::test_obligation::hasher::ContentHasherPort;

use crate::agent_profiles::AgentProfiles;
use crate::test_obligation::diagnostic;
use crate::test_obligation::semantic_verifier::{
    SemanticVerifierRunner, VerdictKindWire, default_waiver_verifier_runner, extract_verdict_json,
    resolve_execution_or_err, semantic_verifier_error, tier_to_round_type,
};
use crate::test_obligation::sha256_content_hasher::Sha256ContentHasher;

/// Capability name resolved from `agent-profiles.json` for this verifier.
const CAPABILITY: &str = "waiver-verifier";

/// Instruction preamble prepended to every waiver prompt.
///
/// Encodes the edge-local, citation-required discipline for waiver judgement.
const WAIVER_PROMPT_PREAMBLE: &str = "\
You are a waiver verifier. Decide whether the implementer-authored waiver reason justifies \
leaving THIS obligation edge (the cited anchor as it relates to the given catalogue entry) \
untested. Judge only the reason against this edge; the reason must be self-contained and must \
not rely on other edges.

Reply with exactly one JSON object and nothing else:
{\"kind\": \"pass\" | \"fail\" | \"pending\", \"citation\": string | null, \"reason\": string | null}

- \"pass\": the waiver holds for this edge. \"citation\" MUST quote verbatim the part of the \
entry declaration or anchor that supports the waiver reason. A pass without a citation is invalid.
- \"fail\": the waiver reason does not hold for this edge; set \"reason\" to a human-readable \
explanation of why it was rejected.
- \"pending\": you cannot confirm the waiver from the material provided.

Representative acceptable reasons: the anchor promises no verifiable behaviour for this entry \
(design rationale), the structure is guaranteed by the type system, or the property is enforced \
by a deterministic verify gate.";

/// Returns the SHA-256 content hash of this verifier's judging prompt preamble.
///
/// The hash deliberately excludes execution provider, model, and tier: it
/// invalidates cache entries only when the judging instructions change.
#[must_use]
pub fn waiver_verifier_fingerprint() -> VerifierPromptFingerprint {
    VerifierPromptFingerprint::new(
        Sha256ContentHasher::new().sha256(WAIVER_PROMPT_PREAMBLE.as_bytes()),
    )
}

/// Capability adapter that resolves the waiver verifier provider and delegates
/// the semantic judgement to it (IN-09 / IN-15 / AC-07 / CN-08).
pub struct WaiverVerifierAdapter {
    agent_profile: AgentProfiles,
    runner: Arc<SemanticVerifierRunner>,
}

impl WaiverVerifierAdapter {
    /// Builds an adapter that spawns the configured provider subprocess for each
    /// verification.
    #[must_use]
    pub fn new(agent_profile: AgentProfiles) -> Self {
        Self { agent_profile, runner: default_waiver_verifier_runner() }
    }

    /// Test-only constructor injecting a stubbed provider runner so unit tests
    /// exercise the verdict-decoding path without spawning a subprocess.
    #[cfg(test)]
    fn with_runner(agent_profile: AgentProfiles, runner: Arc<SemanticVerifierRunner>) -> Self {
        Self { agent_profile, runner }
    }

    fn render_prompt(waived_reason: &str, entry_declaration: &str, anchor_text: &str) -> String {
        format!(
            "{WAIVER_PROMPT_PREAMBLE}\n\n\
             ## Entry declaration\n{entry_declaration}\n\n\
             ## Anchor promise\n{anchor_text}\n\n\
             ## Waiver reason\n{waived_reason}\n"
        )
    }
}

/// Fail-closed verifier used when the configured waiver capability cannot load.
#[derive(Debug, Clone)]
pub struct FailingWaiverVerifier {
    message: DiagnosticMessage,
}

impl FailingWaiverVerifier {
    /// Builds a verifier that always returns the supplied adapter failure.
    #[must_use]
    pub fn new(message: DiagnosticMessage) -> Self {
        Self { message }
    }

    /// Builds a verifier failure from text, substituting a bounded diagnostic if needed.
    #[must_use]
    pub fn from_message(message: &str) -> Self {
        Self::new(diagnostic(message))
    }
}

impl WaiverVerifierPort for FailingWaiverVerifier {
    fn verify_pair(
        &self,
        _waived_reason: &str,
        _entry_declaration: &str,
        _anchor_text: &str,
        _tier: ModelTier,
    ) -> Result<WaiverVerdict, SemanticVerifierError> {
        Err(SemanticVerifierError::VerifierPort(self.message.clone()))
    }
}

/// Wire form of the waiver verdict returned by the provider.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct WaiverVerdictDto {
    kind: VerdictKindWire,
    citation: Option<String>,
    reason: Option<String>,
}

fn map_verdict(dto: WaiverVerdictDto) -> Result<WaiverVerdict, SemanticVerifierError> {
    match dto.kind {
        VerdictKindWire::Pass => {
            let citation =
                EvidenceCitation::try_new(dto.citation.unwrap_or_default()).map_err(|e| {
                    semantic_verifier_error(&format!(
                        "waived verdict without a valid citation was rejected: {e}"
                    ))
                })?;
            Ok(WaiverVerdict::Waived { citation })
        }
        VerdictKindWire::Fail => {
            let reason_text = dto
                .reason
                .filter(|r| !r.trim().is_empty())
                .ok_or_else(|| semantic_verifier_error("fail verdict missing required reason"))?;
            Ok(WaiverVerdict::Fail { reason: diagnostic(&reason_text) })
        }
        VerdictKindWire::Pending => Ok(WaiverVerdict::Pending),
    }
}

impl WaiverVerifierPort for WaiverVerifierAdapter {
    fn verify_pair(
        &self,
        waived_reason: &str,
        entry_declaration: &str,
        anchor_text: &str,
        tier: ModelTier,
    ) -> Result<WaiverVerdict, SemanticVerifierError> {
        let round = tier_to_round_type(tier);
        let resolved = resolve_execution_or_err(&self.agent_profile, CAPABILITY, round)?;
        let prompt = Self::render_prompt(waived_reason, entry_declaration, anchor_text);
        let raw = (self.runner)(resolved, prompt)?;
        let dto: WaiverVerdictDto = extract_verdict_json(&raw)?;
        map_verdict(dto)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use std::sync::Mutex;

    use crate::agent_profiles::ResolvedExecution;

    use super::*;

    const CONFIG: &str = r#"{
        "schema_version": 1,
        "providers": { "claude": { "label": "Claude" } },
        "capabilities": {
            "waiver-verifier": {
                "provider": "claude",
                "model": "claude-opus-4-8",
                "fast_provider": "claude",
                "fast_model": "claude-haiku-4-5"
            }
        }
    }"#;

    fn profiles() -> AgentProfiles {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("agent-profiles.json");
        std::fs::write(&path, CONFIG).unwrap();
        AgentProfiles::load(&path).unwrap()
    }

    fn stub_runner(output: &'static str) -> Arc<SemanticVerifierRunner> {
        Arc::new(move |_resolved, _prompt| Ok(output.to_owned()))
    }

    fn adapter(output: &'static str) -> WaiverVerifierAdapter {
        WaiverVerifierAdapter::with_runner(profiles(), stub_runner(output))
    }

    #[test]
    fn failing_verifier_returns_verifier_port_error() {
        let verifier = FailingWaiverVerifier::from_message("profile missing");

        let err = verifier.verify_pair("reason", "entry", "anchor", ModelTier::Fast).unwrap_err();

        let SemanticVerifierError::VerifierPort(message) = err;
        assert_eq!(message.as_str(), "profile missing");
    }

    #[test]
    fn test_failing_waiver_verifier_for_each_pair_returns_fail_closed_port_error() {
        let verifier = FailingWaiverVerifier::from_message("profile unavailable");

        for (waived_reason, entry_declaration, anchor_text, tier) in [
            ("reason", "struct Entry", "anchor promise", ModelTier::Fast),
            ("another reason", "trait Port", "different anchor promise", ModelTier::Final),
        ] {
            let err = verifier
                .verify_pair(waived_reason, entry_declaration, anchor_text, tier)
                .unwrap_err();

            let SemanticVerifierError::VerifierPort(message) = err;
            assert_eq!(message.as_str(), "profile unavailable");
        }
    }

    #[test]
    fn pass_verdict_decodes_to_waived_with_citation() {
        let verdict = adapter(
            r#"{"kind":"pass","citation":"the anchor states this is a design goal","reason":null}"#,
        )
        .verify_pair(
            "this anchor is a design goal with no verifiable behaviour",
            "struct Foo",
            "GO: state the purpose",
            ModelTier::Final,
        )
        .unwrap();
        match verdict {
            WaiverVerdict::Waived { citation } => {
                assert_eq!(citation.as_str(), "the anchor states this is a design goal");
            }
            other => panic!("expected Waived, got {other:?}"),
        }
    }

    #[test]
    fn test_waiver_adapter_pair_uses_waiver_capability_and_citation() {
        let captured: Arc<Mutex<Option<(ResolvedExecution, String)>>> = Arc::new(Mutex::new(None));
        let captured_clone = Arc::clone(&captured);
        let runner: Arc<SemanticVerifierRunner> = Arc::new(move |resolved, prompt| {
            *captured_clone.lock().unwrap() = Some((resolved, prompt));
            Ok(r#"{"kind":"pass","citation":"the anchor is a design goal","reason":null}"#
                .to_owned())
        });
        let adapter = WaiverVerifierAdapter::with_runner(profiles(), runner);

        let verdict = adapter
            .verify_pair(
                "this design goal has no independently observable behaviour",
                "struct Entry",
                "the design goal explains the purpose",
                ModelTier::Final,
            )
            .unwrap();

        assert!(matches!(verdict, WaiverVerdict::Waived { .. }));
        let (resolved, prompt) = captured.lock().unwrap().clone().unwrap();
        assert_eq!(resolved.model.as_deref(), Some("claude-opus-4-8"));
        assert!(prompt.contains("waiver verifier"));
        assert!(prompt.contains("this design goal has no independently observable behaviour"));
        assert!(prompt.contains("struct Entry"));
        assert!(prompt.contains("the design goal explains the purpose"));
    }

    #[test]
    fn fail_verdict_decodes_with_reason() {
        let verdict = adapter(r#"{"kind":"fail","citation":null,"reason":"the anchor promises verifiable behaviour"}"#)
            .verify_pair("reason", "decl", "anchor", ModelTier::Final)
            .unwrap();
        match verdict {
            WaiverVerdict::Fail { reason } => {
                assert_eq!(reason.as_str(), "the anchor promises verifiable behaviour");
            }
            other => panic!("expected Fail, got {other:?}"),
        }
    }

    #[test]
    fn pass_without_citation_fails_closed() {
        let err = adapter(r#"{"kind":"pass","citation":null,"reason":null}"#)
            .verify_pair("reason", "decl", "anchor", ModelTier::Final)
            .unwrap_err();
        let SemanticVerifierError::VerifierPort(message) = err;
        assert!(message.as_str().contains("citation"));
    }

    #[test]
    fn fail_without_reason_fails_closed() {
        let err = adapter(r#"{"kind":"fail","citation":null,"reason":"  "}"#)
            .verify_pair("reason", "decl", "anchor", ModelTier::Final)
            .unwrap_err();
        let SemanticVerifierError::VerifierPort(message) = err;
        assert!(message.as_str().contains("reason"));
    }

    #[test]
    fn pending_verdict_is_preserved() {
        let verdict = adapter(r#"{"kind":"pending","citation":null,"reason":null}"#)
            .verify_pair("reason", "decl", "anchor", ModelTier::Final)
            .unwrap();
        assert_eq!(verdict, WaiverVerdict::Pending);
    }

    #[test]
    fn unknown_field_fails_closed() {
        let err = adapter(r#"{"kind":"pass","citation":"c","reason":null,"category":"x"}"#)
            .verify_pair("reason", "decl", "anchor", ModelTier::Final)
            .unwrap_err();
        let SemanticVerifierError::VerifierPort(message) = err;
        assert!(message.as_str().contains("verdict JSON object"));
    }

    #[test]
    fn missing_capability_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("agent-profiles.json");
        std::fs::write(&path, r#"{ "schema_version": 1, "providers": {}, "capabilities": {} }"#)
            .unwrap();
        let profile = AgentProfiles::load(&path).unwrap();
        let err = WaiverVerifierAdapter::with_runner(profile, stub_runner("{}"))
            .verify_pair("reason", "decl", "anchor", ModelTier::Final)
            .unwrap_err();
        let SemanticVerifierError::VerifierPort(message) = err;
        assert!(message.as_str().contains("waiver-verifier"));
    }

    #[test]
    fn fast_tier_resolves_fast_model() {
        let captured: Arc<Mutex<Option<ResolvedExecution>>> = Arc::new(Mutex::new(None));
        let captured_clone = Arc::clone(&captured);
        let runner: Arc<SemanticVerifierRunner> = Arc::new(move |resolved, _prompt| {
            *captured_clone.lock().unwrap() = Some(resolved);
            Ok(r#"{"kind":"pending","citation":null,"reason":null}"#.to_owned())
        });
        let adapter = WaiverVerifierAdapter::with_runner(profiles(), runner);
        adapter.verify_pair("reason", "decl", "anchor", ModelTier::Fast).unwrap();
        let resolved = captured.lock().unwrap().clone().unwrap();
        assert_eq!(resolved.model.as_deref(), Some("claude-haiku-4-5"));
    }

    #[test]
    fn render_prompt_embeds_all_three_pair_components() {
        let prompt =
            WaiverVerifierAdapter::render_prompt("REASON_MARKER", "DECL_MARKER", "ANCHOR_MARKER");
        assert!(prompt.contains("REASON_MARKER"));
        assert!(prompt.contains("DECL_MARKER"));
        assert!(prompt.contains("ANCHOR_MARKER"));
    }

    #[test]
    fn test_waiver_prompt_requires_reason_to_anchor_semantic_comparison() {
        let prompt = WaiverVerifierAdapter::render_prompt("reason", "entry", "anchor");

        assert!(prompt.contains("implementer-authored waiver reason justifies"));
        assert!(prompt.contains("cited anchor as it relates to the given catalogue entry"));
        assert!(prompt.contains("reason must be self-contained"));
        assert!(!prompt.contains("obligation-fulfillment verifier"));
    }

    #[test]
    fn test_waiver_verifier_fingerprint_hashes_prompt_preamble_only() {
        let fingerprint = waiver_verifier_fingerprint();
        let expected = Sha256ContentHasher::new().sha256(WAIVER_PROMPT_PREAMBLE.as_bytes());

        assert_eq!(fingerprint.as_hash(), &expected);
    }
}
