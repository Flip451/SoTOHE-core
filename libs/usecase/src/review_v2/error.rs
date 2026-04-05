use thiserror::Error;

use domain::review_v2::{ReviewReaderError, ScopeName};

/// Errors from the `Reviewer` usecase port.
#[derive(Debug, Error)]
pub enum ReviewerError {
    #[error("user aborted review")]
    UserAbort,
    #[error("reviewer process aborted")]
    ReviewerAbort,
    #[error("reviewer timed out")]
    Timeout,
    #[error("illegal verdict format from reviewer")]
    IllegalVerdict,
    #[error("unexpected reviewer error: {0}")]
    Unexpected(String),
}

/// Errors from the `DiffGetter` usecase port.
#[derive(Debug, Error)]
pub enum DiffGetError {
    #[error("diff operation failed: {0}")]
    Failed(String),
}

/// Errors from the `ReviewHasher` usecase port.
#[derive(Debug, Error)]
pub enum ReviewHasherError {
    #[error("hash computation failed: {0}")]
    Failed(String),
}

/// Errors from `ReviewCycle` orchestrator operations.
#[derive(Debug, Error)]
pub enum ReviewCycleError {
    #[error("unknown scope: {0}")]
    UnknownScope(ScopeName),
    #[error("file changed during review — before/after hash mismatch")]
    FileChangedDuringReview,
    #[error("diff error: {0}")]
    Diff(#[from] DiffGetError),
    #[error("hash error: {0}")]
    Hash(#[from] ReviewHasherError),
    #[error("reviewer error: {0}")]
    Reviewer(#[from] ReviewerError),
    #[error("review reader error: {0}")]
    Reader(#[from] ReviewReaderError),
}
