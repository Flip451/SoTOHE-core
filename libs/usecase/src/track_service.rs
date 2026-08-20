//! `TrackService` — unified application-service facade for all `track`
//! subcommands.
//!
//! Defines the primary port trait [`TrackService`] and the shared output DTO
//! [`TrackCommandOutput`] that the `cli_driver::track::TrackDriver` consumes.
//! The composition root (`apps/cli-composition`) implements the trait by wiring
//! the appropriate infrastructure adapters and usecase interactors for each
//! subcommand.
//!
//! # Design rationale
//!
//! The `track` family has many subcommands that each delegate to different
//! lower-level usecase services (`TaskOperationService`, `TaskQueryService`,
//! `TrackPhaseService`, etc.) and require infrastructure setup (git discovery,
//! `FsTrackStore`, branch reader, render views).  A single wide service trait
//! lets the `TrackDriver` stay a simple dispatcher with one `Arc<dyn TrackService>`
//! dependency, while the composition root retains full wiring control.
//!
//! The output type [`TrackCommandOutput`] mirrors `cli_driver::CommandOutcome`
//! field-for-field so the driver can convert it in one expression, without
//! `usecase` needing to import `cli_driver`.

use std::path::PathBuf;

use domain::{ImplPlanReader, ImplPlanWriter, TrackReader, TrackWriter};

pub use crate::track_lifecycle::{
    ProcessExitCode, RenderedViewPath, TaskCount, TrackBranchStrategyPort,
    TrackCatalogueEntryCount, TrackCatalogueImplLayerResult, TrackCataloguePath,
    TrackCommitHashPort, TrackDirectoryPath, TrackItemsDirectory, TrackLayerFilter,
    TrackLayerSelection, TrackLayerSignalResult, TrackLifecycleIdInput, TrackMetadataPort,
    TrackNextTaskQueryPort, TrackOverrideClearPort, TrackOverrideSetPort, TrackSelection,
    TrackSelectionPort, TrackSourceWorkspace, TrackSpecAnchorSelection, TrackTaskAddPort,
    TrackTaskCountsQueryPort, TrackTaskTransition, TrackTaskTransitionPort, TrackViewSyncOutcome,
    TrackViewsPort, TrackViewsScope, TrackWorkspaceRoot, TrackWrittenFileCount,
};

use crate::task_ops::{
    TaskOperationInteractor, TaskOperationService, TaskQueryInteractor, TaskQueryService,
};

// ── Output DTO ────────────────────────────────────────────────────────────────

/// Unified output DTO for all `track` subcommands.
///
/// Mirrors `cli_driver::render::CommandOutcome` field-for-field.  Defined here
/// (in the usecase layer) so that the `TrackService` trait does not import
/// `cli_driver`, preserving hexagonal layer order.
///
/// `cli_driver::track` converts this to `CommandOutcome` in one expression.
#[derive(Debug, Clone)]
pub struct TrackCommandOutput {
    /// Optional text written to stdout.
    pub stdout: Option<String>,
    /// Optional text written to stderr.
    pub stderr: Option<String>,
    /// Process exit code (0 = success, non-zero = failure).
    pub exit_code: u8,
}

impl TrackCommandOutput {
    /// Construct a successful output with optional stdout text.
    pub fn success(stdout: Option<String>) -> Self {
        Self { stdout, stderr: None, exit_code: 0 }
    }

    /// Construct a failure output with optional stderr text.
    pub fn failure(stderr: Option<String>) -> Self {
        Self { stdout: None, stderr, exit_code: 1 }
    }
}

// ── Primary port ──────────────────────────────────────────────────────────────

/// Primary port for the `track` command family.
///
/// Each method corresponds to one `sotp track <subcommand>` invocation.
/// Return value is [`TrackCommandOutput`]; the driver converts it to
/// `CommandOutcome`.
pub trait TrackService: Send + Sync {
    /// `track init` — initialize a new track by writing `metadata.json`.
    fn init(&self, items_dir: PathBuf, track_id: String, description: String)
    -> TrackCommandOutput;

    /// `track transition` — transition a task to a new status.
    fn transition(
        &self,
        items_dir: PathBuf,
        track_id: Option<String>,
        task_id: String,
        target_status: String,
        commit_hash: Option<String>,
    ) -> TrackCommandOutput;

    /// `track resolve` — resolve current track phase, next command, and blocker.
    fn resolve(&self, items_dir: PathBuf, track_id: Option<String>) -> TrackCommandOutput;

    /// `track branch create` — create a new track branch from `main`.
    fn branch_create(&self, items_dir: PathBuf, track_id: String) -> TrackCommandOutput;

    /// `track branch switch` — switch to an existing track branch.
    fn branch_switch(&self, items_dir: PathBuf, track_id: String) -> TrackCommandOutput;

    /// `track views validate` — validate `metadata.json` files.
    fn views_validate(&self, project_root: PathBuf) -> TrackCommandOutput;

    /// `track views sync` — render `plan.md` and `registry.md` from metadata.
    fn views_sync(&self, project_root: PathBuf, track_id: Option<String>) -> TrackCommandOutput;

    /// `track add-task` — add a new task to a track.
    fn add_task(
        &self,
        items_dir: PathBuf,
        track_id: Option<String>,
        description: String,
        section: Option<String>,
        after: Option<String>,
    ) -> TrackCommandOutput;

    /// `track set-override` — set a status override on a track.
    fn set_override(
        &self,
        items_dir: PathBuf,
        track_id: Option<String>,
        status: String,
        reason: String,
    ) -> TrackCommandOutput;

    /// `track clear-override` — clear a status override on a track.
    fn clear_override(&self, items_dir: PathBuf, track_id: Option<String>) -> TrackCommandOutput;

    /// `track next-task` — show the next open task (JSON output).
    fn next_task(&self, items_dir: PathBuf, track_id: Option<String>) -> TrackCommandOutput;

    /// `track task-counts` — show task status counts (JSON output).
    fn task_counts(&self, items_dir: PathBuf, track_id: Option<String>) -> TrackCommandOutput;

    /// `track archive` — archive a completed track.
    fn archive(&self, items_dir: PathBuf, track_id: String) -> TrackCommandOutput;

    /// `track detect-active` — detect the active track ID from the current git branch.
    fn detect_active(&self, project_root: PathBuf) -> TrackCommandOutput;

    /// `track switch-base` — switch to the base branch from the active track's
    /// `branch_strategy_snapshot`.
    ///
    /// Default implementation returns a failure; overridden by the composition root
    /// after `BranchStrategyPort` is wired (wiring happens in T009/T011).
    fn switch_base(&self, _project_root: PathBuf) -> TrackCommandOutput {
        TrackCommandOutput::failure(Some(
            "switch_base is not yet implemented in this composition root".to_string(),
        ))
    }

    /// `catalogue-lint check-active-track` — run the catalogue lint ruleset
    /// across every `tddd.enabled` layer of the active track and aggregate
    /// violations.
    fn catalogue_lint_check_active_track(
        &self,
        track_id: Option<String>,
        workspace_root: PathBuf,
        rules_file: Option<PathBuf>,
    ) -> TrackCommandOutput;
}

impl<S> TrackTaskTransitionPort for TaskOperationInteractor<S>
where
    S: TrackReader + TrackWriter + ImplPlanReader + ImplPlanWriter + Send + Sync,
{
    fn transition_task(
        &self,
        track_id: domain::TrackId,
        items_dir: crate::track_lifecycle::TrackItemsDirectory,
        task_id: domain::TaskId,
        transition: crate::track_lifecycle::TrackTaskTransition,
    ) -> Result<crate::task_ops::TaskTransitionOutcome, crate::task_ops::TaskOperationError> {
        let (target_status, commit_hash) = match transition {
            crate::track_lifecycle::TrackTaskTransition::Todo => ("todo".to_owned(), None),
            crate::track_lifecycle::TrackTaskTransition::InProgress => {
                ("in_progress".to_owned(), None)
            }
            crate::track_lifecycle::TrackTaskTransition::Done { commit_hash } => {
                ("done".to_owned(), commit_hash.map(|hash| hash.to_string()))
            }
            crate::track_lifecycle::TrackTaskTransition::Skipped => ("skipped".to_owned(), None),
        };
        <Self as TaskOperationService>::transition_task(
            self,
            crate::task_ops::TaskTransitionCommand {
                items_dir: items_dir.as_path().to_path_buf(),
                track_id: track_id.as_ref().to_owned(),
                task_id: task_id.as_ref().to_owned(),
                target_status,
                commit_hash,
            },
        )
    }
}

impl<S> TrackTaskAddPort for TaskOperationInteractor<S>
where
    S: TrackReader + TrackWriter + ImplPlanReader + ImplPlanWriter + Send + Sync,
{
    fn add_task(
        &self,
        track_id: domain::TrackId,
        items_dir: crate::track_lifecycle::TrackItemsDirectory,
        description: domain::NonEmptyString,
        section: Option<domain::NonEmptyString>,
        after: Option<domain::TaskId>,
    ) -> Result<crate::task_ops::TaskOperationOutput, crate::task_ops::TaskOperationError> {
        <Self as TaskOperationService>::add_task(
            self,
            crate::task_ops::AddTaskCommand {
                items_dir: items_dir.as_path().to_path_buf(),
                track_id: track_id.as_ref().to_owned(),
                description: description.as_ref().to_owned(),
                section: section.map(|value| value.as_ref().to_owned()),
                after_task_id: after.map(|value| value.as_ref().to_owned()),
            },
        )
    }
}

impl<S> TrackOverrideSetPort for TaskOperationInteractor<S>
where
    S: TrackReader + TrackWriter + ImplPlanReader + ImplPlanWriter + Send + Sync,
{
    fn set_override(
        &self,
        track_id: domain::TrackId,
        items_dir: crate::track_lifecycle::TrackItemsDirectory,
        status: domain::StatusOverrideKind,
        reason: crate::git_workflow::DiagnosticText,
    ) -> Result<crate::task_ops::TaskOperationOutput, crate::task_ops::TaskOperationError> {
        <Self as TaskOperationService>::set_override(
            self,
            crate::task_ops::SetOverrideCommand {
                items_dir: items_dir.as_path().to_path_buf(),
                track_id: track_id.as_ref().to_owned(),
                status: status.to_string(),
                reason: reason.as_str().to_owned(),
            },
        )
    }
}

impl<S> TrackOverrideClearPort for TaskOperationInteractor<S>
where
    S: TrackReader + TrackWriter + ImplPlanReader + ImplPlanWriter + Send + Sync,
{
    fn clear_override(
        &self,
        track_id: domain::TrackId,
        items_dir: crate::track_lifecycle::TrackItemsDirectory,
    ) -> Result<crate::task_ops::TaskOperationOutput, crate::task_ops::TaskOperationError> {
        <Self as TaskOperationService>::clear_override(
            self,
            crate::task_ops::ClearOverrideCommand {
                items_dir: items_dir.as_path().to_path_buf(),
                track_id: track_id.as_ref().to_owned(),
            },
        )
    }
}

impl<S> TrackNextTaskQueryPort for TaskQueryInteractor<S>
where
    S: TrackReader + ImplPlanReader + Send + Sync,
{
    fn next_task(
        &self,
        track_id: domain::TrackId,
        items_dir: crate::track_lifecycle::TrackItemsDirectory,
    ) -> Result<
        Option<crate::task_ops::NextTaskOutput>,
        crate::track_lifecycle::track_next_task::TrackNextTaskError,
    > {
        <Self as TaskQueryService>::next_task(
            self,
            track_id.as_ref().to_owned(),
            items_dir.as_path().to_path_buf(),
        )
        .map_err(|error| {
            crate::track_lifecycle::track_next_task::TrackNextTaskError::ExecutionFailed(
                crate::git_workflow::DiagnosticText::new(error.to_string()),
            )
        })
    }
}

impl<S> TrackTaskCountsQueryPort for TaskQueryInteractor<S>
where
    S: TrackReader + ImplPlanReader + Send + Sync,
{
    fn task_counts(
        &self,
        track_id: domain::TrackId,
        items_dir: crate::track_lifecycle::TrackItemsDirectory,
    ) -> Result<
        crate::task_ops::TaskCountsOutput,
        crate::track_lifecycle::track_task_counts::TrackTaskCountsError,
    > {
        <Self as TaskQueryService>::task_counts(
            self,
            track_id.as_ref().to_owned(),
            items_dir.as_path().to_path_buf(),
        )
        .map_err(|error| {
            crate::track_lifecycle::track_task_counts::TrackTaskCountsError::ExecutionFailed(
                crate::git_workflow::DiagnosticText::new(error.to_string()),
            )
        })
    }
}
