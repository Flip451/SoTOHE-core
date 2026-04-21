//! CLI handlers for add-task, set-override, clear-override, next-task, and task-counts.

use domain::{ImplPlanReader, TaskStatusKind, TrackId, TrackStatus, TrackWriter};
use infrastructure::track::fs_store::read_track_metadata;

use crate::CliError;

use super::*;

pub(super) fn execute_add_task(
    items_dir: PathBuf,
    track_id: String,
    description: String,
    section: Option<String>,
    after: Option<String>,
    skip_branch_check: bool,
) -> Result<ExitCode, CliError> {
    let track_id = TrackId::try_new(&track_id)
        .map_err(|err| CliError::Message(format!("invalid track id: {err}")))?;

    // If --after is provided but not a valid TaskId, silently ignore it (append to end).
    // This matches Python behavior where invalid after_task_id falls back to append.
    let after_task_id = after.and_then(|a| TaskId::try_new(a).ok());

    let repo_dir = items_dir.clone();
    let project_root = resolve_project_root(&repo_dir).map_err(CliError::Message)?;

    let store = Arc::new(FsTrackStore::new(items_dir));

    // Branch guard
    if !skip_branch_check {
        transition::verify_branch_guard(&*store, &track_id, &repo_dir)
            .map_err(|msg| CliError::Message(format!("branch guard: {msg}")))?;
    }

    let add_task = usecase::AddTaskUseCase::new(Arc::clone(&store));
    let (track, new_task_id) = add_task
        .execute(&track_id, &description, section.as_deref(), after_task_id.as_ref())
        .map_err(|err| CliError::Message(format!("add-task failed: {err}")))?;

    println!("[OK] Added task {new_task_id}: {description} (track status: {})", track.status());

    sync_views(&project_root, &track_id);
    Ok(ExitCode::SUCCESS)
}

pub(super) fn execute_set_override(
    items_dir: PathBuf,
    track_id: String,
    status: String,
    reason: String,
    skip_branch_check: bool,
) -> Result<ExitCode, CliError> {
    let track_id = TrackId::try_new(&track_id)
        .map_err(|err| CliError::Message(format!("invalid track id: {err}")))?;

    let override_value = match status.as_str() {
        "blocked" => domain::StatusOverride::blocked(reason)
            .map_err(|e| CliError::Message(format!("invalid blocked reason: {e}")))?,
        "cancelled" => domain::StatusOverride::cancelled(reason)
            .map_err(|e| CliError::Message(format!("invalid cancelled reason: {e}")))?,
        other => {
            return Err(CliError::Message(format!(
                "invalid override status '{other}': must be 'blocked' or 'cancelled'"
            )));
        }
    };

    let repo_dir = items_dir.clone();
    let project_root = resolve_project_root(&repo_dir).map_err(CliError::Message)?;

    let store = Arc::new(FsTrackStore::new(items_dir));

    if !skip_branch_check {
        transition::verify_branch_guard(&*store, &track_id, &repo_dir)
            .map_err(|msg| CliError::Message(format!("branch guard: {msg}")))?;
    }

    let set_override = usecase::SetOverrideUseCase::new(Arc::clone(&store));
    let track = set_override
        .execute(&track_id, Some(override_value))
        .map_err(|err| CliError::Message(format!("set-override failed: {err}")))?;

    println!("[OK] Override set to '{}' (track status: {})", status, track.status());

    sync_views(&project_root, &track_id);
    Ok(ExitCode::SUCCESS)
}

pub(super) fn execute_clear_override(
    items_dir: PathBuf,
    track_id: String,
    skip_branch_check: bool,
) -> Result<ExitCode, CliError> {
    let track_id = TrackId::try_new(&track_id)
        .map_err(|err| CliError::Message(format!("invalid track id: {err}")))?;

    let repo_dir = items_dir.clone();
    let project_root = resolve_project_root(&repo_dir).map_err(CliError::Message)?;

    let store = Arc::new(FsTrackStore::new(items_dir));

    if !skip_branch_check {
        transition::verify_branch_guard(&*store, &track_id, &repo_dir)
            .map_err(|msg| CliError::Message(format!("branch guard: {msg}")))?;
    }

    // T007: Re-derive status from impl-plan before clearing the override so both mutations
    // happen in a single atomic `store.update()` call. This avoids partial-write state if
    // the impl-plan read fails (the override is only cleared if the status is derivable).
    let restored_status = store
        .load_impl_plan(&track_id)
        .map_err(|err| {
            CliError::Message(format!("clear-override failed reading impl-plan: {err}"))
        })?
        .map(|doc| derive_track_status_from_tasks(doc.tasks()))
        .unwrap_or(TrackStatus::Planned);

    // Single atomic write: clear override and restore derived status together.
    let track = store
        .update(&track_id, |t| {
            t.set_status_override(None);
            t.set_status(restored_status);
            Ok(())
        })
        .map_err(|err| CliError::Message(format!("clear-override failed: {err}")))?;

    println!("[OK] Override cleared (track status: {})", track.status());

    sync_views(&project_root, &track_id);
    Ok(ExitCode::SUCCESS)
}

pub(super) fn execute_next_task(
    items_dir: PathBuf,
    track_id: String,
) -> Result<ExitCode, CliError> {
    let valid_id = TrackId::try_new(&track_id)
        .map_err(|err| CliError::Message(format!("invalid track id: {err}")))?;

    // Validate the track exists before proceeding; propagate read/not-found errors.
    let (track, _meta) = read_track_metadata(&items_dir, &valid_id)
        .map_err(|err| CliError::Message(format!("next-task failed: {err}")))?;

    // Load impl-plan.json. Missing on an activated track is a corruption
    // state — fail closed. Missing on a planning-only track is valid and
    // reports "no open task".
    let store = FsTrackStore::new(items_dir);
    let impl_plan = store
        .load_impl_plan(&valid_id)
        .map_err(|err| CliError::Message(format!("next-task failed reading impl-plan: {err}")))?;

    if impl_plan.is_none() && (track.branch().is_some() || track.status() != TrackStatus::Planned) {
        return Err(CliError::Message(format!(
            "next-task: activated track '{valid_id}' is missing impl-plan.json; \
             refusing to report no-open-task for a potentially corrupt track state"
        )));
    }

    match impl_plan.as_ref().and_then(|doc| doc.next_open_task()) {
        Some(task) => {
            let payload = serde_json::json!({
                "task_id": task.id().as_ref(),
                "description": task.description(),
                "status": task.status().kind().to_string(),
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
    let valid_id = TrackId::try_new(&track_id)
        .map_err(|err| CliError::Message(format!("invalid track id: {err}")))?;

    // Validate the track exists before proceeding; propagate read/not-found errors.
    let (track, _meta) = read_track_metadata(&items_dir, &valid_id)
        .map_err(|err| CliError::Message(format!("task-counts failed: {err}")))?;

    // Load impl-plan.json.
    let store = FsTrackStore::new(items_dir);
    let impl_plan = store
        .load_impl_plan(&valid_id)
        .map_err(|err| CliError::Message(format!("task-counts failed reading impl-plan: {err}")))?;

    let (total, todo, in_progress, done, skipped) = match &impl_plan {
        Some(doc) => {
            let total = doc.tasks().len();
            let todo =
                doc.tasks().iter().filter(|t| t.status().kind() == TaskStatusKind::Todo).count();
            let in_progress = doc
                .tasks()
                .iter()
                .filter(|t| t.status().kind() == TaskStatusKind::InProgress)
                .count();
            let done =
                doc.tasks().iter().filter(|t| t.status().kind() == TaskStatusKind::Done).count();
            let skipped =
                doc.tasks().iter().filter(|t| t.status().kind() == TaskStatusKind::Skipped).count();
            (total, todo, in_progress, done, skipped)
        }
        None => {
            // Only planning-only tracks (no branch, status still Planned) may
            // have no impl-plan.json. An activated track (branch is Some, or
            // status has moved past Planned) missing its impl-plan is a
            // corruption state — fail closed so automation / operators are
            // not told "0 tasks" when work may actually be outstanding.
            if track.branch().is_some() || track.status() != TrackStatus::Planned {
                return Err(CliError::Message(format!(
                    "task-counts: activated track '{valid_id}' is missing impl-plan.json; \
                     refusing to report zero counts for a potentially corrupt track state"
                )));
            }
            (0, 0, 0, 0, 0)
        }
    };

    println!(
        r#"{{"total":{total},"todo":{todo},"in_progress":{in_progress},"done":{done},"skipped":{skipped}}}"#
    );
    Ok(ExitCode::SUCCESS)
}

/// Derives the track-level `TrackStatus` from a task slice.
///
/// Mirrors the logic in `usecase::derive_track_status_from_impl_plan`:
/// - Empty → `Planned`
/// - All resolved (done/skipped) → `Done`
/// - Any `InProgress`, OR mix of at least one resolved + at least one unresolved → `InProgress`
/// - All `Todo` → `Planned`
fn derive_track_status_from_tasks(tasks: &[domain::TrackTask]) -> TrackStatus {
    if tasks.is_empty() {
        return TrackStatus::Planned;
    }
    let all_resolved = tasks
        .iter()
        .all(|t| matches!(t.status().kind(), TaskStatusKind::Done | TaskStatusKind::Skipped));
    if all_resolved {
        return TrackStatus::Done;
    }
    let any_in_progress = tasks.iter().any(|t| t.status().kind() == TaskStatusKind::InProgress);
    let any_resolved = tasks
        .iter()
        .any(|t| matches!(t.status().kind(), TaskStatusKind::Done | TaskStatusKind::Skipped));
    if any_in_progress || any_resolved { TrackStatus::InProgress } else { TrackStatus::Planned }
}

/// Sync rendered views, printing results. Non-fatal on failure.
fn sync_views(project_root: &std::path::Path, track_id: &TrackId) {
    match render::sync_rendered_views(project_root, Some(track_id.as_ref())) {
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

    /// Write a minimal valid v4 (identity-only) metadata.json.
    fn write_metadata_v4(track_dir: &std::path::Path, track_id: &str) {
        let metadata = format!(
            r#"{{
  "schema_version": 4,
  "id": "{track_id}",
  "branch": "track/{track_id}",
  "title": "Test Track",
  "status": "in_progress",
  "created_at": "2026-01-01T00:00:00Z",
  "updated_at": "2026-01-01T00:00:00Z"
}}"#
        );
        fs::write(track_dir.join("metadata.json"), &metadata).unwrap();
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
        write_metadata_v4(&track_dir, track_id);
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
    fn test_execute_next_task_with_no_impl_plan_on_activated_track_errors() {
        // An activated track (branch set, status != planned) missing impl-plan.json is a
        // corruption state. The command fails closed rather than reporting no-open-task.
        let tmp = tempfile::tempdir().unwrap();
        let (_root, items_dir, _track_dir) = setup_test_track(tmp.path(), "test-track");

        let result = execute_next_task(items_dir, "test-track".to_string());
        assert!(result.is_err(), "expected Err on activated track without impl-plan.json");
        if let Err(CliError::Message(msg)) = result {
            assert!(msg.contains("missing impl-plan.json"), "message: {msg}");
        }
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
    fn test_execute_task_counts_with_no_impl_plan_on_activated_track_errors() {
        // Same fail-closed behavior as next-task: activated track missing impl-plan.json
        // → error rather than silently reporting zero counts.
        let tmp = tempfile::tempdir().unwrap();
        let (_root, items_dir, _track_dir) = setup_test_track(tmp.path(), "test-track");

        let result = execute_task_counts(items_dir, "test-track".to_string());
        assert!(result.is_err(), "expected Err on activated track without impl-plan.json");
        if let Err(CliError::Message(msg)) = result {
            assert!(msg.contains("missing impl-plan.json"), "message: {msg}");
        }
    }

    #[test]
    fn test_execute_task_counts_with_no_impl_plan_on_planning_only_track_succeeds() {
        // A planning-only track (no branch, status: planned) legitimately has no
        // impl-plan.json; the command reports zeros without erroring.
        let tmp = tempfile::tempdir().unwrap();
        let track_id = "planning-only";
        let track_dir = tmp.path().join("track").join("items").join(track_id);
        fs::create_dir_all(&track_dir).unwrap();
        let metadata = format!(
            r#"{{
  "schema_version": 4,
  "id": "{track_id}",
  "title": "Planning Only",
  "status": "planned",
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

        let result = execute_add_task(
            items_dir,
            "INVALID".to_string(),
            "task desc".to_string(),
            None,
            None,
            true,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_execute_set_override_invalid_status() {
        let tmp = tempfile::tempdir().unwrap();
        let (_root, items_dir, _track_dir) = setup_test_track(tmp.path(), "test-track");

        let result = execute_set_override(
            items_dir,
            "test-track".to_string(),
            "invalid_status".to_string(),
            "reason".to_string(),
            true,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_execute_add_task_happy_path() {
        let tmp = tempfile::tempdir().unwrap();
        let (_root, items_dir, _track_dir) =
            setup_test_track_with_impl_plan(tmp.path(), "test-track");

        let result = execute_add_task(
            items_dir.clone(),
            "test-track".to_string(),
            "New task".to_string(),
            None,
            None,
            true, // skip branch check for test
        );
        assert!(result.is_ok(), "add-task must succeed: {result:?}");
    }

    #[test]
    fn test_execute_add_task_without_impl_plan_fails() {
        // Without impl-plan.json, add-task should fail (can't add to missing plan).
        let tmp = tempfile::tempdir().unwrap();
        let (_root, items_dir, _track_dir) = setup_test_track(tmp.path(), "test-track");

        let result = execute_add_task(
            items_dir.clone(),
            "test-track".to_string(),
            "New task".to_string(),
            None,
            None,
            true,
        );
        assert!(result.is_err(), "add-task without impl-plan.json must fail");
    }

    #[test]
    fn test_execute_set_override_happy_path() {
        let tmp = tempfile::tempdir().unwrap();
        let (_root, items_dir, _track_dir) = setup_test_track(tmp.path(), "test-track");

        let result = execute_set_override(
            items_dir.clone(),
            "test-track".to_string(),
            "blocked".to_string(),
            "blocker reason".to_string(),
            true,
        );
        assert!(result.is_ok());

        let metadata_path = items_dir.join("test-track").join("metadata.json");
        let content = fs::read_to_string(metadata_path).unwrap();
        assert!(content.contains("blocked"));
    }

    #[test]
    fn test_execute_clear_override_happy_path() {
        let tmp = tempfile::tempdir().unwrap();
        let (_root, items_dir, _track_dir) = setup_test_track(tmp.path(), "test-track");

        // First set an override
        execute_set_override(
            items_dir.clone(),
            "test-track".to_string(),
            "blocked".to_string(),
            "reason".to_string(),
            true,
        )
        .unwrap();

        // Then clear it
        let result = execute_clear_override(items_dir, "test-track".to_string(), true);
        assert!(result.is_ok());
    }
}
