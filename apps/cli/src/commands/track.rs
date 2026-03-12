//! CLI subcommand for track operations using FsTrackStore.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use clap::Subcommand;
use domain::{CommitHash, TaskId, TaskStatusKind, TaskTransition, TrackId, TrackWriter};
use infrastructure::lock::FsFileLockManager;
use infrastructure::track::fs_store::FsTrackStore;

/// Default timeout for lock acquisition during track operations.
const DEFAULT_LOCK_TIMEOUT: Duration = Duration::from_millis(5000);

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
    },

    /// Create or switch to a track branch.
    Branch {
        #[command(subcommand)]
        action: BranchAction,
    },
}

#[derive(Debug, Subcommand)]
pub enum BranchAction {
    /// Create a new branch `track/<track-id>` from `main` and switch to it.
    Create {
        /// Track ID used to form the branch name `track/<track-id>`.
        track_id: String,
    },

    /// Switch to an existing branch `track/<track-id>`.
    Switch {
        /// Track ID used to form the branch name `track/<track-id>`.
        track_id: String,
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
        } => {
            execute_transition(items_dir, locks_dir, track_id, task_id, target_status, commit_hash)
        }
        TrackCommand::Branch { action } => execute_branch(action),
    }
}

fn execute_transition(
    items_dir: PathBuf,
    locks_dir: PathBuf,
    track_id: String,
    task_id: String,
    target_status: String,
    commit_hash: Option<String>,
) -> ExitCode {
    // Validate inputs.
    let track_id = match TrackId::new(&track_id) {
        Ok(id) => id,
        Err(err) => {
            eprintln!("invalid track id: {err}");
            return ExitCode::FAILURE;
        }
    };

    let task_id = match TaskId::new(&task_id) {
        Ok(id) => id,
        Err(err) => {
            eprintln!("invalid task id: {err}");
            return ExitCode::FAILURE;
        }
    };

    // Validate commit_hash early if provided.
    let parsed_hash = match commit_hash {
        Some(h) => match CommitHash::new(h) {
            Ok(hash) => Some(hash),
            Err(err) => {
                eprintln!("invalid commit hash: {err}");
                return ExitCode::FAILURE;
            }
        },
        None => None,
    };

    // Build FsTrackStore.
    let lock_manager = match FsFileLockManager::new(&locks_dir) {
        Ok(lm) => Arc::new(lm),
        Err(err) => {
            eprintln!("failed to initialize lock manager: {err}");
            return ExitCode::FAILURE;
        }
    };

    let store = Arc::new(FsTrackStore::new(items_dir, lock_manager, DEFAULT_LOCK_TIMEOUT));

    // Validate target_status before entering the locked update section.
    if !["todo", "in_progress", "done", "skipped"].contains(&target_status.as_str()) {
        eprintln!("unsupported target status: {target_status}");
        return ExitCode::FAILURE;
    }

    // Use TrackWriter::update directly to resolve the correct transition
    // based on current task status (e.g., "in_progress" from "done" is Reopen, not Start).
    match store.update(&track_id, |track| {
        let task = track.tasks().iter().find(|t| *t.id() == task_id).ok_or_else(|| {
            domain::TransitionError::TaskNotFound { task_id: task_id.to_string() }
        })?;
        let current_kind = task.status().kind();

        // target_status was validated above, so this branch is unreachable in practice.
        let transition = match resolve_transition(&target_status, current_kind, parsed_hash) {
            Ok(t) => t,
            Err(msg) => {
                return Err(domain::ValidationError::InvalidTrackId(msg).into());
            }
        };

        track.transition_task(&task_id, transition)?;
        Ok(())
    }) {
        Ok(track) => {
            println!(
                "[OK] {}: transitioned to {} (track status: {})",
                task_id,
                target_status,
                track.status()
            );
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("transition failed: {err}");
            ExitCode::FAILURE
        }
    }
}

fn execute_branch(action: BranchAction) -> ExitCode {
    let (track_id, create) = match &action {
        BranchAction::Create { track_id } => (track_id.as_str(), true),
        BranchAction::Switch { track_id } => (track_id.as_str(), false),
    };

    if let Err(err) = TrackId::new(track_id) {
        eprintln!("invalid track id: {err}");
        return ExitCode::FAILURE;
    }

    let branch_name = format!("track/{track_id}");

    let mut cmd = std::process::Command::new("git");
    if create {
        cmd.args(["switch", "-c", &branch_name, "main"]);
    } else {
        cmd.args(["switch", &branch_name]);
    }

    match cmd.status() {
        Ok(status) if status.success() => {
            if create {
                println!("[OK] Created and switched to branch: {branch_name}");
            } else {
                println!("[OK] Switched to branch: {branch_name}");
            }
            ExitCode::SUCCESS
        }
        Ok(_) => {
            eprintln!("git switch failed for branch: {branch_name}");
            ExitCode::FAILURE
        }
        Err(err) => {
            eprintln!("failed to run git: {err}");
            ExitCode::FAILURE
        }
    }
}

/// Resolves the correct `TaskTransition` based on target status and current task status.
/// This handles cases like `done -> in_progress` (Reopen) vs `todo -> in_progress` (Start).
fn resolve_transition(
    target_status: &str,
    current_kind: TaskStatusKind,
    commit_hash: Option<CommitHash>,
) -> Result<TaskTransition, String> {
    match target_status {
        "in_progress" => match current_kind {
            TaskStatusKind::Done => Ok(TaskTransition::Reopen),
            _ => Ok(TaskTransition::Start),
        },
        "done" => Ok(TaskTransition::Complete { commit_hash }),
        "todo" => Ok(TaskTransition::ResetToTodo),
        "skipped" => Ok(TaskTransition::Skip),
        other => Err(format!("unsupported target status: {other}")),
    }
}
