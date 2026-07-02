//! `track` command family — primary adapter driver.
//!
//! `TrackDriver` holds an injected [`usecase::track_service::TrackService`]
//! and exposes `handle(input) -> CommandOutcome`.

use std::path::PathBuf;
use std::sync::Arc;

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
    /// Create a new track branch from `main`.
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

// ---------------------------------------------------------------------------
// Conversion helpers
// ---------------------------------------------------------------------------

fn service_output_to_outcome(output: TrackCommandOutput) -> CommandOutcome {
    CommandOutcome { stdout: output.stdout, stderr: output.stderr, exit_code: output.exit_code }
}

// ---------------------------------------------------------------------------
// Driver
// ---------------------------------------------------------------------------

/// Primary adapter driver for the `track` command family.
///
/// Holds an injected [`TrackService`]; exposes `handle(input) -> CommandOutcome`.
pub struct TrackDriver {
    service: Arc<dyn TrackService>,
}

impl TrackDriver {
    /// Create a new `TrackDriver` with the given service.
    pub fn new(service: Arc<dyn TrackService>) -> Self {
        Self { service }
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
