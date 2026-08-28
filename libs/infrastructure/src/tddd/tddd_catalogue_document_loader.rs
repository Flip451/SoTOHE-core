//! `FsCatalogueDocumentLoader` — filesystem adapter for `AttestedCatalogueDocumentLoaderPort`.
//!
//! Reads through the shared bounded UTF-8 adapter and maps codec errors to the
//! domain port error variants so that `libs/usecase` never imports
//! infrastructure error types.
//!
//! [source: ADR 2026-05-11-2330 §D2]

use std::io::{Error, ErrorKind, Read};
use std::path::Path;

#[cfg(unix)]
use std::ffi::OsString;
#[cfg(unix)]
use std::path::Component;

use domain::tddd::catalogue_v2::{AttestedCatalogueDocument, CatalogueDocumentLoaderError};
use usecase::catalogue_document_loader::AttestedCatalogueDocumentLoaderPort;

use crate::tddd::catalogue_document_codec::{
    CatalogueDocumentCodec, CatalogueDocumentCodecError, derive_filename_stem,
};
use crate::track::symlink_guard::reject_symlinks_up_to_root;

const MAX_TYPE_CATALOGUE_BYTES: u64 = 16 * 1024 * 1024;

// ---------------------------------------------------------------------------
// FsCatalogueDocumentLoader
// ---------------------------------------------------------------------------

/// Filesystem adapter implementing [`AttestedCatalogueDocumentLoaderPort`].
///
/// Uses the shared bounded UTF-8 reader and returns the decoded document with
/// the exact declaration hash of the bytes it read. The pre-review gate compares
/// that attestation with the hash on the request's signal document before
/// resolving namespaces.
/// Injected into `CatalogueImplSignalsInteractor` at the `apps/cli` composition
/// root.
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

impl AttestedCatalogueDocumentLoaderPort for FsCatalogueDocumentLoader {
    /// Loads an attested `CatalogueDocument` from the given filesystem path.
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
    fn load(&self, path: &Path) -> Result<AttestedCatalogueDocument, CatalogueDocumentLoaderError> {
        // Security: fail-closed symlink guard before reading. The loader has no
        // caller-supplied trusted root, so every ancestor must be inspected;
        // anchoring the check at `path.parent()` would allow a symlinked
        // grandparent to redirect the bounded read.
        reject_symlinks_up_to_root(path).map_err(|e| CatalogueDocumentLoaderError::Io {
            path: path.to_path_buf(),
            reason: format!("symlink guard rejected catalogue path: {e}"),
        })?;

        let content = read_catalogue_file(path).map_err(|error| {
            if error.kind() == ErrorKind::NotFound {
                CatalogueDocumentLoaderError::NotFound { path: path.to_path_buf() }
            } else {
                CatalogueDocumentLoaderError::Io {
                    path: path.to_path_buf(),
                    reason: error.to_string(),
                }
            }
        })?;
        let filename_stem = derive_filename_stem(path);
        AttestedCatalogueDocument::attest(content.as_bytes(), |source| {
            let json = std::str::from_utf8(source).map_err(|error| {
                CatalogueDocumentCodecError::Io(Error::new(
                    ErrorKind::InvalidData,
                    error.to_string(),
                ))
            })?;
            CatalogueDocumentCodec::decode(json, &filename_stem)
        })
        .map_err(|e| match e {
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

/// Opens a catalogue through the platform's no-follow primitive.
///
/// The pre-check in [`FsCatalogueDocumentLoader::load`] provides a useful
/// diagnostic for already-present symlinks. This second walk closes the
/// check-then-open race by resolving every directory component relative to a
/// descriptor and opening the leaf with `NOFOLLOW`.
#[cfg(unix)]
fn open_catalogue_file_nofollow(path: &Path) -> Result<std::fs::File, std::io::Error> {
    let absolute =
        if path.is_absolute() { path.to_path_buf() } else { std::env::current_dir()?.join(path) };
    let file_name: OsString = absolute
        .file_name()
        .ok_or_else(|| Error::new(ErrorKind::InvalidInput, "catalogue path has no file name"))?
        .to_os_string();
    let parent = absolute
        .parent()
        .ok_or_else(|| Error::new(ErrorKind::InvalidInput, "catalogue path has no parent"))?;

    let mut directory = rustix::fs::open(
        Path::new("/"),
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::DIRECTORY
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .map(std::fs::File::from)
    .map_err(std::io::Error::from)?;

    for component in parent.components() {
        let Component::Normal(name) = component else {
            if matches!(component, Component::RootDir | Component::CurDir) {
                continue;
            }
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "catalogue path contains a parent or prefix component",
            ));
        };
        directory = rustix::fs::openat(
            &directory,
            name,
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::DIRECTORY
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .map(std::fs::File::from)
        .map_err(std::io::Error::from)?;
    }

    rustix::fs::openat(
        &directory,
        &file_name,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::NONBLOCK
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .map(std::fs::File::from)
    .map_err(std::io::Error::from)
}

/// Opens the catalogue leaf without following a Windows reparse point.
#[cfg(windows)]
fn open_catalogue_file_nofollow(path: &Path) -> Result<std::fs::File, std::io::Error> {
    use std::os::windows::fs::OpenOptionsExt as _;

    // FILE_FLAG_OPEN_REPARSE_POINT opens the reparse point itself so the
    // opened-handle metadata check rejects a symlink or junction.
    std::fs::OpenOptions::new().read(true).custom_flags(0x0020_0000).open(path)
}

/// Refuses catalogue reads on platforms without a descriptor-relative,
/// no-follow open primitive rather than weakening the symlink race guarantee
/// with a path-based fallback.
#[cfg(not(any(unix, windows)))]
fn open_catalogue_file_nofollow(_path: &Path) -> Result<std::fs::File, std::io::Error> {
    Err(Error::new(
        ErrorKind::Unsupported,
        "atomic no-follow catalogue open is unavailable on this platform",
    ))
}

/// Reads an already-opened regular catalogue descriptor with the shared byte
/// limit semantics used by repository-authored text files.
fn read_catalogue_file(path: &Path) -> Result<String, std::io::Error> {
    let file = open_catalogue_file_nofollow(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            format!("{} is not a regular file", path.display()),
        ));
    }
    if metadata.len() > MAX_TYPE_CATALOGUE_BYTES {
        return Err(Error::new(
            ErrorKind::InvalidData,
            format!(
                "{} exceeds the maximum allowed size of {MAX_TYPE_CATALOGUE_BYTES} bytes",
                path.display()
            ),
        ));
    }

    let mut reader = file.take(MAX_TYPE_CATALOGUE_BYTES.saturating_add(1));
    let mut content = String::new();
    reader.read_to_string(&mut content)?;
    if content.len()
        > usize::try_from(MAX_TYPE_CATALOGUE_BYTES).map_err(|_| {
            Error::new(ErrorKind::InvalidData, "catalogue byte limit exceeds platform capacity")
        })?
    {
        return Err(Error::new(
            ErrorKind::InvalidData,
            format!(
                "{} exceeds the maximum allowed size of {MAX_TYPE_CATALOGUE_BYTES} bytes",
                path.display()
            ),
        ));
    }
    Ok(content)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::tddd::type_signals_codec;
    use tempfile::NamedTempFile;

    fn minimal_v3_json(crate_name: &str) -> String {
        format!(
            r#"{{
  "schema_version": 5,
  "crate_name": "{crate_name}",
  "layer": "{crate_name}",
  "types": {{}},
  "traits": {{}},
  "functions": {{}}
}}"#
        )
    }

    #[cfg(any(unix, windows))]
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
        let attested = loader.load(&path).unwrap();
        assert_eq!(attested.document().crate_name().as_str(), "domain");
        assert_eq!(
            attested.declaration_hash(),
            &type_signals_codec::declaration_hash(json.as_bytes())
        );
    }

    #[cfg(any(unix, windows))]
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

    #[cfg(any(unix, windows))]
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

    #[cfg(any(unix, windows))]
    #[test]
    fn test_load_oversized_catalogue_returns_typed_io_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("domain-types.json");
        std::fs::File::create(&path).unwrap().set_len(MAX_TYPE_CATALOGUE_BYTES + 1).unwrap();

        let err = FsCatalogueDocumentLoader::new().load(&path).unwrap_err();

        match err {
            CatalogueDocumentLoaderError::Io { reason, .. } => {
                assert!(reason.contains("exceeds the maximum allowed size"), "{reason}");
            }
            other => panic!("expected typed Io overflow error, got: {other}"),
        }
    }

    #[cfg(any(unix, windows))]
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

    #[cfg(not(any(unix, windows)))]
    #[test]
    fn test_load_fails_closed_when_atomic_catalogue_open_is_unavailable() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("domain-types.json");
        std::fs::write(&path, minimal_v3_json("domain")).unwrap();

        let err = FsCatalogueDocumentLoader::new().load(&path).unwrap_err();

        assert!(matches!(
            err,
            CatalogueDocumentLoaderError::Io { reason, .. }
                if reason.contains("atomic no-follow catalogue open is unavailable")
        ));
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
    fn test_atomic_catalogue_open_rejects_symlinked_leaf() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("domain-types.json");
        std::fs::write(&target, minimal_v3_json("domain")).unwrap();
        let link = dir.path().join("domain-types-link.json");
        std::os::unix::fs::symlink(&target, &link).unwrap();

        let error = open_catalogue_file_nofollow(&link).unwrap_err();

        assert!(error.raw_os_error().is_some(), "expected an OS-level no-follow error: {error}");
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

    #[cfg(unix)]
    #[test]
    fn test_load_symlinked_grandparent_dir_returns_io_error() {
        // Security: a symlinked grandparent must be rejected even though the
        // immediate parent resolves to a regular directory.
        let dir = tempfile::tempdir().unwrap();
        let real_root = dir.path().join("real-root");
        let real_sub = real_root.join("sub");
        std::fs::create_dir_all(&real_sub).unwrap();
        let json = minimal_v3_json("domain");
        std::fs::write(real_sub.join("domain-types.json"), &json).unwrap();

        let link_root = dir.path().join("link-root");
        std::os::unix::fs::symlink(&real_root, &link_root).unwrap();

        let path = link_root.join("sub/domain-types.json");
        let loader = FsCatalogueDocumentLoader::new();
        let err = loader.load(&path).unwrap_err();
        assert!(
            matches!(err, CatalogueDocumentLoaderError::Io { .. }),
            "expected Io (symlinked grandparent directory rejection), got: {err}"
        );
    }

    #[test]
    fn test_fs_catalogue_document_loader_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<FsCatalogueDocumentLoader>();
    }
}
