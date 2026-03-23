//! CLI subcommand for track operations using FsTrackStore.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use clap::{Args, Subcommand};
use domain::{CommitHash, TaskId, TrackBranch, TrackId, TrackReader};
use infrastructure::git_cli::{
    GitRepository, SystemGitRepo, TrackBranchRecord, load_explicit_track_branch_from_items_dir,
};
use infrastructure::track::codec::DocumentMeta;
use infrastructure::track::fs_store::FsTrackStore;
use infrastructure::track::render;
use usecase::track_activation::{ActivateTrackOutcome, ActivateTrackUseCase};

mod activate;
mod domain_state_signals;
mod resolve;
mod signals;
mod state_ops;
mod transition;
mod views;

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

    /// Add a new task to a track (atomic read-modify-write).
    AddTask {
        /// Path to the track items root directory (e.g., `track/items`).
        #[arg(long, default_value = "track/items")]
        items_dir: PathBuf,

        /// Track ID (directory name under items_dir).
        track_id: String,

        /// Task description.
        description: String,

        /// Target section ID. If omitted, appends to the first section.
        #[arg(long)]
        section: Option<String>,

        /// Insert after this task ID within the section. If omitted or not found, appends to end.
        #[arg(long)]
        after: Option<String>,

        /// Skip branch validation (escape hatch for CI/testing).
        #[arg(long, default_value_t = false)]
        skip_branch_check: bool,
    },

    /// Set a status override on a track (blocked/cancelled).
    SetOverride {
        /// Path to the track items root directory (e.g., `track/items`).
        #[arg(long, default_value = "track/items")]
        items_dir: PathBuf,

        /// Track ID (directory name under items_dir).
        track_id: String,

        /// Override status: blocked or cancelled.
        status: String,

        /// Reason for the override.
        #[arg(long, default_value = "")]
        reason: String,

        /// Skip branch validation.
        #[arg(long, default_value_t = false)]
        skip_branch_check: bool,
    },

    /// Clear a status override on a track.
    ClearOverride {
        /// Path to the track items root directory (e.g., `track/items`).
        #[arg(long, default_value = "track/items")]
        items_dir: PathBuf,

        /// Track ID (directory name under items_dir).
        track_id: String,

        /// Skip branch validation.
        #[arg(long, default_value_t = false)]
        skip_branch_check: bool,
    },

    /// Show the next open task for a track (JSON output).
    NextTask {
        /// Path to the track items root directory (e.g., `track/items`).
        #[arg(long, default_value = "track/items")]
        items_dir: PathBuf,

        /// Track ID (directory name under items_dir).
        track_id: String,
    },

    /// Show task status counts for a track (JSON output).
    TaskCounts {
        /// Path to the track items root directory (e.g., `track/items`).
        #[arg(long, default_value = "track/items")]
        items_dir: PathBuf,

        /// Track ID (directory name under items_dir).
        track_id: String,
    },

    /// Evaluate spec.md source tags and store results in metadata.json spec_signals.
    Signals {
        /// Path to the track items root directory (e.g., `track/items`).
        #[arg(long, default_value = "track/items")]
        items_dir: PathBuf,

        /// Track ID (directory name under items_dir).
        track_id: String,
    },

    /// Evaluate domain state signals by scanning domain code and store results in spec.json.
    DomainStateSignals {
        /// Path to the track items root directory (e.g., `track/items`).
        #[arg(long, default_value = "track/items")]
        items_dir: PathBuf,

        /// Track ID (directory name under items_dir).
        track_id: String,

        /// Path to the domain source directory to scan (relative to cwd).
        #[arg(long, default_value = "libs/domain/src")]
        domain_dir: PathBuf,
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
    use crate::CliError;

    let result: Result<ExitCode, CliError> = match cmd {
        TrackCommand::Transition {
            items_dir,
            track_id,
            task_id,
            target_status,
            commit_hash,
            skip_branch_check,
        } => transition::execute_transition(
            items_dir,
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
        TrackCommand::AddTask {
            items_dir,
            track_id,
            description,
            section,
            after,
            skip_branch_check,
        } => state_ops::execute_add_task(
            items_dir,
            track_id,
            description,
            section,
            after,
            skip_branch_check,
        ),
        TrackCommand::SetOverride { items_dir, track_id, status, reason, skip_branch_check } => {
            state_ops::execute_set_override(items_dir, track_id, status, reason, skip_branch_check)
        }
        TrackCommand::ClearOverride { items_dir, track_id, skip_branch_check } => {
            state_ops::execute_clear_override(items_dir, track_id, skip_branch_check)
        }
        TrackCommand::NextTask { items_dir, track_id } => {
            state_ops::execute_next_task(items_dir, track_id)
        }
        TrackCommand::TaskCounts { items_dir, track_id } => {
            state_ops::execute_task_counts(items_dir, track_id)
        }
        TrackCommand::Signals { items_dir, track_id } => {
            signals::execute_signals(items_dir, track_id)
        }
        TrackCommand::DomainStateSignals { items_dir, track_id, domain_dir } => {
            domain_state_signals::execute_domain_state_signals(items_dir, track_id, domain_dir)
        }
    };
    match result {
        Ok(code) => code,
        Err(err) => {
            eprintln!("{err}");
            err.exit_code()
        }
    }
}
