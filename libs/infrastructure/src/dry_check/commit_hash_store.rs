//! `FsDryCheckCommitHashStore` — dry-check's own filesystem adapter for
//! reading the per-track `.commit_hash` file to resolve the diff base.
//!
//! CN-01: this is NOT `FsCommitHashStore` from `review_v2` — it is a
//! dry-check-owned adapter with its own error type [`DryCheckCommitHashError`].
//! Behavior mirrors `FsCommitHashStore::read()`.

use std::path::PathBuf;

use domain::CommitHash;
use thiserror::Error;

use crate::git_cli::{GitRepository, SystemGitRepo};
use crate::track::symlink_guard::reject_symlinks_below;

// ── DryCheckCommitHashError ───────────────────────────────────────────────────

/// Error from [`FsDryCheckCommitHashStore::read`].
///
/// CN-01: independent of `domain::review_v2::CommitHashError`.
/// Three failure modes: I/O error, symlink detected, or invalid hash format.
/// An absent file returns `Ok(None)` — there is no `NotFound` variant.
#[derive(Debug, Error)]
pub enum DryCheckCommitHashError {
    /// File system I/O failure.
    #[error("dry-check commit hash I/O error: {path}: {detail}")]
    Io {
        /// The file path involved.
        path: String,
        /// Human-readable description of the failure.
        detail: String,
    },
    /// The target path is a symlink (rejected for security).
    #[error("dry-check commit hash: symlink detected at {path}")]
    SymlinkDetected {
        /// The symlink path.
        path: String,
    },
    /// The stored content is not a valid commit hash.
    #[error("dry-check commit hash: invalid hash format: {0}")]
    Format(String),
}

// ── FsDryCheckCommitHashStore ─────────────────────────────────────────────────

/// Filesystem adapter for reading the per-track `.commit_hash` file used by the
/// dry-check gate to resolve the diff base.
///
/// Three outcomes from [`read`](Self::read):
/// 1. File absent → `Ok(None)` (main-tip fallback).
/// 2. Stored content is not a valid `CommitHash` → `Err(DryCheckCommitHashError::Format)`.
/// 3. Hash is not an ancestor of HEAD → `Ok(None)` (fail-closed, main-tip fallback).
///
/// The fail-closed policy (absorbing `Err(Format)` and falling through to
/// `git rev-parse main`) is applied by the CLI composition layer (T007/T009),
/// not here.
#[derive(Debug)]
pub struct FsDryCheckCommitHashStore {
    path: PathBuf,
    trusted_root: PathBuf,
}

impl FsDryCheckCommitHashStore {
    /// Construct a new [`FsDryCheckCommitHashStore`].
    #[must_use]
    pub fn new(path: PathBuf, trusted_root: PathBuf) -> FsDryCheckCommitHashStore {
        Self { path, trusted_root }
    }

    /// Read the stored commit hash, validate its format, and check ancestry.
    ///
    /// # Errors
    ///
    /// - `Err(DryCheckCommitHashError::SymlinkDetected)` if the path is a symlink.
    /// - `Err(DryCheckCommitHashError::Io)` on I/O errors other than `NotFound`.
    /// - `Err(DryCheckCommitHashError::Format)` if the file content is not a
    ///   valid `CommitHash`.
    ///
    /// Returns `Ok(None)` when the file is absent or the hash is not an ancestor
    /// of HEAD (fail-closed).
    pub fn read(&self) -> Result<Option<CommitHash>, DryCheckCommitHashError> {
        let path_str = self.path.display().to_string();

        // Symlink check before reading.
        reject_symlinks_below(&self.path, &self.trusted_root).map_err(|e| {
            if e.kind() == std::io::ErrorKind::InvalidInput {
                DryCheckCommitHashError::SymlinkDetected { path: path_str.clone() }
            } else {
                DryCheckCommitHashError::Io { path: path_str.clone(), detail: e.to_string() }
            }
        })?;

        let content = match std::fs::read_to_string(&self.path) {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // File absent → Ok(None) (main-tip fallback).
                return Ok(None);
            }
            Err(e) => {
                return Err(DryCheckCommitHashError::Io {
                    path: path_str,
                    detail: format!("read: {e}"),
                });
            }
        };

        let trimmed = content.trim();

        // Validate hash format.
        let hash = CommitHash::try_new(trimmed).map_err(|e| {
            DryCheckCommitHashError::Format(format!(
                "invalid commit hash in {}: {e}",
                self.path.display()
            ))
        })?;

        // Ancestry check: is the stored hash an ancestor of HEAD?
        match SystemGitRepo::discover() {
            Ok(git) => {
                let output = git.output(&["merge-base", "--is-ancestor", trimmed, "HEAD"]);
                match output {
                    Ok(o) if o.status.success() => Ok(Some(hash)),
                    // Not an ancestor or any error → fail-closed (main fallback).
                    _ => Ok(None),
                }
            }
            // git unavailable → fail-closed.
            Err(_) => Ok(None),
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn store_in_dir(dir: &tempfile::TempDir, filename: &str) -> FsDryCheckCommitHashStore {
        let path = dir.path().join(filename);
        FsDryCheckCommitHashStore::new(path, dir.path().to_owned())
    }

    #[test]
    fn test_read_returns_ok_none_when_file_absent() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in_dir(&dir, ".commit_hash");
        let result = store.read().unwrap();
        assert!(result.is_none(), "absent file should return Ok(None)");
    }

    #[test]
    fn test_read_returns_err_format_when_content_is_invalid_hash() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".commit_hash");
        // Write content that is not a valid commit hash.
        std::fs::write(&path, "not-a-valid-hash\n").unwrap();
        let store = FsDryCheckCommitHashStore::new(path, dir.path().to_owned());
        let result = store.read();
        assert!(
            matches!(result, Err(DryCheckCommitHashError::Format(_))),
            "invalid hash should return Err(Format), got: {result:?}"
        );
    }

    /// For a valid ancestor hash, `SystemGitRepo::discover()` is used. In a
    /// test environment, the hash may not be a real ancestor, so the function
    /// returns `Ok(None)` (non-ancestor → fail-closed). This tests the format
    /// validation passes and the ancestry step is reached.
    #[test]
    fn test_read_with_valid_hash_format_reaches_ancestry_check() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".commit_hash");
        // Write a syntactically valid hash (7 lowercase hex chars).
        std::fs::write(&path, "abc1234\n").unwrap();
        let store = FsDryCheckCommitHashStore::new(path, dir.path().to_owned());
        // Returns Ok(None) because `abc1234` is not an ancestor in the real repo,
        // OR returns Ok(Some) if it happens to be. Either is valid here.
        let result = store.read();
        assert!(result.is_ok(), "valid hash format should not return Err(Format), got: {result:?}");
    }

    #[cfg(unix)]
    #[test]
    fn test_read_returns_symlink_detected_for_symlink_path() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real_hash");
        std::fs::write(&real, "abc1234\n").unwrap();
        let link = dir.path().join(".commit_hash");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let store = FsDryCheckCommitHashStore::new(link, dir.path().to_owned());
        let result = store.read();
        assert!(
            matches!(result, Err(DryCheckCommitHashError::SymlinkDetected { .. })),
            "symlink should return Err(SymlinkDetected), got: {result:?}"
        );
    }
}
