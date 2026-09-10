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
/// central-unverified) and the citation-required discipline so the model returns
/// a decodable verdict.
const FULFILLMENT_PROMPT_PREAMBLE: &str = "\
You are an obligation-fulfillment verifier. Decide whether the provided test source \
actually verifies the observable behaviour promised by the cited anchor for the target \
obligation item on the target catalogue entry. Use the entry declaration together with the \
typed obligation identity (entry key, obligation kind, and item identifier) and the obligation \
brief to identify the target item's observable promises within that anchor. The cited anchor \
remains authoritative: identity and brief provide context for locating the target-owned \
promise, but they must not override the anchor, add requirements, or invent ownership. \
A composite or shared anchor can contain promises owned by multiple entries or multiple items. \
Distinguish the target-owned portion from those other portions and judge only the target-owned \
portion for this pair. Do not fail because the bound tests omit behaviour owned by another entry \
or item. Still fail when the target-owned behaviour is contradicted or its central behaviour \
is not verified; evidence for another item does not satisfy this target item.

Reply with exactly one JSON object and nothing else:
{\"kind\": \"pass\" | \"fail\" | \"pending\", \"citation\": string | null, \"reason\": string | null, \"category\": \"contradiction\" | \"substitution\" | \"central_unverified\" | null}

- \"pass\": the bound tests fulfil the obligation. \"citation\" MUST quote verbatim the \
part of the test source that fulfils it. A pass without a citation is invalid.
If the tests fully verify the target item's observable promises within this cited anchor, return \
pass even when the anchor contains promises owned by other entries or items.
- \"fail\": set \"reason\" and \"category\": \"contradiction\" (a test asserts the opposite of \
the target-owned promise), \"substitution\" (the tests cite the anchor but verify content \
unrelated to the target item's promise), or \"central_unverified\" (no contradiction or \
irrelevance, but the central target-owned part of the cited anchor's promise is left unverified; \
do NOT demand from this edge promise parts belonging to other entries' or items' responsibilities).
- \"pending\": you cannot confirm fulfilment from the material provided.";

/// Returns the SHA-256 content hash of this verifier's judging prompt preamble.
///
/// The hash deliberately excludes execution provider, model, and tier: it
/// invalidates cache entries only when the judging instructions change.
#[must_use]
pub fn fulfillment_verifier_fingerprint() -> VerifierPromptFingerprint {
    VerifierPromptFingerprint::new(
        Sha256ContentHasher::new().sha256(FULFILLMENT_PROMPT_PREAMBLE.as_bytes()),
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
        format!(
            "{FULFILLMENT_PROMPT_PREAMBLE}\n\n\
             ## Obligation identity\nentry_key: {}\nobligation_kind: {}\nitem_identifier: {}\n\n\
             ## Obligation brief\n{}\n\n\
             ## Entry declaration\n{entry_declaration}\n\n\
             ## Anchor promise\n{anchor_text}\n\n\
             ## Bound test source\n{tests_source}\n",
            obligation_id.entry_key().as_str(),
            obligation_id.obligation_kind().as_kebab(),
            obligation_id.item_identifier().as_str(),
            obligation_brief.as_str(),
            entry_declaration = pair.entry_declaration().as_str(),
            anchor_text = pair.anchor_text().as_str(),
            tests_source = pair.tests_source().as_str(),
        )
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

    use domain::tddd::catalogue_v2::CatalogueEntryKey;
    use domain::tddd::semantic_verify::ModelTier;
    use domain::tddd::test_obligation::ids::{
        TestObligationBrief, TestObligationId, TestObligationItemIdentifier,
    };
    use domain::tddd::test_obligation::pair::{AnchorText, EntryDeclaration, TestsSource};
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
        anchor_text: &str,
    ) -> ObligationFulfillmentPair {
        ObligationFulfillmentPair::new(
            TestsSource::try_new(tests_source.to_owned()).unwrap(),
            EntryDeclaration::try_new(entry_declaration.to_owned()).unwrap(),
            AnchorText::try_new(anchor_text.to_owned()).unwrap(),
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
        assert!(prompt.contains("obligation-fulfillment verifier"));
        assert!(prompt.contains("assert_eq!(actual, expected);"));
        assert!(prompt.contains("struct Entry"));
        assert!(prompt.contains("the operation returns the expected value"));
        assert!(prompt.contains("entry_key: Entry"));
        assert!(prompt.contains("obligation_kind: contract"));
        assert!(prompt.contains("trait_method:verify"));
        assert!(prompt.contains("verify the entry-local contract"));
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
    fn render_prompt_embeds_all_three_pair_components() {
        let prompt = ObligationFulfillmentVerifierAdapter::render_prompt(&fulfillment_pair(
            "TEST_BODY_MARKER",
            "DECL_MARKER",
            "ANCHOR_MARKER",
        ));
        assert!(prompt.contains("TEST_BODY_MARKER"));
        assert!(prompt.contains("DECL_MARKER"));
        assert!(prompt.contains("ANCHOR_MARKER"));
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
            "## Anchor promise\nanchor",
            "## Bound test source\ntests",
        ] {
            assert!(prompt.contains(marker), "missing prompt payload marker: {marker}");
        }
        assert!(!prompt.contains("implementer-authored waiver reason"));
    }

    #[test]
    #[ignore = "host-only semantic evidence; run explicitly with configured provider credentials"]
    fn test_configured_provider_distinguishes_memory_and_persistence_ownership() {
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
                AnchorText::try_new(
                    "An in-memory name index keeps independently named values distinct and available; a separate persistence path target owns a project-local storage location and excludes user-global placement."
                        .to_owned(),
                )
                .unwrap(),
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
        let memory_negative = r#"
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
        let persistence_negative = r#"
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
                "memory-negative",
                memory_negative,
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
                "persistence-negative",
                persistence_negative,
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
    fn test_fulfillment_verifier_fingerprint_hashes_prompt_preamble_only() {
        let fingerprint = fulfillment_verifier_fingerprint();
        let expected = Sha256ContentHasher::new().sha256(FULFILLMENT_PROMPT_PREAMBLE.as_bytes());

        assert_eq!(fingerprint.as_hash(), &expected);
    }
}
