//! `RustdocCrateAdapter` — infrastructure adapter for `RustdocCratePort`.
//!
//! - `load_from_path`: wraps `BaselineRustdocCodec::load` (B-side baseline).
//! - `capture_current`: wraps `RustdocSchemaExporter::export_rustdoc_json_path`
//!   + `BaselineRustdocCodec::from_json` (C-side live capture).
//!
//! `workspace_root` is passed to `RustdocSchemaExporter::new` so it knows
//! where to invoke `cargo +nightly rustdoc`.
//!
//! [source: ADR 2026-05-11-2330 §D2]

use std::path::{Path, PathBuf};

use domain::tddd::catalogue_v2::{RustdocCratePort, RustdocCratePortError};

use crate::schema_export::RustdocSchemaExporter;
use crate::tddd::baseline_rustdoc_codec::{BaselineRustdocCodec, BaselineRustdocCodecError};
use crate::track::symlink_guard::reject_symlinks_below;

// ---------------------------------------------------------------------------
// RustdocCrateAdapter
// ---------------------------------------------------------------------------

/// Adapter implementing [`RustdocCratePort`].
///
/// - `load_from_path` wraps `BaselineRustdocCodec::load` (B-side baseline).
/// - `capture_current` wraps `RustdocSchemaExporter::export_rustdoc_json_path`
///   + `BaselineRustdocCodec::from_json` (C-side live capture).
///
/// `workspace_root` is passed to `RustdocSchemaExporter::new`. Injected into
/// `CatalogueImplSignalsInteractor` at the `apps/cli` composition root.
///
/// [source: ADR 2026-05-11-2330 D2]
pub struct RustdocCrateAdapter {
    workspace_root: PathBuf,
}

impl RustdocCrateAdapter {
    /// Creates a new adapter for the given workspace root.
    #[must_use]
    pub fn new(workspace_root: PathBuf) -> Self {
        Self { workspace_root }
    }
}

impl RustdocCratePort for RustdocCrateAdapter {
    /// Loads a `rustdoc_types::Crate` from the given JSON file path (B-side baseline).
    ///
    /// # Errors
    ///
    /// Returns [`RustdocCratePortError::NotFound`] if the file is absent.
    ///
    /// Returns [`RustdocCratePortError::Io`] if a non-symlink I/O error occurs.
    ///
    /// Returns [`RustdocCratePortError::ParseFailed`] if JSON deserialization or
    /// format-version validation fails.
    fn load_from_path(&self, path: &Path) -> Result<rustdoc_types::Crate, RustdocCratePortError> {
        let crate_name =
            path.file_stem().and_then(|s| s.to_str()).unwrap_or("<unknown>").to_owned();

        // Security: fail-closed symlink guard before reading the baseline JSON.
        //
        // Step 1: guard the parent directory itself — `reject_symlinks_below` does
        // not inspect the anchor, so a symlinked parent (e.g. symlinked track dir)
        // must be caught separately, same pattern as in `baseline_capture.rs`.
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            match parent.symlink_metadata() {
                Ok(meta) if meta.file_type().is_symlink() => {
                    return Err(RustdocCratePortError::Io {
                        path: path.to_path_buf(),
                        reason: format!(
                            "symlink guard: refusing to read under symlinked directory: {}",
                            parent.display()
                        ),
                    });
                }
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => {
                    return Err(RustdocCratePortError::Io {
                        path: path.to_path_buf(),
                        reason: format!(
                            "symlink guard: cannot stat parent directory '{}': {e}",
                            parent.display()
                        ),
                    });
                }
            }
        }
        // Step 2: guard the leaf path itself (the baseline JSON file).
        let trusted_root = path.parent().unwrap_or(path);
        reject_symlinks_below(path, trusted_root).map_err(|e| RustdocCratePortError::Io {
            path: path.to_path_buf(),
            reason: format!("symlink guard rejected baseline path: {e}"),
        })?;

        BaselineRustdocCodec::load(path).map_err(|e| match e {
            BaselineRustdocCodecError::IoError(io_err)
                if io_err.kind() == std::io::ErrorKind::NotFound =>
            {
                RustdocCratePortError::NotFound { path: path.to_path_buf() }
            }
            BaselineRustdocCodecError::IoError(io_err) => {
                RustdocCratePortError::Io { path: path.to_path_buf(), reason: io_err.to_string() }
            }
            other => RustdocCratePortError::ParseFailed {
                crate_name,
                reason: format!("{} (file: {})", other, path.display()),
            },
        })
    }

    /// Captures the current `rustdoc_types::Crate` via `cargo +nightly rustdoc`
    /// (C-side live capture).
    ///
    /// # Errors
    ///
    /// Returns [`RustdocCratePortError::CaptureFailed`] if `cargo rustdoc` fails.
    ///
    /// Returns [`RustdocCratePortError::ParseFailed`] if the generated JSON
    /// cannot be deserialized.
    fn capture_current(
        &self,
        crate_name: &str,
    ) -> Result<rustdoc_types::Crate, RustdocCratePortError> {
        // Security: guard workspace_root against being a symlink before invoking the
        // exporter. A symlinked workspace root could redirect `cargo rustdoc` to run
        // in an arbitrary target directory, bypassing workspace confinement.
        // This mirrors the guard in `baseline_capture.rs` / `type_signals_evaluator.rs`.
        match self.workspace_root.symlink_metadata() {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(RustdocCratePortError::CaptureFailed {
                    crate_name: crate_name.to_owned(),
                    reason: format!(
                        "symlink guard: refusing to use symlinked workspace_root: {}",
                        self.workspace_root.display()
                    ),
                });
            }
            Ok(_) => {}
            Err(e) => {
                return Err(RustdocCratePortError::CaptureFailed {
                    crate_name: crate_name.to_owned(),
                    reason: format!(
                        "symlink guard: cannot stat workspace_root '{}': {e}",
                        self.workspace_root.display()
                    ),
                });
            }
        }

        let exporter = RustdocSchemaExporter::new(self.workspace_root.clone());

        // Run cargo +nightly rustdoc and get the JSON file path.
        let json_path = exporter.export_rustdoc_json_path(crate_name).map_err(|e| {
            RustdocCratePortError::CaptureFailed {
                crate_name: crate_name.to_owned(),
                reason: e.to_string(),
            }
        })?;

        // Read and parse the generated JSON.
        let json_content = std::fs::read_to_string(&json_path).map_err(|e| {
            RustdocCratePortError::Io { path: json_path.clone(), reason: e.to_string() }
        })?;

        BaselineRustdocCodec::from_json(&json_content).map_err(|e| {
            RustdocCratePortError::ParseFailed {
                crate_name: crate_name.to_owned(),
                reason: e.to_string(),
            }
        })
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
    fn test_load_from_path_nonexistent_file_returns_not_found() {
        let adapter = RustdocCrateAdapter::new(PathBuf::from("."));
        let path = Path::new("/nonexistent/path/does-not-exist.json");
        let err = adapter.load_from_path(path).unwrap_err();
        assert!(
            matches!(err, RustdocCratePortError::NotFound { .. }),
            "expected NotFound, got: {err}"
        );
    }

    #[test]
    fn test_load_from_path_invalid_json_returns_parse_failed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("domain-types-baseline.json");
        std::fs::write(&path, "{ not valid json }").unwrap();

        let adapter = RustdocCrateAdapter::new(PathBuf::from("."));
        let err = adapter.load_from_path(&path).unwrap_err();
        assert!(
            matches!(err, RustdocCratePortError::ParseFailed { .. }),
            "expected ParseFailed, got: {err}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_load_from_path_symlinked_file_returns_io_error() {
        // Security: a symlinked baseline JSON (leaf) must be rejected before loading.
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real-baseline.json");
        std::fs::write(&real, "{}").unwrap();

        let link = dir.path().join("domain-types-baseline.json");
        std::os::unix::fs::symlink(&real, &link).unwrap();

        let adapter = RustdocCrateAdapter::new(PathBuf::from("."));
        let err = adapter.load_from_path(&link).unwrap_err();
        assert!(
            matches!(err, RustdocCratePortError::Io { .. }),
            "expected Io (symlink rejection), got: {err}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_load_from_path_symlinked_parent_dir_returns_io_error() {
        // Security: reading through a symlinked parent directory (e.g. symlinked track dir)
        // must be rejected before the leaf check.
        let dir = tempfile::tempdir().unwrap();
        let real_sub = dir.path().join("real-sub");
        std::fs::create_dir_all(&real_sub).unwrap();
        std::fs::write(real_sub.join("domain-types-baseline.json"), "{}").unwrap();

        let link_sub = dir.path().join("link-sub");
        std::os::unix::fs::symlink(&real_sub, &link_sub).unwrap();

        let path = link_sub.join("domain-types-baseline.json");
        let adapter = RustdocCrateAdapter::new(PathBuf::from("."));
        let err = adapter.load_from_path(&path).unwrap_err();
        assert!(
            matches!(err, RustdocCratePortError::Io { .. }),
            "expected Io (symlinked parent directory rejection), got: {err}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_capture_current_symlinked_workspace_root_returns_capture_failed() {
        // Security: a symlinked workspace root must be rejected before invoking the exporter.
        let dir = tempfile::tempdir().unwrap();
        let real_ws = dir.path().join("real-workspace");
        std::fs::create_dir_all(&real_ws).unwrap();

        let link_ws = dir.path().join("link-workspace");
        std::os::unix::fs::symlink(&real_ws, &link_ws).unwrap();

        let adapter = RustdocCrateAdapter::new(link_ws);
        let err = adapter.capture_current("some_crate").unwrap_err();
        assert!(
            matches!(err, RustdocCratePortError::CaptureFailed { .. }),
            "expected CaptureFailed (symlink workspace_root rejection), got: {err}"
        );
    }

    #[test]
    fn test_rustdoc_crate_adapter_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<RustdocCrateAdapter>();
    }
}
