//! Track resolution and guard logic extracted from CLI layer.
//!
//! These functions contain business rules that belong in the use case layer
//! rather than CLI: track ID detection from branch names, task transition
//! resolution, and activation guard checks.

use std::sync::Arc;

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

// ── BranchReadError ───────────────────────────────────────────────────────────

/// Error type for [`BranchReaderPort::current_branch`].
///
/// `ReadFailed` carries a free-text diagnostic string — opaque infrastructure
/// failure message, no domain concept requiring value-object treatment.
#[derive(Debug, Error)]
pub enum BranchReadError {
    #[error("branch read failed: {0}")]
    ReadFailed(String),
}

// ── BranchReaderPort ──────────────────────────────────────────────────────────

/// Secondary port for reading the current git branch name.
///
/// Returns `Some("HEAD")` for detached HEAD when the underlying `GitRepository`
/// reports that sentinel, `Some(branch_name)` for named branches, and `None`
/// only when no branch name can be determined. Infrastructure's `SystemGitRepo`
/// implements this port. Declared in usecase (CN-04: usecase must not depend on
/// infra; port inverts dependency). The `current_branch` return value is a raw
/// `Option<String>` — the branch name is an opaque VCS string, not a validated
/// domain concept at this boundary; callers (`resolve_track_id_from_branch`)
/// apply the `track/` validation on top, including mapping `Some("HEAD")` to
/// `DetachedHead`.
pub trait BranchReaderPort: Send + Sync {
    /// Returns the current git branch name.
    ///
    /// # Errors
    ///
    /// Returns [`BranchReadError::ReadFailed`] if the underlying git operation
    /// cannot complete (I/O error, git not found, etc.).
    fn current_branch(&self) -> Result<Option<String>, BranchReadError>;
}

// ── ActiveTrackResolveError ───────────────────────────────────────────────────

/// Error type for [`ActiveTrackResolveService`].
///
/// Aggregates branch-read failures ([`BranchReadError`]) and resolution
/// failures ([`TrackResolutionError`]) from `resolve_track_id_from_branch`.
/// `BranchRead` and `Resolution` carry free-text / nested error types — no
/// domain concept requiring value-object treatment.
#[derive(Debug, Error)]
pub enum ActiveTrackResolveError {
    #[error("branch read error: {0}")]
    BranchRead(#[from] BranchReadError),
    #[error("track resolution error: {0}")]
    Resolution(#[from] TrackResolutionError),
}

// ── ActiveTrackResolveService ─────────────────────────────────────────────────

/// Application service trait for resolving the active track id from the current
/// git branch.
///
/// The single shared resolution path (IN-04, D2). CLI drives this as the
/// composition root; tests inject a stub [`BranchReaderPort`]. Returns the
/// track-id string (opaque slug, caller converts to `TrackId` if needed).
pub trait ActiveTrackResolveService: Send + Sync {
    /// Resolves the active track id from the current git branch.
    ///
    /// # Errors
    ///
    /// Returns [`ActiveTrackResolveError::BranchRead`] if the branch cannot be
    /// read, or [`ActiveTrackResolveError::Resolution`] if the branch is not a
    /// valid `track/<id>` branch (e.g. `main`, detached HEAD, or `None`).
    fn resolve_active_track(&self) -> Result<String, ActiveTrackResolveError>;
}

// ── ActiveTrackResolveInteractor ──────────────────────────────────────────────

/// Concrete struct implementing [`ActiveTrackResolveService`].
///
/// Holds a [`BranchReaderPort`] injection for testability (git I/O stays in
/// infrastructure; usecase is pure). Delegates to
/// `resolve_track_id_from_branch` for the branch → track-id parse rule (IN-05).
pub struct ActiveTrackResolveInteractor {
    branch_reader: Arc<dyn BranchReaderPort>,
}

impl ActiveTrackResolveInteractor {
    /// Creates a new interactor with the given branch reader port.
    #[must_use]
    pub fn new(branch_reader: Arc<dyn BranchReaderPort>) -> Self {
        Self { branch_reader }
    }
}

impl ActiveTrackResolveService for ActiveTrackResolveInteractor {
    fn resolve_active_track(&self) -> Result<String, ActiveTrackResolveError> {
        let branch = self.branch_reader.current_branch()?;
        let track_id = resolve_track_id_from_branch(branch.as_deref())?;
        Ok(track_id)
    }
}

// ── Free functions ─────────────────────────────────────────────────────────────

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

    // --- ActiveTrackResolveInteractor ---

    /// Stub BranchReaderPort that returns a fixed value.
    struct StubBranchReader {
        value: Result<Option<String>, BranchReadError>,
    }

    impl StubBranchReader {
        fn ok(branch: impl Into<Option<String>>) -> Self {
            Self { value: Ok(branch.into()) }
        }

        fn err(msg: impl Into<String>) -> Self {
            Self { value: Err(BranchReadError::ReadFailed(msg.into())) }
        }
    }

    impl BranchReaderPort for StubBranchReader {
        fn current_branch(&self) -> Result<Option<String>, BranchReadError> {
            match &self.value {
                Ok(v) => Ok(v.clone()),
                Err(BranchReadError::ReadFailed(msg)) => {
                    Err(BranchReadError::ReadFailed(msg.clone()))
                }
            }
        }
    }

    #[test]
    fn test_active_track_resolve_interactor_with_track_branch_resolves_track_id() {
        // (a) track/<id> branch resolves correctly to the track id slug.
        let reader = Arc::new(StubBranchReader::ok(Some("track/my-feature-2026".to_owned())));
        let interactor = ActiveTrackResolveInteractor::new(reader);
        let result = interactor.resolve_active_track().unwrap();
        assert_eq!(result, "my-feature-2026");
    }

    #[test]
    fn test_active_track_resolve_interactor_with_main_branch_returns_not_track_branch_error() {
        // (b) main branch (non-track branch) returns NotTrackBranch wrapped in Resolution.
        let reader = Arc::new(StubBranchReader::ok(Some("main".to_owned())));
        let interactor = ActiveTrackResolveInteractor::new(reader);
        let err = interactor.resolve_active_track().unwrap_err();
        assert!(
            matches!(
                err,
                ActiveTrackResolveError::Resolution(TrackResolutionError::NotTrackBranch(_))
            ),
            "expected Resolution(NotTrackBranch), got: {err}"
        );
    }

    #[test]
    fn test_active_track_resolve_interactor_with_detached_head_returns_detached_head_error() {
        // (c) detached HEAD (Some("HEAD") from current_branch) returns DetachedHead
        // wrapped in Resolution.
        let reader = Arc::new(StubBranchReader::ok(Some("HEAD".to_owned())));
        let interactor = ActiveTrackResolveInteractor::new(reader);
        let err = interactor.resolve_active_track().unwrap_err();
        assert!(
            matches!(err, ActiveTrackResolveError::Resolution(TrackResolutionError::DetachedHead)),
            "expected Resolution(DetachedHead), got: {err}"
        );
    }

    #[test]
    fn test_active_track_resolve_interactor_with_none_branch_returns_no_branch_error() {
        // (d) None from current_branch returns NoBranch wrapped in Resolution.
        let reader = Arc::new(StubBranchReader::ok(None));
        let interactor = ActiveTrackResolveInteractor::new(reader);
        let err = interactor.resolve_active_track().unwrap_err();
        assert!(
            matches!(err, ActiveTrackResolveError::Resolution(TrackResolutionError::NoBranch)),
            "expected Resolution(NoBranch), got: {err}"
        );
    }

    #[test]
    fn test_active_track_resolve_interactor_with_read_error_returns_branch_read_error() {
        // (e) BranchReadError::ReadFailed from the port propagates as BranchRead variant.
        let reader = Arc::new(StubBranchReader::err("git not found"));
        let interactor = ActiveTrackResolveInteractor::new(reader);
        let err = interactor.resolve_active_track().unwrap_err();
        assert!(
            matches!(err, ActiveTrackResolveError::BranchRead(BranchReadError::ReadFailed(_))),
            "expected BranchRead(ReadFailed), got: {err}"
        );
    }
}
