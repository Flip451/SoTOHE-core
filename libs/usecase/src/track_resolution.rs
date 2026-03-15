//! Track resolution and guard logic extracted from CLI layer.
//!
//! These functions contain business rules that belong in the use case layer
//! rather than CLI: track ID detection from branch names, task transition
//! resolution, and activation guard checks.

use domain::{CommitHash, TaskStatusKind, TaskTransition, TrackId, TrackReader};

/// Resolves a track ID from the current git branch name.
///
/// # Errors
/// Returns an error message if the branch is not a `track/` branch,
/// is detached HEAD, or is `None`.
pub fn resolve_track_id_from_branch(branch: Option<&str>) -> Result<String, String> {
    match branch {
        Some(b) if b.starts_with("track/") => Ok(b["track/".len()..].to_owned()),
        Some("HEAD") => Err("detached HEAD; provide an explicit track-id".to_owned()),
        Some(b) => Err(format!("not on a track branch (on '{b}'); provide an explicit track-id")),
        None => Err("cannot determine current git branch; provide an explicit track-id".to_owned()),
    }
}

/// Resolves the correct `TaskTransition` based on target status string and
/// current task status.
///
/// Handles cases like `done -> in_progress` (Reopen) vs `todo -> in_progress` (Start).
///
/// # Errors
/// Returns an error if the target status string is not recognized.
pub fn resolve_transition(
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
        other => Err(other.to_owned()),
    }
}

/// Rejects implementation-phase task transitions on branchless (planning-only) tracks.
///
/// If the target status is an implementation status (`in_progress`, `done`, `skipped`)
/// and the track is a v3 track without an activated branch, this returns an error
/// directing the user to activate the track first.
///
/// # Errors
/// Returns an error message if the transition is blocked by the activation guard.
pub fn reject_branchless_implementation_transition(
    schema_version: u32,
    branch: Option<&str>,
    track_id: &TrackId,
    target_status: &str,
) -> Result<(), String> {
    if !matches!(target_status, "in_progress" | "done" | "skipped") {
        return Ok(());
    }

    if schema_version == 3 && branch.is_none() {
        return Err(format!(
            "track '{track_id}' is not activated yet; run /track:activate {track_id}"
        ));
    }

    Ok(())
}

/// Autonomously checks the branchless activation guard using a `TrackReader` port.
///
/// Reads the track branch state from the reader and delegates to
/// [`reject_branchless_implementation_transition`]. `schema_version` is passed
/// separately because `TrackReader` does not expose document-level metadata
/// (same pattern as `ActivateTrackUseCase::execute`).
///
/// # Errors
/// Returns an error message if the transition is blocked or the track cannot be read.
pub fn reject_branchless_guard(
    reader: &impl TrackReader,
    track_id: &TrackId,
    target_status: &str,
    schema_version: u32,
) -> Result<(), String> {
    if !matches!(target_status, "in_progress" | "done" | "skipped") {
        return Ok(());
    }
    let track = reader
        .find(track_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("track '{track_id}' not found"))?;
    reject_branchless_implementation_transition(
        schema_version,
        track.branch().map(|b| b.as_str()),
        track_id,
        target_status,
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    // --- resolve_track_id_from_branch ---

    #[test]
    fn test_resolve_track_id_from_branch_with_valid_track_branch_succeeds() {
        let result = resolve_track_id_from_branch(Some("track/my-feature"));
        assert_eq!(result.unwrap(), "my-feature");
    }

    #[test]
    fn test_resolve_track_id_from_branch_with_detached_head_returns_error() {
        let result = resolve_track_id_from_branch(Some("HEAD"));
        assert!(result.unwrap_err().contains("detached HEAD"));
    }

    #[test]
    fn test_resolve_track_id_from_branch_with_non_track_branch_returns_error() {
        let result = resolve_track_id_from_branch(Some("main"));
        assert!(result.unwrap_err().contains("not on a track branch"));
    }

    #[test]
    fn test_resolve_track_id_from_branch_with_none_returns_error() {
        let result = resolve_track_id_from_branch(None);
        assert!(result.unwrap_err().contains("cannot determine"));
    }

    // --- resolve_transition ---

    #[test]
    fn test_resolve_transition_todo_to_in_progress_returns_start() {
        let result = resolve_transition("in_progress", TaskStatusKind::Todo, None);
        assert!(matches!(result.unwrap(), TaskTransition::Start));
    }

    #[test]
    fn test_resolve_transition_done_to_in_progress_returns_reopen() {
        let result = resolve_transition("in_progress", TaskStatusKind::Done, None);
        assert!(matches!(result.unwrap(), TaskTransition::Reopen));
    }

    #[test]
    fn test_resolve_transition_to_done_returns_complete() {
        let hash = CommitHash::new("abc1234").unwrap();
        let result = resolve_transition("done", TaskStatusKind::InProgress, Some(hash));
        assert!(matches!(result.unwrap(), TaskTransition::Complete { .. }));
    }

    #[test]
    fn test_resolve_transition_to_todo_returns_reset() {
        let result = resolve_transition("todo", TaskStatusKind::InProgress, None);
        assert!(matches!(result.unwrap(), TaskTransition::ResetToTodo));
    }

    #[test]
    fn test_resolve_transition_to_skipped_returns_skip() {
        let result = resolve_transition("skipped", TaskStatusKind::Todo, None);
        assert!(matches!(result.unwrap(), TaskTransition::Skip));
    }

    #[test]
    fn test_resolve_transition_with_unsupported_status_returns_raw_token() {
        let result = resolve_transition("invalid", TaskStatusKind::Todo, None);
        assert_eq!(result.unwrap_err(), "invalid");
    }

    use rstest::rstest;

    // --- reject_branchless_implementation_transition ---

    #[rstest]
    #[case::todo_target_is_allowed(3, None, "todo", true)]
    #[case::in_progress_on_branchless_v3_is_rejected(3, None, "in_progress", false)]
    #[case::in_progress_with_branch_is_allowed(3, Some("track/test"), "in_progress", true)]
    #[case::in_progress_on_v2_is_allowed(2, None, "in_progress", true)]
    #[case::done_on_branchless_v3_is_rejected(3, None, "done", false)]
    #[case::skipped_on_branchless_v3_is_rejected(3, None, "skipped", false)]
    fn test_reject_branchless_implementation_transition(
        #[case] schema_version: u32,
        #[case] branch: Option<&str>,
        #[case] target_status: &str,
        #[case] expect_ok: bool,
    ) {
        let id = TrackId::new("test").unwrap();
        let result =
            reject_branchless_implementation_transition(schema_version, branch, &id, target_status);
        if expect_ok {
            assert!(result.is_ok());
        } else {
            assert!(result.unwrap_err().contains("not activated yet"));
        }
    }

    // --- reject_branchless_guard (with TrackReader) ---

    use std::collections::HashMap;
    use std::sync::Mutex;

    use domain::{
        PlanSection, PlanView, RepositoryError, TrackBranch, TrackMetadata, TrackReadError,
        TrackReader, TrackTask,
    };

    #[derive(Default)]
    struct StubReader {
        tracks: Mutex<HashMap<TrackId, TrackMetadata>>,
    }

    impl TrackReader for StubReader {
        fn find(&self, id: &TrackId) -> Result<Option<TrackMetadata>, TrackReadError> {
            let tracks = self
                .tracks
                .lock()
                .map_err(|_| RepositoryError::Message("lock error".to_owned()))?;
            Ok(tracks.get(id).cloned())
        }
    }

    fn sample_track(id: &str, branch: Option<&str>) -> TrackMetadata {
        let task_id = domain::TaskId::new("T1").unwrap();
        let task = TrackTask::new(task_id.clone(), "Implement feature").unwrap();
        let section = PlanSection::new("S1", "Build", Vec::new(), vec![task_id]).unwrap();
        let plan = PlanView::new(Vec::new(), vec![section]);
        TrackMetadata::with_branch(
            TrackId::new(id).unwrap(),
            branch.map(|b| TrackBranch::new(b).unwrap()),
            "Test Track",
            vec![task],
            plan,
            None,
        )
        .unwrap()
    }

    #[test]
    fn test_reject_branchless_guard_allows_todo_target() {
        let reader = StubReader::default();
        let id = TrackId::new("test").unwrap();
        let result = reject_branchless_guard(&reader, &id, "todo", 3);
        assert!(result.is_ok());
    }

    #[test]
    fn test_reject_branchless_guard_rejects_branchless_v3() {
        let reader = StubReader::default();
        let track = sample_track("test", None);
        reader.tracks.lock().unwrap().insert(track.id().clone(), track);

        let id = TrackId::new("test").unwrap();
        let result = reject_branchless_guard(&reader, &id, "in_progress", 3);
        assert!(result.unwrap_err().contains("not activated yet"));
    }

    #[test]
    fn test_reject_branchless_guard_allows_materialized_v3() {
        let reader = StubReader::default();
        let track = sample_track("test", Some("track/test"));
        reader.tracks.lock().unwrap().insert(track.id().clone(), track);

        let id = TrackId::new("test").unwrap();
        let result = reject_branchless_guard(&reader, &id, "in_progress", 3);
        assert!(result.is_ok());
    }

    #[test]
    fn test_reject_branchless_guard_returns_error_for_missing_track() {
        let reader = StubReader::default();
        let id = TrackId::new("missing").unwrap();
        let result = reject_branchless_guard(&reader, &id, "in_progress", 3);
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn test_reject_branchless_guard_error_message_contains_activate_guidance() {
        let reader = StubReader::default();
        let track = sample_track("test", None);
        reader.tracks.lock().unwrap().insert(track.id().clone(), track);

        let id = TrackId::new("test").unwrap();
        let err = reject_branchless_guard(&reader, &id, "done", 3).unwrap_err();
        assert!(err.contains("/track:activate test"), "expected activate guidance in: {err}");
    }

    #[test]
    fn test_reject_branchless_allows_done_with_branch_on_v3() {
        let reader = StubReader::default();
        let track = sample_track("test", Some("track/test"));
        reader.tracks.lock().unwrap().insert(track.id().clone(), track);

        let id = TrackId::new("test").unwrap();
        assert!(reject_branchless_guard(&reader, &id, "done", 3).is_ok());
    }

    #[test]
    fn test_reject_branchless_allows_skipped_with_branch_on_v3() {
        let reader = StubReader::default();
        let track = sample_track("test", Some("track/test"));
        reader.tracks.lock().unwrap().insert(track.id().clone(), track);

        let id = TrackId::new("test").unwrap();
        assert!(reject_branchless_guard(&reader, &id, "skipped", 3).is_ok());
    }

    #[test]
    fn test_resolve_track_id_from_plan_branch_returns_error() {
        let result = resolve_track_id_from_branch(Some("plan/my-feature"));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not on a track branch"));
    }
}
