//! Capability adapter for the obligation-fulfillment semantic verifier.
//!
//! Judges whether the tests bound to an obligation actually verify the behaviour
//! an anchor promises for a catalogue entry — a second-order, code → natural
//! language comparison (ADR D6 / D8). The provider is resolved from the
//! `obligation-fulfillment-verifier` capability in `agent-profiles.json`
//! (IN-09 / IN-11 / CN-08); the LLM call is delegated through the shared
//! semantic-verifier runtime (IN-12 / AC-06 / AC-08).
//!
//! Fail-closed decoding: a `pass` without a citation and a `fail` without a
//! reason are both rejected at the codec boundary, so a silent pass is
//! impossible (AC-07). A `pending` verdict is preserved and treated as fail at
//! the gate by the caller.

use std::path::PathBuf;
use std::sync::Arc;

use domain::EvidenceCitation;
use domain::ModelTier;
use domain::tddd::test_obligation::errors::SemanticVerifierError;
use domain::tddd::test_obligation::hashes::VerifierPromptFingerprint;
use domain::tddd::test_obligation::ids::DiagnosticMessage;
use domain::tddd::test_obligation::pair::ObligationFulfillmentPair;
use domain::tddd::test_obligation::ports::ObligationFulfillmentVerifierPort;
use domain::tddd::test_obligation::verdict::ObligationFulfillmentVerdict;
use domain::tddd::test_obligation::vocab::FulfillmentFailCategory;
use usecase::test_obligation::hasher::ContentHasherPort;

use crate::agent_profiles::AgentProfiles;
use crate::test_obligation::diagnostic;
use crate::test_obligation::semantic_verifier::{
    SemanticVerifierRunner, VerdictKindWire, default_fulfillment_verifier_runner,
    extract_verdict_json, resolve_execution_or_err, semantic_verifier_error, tier_to_round_type,
};
use crate::test_obligation::sha256_content_hasher::Sha256ContentHasher;

/// Capability name resolved from `agent-profiles.json` for this verifier.
const CAPABILITY: &str = "obligation-fulfillment-verifier";

/// Instruction preamble prepended to every obligation-fulfillment prompt.
///
/// Encodes the D6 fail taxonomy (contradiction / substitution /
/// central-unverified), the section-aware ownership rules, and the
/// citation-required discipline so the model returns a decodable verdict.
const FULFILLMENT_PROMPT_PREAMBLE: &str = "\
You are an obligation-fulfillment verifier. Decide whether the provided test source \
actually verifies the observable behaviour promised by the cited specification element for the \
target obligation item on the target catalogue entry. Use the entry declaration together with \
the typed obligation identity (entry key, obligation kind, and item identifier), the obligation \
brief, and the structured specification element to identify the target item's observable \
promises. The specification element's section, identifier, and verbatim text are authoritative; \
do not infer its section from the identifier or text. Identity and brief provide context for \
locating the target-owned promise, but they must not override the specification element, add \
requirements, or invent ownership. An out-of-scope element marks a boundary and does not become \
an affirmative implementation requirement. Do not blanket-exclude or automatically waive a \
reference merely because it is out of scope; inspect whether the target-owned edge is affected. \
Do not turn a non-guarantee into a prohibition; judge an explicit prohibition only when it belongs \
to the target responsibility. For a declaration that handles one already-selected input, do not \
demand caller-side selection or configuration loading; detect selection defects only for a declaration \
that owns selection. A composite or shared element can contain promises owned by multiple entries or \
multiple items. Distinguish the target-owned portion from those other portions and judge only the \
target-owned portion for this pair. Do not fail because the bound tests omit behaviour owned by \
another entry or item. Still fail when the target-owned behaviour is contradicted or its central \
behaviour is not verified; evidence for another item does not satisfy this target item.

This adapter delegates semantic comparison to the configured provider; it does not independently \
parse test meaning. Adapter tests establish delegation only when they verify that the complete, \
meaningful normalized request and these ownership instructions reach the provider and that provider \
verdicts and provider errors are preserved. A canned provider response without those checks does not \
prove semantic fulfilment, and adapter tests must not reimplement the provider's semantic engine. Use \
bounded configured-provider calibration for semantic positive and negative cases. If the target-owned \
promise lacks direct evidence, return fail with central_unverified; if required comparison material \
is actually missing, return pending. Do not retrieve or infer missing context.

Reply with exactly one JSON object and nothing else:
{\"kind\": \"pass\" | \"fail\" | \"pending\", \"citation\": string | null, \"reason\": string | null, \"category\": \"contradiction\" | \"substitution\" | \"central_unverified\" | null}

- \"pass\": the bound tests fulfil the obligation. \"citation\" MUST quote verbatim the \
part of the test source that fulfils it. A pass without a citation is invalid.
If the tests fully verify the target item's observable promises within this cited specification \
element, return pass even when the element contains promises owned by other entries or items.
- \"fail\": set \"reason\" and \"category\": \"contradiction\" (a test asserts the opposite of \
the target-owned promise), \"substitution\" (the tests cite the specification element but verify \
content unrelated to the target item's promise), or \"central_unverified\" (no contradiction or \
irrelevance, but the central target-owned part of the cited specification element's promise is \
left unverified; \
do NOT demand from this edge promise parts belonging to other entries' or items' responsibilities).
- \"pending\": you cannot confirm fulfilment from the material provided.";

/// Version of the normalized fulfillment-verifier request layout.
///
/// Dynamic values from this layout are supplied by the pair and participate in
/// the pair's cache key. This version identifies changes to the shape or
/// interpretation of those inputs without including provider, model, or tier.
const FULFILLMENT_PROMPT_FORMAT_VERSION: &str = "fulfillment-verifier-input-v3";

/// Static description of every normalized input rendered into a fulfillment
/// prompt. It is part of the verifier fingerprint so a rendering change cannot
/// reuse a verdict produced for the previous request shape.
const FULFILLMENT_PROMPT_INPUT_LAYOUT: &str = "\
obligation_identity: entry_key, obligation_kind, item_identifier\n\
obligation_brief: text\n\
entry_declaration: text\n\
specification_element: section, element_id, text\n\
bound_test_source: text";

/// Returns the SHA-256 content hash of this verifier's judging contract.
///
/// The hash includes the instructions and normalized input layout/version, but
/// deliberately excludes execution provider, model, and tier. Dynamic pair
/// values are represented by their cache-key hashes rather than this static
/// verifier identity.
#[must_use]
pub fn fulfillment_verifier_fingerprint() -> VerifierPromptFingerprint {
    let fingerprint_material = format!(
        "verifier_format_version={FULFILLMENT_PROMPT_FORMAT_VERSION}\n\
         preamble:\n{FULFILLMENT_PROMPT_PREAMBLE}\n\
         input_layout:\n{FULFILLMENT_PROMPT_INPUT_LAYOUT}\n"
    );
    VerifierPromptFingerprint::new(
        Sha256ContentHasher::new().sha256(fingerprint_material.as_bytes()),
    )
}

/// Capability adapter that resolves the obligation-fulfillment verifier provider
/// and delegates the semantic judgement to it (IN-09 / IN-11 / AC-07 / CN-08).
pub struct ObligationFulfillmentVerifierAdapter {
    agent_profile: AgentProfiles,
    runner: Arc<SemanticVerifierRunner>,
}

impl ObligationFulfillmentVerifierAdapter {
    /// Builds an adapter that spawns the configured provider subprocess for each
    /// verification, anchored at `workspace_root`.
    #[must_use]
    pub fn new(agent_profile: AgentProfiles, workspace_root: PathBuf) -> Self {
        Self { agent_profile, runner: default_fulfillment_verifier_runner(workspace_root) }
    }

    /// Test-only constructor injecting a stubbed provider runner so unit tests
    /// exercise the verdict-decoding path without spawning a subprocess.
    #[cfg(test)]
    fn with_runner(agent_profile: AgentProfiles, runner: Arc<SemanticVerifierRunner>) -> Self {
        Self { agent_profile, runner }
    }

    fn render_prompt(pair: &ObligationFulfillmentPair) -> String {
        let obligation_id = pair.obligation_id();
        let obligation_brief = pair.obligation_brief();
        let spec_element = pair.spec_element();
        format!(
            "{FULFILLMENT_PROMPT_PREAMBLE}\n\n\
             ## Obligation identity\nentry_key: {}\nobligation_kind: {}\nitem_identifier: {}\n\n\
             ## Obligation brief\n{}\n\n\
             ## Entry declaration\n{entry_declaration}\n\n\
             ## Specification element\nsection: {section}\nelement_id: {element_id}\ntext: {spec_text}\n\n\
             ## Bound test source\n{tests_source}\n",
            obligation_id.entry_key().as_str(),
            obligation_id.obligation_kind().as_kebab(),
            obligation_id.item_identifier().as_str(),
            obligation_brief.as_str(),
            entry_declaration = pair.entry_declaration().as_str(),
            section = spec_section_label(&spec_element.section),
            element_id = spec_element.element_id.as_ref(),
            spec_text = spec_element.text_label.as_str(),
            tests_source = pair.tests_source().as_str(),
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

/// Fail-closed verifier used when the configured fulfillment capability cannot load.
#[derive(Debug, Clone)]
pub struct FailingObligationFulfillmentVerifier {
    message: DiagnosticMessage,
}

impl FailingObligationFulfillmentVerifier {
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

impl ObligationFulfillmentVerifierPort for FailingObligationFulfillmentVerifier {
    fn verify_pair(
        &self,
        _pair: &ObligationFulfillmentPair,
        _tier: ModelTier,
    ) -> Result<ObligationFulfillmentVerdict, SemanticVerifierError> {
        Err(SemanticVerifierError::VerifierPort(self.message.clone()))
    }
}

/// Wire form of the obligation-fulfillment verdict returned by the provider.
///
/// `category` is only meaningful on a `fail`; when the routed provider is
/// codex, the fulfillment structured-output schema
/// ([`crate::ref_verify::process_runner::CODEX_FULFILLMENT_OUTPUT_SCHEMA`])
/// includes it explicitly so a real category is preserved through calibration.
/// It is still declared optional here so a `pass` or `pending` verdict
/// (which emits `category: null`) decodes, and so a provider constrained to a
/// three-field schema still decodes with the conservative `central_unverified`
/// fallback (the finding on PR #189 required the schema to admit categories,
/// not that this fallback be removed).
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FulfillmentVerdictDto {
    kind: VerdictKindWire,
    citation: Option<String>,
    reason: Option<String>,
    category: Option<FulfillmentFailCategoryWire>,
}

/// Wire form of the obligation-fulfillment fail category.
#[derive(serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum FulfillmentFailCategoryWire {
    Contradiction,
    Substitution,
    CentralUnverified,
}

impl FulfillmentFailCategoryWire {
    fn into_domain(self) -> FulfillmentFailCategory {
        match self {
            Self::Contradiction => FulfillmentFailCategory::Contradiction,
            Self::Substitution => FulfillmentFailCategory::Substitution,
            Self::CentralUnverified => FulfillmentFailCategory::CentralUnverified,
        }
    }
}

fn map_verdict(
    dto: FulfillmentVerdictDto,
) -> Result<ObligationFulfillmentVerdict, SemanticVerifierError> {
    match dto.kind {
        VerdictKindWire::Pass => {
            let citation =
                EvidenceCitation::try_new(dto.citation.unwrap_or_default()).map_err(|e| {
                    semantic_verifier_error(&format!(
                        "fulfilled verdict without a valid citation was rejected: {e}"
                    ))
                })?;
            Ok(ObligationFulfillmentVerdict::Fulfilled { citation })
        }
        VerdictKindWire::Fail => {
            let reason_text = dto
                .reason
                .filter(|r| !r.trim().is_empty())
                .ok_or_else(|| semantic_verifier_error("fail verdict missing required reason"))?;
            let category = dto.category.map_or(
                FulfillmentFailCategory::CentralUnverified,
                FulfillmentFailCategoryWire::into_domain,
            );
            Ok(ObligationFulfillmentVerdict::Fail { category, reason: diagnostic(&reason_text) })
        }
        VerdictKindWire::Pending => Ok(ObligationFulfillmentVerdict::Pending),
    }
}

impl ObligationFulfillmentVerifierPort for ObligationFulfillmentVerifierAdapter {
    fn verify_pair(
        &self,
        pair: &ObligationFulfillmentPair,
        tier: ModelTier,
    ) -> Result<ObligationFulfillmentVerdict, SemanticVerifierError> {
        let round = tier_to_round_type(tier);
        let resolved = resolve_execution_or_err(&self.agent_profile, CAPABILITY, round)?;
        let prompt = Self::render_prompt(pair);
        let raw = (self.runner)(resolved, prompt)?;
        let dto: FulfillmentVerdictDto = extract_verdict_json(&raw)?;
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
    use domain::tddd::semantic_verify::{ModelTier, SpecElementRef, SpecSectionKind};
    use domain::tddd::test_obligation::ids::{
        TestObligationBrief, TestObligationId, TestObligationItemIdentifier,
    };
    use domain::tddd::test_obligation::pair::{EntryDeclaration, TestsSource};
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
            "obligation-fulfillment-verifier": {
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
                    "obligation-fulfillment-verifier": {
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

    fn adapter(output: &'static str) -> ObligationFulfillmentVerifierAdapter {
        ObligationFulfillmentVerifierAdapter::with_runner(profiles(), stub_runner(output))
    }

    fn fulfillment_pair(
        tests_source: &str,
        entry_declaration: &str,
        spec_text: &str,
    ) -> ObligationFulfillmentPair {
        fulfillment_pair_with_spec_element(
            tests_source,
            entry_declaration,
            SpecSectionKind::InScope,
            "IN-01",
            spec_text,
        )
    }

    fn fulfillment_pair_with_spec_element(
        tests_source: &str,
        entry_declaration: &str,
        section: SpecSectionKind,
        element_id: &str,
        spec_text: &str,
    ) -> ObligationFulfillmentPair {
        ObligationFulfillmentPair::new(
            TestsSource::try_new(tests_source.to_owned()).unwrap(),
            EntryDeclaration::try_new(entry_declaration.to_owned()).unwrap(),
            SpecElementRef::new(
                section,
                SpecElementId::try_new(element_id.to_owned()).unwrap(),
                spec_text.to_owned(),
            ),
            TestObligationId::new(
                CatalogueEntryKey::try_new("Entry".to_owned()).unwrap(),
                TestObligationKind::Contract,
                TestObligationItemIdentifier::try_new("trait_method:verify".to_owned()).unwrap(),
            ),
            TestObligationBrief::try_new("verify the entry-local contract".to_owned()).unwrap(),
        )
    }

    #[test]
    fn test_fulfillment_verifier_new_missing_workspace_root_threads_root_to_runner() {
        let tempdir = tempfile::tempdir().unwrap();
        let workspace_root = tempdir.path().join("missing-workspace-root");
        let verifier =
            ObligationFulfillmentVerifierAdapter::new(codex_profiles(), workspace_root.clone());

        let err = verifier
            .verify_pair(&fulfillment_pair("tests", "entry", "anchor"), ModelTier::Final)
            .unwrap_err();

        let SemanticVerifierError::VerifierPort(message) = err;
        assert!(message.as_str().contains("cannot canonicalize project root"));
        assert!(message.as_str().contains(&workspace_root.display().to_string()));
    }

    #[test]
    fn failing_verifier_returns_verifier_port_error() {
        let verifier = FailingObligationFulfillmentVerifier::from_message("profile missing");

        let err = verifier
            .verify_pair(&fulfillment_pair("tests", "entry", "anchor"), ModelTier::Fast)
            .unwrap_err();

        let SemanticVerifierError::VerifierPort(message) = err;
        assert_eq!(message.as_str(), "profile missing");
    }

    /// AC-06 (entry-relevant part): a pass verdict must carry a citation, and a
    /// citation-less pass must fail closed. The fail-closed stand-in can never
    /// emit a pass verdict at all — every invocation returns a VerifierPort
    /// error — therefore no uncited pass can exist. This test verifies that
    /// premise directly on the stand-in itself.
    #[test]
    fn failing_verifier_never_emits_pass_verdict_so_no_uncited_pass_can_exist() {
        let verifier = FailingObligationFulfillmentVerifier::from_message("profile unavailable");

        let result =
            verifier.verify_pair(&fulfillment_pair("tests", "entry", "anchor"), ModelTier::Fast);

        assert!(result.is_err());
    }

    #[test]
    fn test_failing_fulfillment_verifier_for_each_pair_returns_fail_closed_port_error() {
        let verifier = FailingObligationFulfillmentVerifier::from_message("profile unavailable");

        for (tests_source, entry_declaration, anchor_text, tier) in [
            ("assert!(covered)", "struct Entry", "anchor promise", ModelTier::Fast),
            (
                "assert_eq!(actual, expected)",
                "trait Port",
                "different anchor promise",
                ModelTier::Final,
            ),
        ] {
            let err = verifier
                .verify_pair(&fulfillment_pair(tests_source, entry_declaration, anchor_text), tier)
                .unwrap_err();

            let SemanticVerifierError::VerifierPort(message) = err;
            assert_eq!(message.as_str(), "profile unavailable");
        }
    }

    #[test]
    fn pass_verdict_decodes_to_fulfilled_with_citation() {
        let verdict = adapter(r#"{"kind":"pass","citation":"asserts empty input is rejected","reason":null,"category":null}"#)
            .verify_pair(
                &fulfillment_pair(
                    "assert!(User::new(\"\").is_err())",
                    "struct User",
                    "rejects empty input",
                ),
                ModelTier::Final,
            )
            .unwrap();
        match verdict {
            ObligationFulfillmentVerdict::Fulfilled { citation } => {
                assert_eq!(citation.as_str(), "asserts empty input is rejected");
            }
            other => panic!("expected Fulfilled, got {other:?}"),
        }
    }

    #[test]
    fn test_fulfillment_adapter_pair_uses_fulfillment_capability_and_citation() {
        let captured: Arc<Mutex<Option<(ResolvedExecution, String)>>> = Arc::new(Mutex::new(None));
        let captured_clone = Arc::clone(&captured);
        let runner: Arc<SemanticVerifierRunner> = Arc::new(move |resolved, prompt| {
            *captured_clone.lock().unwrap() = Some((resolved, prompt));
            Ok(
                r#"{"kind":"pass","citation":"assert_eq!(actual, expected)","reason":null,"category":null}"#
                    .to_owned(),
            )
        });
        let adapter = ObligationFulfillmentVerifierAdapter::with_runner(profiles(), runner);

        let verdict = adapter
            .verify_pair(
                &fulfillment_pair(
                    "assert_eq!(actual, expected);",
                    "struct Entry",
                    "the operation returns the expected value",
                ),
                ModelTier::Final,
            )
            .unwrap();

        assert!(matches!(verdict, ObligationFulfillmentVerdict::Fulfilled { .. }));
        let (resolved, prompt) = captured.lock().unwrap().clone().unwrap();
        assert!(matches!(
            resolved,
            ResolvedExecution::ProviderCli { model, .. } if model.as_str() == "claude-opus-4-8"
        ));
        assert!(prompt.starts_with(
            "You are an obligation-fulfillment verifier. Decide whether the provided test source"
        ));
        let payload = prompt.split_once("\n\n## Obligation identity\n").unwrap().1;
        assert_eq!(
            payload,
            "entry_key: Entry\nobligation_kind: contract\n\
item_identifier: trait_method:verify\n\n\
## Obligation brief\nverify the entry-local contract\n\n\
## Entry declaration\nstruct Entry\n\n\
## Specification element\nsection: in_scope\nelement_id: IN-01\n\
text: the operation returns the expected value\n\n\
## Bound test source\nassert_eq!(actual, expected);\n"
        );
        for marker in [
            "entry_key: Entry",
            "obligation_kind: contract",
            "item_identifier: trait_method:verify",
            "## Obligation brief\nverify the entry-local contract",
            "## Entry declaration\nstruct Entry",
            "## Specification element\nsection: in_scope\nelement_id: IN-01\ntext: the operation returns the expected value",
            "## Bound test source\nassert_eq!(actual, expected);",
            "An out-of-scope element marks a boundary and does not become an affirmative implementation requirement.",
            "Do not blanket-exclude or automatically waive a reference merely because it is out of scope;",
            "For a declaration that handles one already-selected input, do not demand caller-side selection or configuration loading;",
            "This adapter delegates semantic comparison to the configured provider;",
            "verdicts and provider errors are preserved.",
            "If the target-owned promise lacks direct evidence, return fail with central_unverified;",
        ] {
            assert!(prompt.contains(marker), "missing forwarded verifier input: {marker}");
        }
    }

    #[test]
    fn test_fulfillment_adapter_propagates_provider_errors() {
        let runner: Arc<SemanticVerifierRunner> =
            Arc::new(|_, _| Err(semantic_verifier_error("configured provider failed")));
        let adapter = ObligationFulfillmentVerifierAdapter::with_runner(profiles(), runner);

        let err = adapter
            .verify_pair(&fulfillment_pair("tests", "entry", "target promise"), ModelTier::Final)
            .unwrap_err();

        let SemanticVerifierError::VerifierPort(message) = err;
        assert_eq!(message.as_str(), "configured provider failed");
    }

    #[test]
    fn render_prompt_preserves_authoritative_section_even_when_identifier_disagrees() {
        let prompt = ObligationFulfillmentVerifierAdapter::render_prompt(
            &fulfillment_pair_with_spec_element(
                "OUT_OF_SCOPE_TEST",
                "DECL_MARKER",
                SpecSectionKind::OutOfScope,
                "IN-01",
                "same specification text",
            ),
        );

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
            let prompt = ObligationFulfillmentVerifierAdapter::render_prompt(
                &fulfillment_pair_with_spec_element(
                    "TEST_SOURCE",
                    "ENTRY_DECLARATION",
                    section,
                    element_id,
                    "SHARED_SPECIFICATION_TEXT",
                ),
            );
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
        let prompt = ObligationFulfillmentVerifierAdapter::render_prompt(&fulfillment_pair(
            "TEST_SOURCE",
            "ENTRY_DECLARATION",
            "SPECIFICATION_TEXT",
        ));

        for marker in [
            "An out-of-scope element marks a boundary and does not become an affirmative implementation requirement.",
            "Do not blanket-exclude or automatically waive a reference merely because it is out of scope; inspect whether the target-owned edge is affected.",
            "Do not turn a non-guarantee into a prohibition; judge an explicit prohibition only when it belongs to the target responsibility.",
            "For a declaration that handles one already-selected input, do not demand caller-side selection or configuration loading; detect selection defects only for a declaration that owns selection.",
            "This adapter delegates semantic comparison to the configured provider; it does not independently parse test meaning.",
            "Still fail when the target-owned behaviour is contradicted or its central behaviour is not verified;",
            "evidence for another item does not satisfy this target item.",
            "if required comparison material is actually missing, return pending.",
        ] {
            assert!(prompt.contains(marker), "missing ownership guidance: {marker}");
        }
    }

    #[test]
    fn render_prompt_keeps_single_input_and_selection_responsibilities_distinct() {
        let single_input = ObligationFulfillmentVerifierAdapter::render_prompt(
            &fulfillment_pair_with_spec_element(
                "assert_selected_input_is_decoded",
                "SingleInputDecoder { selected_input: Input }",
                SpecSectionKind::InScope,
                "IN-01",
                "the decoder processes one already-selected input and does not choose configuration",
            ),
        );
        let input_selector = ObligationFulfillmentVerifierAdapter::render_prompt(
            &fulfillment_pair_with_spec_element(
                "assert_configuration_selects_input",
                "InputSelector { configuration: Settings }",
                SpecSectionKind::InScope,
                "IN-01",
                "the selector owns choosing an input from configuration",
            ),
        );

        assert!(single_input.contains("SingleInputDecoder { selected_input: Input }"));
        assert!(single_input.contains(
            "the decoder processes one already-selected input and does not choose configuration"
        ));
        assert!(input_selector.contains("InputSelector { configuration: Settings }"));
        assert!(input_selector.contains("the selector owns choosing an input from configuration"));
        assert_ne!(single_input, input_selector);
    }

    #[test]
    fn test_fulfillment_adapter_preserves_fail_categories_and_pending_results() {
        let pair = fulfillment_pair("TEST_SOURCE", "ENTRY_DECLARATION", "SPECIFICATION_TEXT");

        let contradiction = adapter(
            r#"{"kind":"fail","citation":null,"reason":"the test asserts the opposite","category":"contradiction"}"#,
        )
        .verify_pair(&pair, ModelTier::Final)
        .unwrap();
        assert!(matches!(
            contradiction,
            ObligationFulfillmentVerdict::Fail {
                category: FulfillmentFailCategory::Contradiction,
                ..
            }
        ));

        let substitution = adapter(
            r#"{"kind":"fail","citation":null,"reason":"the test covers another responsibility","category":"substitution"}"#,
        )
        .verify_pair(&pair, ModelTier::Final)
        .unwrap();
        assert!(matches!(
            substitution,
            ObligationFulfillmentVerdict::Fail {
                category: FulfillmentFailCategory::Substitution,
                ..
            }
        ));

        let missing_evidence = adapter(
            r#"{"kind":"fail","citation":null,"reason":"direct evidence is missing","category":"central_unverified"}"#,
        )
        .verify_pair(&pair, ModelTier::Final)
        .unwrap();
        assert!(matches!(
            missing_evidence,
            ObligationFulfillmentVerdict::Fail {
                category: FulfillmentFailCategory::CentralUnverified,
                ..
            }
        ));

        let pending =
            adapter(r#"{"kind":"pending","citation":null,"reason":null,"category":null}"#)
                .verify_pair(&pair, ModelTier::Final)
                .unwrap();
        assert_eq!(pending, ObligationFulfillmentVerdict::Pending);
    }

    #[test]
    fn fail_verdict_decodes_with_explicit_category() {
        let verdict = adapter(r#"{"kind":"fail","citation":null,"reason":"asserts the opposite","category":"contradiction"}"#)
            .verify_pair(&fulfillment_pair("tests", "decl", "anchor"), ModelTier::Final)
            .unwrap();
        match verdict {
            ObligationFulfillmentVerdict::Fail { category, reason } => {
                assert_eq!(category, FulfillmentFailCategory::Contradiction);
                assert_eq!(reason.as_str(), "asserts the opposite");
            }
            other => panic!("expected Fail, got {other:?}"),
        }
    }

    #[test]
    fn fail_verdict_without_category_defaults_to_central_unverified() {
        let verdict = adapter(r#"{"kind":"fail","citation":null,"reason":"happy path only"}"#)
            .verify_pair(&fulfillment_pair("tests", "decl", "anchor"), ModelTier::Fast)
            .unwrap();
        match verdict {
            ObligationFulfillmentVerdict::Fail { category, .. } => {
                assert_eq!(category, FulfillmentFailCategory::CentralUnverified);
            }
            other => panic!("expected Fail, got {other:?}"),
        }
    }

    #[test]
    fn pass_without_citation_fails_closed() {
        let err = adapter(r#"{"kind":"pass","citation":null,"reason":null,"category":null}"#)
            .verify_pair(&fulfillment_pair("tests", "decl", "anchor"), ModelTier::Final)
            .unwrap_err();
        let SemanticVerifierError::VerifierPort(message) = err;
        assert!(message.as_str().contains("citation"));
    }

    #[test]
    fn fail_without_reason_fails_closed() {
        let err =
            adapter(r#"{"kind":"fail","citation":null,"reason":null,"category":"substitution"}"#)
                .verify_pair(&fulfillment_pair("tests", "decl", "anchor"), ModelTier::Final)
                .unwrap_err();
        let SemanticVerifierError::VerifierPort(message) = err;
        assert!(message.as_str().contains("reason"));
    }

    #[test]
    fn pending_verdict_is_preserved() {
        let verdict =
            adapter(r#"{"kind":"pending","citation":null,"reason":null,"category":null}"#)
                .verify_pair(&fulfillment_pair("tests", "decl", "anchor"), ModelTier::Final)
                .unwrap();
        assert_eq!(verdict, ObligationFulfillmentVerdict::Pending);
    }

    #[test]
    fn unknown_field_fails_closed() {
        let err =
            adapter(r#"{"kind":"pass","citation":"c","reason":null,"category":null,"extra":true}"#)
                .verify_pair(&fulfillment_pair("tests", "decl", "anchor"), ModelTier::Final)
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
        let adapter = ObligationFulfillmentVerifierAdapter::with_runner(profile, stub_runner("{}"));
        let err = adapter
            .verify_pair(&fulfillment_pair("tests", "decl", "anchor"), ModelTier::Final)
            .unwrap_err();
        let SemanticVerifierError::VerifierPort(message) = err;
        assert!(message.as_str().contains("obligation-fulfillment-verifier"));
    }

    #[test]
    fn fast_tier_resolves_fast_model() {
        let captured: Arc<Mutex<Option<ResolvedExecution>>> = Arc::new(Mutex::new(None));
        let captured_clone = Arc::clone(&captured);
        let runner: Arc<SemanticVerifierRunner> = Arc::new(move |resolved, _prompt| {
            *captured_clone.lock().unwrap() = Some(resolved);
            Ok(r#"{"kind":"pending","citation":null,"reason":null,"category":null}"#.to_owned())
        });
        let adapter = ObligationFulfillmentVerifierAdapter::with_runner(profiles(), runner);
        adapter.verify_pair(&fulfillment_pair("tests", "decl", "anchor"), ModelTier::Fast).unwrap();
        let resolved = captured.lock().unwrap().clone().unwrap();
        assert!(matches!(
            resolved,
            ResolvedExecution::ProviderCli { model, .. } if model.as_str() == "claude-haiku-4-5"
        ));
    }

    #[test]
    fn render_prompt_embeds_the_structured_specification_element() {
        let prompt = ObligationFulfillmentVerifierAdapter::render_prompt(
            &fulfillment_pair_with_spec_element(
                "TEST_BODY_MARKER",
                "DECL_MARKER",
                SpecSectionKind::OutOfScope,
                "OUT-04",
                "ANCHOR_MARKER",
            ),
        );
        assert!(prompt.contains("TEST_BODY_MARKER"));
        assert!(prompt.contains("DECL_MARKER"));
        assert!(prompt.contains(
            "## Specification element\nsection: out_of_scope\nelement_id: OUT-04\ntext: ANCHOR_MARKER\n"
        ));
    }

    #[test]
    fn render_prompt_keeps_same_text_in_distinct_spec_sections_distinct() {
        let in_scope = ObligationFulfillmentVerifierAdapter::render_prompt(
            &fulfillment_pair_with_spec_element(
                "tests",
                "entry",
                SpecSectionKind::InScope,
                "IN-01",
                "same text",
            ),
        );
        let out_of_scope = ObligationFulfillmentVerifierAdapter::render_prompt(
            &fulfillment_pair_with_spec_element(
                "tests",
                "entry",
                SpecSectionKind::OutOfScope,
                "OUT-01",
                "same text",
            ),
        );

        assert_ne!(in_scope, out_of_scope);
        assert!(in_scope.contains("section: in_scope\nelement_id: IN-01\ntext: same text"));
        assert!(
            out_of_scope.contains("section: out_of_scope\nelement_id: OUT-01\ntext: same text")
        );
    }

    #[test]
    fn render_prompt_carries_target_identity_and_brief_with_payload() {
        let prompt = ObligationFulfillmentVerifierAdapter::render_prompt(&fulfillment_pair(
            "tests", "entry", "anchor",
        ));

        // These are structural payload checks. The verifier's semantic judgment
        // is exercised by provider/calibration paths, not by freezing prose
        // fragments in this adapter test.
        for marker in [
            "## Obligation identity",
            "entry_key: Entry",
            "obligation_kind: contract",
            "item_identifier: trait_method:verify",
            "## Obligation brief\nverify the entry-local contract",
            "## Entry declaration\nentry",
            "## Specification element\nsection: in_scope\nelement_id: IN-01\ntext: anchor",
            "## Bound test source\ntests",
        ] {
            assert!(prompt.contains(marker), "missing prompt payload marker: {marker}");
        }
        assert!(!prompt.contains("implementer-authored waiver reason"));
    }

    #[test]
    #[ignore = "host-only semantic evidence; run explicitly with configured provider credentials"]
    fn test_configured_provider_distinguishes_memory_and_persistence_ownership() {
        // This is deliberately the only live-provider reproducer in this module:
        // deterministic adapter tests prove forwarding, while these bounded
        // owner/outcome pairs calibrate the provider's semantic boundary.
        let workspace_root =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap();
        let profiles_path = workspace_root.join(".harness/config/agent-profiles.json");
        let profiles = AgentProfiles::load(&workspace_root, &profiles_path).unwrap();
        let verifier = ObligationFulfillmentVerifierAdapter::new(profiles, workspace_root);

        let pair_for = |tests_source: &str,
                        entry_key: &str,
                        item_identifier: &str,
                        brief: &str,
                        entry_declaration: &str| {
            ObligationFulfillmentPair::new(
                TestsSource::try_new(tests_source.to_owned()).unwrap(),
                EntryDeclaration::try_new(entry_declaration.to_owned()).unwrap(),
                SpecElementRef::new(
                    SpecSectionKind::InScope,
                    SpecElementId::try_new("IN-01".to_owned()).unwrap(),
                    "An in-memory name index keeps independently named values distinct and available; a separate persistence path target owns a project-local storage location and excludes user-global placement."
                        .to_owned(),
                ),
                TestObligationId::new(
                    CatalogueEntryKey::try_new(entry_key.to_owned()).unwrap(),
                    TestObligationKind::Contract,
                    TestObligationItemIdentifier::try_new(item_identifier.to_owned()).unwrap(),
                ),
                TestObligationBrief::try_new(brief.to_owned()).unwrap(),
            )
        };

        let memory_positive = r#"
            #[test]
            fn memory_names_coexist_without_storage() {
                let mut names = std::collections::BTreeMap::new();
                names.insert("alpha", 1_u8);
                names.insert("beta", 2_u8);
                assert_eq!(names.get("alpha"), Some(&1_u8));
                assert_eq!(names.get("beta"), Some(&2_u8));
            }
        "#;
        // The second memory probe drops the independently named value that the
        // target-owned contract requires; a false pass would hide missing input.
        let memory_dropped_input_negative = r#"
            #[test]
            fn memory_one_name_leaves_independent_names_unverified() {
                let mut names = std::collections::BTreeMap::new();
                names.insert("alpha", 1_u8);
                assert_eq!(names.get("alpha"), Some(&1_u8));
            }
        "#;
        let persistence_positive = r#"
            #[test]
            fn persistence_path_is_project_local_and_not_user_global() {
                use std::path::{Path, PathBuf};

                fn project_local_storage_location(project_root: &Path) -> PathBuf {
                    project_root.join(".state").join("values.data")
                }

                let project_root = Path::new("project-root");
                let user_global_root = Path::new("user-global-root");
                let location = project_local_storage_location(project_root);
                assert!(location.starts_with(project_root));
                assert!(!location.starts_with(user_global_root));
            }
        "#;
        // The second persistence probe exercises a false-success implementation:
        // it chooses a user-global location instead of the target-owned project
        // local location.
        let persistence_false_success_negative = r#"
            #[test]
            fn persistence_path_uses_user_global_location() {
                use std::path::{Path, PathBuf};

                fn user_global_storage_location(user_global_root: &Path) -> PathBuf {
                    user_global_root.join("values.data")
                }

                let project_root = Path::new("project-root");
                let user_global_root = Path::new("user-global-root");
                let location = user_global_storage_location(user_global_root);
                assert!(location.starts_with(user_global_root));
                assert!(!location.starts_with(project_root));
            }
        "#;

        let cases = [
            (
                "memory-positive",
                memory_positive,
                "InMemoryNameIndex",
                "method:lookup",
                "verify independently named values remain distinct and available in memory",
                "InMemoryNameIndex { names: mapping of independently named values }",
                true,
            ),
            (
                "memory-dropped-input-negative",
                memory_dropped_input_negative,
                "InMemoryNameIndex",
                "method:lookup",
                "verify independently named values remain distinct and available in memory",
                "InMemoryNameIndex { names: mapping of independently named values }",
                false,
            ),
            (
                "persistence-positive",
                persistence_positive,
                "PersistencePathTarget",
                "field:storage_location",
                "verify the storage location is project-local and excludes user-global placement",
                "PersistencePathTarget { storage_location: project-local path; user-global placement: excluded }",
                true,
            ),
            (
                "persistence-false-success-negative",
                persistence_false_success_negative,
                "PersistencePathTarget",
                "field:storage_location",
                "verify the storage location is project-local and excludes user-global placement",
                "PersistencePathTarget { storage_location: project-local path; user-global placement: excluded }",
                false,
            ),
        ];

        for (case, tests_source, entry_key, item_identifier, brief, declaration, expect_pass) in
            cases
        {
            let verdict = verifier
                .verify_pair(
                    &pair_for(tests_source, entry_key, item_identifier, brief, declaration),
                    ModelTier::Fast,
                )
                .unwrap();
            assert_eq!(
                matches!(verdict, ObligationFulfillmentVerdict::Fulfilled { .. }),
                expect_pass,
                "unexpected {case} verdict: {verdict:?}"
            );
        }
    }

    #[test]
    #[ignore = "host-only semantic calibration; run explicitly with configured provider credentials"]
    fn test_configured_provider_calibrates_section_and_ownership_boundaries() {
        let workspace_root =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap();
        let profiles_path = workspace_root.join(".harness/config/agent-profiles.json");
        let profiles = AgentProfiles::load(&workspace_root, &profiles_path).unwrap();
        let verifier = ObligationFulfillmentVerifierAdapter::new(profiles, workspace_root);

        struct Case {
            name: &'static str,
            tests_source: &'static str,
            entry_key: &'static str,
            item_identifier: &'static str,
            brief: &'static str,
            declaration: &'static str,
            section: SpecSectionKind,
            element_id: &'static str,
            spec_text: &'static str,
            expect_pass: bool,
        }

        let cases = [
            Case {
                name: "selected-input-responsibility",
                tests_source: r#"
                    #[test]
                    fn decoder_parses_the_input_already_selected_by_its_caller() {
                        let selected = CapturedInput::from_bytes(b"{\"id\": 7}");
                        let decoder = SelectedInputDecoder::new(selected);

                        assert_eq!(decoder.decode().unwrap().id, 7);
                    }
                "#,
                entry_key: "SelectedInputDecoder",
                item_identifier: "method:decode",
                brief: "verify decoding of one already-selected input without owning input selection",
                declaration: "SelectedInputDecoder { selected_input: CapturedInput }",
                section: SpecSectionKind::InScope,
                element_id: "IN-02",
                spec_text: "The decoder receives one already-selected input and parses it; input selection and configuration loading belong to the caller.",
                expect_pass: true,
            },
            Case {
                name: "selection-owned-fallback",
                tests_source: r#"
                    #[test]
                    fn selector_falls_back_to_stdout_for_an_invalid_configured_input() {
                        let configuration = InputConfiguration::selected_stream("stderr");
                        let streams = Streams { stdout: valid_json(), stderr: invalid_json() };
                        let selected = InputSelector::new(configuration).select(&streams).unwrap();

                        assert_eq!(selected, streams.stdout);
                    }
                "#,
                entry_key: "InputSelector",
                item_identifier: "method:select",
                brief: "verify that the selector uses the configured input and does not fall back",
                declaration: "InputSelector { configuration: InputConfiguration, streams: Streams }",
                section: SpecSectionKind::InScope,
                element_id: "IN-02",
                spec_text: "The input selector owns choosing the configured input and must reject an invalid configured input rather than fall back to another stream.",
                expect_pass: false,
            },
            Case {
                name: "out-of-scope-non-guarantee",
                tests_source: r#"
                    #[test]
                    fn local_decoder_processes_selected_input_without_remote_sync() {
                        let selected = CapturedInput::from_bytes(b"{\"id\": 7}");
                        let decoder = LocalInputDecoder::new(selected);

                        assert_eq!(decoder.decode().unwrap().id, 7);
                    }
                "#,
                entry_key: "LocalInputDecoder",
                item_identifier: "method:decode",
                brief: "verify local decoding without assuming remote synchronization",
                declaration: "LocalInputDecoder { selected_input: CapturedInput; remote_sync: not owned }",
                section: SpecSectionKind::OutOfScope,
                element_id: "OUT-01",
                spec_text: "Remote synchronization is outside this component's scope and is not guaranteed; the local decoder only processes its already-selected input.",
                expect_pass: true,
            },
            Case {
                name: "non-guarantee-turned-into-prohibition",
                tests_source: r#"
                    #[test]
                    fn local_decoder_rejects_input_when_remote_sync_is_unavailable() {
                        let selected = CapturedInput::from_bytes(b"{\"id\": 7}");
                        let decoder = LocalInputDecoder::new(selected);

                        assert!(decoder.decode_without_remote_sync().is_err());
                    }
                "#,
                entry_key: "LocalInputDecoder",
                item_identifier: "method:decode",
                brief: "verify local decoding without assuming remote synchronization",
                declaration: "LocalInputDecoder { selected_input: CapturedInput; remote_sync: not owned }",
                section: SpecSectionKind::OutOfScope,
                element_id: "OUT-01",
                spec_text: "Remote synchronization is outside this component's scope and is not guaranteed; the local decoder only processes its already-selected input.",
                expect_pass: false,
            },
            Case {
                name: "target-owned-prohibition",
                tests_source: r#"
                    #[test]
                    fn writer_uses_only_a_project_local_state_path() {
                        let writer = ProjectStateWriter::new("project-root");
                        let path = writer.state_path();

                        assert!(path.starts_with("project-root"));
                        assert!(!path.starts_with("user-global-root"));
                    }
                "#,
                entry_key: "ProjectStateWriter",
                item_identifier: "method:state_path",
                brief: "verify that state is written only below the project root",
                declaration: "ProjectStateWriter { project_root: Path; user_global_storage: prohibited }",
                section: SpecSectionKind::OutOfScope,
                element_id: "OUT-01",
                spec_text: "This component must not write state to a user-global location; only a project-local location is permitted.",
                expect_pass: true,
            },
            Case {
                name: "target-owned-prohibition-violation",
                tests_source: r#"
                    #[test]
                    fn writer_uses_a_user_global_state_path() {
                        let writer = ProjectStateWriter::new("project-root");
                        let path = writer.state_path();

                        assert!(path.starts_with("user-global-root"));
                        assert!(!path.starts_with("project-root"));
                    }
                "#,
                entry_key: "ProjectStateWriter",
                item_identifier: "method:state_path",
                brief: "verify that state is written only below the project root",
                declaration: "ProjectStateWriter { project_root: Path; user_global_storage: prohibited }",
                section: SpecSectionKind::OutOfScope,
                element_id: "OUT-01",
                spec_text: "This component must not write state to a user-global location; only a project-local location is permitted.",
                expect_pass: false,
            },
        ];

        for Case {
            name,
            tests_source,
            entry_key,
            item_identifier,
            brief,
            declaration,
            section,
            element_id,
            spec_text,
            expect_pass,
        } in cases
        {
            let pair = ObligationFulfillmentPair::new(
                TestsSource::try_new(tests_source.to_owned()).unwrap(),
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
                matches!(verdict, ObligationFulfillmentVerdict::Fulfilled { .. }),
                expect_pass,
                "unexpected {name} verdict: {verdict:?}"
            );
        }
    }

    #[test]
    fn test_fulfillment_verifier_fingerprint_hashes_prompt_contract() {
        let fingerprint = fulfillment_verifier_fingerprint();
        let expected_material = format!(
            "verifier_format_version={FULFILLMENT_PROMPT_FORMAT_VERSION}\n\
             preamble:\n{FULFILLMENT_PROMPT_PREAMBLE}\n\
             input_layout:\n{FULFILLMENT_PROMPT_INPUT_LAYOUT}\n"
        );
        let expected = Sha256ContentHasher::new().sha256(expected_material.as_bytes());

        assert_eq!(fingerprint.as_hash(), &expected);
        assert_ne!(
            fingerprint.as_hash(),
            &Sha256ContentHasher::new().sha256(FULFILLMENT_PROMPT_PREAMBLE.as_bytes())
        );
    }
}
