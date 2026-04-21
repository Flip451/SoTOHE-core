use crate::CliError;

use super::*;

pub(super) fn execute_transition(
    items_dir: PathBuf,
    track_id: String,
    task_id: String,
    target_status: String,
    commit_hash: Option<String>,
    skip_branch_check: bool,
) -> Result<ExitCode, CliError> {
    // Validate inputs.
    let track_id = TrackId::try_new(&track_id)
        .map_err(|err| CliError::Message(format!("invalid track id: {err}")))?;

    let task_id = TaskId::try_new(&task_id)
        .map_err(|err| CliError::Message(format!("invalid task id: {err}")))?;

    // Validate commit_hash early if provided.
    let parsed_hash = match commit_hash {
        Some(h) => {
            let hash = CommitHash::try_new(h)
                .map_err(|err| CliError::Message(format!("invalid commit hash: {err}")))?;
            Some(hash)
        }
        None => None,
    };

    // Preserve items_dir for branch guard before moving into FsTrackStore.
    let repo_dir = items_dir.clone();
    let project_root = resolve_project_root(&repo_dir).map_err(CliError::Message)?;

    // Build FsTrackStore.
    let store = Arc::new(FsTrackStore::new(items_dir.clone()));

    // Activation guard: read schema_version from DocumentMeta, then delegate to usecase.
    let schema_version = match store.find_with_meta(&track_id)? {
        Some((_, meta)) => meta.schema_version,
        None => 2, // fallback for missing tracks; store.update will handle the error
    };
    usecase::track_resolution::reject_branchless_guard(
        &*store,
        &track_id,
        &target_status,
        schema_version,
    )
    .map_err(|msg| CliError::Message(format!("activation guard: {msg}")))?;

    // Branch guard: reject if current git branch does not match metadata.json branch.
    if !skip_branch_check {
        verify_branch_guard(&*store, &track_id, &repo_dir)
            .map_err(|msg| CliError::Message(format!("branch guard: {msg}")))?;
    }

    // Delegate task-lookup → resolve-transition → transition-task to usecase.
    // T005: `TransitionTaskUseCase::execute_by_status` persists `impl-plan.json` and
    // syncs `metadata.json` status in one atomic sequence, returning the updated metadata.
    let transition = usecase::TransitionTaskUseCase::new(Arc::clone(&store));
    let track = transition
        .execute_by_status(&track_id, &task_id, &target_status, parsed_hash)
        .map_err(|err| CliError::Message(format!("transition failed: {err}")))?;

    println!(
        "[OK] {}: transitioned to {} (track status: {})",
        task_id,
        target_status,
        track.status()
    );
    match render::sync_rendered_views(&project_root, Some(track_id.as_ref())) {
        Ok(changed) => {
            for path in changed {
                match path.strip_prefix(&project_root) {
                    Ok(relative) => println!("[OK] Rendered: {}", relative.display()),
                    Err(_) => println!("[OK] Rendered: {}", path.display()),
                }
            }
        }
        Err(err) => {
            eprintln!("warning: transition persisted but sync-views failed: {err}");
        }
    }
    Ok(ExitCode::SUCCESS)
}

// Note: reject_branchless_implementation_transition has been replaced by
// usecase::track_resolution::reject_branchless_guard which reads branch
// state autonomously through the TrackReader port. The CLI now calls it
// directly in execute_transition with schema_version from DocumentMeta.

pub(super) fn verify_branch_guard<R: TrackReader>(
    reader: &R,
    track_id: &TrackId,
    repo_dir: &std::path::Path,
) -> Result<(), String> {
    // Read metadata first: branchless tracks skip the guard without touching git.
    let track = reader
        .find(track_id)
        .map_err(|e| format!("failed to read track: {e}"))?
        .ok_or_else(|| format!("track '{track_id}' not found"))?;
    if track.branch().is_none() {
        return Ok(());
    }
    let actual = current_git_branch(repo_dir)?;
    verify_branch_guard_with_branch(reader, track_id, &actual)
}

pub(super) fn verify_branch_guard_with_branch<R: TrackReader>(
    reader: &R,
    track_id: &TrackId,
    current_branch: &str,
) -> Result<(), String> {
    let track = reader
        .find(track_id)
        .map_err(|e| format!("failed to read track: {e}"))?
        .ok_or_else(|| format!("track '{track_id}' not found"))?;

    let expected_branch = match track.branch() {
        Some(branch) => branch,
        None => return Ok(()), // branch=null → skip guard
    };

    // Detached HEAD → reject (ambiguous state).
    if current_branch == "HEAD" {
        return Err(format!("detached HEAD — expected branch '{expected_branch}', cannot verify"));
    }

    if current_branch != expected_branch.as_ref() {
        return Err(format!(
            "current branch '{current_branch}' does not match expected '{expected_branch}'"
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Mutex;

    use domain::{RepositoryError, TrackBranch, TrackMetadata, TrackReadError, TrackStatus};

    use super::*;

    struct StubReader {
        tracks: Mutex<HashMap<TrackId, TrackMetadata>>,
    }

    impl StubReader {
        fn new() -> Self {
            Self { tracks: Mutex::new(HashMap::new()) }
        }

        fn insert(&self, track: TrackMetadata) {
            self.tracks.lock().unwrap().insert(track.id().clone(), track);
        }
    }

    impl TrackReader for StubReader {
        fn find(&self, id: &TrackId) -> Result<Option<TrackMetadata>, TrackReadError> {
            Ok(self.tracks.lock().unwrap().get(id).cloned())
        }
    }

    struct FailingReader;

    impl TrackReader for FailingReader {
        fn find(&self, _id: &TrackId) -> Result<Option<TrackMetadata>, TrackReadError> {
            Err(RepositoryError::Message("corrupt metadata".to_owned()).into())
        }
    }

    fn sample_track(id: &str, branch: Option<&str>) -> TrackMetadata {
        // T005: TrackMetadata is identity-only; no tasks/plan fields.
        TrackMetadata::with_branch(
            TrackId::try_new(id).unwrap(),
            branch.map(|b| TrackBranch::try_new(b).unwrap()),
            "Test Track",
            TrackStatus::Planned,
            None,
        )
        .unwrap()
    }

    #[test]
    fn verify_branch_guard_with_branch_matching_branch_passes() {
        let reader = StubReader::new();
        reader.insert(sample_track("test", Some("track/test")));
        let id = TrackId::try_new("test").unwrap();
        assert!(verify_branch_guard_with_branch(&reader, &id, "track/test").is_ok());
    }

    #[test]
    fn verify_branch_guard_with_branch_mismatched_branch_raises() {
        let reader = StubReader::new();
        reader.insert(sample_track("test", Some("track/test")));
        let id = TrackId::try_new("test").unwrap();
        let err = verify_branch_guard_with_branch(&reader, &id, "track/other").unwrap_err();
        assert!(err.contains("track/test"), "error should reference expected branch: {err}");
        assert!(err.contains("track/other"), "error should reference actual branch: {err}");
    }

    #[test]
    fn verify_branch_guard_with_branch_null_branch_skips_guard() {
        let reader = StubReader::new();
        reader.insert(sample_track("test", None));
        let id = TrackId::try_new("test").unwrap();
        assert!(verify_branch_guard_with_branch(&reader, &id, "any/branch").is_ok());
    }

    #[test]
    fn verify_branch_guard_with_branch_detached_head_raises() {
        let reader = StubReader::new();
        reader.insert(sample_track("test", Some("track/test")));
        let id = TrackId::try_new("test").unwrap();
        let err = verify_branch_guard_with_branch(&reader, &id, "HEAD").unwrap_err();
        assert!(err.contains("detached"), "error should mention detached HEAD: {err}");
    }

    #[test]
    fn verify_branch_guard_with_branch_corrupt_metadata_raises() {
        let reader = FailingReader;
        let id = TrackId::try_new("test").unwrap();
        let err = verify_branch_guard_with_branch(&reader, &id, "track/test").unwrap_err();
        assert!(
            err.contains("failed to read track"),
            "error should propagate reader failure: {err}"
        );
    }

    /// Verifies that `verify_branch_guard` skips the git subprocess entirely for
    /// branchless tracks. A non-existent `repo_dir` is intentional: if the guard
    /// ever regresses to calling `current_git_branch` before the null-branch check,
    /// the subprocess will fail in a missing directory and the test will catch it.
    #[test]
    fn verify_branch_guard_null_branch_skips_git() {
        let reader = StubReader::new();
        reader.insert(sample_track("test", None));
        let id = TrackId::try_new("test").unwrap();
        let nonexistent = std::path::Path::new("/nonexistent/repo/dir");
        assert!(
            verify_branch_guard(&reader, &id, nonexistent).is_ok(),
            "branchless track must return Ok without running git"
        );
    }
}
