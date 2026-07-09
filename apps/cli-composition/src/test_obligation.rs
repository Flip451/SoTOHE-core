//! Composition root for `sotp test-obligation`.

use std::num::NonZeroU8;
use std::path::PathBuf;
use std::sync::Arc;

use domain::tddd::catalogue_v2::catalogue_impl_signals_ports::CatalogueDocumentLoaderPort;
use domain::tddd::test_obligation::errors::SemanticVerifierError;
use domain::tddd::test_obligation::pair::{ObligationFulfillmentPair, WaiverPair};
use domain::tddd::test_obligation::ports::{ObligationFulfillmentVerifierPort, WaiverVerifierPort};
use domain::tddd::test_obligation::projection::RoleObligationItemsProjector;
use domain::tddd::test_obligation::verdict::{
    ObligationFulfillmentCacheKey, ObligationFulfillmentVerdict, WaiverCacheKey, WaiverVerdict,
};
use infrastructure::agent_profiles::{AGENT_PROFILES_PATH, AgentProfiles};
use infrastructure::spec::FsSpecDocumentLoader;
use infrastructure::tddd::tddd_catalogue_document_loader::FsCatalogueDocumentLoader;
use infrastructure::test_obligation::bindings_codec::JsonTestBindingsCodec;
use infrastructure::test_obligation::fulfillment_cache_codec::JsonObligationFulfillmentCacheCodec;
use infrastructure::test_obligation::fulfillment_escalation_driver::ObligationFulfillmentEscalationDriver;
use infrastructure::test_obligation::fulfillment_verifier::{
    FailingObligationFulfillmentVerifier, ObligationFulfillmentVerifierAdapter,
};
use infrastructure::test_obligation::obligations_codec::JsonObligationsCodec;
use infrastructure::test_obligation::rules_codec::JsonTestObligationRulesLoader;
use infrastructure::test_obligation::sha256_content_hasher::Sha256ContentHasher;
use infrastructure::test_obligation::source_scanner::SynTestSourceScanner;
use infrastructure::test_obligation::waiver_cache_codec::JsonWaiverCacheCodec;
use infrastructure::test_obligation::waiver_escalation_driver::WaiverEscalationDriver;
use infrastructure::test_obligation::waiver_verifier::{
    FailingWaiverVerifier, WaiverVerifierAdapter,
};
use usecase::semantic_verdict_core::driver::SemanticEscalationDriverPort;
use usecase::semantic_verdict_core::probe::SemanticCalibrationProbeConfig;
use usecase::test_obligation::bindings_skeleton::TestBindingsSkeletonInteractor;
use usecase::test_obligation::check::CheckTestObligationsInteractor;
use usecase::test_obligation::derive::DeriveTestObligationsInteractor;
use usecase::test_obligation::evaluate::{
    EvaluateTestObligationsInteractor, TestObligationEvaluateConfig,
};
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
        let service = Arc::new(CheckTestObligationsInteractor::new(
            self.rules_loader(),
            self.obligations_codec(),
            self.bindings_codec(),
            self.source_scanner(),
            self.fulfillment_cache(),
            self.waiver_cache(),
            self.spec_loader(),
            self.catalogue_loader(),
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
        let service = Arc::new(TestObligationResultsInteractor::new(
            self.obligations_codec(),
            self.bindings_codec(),
            self.fulfillment_cache(),
            self.waiver_cache(),
        ));
        cli_driver::test_obligation::results::TestObligationResultsHandler::new(service)
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

    fn fulfillment_cache(
        &self,
    ) -> Arc<dyn domain::tddd::test_obligation::ports::ObligationFulfillmentCachePort + Send + Sync>
    {
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

    fn catalogue_loader(&self) -> Arc<dyn CatalogueDocumentLoaderPort + Send + Sync> {
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
            Ok(profiles) => Arc::new(ObligationFulfillmentVerifierAdapter::new(profiles)),
            Err(message) => Arc::new(FailingObligationFulfillmentVerifier::from_message(&message)),
        }
    }

    fn waiver_verifier(&self) -> Arc<dyn WaiverVerifierPort + Send + Sync> {
        match self.agent_profiles() {
            Ok(profiles) => Arc::new(WaiverVerifierAdapter::new(profiles)),
            Err(message) => Arc::new(FailingWaiverVerifier::from_message(&message)),
        }
    }

    fn agent_profiles(&self) -> Result<AgentProfiles, String> {
        let path = self.workspace_root.join(AGENT_PROFILES_PATH);
        AgentProfiles::load(&path).map_err(|e| format!("cannot load agent-profiles.json: {e}"))
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
}
