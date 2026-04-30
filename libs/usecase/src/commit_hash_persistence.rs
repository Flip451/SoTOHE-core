//! Commit hash persistence application service (usecase layer).
//!
//! Wraps `domain::CommitHash` and `domain::review_v2::CommitHashWriter` so
//! the CLI never imports these domain types directly (CN-01 / D1). Returns
//! [`CommitHashPersistenceService`] results so `commands/make.rs` can persist
//! the HEAD SHA to `.commit_hash` without touching domain types.

use std::path::PathBuf;
use std::sync::Arc;

use thiserror::Error;

// ── CommitHashPersistenceError ────────────────────────────────────────────────

/// Error type for [`CommitHashPersistenceService`].
///
/// Wraps invalid track ID, git discover failures, branch mismatch, git
/// rev-parse failures, invalid SHA, and store write failures without leaking
/// `domain::CommitHash` or `domain::review_v2::CommitHashWriter` across the
/// usecase boundary.
#[derive(Debug, Error)]
pub enum CommitHashPersistenceError {
    #[error("invalid track ID: {0}")]
    InvalidTrackId(String),
    #[error("git discover failed: {0}")]
    GitDiscoverFailed(String),
    #[error("branch mismatch: {0}")]
    BranchMismatch(String),
    #[error("rev-parse failed: {0}")]
    RevParseFailed(String),
    #[error("invalid SHA: {0}")]
    InvalidSha(String),
    #[error("store write failed: {0}")]
    StoreWriteFailed(String),
    #[error("track dir missing: {0}")]
    TrackDirMissing(String),
}

// ── CommitHashPersistenceService ──────────────────────────────────────────────

/// Application service trait for persisting the HEAD SHA to `.commit_hash`
/// after a successful commit (`sotp make track-commit-message` post-commit step).
///
/// Driven by the CLI layer. Wraps `domain::CommitHash` and
/// `domain::review_v2::CommitHashWriter` so that `commands/make.rs` never
/// imports these domain types directly. Returns the persisted SHA as a `String`.
pub trait CommitHashPersistenceService: Send + Sync {
    /// Persists the HEAD SHA for the given track.
    ///
    /// # Errors
    ///
    /// Returns [`CommitHashPersistenceError`] on track ID validation, git, branch,
    /// SHA, or store write failures.
    fn persist(
        &self,
        track_id: String,
        workspace_root: PathBuf,
    ) -> Result<String, CommitHashPersistenceError>;
}

// ── CommitHashPersistenceInteractor ───────────────────────────────────────────

/// Concrete struct implementing [`CommitHashPersistenceService`].
///
/// Constructs domain types (`CommitHash`) internally and invokes
/// `CommitHashWriter` to persist the HEAD SHA without leaking domain types
/// to the CLI.
pub struct CommitHashPersistenceInteractor {
    run_fn:
        Arc<dyn Fn(String, PathBuf) -> Result<String, CommitHashPersistenceError> + Send + Sync>,
}

impl CommitHashPersistenceInteractor {
    /// Creates a new interactor with the given runner function.
    #[must_use]
    pub fn new(
        run_fn: Arc<
            dyn Fn(String, PathBuf) -> Result<String, CommitHashPersistenceError> + Send + Sync,
        >,
    ) -> Self {
        Self { run_fn }
    }
}

impl CommitHashPersistenceService for CommitHashPersistenceInteractor {
    fn persist(
        &self,
        track_id: String,
        workspace_root: PathBuf,
    ) -> Result<String, CommitHashPersistenceError> {
        (self.run_fn)(track_id, workspace_root)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_commit_hash_persistence_error_variants_exist() {
        let e1 = CommitHashPersistenceError::InvalidTrackId("bad".to_owned());
        assert!(matches!(e1, CommitHashPersistenceError::InvalidTrackId(_)));
        let e2 = CommitHashPersistenceError::GitDiscoverFailed("git".to_owned());
        assert!(matches!(e2, CommitHashPersistenceError::GitDiscoverFailed(_)));
        let e3 = CommitHashPersistenceError::BranchMismatch("mismatch".to_owned());
        assert!(matches!(e3, CommitHashPersistenceError::BranchMismatch(_)));
        let e4 = CommitHashPersistenceError::RevParseFailed("rev".to_owned());
        assert!(matches!(e4, CommitHashPersistenceError::RevParseFailed(_)));
        let e5 = CommitHashPersistenceError::InvalidSha("sha".to_owned());
        assert!(matches!(e5, CommitHashPersistenceError::InvalidSha(_)));
        let e6 = CommitHashPersistenceError::StoreWriteFailed("store".to_owned());
        assert!(matches!(e6, CommitHashPersistenceError::StoreWriteFailed(_)));
        let e7 = CommitHashPersistenceError::TrackDirMissing("dir".to_owned());
        assert!(matches!(e7, CommitHashPersistenceError::TrackDirMissing(_)));
    }

    #[test]
    fn test_commit_hash_persistence_interactor_delegates() {
        let run_fn = Arc::new(|_: String, _: PathBuf| {
            Ok("abc1234567890abcdef1234567890abcdef12345".to_owned())
        });
        let interactor = CommitHashPersistenceInteractor::new(run_fn);
        let sha = interactor.persist("my-track-2026".to_owned(), PathBuf::new()).unwrap();
        assert_eq!(sha.len(), 40);
    }
}
