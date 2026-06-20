//! `FsSymlinkGuard` — filesystem-backed [`SymlinkGuardPort`] adapter.
//!
//! Implements the domain [`SymlinkGuardPort`] trait by performing real
//! `symlink_metadata` stat calls. This adapter lives in the infrastructure
//! layer; the usecase layer only depends on the domain trait.

use std::path::Path;

use domain::{SymlinkGuardError, SymlinkGuardPort};

use super::symlink_guard::reject_symlinks_below as infra_reject_symlinks_below;

// ---------------------------------------------------------------------------
// Adapter
// ---------------------------------------------------------------------------

/// Filesystem-backed implementation of [`SymlinkGuardPort`].
///
/// Reuses [`crate::track::symlink_guard::reject_symlinks_below`] for the
/// "below trusted_root" check. For the "from root" check it walks all
/// ancestors (root → leaf) and checks each with `symlink_metadata`.
#[derive(Debug, Default)]
pub struct FsSymlinkGuard;

impl FsSymlinkGuard {
    /// Creates a new `FsSymlinkGuard`.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl SymlinkGuardPort for FsSymlinkGuard {
    /// Rejects any symlink component in the full path from the filesystem root
    /// down to (and including) `path`.
    ///
    /// Relative paths are absolutized against `std::env::current_dir()` so
    /// `path.ancestors()` walks the full filesystem-root → leaf chain instead
    /// of just the relative components (which would skip every symlink in the
    /// CWD's actual ancestors). Ancestors are collected from the absolutized
    /// path (leaf → root) and reversed so we walk root → leaf. Empty
    /// components are skipped. Components that cannot be stat'd are silently
    /// skipped — only confirmed symlinks trigger an error.
    ///
    /// This method intentionally stays separate from the below-`trusted_root`
    /// helper: that helper assumes the caller has already vetted the root and
    /// only checks descendants, while this port method must establish its own
    /// absolute boundary and scan from the filesystem root for callers that do
    /// not have a trusted root to pass in.
    ///
    /// ## Trust model — accepted deviation
    ///
    /// `current_dir()` (POSIX `getcwd()`) returns the *physical* absolute path,
    /// which resolves symlinks in the CWD chain. A process launched from a
    /// symlinked workspace will therefore not see the symlink-in-the-CWD
    /// itself, only its physical target ancestry. This is an accepted
    /// limitation: this guard is a defensive layer against accidental
    /// catalogue / baseline redirection, not a hardened mitigation against
    /// adversaries with shell access. An adversary who can change the process
    /// CWD can also override `$PWD`, set custom `LD_PRELOAD`, or redirect I/O
    /// via many other mechanisms — symlink-in-CWD is one of dozens of such
    /// vectors. A robust CWD-symlink detector would need either (a)
    /// `O_NOFOLLOW` + inode verification (future hardening) or (b) a
    /// higher-layer trusted-root contract (composition root passes a vetted
    /// absolute path).
    ///
    /// # Errors
    ///
    /// Returns [`SymlinkGuardError::SymlinkFound`] if any ancestor or the leaf
    /// itself is a symlink.
    /// Returns [`SymlinkGuardError::Io`] if a stat call fails with an
    /// unexpected I/O error or `current_dir()` lookup fails.
    fn reject_symlinks_from_root(&self, path: &Path) -> Result<(), SymlinkGuardError> {
        let absolute_path: std::path::PathBuf = if path.is_absolute() {
            path.to_path_buf()
        } else {
            let cwd = std::env::current_dir().map_err(|e| SymlinkGuardError::Io {
                path: path.display().to_string(),
                reason: format!("cannot determine current directory to absolutize: {e}"),
            })?;
            cwd.join(path)
        };

        // This traversal stays local to the port adapter because callers do not
        // provide a vetted `trusted_root`; the shared below-root helper has a
        // different boundary contract.
        // Collect all ancestors from root down to the absolutized path (inclusive).
        let mut components: Vec<&Path> = absolute_path.ancestors().collect();
        // `ancestors()` yields leaf → root; reverse to root → leaf.
        components.reverse();

        for component in &components {
            if component.as_os_str().is_empty() {
                continue;
            }
            match component.symlink_metadata() {
                Ok(meta) if meta.file_type().is_symlink() => {
                    return Err(SymlinkGuardError::SymlinkFound {
                        path: component.display().to_string(),
                    });
                }
                Ok(_) => {}
                // Skip components that don't exist yet or can't be stat'd.
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => {
                    return Err(SymlinkGuardError::Io {
                        path: component.display().to_string(),
                        reason: e.to_string(),
                    });
                }
            }
        }
        Ok(())
    }

    /// Rejects any symlink component in `path` below `trusted_root`
    /// (exclusive).
    ///
    /// Delegates to [`crate::track::symlink_guard::reject_symlinks_below`].
    /// Converts `std::io::Error` "symlink detected" results into
    /// [`SymlinkGuardError::SymlinkFound`] and other I/O errors into
    /// [`SymlinkGuardError::Io`].
    ///
    /// # Errors
    ///
    /// Returns [`SymlinkGuardError::SymlinkFound`] if any component of `path`
    /// below `trusted_root` is a symlink.
    /// Returns [`SymlinkGuardError::Io`] if an unexpected I/O error occurs.
    ///
    /// Classification caveat: the shared `track::symlink_guard::reject_symlinks_below`
    /// reports a detected symlink as `std::io::Error` of kind `InvalidInput`, but
    /// `symlink_metadata` can also fail with `InvalidInput` for other reasons (most
    /// notably a path containing an embedded NUL byte). This adapter therefore maps
    /// *every* `InvalidInput` to `SymlinkFound`, so such a malformed-path failure is
    /// reported as `SymlinkFound` rather than `Io`. This is acceptable here: the call
    /// sites only pass paths derived from validated trusted catalogue/baseline
    /// filenames (no NUL bytes), and both outcomes are "reject". Giving the shared
    /// helper a structured error type that distinguishes "symlink found" from
    /// "stat failed" is part of the `symlink_guard` consolidation follow-up.
    fn reject_symlinks_below(
        &self,
        path: &Path,
        trusted_root: &Path,
    ) -> Result<(), SymlinkGuardError> {
        match infra_reject_symlinks_below(path, trusted_root) {
            // leaf exists, no symlinks found
            Ok(_exists) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::InvalidInput => {
                // infra's reject_symlinks_below uses InvalidInput for symlink
                // rejection; see the classification caveat above.
                Err(SymlinkGuardError::SymlinkFound { path: path.display().to_string() })
            }
            Err(e) => Err(SymlinkGuardError::Io {
                path: path.display().to_string(),
                reason: e.to_string(),
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // reject_symlinks_from_root tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_reject_symlinks_from_root_regular_dir_passes() {
        let dir = tempfile::tempdir().unwrap();
        let guard = FsSymlinkGuard::new();
        // A real directory should not be rejected.
        guard.reject_symlinks_from_root(dir.path()).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn test_reject_symlinks_from_root_symlinked_path_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let real_dir = tmp.path().join("real");
        std::fs::create_dir_all(&real_dir).unwrap();
        let link_dir = tmp.path().join("link");
        std::os::unix::fs::symlink(&real_dir, &link_dir).unwrap();

        let guard = FsSymlinkGuard::new();
        let err = guard.reject_symlinks_from_root(&link_dir).unwrap_err();
        assert!(
            matches!(err, SymlinkGuardError::SymlinkFound { .. }),
            "expected SymlinkFound, got: {err:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_reject_symlinks_from_root_stat_failure_is_io_not_skipped() {
        // A path component longer than NAME_MAX makes `symlink_metadata` fail with
        // a non-`NotFound` error; the guard must surface it as `Io` (fail-closed),
        // never silently skip it.
        let dir = tempfile::tempdir().unwrap();
        let bad = dir.path().join("a".repeat(512));

        let guard = FsSymlinkGuard::new();
        let err = guard.reject_symlinks_from_root(&bad).unwrap_err();
        assert!(matches!(err, SymlinkGuardError::Io { .. }), "expected Io, got: {err:?}");
    }

    // -------------------------------------------------------------------------
    // reject_symlinks_below tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_reject_symlinks_below_regular_file_passes() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("test.json");
        std::fs::write(&file, "{}").unwrap();

        let guard = FsSymlinkGuard::new();
        guard.reject_symlinks_below(&file, dir.path()).unwrap();
    }

    #[test]
    fn test_reject_symlinks_below_nonexistent_passes() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("missing.json");

        let guard = FsSymlinkGuard::new();
        // A missing file is not a symlink — should not error.
        guard.reject_symlinks_below(&file, dir.path()).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn test_reject_symlinks_below_symlink_leaf_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("real.json");
        std::fs::write(&target, "{}").unwrap();
        let link = dir.path().join("link.json");
        std::os::unix::fs::symlink(&target, &link).unwrap();

        let guard = FsSymlinkGuard::new();
        let err = guard.reject_symlinks_below(&link, dir.path()).unwrap_err();
        assert!(
            matches!(err, SymlinkGuardError::SymlinkFound { .. }),
            "expected SymlinkFound, got: {err:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_reject_symlinks_below_symlink_parent_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let real_sub = dir.path().join("real-sub");
        std::fs::create_dir_all(&real_sub).unwrap();
        std::fs::write(real_sub.join("test.json"), "{}").unwrap();

        let link_sub = dir.path().join("link-sub");
        std::os::unix::fs::symlink(&real_sub, &link_sub).unwrap();

        let guard = FsSymlinkGuard::new();
        let err = guard.reject_symlinks_below(&link_sub.join("test.json"), dir.path()).unwrap_err();
        assert!(
            matches!(err, SymlinkGuardError::SymlinkFound { .. }),
            "expected SymlinkFound, got: {err:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_reject_symlinks_below_stat_failure_is_io_not_skipped() {
        // A path component longer than NAME_MAX makes the underlying
        // `symlink_metadata` fail with a non-`NotFound`, non-`InvalidInput` error;
        // the guard must surface it as `Io` (fail-closed), never silently skip it.
        let dir = tempfile::tempdir().unwrap();
        let bad = dir.path().join("a".repeat(512));

        let guard = FsSymlinkGuard::new();
        let err = guard.reject_symlinks_below(&bad, dir.path()).unwrap_err();
        assert!(matches!(err, SymlinkGuardError::Io { .. }), "expected Io, got: {err:?}");
    }
}
