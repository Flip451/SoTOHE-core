//! Diff types: `DiffHunkRange` and `DiffFileHunks`.

use thiserror::Error;

use crate::review_v2::types::FilePath;

// ── DiffHunkRange ─────────────────────────────────────────────────────────────

/// A 1-indexed inclusive line range `[start_line, end_line]` for a single
/// added/changed hunk from `git diff`.
///
/// The invariant `start <= end` (both >= 1) makes the empty/inverted-range
/// and zero-line states unrepresentable at construction time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffHunkRange {
    start_line: u32,
    end_line: u32,
}

impl DiffHunkRange {
    /// Construct a [`DiffHunkRange`].
    ///
    /// # Errors
    ///
    /// Returns [`DiffHunkRangeError::ZeroLine`] when `start_line` or `end_line` is 0.
    /// Returns [`DiffHunkRangeError::StartExceedsEnd`] when `start_line > end_line`.
    pub fn new(start_line: u32, end_line: u32) -> Result<DiffHunkRange, DiffHunkRangeError> {
        if start_line == 0 || end_line == 0 {
            return Err(DiffHunkRangeError::ZeroLine);
        }
        if start_line > end_line {
            return Err(DiffHunkRangeError::StartExceedsEnd { start: start_line, end: end_line });
        }
        Ok(DiffHunkRange { start_line, end_line })
    }

    /// Return the 1-indexed start line (inclusive).
    pub fn start_line(&self) -> u32 {
        self.start_line
    }

    /// Return the 1-indexed end line (inclusive).
    pub fn end_line(&self) -> u32 {
        self.end_line
    }
}

/// Error from [`DiffHunkRange::new`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DiffHunkRangeError {
    /// `start_line > end_line` (empty or inverted range).
    #[error("start_line ({start}) exceeds end_line ({end}) — range must be non-empty")]
    StartExceedsEnd {
        /// The rejected start line.
        start: u32,
        /// The rejected end line.
        end: u32,
    },
    /// A line number is 0 (line numbers are 1-indexed).
    #[error("line numbers must be >= 1 (got 0)")]
    ZeroLine,
}

// ── DiffFileHunks ─────────────────────────────────────────────────────────────

/// A changed file path with its non-empty list of added/changed hunk line ranges.
///
/// Returned by `DryCheckDiffSource::list_changed_hunks()`. The "hunks non-empty"
/// invariant is enforced in the constructor: a file with no added/changed hunks
/// is structurally absent from the result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffFileHunks {
    path: FilePath,
    hunks: Vec<DiffHunkRange>,
}

impl DiffFileHunks {
    /// Construct a [`DiffFileHunks`].
    ///
    /// # Errors
    ///
    /// Returns [`DiffFileHunksError::EmptyHunks`] when `hunks` is empty.
    pub fn new(
        path: FilePath,
        hunks: Vec<DiffHunkRange>,
    ) -> Result<DiffFileHunks, DiffFileHunksError> {
        if hunks.is_empty() {
            return Err(DiffFileHunksError::EmptyHunks);
        }
        Ok(DiffFileHunks { path, hunks })
    }

    /// Return the repo-relative file path.
    pub fn path(&self) -> &FilePath {
        &self.path
    }

    /// Return the non-empty list of hunk ranges (always >= 1 element).
    pub fn hunks(&self) -> &[DiffHunkRange] {
        &self.hunks
    }
}

/// Error from [`DiffFileHunks::new`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DiffFileHunksError {
    /// The hunk list is empty. A file with no added/changed hunks should be
    /// omitted from the result entirely.
    #[error("hunks list must not be empty")]
    EmptyHunks,
}
