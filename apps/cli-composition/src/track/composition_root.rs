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
    let track_transition_service =
        Arc::new(usecase::track_lifecycle::track_transition::TrackTransitionInteractor::new(
            Arc::new(RequestScopedTrackTaskTransitionAdapter),
            Arc::new(infrastructure::track::GitTrackSelectionAdapter),
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
    let track_set_override_service =
        Arc::new(usecase::track_lifecycle::track_set_override::TrackSetOverrideInteractor::new(
            Arc::new(RequestScopedTrackOverrideSetAdapter),
            Arc::new(infrastructure::track::GitTrackSelectionAdapter),
            Arc::new(infrastructure::track::FsTrackViewsAdapter::new()),
        ));
    let track_clear_override_service = Arc::new(
        usecase::track_lifecycle::track_clear_override::TrackClearOverrideInteractor::new(
            Arc::new(RequestScopedTrackOverrideClearAdapter),
            Arc::new(infrastructure::track::GitTrackSelectionAdapter),
            Arc::new(infrastructure::track::FsTrackViewsAdapter::new()),
        ),
    );
    let track_set_commit_hash_service = Arc::new(
        usecase::track_lifecycle::track_set_commit_hash::TrackSetCommitHashInteractor::new(
            Arc::new(infrastructure::track::GitTrackCommitHashAdapter::new()),
        ),
    );
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
        track_transition_service,
        track_set_override_service,
        track_clear_override_service,
        track_set_commit_hash_service,
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
    let spec_element_hash_service = Arc::new(
        usecase::track_lifecycle::tddd::spec_element_hash::TrackSpecElementHashInteractor::new(
            Arc::new(
                infrastructure::track_lifecycle::tddd::spec_element_hash::SystemTrackSpecElementHashAdapter,
            ),
            Arc::new(infrastructure::track::GitTrackSelectionAdapter),
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
    let contract_map_operation = Arc::new(
        infrastructure::track_lifecycle::tddd::contract_map::SystemTrackContractMapAdapter,
    );
    let contract_map_resolver = Arc::new(infrastructure::track::GitTrackSelectionAdapter);
    let contract_map_service =
        Arc::new(usecase::track_lifecycle::tddd::contract_map::TrackContractMapInteractor::new(
            contract_map_operation,
            contract_map_resolver,
        ));
    let type_signals_service =
        Arc::new(usecase::track_lifecycle::tddd::type_signals::TrackTypeSignalsInteractor::new(
            Arc::new(
                infrastructure::track_lifecycle::tddd::type_signals::SystemTrackTypeSignalsAdapter,
            ),
            Arc::new(infrastructure::track::GitTrackSelectionAdapter),
        ));
    cli_driver::track_tddd::TrackTdddDriver::new(
        service,
        baseline_graph_service,
        catalogue_impl_signals_service,
        catalogue_spec_signals_service,
        spec_element_hash_service,
        catalogue_lint_active_service,
        lint_service,
        contract_map_service,
        type_signals_service,
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

/// Rebuilds the existing task-operation interactor from the command's items directory
/// for a task transition.
struct RequestScopedTrackTaskTransitionAdapter;

impl usecase::track_lifecycle::TrackTaskTransitionPort for RequestScopedTrackTaskTransitionAdapter {
    fn transition_task(
        &self,
        track_id: domain::TrackId,
        items_dir: usecase::track_lifecycle::TrackItemsDirectory,
        task_id: domain::TaskId,
        transition: usecase::track_lifecycle::TrackTaskTransition,
    ) -> Result<usecase::task_ops::TaskTransitionOutcome, usecase::task_ops::TaskOperationError>
    {
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
        usecase::track_lifecycle::TrackTaskTransitionPort::transition_task(
            &interactor,
            track_id,
            items_dir,
            task_id,
            transition,
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

/// Rebuilds the existing task-operation interactor from the command's items directory
/// for a clear-override mutation. The port remains request-scoped so the storage root
/// is never captured in a long-lived composition object.
struct RequestScopedTrackOverrideClearAdapter;

impl usecase::track_lifecycle::TrackOverrideClearPort for RequestScopedTrackOverrideClearAdapter {
    fn clear_override(
        &self,
        track_id: domain::TrackId,
        items_dir: usecase::track_lifecycle::TrackItemsDirectory,
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
        usecase::task_ops::TaskOperationService::clear_override(
            &interactor,
            usecase::task_ops::ClearOverrideCommand {
                items_dir: items_dir.as_path().to_path_buf(),
                track_id: track_id.as_ref().to_owned(),
            },
        )
    }
}

/// Rebuilds the existing task-operation interactor from the command's items directory
/// for a set-override mutation. The port remains request-scoped so the storage root
/// is never captured in a long-lived composition object.
struct RequestScopedTrackOverrideSetAdapter;

impl usecase::track_lifecycle::TrackOverrideSetPort for RequestScopedTrackOverrideSetAdapter {
    fn set_override(
        &self,
        track_id: domain::TrackId,
        items_dir: usecase::track_lifecycle::TrackItemsDirectory,
        status: domain::StatusOverrideKind,
        reason: usecase::git_workflow::DiagnosticText,
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
        usecase::track_lifecycle::TrackOverrideSetPort::set_override(
            &interactor,
            track_id,
            items_dir,
            status,
            reason,
        )
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use cli_driver::track::TrackInput;

    #[test]
    fn test_track_set_override_active_selection_call_site_preserves_cli_contract() {
        let root = tempfile::tempdir().unwrap();
        crate::test_support::seed_repo(root.path(), "track/active-track");
        let items_dir = root.path().join("track/items");
        let track_dir = items_dir.join("active-track");
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(
            track_dir.join("metadata.json"),
            r#"{
  "schema_version": 6,
  "id": "active-track",
  "branch": null,
  "title": "Active Track",
  "created_at": "2026-03-13T00:00:00Z",
  "updated_at": "2026-03-13T00:00:00Z",
  "branch_strategy_snapshot": {
    "base_branch": "main",
    "merge_target": "main",
    "merge_method": "squash"
  }
}"#,
        )
        .unwrap();

        let argv_items_dir = items_dir.clone();
        let argv_track_id = None;
        let argv_status = "blocked".to_owned();
        let argv_reason = "active-selection blocker".to_owned();
        let outcome = TrackCompositionRoot::new().track_driver().handle(TrackInput::SetOverride {
            items_dir: argv_items_dir.clone(),
            track_id: argv_track_id,
            status: argv_status.clone(),
            reason: argv_reason.clone(),
        });

        assert_eq!(outcome.exit_code, 0);
        assert!(
            outcome
                .stdout
                .as_deref()
                .is_some_and(|stdout| stdout.contains("Override set to 'blocked'"))
        );
        assert_eq!(outcome.stderr, None);
        assert_eq!(argv_items_dir, items_dir);
        assert_eq!(argv_status, "blocked");
        assert_eq!(argv_reason, "active-selection blocker");
        let persisted = std::fs::read_to_string(track_dir.join("metadata.json")).unwrap();
        assert!(persisted.contains("\"blocked\""));
        assert!(persisted.contains("active-selection blocker"));
    }

    #[test]
    fn test_track_transition_call_site_persists_status() {
        let root = tempfile::tempdir().unwrap();
        let run_git = |args: &[&str]| {
            let status = std::process::Command::new("git")
                .args(args)
                .current_dir(root.path())
                .status()
                .expect("git command failed to spawn");
            assert!(status.success(), "git {args:?} failed with {status}");
        };
        run_git(&["init", "-q"]);
        run_git(&["config", "user.email", "test@example.com"]);
        run_git(&["config", "user.name", "Test"]);
        run_git(&["config", "commit.gpgsign", "false"]);
        run_git(&["commit", "--allow-empty", "-q", "-m", "init", "--no-gpg-sign"]);
        run_git(&["branch", "-m", "track/synthetic-2026"]);
        run_git(&["branch", "main"]);
        let items_dir = root.path().join("track/items");
        let track_dir = items_dir.join("synthetic-2026");
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(
            track_dir.join("metadata.json"),
            r#"{
  "schema_version": 6,
  "id": "synthetic-2026",
  "branch": null,
  "title": "Persist Track",
  "created_at": "2026-03-13T00:00:00Z",
  "updated_at": "2026-03-13T00:00:00Z",
  "branch_strategy_snapshot": {
    "base_branch": "main",
    "merge_target": "main",
    "merge_method": "squash"
  }
}"#,
        )
        .unwrap();
        std::fs::write(
            track_dir.join("impl-plan.json"),
            r#"{
  "schema_version": 1,
  "tasks": [
    { "id": "T001", "description": "First task", "status": "todo" },
    { "id": "T002", "description": "Remaining work", "status": "todo" }
  ],
  "plan": {
    "summary": [],
    "sections": [
      { "id": "S1", "title": "Phase 1", "description": [], "task_ids": ["T001", "T002"] }
    ]
  }
}"#,
        )
        .unwrap();
        std::fs::write(
            track_dir.join("batch-plan.json"),
            r#"{
  "schema_version": 1,
  "track_id": "synthetic-2026",
  "task_estimates": [
    {"task_id":"T001","scope_estimates":[{"scope":"domain","production_lines":10,"test_lines":5}],"oversize_justification":null},
    {"task_id":"T002","scope_estimates":[{"scope":"domain","production_lines":10,"test_lines":5}],"oversize_justification":null}
  ],
  "batches":[{"id":"B1","task_ids":["T001","T002"]}]
}"#,
        )
        .unwrap();
        let config_dir = root.path().join(".harness/config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("review-scope.json"),
            r#"{"version":2,"groups":{"domain":{"patterns":["libs/domain/**"]}},"review_operational":[],"other_track":[],"default_diff_ceiling_lines":500}"#,
        )
        .unwrap();
        std::fs::write(
            config_dir.join("scope-diff-exclusions.json"),
            r#"{"schema_version":1,"exclusions":["target/**"]}"#,
        )
        .unwrap();

        let outcome = TrackCompositionRoot::new().track_driver().handle(TrackInput::Transition {
            items_dir: items_dir.clone(),
            track_id: Some("synthetic-2026".to_owned()),
            task_id: "T001".to_owned(),
            target_status: "in_progress".to_owned(),
            commit_hash: None,
        });
        assert_eq!(outcome.exit_code, 0, "stderr={:?}", outcome.stderr);
        let persisted = std::fs::read_to_string(track_dir.join("impl-plan.json")).unwrap();
        assert!(
            persisted.contains("\"in_progress\""),
            "transition must persist in_progress:\n{persisted}"
        );
        let metadata = std::fs::read_to_string(track_dir.join("metadata.json")).unwrap();
        assert!(
            !metadata.contains("\"status\""),
            "metadata must not store derived status:\n{metadata}"
        );
    }
}
