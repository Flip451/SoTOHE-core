//! Track resolution and guard logic extracted from CLI layer.
//!
//! These functions contain business rules that belong in the use case layer
//! rather than CLI: track ID detection from branch names, task transition
//! resolution, and activation guard checks.

use domain::{
    CommitHash, TaskStatus, TaskStatusKind, TaskTransition, TrackId, TrackReadError, TrackReader,
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
    #[error("track '{0}' is not activated yet; run /track:activate {0}")]
    NotActivated(String),
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

/// Resolves a track ID from the current git branch name (lenient mode).
///
/// Accepts both `track/<id>` and `plan/<id>` branches and returns the bare
/// track id in either case. Use this for callers where plan-phase branches
/// should also be auto-detected (e.g., view sync, where rendering plan.md
/// from a `plan/<id>` checkout is meaningful even before activation).
///
/// The id segment is validated through [`TrackId::try_new`] so an invalid
/// slug returns [`TrackResolutionError::InvalidTrackId`] for both prefixes.
///
/// # Errors
/// Returns an error if the branch is neither a `track/` nor a `plan/`
/// branch, is detached HEAD, or is `None`.
pub fn resolve_track_or_plan_id_from_branch(
    branch: Option<&str>,
) -> Result<String, TrackResolutionError> {
    match branch {
        Some(b) => {
            let stripped = b.strip_prefix("track/").or_else(|| b.strip_prefix("plan/"));
            match stripped {
                Some(slug) => {
                    TrackId::try_new(slug)
                        .map_err(|e| TrackResolutionError::InvalidTrackId(slug.to_owned(), e))?;
                    Ok(slug.to_owned())
                }
                None if b == "HEAD" => Err(TrackResolutionError::DetachedHead),
                None => Err(TrackResolutionError::NotTrackBranch(b.to_owned())),
            }
        }
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

/// Rejects implementation-phase task transitions on branchless (planning-only) tracks.
///
/// If the target status is an implementation status (`in_progress`, `done`, `skipped`)
/// and the track is a v3 or v4 (or newer) track without an activated branch, this
/// returns an error directing the user to activate the track first.
///
/// Schema versions 1 and 2 predate the activation workflow and are allowed through
/// unconditionally (legacy compatibility).
///
/// # Errors
/// Returns an error message if the transition is blocked by the activation guard.
pub fn reject_branchless_implementation_transition(
    schema_version: u32,
    branch: Option<&str>,
    track_id: &TrackId,
    target_status: &str,
) -> Result<(), TrackResolutionError> {
    if !matches!(target_status, "in_progress" | "done" | "skipped") {
        return Ok(());
    }

    if schema_version >= 3 && branch.is_none() {
        return Err(TrackResolutionError::NotActivated(track_id.to_string()));
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
) -> Result<(), TrackResolutionError> {
    if !matches!(target_status, "in_progress" | "done" | "skipped") {
        return Ok(());
    }
    let track = reader
        .find(track_id)?
        .ok_or_else(|| TrackResolutionError::TrackNotFound(track_id.to_string()))?;
    reject_branchless_implementation_transition(
        schema_version,
        track.branch().map(|b| b.as_ref()),
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

    // --- resolve_track_or_plan_id_from_branch (lenient) ---

    #[test]
    fn test_resolve_track_or_plan_id_accepts_track_branch() {
        let result = resolve_track_or_plan_id_from_branch(Some("track/my-feature"));
        assert_eq!(result.unwrap(), "my-feature");
    }

    #[test]
    fn test_resolve_track_or_plan_id_accepts_plan_branch() {
        // Lenient mode: plan/<id> resolves to the same bare id as track/<id>.
        let result = resolve_track_or_plan_id_from_branch(Some("plan/my-feature"));
        assert_eq!(result.unwrap(), "my-feature");
    }

    #[test]
    fn test_resolve_track_or_plan_id_rejects_main_branch() {
        let result = resolve_track_or_plan_id_from_branch(Some("main"));
        assert!(matches!(result.unwrap_err(), TrackResolutionError::NotTrackBranch(_)));
    }

    #[test]
    fn test_resolve_track_or_plan_id_rejects_detached_head() {
        let result = resolve_track_or_plan_id_from_branch(Some("HEAD"));
        assert!(matches!(result.unwrap_err(), TrackResolutionError::DetachedHead));
    }

    #[test]
    fn test_resolve_track_or_plan_id_rejects_none() {
        let result = resolve_track_or_plan_id_from_branch(None);
        assert!(matches!(result.unwrap_err(), TrackResolutionError::NoBranch));
    }

    #[test]
    fn test_resolve_track_or_plan_id_validates_slug_for_plan_branch() {
        // The id segment after `plan/` must still pass TrackId validation —
        // the lenient mode only relaxes the prefix, not the slug rules.
        let result = resolve_track_or_plan_id_from_branch(Some("plan/Not Valid"));
        assert!(matches!(result.unwrap_err(), TrackResolutionError::InvalidTrackId(_, _)));
    }

    #[test]
    fn test_resolve_track_or_plan_id_rejects_empty_plan_suffix() {
        let result = resolve_track_or_plan_id_from_branch(Some("plan/"));
        assert!(matches!(result.unwrap_err(), TrackResolutionError::InvalidTrackId(_, _)));
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

    use rstest::rstest;

    // --- reject_branchless_implementation_transition ---

    #[rstest]
    #[case::todo_target_is_allowed(3, None, "todo", true)]
    #[case::in_progress_on_branchless_v3_is_rejected(3, None, "in_progress", false)]
    #[case::in_progress_on_branchless_v4_is_rejected(4, None, "in_progress", false)]
    #[case::in_progress_with_branch_is_allowed(3, Some("track/test"), "in_progress", true)]
    #[case::in_progress_on_v4_with_branch_is_allowed(4, Some("track/test"), "in_progress", true)]
    #[case::in_progress_on_v2_is_allowed(2, None, "in_progress", true)]
    #[case::done_on_branchless_v3_is_rejected(3, None, "done", false)]
    #[case::done_on_branchless_v4_is_rejected(4, None, "done", false)]
    #[case::skipped_on_branchless_v3_is_rejected(3, None, "skipped", false)]
    #[case::skipped_on_branchless_v4_is_rejected(4, None, "skipped", false)]
    fn test_reject_branchless_implementation_transition(
        #[case] schema_version: u32,
        #[case] branch: Option<&str>,
        #[case] target_status: &str,
        #[case] expect_ok: bool,
    ) {
        let id = TrackId::try_new("test").unwrap();
        let result =
            reject_branchless_implementation_transition(schema_version, branch, &id, target_status);
        if expect_ok {
            assert!(result.is_ok());
        } else {
            assert!(matches!(result.unwrap_err(), TrackResolutionError::NotActivated(_)));
        }
    }

    // --- reject_branchless_guard (with TrackReader) ---

    use std::collections::HashMap;
    use std::sync::Mutex;

    use domain::{
        RepositoryError, TrackBranch, TrackId, TrackMetadata, TrackReadError, TrackReader,
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
        // Identity-only TrackMetadata; status derived on demand.
        TrackMetadata::with_branch(
            TrackId::try_new(id).unwrap(),
            branch.map(|b| TrackBranch::try_new(b).unwrap()),
            "Test Track",
            None,
        )
        .unwrap()
    }

    #[test]
    fn test_reject_branchless_guard_allows_todo_target() {
        let reader = StubReader::default();
        let id = TrackId::try_new("test").unwrap();
        let result = reject_branchless_guard(&reader, &id, "todo", 3);
        assert!(result.is_ok());
    }

    #[test]
    fn test_reject_branchless_guard_rejects_branchless_v3() {
        let reader = StubReader::default();
        let track = sample_track("test", None);
        reader.tracks.lock().unwrap().insert(track.id().clone(), track);

        let id = TrackId::try_new("test").unwrap();
        let result = reject_branchless_guard(&reader, &id, "in_progress", 3);
        assert!(matches!(result.unwrap_err(), TrackResolutionError::NotActivated(_)));
    }

    #[test]
    fn test_reject_branchless_guard_allows_materialized_v3() {
        let reader = StubReader::default();
        let track = sample_track("test", Some("track/test"));
        reader.tracks.lock().unwrap().insert(track.id().clone(), track);

        let id = TrackId::try_new("test").unwrap();
        let result = reject_branchless_guard(&reader, &id, "in_progress", 3);
        assert!(result.is_ok());
    }

    #[test]
    fn test_reject_branchless_guard_returns_error_for_missing_track() {
        let reader = StubReader::default();
        let id = TrackId::try_new("missing").unwrap();
        let result = reject_branchless_guard(&reader, &id, "in_progress", 3);
        assert!(matches!(result.unwrap_err(), TrackResolutionError::TrackNotFound(_)));
    }

    #[test]
    fn test_reject_branchless_guard_error_message_contains_activate_guidance() {
        let reader = StubReader::default();
        let track = sample_track("test", None);
        reader.tracks.lock().unwrap().insert(track.id().clone(), track);

        let id = TrackId::try_new("test").unwrap();
        let err = reject_branchless_guard(&reader, &id, "done", 3).unwrap_err();
        assert!(
            err.to_string().contains("/track:activate test"),
            "expected activate guidance in: {err}"
        );
    }

    #[test]
    fn test_reject_branchless_allows_done_with_branch_on_v3() {
        let reader = StubReader::default();
        let track = sample_track("test", Some("track/test"));
        reader.tracks.lock().unwrap().insert(track.id().clone(), track);

        let id = TrackId::try_new("test").unwrap();
        assert!(reject_branchless_guard(&reader, &id, "done", 3).is_ok());
    }

    #[test]
    fn test_reject_branchless_allows_skipped_with_branch_on_v3() {
        let reader = StubReader::default();
        let track = sample_track("test", Some("track/test"));
        reader.tracks.lock().unwrap().insert(track.id().clone(), track);

        let id = TrackId::try_new("test").unwrap();
        assert!(reject_branchless_guard(&reader, &id, "skipped", 3).is_ok());
    }

    // --- T003: TrackId validation tests (written first — Red phase) ---

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

    struct FailingReader;

    impl TrackReader for FailingReader {
        fn find(&self, _id: &TrackId) -> Result<Option<TrackMetadata>, TrackReadError> {
            Err(RepositoryError::Message("reader I/O failure".to_owned()).into())
        }
    }

    #[test]
    fn test_reject_branchless_guard_propagates_read_error() {
        let reader = FailingReader;
        let id = TrackId::try_new("test").unwrap();
        let result = reject_branchless_guard(&reader, &id, "in_progress", 3);
        assert!(matches!(result.unwrap_err(), TrackResolutionError::ReadError(_)));
    }
}
