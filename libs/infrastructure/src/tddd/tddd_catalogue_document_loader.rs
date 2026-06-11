//! `FsCatalogueDocumentLoader` — filesystem adapter for `CatalogueDocumentLoaderPort`.
//!
//! Wraps `CatalogueDocumentCodec::load` and maps codec errors to the domain
//! port error variants so that `libs/usecase` never imports infrastructure
//! error types.
//!
//! [source: ADR 2026-05-11-2330 §D2]

use std::path::Path;

use domain::tddd::catalogue_v2::{
    CatalogueDocument, CatalogueDocumentLoaderError, CatalogueDocumentLoaderPort,
};

use crate::tddd::catalogue_document_codec::{CatalogueDocumentCodec, CatalogueDocumentCodecError};
use crate::track::symlink_guard::reject_symlinks_below;

// ---------------------------------------------------------------------------
// FsCatalogueDocumentLoader
// ---------------------------------------------------------------------------

/// Stateless filesystem adapter implementing [`CatalogueDocumentLoaderPort`].
///
/// Delegates to [`CatalogueDocumentCodec::load`] and maps codec errors to
/// [`CatalogueDocumentLoaderError`] variants. Injected into
/// `CatalogueImplSignalsInteractor` at the `apps/cli` composition root.
///
/// [source: ADR 2026-05-11-2330 D2]
#[derive(Debug, Clone, Default)]
pub struct FsCatalogueDocumentLoader;

impl FsCatalogueDocumentLoader {
    /// Creates a new loader instance.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl CatalogueDocumentLoaderPort for FsCatalogueDocumentLoader {
    /// Loads a `CatalogueDocument` from the given filesystem path.
    ///
    /// # Errors
    ///
    /// Returns [`CatalogueDocumentLoaderError::NotFound`] if the file is absent.
    ///
    /// Returns [`CatalogueDocumentLoaderError::Io`] if a non-symlink I/O error
    /// occurs while reading the file, or if a symlink is detected at the path
    /// (symlink rejection is fail-closed — the path must be a regular file).
    ///
    /// Returns [`CatalogueDocumentLoaderError::Decode`] if JSON deserialization
    /// or schema-version validation fails.
    fn load(&self, path: &Path) -> Result<CatalogueDocument, CatalogueDocumentLoaderError> {
        // Security: fail-closed symlink guard before reading.
        //
        // Step 1: guard the parent directory itself — `reject_symlinks_below` does
        // not inspect the anchor, so a symlinked parent must be caught separately.
        // This is the same pattern used by `catalogue_spec_signals_refresher` and
        // `baseline_capture` for their root directory arguments.
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            match parent.symlink_metadata() {
                Ok(meta) if meta.file_type().is_symlink() => {
                    return Err(CatalogueDocumentLoaderError::Io {
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
                    return Err(CatalogueDocumentLoaderError::Io {
                        path: path.to_path_buf(),
                        reason: format!(
                            "symlink guard: cannot stat parent directory '{}': {e}",
                            parent.display()
                        ),
                    });
                }
            }
        }
        // Step 2: guard the leaf path itself (the catalogue file).
        let trusted_root = path.parent().unwrap_or(path);
        reject_symlinks_below(path, trusted_root).map_err(|e| {
            CatalogueDocumentLoaderError::Io {
                path: path.to_path_buf(),
                reason: format!("symlink guard rejected catalogue path: {e}"),
            }
        })?;

        CatalogueDocumentCodec::load(path).map_err(|e| match e {
            CatalogueDocumentCodecError::Io(io_err)
                if io_err.kind() == std::io::ErrorKind::NotFound =>
            {
                CatalogueDocumentLoaderError::NotFound { path: path.to_path_buf() }
            }
            CatalogueDocumentCodecError::Io(io_err) => CatalogueDocumentLoaderError::Io {
                path: path.to_path_buf(),
                reason: io_err.to_string(),
            },
            other => CatalogueDocumentLoaderError::Decode {
                path: path.to_path_buf(),
                reason: other.to_string(),
            },
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
    use tempfile::NamedTempFile;

    fn minimal_v3_json(crate_name: &str) -> String {
        format!(
            r#"{{
  "schema_version": 4,
  "crate_name": "{crate_name}",
  "layer": "{crate_name}",
  "types": {{}},
  "traits": {{}},
  "functions": {{}}
}}"#
        )
    }

    #[test]
    fn test_load_valid_catalogue_document_succeeds() {
        let json = minimal_v3_json("domain");
        // NamedTempFile is created only to demonstrate it can be constructed;
        // the actual test path uses a named file in a tempdir so the stem check passes.
        let _tmp = NamedTempFile::with_suffix("-types.json").unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("domain-types.json");
        std::fs::write(&path, &json).unwrap();

        let loader = FsCatalogueDocumentLoader::new();
        let doc = loader.load(&path).unwrap();
        assert_eq!(doc.crate_name.as_str(), "domain");
    }

    #[test]
    fn test_load_nonexistent_file_returns_not_found() {
        let loader = FsCatalogueDocumentLoader::new();
        let path = std::path::Path::new("/nonexistent/path/does-not-exist-types.json");
        let err = loader.load(path).unwrap_err();
        assert!(
            matches!(err, CatalogueDocumentLoaderError::NotFound { .. }),
            "expected NotFound, got: {err}"
        );
    }

    #[test]
    fn test_load_invalid_json_returns_decode_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("domain-types.json");
        std::fs::write(&path, "{ not valid json }").unwrap();

        let loader = FsCatalogueDocumentLoader::new();
        let err = loader.load(&path).unwrap_err();
        assert!(
            matches!(err, CatalogueDocumentLoaderError::Decode { .. }),
            "expected Decode, got: {err}"
        );
    }

    #[test]
    fn test_load_wrong_schema_version_returns_decode_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("domain-types.json");
        std::fs::write(
            &path,
            r#"{"schema_version": 2, "crate_name": "domain", "layer": "domain"}"#,
        )
        .unwrap();

        let loader = FsCatalogueDocumentLoader::new();
        let err = loader.load(&path).unwrap_err();
        assert!(
            matches!(err, CatalogueDocumentLoaderError::Decode { .. }),
            "expected Decode for schema version mismatch, got: {err}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_load_symlinked_catalogue_returns_io_error() {
        // Security: a symlinked catalogue file (leaf) must be rejected before reading.
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real-types.json");
        let json = minimal_v3_json("domain");
        std::fs::write(&real, &json).unwrap();

        let link = dir.path().join("domain-types.json");
        std::os::unix::fs::symlink(&real, &link).unwrap();

        let loader = FsCatalogueDocumentLoader::new();
        let err = loader.load(&link).unwrap_err();
        assert!(
            matches!(err, CatalogueDocumentLoaderError::Io { .. }),
            "expected Io (symlink rejection), got: {err}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_load_symlinked_parent_dir_returns_io_error() {
        // Security: reading through a symlinked parent directory must be rejected.
        // A symlinked track directory would otherwise bypass the leaf check.
        let dir = tempfile::tempdir().unwrap();
        let real_sub = dir.path().join("real-sub");
        std::fs::create_dir_all(&real_sub).unwrap();
        let json = minimal_v3_json("domain");
        std::fs::write(real_sub.join("domain-types.json"), &json).unwrap();

        let link_sub = dir.path().join("link-sub");
        std::os::unix::fs::symlink(&real_sub, &link_sub).unwrap();

        let path = link_sub.join("domain-types.json");
        let loader = FsCatalogueDocumentLoader::new();
        let err = loader.load(&path).unwrap_err();
        assert!(
            matches!(err, CatalogueDocumentLoaderError::Io { .. }),
            "expected Io (symlinked parent directory rejection), got: {err}"
        );
    }

    #[test]
    fn test_fs_catalogue_document_loader_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<FsCatalogueDocumentLoader>();
    }
}
