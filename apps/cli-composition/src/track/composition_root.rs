//! Per-context composition root for the `track` command family.
//!
//! `TrackCompositionRoot` replaces the `CliApp` god-facade for all `track`
//! subcommands.  The struct is a unit struct because no adapter dependencies
//! are injected at construction time — each method constructs its own adapters
//! inline from the arguments it receives (hexagonal composition pattern).
//!
//! `CliApp` keeps backward-compatible shim methods in `track/shim.rs` that
//! construct a `TrackCompositionRoot` and delegate, so all existing call-sites
//! in `apps/cli` continue to compile without change.

/// Composition root for the `track` command family.
///
/// This is a unit struct: no adapter dependencies are injected at construction
/// time.  All port adapters are wired inside individual methods from the
/// runtime arguments they receive (in-method composition).
pub struct TrackCompositionRoot;

impl TrackCompositionRoot {
    /// Create a new `TrackCompositionRoot`.
    pub fn new() -> Self {
        Self
    }
}

impl Default for TrackCompositionRoot {
    fn default() -> Self {
        Self::new()
    }
}

impl TrackCompositionRoot {
    /// Build a wired [`cli_driver::track::TrackDriver`] for the track family.
    ///
    /// Only constructs and injects the fixpoint-resolve adapter chain — never
    /// calls `FixpointResolveDriverService::fixpoint_resolve` itself (ADR
    /// 2026-06-21-1328 D2: composition root is wire-only).
    pub fn track_driver(&self) -> cli_driver::track::TrackDriver {
        build_track_driver()
    }

    /// Build the focused TDDD baseline-capture driver.
    pub fn track_tddd_driver(&self) -> cli_driver::track_tddd::TrackTdddDriver {
        build_track_tddd_driver()
    }

    /// Build the compatibility resolution driver used by foreign command
    /// contexts while the legacy resolution wrappers remain in place.
    pub fn track_resolution_driver(&self) -> cli_driver::track_resolution::TrackResolutionDriver {
        use std::sync::Arc;

        let adapter = Arc::new(
            infrastructure::track_lifecycle::resolution_compat::SystemTrackResolutionAdapter,
        );
        let service = Arc::new(
            usecase::track_lifecycle::resolution_compat::TrackResolutionInteractor::new(adapter),
        );
        cli_driver::track_resolution::TrackResolutionDriver::new(service)
    }
}

pub(crate) fn build_track_driver() -> cli_driver::track::TrackDriver {
    use std::sync::Arc;

    use super::service_impl::TrackServiceImpl;

    let track_init_service =
        Arc::new(usecase::track_lifecycle::track_init::TrackInitInteractor::new(
            Arc::new(infrastructure::track::FsTrackMetadataAdapter::new()),
            Arc::new(infrastructure::track::FsTrackBranchStrategyAdapter),
            Arc::new(infrastructure::track::FsTrackViewsAdapter::new()),
        ));
    let track_archive_service =
        Arc::new(usecase::track_lifecycle::track_archive::TrackArchiveInteractor::new(
            Arc::new(infrastructure::FsGitWorkflowAdapter::new()),
            Arc::new(infrastructure::git_cli::workflow_adapter::FsWorkspaceAdapter::new()),
        ));
    let track_branch_create_service =
        Arc::new(usecase::track_lifecycle::track_branch_create::TrackBranchCreateInteractor::new(
            Arc::new(infrastructure::FsGitWorkflowAdapter::new()),
            Arc::new(infrastructure::git_cli::workflow_adapter::FsWorkspaceAdapter::new()),
            Arc::new(infrastructure::track::FsTrackBranchStrategyAdapter),
        ));
    let track_branch_switch_service =
        Arc::new(usecase::track_lifecycle::track_branch_switch::TrackBranchSwitchInteractor::new(
            Arc::new(infrastructure::FsGitWorkflowAdapter::new()),
        ));
    let track_add_task_service =
        Arc::new(usecase::track_lifecycle::track_add_task::TrackAddTaskInteractor::new(
            Arc::new(RequestScopedTrackTaskAddAdapter),
            Arc::new(infrastructure::track::GitTrackSelectionAdapter),
            Arc::new(infrastructure::track::FsTrackViewsAdapter::new()),
        ));
    let track_next_task_service =
        Arc::new(usecase::track_lifecycle::track_next_task::TrackNextTaskInteractor::new(
            Arc::new(RequestScopedTrackNextTaskQueryAdapter),
            Arc::new(infrastructure::track::GitTrackSelectionAdapter),
        ));
    let service = Arc::new(TrackServiceImpl);
    let fixpoint_resolve_service =
        Arc::new(usecase::fixpoint_resolve_driver::FixpointResolveDriverInteractor::new(
            Arc::new(
                infrastructure::track::fixpoint_resolve_driver::FsFixpointWorkspaceContextAdapter,
            ),
            Arc::new(infrastructure::track::fixpoint_resolve_driver::FsDryCheckConfigLoaderAdapter),
            Arc::new(
                infrastructure::track::fixpoint_resolve_driver::FsFixpointDryGateFactoryAdapter,
            ),
            Arc::new(
                infrastructure::track::fixpoint_resolve_driver::FsFixpointGateStateFactoryAdapter,
            ),
        ));
    let base_merge_cleanup: Arc<dyn usecase::base_merge::BaseMergeCleanupPort> =
        Arc::new(infrastructure::base_merge::FsBaseMergeCleanupAdapter::new());
    let base_merge_service = Arc::new(usecase::base_merge::BaseMergeInteractor::new(
        Arc::new(infrastructure::base_merge::FsBaseMergeContextAdapter::new()),
        Arc::new(infrastructure::base_merge::FsBaseMergeGitAdapter::new()),
        base_merge_cleanup,
    ));
    cli_driver::track::TrackDriver::new(
        track_init_service,
        track_archive_service,
        track_branch_create_service,
        track_branch_switch_service,
        service,
        fixpoint_resolve_service,
        base_merge_service,
        track_add_task_service,
        track_next_task_service,
    )
}

pub(crate) fn build_track_tddd_driver() -> cli_driver::track_tddd::TrackTdddDriver {
    use std::sync::Arc;

    let operation = Arc::new(
        infrastructure::track_lifecycle::tddd::baseline_capture::SystemTrackBaselineCaptureAdapter,
    );
    let resolver = Arc::new(infrastructure::track::GitTrackSelectionAdapter);
    let service = Arc::new(
        usecase::track_lifecycle::tddd::baseline_capture::TrackBaselineCaptureInteractor::new(
            operation, resolver,
        ),
    );
    let baseline_graph_operation = Arc::new(
        infrastructure::track_lifecycle::tddd::baseline_graph::SystemTrackBaselineGraphAdapter,
    );
    let baseline_graph_resolver = Arc::new(infrastructure::track::GitTrackSelectionAdapter);
    let baseline_graph_service = Arc::new(
        usecase::track_lifecycle::tddd::baseline_graph::TrackBaselineGraphInteractor::new(
            baseline_graph_operation,
            baseline_graph_resolver,
        ),
    );
    let catalogue_impl_signals_operation = Arc::new(
        infrastructure::track_lifecycle::tddd::catalogue_impl_signals::SystemTrackCatalogueImplSignalsAdapter,
    );
    let catalogue_impl_signals_resolver = Arc::new(infrastructure::track::GitTrackSelectionAdapter);
    let catalogue_impl_signals_service = Arc::new(
        usecase::track_lifecycle::tddd::catalogue_impl_signals::TrackCatalogueImplSignalsInteractor::new(
            catalogue_impl_signals_operation,
            catalogue_impl_signals_resolver,
        ),
    );
    let catalogue_spec_signals_operation = Arc::new(
        infrastructure::track_lifecycle::tddd::catalogue_spec_signals::SystemTrackCatalogueSpecSignalsAdapter,
    );
    let catalogue_spec_signals_resolver = Arc::new(infrastructure::track::GitTrackSelectionAdapter);
    let catalogue_spec_signals_service = Arc::new(
        usecase::track_lifecycle::tddd::catalogue_spec_signals::TrackCatalogueSpecSignalsInteractor::new(
            catalogue_spec_signals_operation,
            catalogue_spec_signals_resolver,
        ),
    );
    let catalogue_lint_active_operation = Arc::new(
        infrastructure::track_lifecycle::tddd::catalogue_lint_active::SystemTrackCatalogueLintActiveAdapter,
    );
    let catalogue_lint_active_resolver = Arc::new(infrastructure::track::GitTrackSelectionAdapter);
    let catalogue_lint_active_service = Arc::new(
        usecase::track_lifecycle::tddd::catalogue_lint_active::TrackCatalogueLintActiveInteractor::new(
            catalogue_lint_active_operation,
            catalogue_lint_active_resolver,
        ),
    );
    let lint_operation =
        Arc::new(infrastructure::track_lifecycle::tddd::lint::SystemTrackLintAdapter);
    let lint_resolver = Arc::new(infrastructure::track::GitTrackSelectionAdapter);
    let lint_service = Arc::new(usecase::track_lifecycle::tddd::lint::TrackLintInteractor::new(
        lint_operation,
        lint_resolver,
    ));
    cli_driver::track_tddd::TrackTdddDriver::new(
        service,
        baseline_graph_service,
        catalogue_impl_signals_service,
        catalogue_spec_signals_service,
        catalogue_lint_active_service,
        lint_service,
    )
}

/// Rebuilds the existing task-operation interactor from the command's items directory.
///
/// T006 reuses `TaskOperationInteractor` as `TrackTaskAddPort`; the store root is
/// request-scoped, so this wiring helper reconstructs that interactor per call.
struct RequestScopedTrackTaskAddAdapter;

impl usecase::track_lifecycle::TrackTaskAddPort for RequestScopedTrackTaskAddAdapter {
    fn add_task(
        &self,
        track_id: domain::TrackId,
        items_dir: usecase::track_lifecycle::TrackItemsDirectory,
        description: domain::NonEmptyString,
        section: Option<domain::NonEmptyString>,
        after: Option<domain::TaskId>,
    ) -> Result<usecase::task_ops::TaskOperationOutput, usecase::task_ops::TaskOperationError> {
        use std::sync::Arc;

        use infrastructure::track::fs_store::FsTrackStore;

        let project_root = super::resolve_project_root(items_dir.as_path()).map_err(|error| {
            usecase::task_ops::TaskOperationError::StoreFailed(error.to_string())
        })?;
        let store = Arc::new(FsTrackStore::new(items_dir.as_path().to_path_buf()));
        let interactor = super::build_task_operation_interactor(
            store,
            super::build_branch_reader(&project_root),
        );
        usecase::track_lifecycle::TrackTaskAddPort::add_task(
            &interactor,
            track_id,
            items_dir,
            description,
            section,
            after,
        )
    }
}

struct RequestScopedTrackNextTaskQueryAdapter;

impl usecase::track_lifecycle::TrackNextTaskQueryPort for RequestScopedTrackNextTaskQueryAdapter {
    fn next_task(
        &self,
        track_id: domain::TrackId,
        items_dir: usecase::track_lifecycle::TrackItemsDirectory,
    ) -> Result<
        Option<usecase::task_ops::NextTaskOutput>,
        usecase::track_lifecycle::track_next_task::TrackNextTaskError,
    > {
        use std::sync::Arc;

        use infrastructure::track::fs_store::FsTrackStore;

        let store = Arc::new(FsTrackStore::new(items_dir.as_path().to_path_buf()));
        let query = usecase::task_ops::TaskQueryInteractor::new(store);
        usecase::track_lifecycle::TrackNextTaskQueryPort::next_task(&query, track_id, items_dir)
    }
}
