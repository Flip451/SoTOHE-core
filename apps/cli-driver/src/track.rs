// STAGED FOR T021 — not yet compiled; Cargo.toml + workspace member added atomically in T021 per CN-06.
//
//! `track` command family — primary adapter driver.
//!
//! `TrackDriver` holds injected use-case interactors and exposes
//! `handle(input) -> CommandOutcome`.  The render helpers here mirror those in
//! `apps/cli-composition/src/track/mod.rs` (lines 114-130 `sync_views_to_stdout`,
//! lines 225-230 transition format, lines 263/292/318-325/425-430/460-465/490-495/599
//! task-status / branch format lines) and `track/ops.rs`
//! (lines 177-204 JSON task payload, lines 226-231 JSON hand-built string);
//! T021 removes the `cli_composition` duplicates when the live path is flipped.

// TODO(T021): add use-case + infrastructure imports once Cargo.toml is materialized.
// use std::path::{Path, PathBuf};
// use std::sync::Arc;
// use infrastructure::git_cli::SystemGitRepo;
// use infrastructure::track::fs_store::FsTrackStore;
// use usecase::task_ops::{
//     AddTaskCommand, ClearOverrideCommand, SetOverrideCommand,
//     TaskOperationInteractor, TaskOperationService as _,
//     TaskQueryInteractor, TaskQueryService as _,
// };
// use usecase::track_phase::{TrackPhaseInteractor, TrackPhaseService as _};
// use usecase::track_resolution::{ActiveTrackResolveInteractor, BranchReaderPort};

use std::path::PathBuf;

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
}

// ---------------------------------------------------------------------------
// Driver
// ---------------------------------------------------------------------------

/// Primary adapter driver for the `track` command family.
///
/// Holds injected use-case interactors; exposes `handle(input) -> CommandOutcome`.
pub struct TrackDriver {
    // TODO(T021): inject use-case interactors here.
    // store: Arc<dyn usecase::track_store::TrackStore>,
    // branch_reader: Option<Arc<dyn BranchReaderPort>>,
}

impl TrackDriver {
    /// Create a new `TrackDriver`.
    ///
    /// TODO(T021): accept injected interactors as parameters once the crate
    /// dependency graph is materialized.
    pub fn new() -> Self {
        Self {}
    }

    /// Handle a track command.
    ///
    /// TODO(T021): wire real use-case invocation once Cargo.toml is materialized.
    pub fn handle(&self, input: TrackInput) -> CommandOutcome {
        match input {
            TrackInput::Init { items_dir, track_id, description } => {
                self.track_init(items_dir, track_id, description)
            }
            TrackInput::Transition { items_dir, track_id, task_id, target_status } => {
                self.track_transition(items_dir, track_id, task_id, target_status)
            }
            TrackInput::Resolve { items_dir, track_id } => self.track_resolve(items_dir, track_id),
            TrackInput::BranchCreate { items_dir, track_id } => {
                self.track_branch_create(items_dir, track_id)
            }
            TrackInput::BranchSwitch { items_dir, track_id } => {
                self.track_branch_switch(items_dir, track_id)
            }
            TrackInput::ViewsValidate { project_root } => self.track_views_validate(project_root),
            TrackInput::ViewsSync { project_root, track_id } => {
                self.track_views_sync(project_root, track_id)
            }
            TrackInput::AddTask { items_dir, track_id, description, section, after } => {
                self.track_add_task(items_dir, track_id, description, section, after)
            }
            TrackInput::SetOverride { items_dir, track_id, status, reason } => {
                self.track_set_override(items_dir, track_id, status, reason)
            }
            TrackInput::ClearOverride { items_dir, track_id } => {
                self.track_clear_override(items_dir, track_id)
            }
            TrackInput::NextTask { items_dir, track_id } => {
                self.track_next_task(items_dir, track_id)
            }
            TrackInput::TaskCounts { items_dir, track_id } => {
                self.track_task_counts(items_dir, track_id)
            }
            TrackInput::Archive { items_dir, track_id } => self.track_archive(items_dir, track_id),
            TrackInput::DetectActive { project_root } => {
                self.detect_active_track_from_branch(project_root)
            }
        }
    }

    // -----------------------------------------------------------------------
    // Render helpers (logic duplicated from cli_composition/src/track/mod.rs
    // and track/ops.rs; T021 removes the cli_composition copies).
    // -----------------------------------------------------------------------

    fn track_init(
        &self,
        _items_dir: PathBuf,
        _track_id: String,
        _description: String,
    ) -> CommandOutcome {
        // TODO(T021): invoke infrastructure::track::fs_store::FsTrackStore::init
        // and render the [OK] result here.
        // Mirrors cli_composition/src/track/mod.rs TrackCompositionRoot::track_init.
        CommandOutcome::success(None)
    }

    fn track_transition(
        &self,
        _items_dir: PathBuf,
        _track_id: Option<String>,
        _task_id: String,
        _target_status: String,
    ) -> CommandOutcome {
        // TODO(T021): resolve track_id, build FsTrackStore + TaskTransitionInteractor,
        // invoke transition_task, then format:
        //   format!("[OK] {}: transitioned to {} (track status: {})", task_id, target_status, output.derived_status)
        // followed by sync_views_to_stdout lines.
        // Mirrors cli_composition/src/track/mod.rs lines 225-230.
        CommandOutcome::success(None)
    }

    fn track_resolve(&self, _items_dir: PathBuf, _track_id: Option<String>) -> CommandOutcome {
        // TODO(T021): resolve effective_track_id, build FsTrackStore + TrackPhaseInteractor,
        // invoke resolve(), then format lines:
        //   format!("Current phase: {}", info.phase)
        //   format!("Reason: {}", info.reason)
        //   format!("Recommended next command: {}", info.next_command)
        // and optionally format!("Blocker: {blocker}") when present.
        // Mirrors cli_composition/src/track/mod.rs lines 318-325.
        CommandOutcome::success(None)
    }

    fn track_branch_create(&self, _items_dir: PathBuf, _track_id: String) -> CommandOutcome {
        // TODO(T021): validate track_id, discover git repo, verify branch does not exist,
        // run `git switch -c track/<track_id> main`, then format:
        //   format!("[OK] Created and switched to branch: {branch_name}")
        // Mirrors cli_composition/src/track/mod.rs lines 263-274.
        CommandOutcome::success(None)
    }

    fn track_branch_switch(&self, _items_dir: PathBuf, _track_id: String) -> CommandOutcome {
        // TODO(T021): validate track_id, discover git repo, verify branch exists,
        // run `git switch track/<track_id>`, then format:
        //   format!("[OK] Switched to branch: {branch_name}")
        // Mirrors cli_composition/src/track/mod.rs lines 292-306.
        CommandOutcome::success(None)
    }

    fn track_views_validate(&self, _project_root: PathBuf) -> CommandOutcome {
        // TODO(T021): invoke infrastructure::track::render::validate_track_snapshots,
        // then return CommandOutcome::success with "[OK] Track metadata is valid".
        // Mirrors cli_composition/src/track/mod.rs lines 342-349.
        CommandOutcome::success(None)
    }

    fn track_views_sync(
        &self,
        _project_root: PathBuf,
        _track_id: Option<String>,
    ) -> CommandOutcome {
        // TODO(T021): resolve track_id (WRITE guard for explicit id, auto-detect otherwise),
        // invoke infrastructure::track::render::sync_rendered_views, then format lines:
        //   "[OK] All views already up to date" (if empty)
        //   format!("[OK] Rendered: {}", rel.display()) (per changed path)
        // Mirrors cli_composition/src/track/mod.rs lines 380-396.
        CommandOutcome::success(None)
    }

    fn track_add_task(
        &self,
        _items_dir: PathBuf,
        _track_id: Option<String>,
        _description: String,
        _section: Option<String>,
        _after: Option<String>,
    ) -> CommandOutcome {
        // TODO(T021): resolve track_id for write, validate after_task_id (T<digits> pattern),
        // build FsTrackStore + TaskOperationInteractor, invoke add_task, then format:
        //   format!("[OK] Added task {new_task_id}: {description} (track status: {})", output.derived_status)
        // followed by sync_views_to_stdout lines.
        // Mirrors cli_composition/src/track/mod.rs lines 425-454.
        CommandOutcome::success(None)
    }

    fn track_set_override(
        &self,
        _items_dir: PathBuf,
        _track_id: Option<String>,
        _status: String,
        _reason: String,
    ) -> CommandOutcome {
        // TODO(T021): resolve track_id for write, build FsTrackStore + TaskOperationInteractor,
        // invoke set_override, then format:
        //   format!("[OK] Override set to '{}' (track status: {})", status, output.derived_status)
        // followed by sync_views_to_stdout lines.
        // Mirrors cli_composition/src/track/mod.rs lines 460-492.
        CommandOutcome::success(None)
    }

    fn track_clear_override(
        &self,
        _items_dir: PathBuf,
        _track_id: Option<String>,
    ) -> CommandOutcome {
        // TODO(T021): resolve track_id for write, build FsTrackStore + TaskOperationInteractor,
        // invoke clear_override, then format:
        //   format!("[OK] Override cleared (track status: {})", output.derived_status)
        // followed by sync_views_to_stdout lines.
        // Mirrors cli_composition/src/track/mod.rs lines 490-524.
        CommandOutcome::success(None)
    }

    fn track_next_task(&self, _items_dir: PathBuf, _track_id: Option<String>) -> CommandOutcome {
        // TODO(T021): resolve effective_track_id, build FsTrackStore + TaskQueryInteractor,
        // invoke next_task() + task_counts(), then build JSON payload:
        //   Some(task) → serde_json::json!({ "task_id": task.task_id, "description": task.description, "status": task_status })
        //   None       → serde_json::json!({ "task_id": null, "description": null, "status": null })
        // Mirrors cli_composition/src/track/ops.rs lines 177-204.
        CommandOutcome::success(None)
    }

    fn track_task_counts(&self, _items_dir: PathBuf, _track_id: Option<String>) -> CommandOutcome {
        // TODO(T021): resolve effective_track_id, build FsTrackStore + TaskQueryInteractor,
        // invoke task_counts(), then format JSON string:
        //   let total = counts.todo + counts.in_progress + counts.done + counts.skipped;
        //   format!(r#"{{"total":{total},"todo":{},"in_progress":{},"done":{},"skipped":{}}}"#,
        //       counts.todo, counts.in_progress, counts.done, counts.skipped)
        // Mirrors cli_composition/src/track/ops.rs lines 215-231.
        CommandOutcome::success(None)
    }

    fn track_archive(&self, _items_dir: PathBuf, _track_id: String) -> CommandOutcome {
        // TODO(T021): validate track_id, discover git repo, build src/dst paths,
        // create archive_root dir, run git mv, handle optional logs/ rename, then format:
        //   format!("[OK] Archived track '{track_id}': {} → {}", src_dir.display(), dst_dir.display())
        // Mirrors cli_composition/src/track/mod.rs lines 578-645.
        CommandOutcome::success(None)
    }

    fn detect_active_track_from_branch(&self, _project_root: PathBuf) -> CommandOutcome {
        // TODO(T021): discover git repo from project_root, detect active track
        // (only `track/<id>` branches resolve; others return None).
        // Mirrors cli_composition/src/track/ops.rs TrackCompositionRoot::detect_active_track_from_branch.
        CommandOutcome::success(None)
    }
}

impl Default for TrackDriver {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Render helpers (duplicated from cli_composition/src/track/mod.rs and ops.rs;
// T021 removes the cli_composition copies and moves these to cli_driver::render).
// ---------------------------------------------------------------------------

/// Sync views and return formatted status lines.
///
/// Mirrors `cli_composition::track::sync_views_to_stdout` (mod.rs lines 114-127).
///
/// TODO(T021): call `infrastructure::track::render::sync_rendered_views` directly
/// once the dependency graph is materialized.
#[allow(dead_code)]
fn sync_views_to_stdout(_project_root: &std::path::Path, _track_id: &str) -> Vec<String> {
    // TODO(T021): invoke infrastructure::track::render::sync_rendered_views(project_root, Some(track_id))
    // then map each changed path:
    //   match path.strip_prefix(project_root) {
    //       Ok(rel) => format!("[OK] Rendered: {}", rel.display()),
    //       Err(_)  => format!("[OK] Rendered: {}", path.display()),
    //   }
    // or return vec![format!("warning: operation persisted but sync-views failed: {err}")] on error.
    vec![]
}

/// Format a task status counts JSON string from raw counts.
///
/// Mirrors `cli_composition::track::ops::track_task_counts_resolved` JSON format
/// (ops.rs lines 226-230).
///
/// TODO(T021): invoke this from `track_task_counts` once counts are available from
/// the use-case layer.
#[allow(dead_code)]
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
