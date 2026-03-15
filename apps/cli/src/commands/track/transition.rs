use super::*;

pub(super) fn execute_transition(
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

    // Preserve items_dir for branch guard before moving into FsTrackStore.
    let repo_dir = items_dir.clone();
    let project_root = match resolve_project_root(&repo_dir) {
        Ok(path) => path,
        Err(err) => {
            eprintln!("{err}");
            return ExitCode::FAILURE;
        }
    };

    // Build FsTrackStore.
    let lock_manager = match FsFileLockManager::new(&locks_dir) {
        Ok(lm) => Arc::new(lm),
        Err(err) => {
            eprintln!("failed to initialize lock manager: {err}");
            return ExitCode::FAILURE;
        }
    };

    let store = Arc::new(FsTrackStore::new(items_dir.clone(), lock_manager, DEFAULT_LOCK_TIMEOUT));

    if let Err(msg) = reject_branchless_implementation_transition(
        &project_root,
        &items_dir,
        &track_id,
        &target_status,
    ) {
        eprintln!("activation guard: {msg}");
        return ExitCode::FAILURE;
    }

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

pub(super) fn reject_branchless_implementation_transition(
    project_root: &std::path::Path,
    items_dir: &std::path::Path,
    track_id: &TrackId,
    target_status: &str,
) -> Result<(), String> {
    if !matches!(target_status, "in_progress" | "done" | "skipped") {
        return Ok(());
    }

    let track = activate::load_track_branch_record(project_root, items_dir, track_id)?;
    if track.schema_version == 3 && track.branch.is_none() {
        return Err(format!(
            "track '{track_id}' is not activated yet; run /track:activate {track_id}"
        ));
    }

    Ok(())
}

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
