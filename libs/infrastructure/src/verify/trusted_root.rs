//! Resolves the absolute `trusted_root` for the spec-states symlink guard.
//!
//! `reject_symlinks_below` treats `trusted_root` as inherently safe and
//! does NOT stat it — so the caller must supply a path that is:
//! 1. An absolute lexical ancestor of the spec path (otherwise the guard
//!    walks to the filesystem root and can trip over host-level symlinks
//!    like `/var` on macOS).
//! 2. Itself not a symlink (so the guard does not silently pass a
//!    symlinked parent directory as the root).
//!
//! This module provides the resolution policy:
//! - Prefer `SystemGitRepo::discover()` (authoritative repo root) when it
//!   succeeds AND the spec path is lexically under the discovered repo.
//! - Otherwise walk up from the spec path looking for a `.git` marker
//!   (standalone repo checkout).
//! - Otherwise fall back to `spec_path.parent()` (standalone / non-repo
//!   use case).
//! - Always verify the final selection is not itself a symlink; return
//!   `io::Error` if it is.
//!
//! The CLI `VerifyCommand::SpecStates` handler delegates to
//! [`resolve_trusted_root`] to get a single `Result<PathBuf, io::Error>`,
//! keeping filesystem I/O and path resolution out of the CLI layer.
//!
//! Reference: ADR `knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md`
//! §D4.3, §D5.3, §D5.4.

use std::path::{Path, PathBuf};

use crate::git_cli::{GitRepository, SystemGitRepo};

/// Resolves the trusted_root to use for `spec_states::verify` /
/// `verify_from_spec_json` given a `spec_path`.
///
/// Resolution order:
/// 1. `SystemGitRepo::discover()` + lexical containment check: if the
///    absolutized spec path is under the discovered repo root, use that.
/// 2. `.git` walk-up from `spec_path.parent()`: find the nearest repo
///    ancestor.
/// 3. `spec_path.parent()`: last resort, with an explicit symlink check.
///
/// All paths returned are verified non-symlink via `ensure_not_symlink_root`.
///
/// # Errors
///
/// Returns `io::Error::InvalidInput` when the selected trusted_root is
/// itself a symlink (this can happen when the spec path is under a
/// symlinked parent and no `.git` marker is found). The caller should
/// map this to a clear user-facing error.
pub fn resolve_trusted_root(spec_path: &Path) -> Result<PathBuf, std::io::Error> {
    let spec_abs = absolutize(spec_path);

    // Priority 1: authoritative repo root, if the spec is under it.
    if let Ok(repo) = SystemGitRepo::discover() {
        let repo_root = repo.root().to_path_buf();
        if spec_abs.starts_with(&repo_root) {
            return ensure_not_symlink_root(repo_root);
        }
    }

    // Priority 2 / 3: walk up from the spec path.
    fallback_trusted_root(spec_path)
}

/// Normalizes `path` to an absolute path **without following symlinks**.
///
/// Uses `std::env::current_dir()` as the base for relative paths. If
/// `current_dir()` fails, falls back to the original path. Does NOT
/// canonicalize — callers that need symlink detection must preserve the
/// original link structure.
pub(crate) fn absolutize(path: &Path) -> PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }
    std::env::current_dir().map(|cwd| cwd.join(path)).unwrap_or_else(|_| path.to_path_buf())
}

/// Standalone / fallback `trusted_root` resolution: walks up from
/// `spec_path.parent()` looking for a `.git` marker, then verifies the
/// result is not a symlink.
pub(crate) fn fallback_trusted_root(spec_path: &Path) -> Result<PathBuf, std::io::Error> {
    let start = spec_path.parent().unwrap_or(spec_path);
    for ancestor in start.ancestors() {
        if ancestor.join(".git").exists() {
            return ensure_not_symlink_root(ancestor.to_path_buf());
        }
    }
    ensure_not_symlink_root(start.to_path_buf())
}

/// Verifies that the selected `trusted_root` is itself not a symlink.
///
/// Non-existent paths are accepted (they will produce a clearer
/// "file not found" error downstream when the caller attempts to read
/// the spec file).
pub(crate) fn ensure_not_symlink_root(path: PathBuf) -> Result<PathBuf, std::io::Error> {
    match path.symlink_metadata() {
        Ok(meta) if meta.file_type().is_symlink() => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("refusing to use symlinked trusted_root: {}", path.display()),
        )),
        _ => Ok(path),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn test_absolutize_preserves_absolute_paths() {
        let abs = PathBuf::from("/tmp/foo/bar.json");
        assert_eq!(absolutize(&abs), abs);
    }

    #[test]
    fn test_absolutize_expands_relative_paths_against_cwd() {
        let rel = Path::new("track/items/foo/spec.json");
        let result = absolutize(rel);
        assert!(result.is_absolute(), "absolutize should return an absolute path: {result:?}");
        assert!(result.ends_with("track/items/foo/spec.json"));
    }

    #[test]
    fn test_ensure_not_symlink_root_accepts_regular_dir() {
        let dir = tempfile::tempdir().unwrap();
        let result = ensure_not_symlink_root(dir.path().to_path_buf());
        assert!(result.is_ok(), "regular dir must be accepted: {result:?}");
    }

    #[test]
    fn test_ensure_not_symlink_root_accepts_nonexistent_path() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("does-not-exist");
        let result = ensure_not_symlink_root(missing);
        assert!(result.is_ok(), "nonexistent path must be accepted (downstream read will fail)");
    }

    #[cfg(unix)]
    #[test]
    fn test_ensure_not_symlink_root_rejects_symlinked_dir() {
        let dir = tempfile::tempdir().unwrap();
        let real_dir = dir.path().join("real");
        std::fs::create_dir(&real_dir).unwrap();
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&real_dir, &link).unwrap();

        let result = ensure_not_symlink_root(link);
        assert!(result.is_err(), "symlinked dir must be rejected");
        let err = result.unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
        assert!(err.to_string().contains("symlinked trusted_root"));
    }

    #[test]
    fn test_fallback_trusted_root_finds_git_marker() {
        // Create: tempdir/project/.git, tempdir/project/sub/spec.json
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        std::fs::create_dir_all(project.join(".git")).unwrap();
        let sub = project.join("sub");
        std::fs::create_dir(&sub).unwrap();
        let spec = sub.join("spec.json");
        std::fs::write(&spec, "{}").unwrap();

        let result = fallback_trusted_root(&spec).unwrap();
        assert_eq!(result, project, "should find the .git-marked project root");
    }

    #[test]
    fn test_fallback_trusted_root_uses_parent_when_no_git() {
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("spec.json");
        std::fs::write(&spec, "{}").unwrap();

        let result = fallback_trusted_root(&spec).unwrap();
        assert_eq!(result, dir.path(), "should fall back to spec parent when no .git marker");
    }

    #[cfg(unix)]
    #[test]
    fn test_fallback_trusted_root_rejects_symlinked_parent_when_no_git() {
        // tempdir/real/, tempdir/link -> real, spec at tempdir/link/spec.json
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real");
        std::fs::create_dir(&real).unwrap();
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let spec_via_link = link.join("spec.json");
        // Note: writing via the link creates the file in real/, but the
        // parent component as seen from spec_via_link IS the symlink.
        std::fs::write(&spec_via_link, "{}").unwrap();

        let result = fallback_trusted_root(&spec_via_link);
        assert!(result.is_err(), "symlinked parent must be rejected: {result:?}");
    }
}
