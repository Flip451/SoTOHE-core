//! Track resolution and guard logic extracted from CLI layer.
//!
//! These functions contain business rules that belong in the use case layer
//! rather than CLI: track ID detection from branch names, task transition
//! resolution, and activation guard checks.

use domain::{
    CommitHash, TaskStatus, TaskStatusKind, TaskTransition, TrackId, TrackReadError,
    ValidationError,
};
use thiserror::Error;

/// Errors returned by track resolution functions.
#[derive(Debug, Error)]
pub enum TrackResolutionError {
    #[error("detached HEAD; provide an explicit track-id")]
    DetachedHead,
    #[error("not on a track branch (on '{0}'); provide an explicit track-id")]
    NotTrackBranch(String),
    #[error("cannot determine current git branch; provide an explicit track-id")]
    NoBranch,
    #[error("invalid track id from branch '{0}': {1}")]
    InvalidTrackId(String, #[source] ValidationError),
    #[error("unsupported target status: {0}")]
    UnsupportedTargetStatus(String),
    #[error("track '{0}' not found")]
    TrackNotFound(String),
    #[error("{0}")]
    ReadError(#[from] TrackReadError),
}

/// Resolves a track ID from the current git branch name (strict mode).
///
/// Accepts only `track/<id>` branches. `plan/<id>` branches return
/// [`TrackResolutionError::NotTrackBranch`]. Use this for callers that must
/// fail closed on non-implementation branches (e.g., commit-time review
/// guard, post-commit hash persistence).
///
/// # Errors
/// Returns an error if the branch is not a `track/` branch,
/// is detached HEAD, or is `None`.
pub fn resolve_track_id_from_branch(branch: Option<&str>) -> Result<String, TrackResolutionError> {
    match branch {
        Some(b) => match b.strip_prefix("track/") {
            Some(slug) => {
                TrackId::try_new(slug)
                    .map_err(|e| TrackResolutionError::InvalidTrackId(slug.to_owned(), e))?;
                Ok(slug.to_owned())
            }
            None if b == "HEAD" => Err(TrackResolutionError::DetachedHead),
            None => Err(TrackResolutionError::NotTrackBranch(b.to_owned())),
        },
        None => Err(TrackResolutionError::NoBranch),
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
    current_status: &TaskStatus,
    commit_hash: Option<CommitHash>,
) -> Result<TaskTransition, TrackResolutionError> {
    match target_status {
        "in_progress" => match current_status.kind() {
            TaskStatusKind::Done => Ok(TaskTransition::Reopen),
            _ => Ok(TaskTransition::Start),
        },
        "done" => match (current_status, commit_hash) {
            (TaskStatus::DonePending, Some(hash)) => {
                Ok(TaskTransition::BackfillHash { commit_hash: hash })
            }
            (_, hash) => Ok(TaskTransition::Complete { commit_hash: hash }),
        },
        "todo" => Ok(TaskTransition::ResetToTodo),
        "skipped" => Ok(TaskTransition::Skip),
        other => Err(TrackResolutionError::UnsupportedTargetStatus(other.to_owned())),
    }
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
        assert!(matches!(result.unwrap_err(), TrackResolutionError::DetachedHead));
    }

    #[test]
    fn test_resolve_track_id_from_branch_with_non_track_branch_returns_error() {
        let result = resolve_track_id_from_branch(Some("main"));
        assert!(matches!(result.unwrap_err(), TrackResolutionError::NotTrackBranch(_)));
    }

    #[test]
    fn test_resolve_track_id_from_branch_with_none_returns_error() {
        let result = resolve_track_id_from_branch(None);
        assert!(matches!(result.unwrap_err(), TrackResolutionError::NoBranch));
    }

    // --- resolve_transition ---

    #[test]
    fn test_resolve_transition_todo_to_in_progress_returns_start() {
        let result = resolve_transition("in_progress", &TaskStatus::Todo, None);
        assert!(matches!(result.unwrap(), TaskTransition::Start));
    }

    #[test]
    fn test_resolve_transition_done_to_in_progress_returns_reopen() {
        let result = resolve_transition("in_progress", &TaskStatus::DonePending, None);
        assert!(matches!(result.unwrap(), TaskTransition::Reopen));
    }

    #[test]
    fn test_resolve_transition_to_done_returns_complete() {
        let hash = CommitHash::try_new("abc1234").unwrap();
        let result = resolve_transition("done", &TaskStatus::InProgress, Some(hash));
        assert!(matches!(result.unwrap(), TaskTransition::Complete { .. }));
    }

    #[test]
    fn test_resolve_transition_to_todo_returns_reset() {
        let result = resolve_transition("todo", &TaskStatus::InProgress, None);
        assert!(matches!(result.unwrap(), TaskTransition::ResetToTodo));
    }

    #[test]
    fn test_resolve_transition_to_skipped_returns_skip() {
        let result = resolve_transition("skipped", &TaskStatus::Todo, None);
        assert!(matches!(result.unwrap(), TaskTransition::Skip));
    }

    #[test]
    fn test_resolve_transition_with_unsupported_status_returns_raw_token() {
        let result = resolve_transition("invalid", &TaskStatus::Todo, None);
        assert!(matches!(
            result.unwrap_err(),
            TrackResolutionError::UnsupportedTargetStatus(ref s) if s == "invalid"
        ));
    }

    #[test]
    fn test_resolve_transition_done_pending_with_hash_returns_backfill() {
        let hash = CommitHash::try_new("abc1234").unwrap();
        let result = resolve_transition("done", &TaskStatus::DonePending, Some(hash));
        assert!(matches!(result.unwrap(), TaskTransition::BackfillHash { .. }));
    }

    #[test]
    fn test_resolve_transition_done_pending_without_hash_returns_complete() {
        let result = resolve_transition("done", &TaskStatus::DonePending, None);
        assert!(matches!(result.unwrap(), TaskTransition::Complete { commit_hash: None }));
    }

    #[test]
    fn test_resolve_transition_done_traced_with_hash_returns_complete() {
        let existing = CommitHash::try_new("aabbcc1").unwrap();
        let new_hash = CommitHash::try_new("ddeeff2").unwrap();
        let result = resolve_transition(
            "done",
            &TaskStatus::DoneTraced { commit_hash: existing },
            Some(new_hash),
        );
        // Domain layer will reject this (DoneTraced + Complete is invalid),
        // but resolve_transition returns Complete — domain enforces the guard.
        assert!(matches!(result.unwrap(), TaskTransition::Complete { .. }));
    }

    // --- TrackId validation tests ---

    #[test]
    fn test_resolve_track_id_from_branch_with_invalid_slug_returns_invalid_track_id() {
        let result = resolve_track_id_from_branch(Some("track/Not Valid"));
        assert!(matches!(result.unwrap_err(), TrackResolutionError::InvalidTrackId(..)));
    }

    #[test]
    fn test_resolve_track_id_from_branch_with_empty_suffix_returns_invalid_track_id() {
        let result = resolve_track_id_from_branch(Some("track/"));
        assert!(matches!(result.unwrap_err(), TrackResolutionError::InvalidTrackId(..)));
    }

    #[test]
    fn test_resolve_track_id_from_plan_branch_returns_error() {
        let result = resolve_track_id_from_branch(Some("plan/my-feature"));
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), TrackResolutionError::NotTrackBranch(_)));
    }
}
