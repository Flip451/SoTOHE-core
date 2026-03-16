//! CLI handlers for add-task, set-override, clear-override, next-task, and task-counts.

use crate::CliError;

use super::*;

pub(super) fn execute_add_task(
    items_dir: PathBuf,
    locks_dir: PathBuf,
    track_id: String,
    description: String,
    section: Option<String>,
    after: Option<String>,
    skip_branch_check: bool,
) -> Result<ExitCode, CliError> {
    let track_id = TrackId::new(&track_id)
        .map_err(|err| CliError::Message(format!("invalid track id: {err}")))?;

    // If --after is provided but not a valid TaskId, silently ignore it (append to end).
    // This matches Python behavior where invalid after_task_id falls back to append.
    let after_task_id = after.and_then(|a| TaskId::new(a).ok());

    let repo_dir = items_dir.clone();
    let project_root = resolve_project_root(&repo_dir).map_err(CliError::Message)?;

    let lock_manager = FsFileLockManager::new(&locks_dir)
        .map(Arc::new)
        .map_err(|err| CliError::Message(format!("failed to initialize lock manager: {err}")))?;

    let store = Arc::new(FsTrackStore::new(items_dir, lock_manager, DEFAULT_LOCK_TIMEOUT));

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
    locks_dir: PathBuf,
    track_id: String,
    status: String,
    reason: String,
    skip_branch_check: bool,
) -> Result<ExitCode, CliError> {
    let track_id = TrackId::new(&track_id)
        .map_err(|err| CliError::Message(format!("invalid track id: {err}")))?;

    let override_value = match status.as_str() {
        "blocked" => domain::StatusOverride::blocked(reason),
        "cancelled" => domain::StatusOverride::cancelled(reason),
        other => {
            return Err(CliError::Message(format!(
                "invalid override status '{other}': must be 'blocked' or 'cancelled'"
            )));
        }
    };

    let repo_dir = items_dir.clone();
    let project_root = resolve_project_root(&repo_dir).map_err(CliError::Message)?;

    let lock_manager = FsFileLockManager::new(&locks_dir)
        .map(Arc::new)
        .map_err(|err| CliError::Message(format!("failed to initialize lock manager: {err}")))?;

    let store = Arc::new(FsTrackStore::new(items_dir, lock_manager, DEFAULT_LOCK_TIMEOUT));

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
    locks_dir: PathBuf,
    track_id: String,
    skip_branch_check: bool,
) -> Result<ExitCode, CliError> {
    let track_id = TrackId::new(&track_id)
        .map_err(|err| CliError::Message(format!("invalid track id: {err}")))?;

    let repo_dir = items_dir.clone();
    let project_root = resolve_project_root(&repo_dir).map_err(CliError::Message)?;

    let lock_manager = FsFileLockManager::new(&locks_dir)
        .map(Arc::new)
        .map_err(|err| CliError::Message(format!("failed to initialize lock manager: {err}")))?;

    let store = Arc::new(FsTrackStore::new(items_dir, lock_manager, DEFAULT_LOCK_TIMEOUT));

    if !skip_branch_check {
        transition::verify_branch_guard(&*store, &track_id, &repo_dir)
            .map_err(|msg| CliError::Message(format!("branch guard: {msg}")))?;
    }

    let set_override = usecase::SetOverrideUseCase::new(Arc::clone(&store));
    let track = set_override
        .execute(&track_id, None)
        .map_err(|err| CliError::Message(format!("clear-override failed: {err}")))?;

    println!("[OK] Override cleared (track status: {})", track.status());

    sync_views(&project_root, &track_id);
    Ok(ExitCode::SUCCESS)
}

pub(super) fn execute_next_task(
    items_dir: PathBuf,
    track_id: String,
) -> Result<ExitCode, CliError> {
    let track_id = TrackId::new(&track_id)
        .map_err(|err| CliError::Message(format!("invalid track id: {err}")))?;

    let track = read_track_metadata_simple(&items_dir, &track_id)?;

    match track.next_open_task() {
        Some(task) => {
            let obj = serde_json::json!({
                "task_id": task.id().as_str(),
                "description": task.description(),
                "status": task.status().kind().to_string(),
            });
            println!("{obj}");
        }
        None => {
            let obj = serde_json::json!({
                "task_id": null,
                "description": null,
                "status": null,
            });
            println!("{obj}");
        }
    }
    Ok(ExitCode::SUCCESS)
}

pub(super) fn execute_task_counts(
    items_dir: PathBuf,
    track_id: String,
) -> Result<ExitCode, CliError> {
    let track_id = TrackId::new(&track_id)
        .map_err(|err| CliError::Message(format!("invalid track id: {err}")))?;

    let track = read_track_metadata_simple(&items_dir, &track_id)?;

    let (mut total, mut todo, mut in_progress, mut done, mut skipped) =
        (0u32, 0u32, 0u32, 0u32, 0u32);
    for task in track.tasks() {
        total += 1;
        match task.status().kind() {
            domain::TaskStatusKind::Todo => todo += 1,
            domain::TaskStatusKind::InProgress => in_progress += 1,
            domain::TaskStatusKind::Done => done += 1,
            domain::TaskStatusKind::Skipped => skipped += 1,
        }
    }

    let obj = serde_json::json!({
        "total": total,
        "todo": todo,
        "in_progress": in_progress,
        "done": done,
        "skipped": skipped,
    });
    println!("{obj}");
    Ok(ExitCode::SUCCESS)
}

/// Read track metadata without locking (read-only queries).
fn read_track_metadata_simple(
    items_dir: &std::path::Path,
    track_id: &TrackId,
) -> Result<domain::TrackMetadata, CliError> {
    let (track, _meta) = infrastructure::track::fs_store::read_track_metadata(items_dir, track_id)
        .map_err(|err| CliError::Message(format!("failed to read track: {err}")))?;
    Ok(track)
}

/// Sync rendered views, printing results. Non-fatal on failure.
fn sync_views(project_root: &std::path::Path, track_id: &TrackId) {
    match render::sync_rendered_views(project_root, Some(track_id.as_str())) {
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

    /// Create a minimal valid metadata.json in a temp directory structure.
    /// Returns (project_root, items_dir, track_dir).
    fn setup_test_track(
        tmp: &std::path::Path,
        track_id: &str,
        tasks_json: &str,
    ) -> (PathBuf, PathBuf, PathBuf) {
        let track_dir = tmp.join("track").join("items").join(track_id);
        fs::create_dir_all(&track_dir).unwrap();
        let metadata = format!(
            r#"{{
  "schema_version": 3,
  "id": "{track_id}",
  "branch": "track/{track_id}",
  "title": "Test Track",
  "status": "in_progress",
  "created_at": "2026-01-01T00:00:00Z",
  "updated_at": "2026-01-01T00:00:00Z",
  "tasks": {tasks_json},
  "plan": {{
    "summary": [],
    "sections": [{{
      "id": "S1",
      "title": "Section 1",
      "description": [],
      "task_ids": ["T001"]
    }}]
  }},
  "status_override": null
}}"#
        );
        fs::write(track_dir.join("metadata.json"), &metadata).unwrap();
        let items_dir = tmp.join("track").join("items");
        (tmp.to_path_buf(), items_dir, track_dir)
    }

    #[test]
    fn test_execute_next_task_returns_json() {
        let tmp = tempfile::tempdir().unwrap();
        let tasks =
            r#"[{"id":"T001","description":"First task","status":"todo","commit_hash":null}]"#;
        let (_root, items_dir, _track_dir) = setup_test_track(tmp.path(), "test-track", tasks);

        let result = execute_next_task(items_dir, "test-track".to_string());
        assert!(result.is_ok());
    }

    #[test]
    fn test_next_task_json_schema_has_required_fields() {
        let tmp = tempfile::tempdir().unwrap();
        let tasks =
            r#"[{"id":"T001","description":"First task","status":"todo","commit_hash":null}]"#;
        let (_root, items_dir, _track_dir) = setup_test_track(tmp.path(), "test-track", tasks);

        let track_id = TrackId::new("test-track").unwrap();
        let track = read_track_metadata_simple(&items_dir, &track_id).unwrap();
        let task = track.next_open_task().unwrap();

        let obj = serde_json::json!({
            "task_id": task.id().as_str(),
            "description": task.description(),
            "status": task.status().kind().to_string(),
        });
        let json_str = obj.to_string();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed["task_id"], "T001");
        assert_eq!(parsed["description"], "First task");
        assert_eq!(parsed["status"], "todo");
    }

    #[test]
    fn test_next_task_prefers_in_progress_over_todo() {
        let tmp = tempfile::tempdir().unwrap();
        // T001 is todo, T002 is in_progress — should return T002 (in_progress first)
        let track_dir = tmp.path().join("track").join("items").join("test-track");
        fs::create_dir_all(&track_dir).unwrap();
        let metadata = r#"{
  "schema_version": 3, "id": "test-track", "branch": "track/test-track",
  "title": "Test", "status": "in_progress",
  "created_at": "2026-01-01T00:00:00Z", "updated_at": "2026-01-01T00:00:00Z",
  "tasks": [
    {"id":"T001","description":"Todo task","status":"todo","commit_hash":null},
    {"id":"T002","description":"Active task","status":"in_progress","commit_hash":null}
  ],
  "plan": {"summary":[],"sections":[{"id":"S1","title":"S1","description":[],"task_ids":["T001","T002"]}]},
  "status_override": null
}"#;
        fs::write(track_dir.join("metadata.json"), metadata).unwrap();
        let items_dir = tmp.path().join("track").join("items");

        let track_id = TrackId::new("test-track").unwrap();
        let track = read_track_metadata_simple(&items_dir, &track_id).unwrap();
        let task = track.next_open_task().unwrap();
        assert_eq!(task.id().as_str(), "T002");
        assert_eq!(task.status().kind().to_string(), "in_progress");
    }

    #[test]
    fn test_execute_next_task_with_no_open_tasks() {
        let tmp = tempfile::tempdir().unwrap();
        let tasks =
            r#"[{"id":"T001","description":"Done task","status":"done","commit_hash":"abc1234"}]"#;
        let (_root, items_dir, _track_dir) = setup_test_track(tmp.path(), "test-track", tasks);

        let result = execute_next_task(items_dir, "test-track".to_string());
        assert!(result.is_ok());
    }

    #[test]
    fn test_task_counts_json_schema() {
        let tmp = tempfile::tempdir().unwrap();
        let track_dir = tmp.path().join("track").join("items").join("test-track");
        fs::create_dir_all(&track_dir).unwrap();
        let metadata = r#"{
  "schema_version": 3, "id": "test-track", "branch": "track/test-track",
  "title": "Test", "status": "in_progress",
  "created_at": "2026-01-01T00:00:00Z", "updated_at": "2026-01-01T00:00:00Z",
  "tasks": [
    {"id":"T001","description":"A","status":"todo","commit_hash":null},
    {"id":"T002","description":"B","status":"in_progress","commit_hash":null},
    {"id":"T003","description":"C","status":"done","commit_hash":"abc1234"},
    {"id":"T004","description":"D","status":"skipped","commit_hash":null}
  ],
  "plan": {"summary":[],"sections":[{"id":"S1","title":"S1","description":[],"task_ids":["T001","T002","T003","T004"]}]},
  "status_override": null
}"#;
        fs::write(track_dir.join("metadata.json"), metadata).unwrap();
        let items_dir = tmp.path().join("track").join("items");

        let track_id = TrackId::new("test-track").unwrap();
        let track = read_track_metadata_simple(&items_dir, &track_id).unwrap();

        let (mut total, mut todo, mut in_progress, mut done, mut skipped) =
            (0u32, 0u32, 0u32, 0u32, 0u32);
        for task in track.tasks() {
            total += 1;
            match task.status().kind() {
                domain::TaskStatusKind::Todo => todo += 1,
                domain::TaskStatusKind::InProgress => in_progress += 1,
                domain::TaskStatusKind::Done => done += 1,
                domain::TaskStatusKind::Skipped => skipped += 1,
            }
        }
        assert_eq!(total, 4);
        assert_eq!(todo, 1);
        assert_eq!(in_progress, 1);
        assert_eq!(done, 1);
        assert_eq!(skipped, 1);
    }

    #[test]
    fn test_execute_task_counts_returns_json() {
        let tmp = tempfile::tempdir().unwrap();
        let tasks =
            r#"[{"id":"T001","description":"First task","status":"todo","commit_hash":null}]"#;
        let (_root, items_dir, _track_dir) = setup_test_track(tmp.path(), "test-track", tasks);

        let result = execute_task_counts(items_dir, "test-track".to_string());
        assert!(result.is_ok());
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
    fn test_execute_task_counts_missing_track() {
        let tmp = tempfile::tempdir().unwrap();
        let items_dir = tmp.path().join("track").join("items");
        fs::create_dir_all(&items_dir).unwrap();

        let result = execute_task_counts(items_dir, "nonexistent".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_execute_add_task_invalid_track_id() {
        let tmp = tempfile::tempdir().unwrap();
        let items_dir = tmp.path().join("track").join("items");
        fs::create_dir_all(&items_dir).unwrap();

        let result = execute_add_task(
            items_dir,
            PathBuf::from(".locks"),
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
        let tasks = r#"[{"id":"T001","description":"Task","status":"todo","commit_hash":null}]"#;
        let (_root, items_dir, _track_dir) = setup_test_track(tmp.path(), "test-track", tasks);

        let result = execute_set_override(
            items_dir,
            PathBuf::from(".locks"),
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
        let tasks =
            r#"[{"id":"T001","description":"First task","status":"todo","commit_hash":null}]"#;
        let (_root, items_dir, _track_dir) = setup_test_track(tmp.path(), "test-track", tasks);
        let locks_dir = tmp.path().join(".locks");

        let result = execute_add_task(
            items_dir.clone(),
            locks_dir,
            "test-track".to_string(),
            "New task".to_string(),
            None,
            None,
            true, // skip branch check for test
        );
        assert!(result.is_ok());

        // Verify the task was added
        let metadata_path = items_dir.join("test-track").join("metadata.json");
        let content = fs::read_to_string(metadata_path).unwrap();
        assert!(content.contains("New task"));
    }

    #[test]
    fn test_execute_set_override_happy_path() {
        let tmp = tempfile::tempdir().unwrap();
        let tasks = r#"[{"id":"T001","description":"Task","status":"todo","commit_hash":null}]"#;
        let (_root, items_dir, _track_dir) = setup_test_track(tmp.path(), "test-track", tasks);
        let locks_dir = tmp.path().join(".locks");

        let result = execute_set_override(
            items_dir.clone(),
            locks_dir.clone(),
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
        let tasks = r#"[{"id":"T001","description":"Task","status":"todo","commit_hash":null}]"#;
        let (_root, items_dir, _track_dir) = setup_test_track(tmp.path(), "test-track", tasks);
        let locks_dir = tmp.path().join(".locks");

        // First set an override
        execute_set_override(
            items_dir.clone(),
            locks_dir.clone(),
            "test-track".to_string(),
            "blocked".to_string(),
            "reason".to_string(),
            true,
        )
        .unwrap();

        // Then clear it
        let result = execute_clear_override(items_dir, locks_dir, "test-track".to_string(), true);
        assert!(result.is_ok());
    }
}
