//! Track resolution and guard logic extracted from CLI layer.
//!
//! These functions contain business rules that belong in the use case layer
//! rather than CLI: track ID detection from branch names, task transition
//! resolution, and activation guard checks.

use domain::{CommitHash, TaskStatusKind, TaskTransition, TrackId};

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
        other => Err(format!("unsupported target status: {other}")),
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
    fn test_resolve_transition_with_unsupported_status_returns_error() {
        let result = resolve_transition("invalid", TaskStatusKind::Todo, None);
        assert!(result.unwrap_err().contains("unsupported target status"));
    }

    // --- reject_branchless_implementation_transition ---

    #[test]
    fn test_reject_branchless_allows_todo_target() {
        let id = TrackId::new("test").unwrap();
        let result = reject_branchless_implementation_transition(3, None, &id, "todo");
        assert!(result.is_ok());
    }

    #[test]
    fn test_reject_branchless_rejects_in_progress_on_branchless_v3() {
        let id = TrackId::new("test").unwrap();
        let result = reject_branchless_implementation_transition(3, None, &id, "in_progress");
        assert!(result.unwrap_err().contains("not activated yet"));
    }

    #[test]
    fn test_reject_branchless_allows_in_progress_with_branch() {
        let id = TrackId::new("test").unwrap();
        let result =
            reject_branchless_implementation_transition(3, Some("track/test"), &id, "in_progress");
        assert!(result.is_ok());
    }

    #[test]
    fn test_reject_branchless_allows_in_progress_on_v2() {
        let id = TrackId::new("test").unwrap();
        let result = reject_branchless_implementation_transition(2, None, &id, "in_progress");
        assert!(result.is_ok());
    }

    #[test]
    fn test_reject_branchless_rejects_done_on_branchless_v3() {
        let id = TrackId::new("test").unwrap();
        let result = reject_branchless_implementation_transition(3, None, &id, "done");
        assert!(result.unwrap_err().contains("not activated yet"));
    }

    #[test]
    fn test_reject_branchless_rejects_skipped_on_branchless_v3() {
        let id = TrackId::new("test").unwrap();
        let result = reject_branchless_implementation_transition(3, None, &id, "skipped");
        assert!(result.unwrap_err().contains("not activated yet"));
    }
}
