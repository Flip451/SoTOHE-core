use std::collections::HashMap;

use super::error::{CommitHashError, ReviewReaderError, ReviewWriterError};
use super::types::{FastVerdict, ReviewHash, ScopeName, Verdict};
use crate::CommitHash;

/// Read-only port for review.json v2 retrieval.
///
/// Reads the latest final verdict and hash for each scope.
/// Persistence port — defined in domain layer.
pub trait ReviewReader: Send + Sync {
    /// Reads the latest final verdict and hash for all scopes.
    ///
    /// Returns an empty map if review.json does not exist or has no rounds.
    ///
    /// # Errors
    /// Returns `ReviewReaderError` on I/O or codec failure.
    fn read_latest_finals(
        &self,
    ) -> Result<HashMap<ScopeName, (Verdict, ReviewHash)>, ReviewReaderError>;
}

/// Write port for review.json v2 persistence.
///
/// Persistence port — defined in domain layer.
/// Implementations should use `fs4::lock_exclusive` for concurrent write safety.
pub trait ReviewWriter: Send + Sync {
    /// Appends a final verdict (with findings) and hash to a scope's round history.
    ///
    /// # Errors
    /// Returns `ReviewWriterError` on I/O or codec failure.
    fn write_verdict(
        &self,
        scope: &ScopeName,
        verdict: &Verdict,
        hash: &ReviewHash,
    ) -> Result<(), ReviewWriterError>;

    /// Appends a fast verdict (with findings) and hash to a scope's round history.
    ///
    /// # Errors
    /// Returns `ReviewWriterError` on I/O or codec failure.
    fn write_fast_verdict(
        &self,
        scope: &ScopeName,
        verdict: &FastVerdict,
        hash: &ReviewHash,
    ) -> Result<(), ReviewWriterError>;

    /// Creates a new review.json (track initialization).
    ///
    /// Use with `CommitHashWriter::clear()` to reset diff base to main.
    ///
    /// # Errors
    /// Returns `ReviewWriterError` on I/O failure.
    fn init(&self) -> Result<(), ReviewWriterError>;

    /// Archives existing review.json and creates a new one (review restart).
    ///
    /// Does NOT clear `.commit_hash` — diff base is maintained so the next
    /// review cycle uses incremental scope from the last committed point.
    ///
    /// # Errors
    /// Returns `ReviewWriterError` on I/O failure.
    fn reset(&self) -> Result<(), ReviewWriterError>;
}

/// Read-only port for `.commit_hash` file.
///
/// Persistence port — defined in domain layer.
/// Returns the stored commit hash used as diff base for incremental review scope.
pub trait CommitHashReader: Send + Sync {
    /// Reads the commit hash from `.commit_hash`.
    ///
    /// Returns `Ok(None)` if the file does not exist (fallback to main).
    /// Returns `Err` if the file content is not a valid commit hash.
    ///
    /// Note: infrastructure implementations may additionally perform ancestry
    /// validation (`git merge-base --is-ancestor`), returning `None` on failure
    /// (fail-closed for scope expansion). This is an infra implementation detail
    /// and not part of the trait contract.
    ///
    /// # Errors
    /// Returns `CommitHashError` on I/O or format failure.
    fn read(&self) -> Result<Option<CommitHash>, CommitHashError>;
}

/// Write port for `.commit_hash` file.
///
/// Persistence port — defined in domain layer.
pub trait CommitHashWriter: Send + Sync {
    /// Writes a commit hash to `.commit_hash` using atomic write (tmp + rename).
    ///
    /// Called after successful commit with `git rev-parse HEAD`.
    ///
    /// # Errors
    /// Returns `CommitHashError` on I/O failure.
    fn write(&self, hash: &CommitHash) -> Result<(), CommitHashError>;

    /// Deletes `.commit_hash` (track initialization).
    ///
    /// If the file does not exist, this is a no-op (success).
    ///
    /// # Errors
    /// Returns `CommitHashError` on I/O failure.
    fn clear(&self) -> Result<(), CommitHashError>;
}
