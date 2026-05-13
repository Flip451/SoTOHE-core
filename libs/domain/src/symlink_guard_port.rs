//! `SymlinkGuardPort` — secondary port for symlink rejection checks.
//!
//! Abstracts filesystem stat calls behind a domain port so that the usecase
//! layer can guard paths against symlinks without importing any infrastructure
//! crate (hexagonal-purity rule).

use std::path::Path;

use thiserror::Error;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Error returned by [`SymlinkGuardPort`] methods.
///
/// `Ok(())` means no symlink was found. `Err(SymlinkGuardError::SymlinkFound)`
/// means a symlink component was detected and the caller must abort the
/// operation. `Err(SymlinkGuardError::Io)` means a stat call failed for a
/// reason other than "not found" (permission denied, I/O error, etc.).
#[derive(Debug, Error)]
pub enum SymlinkGuardError {
    /// A symlink component was detected at the given path.
    #[error("symlink detected at path: {path}")]
    SymlinkFound {
        /// The symlink component that was found.
        path: String,
    },
    /// A stat call failed with an unexpected I/O error.
    #[error("symlink guard I/O error for path '{path}': {reason}")]
    Io {
        /// The path being checked.
        path: String,
        /// The underlying I/O error message.
        reason: String,
    },
}

// ---------------------------------------------------------------------------
// Port trait
// ---------------------------------------------------------------------------

/// Secondary port for symlink rejection.
///
/// Implementations perform `symlink_metadata` stat calls against the real
/// filesystem and live in the infrastructure layer. The usecase layer depends
/// only on this trait.
///
/// Both methods are **fail-closed**: a path component that does not exist yet
/// (`NotFound`) is silently skipped (it cannot be a symlink), a confirmed
/// symlink causes [`SymlinkGuardError::SymlinkFound`], and a stat failure for
/// *any other reason* (permission denied, I/O error, …) causes
/// [`SymlinkGuardError::Io`] — never silently skipped. A path that cannot be
/// fully verified is rejected, not accepted.
pub trait SymlinkGuardPort: Send + Sync {
    /// Rejects any symlink component in the full path from the filesystem root
    /// down to (and including) `path`.
    ///
    /// This is stricter than checking only the leaf: a symlink in any ancestor
    /// directory redirects all I/O beneath it. Intended for validating an
    /// untrusted root path (e.g. `workspace_root`) before it becomes the
    /// trusted anchor for subsequent path derivations.
    ///
    /// Empty path components are skipped.
    ///
    /// # Errors
    ///
    /// Returns [`SymlinkGuardError::SymlinkFound`] if any component of `path`
    /// is a confirmed symlink.
    /// Returns [`SymlinkGuardError::Io`] if a stat call fails with an
    /// unexpected I/O error.
    fn reject_symlinks_from_root(&self, path: &Path) -> Result<(), SymlinkGuardError>;

    /// Rejects any symlink component in the portion of `path` that lies
    /// **below** `trusted_root` (exclusive).
    ///
    /// `trusted_root` is assumed safe (e.g. it has already been validated by
    /// [`Self::reject_symlinks_from_root`]). Only components between
    /// `trusted_root` and `path` (inclusive of the leaf) are checked.
    ///
    /// # Errors
    ///
    /// Returns [`SymlinkGuardError::SymlinkFound`] if any component of `path`
    /// below `trusted_root` is a confirmed symlink.
    /// Returns [`SymlinkGuardError::Io`] if a stat call fails with an
    /// unexpected I/O error.
    fn reject_symlinks_below(
        &self,
        path: &Path,
        trusted_root: &Path,
    ) -> Result<(), SymlinkGuardError>;
}
