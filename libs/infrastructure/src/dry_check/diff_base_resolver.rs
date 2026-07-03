//! Filesystem-backed adapter for [`usecase::fixpoint_resolve::DiffBaseResolverPort`].
//!
//! Relocated from `cli_composition::track::fixpoint_resolve` per ADR
//! 2026-06-21-1328 D7 (secondary port implementations belong in `libs/infrastructure`).
//!
//! Three-branch fail-closed diff-base resolution:
//!
//! 1. `FsDryCheckCommitHashStore::read()` → `Ok(Some(hash))`: use it.
//! 2. `Ok(None)` (file absent or non-ancestor): fall back to
//!    `git rev-parse <base_branch>`.
//! 3. `Err(...)`: emit `[warn]` and fall back to `git rev-parse <base_branch>`
//!    (absorbed; must not abort the gate).
//!
//! `base_branch` is the configured base branch name (e.g. `"main"`, `"develop"`)
//! taken from `metadata.json#branch_strategy_snapshot.base_branch`. It is passed
//! as a separate process argument (argv-style) and never interpolated into a
//! shell command string (AC-04 command-boundary safety).

use std::path::{Path, PathBuf};

use domain::CommitHash;
use usecase::fixpoint_resolve::{DiffBaseResolverError, DiffBaseResolverPort};

use crate::dry_check::{DryCheckCommitHashError, FsDryCheckCommitHashStore};
use crate::git_cli::{GitRepository, SystemGitRepo};

/// Filesystem adapter implementing [`DiffBaseResolverPort`].
///
/// Owns the configured base branch (from the active track's
/// `branch_strategy_snapshot`) and reads `.commit_hash` from the track directory,
/// falling back to `git rev-parse <base_branch>` per the three-branch policy.
pub struct FsDiffBaseResolverAdapter {
    base_branch: String,
}

impl FsDiffBaseResolverAdapter {
    /// Construct with the configured base branch.
    #[must_use]
    pub fn new(base_branch: String) -> Self {
        Self { base_branch }
    }
}

impl DiffBaseResolverPort for FsDiffBaseResolverAdapter {
    fn resolve_diff_base(
        &self,
        track_dir: &Path,
        canonical_root: &Path,
        repo_root: &Path,
    ) -> Result<CommitHash, DiffBaseResolverError> {
        let commit_hash_path = trusted_commit_hash_path(track_dir, canonical_root)
            .map_err(DiffBaseResolverError::Unavailable)?;
        let store = FsDryCheckCommitHashStore::new(commit_hash_path, canonical_root.to_path_buf());
        match store.read() {
            Ok(Some(hash)) => return Ok(hash),
            Ok(None) => {}
            Err(DryCheckCommitHashError::Format(detail)) => {
                eprintln!(
                    "[warn] fixpoint-resolve: malformed .commit_hash ({detail}); \
                     falling back to {}",
                    self.base_branch
                );
            }
            Err(other) => {
                eprintln!(
                    "[warn] fixpoint-resolve: failed to read .commit_hash ({other}); \
                     falling back to {}",
                    self.base_branch
                );
            }
        }

        git_rev_parse_base(repo_root, &self.base_branch).map_err(DiffBaseResolverError::Unavailable)
    }
}

fn trusted_commit_hash_path(track_dir: &Path, trusted_root: &Path) -> Result<PathBuf, String> {
    let canonical_root = trusted_root.canonicalize().map_err(|e| {
        format!("cannot canonicalize trusted root '{}': {e}", trusted_root.display())
    })?;
    let absolute_track_dir = if track_dir.is_absolute() {
        track_dir.to_path_buf()
    } else {
        canonical_root.join(track_dir)
    };
    let canonical_track_dir = absolute_track_dir
        .canonicalize()
        .map_err(|e| format!("cannot canonicalize track dir '{}': {e}", track_dir.display()))?;

    if !canonical_track_dir.starts_with(&canonical_root) {
        return Err(format!(
            "track dir '{}' resolves outside trusted root '{}'",
            track_dir.display(),
            canonical_root.display()
        ));
    }

    let commit_hash_path = absolute_track_dir.join(".commit_hash");
    crate::track::symlink_guard::reject_symlinks_below(&commit_hash_path, &canonical_root)
        .map_err(|e| {
            format!("symlink guard on commit hash path '{}': {e}", commit_hash_path.display())
        })?;

    Ok(canonical_track_dir.join(".commit_hash"))
}

/// Run `git rev-parse <base_branch>` from `repo_root` and return the resulting
/// `CommitHash`. `base_branch` is passed argv-style (AC-04).
fn git_rev_parse_base(repo_root: &Path, base_branch: &str) -> Result<CommitHash, String> {
    let git = SystemGitRepo::discover_from(repo_root).map_err(|e| format!("git discover: {e}"))?;
    let output = git
        .output(&["rev-parse", base_branch])
        .map_err(|e| format!("git rev-parse {base_branch}: {e}"))?;
    if !output.status.success() {
        return Err(format!("git rev-parse {base_branch} failed"));
    }
    let sha = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    CommitHash::try_new(&sha).map_err(|e| format!("invalid {base_branch} SHA: {e}"))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_trusted_commit_hash_path_when_track_dir_is_under_root_returns_commit_hash_path() {
        let root = tempfile::tempdir().unwrap();
        let track_dir = root.path().join("track").join("items").join("track-a");
        std::fs::create_dir_all(&track_dir).unwrap();

        let path = trusted_commit_hash_path(&track_dir, root.path()).unwrap();

        assert_eq!(path, track_dir.canonicalize().unwrap().join(".commit_hash"));
    }

    #[test]
    fn test_trusted_commit_hash_path_when_track_dir_escapes_root_returns_error() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let track_dir = outside.path().join("track-a");
        std::fs::create_dir_all(&track_dir).unwrap();

        let err = trusted_commit_hash_path(&track_dir, root.path()).unwrap_err();

        assert!(err.contains("resolves outside trusted root"), "got: {err}");
    }

    #[cfg(unix)]
    #[test]
    fn test_trusted_commit_hash_path_when_track_dir_is_symlink_returns_error() {
        let root = tempfile::tempdir().unwrap();
        let real_track_dir = root.path().join("real-track");
        let symlink_track_dir = root.path().join("track-link");
        std::fs::create_dir_all(&real_track_dir).unwrap();
        std::os::unix::fs::symlink(&real_track_dir, &symlink_track_dir).unwrap();

        let err = trusted_commit_hash_path(&symlink_track_dir, root.path()).unwrap_err();

        assert!(err.contains("symlink guard"), "got: {err}");
    }
}
