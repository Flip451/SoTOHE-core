//! CLI handlers for add-task, set-override, clear-override, next-task, and task-counts.

use crate::CliError;

use super::*;
use usecase::task_ops::{TaskOperationService as _, TaskQueryService as _};
use usecase::track_resolution::{BranchReadError, BranchReaderPort};

#[derive(Debug)]
struct LazyBranchReader {
    project_root: PathBuf,
}

impl LazyBranchReader {
    fn new(project_root: PathBuf) -> Self {
        Self { project_root }
    }
}

impl BranchReaderPort for LazyBranchReader {
    fn current_branch(&self) -> Result<Option<String>, BranchReadError> {
        let repo = SystemGitRepo::discover_from(&self.project_root).map_err(|e| {
            BranchReadError::ReadFailed(format!("failed to discover git repo: {e}"))
        })?;
        BranchReaderPort::current_branch(&repo)
    }
}

fn build_branch_reader(project_root: &std::path::Path) -> Option<Arc<dyn BranchReaderPort>> {
    Some(Arc::new(LazyBranchReader::new(project_root.to_path_buf())))
}

pub(super) fn execute_add_task(
    items_dir: PathBuf,
    track_id: String,
    description: String,
    section: Option<String>,
    after: Option<String>,
) -> Result<ExitCode, CliError> {
    // Validate track_id as a safe slug before any filesystem probe.
    validate_track_id_str(&track_id).map_err(CliError::Message)?;

    let repo_dir = items_dir.clone();
    let project_root = resolve_project_root(&repo_dir).map_err(CliError::Message)?;

    let store = Arc::new(FsTrackStore::new(items_dir.clone()));
    let branch_reader = build_branch_reader(&project_root);
    let service =
        usecase::task_ops::TaskOperationInteractor::new(Arc::clone(&store), branch_reader);

    // If --after is provided but not a valid TaskId (format: T followed by one
    // or more digits that fit in u64), silently ignore it and append to the end
    // of the section. This preserves the historical lenient behavior where
    // invalid after-task IDs were treated as "not found" → append.
    // Mirrors domain::TaskId::try_new exactly (T prefix + non-empty digits + u64 parse).
    let after_task_id = after.filter(|a| {
        a.strip_prefix('T').is_some_and(|digits| {
            !digits.is_empty()
                && digits.chars().all(|ch| ch.is_ascii_digit())
                && digits.parse::<u64>().is_ok()
        })
    });

    let cmd = usecase::task_ops::AddTaskCommand {
        items_dir,
        track_id: track_id.clone(),
        description: description.clone(),
        section,
        after_task_id,
    };
    let output = service
        .add_task(cmd)
        .map_err(|err| CliError::Message(format!("add-task failed: {err}")))?;

    let new_task_id = output.task_id.as_deref().unwrap_or("?");
    println!(
        "[OK] Added task {new_task_id}: {description} (track status: {})",
        output.derived_status
    );

    sync_views(&project_root, &output.track_id);
    Ok(ExitCode::SUCCESS)
}

pub(super) fn execute_set_override(
    items_dir: PathBuf,
    track_id: String,
    status: String,
    reason: String,
) -> Result<ExitCode, CliError> {
    // Validate track_id as a safe slug before any filesystem probe.
    validate_track_id_str(&track_id).map_err(CliError::Message)?;

    let repo_dir = items_dir.clone();
    let project_root = resolve_project_root(&repo_dir).map_err(CliError::Message)?;

    let store = Arc::new(FsTrackStore::new(items_dir.clone()));
    let branch_reader = build_branch_reader(&project_root);
    let service =
        usecase::task_ops::TaskOperationInteractor::new(Arc::clone(&store), branch_reader);

    let cmd = usecase::task_ops::SetOverrideCommand {
        items_dir,
        track_id: track_id.clone(),
        status: status.clone(),
        reason,
    };
    let output = service
        .set_override(cmd)
        .map_err(|err| CliError::Message(format!("set-override failed: {err}")))?;

    println!("[OK] Override set to '{}' (track status: {})", status, output.derived_status);

    sync_views(&project_root, &output.track_id);
    Ok(ExitCode::SUCCESS)
}

pub(super) fn execute_clear_override(
    items_dir: PathBuf,
    track_id: String,
) -> Result<ExitCode, CliError> {
    // Validate track_id as a safe slug before any filesystem probe.
    validate_track_id_str(&track_id).map_err(CliError::Message)?;

    let repo_dir = items_dir.clone();
    let project_root = resolve_project_root(&repo_dir).map_err(CliError::Message)?;

    let store = Arc::new(FsTrackStore::new(items_dir.clone()));
    let branch_reader = build_branch_reader(&project_root);
    let service =
        usecase::task_ops::TaskOperationInteractor::new(Arc::clone(&store), branch_reader);

    let cmd = usecase::task_ops::ClearOverrideCommand { items_dir, track_id: track_id.clone() };
    let output = service
        .clear_override(cmd)
        .map_err(|err| CliError::Message(format!("clear-override failed: {err}")))?;

    println!("[OK] Override cleared (track status: {})", output.derived_status);

    sync_views(&project_root, &output.track_id);
    Ok(ExitCode::SUCCESS)
}

pub(super) fn execute_next_task(
    items_dir: PathBuf,
    track_id: String,
) -> Result<ExitCode, CliError> {
    let store = Arc::new(FsTrackStore::new(items_dir.clone()));
    let service = usecase::task_ops::TaskQueryInteractor::new(Arc::clone(&store));

    // Retrieve the next open task. `NextTaskOutput` does not carry a status
    // field, so the task status is determined separately via `task_counts`.
    //
    // `domain::ImplPlanDocument::next_open_task` returns in_progress tasks
    // before todo tasks (in_progress has priority). Therefore:
    //   - counts.in_progress > 0 → the returned task is an in_progress task.
    //   - counts.in_progress == 0 → the returned task is a todo task.
    // Both calls read the same underlying store, so counts and next_task are
    // consistent within a single CLI invocation.
    let next = service
        .next_task(track_id.clone(), items_dir.clone())
        .map_err(|err| CliError::Message(format!("next-task failed: {err}")))?;

    match next {
        Some(task) => {
            // Determine the task's status from counts. In_progress tasks take
            // priority in `next_open_task`, so the returned task is in_progress
            // if and only if `counts.in_progress > 0`.
            let counts = service
                .task_counts(track_id, items_dir)
                .map_err(|err| CliError::Message(format!("next-task failed (counts): {err}")))?;
            let task_status = if counts.in_progress > 0 { "in_progress" } else { "todo" };
            let payload = serde_json::json!({
                "task_id": task.task_id,
                "description": task.description,
                "status": task_status,
            });
            println!("{payload}");
            Ok(ExitCode::SUCCESS)
        }
        None => {
            let payload = serde_json::json!({
                "task_id": null,
                "description": null,
                "status": null,
            });
            println!("{payload}");
            Ok(ExitCode::SUCCESS)
        }
    }
}

pub(super) fn execute_task_counts(
    items_dir: PathBuf,
    track_id: String,
) -> Result<ExitCode, CliError> {
    let store = Arc::new(FsTrackStore::new(items_dir.clone()));
    let service = usecase::task_ops::TaskQueryInteractor::new(Arc::clone(&store));

    let counts = service
        .task_counts(track_id, items_dir)
        .map_err(|err| CliError::Message(format!("task-counts failed: {err}")))?;

    let total = counts.todo + counts.in_progress + counts.done + counts.skipped;
    println!(
        r#"{{"total":{total},"todo":{},"in_progress":{},"done":{},"skipped":{}}}"#,
        counts.todo, counts.in_progress, counts.done, counts.skipped
    );
    Ok(ExitCode::SUCCESS)
}

/// Sync rendered views, printing results. Non-fatal on failure.
fn sync_views(project_root: &std::path::Path, track_id: &str) {
    match render::sync_rendered_views(project_root, Some(track_id)) {
        Ok(changed) => {
            for path in changed {
                match path.strip_prefix(project_root) {
                    Ok(relative) => println!("[OK] Rendered: {}", relative.display()),
                    Err(_) => println!("[OK] Rendered: {}", path.display()),
                }
            }
        }
        Err(err) => {
            eprintln!("warning: operation persisted but sync-views failed: {err}");
        }
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use std::fs;

    /// Write a minimal valid v5 (identity-only, no `status` field) metadata.json.
    fn write_metadata_v5(track_dir: &std::path::Path, track_id: &str) {
        let metadata = format!(
            r#"{{
  "schema_version": 5,
  "id": "{track_id}",
  "branch": "track/{track_id}",
  "title": "Test Track",
  "created_at": "2026-01-01T00:00:00Z",
  "updated_at": "2026-01-01T00:00:00Z"
}}"#
        );
        fs::write(track_dir.join("metadata.json"), &metadata).unwrap();
    }

    /// Write a minimal valid v5 branchless metadata.json (`branch: null`).
    ///
    /// Branchless tracks skip the in-usecase branch guard unconditionally
    /// (`enforce_branch_guard` returns `Ok(())` when `track.branch()` is `None`),
    /// so these fixtures let CLI-layer tests exercise the full mutation logic
    /// without requiring a real git repository in the temp directory.
    fn write_metadata_v5_branchless(track_dir: &std::path::Path, track_id: &str) {
        let metadata = format!(
            r#"{{
  "schema_version": 5,
  "id": "{track_id}",
  "branch": null,
  "title": "Test Track",
  "created_at": "2026-01-01T00:00:00Z",
  "updated_at": "2026-01-01T00:00:00Z"
}}"#
        );
        fs::write(track_dir.join("metadata.json"), &metadata).unwrap();
    }

    /// Create a minimal branchless track setup. Returns (project_root, items_dir, track_dir).
    ///
    /// The branch guard in the usecase layer is skipped for branchless tracks,
    /// so these fixtures let unit tests verify mutation logic without a git repo.
    fn setup_test_track_branchless(
        tmp: &std::path::Path,
        track_id: &str,
    ) -> (PathBuf, PathBuf, PathBuf) {
        let track_dir = tmp.join("track").join("items").join(track_id);
        fs::create_dir_all(&track_dir).unwrap();
        write_metadata_v5_branchless(&track_dir, track_id);
        let items_dir = tmp.join("track").join("items");
        (tmp.to_path_buf(), items_dir, track_dir)
    }

    /// Create a branchless track with both metadata.json and impl-plan.json.
    fn setup_test_track_branchless_with_impl_plan(
        tmp: &std::path::Path,
        track_id: &str,
    ) -> (PathBuf, PathBuf, PathBuf) {
        let (root, items_dir, track_dir) = setup_test_track_branchless(tmp, track_id);
        write_impl_plan(&track_dir);
        (root, items_dir, track_dir)
    }

    /// Write a minimal valid impl-plan.json with a single task T001.
    fn write_impl_plan(track_dir: &std::path::Path) {
        let impl_plan = r#"{
  "schema_version": 1,
  "tasks": [
    {"id": "T001", "description": "First task", "status": "todo"}
  ],
  "plan": {
    "summary": [],
    "sections": [
      {"id": "S1", "title": "Section 1", "description": [], "task_ids": ["T001"]}
    ]
  }
}"#;
        fs::write(track_dir.join("impl-plan.json"), impl_plan).unwrap();
    }

    /// Create a minimal track setup. Returns (project_root, items_dir, track_dir).
    fn setup_test_track(tmp: &std::path::Path, track_id: &str) -> (PathBuf, PathBuf, PathBuf) {
        let track_dir = tmp.join("track").join("items").join(track_id);
        fs::create_dir_all(&track_dir).unwrap();
        write_metadata_v5(&track_dir, track_id);
        let items_dir = tmp.join("track").join("items");
        (tmp.to_path_buf(), items_dir, track_dir)
    }

    /// Create a track with both metadata.json and impl-plan.json.
    fn setup_test_track_with_impl_plan(
        tmp: &std::path::Path,
        track_id: &str,
    ) -> (PathBuf, PathBuf, PathBuf) {
        let (root, items_dir, track_dir) = setup_test_track(tmp, track_id);
        write_impl_plan(&track_dir);
        (root, items_dir, track_dir)
    }

    #[test]
    fn test_execute_next_task_with_open_task() {
        let tmp = tempfile::tempdir().unwrap();
        let (_root, items_dir, _track_dir) =
            setup_test_track_with_impl_plan(tmp.path(), "test-track");

        let result = execute_next_task(items_dir, "test-track".to_string());
        assert!(result.is_ok(), "expected Ok from next-task: {result:?}");
    }

    #[test]
    fn test_execute_next_task_with_no_impl_plan_on_activated_track_succeeds() {
        // An activated track without impl-plan.json is a legitimate Phase 0-2
        // state (branch materialised before impl-plan.json is authored).
        // `derive_track_status` returns Planned via its fallback; next-task
        // reports no open task rather than erroring. The old invariant
        // `is_activated() ↔ impl-plan.json present` was removed in T025 because
        // it conflicted with /track:init branch creation and Phase 0-2 progression.
        let tmp = tempfile::tempdir().unwrap();
        let (_root, items_dir, _track_dir) = setup_test_track(tmp.path(), "test-track");

        let result = execute_next_task(items_dir, "test-track".to_string());
        assert!(
            result.is_ok(),
            "expected Ok on activated track without impl-plan.json: {result:?}"
        );
    }

    #[test]
    fn test_execute_task_counts_with_impl_plan() {
        let tmp = tempfile::tempdir().unwrap();
        let (_root, items_dir, _track_dir) =
            setup_test_track_with_impl_plan(tmp.path(), "test-track");

        let result = execute_task_counts(items_dir, "test-track".to_string());
        assert!(result.is_ok(), "expected Ok from task-counts: {result:?}");
    }

    #[test]
    fn test_execute_task_counts_with_no_impl_plan_on_activated_track_succeeds() {
        // Matches `execute_next_task` semantics after T025: an activated
        // track without impl-plan.json is a legitimate Phase 0-2 state and
        // reports zero counts without erroring.
        let tmp = tempfile::tempdir().unwrap();
        let (_root, items_dir, _track_dir) = setup_test_track(tmp.path(), "test-track");

        let result = execute_task_counts(items_dir, "test-track".to_string());
        assert!(
            result.is_ok(),
            "expected Ok on activated track without impl-plan.json: {result:?}"
        );
    }

    #[test]
    fn test_execute_task_counts_with_no_impl_plan_on_planning_only_track_succeeds() {
        // A planning-only track (no branch, no override) legitimately has no
        // impl-plan.json; the command reports zeros without erroring.
        let tmp = tempfile::tempdir().unwrap();
        let track_id = "planning-only";
        let track_dir = tmp.path().join("track").join("items").join(track_id);
        fs::create_dir_all(&track_dir).unwrap();
        // v5 metadata without branch (branchless planning-only track)
        let metadata = format!(
            r#"{{
  "schema_version": 5,
  "id": "{track_id}",
  "branch": null,
  "title": "Planning Only",
  "created_at": "2026-01-01T00:00:00Z",
  "updated_at": "2026-01-01T00:00:00Z"
}}"#
        );
        fs::write(track_dir.join("metadata.json"), &metadata).unwrap();
        let items_dir = tmp.path().join("track").join("items");

        let result = execute_task_counts(items_dir, track_id.to_string());
        assert!(
            result.is_ok(),
            "planning-only track without impl-plan.json must succeed with zero counts: {result:?}"
        );
    }

    #[test]
    fn test_execute_next_task_invalid_track_id() {
        let tmp = tempfile::tempdir().unwrap();
        let items_dir = tmp.path().join("track").join("items");
        fs::create_dir_all(&items_dir).unwrap();

        let result = execute_next_task(items_dir, "INVALID ID".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_execute_task_counts_invalid_track_id() {
        let tmp = tempfile::tempdir().unwrap();
        let items_dir = tmp.path().join("track").join("items");
        fs::create_dir_all(&items_dir).unwrap();

        let result = execute_task_counts(items_dir, "INVALID ID".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_execute_add_task_invalid_track_id() {
        let tmp = tempfile::tempdir().unwrap();
        let items_dir = tmp.path().join("track").join("items");
        fs::create_dir_all(&items_dir).unwrap();

        // Invalid track ID is rejected before branch guard is reached.
        let result =
            execute_add_task(items_dir, "INVALID".to_string(), "task desc".to_string(), None, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_execute_set_override_invalid_status() {
        let tmp = tempfile::tempdir().unwrap();
        let (_root, items_dir, _track_dir) = setup_test_track(tmp.path(), "test-track");

        // Invalid status is rejected by the usecase layer (before the branch guard
        // fires on the real git repo, which is absent in this temp directory).
        let result = execute_set_override(
            items_dir,
            "test-track".to_string(),
            "invalid_status".to_string(),
            "reason".to_string(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_execute_add_task_happy_path() {
        // Uses a branchless track fixture (`branch: null`) so that the in-usecase
        // branch guard is a no-op (skipped when track.branch() is None). This
        // allows the test to verify the full CLI→domain mutation path without
        // requiring a real git repository.
        let tmp = tempfile::tempdir().unwrap();
        let (_root, items_dir, _track_dir) =
            setup_test_track_branchless_with_impl_plan(tmp.path(), "test-track");

        let result = execute_add_task(
            items_dir.clone(),
            "test-track".to_string(),
            "New task".to_string(),
            None,
            None,
        );
        assert!(result.is_ok(), "add-task with branchless track must succeed: {result:?}");
    }

    #[test]
    fn test_execute_add_task_without_impl_plan_fails() {
        // Without impl-plan.json, add-task should fail (can't add to missing plan).
        // Uses branchless fixture so the domain layer (not the branch guard) returns
        // the error: impl-plan.json absent → TrackNotFound or ImplPlanNotFound.
        let tmp = tempfile::tempdir().unwrap();
        let (_root, items_dir, _track_dir) = setup_test_track_branchless(tmp.path(), "test-track");

        let result = execute_add_task(
            items_dir.clone(),
            "test-track".to_string(),
            "New task".to_string(),
            None,
            None,
        );
        assert!(result.is_err(), "add-task without impl-plan.json must fail");
    }

    #[test]
    fn test_execute_set_override_happy_path() {
        // Uses a branchless track fixture so the in-usecase branch guard is
        // skipped. The test verifies the full CLI→domain mutation path and that
        // the override is actually persisted in metadata.json.
        let tmp = tempfile::tempdir().unwrap();
        let (_root, items_dir, track_dir) = setup_test_track_branchless(tmp.path(), "test-track");

        let result = execute_set_override(
            items_dir.clone(),
            "test-track".to_string(),
            "blocked".to_string(),
            "blocker reason".to_string(),
        );
        assert!(result.is_ok(), "set-override with branchless track must succeed: {result:?}");

        // Verify the override was persisted to metadata.json.
        let content = fs::read_to_string(track_dir.join("metadata.json")).unwrap();
        assert!(
            content.contains("blocked"),
            "metadata.json must contain the override status after set-override:\n{content}"
        );
    }

    #[test]
    fn test_execute_clear_override_happy_path() {
        // Uses a branchless track fixture so the in-usecase branch guard is
        // skipped. First sets an override, then clears it. Verifies both the
        // mutation path and that the override is removed from metadata.json.
        let tmp = tempfile::tempdir().unwrap();
        let (_root, items_dir, track_dir) = setup_test_track_branchless(tmp.path(), "test-track");

        // Set up an existing override first so clear-override has something to remove.
        execute_set_override(
            items_dir.clone(),
            "test-track".to_string(),
            "blocked".to_string(),
            "initial blocker".to_string(),
        )
        .expect("set-override must succeed before clear-override test");

        let result = execute_clear_override(items_dir, "test-track".to_string());
        assert!(result.is_ok(), "clear-override with branchless track must succeed: {result:?}");

        // Verify the override was cleared from metadata.json.
        let content = fs::read_to_string(track_dir.join("metadata.json")).unwrap();
        assert!(
            !content.contains("\"blocked\""),
            "metadata.json must not contain override status after clear-override:\n{content}"
        );
    }
}
