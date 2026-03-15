//! CLI subcommand for track operations using FsTrackStore.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use clap::{Args, Subcommand};
use domain::{
    CommitHash, TaskId, TaskStatusKind, TaskTransition, TrackBranch, TrackId, TrackReader,
    TrackWriter,
};
use infrastructure::git_cli::{
    GitRepository, SystemGitRepo, TrackBranchRecord, load_explicit_track_branch_from_items_dir,
};
use infrastructure::lock::FsFileLockManager;
use infrastructure::track::codec::{self, DocumentMeta};
use infrastructure::track::fs_store::FsTrackStore;
use infrastructure::track::render;
use usecase::track_activation::{ActivateTrackOutcome, ActivateTrackUseCase};

mod activate;
mod resolve;
mod transition;
mod views;

/// Default timeout for lock acquisition during track operations.
const DEFAULT_LOCK_TIMEOUT: Duration = Duration::from_millis(5000);

pub(super) fn resolve_project_root(items_dir: &std::path::Path) -> Result<PathBuf, String> {
    let items_name = items_dir.file_name().and_then(|name| name.to_str());
    let track_dir = items_dir.parent();
    let track_name = track_dir.and_then(std::path::Path::file_name).and_then(|name| name.to_str());
    let project_root = track_dir.and_then(std::path::Path::parent);

    match (items_name, track_name, project_root) {
        (Some("items"), Some("track"), Some(root)) => Ok(root.to_path_buf()),
        _ => Err(format!(
            "--items-dir must point to '<project-root>/track/items'; got {}",
            items_dir.display()
        )),
    }
}

#[derive(Debug, Subcommand)]
pub enum TrackCommand {
    /// Transition a task to a new status (atomic read-modify-write).
    Transition {
        /// Path to the track items root directory (e.g., `track/items`).
        #[arg(long)]
        items_dir: PathBuf,

        /// Locks directory for exclusive access.
        #[arg(long, default_value = ".locks")]
        locks_dir: PathBuf,

        /// Track ID (directory name under items_dir).
        track_id: String,

        /// Task ID (e.g., T1, T2).
        task_id: String,

        /// Target status: todo, in_progress, done, skipped.
        target_status: String,

        /// Commit hash (required when target_status is "done", optional).
        #[arg(long)]
        commit_hash: Option<String>,

        /// Skip branch validation (escape hatch for CI/testing).
        #[arg(long, default_value_t = false)]
        skip_branch_check: bool,
    },

    /// Create or switch to a track branch.
    Branch {
        #[command(subcommand)]
        action: BranchAction,
    },

    /// Materialize a planning-only track into its track branch and switch to it.
    Activate(ActivateArgs),

    /// Resolve the current track phase, next command, and blocker.
    Resolve(ResolveArgs),

    /// Validate track metadata and/or regenerate rendered views from metadata.json.
    Views {
        #[command(subcommand)]
        action: ViewAction,
    },
}

#[derive(Debug, Subcommand)]
pub enum BranchAction {
    /// Create a new branch `track/<track-id>` from `main` and switch to it.
    Create(BranchArgs),

    /// Switch to an existing branch `track/<track-id>`.
    Switch(BranchArgs),
}

#[derive(Debug, Args, Clone)]
pub struct BranchArgs {
    /// Path to the track items root directory (e.g., `track/items`).
    #[arg(long, default_value = "track/items")]
    items_dir: PathBuf,

    /// Locks directory for exclusive access.
    #[arg(long, default_value = ".locks")]
    locks_dir: PathBuf,

    /// Track ID used to form the branch name `track/<track-id>`.
    track_id: String,
}

#[derive(Debug, Args, Clone)]
pub struct ResolveArgs {
    /// Path to the track items root directory (e.g., `track/items`).
    #[arg(long, default_value = "track/items")]
    items_dir: PathBuf,

    /// Track ID. If omitted, auto-detects from the current git branch.
    track_id: Option<String>,
}

#[derive(Debug, Args, Clone)]
pub struct ActivateArgs {
    /// Path to the track items root directory (e.g., `track/items`).
    #[arg(long, default_value = "track/items")]
    items_dir: PathBuf,

    /// Locks directory for exclusive access.
    #[arg(long, default_value = ".locks")]
    locks_dir: PathBuf,

    /// Track ID used to form the branch name `track/<track-id>`.
    track_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BranchMode {
    Create,
    Switch,
    Auto,
}

#[derive(Debug, Subcommand)]
pub enum ViewAction {
    /// Validate metadata.json files under the repository.
    Validate {
        /// Project root containing `track/items` and `track/archive`.
        #[arg(long, default_value = ".")]
        project_root: PathBuf,
    },

    /// Render `plan.md` and `registry.md` from metadata.json.
    Sync {
        /// Project root containing `track/items` and `track/archive`.
        #[arg(long, default_value = ".")]
        project_root: PathBuf,

        /// Sync only one active track's `plan.md`.
        #[arg(long)]
        track_id: Option<String>,
    },
}

pub fn execute(cmd: TrackCommand) -> ExitCode {
    match cmd {
        TrackCommand::Transition {
            items_dir,
            locks_dir,
            track_id,
            task_id,
            target_status,
            commit_hash,
            skip_branch_check,
        } => transition::execute_transition(
            items_dir,
            locks_dir,
            track_id,
            task_id,
            target_status,
            commit_hash,
            skip_branch_check,
        ),
        TrackCommand::Branch { action } => activate::execute_branch(action),
        TrackCommand::Activate(args) => activate::execute_activate(args, BranchMode::Auto),
        TrackCommand::Resolve(args) => resolve::execute_resolve(args),
        TrackCommand::Views { action } => views::execute_views(action),
    }
}
