//! Driver-level application service for `sotp dry check-approved`.
//!
//! [`DryCheckApprovedDriverInteractor`] performs the entire check-approved
//! orchestration itself (ADR 2026-06-21-1328 D4), reproducing
//! `DryCompositionRoot::dry_check_approved`'s call order and its deliberate
//! error-absorption quirk exactly (CN-07).
//!
//! Ordered after [`crate::dry_driver_shared`] (T024): reuses its
//! `DryRepoRootPort` / `DryBaseBranchPort` / `DryCheckStorageFactoryPort` /
//! `DryDiffBaseFactoryPort`. Also reuses three EXISTING usecase ports/services
//! unmodified: [`crate::fixpoint_resolve::DryCorpusMetaPort`],
//! [`crate::fixpoint_resolve_driver::DryCheckConfigLoaderPort`] (T017), and
//! [`crate::dry_check::fragment_pipeline::DryFragmentPipelineService`].
//!
//! This module is purely additive: nothing outside it references these types
//! yet, so it compiles standalone.
//!
//! Design: ADR 2026-06-21-1328 D2 / D4 / D7, IN-14, AC-20, CN-07.

use std::sync::Arc;

use domain::TrackId;
use domain::dry_check::DryCheckApprovalVerdict;

use crate::dry_check::fragment_pipeline::{DryFragmentPipelineCommand, DryFragmentPipelineService};
use crate::dry_check::{DryCheckApprovalInteractor, DryCheckApprovalService as _};
use crate::dry_driver::{DryCheckApprovedDriverInput, DryCheckApprovedOutcome};
use crate::dry_driver_shared::{
    DryBaseBranchPort, DryCheckStorageFactoryPort, DryDiffBaseFactoryPort, DryRepoRootPort,
};
use crate::fixpoint_resolve::DryCorpusMetaPort;
use crate::fixpoint_resolve_driver::DryCheckConfigLoaderPort;

// ── DryCheckApprovedDriverService ─────────────────────────────────────────────

/// Application service (primary port) for `sotp dry check-approved`.
///
/// Reuses the pre-existing [`DryCheckApprovedDriverInput`]/
/// [`DryCheckApprovedOutcome`] DTOs already produced by IN-13/T021.
pub trait DryCheckApprovedDriverService: Send + Sync {
    /// Run `sotp dry check-approved`.
    fn dry_check_approved(&self, input: DryCheckApprovedDriverInput) -> DryCheckApprovedOutcome;
}

// ── DryCheckApprovedDriverInteractor ──────────────────────────────────────────

/// Interactor implementing [`DryCheckApprovedDriverService`].
///
/// Holds the seven secondary ports below as private `Arc<dyn Port>` fields
/// and performs the entire check-approved orchestration itself. Carries no
/// clock or telemetry dependency (usecase-purity forbids sampling
/// `std::time::Instant` or invoking a telemetry port from inside a usecase
/// entrypoint); wall-clock timing measurement and `GateEval` telemetry
/// emission for the call to this service live in the bin layer
/// (`apps/cli/src/commands/dry.rs::execute_dry_check_approved`).
pub struct DryCheckApprovedDriverInteractor {
    repo_root: Arc<dyn DryRepoRootPort>,
    base_branch: Arc<dyn DryBaseBranchPort>,
    diff_base_resolver_factory: Arc<dyn DryDiffBaseFactoryPort>,
    config_loader: Arc<dyn DryCheckConfigLoaderPort>,
    corpus_meta: Arc<dyn DryCorpusMetaPort>,
    fragment_pipeline: Arc<dyn DryFragmentPipelineService>,
    storage_factory: Arc<dyn DryCheckStorageFactoryPort>,
}

impl DryCheckApprovedDriverInteractor {
    /// Construct a new [`DryCheckApprovedDriverInteractor`] with all required ports.
    #[must_use]
    pub fn new(
        repo_root: Arc<dyn DryRepoRootPort>,
        base_branch: Arc<dyn DryBaseBranchPort>,
        diff_base_resolver_factory: Arc<dyn DryDiffBaseFactoryPort>,
        config_loader: Arc<dyn DryCheckConfigLoaderPort>,
        corpus_meta: Arc<dyn DryCorpusMetaPort>,
        fragment_pipeline: Arc<dyn DryFragmentPipelineService>,
        storage_factory: Arc<dyn DryCheckStorageFactoryPort>,
    ) -> Self {
        Self {
            repo_root,
            base_branch,
            diff_base_resolver_factory,
            config_loader,
            corpus_meta,
            fragment_pipeline,
            storage_factory,
        }
    }
}

impl DryCheckApprovedDriverService for DryCheckApprovedDriverInteractor {
    fn dry_check_approved(&self, input: DryCheckApprovedDriverInput) -> DryCheckApprovedOutcome {
        // ── (1) Validate track_id. ───────────────────────────────────────────────
        let track_id = match TrackId::try_new(input.track_id) {
            Ok(id) => id,
            Err(e) => return DryCheckApprovedOutcome::Failure { message: e.to_string() },
        };

        // ── (2) Resolve workspace paths (items_dir containment before config). ──
        let workspace = match self.repo_root.resolve(&input.items_dir) {
            Ok(w) => w,
            Err(e) => return DryCheckApprovedOutcome::Failure { message: e.to_string() },
        };

        // ── (3) Load config. ──────────────────────────────────────────────────────
        let (dry_config, config_fingerprint) = match self.config_loader.load(&workspace.repo_root) {
            Ok(v) => v,
            Err(e) => return DryCheckApprovedOutcome::Failure { message: e.to_string() },
        };

        // ── (4) D2/IN-05/CN-06 service-boundary safety net. ─────────────────────
        if !dry_config.enabled {
            return DryCheckApprovedOutcome::Approved;
        }

        // ── (5) Derive track_dir. ───────────────────────────────────────────────
        let track_dir = workspace.canonical_items_dir.join(track_id.as_ref());

        // ── (6) Resolve the diff base commit (override or store fallback). ─────
        let base = match input.base_commit {
            Some(ref s) => match domain::CommitHash::try_new(s) {
                Ok(hash) => hash,
                Err(e) => {
                    return DryCheckApprovedOutcome::Failure {
                        message: format!("invalid --base-commit: {e}"),
                    };
                }
            },
            None => {
                let base_branch = match self.base_branch.resolve_base_branch(
                    &workspace.canonical_root,
                    &track_dir,
                    &track_id,
                ) {
                    Ok(b) => b,
                    Err(e) => return DryCheckApprovedOutcome::Failure { message: e.to_string() },
                };
                let resolver = self.diff_base_resolver_factory.build(&base_branch);
                match resolver.resolve_diff_base(
                    &track_dir,
                    &workspace.canonical_root,
                    &workspace.repo_root,
                ) {
                    Ok(hash) => hash,
                    Err(e) => return DryCheckApprovedOutcome::Failure { message: e.to_string() },
                }
            }
        };

        // ── (7) Resolve corpus workspace root + fingerprint (error absorbed). ──
        // Diagnostic logging for this fallback belongs in infrastructure/CLI
        // (usecase-purity rule forbids output macros here) — the corpus_meta
        // port's own adapter (`FsDryCorpusMetaAdapter`) already emits an
        // equivalent `[warn]` message before returning its own fallback, so
        // this absorption path is effectively unreachable via that adapter;
        // it remains here only to satisfy `DryCorpusMetaPort`'s fallible
        // signature for any future adapter that does return `Err`.
        let (approval_workspace_root, current_corpus_fingerprint) = match self
            .corpus_meta
            .resolve_corpus_meta(&track_dir, &workspace.canonical_root, &workspace.repo_root)
        {
            Ok(v) => v,
            Err(_) => (
                workspace.canonical_root.clone(),
                domain::dry_check::DryCheckCorpusFingerprint::fail_closed(),
            ),
        };

        // ── (8) Build diff fragments and derive current_fragment_refs. ─────────
        let pipeline_cmd = DryFragmentPipelineCommand {
            canonical_root: approval_workspace_root,
            repo_root: workspace.repo_root.clone(),
            base,
        };
        let pipeline_output = match self.fragment_pipeline.derive_current_refs(pipeline_cmd) {
            Ok(v) => v,
            Err(e) => return DryCheckApprovedOutcome::Failure { message: e.to_string() },
        };
        let current_fragment_refs = pipeline_output.fragment_refs;

        // ── (9) Build the reader/coverage pair (.writer unused). ───────────────
        let storage = self.storage_factory.build(&track_dir, &workspace.canonical_root);

        // ── (10) Invoke the domain-level check directly. ─────────────────────────
        let interactor = DryCheckApprovalInteractor::new(
            dry_config,
            storage.reader,
            storage.coverage,
            config_fingerprint,
            current_corpus_fingerprint,
        );
        let verdict = match interactor.check_approved(&track_id, &current_fragment_refs) {
            Ok(v) => v,
            Err(e) => return DryCheckApprovedOutcome::Failure { message: e.to_string() },
        };

        // ── (11) Map the verdict into the outcome. ────────────────────────────────
        match verdict {
            DryCheckApprovalVerdict::Approved => DryCheckApprovedOutcome::Approved,
            DryCheckApprovalVerdict::Blocked { unresolved_pair_count } => {
                DryCheckApprovedOutcome::Blocked { unresolved_pair_count }
            }
        }
    }
}

// ── FeatureDisabledDryGateInteractor ─────────────────────────────────────────

/// Lightweight `dry check-approved` service for binaries without `semantic-dup`.
///
/// It resolves the repository and reads the DRY configuration before deciding
/// whether the omitted semantic-dup implementation is needed. A disabled gate
/// therefore succeeds without constructing or invoking any semantic-dup
/// component; an enabled gate fails closed with an actionable feature message.
pub struct FeatureDisabledDryGateInteractor {
    repo_root: Arc<dyn DryRepoRootPort>,
    config_loader: Arc<dyn DryCheckConfigLoaderPort>,
}

impl FeatureDisabledDryGateInteractor {
    /// Construct the lightweight feature-disabled gate interactor.
    #[must_use]
    pub fn new(
        repo_root: Arc<dyn DryRepoRootPort>,
        config_loader: Arc<dyn DryCheckConfigLoaderPort>,
    ) -> Self {
        Self { repo_root, config_loader }
    }
}

impl DryCheckApprovedDriverService for FeatureDisabledDryGateInteractor {
    fn dry_check_approved(&self, input: DryCheckApprovedDriverInput) -> DryCheckApprovedOutcome {
        let _track_id = match TrackId::try_new(input.track_id) {
            Ok(track_id) => track_id,
            Err(error) => return DryCheckApprovedOutcome::Failure { message: error.to_string() },
        };

        let workspace = match self.repo_root.resolve(&input.items_dir) {
            Ok(workspace) => workspace,
            Err(error) => return DryCheckApprovedOutcome::Failure { message: error.to_string() },
        };
        let (config, _) = match self.config_loader.load(&workspace.repo_root) {
            Ok(config) => config,
            Err(error) => return DryCheckApprovedOutcome::Failure { message: error.to_string() },
        };

        if !config.enabled {
            return DryCheckApprovedOutcome::Approved;
        }

        DryCheckApprovedOutcome::Failure {
            message: "dry check-approved requires the disabled `semantic-dup` feature while the DRY gate is enabled; rebuild sotp with `--features semantic-dup`".to_owned(),
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use std::collections::BTreeSet;
    use std::path::{Path, PathBuf};

    use domain::CommitHash;
    use domain::dry_check::{DryCheckConfigFingerprint, DryCheckCoverageRecord, FragmentRef};

    use super::*;
    use crate::d4_orchestration::D4OrchestrationError;
    use crate::dry_check::fragment_pipeline::DryFragmentPipelineOutput;
    use crate::dry_check::{
        DryCheckConfig, DryCheckCycleError, DryCheckParallelism, DryCheckPercent,
    };
    use crate::dry_driver_shared::{
        DryBaseBranchError, DryCheckStorageHandle, DryRepoWorkspace, DryRepoWorkspaceError,
        GitDiscoveryFailureDetail, IoFailureDetail,
    };
    use crate::fixpoint_resolve::{
        DiffBaseResolverError, DiffBaseResolverPort, DryCorpusMetaError,
    };
    use crate::fixpoint_resolve_driver::{
        DryCheckConfigLoadFailureDetail, DryCheckConfigLoaderError,
    };

    // ── Test doubles ─────────────────────────────────────────────────────────

    struct StubRepoRoot {
        result: Result<DryRepoWorkspace, DryRepoWorkspaceError>,
    }

    impl DryRepoRootPort for StubRepoRoot {
        fn resolve(&self, _items_dir: &Path) -> Result<DryRepoWorkspace, DryRepoWorkspaceError> {
            self.result.clone()
        }
    }

    struct StubBaseBranch {
        result: Result<String, DryBaseBranchError>,
    }

    impl DryBaseBranchPort for StubBaseBranch {
        fn resolve_base_branch(
            &self,
            _canonical_root: &Path,
            _track_dir: &Path,
            _track_id: &TrackId,
        ) -> Result<String, DryBaseBranchError> {
            self.result.clone()
        }
    }

    struct StubDiffBaseResolver {
        result: Result<CommitHash, DiffBaseResolverError>,
    }

    impl DiffBaseResolverPort for StubDiffBaseResolver {
        fn resolve_diff_base(
            &self,
            _track_dir: &Path,
            _canonical_root: &Path,
            _repo_root: &Path,
        ) -> Result<CommitHash, DiffBaseResolverError> {
            self.result.clone()
        }
    }

    struct StubDiffBaseFactory {
        result: Result<CommitHash, DiffBaseResolverError>,
    }

    impl DryDiffBaseFactoryPort for StubDiffBaseFactory {
        fn build(&self, _base_branch: &str) -> Arc<dyn DiffBaseResolverPort> {
            Arc::new(StubDiffBaseResolver { result: self.result.clone() })
        }
    }

    struct StubConfigLoader {
        result: Result<(DryCheckConfig, DryCheckConfigFingerprint), DryCheckConfigLoaderError>,
    }

    impl DryCheckConfigLoaderPort for StubConfigLoader {
        fn load(
            &self,
            _repo_root: &Path,
        ) -> Result<(DryCheckConfig, DryCheckConfigFingerprint), DryCheckConfigLoaderError>
        {
            self.result.clone()
        }
    }

    struct StubCorpusMeta {
        result: Result<(PathBuf, domain::dry_check::DryCheckCorpusFingerprint), DryCorpusMetaError>,
    }

    impl DryCorpusMetaPort for StubCorpusMeta {
        fn resolve_corpus_meta(
            &self,
            _track_dir: &Path,
            _canonical_root: &Path,
            _repo_root: &Path,
        ) -> Result<(PathBuf, domain::dry_check::DryCheckCorpusFingerprint), DryCorpusMetaError>
        {
            self.result.clone()
        }
    }

    struct StubFragmentPipeline {
        result: Result<DryFragmentPipelineOutput, D4OrchestrationError>,
    }

    impl DryFragmentPipelineService for StubFragmentPipeline {
        fn derive_current_refs(
            &self,
            _cmd: DryFragmentPipelineCommand,
        ) -> Result<DryFragmentPipelineOutput, D4OrchestrationError> {
            self.result.clone()
        }
    }

    struct StubReader;

    impl domain::dry_check::DryCheckReader for StubReader {
        fn read_records(
            &self,
        ) -> Result<Vec<domain::dry_check::DryCheckRecord>, domain::dry_check::DryCheckReaderError>
        {
            Ok(vec![])
        }
    }

    struct NoopWriter;

    impl domain::dry_check::DryCheckWriter for NoopWriter {
        fn append_record(
            &self,
            _entry: &domain::dry_check::DryCheckEntry,
        ) -> Result<(), domain::dry_check::DryCheckWriterError> {
            Ok(())
        }
    }

    struct StubCoverage {
        record: Option<DryCheckCoverageRecord>,
    }

    impl crate::dry_check::DryCheckCoveragePort for StubCoverage {
        fn read_coverage(
            &self,
            _track_id: &TrackId,
        ) -> Result<Option<DryCheckCoverageRecord>, DryCheckCycleError> {
            Ok(self.record.clone())
        }

        fn write_coverage(
            &self,
            _track_id: &TrackId,
            _record: DryCheckCoverageRecord,
        ) -> Result<(), DryCheckCycleError> {
            Ok(())
        }
    }

    struct StubStorageFactory {
        coverage_record: Option<DryCheckCoverageRecord>,
    }

    impl DryCheckStorageFactoryPort for StubStorageFactory {
        fn build(&self, _track_dir: &Path, _canonical_root: &Path) -> DryCheckStorageHandle {
            DryCheckStorageHandle {
                reader: Arc::new(StubReader),
                writer: Arc::new(NoopWriter),
                coverage: Arc::new(StubCoverage { record: self.coverage_record.clone() }),
            }
        }
    }

    // ── Helpers ──────────────────────────────────────────────────────────────

    fn test_dry_config(enabled: bool) -> DryCheckConfig {
        DryCheckConfig::new(
            DryCheckPercent::try_new(10).unwrap(),
            DryCheckPercent::try_new(90).unwrap(),
            DryCheckParallelism::try_new(4).unwrap(),
            enabled,
        )
    }

    fn make_config_fingerprint() -> DryCheckConfigFingerprint {
        DryCheckConfigFingerprint::new("c".repeat(64)).unwrap()
    }

    fn make_workspace() -> DryRepoWorkspace {
        DryRepoWorkspace {
            repo_root: PathBuf::from("/repo"),
            canonical_root: PathBuf::from("/repo"),
            canonical_items_dir: PathBuf::from("/repo/track/items"),
        }
    }

    fn make_commit_hash() -> CommitHash {
        CommitHash::try_new("a".repeat(40)).unwrap()
    }

    fn make_test_track_id() -> TrackId {
        TrackId::try_new("test-track-2026").unwrap()
    }

    fn make_input() -> DryCheckApprovedDriverInput {
        DryCheckApprovedDriverInput {
            track_id: "test-track-2026".to_owned(),
            base_commit: None,
            items_dir: PathBuf::from("track/items"),
        }
    }

    fn make_interactor(
        repo_root: Result<DryRepoWorkspace, DryRepoWorkspaceError>,
        config: Result<(DryCheckConfig, DryCheckConfigFingerprint), DryCheckConfigLoaderError>,
        base_branch: Result<String, DryBaseBranchError>,
        diff_base: Result<CommitHash, DiffBaseResolverError>,
        corpus_meta: Result<
            (PathBuf, domain::dry_check::DryCheckCorpusFingerprint),
            DryCorpusMetaError,
        >,
        pipeline: Result<DryFragmentPipelineOutput, D4OrchestrationError>,
        coverage_record: Option<DryCheckCoverageRecord>,
    ) -> DryCheckApprovedDriverInteractor {
        DryCheckApprovedDriverInteractor::new(
            Arc::new(StubRepoRoot { result: repo_root }),
            Arc::new(StubBaseBranch { result: base_branch }),
            Arc::new(StubDiffBaseFactory { result: diff_base }),
            Arc::new(StubConfigLoader { result: config }),
            Arc::new(StubCorpusMeta { result: corpus_meta }),
            Arc::new(StubFragmentPipeline { result: pipeline }),
            Arc::new(StubStorageFactory { coverage_record }),
        )
    }

    fn make_feature_disabled_interactor(enabled: bool) -> FeatureDisabledDryGateInteractor {
        FeatureDisabledDryGateInteractor::new(
            Arc::new(StubRepoRoot { result: Ok(make_workspace()) }),
            Arc::new(StubConfigLoader {
                result: Ok((test_dry_config(enabled), make_config_fingerprint())),
            }),
        )
    }

    fn empty_pipeline_output() -> DryFragmentPipelineOutput {
        DryFragmentPipelineOutput {
            fragment_refs: BTreeSet::new(),
            records_before: 0,
            records_after: 0,
            records_appended: 0,
        }
    }

    // ── enabled=false early-return path ──────────────────────────────────────

    #[test]
    fn dry_check_approved_disabled_config_returns_approved() {
        let interactor = make_interactor(
            Ok(make_workspace()),
            Ok((test_dry_config(false), make_config_fingerprint())),
            Err(DryBaseBranchError::MetadataNotFound { track_id: make_test_track_id() }),
            Err(DiffBaseResolverError::Unavailable("must not be called".to_owned())),
            Err(DryCorpusMetaError::Unavailable("must not be called".to_owned())),
            Err(D4OrchestrationError::DiffFragment("must not be called".to_owned())),
            None,
        );
        let outcome = interactor.dry_check_approved(make_input());
        assert!(
            matches!(outcome, DryCheckApprovedOutcome::Approved),
            "disabled config must short-circuit to Approved, got {outcome:?}"
        );
    }

    #[test]
    fn feature_disabled_gate_disabled_config_returns_approved_without_semantic_work() {
        let outcome = make_feature_disabled_interactor(false).dry_check_approved(make_input());

        assert!(matches!(outcome, DryCheckApprovedOutcome::Approved));
    }

    #[test]
    fn feature_disabled_gate_enabled_config_returns_actionable_failure() {
        let outcome = make_feature_disabled_interactor(true).dry_check_approved(make_input());

        match outcome {
            DryCheckApprovedOutcome::Failure { message } => {
                assert!(message.contains("semantic-dup"));
                assert!(message.contains("enabled"));
            }
            other => panic!("enabled feature-off gate must fail closed, got {other:?}"),
        }
    }

    #[test]
    fn feature_disabled_gate_invalid_track_id_returns_failure() {
        let mut input = make_input();
        input.track_id = String::new();

        let outcome = make_feature_disabled_interactor(false).dry_check_approved(input);

        assert!(matches!(outcome, DryCheckApprovedOutcome::Failure { .. }));
    }

    // ── corpus_meta error absorbed into fallback ────────────────────────────

    #[test]
    fn dry_check_approved_corpus_meta_error_is_absorbed_not_propagated() {
        let interactor = make_interactor(
            Ok(make_workspace()),
            Ok((test_dry_config(true), make_config_fingerprint())),
            Ok("main".to_owned()),
            Ok(make_commit_hash()),
            Err(DryCorpusMetaError::Unavailable("corpus meta boom".to_owned())),
            Ok(empty_pipeline_output()),
            Some(DryCheckCoverageRecord::new(
                BTreeSet::<FragmentRef>::new(),
                BTreeSet::new(),
                make_config_fingerprint(),
                domain::dry_check::DryCheckCorpusFingerprint::fail_closed(),
            )),
        );
        let outcome = interactor.dry_check_approved(make_input());
        assert!(
            !matches!(outcome, DryCheckApprovedOutcome::Failure { .. }),
            "corpus_meta error must be absorbed (warn + fallback), not propagated; got {outcome:?}"
        );
    }

    // ── Approved / Blocked success paths ─────────────────────────────────────

    #[test]
    fn dry_check_approved_missing_coverage_returns_blocked() {
        let interactor = make_interactor(
            Ok(make_workspace()),
            Ok((test_dry_config(true), make_config_fingerprint())),
            Ok("main".to_owned()),
            Ok(make_commit_hash()),
            Ok((
                PathBuf::from("/repo"),
                domain::dry_check::DryCheckCorpusFingerprint::fail_closed(),
            )),
            Ok(empty_pipeline_output()),
            None,
        );
        let outcome = interactor.dry_check_approved(make_input());
        match outcome {
            DryCheckApprovedOutcome::Blocked { unresolved_pair_count } => {
                assert_eq!(unresolved_pair_count, 1, "no coverage record must fail closed");
            }
            other => panic!("expected Blocked, got {other:?}"),
        }
    }

    // ── --base-commit override skips base-branch and diff-base resolution ──────

    #[test]
    fn dry_check_approved_base_commit_override_skips_diff_base_resolver_call() {
        struct PanicIfCalledDiffBaseFactory;
        impl DryDiffBaseFactoryPort for PanicIfCalledDiffBaseFactory {
            fn build(&self, _base_branch: &str) -> Arc<dyn DiffBaseResolverPort> {
                panic!(
                    "diff_base_resolver_factory must not be called when --base-commit is provided"
                )
            }
        }

        let interactor = DryCheckApprovedDriverInteractor::new(
            Arc::new(StubRepoRoot { result: Ok(make_workspace()) }),
            Arc::new(StubBaseBranch {
                result: Err(DryBaseBranchError::MetadataNotFound {
                    track_id: make_test_track_id(),
                }),
            }),
            Arc::new(PanicIfCalledDiffBaseFactory),
            Arc::new(StubConfigLoader {
                result: Ok((test_dry_config(true), make_config_fingerprint())),
            }),
            Arc::new(StubCorpusMeta {
                result: Ok((
                    PathBuf::from("/repo"),
                    domain::dry_check::DryCheckCorpusFingerprint::fail_closed(),
                )),
            }),
            Arc::new(StubFragmentPipeline { result: Ok(empty_pipeline_output()) }),
            Arc::new(StubStorageFactory { coverage_record: None }),
        );
        let mut input = make_input();
        input.base_commit = Some("b".repeat(40));

        let outcome = interactor.dry_check_approved(input);

        assert!(
            !matches!(outcome, DryCheckApprovedOutcome::Failure { .. }),
            "expected non-Failure (no panic from diff_base_resolver_factory), got {outcome:?}"
        );
    }

    // ── Per-port failure propagation ─────────────────────────────────────────

    #[test]
    fn dry_check_approved_invalid_track_id_returns_failure() {
        let interactor = make_interactor(
            Ok(make_workspace()),
            Ok((test_dry_config(true), make_config_fingerprint())),
            Ok("main".to_owned()),
            Ok(make_commit_hash()),
            Ok((
                PathBuf::from("/repo"),
                domain::dry_check::DryCheckCorpusFingerprint::fail_closed(),
            )),
            Ok(empty_pipeline_output()),
            None,
        );
        let mut input = make_input();
        input.track_id = String::new();
        let outcome = interactor.dry_check_approved(input);
        assert!(matches!(outcome, DryCheckApprovedOutcome::Failure { .. }));
    }

    #[test]
    fn dry_check_approved_repo_root_failure_returns_failure() {
        let interactor = make_interactor(
            Err(DryRepoWorkspaceError::GitDiscoveryFailed {
                detail: GitDiscoveryFailureDetail::new("repo boom"),
            }),
            Ok((test_dry_config(true), make_config_fingerprint())),
            Ok("main".to_owned()),
            Ok(make_commit_hash()),
            Ok((
                PathBuf::from("/repo"),
                domain::dry_check::DryCheckCorpusFingerprint::fail_closed(),
            )),
            Ok(empty_pipeline_output()),
            None,
        );
        let outcome = interactor.dry_check_approved(make_input());
        match outcome {
            DryCheckApprovedOutcome::Failure { message } => assert!(message.contains("repo boom")),
            other => panic!("expected Failure, got {other:?}"),
        }
    }

    #[test]
    fn dry_check_approved_config_loader_failure_returns_failure() {
        let interactor = make_interactor(
            Ok(make_workspace()),
            Err(DryCheckConfigLoaderError::ConfigLoadFailed {
                config_path: PathBuf::from(".harness/config/dry-check.json"),
                detail: DryCheckConfigLoadFailureDetail::new("config boom"),
            }),
            Ok("main".to_owned()),
            Ok(make_commit_hash()),
            Ok((
                PathBuf::from("/repo"),
                domain::dry_check::DryCheckCorpusFingerprint::fail_closed(),
            )),
            Ok(empty_pipeline_output()),
            None,
        );
        let outcome = interactor.dry_check_approved(make_input());
        match outcome {
            DryCheckApprovedOutcome::Failure { message } => {
                assert!(message.contains("config boom"))
            }
            other => panic!("expected Failure, got {other:?}"),
        }
    }

    #[test]
    fn dry_check_approved_base_branch_failure_returns_failure() {
        let interactor = make_interactor(
            Ok(make_workspace()),
            Ok((test_dry_config(true), make_config_fingerprint())),
            Err(DryBaseBranchError::MetadataReadFailed {
                track_id: make_test_track_id(),
                detail: IoFailureDetail::new("base branch boom"),
            }),
            Ok(make_commit_hash()),
            Ok((
                PathBuf::from("/repo"),
                domain::dry_check::DryCheckCorpusFingerprint::fail_closed(),
            )),
            Ok(empty_pipeline_output()),
            None,
        );
        let outcome = interactor.dry_check_approved(make_input());
        match outcome {
            DryCheckApprovedOutcome::Failure { message } => {
                assert!(message.contains("base branch boom"));
            }
            other => panic!("expected Failure, got {other:?}"),
        }
    }

    #[test]
    fn dry_check_approved_diff_base_resolver_failure_returns_failure() {
        let interactor = make_interactor(
            Ok(make_workspace()),
            Ok((test_dry_config(true), make_config_fingerprint())),
            Ok("main".to_owned()),
            Err(DiffBaseResolverError::Unavailable("diff base boom".to_owned())),
            Ok((
                PathBuf::from("/repo"),
                domain::dry_check::DryCheckCorpusFingerprint::fail_closed(),
            )),
            Ok(empty_pipeline_output()),
            None,
        );
        let outcome = interactor.dry_check_approved(make_input());
        match outcome {
            DryCheckApprovedOutcome::Failure { message } => {
                assert!(message.contains("diff base boom"))
            }
            other => panic!("expected Failure, got {other:?}"),
        }
    }

    #[test]
    fn dry_check_approved_fragment_pipeline_failure_returns_failure() {
        let interactor = make_interactor(
            Ok(make_workspace()),
            Ok((test_dry_config(true), make_config_fingerprint())),
            Ok("main".to_owned()),
            Ok(make_commit_hash()),
            Ok((
                PathBuf::from("/repo"),
                domain::dry_check::DryCheckCorpusFingerprint::fail_closed(),
            )),
            Err(D4OrchestrationError::DiffFragment("pipeline boom".to_owned())),
            None,
        );
        let outcome = interactor.dry_check_approved(make_input());
        match outcome {
            DryCheckApprovedOutcome::Failure { message } => {
                assert!(message.contains("pipeline boom"))
            }
            other => panic!("expected Failure, got {other:?}"),
        }
    }
}
