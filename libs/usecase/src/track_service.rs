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
}
