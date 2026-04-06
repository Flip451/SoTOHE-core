use domain::CommitHash;
use domain::review_v2::{FastVerdict, FilePath, LogInfo, ReviewHash, ReviewTarget, Verdict};

use super::error::{DiffGetError, ReviewHasherError, ReviewerError};

/// Usecase port for the external reviewer (e.g., Codex).
pub trait Reviewer {
    /// Performs a final review on the given target files.
    ///
    /// # Errors
    /// Returns `ReviewerError` on abort, timeout, or illegal verdict format.
    fn review(&self, target: &ReviewTarget) -> Result<(Verdict, LogInfo), ReviewerError>;

    /// Performs a fast (advisory) review on the given target files.
    ///
    /// Fast verdicts are not used for approval decisions.
    ///
    /// # Errors
    /// Returns `ReviewerError` on abort, timeout, or illegal verdict format.
    fn fast_review(&self, target: &ReviewTarget) -> Result<(FastVerdict, LogInfo), ReviewerError>;
}

/// Usecase port for obtaining the list of changed files (diff).
///
/// Infrastructure implementation uses git merge-base + 4-source union.
pub trait DiffGetter {
    /// Lists all changed files relative to the given base commit.
    ///
    /// Returns the union of: committed diff from merge-base, staged, unstaged,
    /// and untracked files.
    ///
    /// # Errors
    /// Returns `DiffGetError` on git operation failure.
    fn list_diff_files(&self, base: &CommitHash) -> Result<Vec<FilePath>, DiffGetError>;
}

/// Usecase port for computing review hashes from file contents.
///
/// Infrastructure implementation uses sorted manifest + SHA256 + O_NOFOLLOW.
pub trait ReviewHasher {
    /// Computes the hash of a review target's file contents.
    ///
    /// Empty targets return `ReviewHash::Empty`.
    ///
    /// # Errors
    /// Returns `ReviewHasherError` on I/O or computation failure.
    fn calc(&self, target: &ReviewTarget) -> Result<ReviewHash, ReviewHasherError>;
}
