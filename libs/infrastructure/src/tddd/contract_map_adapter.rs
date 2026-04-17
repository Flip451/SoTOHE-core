//! Filesystem-backed adapters for the Contract Map ports defined in
//! `domain::tddd::catalogue_ports`.
//!
//! * [`FsCatalogueLoader`] wraps [`crate::tddd::catalogue_bulk_loader::
//!   load_all_catalogues`] (T002) behind the domain `CatalogueLoader`
//!   trait and maps the infrastructure-level error enum onto the domain
//!   error enum.
//! * [`FsContractMapWriter`] writes `track_dir/contract-map.md`
//!   atomically via [`atomic_write_file`] after guarding the path with
//!   [`reject_symlinks_below`].
//!
//! See ADR 2026-04-17-1528 §D1 and
//! `knowledge/conventions/security.md` §Symlink Rejection.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use domain::TrackId;
use domain::tddd::catalogue::TypeCatalogueDocument;
use domain::tddd::{
    CatalogueLoader, CatalogueLoaderError, ContractMapContent, ContractMapWriter,
    ContractMapWriterError, LayerId,
};

use crate::tddd::catalogue_bulk_loader::{self, LoadAllCataloguesError};
use crate::track::atomic_write::atomic_write_file;
use crate::track::symlink_guard::reject_symlinks_below;
use crate::verify::tddd_layers::LoadTdddLayersError;

/// Filesystem-backed [`CatalogueLoader`] implementation.
///
/// Resolves `track_dir = track_root / track_id` and delegates to
/// [`catalogue_bulk_loader::load_all_catalogues`]. A surface-level
/// symlink check on `track_dir` runs **before** the bulk loader is
/// invoked, so symlinked track directories are rejected with a precise
/// [`CatalogueLoaderError::SymlinkRejected`] variant rather than being
/// absorbed as a generic I/O error.
pub struct FsCatalogueLoader {
    track_root: PathBuf,
    rules_path: PathBuf,
    trusted_root: PathBuf,
}

impl FsCatalogueLoader {
    /// Creates a new `FsCatalogueLoader`.
    ///
    /// * `track_root` — directory that contains per-track subdirectories
    ///   (typically `<workspace>/track/items`).
    /// * `rules_path` — path to `architecture-rules.json`.
    /// * `trusted_root` — directory below which symlink traversal is
    ///   refused fail-closed (usually the workspace root).
    #[must_use]
    pub fn new(track_root: PathBuf, rules_path: PathBuf, trusted_root: PathBuf) -> Self {
        Self { track_root, rules_path, trusted_root }
    }
}

impl CatalogueLoader for FsCatalogueLoader {
    fn load_all(
        &self,
        track_id: &TrackId,
    ) -> Result<(Vec<LayerId>, BTreeMap<LayerId, TypeCatalogueDocument>), CatalogueLoaderError>
    {
        let track_dir = self.track_root.join(track_id.as_ref());
        // Adapter-level symlink guard on the track directory itself —
        // guarantees `SymlinkRejected` (rather than a generic I/O error)
        // when `track_dir` is a symlink.
        if let Err(e) = reject_symlinks_below(&track_dir, &self.trusted_root) {
            return if e.kind() == std::io::ErrorKind::InvalidInput {
                Err(CatalogueLoaderError::SymlinkRejected { path: track_dir })
            } else {
                Err(CatalogueLoaderError::IoError { path: track_dir, reason: e.to_string() })
            };
        }

        catalogue_bulk_loader::load_all_catalogues(&track_dir, &self.rules_path, &self.trusted_root)
            .map_err(map_loader_error)
    }
}

fn map_loader_error(err: LoadAllCataloguesError) -> CatalogueLoaderError {
    match err {
        LoadAllCataloguesError::LayerBindings(ref inner) => {
            // Preserve symlink-rejection classification: if the rules-file load
            // failed because `reject_symlinks_below` returned `InvalidInput`,
            // surface that as `SymlinkRejected` rather than the generic
            // `LayerDiscoveryFailed` so callers can distinguish security
            // rejections from parse failures.
            if let LoadTdddLayersError::Io { path, source } = inner {
                if source.kind() == std::io::ErrorKind::InvalidInput {
                    return CatalogueLoaderError::SymlinkRejected { path: path.clone() };
                }
            }
            CatalogueLoaderError::LayerDiscoveryFailed { reason: err.to_string() }
        }
        LoadAllCataloguesError::ArchRulesParse { path, reason } => {
            CatalogueLoaderError::LayerDiscoveryFailed {
                reason: format!("{}: {reason}", path.display()),
            }
        }
        LoadAllCataloguesError::Io { path, source } => {
            if source.kind() == std::io::ErrorKind::InvalidInput {
                CatalogueLoaderError::SymlinkRejected { path }
            } else {
                CatalogueLoaderError::IoError { path, reason: source.to_string() }
            }
        }
        LoadAllCataloguesError::CatalogueNotFound { layer_id, path } => {
            CatalogueLoaderError::CatalogueNotFound { layer_id, path }
        }
        LoadAllCataloguesError::Decode { layer_id, path: _, source } => {
            CatalogueLoaderError::DecodeFailed { layer_id, reason: source.to_string() }
        }
        LoadAllCataloguesError::TopologicalSortFailed { cycle } => {
            CatalogueLoaderError::TopologicalSortFailed {
                reason: format!("cycle among layers {cycle:?}"),
            }
        }
        LoadAllCataloguesError::InvalidLayerId { value, source } => {
            CatalogueLoaderError::LayerDiscoveryFailed {
                reason: format!("invalid layer id '{value}': {source}"),
            }
        }
    }
}

/// Filesystem-backed [`ContractMapWriter`] that writes to
/// `track_root/<track_id>/contract-map.md`.
///
/// The writer rejects any symlink on the write target (and its parent
/// chain up to `trusted_root`) and performs the write via
/// [`atomic_write_file`] so partial files are never observed.
pub struct FsContractMapWriter {
    track_root: PathBuf,
    trusted_root: PathBuf,
}

impl FsContractMapWriter {
    /// Creates a new `FsContractMapWriter`.
    ///
    /// * `track_root` — directory that contains per-track subdirectories.
    /// * `trusted_root` — directory below which symlink traversal is
    ///   refused fail-closed.
    #[must_use]
    pub fn new(track_root: PathBuf, trusted_root: PathBuf) -> Self {
        Self { track_root, trusted_root }
    }
}

impl ContractMapWriter for FsContractMapWriter {
    fn write(
        &self,
        track_id: &TrackId,
        content: &ContractMapContent,
    ) -> Result<(), ContractMapWriterError> {
        let track_dir = self.track_root.join(track_id.as_ref());
        // Guard the track directory itself before the existence check so that a
        // broken symlink at `track_dir` is classified as `SymlinkRejected` rather
        // than silently collapsed into `TrackNotFound` by `.exists()` (which
        // follows symlinks and returns false for broken ones).
        if let Err(e) = reject_symlinks_below(&track_dir, &self.trusted_root) {
            return if e.kind() == std::io::ErrorKind::InvalidInput {
                Err(ContractMapWriterError::SymlinkRejected { path: track_dir })
            } else {
                Err(ContractMapWriterError::IoError { path: track_dir, reason: e.to_string() })
            };
        }
        if !track_dir.is_dir() {
            return Err(ContractMapWriterError::TrackNotFound {
                track_id: track_id.as_ref().to_owned(),
                expected_dir: track_dir,
            });
        }

        let out_path = track_dir.join("contract-map.md");
        reject_symlinks_below(&out_path, &self.trusted_root).map_err(|e| {
            if e.kind() == std::io::ErrorKind::InvalidInput {
                ContractMapWriterError::SymlinkRejected { path: out_path.clone() }
            } else {
                ContractMapWriterError::IoError { path: out_path.clone(), reason: e.to_string() }
            }
        })?;
        atomic_write_file(&out_path, content.as_ref().as_bytes()).map_err(|e| {
            ContractMapWriterError::IoError { path: out_path.clone(), reason: e.to_string() }
        })?;
        Ok(())
    }
}

/// Returns the expected `contract-map.md` path for a track rooted at
/// `track_root` with id `track_id`. Exposed for tests and CLI composition
/// that need the write destination without constructing the adapter.
#[must_use]
pub fn contract_map_path(track_root: &Path, track_id: &TrackId) -> PathBuf {
    track_root.join(track_id.as_ref()).join("contract-map.md")
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use super::*;
    use domain::tddd::catalogue::TypeCatalogueDocument;

    const RULES_JSON: &str = r#"{
      "version": 2,
      "layers": [
        {
          "crate": "domain",
          "path": "libs/domain",
          "may_depend_on": [],
          "deny_reason": "no reverse dep",
          "tddd": {
            "enabled": true,
            "catalogue_file": "domain-types.json",
            "schema_export": {"method": "rustdoc", "targets": ["domain"]}
          }
        }
      ]
    }"#;

    const EMPTY_CATALOGUE: &str = r#"{"schema_version": 2, "type_definitions": []}"#;

    fn write(path: &std::path::Path, content: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, content).unwrap();
    }

    fn track_id(slug: &str) -> TrackId {
        TrackId::try_new(slug.to_owned()).unwrap()
    }

    #[test]
    fn test_fs_catalogue_loader_happy_path() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let rules = root.join("architecture-rules.json");
        write(&rules, RULES_JSON);

        let track_root = root.join("track-items");
        let id = track_id("t001");
        let track_dir = track_root.join(id.as_ref());
        write(&track_dir.join("domain-types.json"), EMPTY_CATALOGUE);

        let loader = FsCatalogueLoader::new(track_root, rules, root.to_path_buf());
        let (order, catalogues) = loader.load_all(&id).unwrap();
        assert_eq!(order.len(), 1);
        assert_eq!(order[0].as_ref(), "domain");
        assert_eq!(catalogues.len(), 1);
        let doc: &TypeCatalogueDocument = catalogues.get(&order[0]).unwrap();
        assert_eq!(doc.entries().len(), 0);
    }

    #[test]
    fn test_fs_catalogue_loader_missing_catalogue_maps_to_domain_error() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let rules = root.join("architecture-rules.json");
        write(&rules, RULES_JSON);

        let track_root = root.join("track-items");
        let id = track_id("t002");
        std::fs::create_dir_all(track_root.join(id.as_ref())).unwrap();
        // no catalogue file written — fail closed

        let loader = FsCatalogueLoader::new(track_root, rules, root.to_path_buf());
        let err = loader.load_all(&id).unwrap_err();
        match err {
            CatalogueLoaderError::CatalogueNotFound { layer_id, path } => {
                assert_eq!(layer_id, "domain");
                assert!(path.ends_with("domain-types.json"));
            }
            other => panic!("expected CatalogueNotFound, got {other:?}"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn test_fs_catalogue_loader_rejects_symlinked_track_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let rules = root.join("architecture-rules.json");
        write(&rules, RULES_JSON);

        let track_root = root.join("track-items");
        std::fs::create_dir_all(&track_root).unwrap();
        let real_dir = root.join("real-track");
        std::fs::create_dir_all(&real_dir).unwrap();
        let id = track_id("t003");
        let symlinked = track_root.join(id.as_ref());
        std::os::unix::fs::symlink(&real_dir, &symlinked).unwrap();

        let loader = FsCatalogueLoader::new(track_root, rules, root.to_path_buf());
        let err = loader.load_all(&id).unwrap_err();
        assert!(
            matches!(err, CatalogueLoaderError::SymlinkRejected { .. }),
            "expected SymlinkRejected, got {err:?}"
        );
    }

    #[test]
    fn test_fs_contract_map_writer_happy_path_writes_to_track_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let track_root = root.join("track-items");
        let id = track_id("t004");
        let track_dir = track_root.join(id.as_ref());
        std::fs::create_dir_all(&track_dir).unwrap();

        let writer = FsContractMapWriter::new(track_root, root.to_path_buf());
        let content = ContractMapContent::new("```mermaid\nflowchart LR\nend\n```\n");
        writer.write(&id, &content).unwrap();

        let out = track_dir.join("contract-map.md");
        assert!(out.is_file());
        let read = std::fs::read_to_string(&out).unwrap();
        assert!(read.contains("flowchart LR"));
    }

    #[test]
    fn test_fs_contract_map_writer_missing_track_dir_returns_track_not_found() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let track_root = root.join("track-items");
        std::fs::create_dir_all(&track_root).unwrap();
        let id = track_id("t005");

        let writer = FsContractMapWriter::new(track_root, root.to_path_buf());
        let content = ContractMapContent::new("```mermaid\nflowchart LR\nend\n```\n");
        let err = writer.write(&id, &content).unwrap_err();
        match err {
            ContractMapWriterError::TrackNotFound { track_id: t, expected_dir } => {
                assert_eq!(t, "t005");
                assert!(expected_dir.ends_with("t005"));
            }
            other => panic!("expected TrackNotFound, got {other:?}"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn test_fs_contract_map_writer_rejects_symlinked_target() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let track_root = root.join("track-items");
        let id = track_id("t006");
        let track_dir = track_root.join(id.as_ref());
        std::fs::create_dir_all(&track_dir).unwrap();

        let real_target = root.join("outside.md");
        std::fs::write(&real_target, b"").unwrap();
        std::os::unix::fs::symlink(&real_target, track_dir.join("contract-map.md")).unwrap();

        let writer = FsContractMapWriter::new(track_root, root.to_path_buf());
        let content = ContractMapContent::new("```mermaid\nflowchart LR\nend\n```\n");
        let err = writer.write(&id, &content).unwrap_err();
        assert!(
            matches!(err, ContractMapWriterError::SymlinkRejected { .. }),
            "expected SymlinkRejected, got {err:?}"
        );
    }

    #[test]
    fn test_fs_contract_map_writer_overwrites_existing_non_symlink_file() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let track_root = root.join("track-items");
        let id = track_id("t007");
        let track_dir = track_root.join(id.as_ref());
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(track_dir.join("contract-map.md"), b"stale").unwrap();

        let writer = FsContractMapWriter::new(track_root.clone(), root.to_path_buf());
        let content = ContractMapContent::new("```mermaid\nflowchart LR\nend\n```\n");
        writer.write(&id, &content).unwrap();

        let read = std::fs::read_to_string(track_dir.join("contract-map.md")).unwrap();
        assert!(read.contains("flowchart LR"));
        assert!(!read.contains("stale"));
    }

    #[test]
    fn test_contract_map_path_joins_track_root_and_id() {
        let id = track_id("t008");
        let root = std::path::PathBuf::from("/tmp/fake-track-root");
        let got = contract_map_path(&root, &id);
        assert_eq!(got, std::path::PathBuf::from("/tmp/fake-track-root/t008/contract-map.md"));
    }

    /// Verify that a genuine I/O failure during `atomic_write_file` (e.g. a
    /// read-only track directory) is surfaced as `ContractMapWriterError::IoError`.
    #[cfg(unix)]
    #[test]
    fn test_fs_contract_map_writer_io_error_on_unwritable_track_dir() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let track_root = root.join("track-items");
        let id = track_id("t009");
        let track_dir = track_root.join(id.as_ref());
        std::fs::create_dir_all(&track_dir).unwrap();

        // Make track_dir read-only so atomic_write_file cannot create the tmp file.
        std::fs::set_permissions(&track_dir, std::fs::Permissions::from_mode(0o555)).unwrap();

        let writer = FsContractMapWriter::new(track_root, root.to_path_buf());
        let content = ContractMapContent::new("```mermaid\nflowchart LR\nend\n```\n");
        let err = writer.write(&id, &content).unwrap_err();

        // Restore permissions before any assertions so tempdir cleanup succeeds.
        let _ = std::fs::set_permissions(&track_dir, std::fs::Permissions::from_mode(0o755));

        assert!(
            matches!(err, ContractMapWriterError::IoError { .. }),
            "expected IoError, got {err:?}"
        );
    }
}
