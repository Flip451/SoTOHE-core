//! CLI handlers for add-task, set-override, clear-override, next-task, and task-counts.

use std::path::PathBuf;
use std::process::ExitCode;

use cli_composition::TrackCompositionRoot;
use cli_driver::CommandOutcome;
use cli_driver::track::TrackInput;

use crate::CliError;

pub(super) fn track_driver_outcome_to_result(
    outcome: CommandOutcome,
) -> Result<ExitCode, CliError> {
    let exit_code = outcome.exit_code;
    if let Some(stdout) = outcome.stdout {
        println!("{stdout}");
    }
    if exit_code == 0 {
        if let Some(stderr) = outcome.stderr {
            eprintln!("{stderr}");
        }
        Ok(ExitCode::from(exit_code))
    } else {
        let message = outcome.stderr.unwrap_or_else(|| "track command failed".to_owned());
        Err(CliError::Message(message))
    }
}

pub(super) fn execute_add_task(
    items_dir: PathBuf,
    track_id: String,
    description: String,
    section: Option<String>,
    after: Option<String>,
) -> Result<ExitCode, CliError> {
    let outcome = TrackCompositionRoot::new().track_driver().handle(TrackInput::AddTask {
        items_dir,
        track_id: Some(track_id),
        description,
        section,
        after,
    });
    track_driver_outcome_to_result(outcome)
}

pub(super) fn execute_set_override(
    items_dir: PathBuf,
    track_id: String,
    status: String,
    reason: String,
) -> Result<ExitCode, CliError> {
    let outcome = TrackCompositionRoot::new().track_driver().handle(TrackInput::SetOverride {
        items_dir,
        track_id: Some(track_id),
        status,
        reason,
    });
    track_driver_outcome_to_result(outcome)
}

pub(super) fn execute_clear_override(
    items_dir: PathBuf,
    track_id: String,
) -> Result<ExitCode, CliError> {
    let outcome = TrackCompositionRoot::new()
        .track_driver()
        .handle(TrackInput::ClearOverride { items_dir, track_id: Some(track_id) });
    track_driver_outcome_to_result(outcome)
}

pub(super) fn execute_next_task(
    items_dir: PathBuf,
    track_id: String,
) -> Result<ExitCode, CliError> {
    let outcome = TrackCompositionRoot::new()
        .track_driver()
        .handle(TrackInput::NextTask { items_dir, track_id: Some(track_id) });
    track_driver_outcome_to_result(outcome)
}

pub(super) fn execute_task_counts(
    items_dir: PathBuf,
    track_id: String,
) -> Result<ExitCode, CliError> {
    let outcome = TrackCompositionRoot::new()
        .track_driver()
        .handle(TrackInput::TaskCounts { items_dir, track_id: Some(track_id) });
    track_driver_outcome_to_result(outcome)
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
