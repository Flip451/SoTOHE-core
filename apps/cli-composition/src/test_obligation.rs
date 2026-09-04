//! Composition root for `sotp test-obligation`.

use std::num::NonZeroU8;
use std::path::PathBuf;
use std::sync::Arc;

use domain::tddd::test_obligation::errors::SemanticVerifierError;
use domain::tddd::test_obligation::pair::{ObligationFulfillmentPair, WaiverPair};
use domain::tddd::test_obligation::ports::{ObligationFulfillmentVerifierPort, WaiverVerifierPort};
use domain::tddd::test_obligation::projection::RoleObligationItemsProjector;
use domain::tddd::test_obligation::verdict::{
    ObligationFulfillmentCacheKey, ObligationFulfillmentVerdict, WaiverCacheKey, WaiverVerdict,
};
use infrastructure::agent_profiles::{AGENT_PROFILES_PATH, AgentProfiles};
use infrastructure::impl_plan_reader::FsImplPlanReader;
use infrastructure::spec::FsSpecDocumentLoader;
use infrastructure::task_contract_reader::FsTaskContractReader;
use infrastructure::tddd::tddd_catalogue_document_loader::FsCatalogueDocumentLoader;
use infrastructure::test_obligation::bindings_codec::JsonTestBindingsCodec;
use infrastructure::test_obligation::fulfillment_cache_codec::JsonObligationFulfillmentCacheCodec;
use infrastructure::test_obligation::fulfillment_escalation_driver::ObligationFulfillmentEscalationDriver;
use infrastructure::test_obligation::fulfillment_verifier::{
    FailingObligationFulfillmentVerifier, ObligationFulfillmentVerifierAdapter,
    fulfillment_verifier_fingerprint,
};
use infrastructure::test_obligation::obligations_codec::JsonObligationsCodec;
use infrastructure::test_obligation::rules_codec::JsonTestObligationRulesLoader;
use infrastructure::test_obligation::sha256_content_hasher::Sha256ContentHasher;
use infrastructure::test_obligation::source_scanner::SynTestSourceScanner;
use infrastructure::test_obligation::waiver_cache_codec::JsonWaiverCacheCodec;
use infrastructure::test_obligation::waiver_escalation_driver::WaiverEscalationDriver;
use infrastructure::test_obligation::waiver_verifier::{
    FailingWaiverVerifier, WaiverVerifierAdapter, waiver_verifier_fingerprint,
};
use infrastructure::track::track_status_reader_adapter::FsTrackStatusReaderAdapter;
use usecase::catalogue_document_loader::AttestedCatalogueDocumentLoaderPort;
use usecase::pre_review_gate::{ImplPlanReaderPort, TaskContractReaderPort};
use usecase::semantic_verdict_core::driver::SemanticEscalationDriverPort;
use usecase::semantic_verdict_core::probe::SemanticCalibrationProbeConfig;
use usecase::test_obligation::bindings_skeleton::TestBindingsSkeletonInteractor;
use usecase::test_obligation::check::CheckTestObligationsInteractor;
use usecase::test_obligation::derive::DeriveTestObligationsInteractor;
use usecase::test_obligation::evaluate::{
    EvaluateTestObligationsInteractor, TestObligationEvaluateConfig,
};
use usecase::test_obligation::ports::ObligationFulfillmentCachePort;
use usecase::test_obligation::results::TestObligationResultsInteractor;

const TEST_OBLIGATION_RULES_PATH: &str = ".harness/config/test-obligation-rules.json";

/// Composition root for `sotp test-obligation` handler and port wiring.
pub struct TestObligationCompositionRoot {
    pub workspace_root: PathBuf,
    pub config_path: PathBuf,
}

impl TestObligationCompositionRoot {
    /// Builds a test-obligation composition root.
    #[must_use]
    pub fn new(workspace_root: PathBuf, config_path: PathBuf) -> Self {
        Self { workspace_root, config_path }
    }

    /// Discovers the current git worktree and builds the default root.
    ///
    /// # Errors
    ///
    /// Returns an infrastructure error when the current directory is not inside
    /// a discoverable git worktree.
    #[cfg(not(doc))]
    pub fn discover() -> Result<Self, crate::CompositionError> {
        let repo = infrastructure::git_cli::SystemGitRepo::discover().map_err(|e| {
            crate::CompositionError::Infrastructure(format!("cannot discover git repo: {e}"))
        })?;
        let workspace_root = repo.root().to_path_buf();
        Ok(Self::new(workspace_root.clone(), workspace_root.join(TEST_OBLIGATION_RULES_PATH)))
    }

    /// Reads the current branch for active-track validation.
    ///
    /// # Errors
    ///
    /// Returns an infrastructure error when git discovery fails, branch reading
    /// fails, or HEAD is detached.
    #[cfg(not(doc))]
    pub fn current_branch(&self) -> Result<String, crate::CompositionError> {
        infrastructure::git_cli::SystemGitRepo::discover_from(&self.workspace_root)
            .map_err(|e| {
                crate::CompositionError::Infrastructure(format!("cannot discover git repo: {e}"))
            })?
            .current_branch()
            .map_err(|e| {
                crate::CompositionError::Infrastructure(format!("cannot read current branch: {e}"))
            })?
            .ok_or_else(|| {
                crate::CompositionError::Infrastructure(
                    "cannot read current branch: HEAD is detached".to_owned(),
                )
            })
    }

    /// Wires the derive handler.
    #[must_use]
    pub fn derive_handler(
        &self,
    ) -> cli_driver::test_obligation::derive::TestObligationDeriveHandler {
        let service = Arc::new(DeriveTestObligationsInteractor::new(
            self.rules_loader(),
            self.obligations_codec(),
            self.spec_loader(),
            self.catalogue_loader(),
            Arc::new(FsTrackStatusReaderAdapter::new()),
            self.items_dir(),
            RoleObligationItemsProjector::new(),
        ));
        cli_driver::test_obligation::derive::TestObligationDeriveHandler::new(
            service,
            self.workspace_root.clone(),
        )
    }

    /// Wires the check handler.
    #[must_use]
    pub fn check_handler(&self) -> cli_driver::test_obligation::check::TestObligationCheckHandler {
        let task_contract_reader = self.task_contract_reader();
        let impl_plan_reader = self.impl_plan_reader();
        let service = Arc::new(CheckTestObligationsInteractor::new(
            self.rules_loader(),
            self.obligations_codec(),
            self.bindings_codec(),
            self.source_scanner(),
            self.fulfillment_cache(),
            self.waiver_cache(),
            fulfillment_verifier_fingerprint(),
            waiver_verifier_fingerprint(),
            self.spec_loader(),
            self.catalogue_loader(),
            task_contract_reader,
            impl_plan_reader,
        ));
        cli_driver::test_obligation::check::TestObligationCheckHandler::new(
            service,
            self.workspace_root.clone(),
        )
    }

    /// Wires the bindings-skeleton handler.
    #[must_use]
    pub fn bindings_skeleton_handler(
        &self,
    ) -> cli_driver::test_obligation::bindings_skeleton::TestBindingsSkeletonHandler {
        let service = Arc::new(TestBindingsSkeletonInteractor::new(self.obligations_codec()));
        cli_driver::test_obligation::bindings_skeleton::TestBindingsSkeletonHandler::new(service)
    }

    /// Wires the evaluate handler.
    #[must_use]
    pub fn evaluate_handler(
        &self,
    ) -> cli_driver::test_obligation::evaluate::TestObligationEvaluateHandler {
        let config = TestObligationEvaluateConfig::default();
        let service = Arc::new(EvaluateTestObligationsInteractor::new(
            self.obligations_codec(),
            self.bindings_codec(),
            self.source_scanner(),
            self.fulfillment_driver(),
            self.waiver_driver(),
            self.fulfillment_cache(),
            self.waiver_cache(),
            fulfillment_verifier_fingerprint(),
            waiver_verifier_fingerprint(),
            config,
            self.spec_loader(),
            self.catalogue_loader(),
            Arc::new(Sha256ContentHasher::new()),
        ));
        cli_driver::test_obligation::evaluate::TestObligationEvaluateHandler::new(
            service,
            self.workspace_root.clone(),
        )
    }

    /// Wires the results handler.
    #[must_use]
    pub fn results_handler(
        &self,
    ) -> cli_driver::test_obligation::results::TestObligationResultsHandler {
        let task_contract_reader = self.task_contract_reader();
        let impl_plan_reader = self.impl_plan_reader();
        let service = Arc::new(TestObligationResultsInteractor::new(
            self.obligations_codec(),
            self.bindings_codec(),
            self.source_scanner(),
            self.fulfillment_cache(),
            self.waiver_cache(),
            fulfillment_verifier_fingerprint(),
            waiver_verifier_fingerprint(),
            self.spec_loader(),
            self.catalogue_loader(),
            task_contract_reader,
            impl_plan_reader,
        ));
        cli_driver::test_obligation::results::TestObligationResultsHandler::new(
            service,
            self.workspace_root.clone(),
        )
    }

    fn items_dir(&self) -> PathBuf {
        self.workspace_root.join("track").join("items")
    }

    fn rules_loader(
        &self,
    ) -> Arc<dyn domain::tddd::test_obligation::ports::TestObligationRulesLoaderPort + Send + Sync>
    {
        Arc::new(JsonTestObligationRulesLoader::new(
            self.config_path.clone(),
            self.workspace_root.clone(),
        ))
    }

    fn obligations_codec(
        &self,
    ) -> Arc<dyn domain::tddd::test_obligation::ports::ObligationsArtifactPort + Send + Sync> {
        Arc::new(JsonObligationsCodec::new(self.items_dir()))
    }

    fn bindings_codec(
        &self,
    ) -> Arc<dyn domain::tddd::test_obligation::ports::TestBindingsArtifactPort + Send + Sync> {
        Arc::new(JsonTestBindingsCodec::new(self.items_dir()))
    }

    fn task_contract_reader(&self) -> Arc<dyn TaskContractReaderPort> {
        Arc::new(FsTaskContractReader::new(self.items_dir()))
    }

    fn impl_plan_reader(&self) -> Arc<dyn ImplPlanReaderPort> {
        Arc::new(FsImplPlanReader::new(self.items_dir()))
    }

    fn fulfillment_cache(&self) -> Arc<dyn ObligationFulfillmentCachePort + Send + Sync> {
        Arc::new(JsonObligationFulfillmentCacheCodec::new(self.items_dir()))
    }

    fn waiver_cache(
        &self,
    ) -> Arc<dyn domain::tddd::test_obligation::ports::WaiverCachePort + Send + Sync> {
        Arc::new(JsonWaiverCacheCodec::new(self.items_dir()))
    }

    fn source_scanner(
        &self,
    ) -> Arc<dyn domain::tddd::test_obligation::ports::TestSourceScannerPort + Send + Sync> {
        Arc::new(SynTestSourceScanner::new(self.workspace_root.clone()))
    }

    fn spec_loader(&self) -> Arc<dyn domain::SpecDocumentLoaderPort + Send + Sync> {
        // Pass the discovered workspace root as the resolution anchor so that
        // relative spec paths — like the `track/items/<id>/spec.json` that the
        // driver and interactors build from a `TrackId` — resolve against the
        // repo root, not the process cwd. `items_dir()` remains the trusted
        // containment root: no caller-supplied path can escape it.
        Arc::new(FsSpecDocumentLoader::new(self.workspace_root.clone(), self.items_dir()))
    }

    fn catalogue_loader(&self) -> Arc<dyn AttestedCatalogueDocumentLoaderPort + Send + Sync> {
        Arc::new(FsCatalogueDocumentLoader::new())
    }

    fn fulfillment_driver(
        &self,
    ) -> Arc<
        dyn SemanticEscalationDriverPort<
                ObligationFulfillmentPair,
                ObligationFulfillmentCacheKey,
                ObligationFulfillmentVerdict,
                SemanticVerifierError,
            > + Send
            + Sync,
    > {
        Arc::new(ObligationFulfillmentEscalationDriver::new(
            self.fulfillment_verifier(),
            default_probe_config(),
        ))
    }

    fn waiver_driver(
        &self,
    ) -> Arc<
        dyn SemanticEscalationDriverPort<
                WaiverPair,
                WaiverCacheKey,
                WaiverVerdict,
                SemanticVerifierError,
            > + Send
            + Sync,
    > {
        Arc::new(WaiverEscalationDriver::new(self.waiver_verifier(), default_probe_config()))
    }

    fn fulfillment_verifier(&self) -> Arc<dyn ObligationFulfillmentVerifierPort + Send + Sync> {
        match self.agent_profiles() {
            Ok(profiles) => Arc::new(ObligationFulfillmentVerifierAdapter::new(
                profiles,
                self.workspace_root.clone(),
            )),
            Err(message) => Arc::new(FailingObligationFulfillmentVerifier::from_message(&message)),
        }
    }

    fn waiver_verifier(&self) -> Arc<dyn WaiverVerifierPort + Send + Sync> {
        match self.agent_profiles() {
            Ok(profiles) => {
                Arc::new(WaiverVerifierAdapter::new(profiles, self.workspace_root.clone()))
            }
            Err(message) => Arc::new(FailingWaiverVerifier::from_message(&message)),
        }
    }

    fn agent_profiles(&self) -> Result<AgentProfiles, String> {
        let path = self.workspace_root.join(AGENT_PROFILES_PATH);
        AgentProfiles::load(&self.workspace_root, &path)
            .map_err(|e| format!("cannot load agent-profiles.json: {e}"))
    }
}

fn default_probe_config() -> SemanticCalibrationProbeConfig {
    const INJECTION: NonZeroU8 = match NonZeroU8::new(10) {
        Some(value) => value,
        None => NonZeroU8::MIN,
    };
    const THRESHOLD: NonZeroU8 = match NonZeroU8::new(90) {
        Some(value) => value,
        None => NonZeroU8::MIN,
    };
    SemanticCalibrationProbeConfig::new(INJECTION, THRESHOLD)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use cli_driver::test_obligation::check::TestObligationCheckInput;
    use cli_driver::test_obligation::derive::TestObligationDeriveInput;
    use cli_driver::test_obligation::results::TestObligationResultsInput;
    use domain::tddd::catalogue_v2::catalogue_impl_signals_ports::TrackStatusReaderPort;
    use domain::{ModelTier, TrackStatus};
    use infrastructure::agent_profiles::{ResolvedExecution, RoundType};
    use infrastructure::track::track_status_reader_adapter::FsTrackStatusReaderAdapter;
    use usecase::dry_write_driver::CapabilityName;

    use super::*;

    #[test]
    fn test_composition_root_new_holds_paths() {
        let root = TestObligationCompositionRoot::new(
            PathBuf::from("/repo"),
            PathBuf::from("/repo/.harness/config/test-obligation-rules.json"),
        );
        assert_eq!(root.items_dir(), PathBuf::from("/repo/track/items"));
    }

    #[test]
    fn test_spec_loader_resolves_relative_spec_path_against_workspace_root() {
        // Regression: when the composition root discovers the workspace root
        // from `git rev-parse`, the wired spec loader must anchor relative
        // spec paths at that root — not at the process cwd. This is what
        // makes `bin/sotp test-obligation ...` work from a repo subdirectory.
        let tmp = tempfile::tempdir().unwrap();
        let workspace_root = tmp.path().join("workspace");
        let items_root = workspace_root.join("track").join("items");
        std::fs::create_dir_all(items_root.join("example-track")).unwrap();
        std::fs::write(items_root.join("example-track").join("spec.json"), "{ malformed").unwrap();

        let root = TestObligationCompositionRoot::new(
            workspace_root.clone(),
            workspace_root.join(TEST_OBLIGATION_RULES_PATH),
        );
        let loader = root.spec_loader();
        let relative = PathBuf::from("track").join("items").join("example-track").join("spec.json");

        // Anchoring against workspace_root means the malformed body is
        // reached and reported as JsonParse. Without the fix the loader
        // would join against `current_dir()` (the cargo test cwd) and
        // return NotFound.
        let err = loader.load(&relative).unwrap_err();
        assert!(
            matches!(err, domain::SpecDocumentLoadError::JsonParse { .. }),
            "expected JsonParse from workspace-anchored resolution, got: {err:?}"
        );
    }

    #[test]
    fn test_default_probe_config_matches_evaluate_defaults() {
        let config = default_probe_config();
        assert_eq!(config.injection().get(), 10);
        assert_eq!(config.threshold().get(), 90);
    }

    #[test]
    fn test_composition_root_resolves_independent_fast_providers_for_verifier_capabilities() {
        let workspace = tempfile::tempdir().unwrap();
        let config_path = workspace.path().join(AGENT_PROFILES_PATH);
        std::fs::create_dir_all(config_path.parent().unwrap()).unwrap();
        std::fs::write(
            &config_path,
            r#"{
                "schema_version": 1,
                "providers": {
                    "codex": {
                        "label": "Codex",
                        "supported_reasoning_efforts": ["high"]
                    },
                    "claude": {
                        "label": "Claude",
                        "supported_reasoning_efforts": ["low", "high"]
                    },
                    "gemini": {
                        "label": "Gemini",
                        "supported_reasoning_efforts": ["high"]
                    }
                },
                "capabilities": {
                    "obligation-fulfillment-verifier": {
                        "provider": "codex",
                        "model": "fulfillment-final",
                        "fast_provider": "claude",
                        "fast_model": "fulfillment-fast",
                        "reasoning_effort": "high",
                        "fast_reasoning_effort": "low",
                        "execution_mode": "typed-pipeline"
                    },
                    "waiver-verifier": {
                        "provider": "claude",
                        "model": "waiver-final",
                        "fast_provider": "gemini",
                        "fast_model": "waiver-fast",
                        "reasoning_effort": "high",
                        "fast_reasoning_effort": "high",
                        "execution_mode": "typed-pipeline"
                    }
                }
            }"#,
        )
        .unwrap();
        let root = TestObligationCompositionRoot::new(
            workspace.path().to_path_buf(),
            workspace.path().join(TEST_OBLIGATION_RULES_PATH),
        );
        let profiles = root.agent_profiles().unwrap();

        let fulfillment = CapabilityName::try_new("obligation-fulfillment-verifier").unwrap();
        let waiver = CapabilityName::try_new("waiver-verifier").unwrap();
        let fulfillment_fast = profiles.resolve_execution(&fulfillment, RoundType::Fast).unwrap();
        let fulfillment_final = profiles.resolve_execution(&fulfillment, RoundType::Final).unwrap();
        let waiver_fast = profiles.resolve_execution(&waiver, RoundType::Fast).unwrap();
        let waiver_final = profiles.resolve_execution(&waiver, RoundType::Final).unwrap();

        assert!(matches!(
            fulfillment_fast,
            ResolvedExecution::ProviderCli { provider, model, .. }
                if provider.as_str() == "claude" && model.as_str() == "fulfillment-fast"
        ));
        assert!(matches!(
            fulfillment_final,
            ResolvedExecution::ProviderCli { provider, model, .. }
                if provider.as_str() == "codex" && model.as_str() == "fulfillment-final"
        ));
        assert!(matches!(
            waiver_fast,
            ResolvedExecution::ProviderCli { provider, model, .. }
                if provider.as_str() == "gemini" && model.as_str() == "waiver-fast"
        ));
        assert!(matches!(
            waiver_final,
            ResolvedExecution::ProviderCli { provider, model, .. }
                if provider.as_str() == "claude" && model.as_str() == "waiver-final"
        ));
    }

    #[test]
    fn test_composition_root_check_wires_fs_loader_and_fails_closed_for_partial_scope_and_invalid_rules()
     {
        const TRACK_ID: &str = "test-obligation-fulfillment-gate-2026-07-07";

        let workspace = tempfile::tempdir().unwrap();
        let workspace_root = workspace.path();
        let rules_path = workspace_root.join(TEST_OBLIGATION_RULES_PATH);
        let items_dir = workspace_root.join("track/items").join(TRACK_ID);
        let source_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");

        std::fs::create_dir_all(rules_path.parent().unwrap()).unwrap();
        std::fs::create_dir_all(&items_dir).unwrap();
        std::fs::copy(source_root.join(TEST_OBLIGATION_RULES_PATH), &rules_path).unwrap();
        std::fs::copy(
            source_root.join("track/items").join(TRACK_ID).join("obligations.json"),
            items_dir.join("obligations.json"),
        )
        .unwrap();

        let root =
            TestObligationCompositionRoot::new(workspace_root.to_path_buf(), rules_path.clone());
        let input = TestObligationCheckInput::try_from_raw(
            Some(TRACK_ID.to_owned()),
            "detached-but-explicit-track".to_owned(),
        )
        .unwrap();
        assert_ne!(root.check_handler().handle(input).exit_code, 0);

        std::fs::write(&rules_path, "{}").unwrap();
        let invalid_rules_input = TestObligationCheckInput::try_from_raw(
            Some(TRACK_ID.to_owned()),
            "detached-but-explicit-track".to_owned(),
        )
        .unwrap();
        assert_ne!(root.check_handler().handle(invalid_rules_input).exit_code, 0);
    }

    #[test]
    fn test_derive_handler_repeated_active_branch_invocations_write_identical_bytes() {
        const TRACK_ID: &str = "2026-08-13-test-obligation-method-anchor-ownership";

        let _guard = crate::test_support::process_env_lock().lock().unwrap();
        let workspace = tempfile::tempdir().unwrap();
        crate::test_support::seed_repo(workspace.path(), &format!("track/{TRACK_ID}"));
        crate::test_support::run_in_dir(workspace.path(), || {
            let workspace_root = workspace.path();
            let source_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
            let source_track = source_root.join("track/items").join(TRACK_ID);
            let target_track = workspace_root.join("track/items").join(TRACK_ID);
            let rules_path = workspace_root.join(TEST_OBLIGATION_RULES_PATH);

            std::fs::create_dir_all(rules_path.parent().unwrap()).unwrap();
            std::fs::create_dir_all(&target_track).unwrap();
            std::fs::copy(source_root.join(TEST_OBLIGATION_RULES_PATH), &rules_path).unwrap();
            for artifact in [
                "spec.json",
                "domain-types.json",
                "usecase-types.json",
                "infrastructure-types.json",
                "cli_driver-types.json",
                "cli_composition-types.json",
                "cli-types.json",
                "metadata.json",
                "impl-plan.json",
            ] {
                std::fs::copy(source_track.join(artifact), target_track.join(artifact)).unwrap();
            }

            // Keep this fixture active even after the source track is fully completed.
            let impl_plan_path = target_track.join("impl-plan.json");
            let mut impl_plan: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(&impl_plan_path).unwrap()).unwrap();
            let task = impl_plan
                .get_mut("tasks")
                .and_then(serde_json::Value::as_array_mut)
                .unwrap()
                .iter_mut()
                .find(|task| task.get("id").and_then(serde_json::Value::as_str) == Some("T10"))
                .unwrap();
            let task_fields = task.as_object_mut().unwrap();
            task_fields.insert("status".to_owned(), serde_json::json!("todo"));
            task_fields.remove("commit_hash");
            std::fs::write(&impl_plan_path, serde_json::to_string_pretty(&impl_plan).unwrap())
                .unwrap();

            assert_eq!(
                FsTrackStatusReaderAdapter::new()
                    .read_status(&workspace_root.join("track/items"), TRACK_ID)
                    .unwrap(),
                TrackStatus::InProgress,
                "the repeated-derive fixture must remain deterministically active"
            );

            let root = TestObligationCompositionRoot::new(workspace_root.to_path_buf(), rules_path);
            let input =
                TestObligationDeriveInput::try_from_raw(None, format!("track/{TRACK_ID}")).unwrap();

            assert_eq!(root.derive_handler().handle(input.clone()).exit_code, 0);
            let first = std::fs::read(target_track.join("obligations.json")).unwrap();
            assert_eq!(root.derive_handler().handle(input).exit_code, 0);
            let second = std::fs::read(target_track.join("obligations.json")).unwrap();

            assert_eq!(first, second, "active-track derive must not churn JSON bytes");
        });
    }

    #[test]
    fn test_derive_handler_completed_track_invocation_preserves_existing_artifact_bytes() {
        const COMPLETED_TRACK_ID: &str = "test-obligation-fulfillment-gate-2026-07-07";

        let _guard = crate::test_support::process_env_lock().lock().unwrap();
        let workspace = tempfile::tempdir().unwrap();
        crate::test_support::seed_repo(workspace.path(), &format!("track/{COMPLETED_TRACK_ID}"));
        crate::test_support::run_in_dir(workspace.path(), || {
            let workspace_root = workspace.path();
            let source_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
            let source_completed_track = source_root.join("track/items").join(COMPLETED_TRACK_ID);
            let completed_track = workspace_root.join("track/items").join(COMPLETED_TRACK_ID);
            let obligations_path = completed_track.join("obligations.json");
            let rules_path = workspace_root.join(TEST_OBLIGATION_RULES_PATH);

            std::fs::create_dir_all(rules_path.parent().unwrap()).unwrap();
            std::fs::create_dir_all(&completed_track).unwrap();
            std::fs::copy(source_root.join(TEST_OBLIGATION_RULES_PATH), &rules_path).unwrap();
            for artifact in [
                "spec.json",
                "domain-types.json",
                "usecase-types.json",
                "infrastructure-types.json",
                "cli_driver-types.json",
                "cli_composition-types.json",
                "cli-types.json",
                "metadata.json",
                "impl-plan.json",
                "obligations.json",
            ] {
                std::fs::copy(
                    source_completed_track.join(artifact),
                    completed_track.join(artifact),
                )
                .unwrap();
            }
            assert_eq!(
                FsTrackStatusReaderAdapter::new()
                    .read_status(&workspace_root.join("track/items"), COMPLETED_TRACK_ID)
                    .unwrap(),
                TrackStatus::Done,
                "the preserved artifact must belong to a completed track"
            );
            let expected = std::fs::read(&obligations_path).unwrap();

            let root = TestObligationCompositionRoot::new(workspace_root.to_path_buf(), rules_path);
            let input = TestObligationDeriveInput::try_from_raw(
                Some(COMPLETED_TRACK_ID.to_owned()),
                format!("track/{COMPLETED_TRACK_ID}"),
            )
            .unwrap();

            let first = root.derive_handler().handle(input.clone());
            assert_ne!(first.exit_code, 0, "completed tracks must not be derived");
            let first_bytes = std::fs::read(&obligations_path).unwrap();
            assert_eq!(first_bytes, expected, "completed-track artifacts must remain unchanged");

            let second = root.derive_handler().handle(input);
            assert_ne!(second.exit_code, 0, "completed tracks must remain rejected");
            assert_eq!(
                (first.exit_code, first.stdout, first.stderr),
                (second.exit_code, second.stdout, second.stderr),
                "completed-track rejection must be deterministic"
            );
            assert_eq!(
                std::fs::read(&obligations_path).unwrap(),
                first_bytes,
                "repeated completed-track derives must preserve deterministic artifact bytes"
            );
        });
    }

    #[test]
    fn test_composition_root_results_wires_informational_status_lanes() {
        const TRACK_ID: &str = "d15-task-status-check-gate-2026-07-11";

        let _guard = crate::test_support::process_env_lock().lock().unwrap();
        let workspace = tempfile::tempdir().unwrap();
        crate::test_support::seed_repo(workspace.path(), &format!("track/{TRACK_ID}"));
        crate::test_support::run_in_dir(workspace.path(), || {
            let workspace_root = workspace.path();
            let source_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
            let source_track = source_root.join("track/items").join(TRACK_ID);
            let target_track = workspace_root.join("track/items").join(TRACK_ID);
            let rules_path = workspace_root.join(TEST_OBLIGATION_RULES_PATH);

            std::fs::create_dir_all(rules_path.parent().unwrap()).unwrap();
            std::fs::create_dir_all(&target_track).unwrap();
            std::fs::copy(source_root.join(TEST_OBLIGATION_RULES_PATH), &rules_path).unwrap();
            for artifact in [
                "obligations.json",
                "task-contract.json",
                "impl-plan.json",
                "spec.json",
                "domain-types.json",
                "usecase-types.json",
                "infrastructure-types.json",
                "cli-types.json",
                "cli_driver-types.json",
                "cli_composition-types.json",
            ] {
                std::fs::copy(source_track.join(artifact), target_track.join(artifact)).unwrap();
            }
            std::fs::write(
                target_track.join("test-bindings.json"),
                format!(r#"{{"records":[],"track_id":"{TRACK_ID}"}}"#),
            )
            .unwrap();

            let impl_plan_path = target_track.join("impl-plan.json");
            let mut impl_plan: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(&impl_plan_path).unwrap()).unwrap();
            let tasks =
                impl_plan.get_mut("tasks").and_then(serde_json::Value::as_array_mut).unwrap();
            for task in tasks {
                let task_fields = task.as_object_mut().unwrap();
                match task_fields.get("id").and_then(serde_json::Value::as_str) {
                    Some("T002") => {
                        task_fields.insert("status".to_owned(), serde_json::json!("in_progress"));
                        task_fields.insert("commit_hash".to_owned(), serde_json::Value::Null);
                    }
                    Some("T003") => {
                        task_fields.insert("status".to_owned(), serde_json::json!("todo"));
                        task_fields.insert("commit_hash".to_owned(), serde_json::Value::Null);
                    }
                    _ => {}
                }
            }
            std::fs::write(&impl_plan_path, serde_json::to_string_pretty(&impl_plan).unwrap())
                .unwrap();

            let root = TestObligationCompositionRoot::new(workspace_root.to_path_buf(), rules_path);
            let outcome = root.results_handler().handle(TestObligationResultsInput::new(Some(
                domain::TrackId::try_new(TRACK_ID.to_owned()).unwrap(),
            )));

            assert_eq!(outcome.exit_code, 0);
            let stdout = outcome.stdout.unwrap();
            assert!(stdout.contains("status:todo missing=11 stale=0 verdict_absent=0"));
            assert!(stdout.contains("status:in_progress missing=4 stale=0 verdict_absent=0"));
            assert!(stdout.contains("status:done missing=11 stale=0 verdict_absent=0"));
        });
    }

    #[test]
    fn test_composition_root_results_empty_workspace_has_no_status_lanes() {
        let workspace = tempfile::tempdir().unwrap();
        let root = TestObligationCompositionRoot::new(
            workspace.path().to_path_buf(),
            workspace.path().join(TEST_OBLIGATION_RULES_PATH),
        );
        let input = TestObligationResultsInput::new(Some(
            domain::TrackId::try_new("informational-results".to_owned()).unwrap(),
        ));

        let outcome = root.results_handler().handle(input);

        assert_eq!(outcome.exit_code, 0);
        let stdout = outcome.stdout.unwrap();
        assert!(!stdout.contains("status:"));
        assert!(stdout.ends_with("records=0 uncited_findings=0"));
    }

    #[test]
    fn test_composition_root_wires_status_artifacts_for_check_and_results() {
        const TRACK_ID: &str = "d15-task-status-check-gate-2026-07-11";

        let _guard = crate::test_support::process_env_lock().lock().unwrap();
        let workspace = tempfile::tempdir().unwrap();
        crate::test_support::seed_repo(workspace.path(), &format!("track/{TRACK_ID}"));
        crate::test_support::run_in_dir(workspace.path(), || {
            let workspace_root = workspace.path();
            let source_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
            let source_track = source_root.join("track/items").join(TRACK_ID);
            let target_track = workspace_root.join("track/items").join(TRACK_ID);
            let rules_path = workspace_root.join(TEST_OBLIGATION_RULES_PATH);

            std::fs::create_dir_all(rules_path.parent().unwrap()).unwrap();
            std::fs::create_dir_all(&target_track).unwrap();
            std::fs::copy(source_root.join(TEST_OBLIGATION_RULES_PATH), &rules_path).unwrap();
            for artifact in [
                "obligations.json",
                "task-contract.json",
                "impl-plan.json",
                "spec.json",
                "domain-types.json",
                "usecase-types.json",
                "infrastructure-types.json",
                "cli-types.json",
                "cli_driver-types.json",
                "cli_composition-types.json",
            ] {
                std::fs::copy(source_track.join(artifact), target_track.join(artifact)).unwrap();
            }
            std::fs::write(
                target_track.join("test-bindings.json"),
                format!(r#"{{"records":[],"track_id":"{TRACK_ID}"}}"#),
            )
            .unwrap();
            let impl_plan_path = target_track.join("impl-plan.json");
            let mut skipped_plan: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(&impl_plan_path).unwrap()).unwrap();
            let original_plan = skipped_plan.clone();
            let tasks =
                skipped_plan.get_mut("tasks").and_then(serde_json::Value::as_array_mut).unwrap();
            for task in tasks {
                let task_fields = task.as_object_mut().unwrap();
                if task_fields.get("status").and_then(serde_json::Value::as_str) == Some("done") {
                    task_fields.insert("status".to_owned(), serde_json::json!("skipped"));
                    task_fields.insert("commit_hash".to_owned(), serde_json::Value::Null);
                }
            }
            assert_ne!(skipped_plan, original_plan);
            std::fs::write(&impl_plan_path, serde_json::to_string_pretty(&skipped_plan).unwrap())
                .unwrap();

            let root = TestObligationCompositionRoot::new(workspace_root.to_path_buf(), rules_path);
            let check_input = TestObligationCheckInput::try_from_raw(
                Some(TRACK_ID.to_owned()),
                "detached-but-explicit-track".to_owned(),
            )
            .unwrap();
            let check = root.check_handler().handle(check_input);
            assert_ne!(check.exit_code, 0);

            let results = root.results_handler().handle(TestObligationResultsInput::new(Some(
                domain::TrackId::try_new(TRACK_ID.to_owned()).unwrap(),
            )));
            assert_eq!(results.exit_code, 0);
            let stdout = results.stdout.unwrap();
            assert!(stdout.contains("status:skipped missing="));
            assert!(
                !stdout.contains("status:skipped missing=0 stale=0 verdict_absent=0"),
                "expected unresolved skipped findings, got: {stdout}"
            );
        });
    }

    #[test]
    fn test_composition_root_resolves_shared_entry_to_in_progress_lane() {
        const TRACK_ID: &str = "d15-task-status-check-gate-2026-07-11";

        let _guard = crate::test_support::process_env_lock().lock().unwrap();
        let workspace = tempfile::tempdir().unwrap();
        crate::test_support::seed_repo(workspace.path(), &format!("track/{TRACK_ID}"));
        crate::test_support::run_in_dir(workspace.path(), || {
            let workspace_root = workspace.path();
            let source_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
            let source_track = source_root.join("track/items").join(TRACK_ID);
            let target_track = workspace_root.join("track/items").join(TRACK_ID);
            let rules_path = workspace_root.join(TEST_OBLIGATION_RULES_PATH);

            std::fs::create_dir_all(rules_path.parent().unwrap()).unwrap();
            std::fs::create_dir_all(&target_track).unwrap();
            std::fs::copy(source_root.join(TEST_OBLIGATION_RULES_PATH), &rules_path).unwrap();
            for artifact in [
                "obligations.json",
                "task-contract.json",
                "impl-plan.json",
                "spec.json",
                "domain-types.json",
                "usecase-types.json",
                "infrastructure-types.json",
                "cli-types.json",
                "cli_driver-types.json",
                "cli_composition-types.json",
            ] {
                std::fs::copy(source_track.join(artifact), target_track.join(artifact)).unwrap();
            }
            std::fs::write(
                target_track.join("test-bindings.json"),
                format!(r#"{{"records":[],"track_id":"{TRACK_ID}"}}"#),
            )
            .unwrap();

            let task_contract_path = target_track.join("task-contract.json");
            let mut task_contract: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(&task_contract_path).unwrap())
                    .unwrap();
            let entries = task_contract
                .get_mut("entries")
                .and_then(serde_json::Value::as_object_mut)
                .unwrap();
            entries.get_mut("T003").and_then(serde_json::Value::as_array_mut).unwrap().push(
                serde_json::json!({
                    "layer": "usecase",
                    "entry_key": "CheckTestObligationsInteractor",
                }),
            );
            std::fs::write(
                &task_contract_path,
                serde_json::to_string_pretty(&task_contract).unwrap(),
            )
            .unwrap();

            let impl_plan_path = target_track.join("impl-plan.json");
            let mut impl_plan: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(&impl_plan_path).unwrap()).unwrap();
            let tasks =
                impl_plan.get_mut("tasks").and_then(serde_json::Value::as_array_mut).unwrap();
            for task in tasks {
                let task_fields = task.as_object_mut().unwrap();
                match task_fields.get("id").and_then(serde_json::Value::as_str) {
                    Some("T002") => {
                        task_fields.insert("status".to_owned(), serde_json::json!("in_progress"));
                        task_fields.insert("commit_hash".to_owned(), serde_json::Value::Null);
                    }
                    Some("T003") => {
                        task_fields.insert("status".to_owned(), serde_json::json!("todo"));
                        task_fields.insert("commit_hash".to_owned(), serde_json::Value::Null);
                    }
                    _ => {}
                }
            }
            std::fs::write(&impl_plan_path, serde_json::to_string_pretty(&impl_plan).unwrap())
                .unwrap();

            let root = TestObligationCompositionRoot::new(workspace_root.to_path_buf(), rules_path);
            let check_input = TestObligationCheckInput::try_from_raw(
                Some(TRACK_ID.to_owned()),
                "detached-but-explicit-track".to_owned(),
            )
            .unwrap();
            let check = root.check_handler().handle(check_input);
            assert_ne!(check.exit_code, 0);

            let results = root.results_handler().handle(TestObligationResultsInput::new(Some(
                domain::TrackId::try_new(TRACK_ID.to_owned()).unwrap(),
            )));
            assert_eq!(results.exit_code, 0);
            let stdout = results.stdout.unwrap();
            assert!(stdout.contains("status:in_progress missing="));
            assert!(
                !stdout.contains("status:in_progress missing=0 stale=0 verdict_absent=0"),
                "expected the shared entry to resolve to an unresolved in-progress lane, got: {stdout}"
            );
        });
    }

    #[test]
    fn test_composition_root_fails_closed_with_distinct_verifier_fallbacks() {
        let workspace_root = tempfile::tempdir().unwrap();
        let root = TestObligationCompositionRoot::new(
            workspace_root.path().to_path_buf(),
            workspace_root.path().join(TEST_OBLIGATION_RULES_PATH),
        );

        let fulfillment_error = root
            .fulfillment_verifier()
            .verify_pair("assert!(covered)", "entry", "anchor", ModelTier::Fast)
            .unwrap_err();
        let waiver_error = root
            .waiver_verifier()
            .verify_pair("reason", "entry", "anchor", ModelTier::Fast)
            .unwrap_err();

        assert!(matches!(fulfillment_error, SemanticVerifierError::VerifierPort(_)));
        assert!(matches!(waiver_error, SemanticVerifierError::VerifierPort(_)));
    }

    #[test]
    fn test_composition_root_wires_distinct_fallbacks_when_capabilities_unresolvable() {
        let workspace_root = tempfile::tempdir().unwrap();
        let profiles_path = workspace_root.path().join(AGENT_PROFILES_PATH);
        std::fs::create_dir_all(profiles_path.parent().unwrap()).unwrap();
        std::fs::write(
            &profiles_path,
            r#"{
                "schema_version": 1,
                "providers": { "codex": { "label": "Codex", "supported_reasoning_efforts": ["low", "medium", "high", "xhigh", "max"] } },
                "capabilities": {
                    "unrelated-capability": { "provider": "codex", "model": "unrelated", "execution_mode": "typed-pipeline" }
                }
            }"#,
        )
        .unwrap();
        let root = TestObligationCompositionRoot::new(
            workspace_root.path().to_path_buf(),
            workspace_root.path().join(TEST_OBLIGATION_RULES_PATH),
        );

        let profiles = root.agent_profiles().unwrap();
        assert!(
            profiles
                .resolve_capability(
                    &CapabilityName::try_new("obligation-fulfillment-verifier").unwrap()
                )
                .is_none()
        );
        assert!(
            profiles
                .resolve_capability(&CapabilityName::try_new("waiver-verifier").unwrap())
                .is_none()
        );

        let fulfillment_error = root
            .fulfillment_verifier()
            .verify_pair("assert!(covered)", "entry", "anchor", ModelTier::Fast)
            .unwrap_err();
        let waiver_error = root
            .waiver_verifier()
            .verify_pair("reason", "entry", "anchor", ModelTier::Fast)
            .unwrap_err();

        assert!(matches!(fulfillment_error, SemanticVerifierError::VerifierPort(_)));
        assert!(matches!(waiver_error, SemanticVerifierError::VerifierPort(_)));
    }
}
