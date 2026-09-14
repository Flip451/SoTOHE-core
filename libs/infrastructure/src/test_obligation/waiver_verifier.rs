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

use std::path::PathBuf;
use std::sync::Arc;

use domain::EvidenceCitation;
use domain::ModelTier;
use domain::tddd::test_obligation::errors::SemanticVerifierError;
use domain::tddd::test_obligation::hashes::VerifierPromptFingerprint;
use domain::tddd::test_obligation::ids::DiagnosticMessage;
use domain::tddd::test_obligation::pair::WaiverPair;
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
/// Encodes the edge-local, context-preserving, citation-required discipline for
/// waiver judgement.
const WAIVER_PROMPT_PREAMBLE: &str = "\
You are a waiver verifier. Decide whether the implementer-authored waiver reason justifies \
leaving THIS obligation edge (the cited specification element as it relates to the given catalogue \
entry) untested. Use the entry declaration together with the typed obligation identity (entry \
key, obligation kind, and item identifier), the obligation brief, and the structured \
specification element to identify the target-owned promise. The specification element's section, \
identifier, and verbatim text are authoritative; do not infer its section from the identifier or \
text. Identity and brief provide context for locating the target-owned promise, but they must not \
override the specification element, add requirements, or invent ownership. An out-of-scope \
element marks a boundary and does not become an affirmative implementation requirement. Do not \
blanket-exclude or automatically waive a reference merely because it is out of scope; inspect \
whether the target-owned edge is affected. Do not turn a non-guarantee into a prohibition; judge an \
explicit prohibition only when it belongs to the target responsibility. For a declaration that \
handles one already-selected input, do not demand caller-side selection or configuration loading; \
detect selection defects only for a declaration that owns selection. A composite or shared element \
can contain promises owned by multiple entries or multiple items. Judge only the portion owned by \
this obligation edge, and do not require a waiver reason to justify another entry's or item's \
responsibility. The reason must be self-contained and must not rely on other edges. If the reason \
does not directly address the target-owned promise, return fail; if required comparison material is \
actually missing, return pending. Do not retrieve or infer missing context.

This adapter delegates semantic comparison to the configured provider; it does not independently \
parse waiver meaning. Adapter tests establish delegation only when they verify that the complete, \
meaningful normalized request and these ownership instructions reach the provider and that provider \
verdicts and provider errors are preserved. A canned provider response without those checks does not \
prove semantic waiver judgement, and adapter tests must not reimplement the provider's semantic \
engine. Use bounded configured-provider calibration for semantic positive and negative cases.

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

/// Version of the normalized waiver-verifier request layout.
///
/// Dynamic values from this layout are supplied by the pair and participate in
/// the pair's cache key. This version identifies changes to the shape or
/// interpretation of those inputs without including provider, model, or tier.
const WAIVER_PROMPT_FORMAT_VERSION: &str = "waiver-verifier-input-v3";

/// Static description of every normalized input rendered into a waiver prompt.
/// It is part of the verifier fingerprint so a rendering change cannot reuse a
/// verdict produced for the previous request shape.
const WAIVER_PROMPT_INPUT_LAYOUT: &str = "\
obligation_identity: entry_key, obligation_kind, item_identifier\n\
obligation_brief: text\n\
entry_declaration: text\n\
specification_element: section, element_id, text\n\
waived_reason: text";

/// Returns the SHA-256 content hash of this verifier's judging contract.
///
/// The hash includes the instructions and normalized input layout/version, but
/// deliberately excludes execution provider, model, and tier. Dynamic pair
/// values are represented by their cache-key hashes rather than this static
/// verifier identity.
#[must_use]
pub fn waiver_verifier_fingerprint() -> VerifierPromptFingerprint {
    let fingerprint_material = format!(
        "verifier_format_version={WAIVER_PROMPT_FORMAT_VERSION}\n\
         preamble:\n{WAIVER_PROMPT_PREAMBLE}\n\
         input_layout:\n{WAIVER_PROMPT_INPUT_LAYOUT}\n"
    );
    VerifierPromptFingerprint::new(
        Sha256ContentHasher::new().sha256(fingerprint_material.as_bytes()),
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
    /// verification, anchored at `workspace_root`.
    #[must_use]
    pub fn new(agent_profile: AgentProfiles, workspace_root: PathBuf) -> Self {
        Self { agent_profile, runner: default_waiver_verifier_runner(workspace_root) }
    }

    /// Test-only constructor injecting a stubbed provider runner so unit tests
    /// exercise the verdict-decoding path without spawning a subprocess.
    #[cfg(test)]
    fn with_runner(agent_profile: AgentProfiles, runner: Arc<SemanticVerifierRunner>) -> Self {
        Self { agent_profile, runner }
    }

    fn render_prompt(pair: &WaiverPair) -> String {
        let obligation_id = pair.obligation_id();
        let obligation_brief = pair.obligation_brief();
        let spec_element = pair.spec_element();
        format!(
            "{WAIVER_PROMPT_PREAMBLE}\n\n\
             ## Obligation identity\nentry_key: {}\nobligation_kind: {}\nitem_identifier: {}\n\n\
             ## Obligation brief\n{}\n\n\
             ## Entry declaration\n{entry_declaration}\n\n\
             ## Specification element\nsection: {section}\nelement_id: {element_id}\ntext: {spec_text}\n\n\
             ## Waiver reason\n{waived_reason}\n",
            obligation_id.entry_key().as_str(),
            obligation_id.obligation_kind().as_kebab(),
            obligation_id.item_identifier().as_str(),
            obligation_brief.as_str(),
            entry_declaration = pair.entry_declaration().as_str(),
            section = spec_section_label(&spec_element.section),
            element_id = spec_element.element_id.as_ref(),
            spec_text = spec_element.text_label.as_str(),
            waived_reason = pair.waived_reason().as_str(),
        )
    }
}

fn spec_section_label(section: &domain::tddd::semantic_verify::SpecSectionKind) -> &'static str {
    use domain::tddd::semantic_verify::SpecSectionKind;

    match section {
        SpecSectionKind::Goal => "goal",
        SpecSectionKind::InScope => "in_scope",
        SpecSectionKind::OutOfScope => "out_of_scope",
        SpecSectionKind::Constraint => "constraint",
        SpecSectionKind::AcceptanceCriteria => "acceptance_criteria",
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
        _pair: &WaiverPair,
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
        pair: &WaiverPair,
        tier: ModelTier,
    ) -> Result<WaiverVerdict, SemanticVerifierError> {
        let round = tier_to_round_type(tier);
        let resolved = resolve_execution_or_err(&self.agent_profile, CAPABILITY, round)?;
        let prompt = Self::render_prompt(pair);
        let raw = (self.runner)(resolved, prompt)?;
        let dto: WaiverVerdictDto = extract_verdict_json(&raw)?;
        map_verdict(dto)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Mutex;

    use domain::plan_ref::SpecElementId;
    use domain::tddd::catalogue_v2::CatalogueEntryKey;
    use domain::tddd::semantic_verify::{SpecElementRef, SpecSectionKind};
    use domain::tddd::test_obligation::ids::{
        TestObligationBrief, TestObligationId, TestObligationItemIdentifier, WaivedReason,
    };
    use domain::tddd::test_obligation::pair::{EntryDeclaration, WaiverPair};
    use domain::tddd::test_obligation::vocab::TestObligationKind;

    use crate::agent_profiles::ResolvedExecution;

    use super::*;

    const CONFIG: &str = r#"{
        "schema_version": 1,
        "providers": {
            "claude": {
                "label": "Claude",
                "supported_reasoning_efforts": ["low", "high"]
            }
        },
        "capabilities": {
            "waiver-verifier": {
                "provider": "claude",
                "model": "claude-opus-4-8",
                "fast_provider": "claude",
                "fast_model": "claude-haiku-4-5",
                "reasoning_effort": "high",
                "fast_reasoning_effort": "low",
                "execution_mode": "typed-pipeline"
            }
        }
    }"#;

    fn profiles() -> AgentProfiles {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("agent-profiles.json");
        std::fs::write(&path, CONFIG).unwrap();
        AgentProfiles::load(dir.path(), &path).unwrap()
    }

    fn codex_profiles() -> AgentProfiles {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("agent-profiles.json");
        std::fs::write(
            &path,
            r#"{
                "schema_version": 1,
                "providers": {
                    "codex": {
                        "label": "Codex",
                        "supported_reasoning_efforts": ["high"]
                    }
                },
                "capabilities": {
                    "waiver-verifier": {
                        "provider": "codex",
                        "model": "gpt-5",
                        "reasoning_effort": "high",
                        "execution_mode": "typed-pipeline"
                    }
                }
            }"#,
        )
        .unwrap();
        AgentProfiles::load(dir.path(), &path).unwrap()
    }

    fn stub_runner(output: &'static str) -> Arc<SemanticVerifierRunner> {
        Arc::new(move |_resolved, _prompt| Ok(output.to_owned()))
    }

    fn adapter(output: &'static str) -> WaiverVerifierAdapter {
        WaiverVerifierAdapter::with_runner(profiles(), stub_runner(output))
    }

    fn waiver_pair(reason: &str, entry_declaration: &str, anchor_text: &str) -> WaiverPair {
        waiver_pair_with_context(
            reason,
            entry_declaration,
            SpecSectionKind::Goal,
            "GO-01",
            anchor_text,
            "Entry",
            "trait_method:verify",
            "verify the entry-local contract",
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn waiver_pair_with_context(
        reason: &str,
        entry_declaration: &str,
        section: SpecSectionKind,
        element_id: &str,
        spec_text: &str,
        entry_key: &str,
        item_identifier: &str,
        obligation_brief: &str,
    ) -> WaiverPair {
        WaiverPair::new(
            WaivedReason::try_new(reason.to_owned()).unwrap(),
            EntryDeclaration::try_new(entry_declaration.to_owned()).unwrap(),
            SpecElementRef::new(
                section,
                SpecElementId::try_new(element_id.to_owned()).unwrap(),
                spec_text.to_owned(),
            ),
            TestObligationId::new(
                CatalogueEntryKey::try_new(entry_key.to_owned()).unwrap(),
                TestObligationKind::Contract,
                TestObligationItemIdentifier::try_new(item_identifier.to_owned()).unwrap(),
            ),
            TestObligationBrief::try_new(obligation_brief.to_owned()).unwrap(),
        )
    }

    #[test]
    fn test_waiver_verifier_new_missing_workspace_root_threads_root_to_runner() {
        let tempdir = tempfile::tempdir().unwrap();
        let workspace_root = tempdir.path().join("missing-workspace-root");
        let verifier = WaiverVerifierAdapter::new(codex_profiles(), workspace_root.clone());

        let err = verifier
            .verify_pair(&waiver_pair("reason", "entry", "anchor"), ModelTier::Final)
            .unwrap_err();

        let SemanticVerifierError::VerifierPort(message) = err;
        assert!(message.as_str().contains("cannot canonicalize project root"));
        assert!(message.as_str().contains(&workspace_root.display().to_string()));
    }

    #[test]
    fn failing_verifier_returns_verifier_port_error() {
        let verifier = FailingWaiverVerifier::from_message("profile missing");

        let err = verifier
            .verify_pair(&waiver_pair("reason", "entry", "anchor"), ModelTier::Fast)
            .unwrap_err();

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
                .verify_pair(&waiver_pair(waived_reason, entry_declaration, anchor_text), tier)
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
            &waiver_pair(
                "this anchor is a design goal with no verifiable behaviour",
                "struct Foo",
                "GO: state the purpose",
            ),
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
                &waiver_pair(
                    "this design goal has no independently observable behaviour",
                    "struct Entry",
                    "the design goal explains the purpose",
                ),
                ModelTier::Final,
            )
            .unwrap();

        assert!(matches!(verdict, WaiverVerdict::Waived { .. }));
        let (resolved, prompt) = captured.lock().unwrap().clone().unwrap();
        assert!(matches!(
            resolved,
            ResolvedExecution::ProviderCli { model, .. } if model.as_str() == "claude-opus-4-8"
        ));
        assert!(prompt.starts_with(
            "You are a waiver verifier. Decide whether the implementer-authored waiver reason"
        ));
        let payload = prompt.split_once("\n\n## Obligation identity\n").unwrap().1;
        assert_eq!(
            payload,
            "entry_key: Entry\nobligation_kind: contract\n\
item_identifier: trait_method:verify\n\n\
## Obligation brief\nverify the entry-local contract\n\n\
## Entry declaration\nstruct Entry\n\n\
## Specification element\nsection: goal\nelement_id: GO-01\n\
text: the design goal explains the purpose\n\n\
## Waiver reason\nthis design goal has no independently observable behaviour\n"
        );
        for marker in [
            "An out-of-scope element marks a boundary and does not become an affirmative implementation requirement.",
            "Do not blanket-exclude or automatically waive a reference merely because it is out of scope;",
            "For a declaration that handles one already-selected input, do not demand caller-side selection or configuration loading;",
            "This adapter delegates semantic comparison to the configured provider;",
            "verdicts and provider errors are preserved.",
            "If the reason does not directly address the target-owned promise, return fail;",
            "if required comparison material is actually missing, return pending.",
        ] {
            assert!(prompt.contains(marker), "missing forwarded waiver input: {marker}");
        }
    }

    #[test]
    fn test_waiver_adapter_propagates_provider_errors() {
        let runner: Arc<SemanticVerifierRunner> =
            Arc::new(|_, _| Err(semantic_verifier_error("configured provider failed")));
        let adapter = WaiverVerifierAdapter::with_runner(profiles(), runner);

        let err = adapter
            .verify_pair(&waiver_pair("reason", "entry", "target promise"), ModelTier::Final)
            .unwrap_err();

        let SemanticVerifierError::VerifierPort(message) = err;
        assert_eq!(message.as_str(), "configured provider failed");
    }

    #[test]
    fn render_prompt_preserves_authoritative_section_even_when_identifier_disagrees() {
        let prompt = WaiverVerifierAdapter::render_prompt(&waiver_pair_with_context(
            "REASON_MARKER",
            "DECL_MARKER",
            SpecSectionKind::OutOfScope,
            "IN-01",
            "same specification text",
            "Entry",
            "trait_method:verify",
            "verify the entry-local contract",
        ));

        assert!(prompt.contains(
            "## Specification element\nsection: out_of_scope\nelement_id: IN-01\ntext: same specification text\n"
        ));
        assert!(prompt.contains(
            "The specification element's section, identifier, and verbatim text are authoritative; do not infer its section from the identifier or text."
        ));
    }

    #[test]
    fn render_prompt_preserves_every_specification_section() {
        let cases = [
            (SpecSectionKind::Goal, "GO-01"),
            (SpecSectionKind::Constraint, "CN-01"),
            (SpecSectionKind::AcceptanceCriteria, "AC-01"),
            (SpecSectionKind::InScope, "IN-01"),
            (SpecSectionKind::OutOfScope, "OUT-01"),
        ];

        for (section, element_id) in cases {
            let section_label = spec_section_label(&section);
            let prompt = WaiverVerifierAdapter::render_prompt(&waiver_pair_with_context(
                "WAIVER_REASON",
                "ENTRY_DECLARATION",
                section,
                element_id,
                "SHARED_SPECIFICATION_TEXT",
                "Entry",
                "trait_method:verify",
                "verify the entry-local contract",
            ));
            let expected = format!(
                "## Specification element\nsection: {}\nelement_id: {element_id}\ntext: SHARED_SPECIFICATION_TEXT\n",
                section_label,
            );

            assert!(
                prompt.contains(&expected),
                "missing structured specification input: {expected}"
            );
        }
    }

    #[test]
    fn render_prompt_includes_section_aware_ownership_guidance() {
        let prompt = WaiverVerifierAdapter::render_prompt(&waiver_pair(
            "WAIVER_REASON",
            "ENTRY_DECLARATION",
            "SPECIFICATION_TEXT",
        ));

        for marker in [
            "An out-of-scope element marks a boundary and does not become an affirmative implementation requirement.",
            "Do not blanket-exclude or automatically waive a reference merely because it is out of scope; inspect whether the target-owned edge is affected.",
            "Do not turn a non-guarantee into a prohibition; judge an explicit prohibition only when it belongs to the target responsibility.",
            "For a declaration that handles one already-selected input, do not demand caller-side selection or configuration loading; detect selection defects only for a declaration that owns selection.",
            "This adapter delegates semantic comparison to the configured provider; it does not independently parse waiver meaning.",
            "The reason must be self-contained and must not rely on other edges.",
            "If the reason does not directly address the target-owned promise, return fail;",
            "if required comparison material is actually missing, return pending.",
        ] {
            assert!(prompt.contains(marker), "missing ownership guidance: {marker}");
        }
    }

    #[test]
    fn render_prompt_keeps_single_input_and_selection_responsibilities_distinct() {
        let single_input = WaiverVerifierAdapter::render_prompt(&waiver_pair_with_context(
            "single-input waiver reason",
            "SingleInputDecoder { selected_input: Input }",
            SpecSectionKind::InScope,
            "IN-01",
            "the decoder processes one already-selected input and does not choose configuration",
            "SingleInputDecoder",
            "method:decode",
            "verify selected-input decoding without input selection",
        ));
        let input_selector = WaiverVerifierAdapter::render_prompt(&waiver_pair_with_context(
            "selection waiver reason",
            "InputSelector { configuration: Settings }",
            SpecSectionKind::InScope,
            "IN-01",
            "the selector owns choosing an input from configuration",
            "InputSelector",
            "method:select",
            "verify configuration-owned input selection",
        ));

        assert!(single_input.contains("SingleInputDecoder { selected_input: Input }"));
        assert!(single_input.contains(
            "the decoder processes one already-selected input and does not choose configuration"
        ));
        assert!(input_selector.contains("InputSelector { configuration: Settings }"));
        assert!(input_selector.contains("the selector owns choosing an input from configuration"));
        assert_ne!(single_input, input_selector);
    }

    #[test]
    fn test_waiver_adapter_preserves_fail_and_pending_verdicts_from_provider() {
        let pair = waiver_pair("WAIVER_REASON", "ENTRY_DECLARATION", "SPECIFICATION_TEXT");

        let contradiction = adapter(
            r#"{"kind":"fail","citation":null,"reason":"the reason contradicts the target responsibility"}"#,
        )
        .verify_pair(&pair, ModelTier::Final)
        .unwrap();
        assert!(matches!(
            contradiction,
            WaiverVerdict::Fail { reason } if reason.as_str() == "the reason contradicts the target responsibility"
        ));

        let substitution = adapter(
            r#"{"kind":"fail","citation":null,"reason":"the reason covers another responsibility only"}"#,
        )
        .verify_pair(&pair, ModelTier::Final)
        .unwrap();
        assert!(matches!(
            substitution,
            WaiverVerdict::Fail { reason } if reason.as_str() == "the reason covers another responsibility only"
        ));

        let missing_direct_evidence = adapter(
            r#"{"kind":"fail","citation":null,"reason":"the reason provides no direct evidence for the target obligation"}"#,
        )
        .verify_pair(&pair, ModelTier::Final)
        .unwrap();
        assert!(matches!(
            missing_direct_evidence,
            WaiverVerdict::Fail { reason } if reason.as_str() == "the reason provides no direct evidence for the target obligation"
        ));

        let pending = adapter(r#"{"kind":"pending","citation":null,"reason":null}"#)
            .verify_pair(&pair, ModelTier::Final)
            .unwrap();
        assert_eq!(pending, WaiverVerdict::Pending);
    }

    #[test]
    fn fail_verdict_decodes_with_reason() {
        let verdict = adapter(r#"{"kind":"fail","citation":null,"reason":"the anchor promises verifiable behaviour"}"#)
            .verify_pair(
                &waiver_pair("reason", "decl", "anchor"),
                ModelTier::Final,
            )
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
            .verify_pair(&waiver_pair("reason", "decl", "anchor"), ModelTier::Final)
            .unwrap_err();
        let SemanticVerifierError::VerifierPort(message) = err;
        assert!(message.as_str().contains("citation"));
    }

    #[test]
    fn fail_without_reason_fails_closed() {
        let err = adapter(r#"{"kind":"fail","citation":null,"reason":"  "}"#)
            .verify_pair(&waiver_pair("reason", "decl", "anchor"), ModelTier::Final)
            .unwrap_err();
        let SemanticVerifierError::VerifierPort(message) = err;
        assert!(message.as_str().contains("reason"));
    }

    #[test]
    fn pending_verdict_is_preserved() {
        let verdict = adapter(r#"{"kind":"pending","citation":null,"reason":null}"#)
            .verify_pair(&waiver_pair("reason", "decl", "anchor"), ModelTier::Final)
            .unwrap();
        assert_eq!(verdict, WaiverVerdict::Pending);
    }

    #[test]
    fn unknown_field_fails_closed() {
        let err = adapter(r#"{"kind":"pass","citation":"c","reason":null,"category":"x"}"#)
            .verify_pair(&waiver_pair("reason", "decl", "anchor"), ModelTier::Final)
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
        let profile = AgentProfiles::load(dir.path(), &path).unwrap();
        let err = WaiverVerifierAdapter::with_runner(profile, stub_runner("{}"))
            .verify_pair(&waiver_pair("reason", "decl", "anchor"), ModelTier::Final)
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
        adapter.verify_pair(&waiver_pair("reason", "decl", "anchor"), ModelTier::Fast).unwrap();
        let resolved = captured.lock().unwrap().clone().unwrap();
        assert!(matches!(
            resolved,
            ResolvedExecution::ProviderCli { model, .. } if model.as_str() == "claude-haiku-4-5"
        ));
    }

    #[test]
    fn render_prompt_preserves_complete_waiver_pair() {
        let pair = waiver_pair("REASON_MARKER", "DECL_MARKER", "ANCHOR_MARKER");
        let prompt = WaiverVerifierAdapter::render_prompt(&pair);
        assert!(prompt.starts_with("You are a waiver verifier."));
        let payload = prompt.split_once("\n\n## Obligation identity\n").unwrap().1;
        assert_eq!(
            payload,
            "entry_key: Entry\nobligation_kind: contract\n\
item_identifier: trait_method:verify\n\n\
## Obligation brief\nverify the entry-local contract\n\n\
## Entry declaration\nDECL_MARKER\n\n\
## Specification element\nsection: goal\nelement_id: GO-01\n\
text: ANCHOR_MARKER\n\n\
## Waiver reason\nREASON_MARKER\n"
        );
    }

    #[test]
    fn render_prompt_distinguishes_specification_sections_with_identical_text() {
        let in_scope = WaiverVerifierAdapter::render_prompt(&waiver_pair_with_context(
            "reason",
            "entry",
            SpecSectionKind::InScope,
            "IN-01",
            "same specification text",
            "Entry",
            "trait_method:verify",
            "verify the entry-local contract",
        ));
        let out_of_scope = WaiverVerifierAdapter::render_prompt(&waiver_pair_with_context(
            "reason",
            "entry",
            SpecSectionKind::OutOfScope,
            "IN-01",
            "same specification text",
            "Entry",
            "trait_method:verify",
            "verify the entry-local contract",
        ));

        assert_ne!(in_scope, out_of_scope);
    }

    #[test]
    #[ignore = "host-only semantic calibration; run explicitly with configured provider credentials"]
    fn test_configured_provider_calibrates_waiver_section_and_ownership_boundaries() {
        let workspace_root =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap();
        let profiles_path = workspace_root.join(".harness/config/agent-profiles.json");
        let profiles = AgentProfiles::load(&workspace_root, &profiles_path).unwrap();
        let verifier = WaiverVerifierAdapter::new(profiles, workspace_root);

        struct Case {
            name: &'static str,
            reason: &'static str,
            entry_key: &'static str,
            item_identifier: &'static str,
            brief: &'static str,
            declaration: &'static str,
            section: SpecSectionKind,
            element_id: &'static str,
            spec_text: &'static str,
            expect_waived: bool,
        }

        let cases = [
            Case {
                name: "selected-input-responsibility",
                reason: "The declaration accepts an already-selected input and has no selection or configuration-loading responsibility, so the caller-owned selection edge does not require a test here.",
                entry_key: "SelectedInputDecoder",
                item_identifier: "method:decode",
                brief: "verify that the decoder does not own input selection",
                declaration: "SelectedInputDecoder { selected_input: CapturedInput }",
                section: SpecSectionKind::InScope,
                element_id: "IN-02",
                spec_text: "Caller-side input selection and configuration loading are outside this decoder's responsibility; this declaration receives one already-selected input.",
                expect_waived: true,
            },
            Case {
                name: "selection-owned-waiver",
                reason: "Input selection belongs to the caller, so this selector has no selection responsibility and needs no test.",
                entry_key: "InputSelector",
                item_identifier: "method:select",
                brief: "verify that the selector uses the configured input and does not fall back",
                declaration: "InputSelector { configuration: InputConfiguration, streams: Streams }",
                section: SpecSectionKind::InScope,
                element_id: "IN-02",
                spec_text: "The input selector owns choosing the configured input and must reject an invalid configured input rather than fall back to another stream.",
                expect_waived: false,
            },
            Case {
                name: "out-of-scope-non-guarantee",
                reason: "The cited remote synchronization is explicitly outside the component's scope and is a non-guarantee, so this edge does not justify requiring a remote-synchronization test.",
                entry_key: "LocalInputDecoder",
                item_identifier: "method:decode",
                brief: "verify the local decoder's remote synchronization boundary",
                declaration: "LocalInputDecoder { selected_input: CapturedInput; remote_sync: not owned }",
                section: SpecSectionKind::OutOfScope,
                element_id: "OUT-01",
                spec_text: "Remote synchronization is outside this local component's scope and is not guaranteed; the local decoder only processes its already-selected input.",
                expect_waived: true,
            },
            Case {
                name: "non-guarantee-turned-into-prohibition",
                reason: "The local decoder must reject input whenever remote synchronization is unavailable, so the absence of that behavior needs no test.",
                entry_key: "LocalInputDecoder",
                item_identifier: "method:decode",
                brief: "verify the local decoder's remote synchronization boundary",
                declaration: "LocalInputDecoder { selected_input: CapturedInput; remote_sync: not owned }",
                section: SpecSectionKind::OutOfScope,
                element_id: "OUT-01",
                spec_text: "Remote synchronization is outside this local component's scope and is not guaranteed; the local decoder only processes its already-selected input.",
                expect_waived: false,
            },
            Case {
                name: "target-owned-prohibition",
                reason: "The path rule is in the out-of-scope section, so no test is needed for the storage location.",
                entry_key: "ProjectStateWriter",
                item_identifier: "method:state_path",
                brief: "verify the writer's project-local storage boundary",
                declaration: "ProjectStateWriter { project_root: Path; user_global_storage: prohibited }",
                section: SpecSectionKind::OutOfScope,
                element_id: "OUT-01",
                spec_text: "This component must not write state to a user-global location; only a project-local location is permitted.",
                expect_waived: false,
            },
        ];

        for Case {
            name,
            reason,
            entry_key,
            item_identifier,
            brief,
            declaration,
            section,
            element_id,
            spec_text,
            expect_waived,
        } in cases
        {
            let pair = WaiverPair::new(
                WaivedReason::try_new(reason.to_owned()).unwrap(),
                EntryDeclaration::try_new(declaration.to_owned()).unwrap(),
                SpecElementRef::new(
                    section,
                    SpecElementId::try_new(element_id.to_owned()).unwrap(),
                    spec_text.to_owned(),
                ),
                TestObligationId::new(
                    CatalogueEntryKey::try_new(entry_key.to_owned()).unwrap(),
                    TestObligationKind::Contract,
                    TestObligationItemIdentifier::try_new(item_identifier.to_owned()).unwrap(),
                ),
                TestObligationBrief::try_new(brief.to_owned()).unwrap(),
            );
            let verdict = verifier.verify_pair(&pair, ModelTier::Fast).unwrap();

            assert_eq!(
                matches!(verdict, WaiverVerdict::Waived { .. }),
                expect_waived,
                "unexpected {name} verdict: {verdict:?}"
            );
        }
    }

    #[test]
    fn test_waiver_verifier_fingerprint_hashes_contract_without_provider_identity() {
        let fingerprint = waiver_verifier_fingerprint();
        let expected_material = format!(
            "verifier_format_version={WAIVER_PROMPT_FORMAT_VERSION}\n\
             preamble:\n{WAIVER_PROMPT_PREAMBLE}\n\
             input_layout:\n{WAIVER_PROMPT_INPUT_LAYOUT}\n"
        );
        let expected = Sha256ContentHasher::new().sha256(expected_material.as_bytes());

        assert_eq!(fingerprint.as_hash(), &expected);
    }
}
