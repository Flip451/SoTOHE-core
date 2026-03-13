//! CLI subcommand for track operations using FsTrackStore.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use clap::Subcommand;
use domain::{
    CommitHash, TaskId, TaskStatusKind, TaskTransition, TrackId, TrackReader, TrackWriter,
};
use infrastructure::lock::FsFileLockManager;
use infrastructure::track::fs_store::FsTrackStore;
use infrastructure::track::render;

/// Default timeout for lock acquisition during track operations.
const DEFAULT_LOCK_TIMEOUT: Duration = Duration::from_millis(5000);

fn resolve_project_root(items_dir: &std::path::Path) -> Result<PathBuf, String> {
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

    /// Validate track metadata and/or regenerate rendered views from metadata.json.
    Views {
        #[command(subcommand)]
        action: ViewAction,
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
        } => execute_transition(
            items_dir,
            locks_dir,
            track_id,
            task_id,
            target_status,
            commit_hash,
            skip_branch_check,
        ),
        TrackCommand::Branch { action } => execute_branch(action),
        TrackCommand::Views { action } => execute_views(action),
    }
}

fn execute_views(action: ViewAction) -> ExitCode {
    match action {
        ViewAction::Validate { project_root } => {
            match render::validate_track_snapshots(&project_root) {
                Ok(()) => {
                    println!("[OK] Track metadata is valid");
                    ExitCode::SUCCESS
                }
                Err(err) => {
                    eprintln!("track metadata validation failed: {err}");
                    ExitCode::FAILURE
                }
            }
        }
        ViewAction::Sync { project_root, track_id } => {
            match render::sync_rendered_views(&project_root, track_id.as_deref()) {
                Ok(changed) => {
                    if changed.is_empty() {
                        println!("[OK] All views already up to date");
                    } else {
                        for path in changed {
                            match path.strip_prefix(&project_root) {
                                Ok(relative) => println!("[OK] Rendered: {}", relative.display()),
                                Err(_) => println!("[OK] Rendered: {}", path.display()),
                            }
                        }
                    }
                    ExitCode::SUCCESS
                }
                Err(err) => {
                    eprintln!("sync-views failed: {err}");
                    ExitCode::FAILURE
                }
            }
        }
    }
}

fn execute_transition(
    items_dir: PathBuf,
    locks_dir: PathBuf,
    track_id: String,
    task_id: String,
    target_status: String,
    commit_hash: Option<String>,
    skip_branch_check: bool,
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

    // Preserve items_dir for branch guard before moving into FsTrackStore.
    let repo_dir = items_dir.clone();
    let store = Arc::new(FsTrackStore::new(items_dir, lock_manager, DEFAULT_LOCK_TIMEOUT));

    // Branch guard: reject if current git branch does not match metadata.json branch.
    if !skip_branch_check {
        if let Err(msg) = verify_branch_guard(&*store, &track_id, &repo_dir) {
            eprintln!("branch guard: {msg}");
            return ExitCode::FAILURE;
        }
    }

    // Validate target_status before entering the locked update section.
    if !["todo", "in_progress", "done", "skipped"].contains(&target_status.as_str()) {
        eprintln!("unsupported target status: {target_status}");
        return ExitCode::FAILURE;
    }

    let project_root = match resolve_project_root(&repo_dir) {
        Ok(path) => path,
        Err(err) => {
            eprintln!("{err}");
            return ExitCode::FAILURE;
        }
    };
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
            match render::sync_rendered_views(&project_root, Some(track_id.as_str())) {
                Ok(changed) => {
                    for path in changed {
                        match path.strip_prefix(&project_root) {
                            Ok(relative) => println!("[OK] Rendered: {}", relative.display()),
                            Err(_) => println!("[OK] Rendered: {}", path.display()),
                        }
                    }
                    ExitCode::SUCCESS
                }
                Err(err) => {
                    eprintln!("warning: transition persisted but sync-views failed: {err}");
                    ExitCode::SUCCESS
                }
            }
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

/// Returns the current git branch name, or `"HEAD"` for detached HEAD.
///
/// Resolves the branch relative to the given directory so that the command
/// works even when `sotp` is launched from outside the repository.
///
/// # Errors
/// Returns an error message if `git` cannot be executed or fails.
fn current_git_branch(cwd: &std::path::Path) -> Result<String, String> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(cwd)
        .output()
        .map_err(|e| format!("failed to run git: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git rev-parse failed: {stderr}"));
    }

    let branch = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    Ok(branch)
}

/// Verifies that the current git branch matches the track's expected branch.
///
/// Skip policy:
/// - branch=None in metadata → skip (legacy/planning phase)
/// - detached HEAD (`"HEAD"`) → reject (ambiguous state)
/// - mismatch → reject
///
/// # Errors
/// Returns an error message describing the branch mismatch or detection failure.
fn verify_branch_guard<R: TrackReader>(
    reader: &R,
    track_id: &TrackId,
    repo_dir: &std::path::Path,
) -> Result<(), String> {
    let track = reader
        .find(track_id)
        .map_err(|e| format!("failed to read track: {e}"))?
        .ok_or_else(|| format!("track '{track_id}' not found"))?;

    let expected_branch = match track.branch() {
        Some(branch) => branch,
        None => return Ok(()), // branch=null → skip guard
    };

    let actual = current_git_branch(repo_dir)?;

    // Detached HEAD → reject (ambiguous state).
    if actual == "HEAD" {
        return Err(format!("detached HEAD — expected branch '{expected_branch}', cannot verify"));
    }

    if actual != expected_branch.as_str() {
        return Err(format!(
            "current branch '{actual}' does not match expected '{expected_branch}'"
        ));
    }

    Ok(())
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

#[cfg(test)]
mod tests {
    use super::resolve_project_root;
    use std::path::Path;

    #[test]
    fn resolve_project_root_accepts_standard_track_items_layout() {
        assert_eq!(
            resolve_project_root(Path::new("repo/track/items")),
            Ok(std::path::PathBuf::from("repo"))
        );
    }

    #[test]
    fn resolve_project_root_rejects_non_standard_layout() {
        assert!(matches!(
            resolve_project_root(Path::new("repo/custom-items")),
            Err(err) if err.contains("track/items")
        ));
    }
}
