//! Driven ports the task-operation use cases read the outside world through
//! (spec `IN-22`, `AC-28`, `AC-29`).
//!
//! Re-exported from the parent module, whose type contract this file is part
//! of; it is a separate file only so that neither module outgrows the
//! workspace module-size limit.
//!
//! The port here is synchronous: the callers are single-shot CLI transitions
//! with no concurrency and no ambient async runtime on this path, so the
//! blocking call is the deliberate boundary choice rather than an unstated
//! default.

use std::path::Path;

use domain::{CommitHash, FreeText};
use thiserror::Error;

// ── CommitRecordVerifyError ───────────────────────────────────────────────────

/// Why a commit hash may not be recorded against a task (`IN-22`, `AC-28`).
///
/// The two rejections and the read failure are separate variants because the
/// caller answers them differently: a hash the repository refuses is a bad
/// record to reject, while a repository that could not be read is an inability
/// to judge. Folding them together would leave the caller unable to tell a
/// refused record from an unperformed check, which is exactly the distinction
/// that keeps an unverified hash from being written anyway.
///
/// The rejections carry the hash they are about, so the operator sees which
/// value was refused rather than only that something was.
#[derive(Debug, Error)]
pub enum CommitRecordVerifyError {
    /// The hash is well-formed but names no commit object in this repository.
    #[error("the commit {commit_hash} does not exist in this repository")]
    CommitNotFound {
        /// The hash that was verified.
        commit_hash: CommitHash,
    },
    /// The commit exists but is not reachable from `HEAD`.
    #[error("the commit {commit_hash} is not an ancestor of HEAD")]
    NotAncestorOfHead {
        /// The hash that was verified.
        commit_hash: CommitHash,
    },
    /// The repository could not be read, so no verdict could be produced.
    #[error("the repository could not be read: {message}")]
    RepositoryUnreadable {
        /// Opaque adapter diagnostic.
        message: FreeText,
    },
}

// ── CommitRecordVerifierPort ──────────────────────────────────────────────────

/// Verifies that a commit hash may be recorded against a task of the track
/// under `items_dir` (`IN-22`, `AC-28`, `AC-29`).
///
/// No verdict type is minted: the acceptance carries nothing beyond itself, so
/// `Ok(())` is the whole of it and every refusal is an
/// [`CommitRecordVerifyError`] variant. Which repository the track lives in is
/// the adapter's to resolve from `items_dir`, so the use case holds no path
/// arithmetic and no git knowledge.
pub trait CommitRecordVerifierPort: Send + Sync {
    /// Verifies `commit_hash` against the repository holding `items_dir`.
    ///
    /// # Errors
    ///
    /// Returns [`CommitRecordVerifyError::CommitNotFound`] when the hash names
    /// no commit object, [`CommitRecordVerifyError::NotAncestorOfHead`] when
    /// the commit exists but is unreachable from `HEAD`, and
    /// [`CommitRecordVerifyError::RepositoryUnreadable`] when the repository
    /// could not be read at all.
    fn verify_commit_record(
        &self,
        items_dir: &Path,
        commit_hash: &CommitHash,
    ) -> Result<(), CommitRecordVerifyError>;
}
