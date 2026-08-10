//! `track` command family — primary adapter driver.
//!
//! `TrackDriver` holds an injected [`usecase::track_service::TrackService`]
//! and exposes `handle(input) -> CommandOutcome`.

use std::path::PathBuf;
use std::sync::Arc;

use usecase::base_merge::{BaseMergeCommand, BaseMergeOutcome, BaseMergeService};
use usecase::fixpoint_resolve_driver::{
    FixpointResolveDriverInput, FixpointResolveDriverOutcome, FixpointResolveDriverService,
};
use usecase::track_service::{TrackCommandOutput, TrackService};

use crate::render::CommandOutcome;

// ---------------------------------------------------------------------------
// Input type
// ---------------------------------------------------------------------------

/// Typed input for the `track` command family.
pub enum TrackInput {
    /// Initialize a new track (write metadata.json).
    Init {
        /// Items directory (`track/items`).
        items_dir: PathBuf,
        /// Track ID string (e.g. `T001`).
        track_id: String,
        /// Short description of the track.
        description: String,
    },
    /// Transition a task to a new status.
    Transition {
        /// Items directory (`track/items`).
        items_dir: PathBuf,
        /// Track ID string.
        track_id: Option<String>,
        /// Task ID (e.g. `T001`).
        task_id: String,
        /// Target status string (e.g. `done`, `in_progress`).
        target_status: String,
        /// Commit hash (required when target_status is `done`, optional otherwise).
        commit_hash: Option<String>,
    },
    /// Resolve the current track phase, next command, and optional blocker.
    Resolve {
        /// Items directory (`track/items`).
        items_dir: PathBuf,
        /// Track ID string (resolved from git branch if `None`).
        track_id: Option<String>,
    },
    /// Create a new track branch from the configured base branch.
    BranchCreate {
        /// Items directory (`track/items`).
        items_dir: PathBuf,
        /// Track ID string.
        track_id: String,
    },
    /// Switch to an existing track branch.
    BranchSwitch {
        /// Items directory (`track/items`).
        items_dir: PathBuf,
        /// Track ID string.
        track_id: String,
    },
    /// Validate metadata.json files under the repository.
    ViewsValidate {
        /// Project root directory.
        project_root: PathBuf,
    },
    /// Render plan.md and registry.md from metadata.json.
    ViewsSync {
        /// Project root directory.
        project_root: PathBuf,
        /// Track ID string (auto-detected from branch if `None`).
        track_id: Option<String>,
    },
    /// Add a new task to a track.
    AddTask {
        /// Items directory (`track/items`).
        items_dir: PathBuf,
        /// Track ID string (resolved from git branch if `None`).
        track_id: Option<String>,
        /// Short task description.
        description: String,
        /// Optional plan section to file the task under.
        section: Option<String>,
        /// Insert after this task ID (e.g. `T003`).
        after: Option<String>,
    },
    /// Set a status override (blocked/cancelled) on a track.
    SetOverride {
        /// Items directory (`track/items`).
        items_dir: PathBuf,
        /// Track ID string (resolved from git branch if `None`).
        track_id: Option<String>,
        /// Override status string.
        status: String,
        /// Human-readable reason for the override.
        reason: String,
    },
    /// Clear a status override on a track.
    ClearOverride {
        /// Items directory (`track/items`).
        items_dir: PathBuf,
        /// Track ID string (resolved from git branch if `None`).
        track_id: Option<String>,
    },
    /// Show the next open task for a track (JSON output).
    NextTask {
        /// Items directory (`track/items`).
        items_dir: PathBuf,
        /// Track ID string (resolved from git branch if `None`).
        track_id: Option<String>,
    },
    /// Show task status counts for a track (JSON output).
    TaskCounts {
        /// Items directory (`track/items`).
        items_dir: PathBuf,
        /// Track ID string (resolved from git branch if `None`).
        track_id: Option<String>,
    },
    /// Archive a completed track.
    Archive {
        /// Items directory (`track/items`).
        items_dir: PathBuf,
        /// Track ID string.
        track_id: String,
    },
    /// Detect the active track ID from the current git branch.
    DetectActive {
        /// Project root directory.
        project_root: PathBuf,
    },
    /// Resolve the next fixpoint step for the active track.
    FixpointResolve {
        /// Active track ID (directory name under `items_dir/<id>`).
        track_id: String,
        /// Current git branch label (e.g. `"track/my-feature-2026"`).
        current_branch: String,
        /// Path to the `track/items` directory.
        items_dir: PathBuf,
    },
    /// Switch to the configured base branch from the active track's
    /// `branch_strategy_snapshot`.
    SwitchBase {
        /// Project root directory.
        project_root: PathBuf,
    },
    /// Run the catalogue lint ruleset across every `tddd.enabled` layer of
    /// the active track and aggregate violations.
    CatalogueLintCheckActiveTrack {
        /// Track ID string (resolved from git branch if `None`).
        track_id: Option<String>,
        /// Workspace root directory (contains `architecture-rules.json` and
        /// `track/items/`).
        workspace_root: PathBuf,
        /// Optional override for the lint config file path (defaults to
        /// `.harness/catalogue-lint/config.json` under `workspace_root`).
        rules_file: Option<PathBuf>,
    },
}

/// Typed input for a guarded base-to-track merge.
pub struct BaseMergeInput {
    /// Workspace root containing the active track checkout.
    pub workspace_root: PathBuf,
}

// ---------------------------------------------------------------------------
// Conversion helpers
// ---------------------------------------------------------------------------

fn service_output_to_outcome(output: TrackCommandOutput) -> CommandOutcome {
    CommandOutcome { stdout: output.stdout, stderr: output.stderr, exit_code: output.exit_code }
}

/// Render a [`FixpointResolveDriverOutcome`] into a [`CommandOutcome`].
///
/// Reproduces the exact text contract previously produced by
/// `cli_composition::track::fixpoint_resolve::format_fixpoint_step`:
/// - `RunDfp` → `"run-dfp"`
/// - `RunRfp { scopes }` → `"run-rfp scopes=<s1>,<s2>..."` (scopes already
///   sorted by the usecase layer)
/// - `RunRefVerify` → `"run-ref-verify"`
/// - `Commit` → `"commit"`
/// - `Failure { message }` → failure outcome carrying `message`
fn render_fixpoint_resolve_outcome(outcome: FixpointResolveDriverOutcome) -> CommandOutcome {
    match outcome {
        FixpointResolveDriverOutcome::RunDfp => CommandOutcome::success(Some("run-dfp".to_owned())),
        FixpointResolveDriverOutcome::RunRfp { scopes } => {
            CommandOutcome::success(Some(format!("run-rfp scopes={}", scopes.join(","))))
        }
        FixpointResolveDriverOutcome::RunRefVerify => {
            CommandOutcome::success(Some("run-ref-verify".to_owned()))
        }
        FixpointResolveDriverOutcome::Commit => CommandOutcome::success(Some("commit".to_owned())),
        FixpointResolveDriverOutcome::Failure { message } => CommandOutcome::failure(Some(message)),
    }
}

/// Render the application-service result of a guarded base merge.
fn render_base_merge_result(
    result: Result<BaseMergeOutcome, usecase::base_merge::BaseMergeError>,
) -> CommandOutcome {
    match result {
        Ok(BaseMergeOutcome::Completed) => {
            CommandOutcome::success(Some("base merge completed".to_owned()))
        }
        Ok(BaseMergeOutcome::Conflicted) => CommandOutcome::failure(Some(
            "base merge conflicted; continue with the recover workflow (/track:recover on Claude Code, $track-recover on Codex)".to_owned(),
        )),
        Err(error) => CommandOutcome::failure(Some(format!("base merge failed: {error}"))),
    }
}

// ---------------------------------------------------------------------------
// Driver
// ---------------------------------------------------------------------------

/// Primary adapter driver for the `track` command family.
///
/// Holds an injected [`TrackService`]; exposes `handle(input) -> CommandOutcome`.
pub struct TrackDriver {
    service: Arc<dyn TrackService>,
    fixpoint_resolve_service: Arc<dyn FixpointResolveDriverService>,
    base_merge_service: Arc<dyn BaseMergeService>,
}

impl TrackDriver {
    /// Create a new `TrackDriver` with the given services.
    pub fn new(
        service: Arc<dyn TrackService>,
        fixpoint_resolve_service: Arc<dyn FixpointResolveDriverService>,
        base_merge_service: Arc<dyn BaseMergeService>,
    ) -> Self {
        Self { service, fixpoint_resolve_service, base_merge_service }
    }

    /// Handle a guarded base-to-track merge through the application service.
    #[must_use]
    pub fn handle_base_merge(&self, input: BaseMergeInput) -> CommandOutcome {
        render_base_merge_result(
            self.base_merge_service
                .execute(BaseMergeCommand { workspace_root: input.workspace_root }),
        )
    }

    /// Handle a track command.
    pub fn handle(&self, input: TrackInput) -> CommandOutcome {
        match input {
            TrackInput::Init { items_dir, track_id, description } => {
                service_output_to_outcome(self.service.init(items_dir, track_id, description))
            }
            TrackInput::Transition { items_dir, track_id, task_id, target_status, commit_hash } => {
                service_output_to_outcome(self.service.transition(
                    items_dir,
                    track_id,
                    task_id,
                    target_status,
                    commit_hash,
                ))
            }
            TrackInput::Resolve { items_dir, track_id } => {
                service_output_to_outcome(self.service.resolve(items_dir, track_id))
            }
            TrackInput::BranchCreate { items_dir, track_id } => {
                service_output_to_outcome(self.service.branch_create(items_dir, track_id))
            }
            TrackInput::BranchSwitch { items_dir, track_id } => {
                service_output_to_outcome(self.service.branch_switch(items_dir, track_id))
            }
            TrackInput::ViewsValidate { project_root } => {
                service_output_to_outcome(self.service.views_validate(project_root))
            }
            TrackInput::ViewsSync { project_root, track_id } => {
                service_output_to_outcome(self.service.views_sync(project_root, track_id))
            }
            TrackInput::AddTask { items_dir, track_id, description, section, after } => {
                service_output_to_outcome(self.service.add_task(
                    items_dir,
                    track_id,
                    description,
                    section,
                    after,
                ))
            }
            TrackInput::SetOverride { items_dir, track_id, status, reason } => {
                service_output_to_outcome(
                    self.service.set_override(items_dir, track_id, status, reason),
                )
            }
            TrackInput::ClearOverride { items_dir, track_id } => {
                service_output_to_outcome(self.service.clear_override(items_dir, track_id))
            }
            TrackInput::NextTask { items_dir, track_id } => {
                service_output_to_outcome(self.service.next_task(items_dir, track_id))
            }
            TrackInput::TaskCounts { items_dir, track_id } => {
                service_output_to_outcome(self.service.task_counts(items_dir, track_id))
            }
            TrackInput::Archive { items_dir, track_id } => {
                service_output_to_outcome(self.service.archive(items_dir, track_id))
            }
            TrackInput::DetectActive { project_root } => {
                service_output_to_outcome(self.service.detect_active(project_root))
            }
            TrackInput::FixpointResolve { track_id, current_branch, items_dir } => {
                let outcome =
                    self.fixpoint_resolve_service.fixpoint_resolve(FixpointResolveDriverInput {
                        track_id,
                        current_branch,
                        items_dir,
                    });
                render_fixpoint_resolve_outcome(outcome)
            }
            TrackInput::SwitchBase { project_root } => {
                service_output_to_outcome(self.service.switch_base(project_root))
            }
            TrackInput::CatalogueLintCheckActiveTrack { track_id, workspace_root, rules_file } => {
                service_output_to_outcome(self.service.catalogue_lint_check_active_track(
                    track_id,
                    workspace_root,
                    rules_file,
                ))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Render helpers (previously duplicated from cli_composition; now unused in
// cli_driver since delegation happens via TrackService)
// ---------------------------------------------------------------------------

/// Sync views and return formatted status lines.
///
/// Mirrors `cli_composition::track::sync_views_to_stdout` (mod.rs lines 114-127).
fn sync_views_to_stdout(_project_root: &std::path::Path, _track_id: &str) -> Vec<String> {
    vec![]
}

/// Format a task status counts JSON string from raw counts.
///
/// Mirrors `cli_composition::track::ops::track_task_counts_resolved` JSON format
/// (ops.rs lines 226-230).
fn format_task_counts_json(
    total: u64,
    todo: u64,
    in_progress: u64,
    done: u64,
    skipped: u64,
) -> String {
    format!(
        r#"{{"total":{total},"todo":{todo},"in_progress":{in_progress},"done":{done},"skipped":{skipped}}}"#,
    )
}

// Keep helpers in scope — will be removed in T024 once composition copies are deleted.
const _: fn() = || {
    let _ = sync_views_to_stdout;
    let _ = format_task_counts_json;
};

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unreachable, clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    use domain::{
        BaseMergeDirection, BranchStrategySnapshot, CommitHash, MergeMethod, NonEmptyString,
        TrackBranch, TrackId, TrackMetadata,
    };
    use usecase::base_merge::{
        BaseMergeAttemptOutcome, BaseMergeCleanupPort, BaseMergeCleanupRequest,
        BaseMergeContextError, BaseMergeContextPort, BaseMergeGitError, BaseMergeGitPort,
        BaselineReplacementError, SyncBaseRecordError, ViewsRegenerationError,
    };

    struct UnusedTrackService;

    impl TrackService for UnusedTrackService {
        fn init(&self, _: PathBuf, _: String, _: String) -> TrackCommandOutput {
            unreachable!()
        }
        fn transition(
            &self,
            _: PathBuf,
            _: Option<String>,
            _: String,
            _: String,
            _: Option<String>,
        ) -> TrackCommandOutput {
            unreachable!()
        }
        fn resolve(&self, _: PathBuf, _: Option<String>) -> TrackCommandOutput {
            unreachable!()
        }
        fn branch_create(&self, _: PathBuf, _: String) -> TrackCommandOutput {
            unreachable!()
        }
        fn branch_switch(&self, _: PathBuf, _: String) -> TrackCommandOutput {
            unreachable!()
        }
        fn views_validate(&self, _: PathBuf) -> TrackCommandOutput {
            unreachable!()
        }
        fn views_sync(&self, _: PathBuf, _: Option<String>) -> TrackCommandOutput {
            unreachable!()
        }
        fn add_task(
            &self,
            _: PathBuf,
            _: Option<String>,
            _: String,
            _: Option<String>,
            _: Option<String>,
        ) -> TrackCommandOutput {
            unreachable!()
        }
        fn set_override(
            &self,
            _: PathBuf,
            _: Option<String>,
            _: String,
            _: String,
        ) -> TrackCommandOutput {
            unreachable!()
        }
        fn clear_override(&self, _: PathBuf, _: Option<String>) -> TrackCommandOutput {
            unreachable!()
        }
        fn next_task(&self, _: PathBuf, _: Option<String>) -> TrackCommandOutput {
            unreachable!()
        }
        fn task_counts(&self, _: PathBuf, _: Option<String>) -> TrackCommandOutput {
            unreachable!()
        }
        fn archive(&self, _: PathBuf, _: String) -> TrackCommandOutput {
            unreachable!()
        }
        fn detect_active(&self, _: PathBuf) -> TrackCommandOutput {
            unreachable!()
        }
        fn catalogue_lint_check_active_track(
            &self,
            _: Option<String>,
            _: PathBuf,
            _: Option<PathBuf>,
        ) -> TrackCommandOutput {
            unreachable!()
        }
    }

    struct UnusedFixpointResolveService;

    impl FixpointResolveDriverService for UnusedFixpointResolveService {
        fn fixpoint_resolve(&self, _: FixpointResolveDriverInput) -> FixpointResolveDriverOutcome {
            unreachable!()
        }
    }

    struct StubBaseMergeService {
        result: Mutex<Option<Result<BaseMergeOutcome, usecase::base_merge::BaseMergeError>>>,
        workspaces: Mutex<Vec<PathBuf>>,
    }

    impl StubBaseMergeService {
        fn new(result: Result<BaseMergeOutcome, usecase::base_merge::BaseMergeError>) -> Self {
            Self { result: Mutex::new(Some(result)), workspaces: Mutex::new(Vec::new()) }
        }
    }

    impl BaseMergeService for StubBaseMergeService {
        fn execute(
            &self,
            command: BaseMergeCommand,
        ) -> Result<BaseMergeOutcome, usecase::base_merge::BaseMergeError> {
            self.workspaces.lock().unwrap().push(command.workspace_root);
            self.result.lock().unwrap().take().unwrap()
        }
    }

    fn base_merge_driver<T: BaseMergeService + 'static>(service: Arc<T>) -> TrackDriver {
        TrackDriver::new(
            Arc::new(UnusedTrackService),
            Arc::new(UnusedFixpointResolveService),
            service,
        )
    }

    fn base_merge_direction() -> BaseMergeDirection {
        let track_id = TrackId::try_new("merge-track").unwrap();
        let branch = TrackBranch::try_new("track/merge-track").unwrap();
        let metadata = TrackMetadata::with_branch(
            track_id,
            Some(branch),
            "Merge track",
            None,
            BranchStrategySnapshot::new(
                NonEmptyString::try_new("develop").unwrap(),
                NonEmptyString::try_new("develop").unwrap(),
                MergeMethod::Merge,
            ),
        )
        .unwrap();
        domain::derive_base_merge_direction(&metadata).unwrap()
    }

    struct RecordingContext {
        direction: Option<BaseMergeDirection>,
        mismatch: Option<(TrackBranch, TrackBranch)>,
        workspaces: Arc<Mutex<Vec<PathBuf>>>,
    }

    impl BaseMergeContextPort for RecordingContext {
        fn load_direction(
            &self,
            workspace_root: &std::path::Path,
        ) -> Result<BaseMergeDirection, BaseMergeContextError> {
            self.workspaces.lock().unwrap().push(workspace_root.to_path_buf());
            match (&self.direction, &self.mismatch) {
                (Some(direction), None) => Ok(direction.clone()),
                (None, Some((current, expected))) => {
                    Err(BaseMergeContextError::ActiveTrackMismatch {
                        current: current.clone(),
                        expected: expected.clone(),
                    })
                }
                _ => Err(BaseMergeContextError::Unavailable(
                    usecase::git_workflow::DiagnosticText::new("invalid test context"),
                )),
            }
        }
    }

    struct SnapshotSourceMismatchContext {
        workspaces: Arc<Mutex<Vec<PathBuf>>>,
    }

    impl BaseMergeContextPort for SnapshotSourceMismatchContext {
        fn load_direction(
            &self,
            workspace_root: &std::path::Path,
        ) -> Result<BaseMergeDirection, BaseMergeContextError> {
            self.workspaces.lock().unwrap().push(workspace_root.to_path_buf());
            Err(BaseMergeContextError::Unavailable(usecase::git_workflow::DiagnosticText::new(
                "requested source 'release' differs from snapshot base 'develop'",
            )))
        }
    }

    struct RecordingGitPort {
        calls: Arc<Mutex<Vec<(PathBuf, BaseMergeDirection)>>>,
    }

    impl BaseMergeGitPort for RecordingGitPort {
        fn ensure_worktree_clean(
            &self,
            _workspace_root: &std::path::Path,
        ) -> Result<(), BaseMergeGitError> {
            Ok(())
        }

        fn merge_base(
            &self,
            workspace_root: &std::path::Path,
            direction: &BaseMergeDirection,
        ) -> Result<BaseMergeAttemptOutcome, BaseMergeGitError> {
            self.calls.lock().unwrap().push((workspace_root.to_path_buf(), direction.clone()));
            Ok(BaseMergeAttemptOutcome::Clean {
                base_commit: CommitHash::try_new("fedcba9876543210").unwrap(),
            })
        }
    }

    struct RecordingCleanupPort {
        calls: Arc<Mutex<Vec<(&'static str, BaseMergeCleanupRequest)>>>,
    }

    impl RecordingCleanupPort {
        fn record(&self, stage: &'static str, request: &BaseMergeCleanupRequest) {
            self.calls.lock().unwrap().push((stage, request.clone()));
        }
    }

    impl BaseMergeCleanupPort for RecordingCleanupPort {
        fn regenerate_views(
            &self,
            request: &BaseMergeCleanupRequest,
        ) -> Result<(), ViewsRegenerationError> {
            self.record("views", request);
            Ok(())
        }

        fn replace_baselines(
            &self,
            request: &BaseMergeCleanupRequest,
        ) -> Result<(), BaselineReplacementError> {
            self.record("baseline", request);
            Ok(())
        }

        fn write_sync_base_record(
            &self,
            request: &BaseMergeCleanupRequest,
        ) -> Result<(), SyncBaseRecordError> {
            self.record("sync-base", request);
            Ok(())
        }
    }

    // ── render_fixpoint_resolve_outcome ─────────────────────────────────────
    //
    // Relocated from `apps/cli-composition/src/track/fixpoint_resolve.rs`'s
    // `format_fixpoint_step` tests, adapted to exercise the render logic
    // directly with `FixpointResolveDriverOutcome` inputs instead of a domain
    // `FixpointStep` (cli_driver may only depend on usecase, not domain).

    #[test]
    fn test_render_fixpoint_resolve_outcome_run_dfp() {
        let outcome = render_fixpoint_resolve_outcome(FixpointResolveDriverOutcome::RunDfp);
        assert_eq!(outcome.stdout.as_deref(), Some("run-dfp"));
        assert_eq!(outcome.exit_code, 0);
    }

    #[test]
    fn test_render_fixpoint_resolve_outcome_run_rfp_single_scope() {
        let outcome = render_fixpoint_resolve_outcome(FixpointResolveDriverOutcome::RunRfp {
            scopes: vec!["impl-plan".to_owned()],
        });
        assert_eq!(outcome.stdout.as_deref(), Some("run-rfp scopes=impl-plan"));
    }

    #[test]
    fn test_render_fixpoint_resolve_outcome_run_rfp_multiple_scopes_in_btreeset_order() {
        // "code" < "impl-plan" in BTreeSet order.
        let outcome = render_fixpoint_resolve_outcome(FixpointResolveDriverOutcome::RunRfp {
            scopes: vec!["code".to_owned(), "impl-plan".to_owned()],
        });
        assert_eq!(outcome.stdout.as_deref(), Some("run-rfp scopes=code,impl-plan"));
    }

    #[test]
    fn test_render_fixpoint_resolve_outcome_run_ref_verify() {
        let outcome = render_fixpoint_resolve_outcome(FixpointResolveDriverOutcome::RunRefVerify);
        assert_eq!(outcome.stdout.as_deref(), Some("run-ref-verify"));
    }

    #[test]
    fn test_render_fixpoint_resolve_outcome_commit() {
        let outcome = render_fixpoint_resolve_outcome(FixpointResolveDriverOutcome::Commit);
        assert_eq!(outcome.stdout.as_deref(), Some("commit"));
    }

    #[test]
    fn test_render_fixpoint_resolve_outcome_failure() {
        let outcome = render_fixpoint_resolve_outcome(FixpointResolveDriverOutcome::Failure {
            message: "boom".to_owned(),
        });
        assert_eq!(outcome.stderr.as_deref(), Some("boom"));
        assert_eq!(outcome.exit_code, 1);
    }

    #[test]
    fn test_render_base_merge_result_completed_is_success() {
        let outcome = render_base_merge_result(Ok(BaseMergeOutcome::Completed));

        assert_eq!(outcome.exit_code, 0);
        assert_eq!(outcome.stdout.as_deref(), Some("base merge completed"));
    }

    #[test]
    fn test_render_base_merge_result_conflicted_requires_recovery() {
        let outcome = render_base_merge_result(Ok(BaseMergeOutcome::Conflicted));

        assert_eq!(outcome.exit_code, 1);
        assert_eq!(
            outcome.stderr.as_deref(),
            Some(
                "base merge conflicted; continue with the recover workflow (/track:recover on Claude Code, $track-recover on Codex)"
            )
        );
    }

    #[test]
    fn test_render_base_merge_result_error_is_failure() {
        let outcome = render_base_merge_result(Err(usecase::base_merge::BaseMergeError::Context(
            usecase::git_workflow::DiagnosticText::new("missing active track"),
        )));

        assert_eq!(outcome.exit_code, 1);
        assert_eq!(
            outcome.stderr.as_deref(),
            Some("base merge failed: base-merge context failed: missing active track")
        );
    }

    #[test]
    fn test_track_driver_handle_base_merge_delegates_workspace_and_renders_completed() {
        let service = Arc::new(StubBaseMergeService::new(Ok(BaseMergeOutcome::Completed)));
        let driver = base_merge_driver(Arc::clone(&service));
        let workspace = PathBuf::from("/workspace/unchanged");

        let outcome =
            driver.handle_base_merge(BaseMergeInput { workspace_root: workspace.clone() });

        assert_eq!(service.workspaces.lock().unwrap().as_slice(), [workspace]);
        assert_eq!(outcome.exit_code, 0);
        assert_eq!(outcome.stdout.as_deref(), Some("base merge completed"));
    }

    #[test]
    fn test_track_driver_handle_base_merge_real_interactor_orders_cleanup_at_exact_commit() {
        let workspace = PathBuf::from("/workspace/track-driver");
        let context_workspaces = Arc::new(Mutex::new(Vec::new()));
        let git_calls = Arc::new(Mutex::new(Vec::new()));
        let cleanup_calls = Arc::new(Mutex::new(Vec::new()));
        let direction = base_merge_direction();
        let interactor = usecase::base_merge::BaseMergeInteractor::new(
            Arc::new(RecordingContext {
                direction: Some(direction.clone()),
                mismatch: None,
                workspaces: Arc::clone(&context_workspaces),
            }),
            Arc::new(RecordingGitPort { calls: Arc::clone(&git_calls) }),
            Arc::new(RecordingCleanupPort { calls: Arc::clone(&cleanup_calls) }),
        );
        let driver = base_merge_driver(Arc::new(interactor));

        let outcome =
            driver.handle_base_merge(BaseMergeInput { workspace_root: workspace.clone() });

        assert_eq!(outcome.exit_code, 0);
        assert_eq!(outcome.stdout.as_deref(), Some("base merge completed"));
        assert_eq!(*context_workspaces.lock().unwrap(), vec![workspace.clone()]);
        assert_eq!(git_calls.lock().unwrap().as_slice(), [(workspace.clone(), direction)]);
        let cleanup_calls = cleanup_calls.lock().unwrap();
        assert_eq!(
            cleanup_calls.iter().map(|(stage, _)| *stage).collect::<Vec<_>>(),
            vec!["views", "baseline", "sync-base"]
        );
        for (_, request) in cleanup_calls.iter() {
            assert_eq!(request.workspace_root, workspace);
            assert_eq!(request.base_commit.as_ref(), "fedcba9876543210");
        }
    }

    #[test]
    fn test_track_driver_handle_base_merge_real_interactor_surfaces_active_track_guard_before_ports()
     {
        let context_workspaces = Arc::new(Mutex::new(Vec::new()));
        let git_calls = Arc::new(Mutex::new(Vec::new()));
        let cleanup_calls = Arc::new(Mutex::new(Vec::new()));
        let interactor = usecase::base_merge::BaseMergeInteractor::new(
            Arc::new(RecordingContext {
                direction: None,
                mismatch: Some((
                    TrackBranch::try_new("track/other").unwrap(),
                    TrackBranch::try_new("track/merge-track").unwrap(),
                )),
                workspaces: Arc::clone(&context_workspaces),
            }),
            Arc::new(RecordingGitPort { calls: Arc::clone(&git_calls) }),
            Arc::new(RecordingCleanupPort { calls: Arc::clone(&cleanup_calls) }),
        );
        let driver = base_merge_driver(Arc::new(interactor));

        let outcome = driver.handle_base_merge(BaseMergeInput {
            workspace_root: PathBuf::from("/workspace/guarded"),
        });

        assert_eq!(outcome.exit_code, 1);
        assert!(outcome.stderr.as_deref().unwrap().contains("does not match active track"));
        assert_eq!(context_workspaces.lock().unwrap().len(), 1);
        assert!(git_calls.lock().unwrap().is_empty());
        assert!(cleanup_calls.lock().unwrap().is_empty());
    }

    #[test]
    fn test_track_driver_handle_base_merge_real_interactor_rejects_snapshot_source_before_ports() {
        let workspace = PathBuf::from("/workspace/snapshot-source-guard");
        let context_workspaces = Arc::new(Mutex::new(Vec::new()));
        let git_calls = Arc::new(Mutex::new(Vec::new()));
        let cleanup_calls = Arc::new(Mutex::new(Vec::new()));
        let interactor = usecase::base_merge::BaseMergeInteractor::new(
            Arc::new(SnapshotSourceMismatchContext { workspaces: Arc::clone(&context_workspaces) }),
            Arc::new(RecordingGitPort { calls: Arc::clone(&git_calls) }),
            Arc::new(RecordingCleanupPort { calls: Arc::clone(&cleanup_calls) }),
        );
        let driver = base_merge_driver(Arc::new(interactor));

        let outcome =
            driver.handle_base_merge(BaseMergeInput { workspace_root: workspace.clone() });

        assert_eq!(outcome.exit_code, 1);
        assert!(
            outcome.stderr.as_deref().unwrap().contains("differs from snapshot base 'develop'")
        );
        assert_ne!(outcome.stdout.as_deref(), Some("base merge completed"));
        assert_eq!(*context_workspaces.lock().unwrap(), vec![workspace]);
        assert!(git_calls.lock().unwrap().is_empty());
        assert!(cleanup_calls.lock().unwrap().is_empty());
    }

    #[test]
    fn test_track_driver_handle_base_merge_renders_guard_failures_without_success() {
        let mismatch = usecase::base_merge::BaseMergeError::ActiveTrackMismatch {
            current: domain::TrackBranch::try_new("track/other").unwrap(),
            expected: domain::TrackBranch::try_new("track/active").unwrap(),
        };
        let mismatch_service = Arc::new(StubBaseMergeService::new(Err(mismatch)));
        let mismatch_outcome = base_merge_driver(mismatch_service)
            .handle_base_merge(BaseMergeInput { workspace_root: PathBuf::from("/workspace") });
        assert_eq!(mismatch_outcome.exit_code, 1);
        assert!(
            mismatch_outcome.stderr.as_deref().unwrap().contains("does not match active track")
        );
        assert_ne!(mismatch_outcome.stdout.as_deref(), Some("base merge completed"));

        let stale_service =
            Arc::new(StubBaseMergeService::new(Err(usecase::base_merge::BaseMergeError::Context(
                usecase::git_workflow::DiagnosticText::new("snapshot source is stale"),
            ))));
        let stale_outcome = base_merge_driver(stale_service)
            .handle_base_merge(BaseMergeInput { workspace_root: PathBuf::from("/workspace") });
        assert_eq!(stale_outcome.exit_code, 1);
        assert!(stale_outcome.stderr.as_deref().unwrap().contains("snapshot source is stale"));
        assert_ne!(stale_outcome.stdout.as_deref(), Some("base merge completed"));
    }

    #[test]
    fn test_track_driver_handle_base_merge_renders_conflict_recovery_handoff() {
        let service = Arc::new(StubBaseMergeService::new(Ok(BaseMergeOutcome::Conflicted)));

        let outcome = base_merge_driver(service)
            .handle_base_merge(BaseMergeInput { workspace_root: PathBuf::from("/workspace") });

        assert_eq!(outcome.exit_code, 1);
        assert_eq!(
            outcome.stderr.as_deref(),
            Some(
                "base merge conflicted; continue with the recover workflow (/track:recover on Claude Code, $track-recover on Codex)"
            )
        );
        assert_ne!(outcome.stdout.as_deref(), Some("base merge completed"));

        let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(std::path::Path::parent)
            .unwrap();
        let workflow =
            std::fs::read_to_string(workspace_root.join(".harness/workflows/track/recover.md"))
                .unwrap();
        let claude_adapter =
            std::fs::read_to_string(workspace_root.join(".claude/commands/track/recover.md"))
                .unwrap();
        let codex_adapter =
            std::fs::read_to_string(workspace_root.join(".agents/skills/track-recover/SKILL.md"))
                .unwrap();
        assert!(workflow.contains("Step 1: Confirm the recovery context"));
        assert!(workflow.contains("Step 2: Resolve the conflict"));
        assert!(workflow.contains("Step 3: Verify and review"));
        assert!(workflow.contains("Step 4: Guarded commit"));
        assert!(claude_adapter.contains("Operational SSoT: `.harness/workflows/track/recover.md`"));
        assert!(claude_adapter.contains("free of recovery sequence"));
        assert!(
            codex_adapter
                .contains("canonical recover workflow is `.harness/workflows/track/recover.md`")
        );
        assert!(codex_adapter.contains("must not duplicate its state machine"));
    }
}
