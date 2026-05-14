//! `FsSpecFileLoader` — infrastructure adapter for [`SpecFileLoaderPort`].
//!
//! Reads `spec.json` from the filesystem via the symlink guard
//! so that callers in `apps/cli` never call `std::fs::read_to_string`
//! directly when loading spec files (hexagonal-purity rule).

use std::path::Path;

use domain::spec_file_loader_port::SpecFileLoadError;
pub use domain::spec_file_loader_port::SpecFileLoaderPort;

use crate::track::symlink_guard::reject_symlinks_below;

// ---------------------------------------------------------------------------
// FsSpecFileLoader
// ---------------------------------------------------------------------------

/// Stateless filesystem adapter implementing [`SpecFileLoaderPort`].
///
/// Applies the shared symlink guard before reading, then returns the raw
/// text content. Injected at the `apps/cli` composition root so the CLI
/// command no longer calls `std::fs` directly.
#[derive(Debug, Clone, Default)]
pub struct FsSpecFileLoader {
    /// Trusted root for the symlink guard (typically `items_dir`).
    trusted_root: std::path::PathBuf,
}

impl FsSpecFileLoader {
    /// Creates a new adapter with the given `trusted_root`.
    ///
    /// `trusted_root` is passed to [`reject_symlinks_below`] — only path
    /// components below it are inspected for symlinks.
    #[must_use]
    pub fn new(trusted_root: std::path::PathBuf) -> Self {
        Self { trusted_root }
    }
}

impl SpecFileLoaderPort for FsSpecFileLoader {
    /// Load the raw text content of `spec_path`, guarding against symlinks.
    ///
    /// # Errors
    ///
    /// Returns [`SpecFileLoadError`] when:
    /// - `trusted_root` itself is a symlink (which would allow it to redirect all reads),
    /// - a symlink is detected at `spec_path` or any ancestor below `trusted_root`,
    /// - the file does not exist or cannot be read.
    fn load(&self, spec_path: &Path) -> Result<String, SpecFileLoadError> {
        // Security: verify that trusted_root itself is not a symlink.
        // reject_symlinks_below trusts the root and only checks components below it,
        // so a symlinked root would bypass all subsequent path checks.
        match self.trusted_root.symlink_metadata() {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(SpecFileLoadError(format!(
                    "symlink guard: refusing to use symlinked trusted_root: '{}'",
                    self.trusted_root.display()
                )));
            }
            Ok(_) => {}
            Err(e) => {
                return Err(SpecFileLoadError(format!(
                    "symlink guard: cannot stat trusted_root '{}': {e}",
                    self.trusted_root.display()
                )));
            }
        }

        // Security: verify that spec_path is below trusted_root before delegating
        // to `reject_symlinks_below`. That helper only checks symlinks along the
        // ancestor chain; it does not verify containment, so paths with `..`
        // components (e.g. `<trusted_root>/../etc/passwd`) or absolute escapes
        // would pass the ancestor walk and be read.
        //
        // We reject any path that:
        //   (a) is not below trusted_root (no trusted_root ancestor found), or
        //   (b) contains `..` (ParentDir) components, which can escape the root
        //       even if trusted_root appears somewhere in the ancestor list.
        // This check is performed on the raw (un-canonicalized) path so it works
        // even when the file does not yet exist.
        let has_parent_dir = spec_path.components().any(|c| c == std::path::Component::ParentDir);
        let below_root = spec_path.ancestors().any(|a| a == self.trusted_root);
        if has_parent_dir || !below_root {
            return Err(SpecFileLoadError(format!(
                "symlink guard: spec_path '{}' is not safely below trusted_root '{}' \
                 (path traversal rejected)",
                spec_path.display(),
                self.trusted_root.display(),
            )));
        }

        match reject_symlinks_below(spec_path, &self.trusted_root) {
            Ok(true) => std::fs::read_to_string(spec_path).map_err(|e| {
                SpecFileLoadError(format!(
                    "cannot read spec.json at '{}': {e}",
                    spec_path.display()
                ))
            }),
            Ok(false) => {
                Err(SpecFileLoadError(format!("spec.json not found at '{}'", spec_path.display())))
            }
            Err(e) => Err(SpecFileLoadError(format!(
                "symlink guard: refusing to read spec.json at '{}': {e}",
                spec_path.display()
            ))),
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

    #[test]
    fn test_load_returns_content_for_regular_file() {
        let dir = tempfile::tempdir().unwrap();
        let spec_path = dir.path().join("spec.json");
        std::fs::write(&spec_path, r#"{"schema_version":2}"#).unwrap();

        let loader = FsSpecFileLoader::new(dir.path().to_path_buf());
        let content = loader.load(&spec_path).unwrap();
        assert!(content.contains("schema_version"));
    }

    #[test]
    fn test_load_returns_error_for_absent_file() {
        let dir = tempfile::tempdir().unwrap();
        let spec_path = dir.path().join("missing.json");

        let loader = FsSpecFileLoader::new(dir.path().to_path_buf());
        let err = loader.load(&spec_path).unwrap_err();
        assert!(err.0.contains("not found"), "expected not-found error, got: {err}");
    }

    #[cfg(unix)]
    #[test]
    fn test_load_rejects_symlink_at_spec_path() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real.json");
        let link = dir.path().join("spec.json");
        std::fs::write(&real, r#"{"schema_version":2}"#).unwrap();
        std::os::unix::fs::symlink(&real, &link).unwrap();

        let loader = FsSpecFileLoader::new(dir.path().to_path_buf());
        let err = loader.load(&link).unwrap_err();
        assert!(err.0.contains("symlink"), "expected symlink error, got: {err}");
    }
}
