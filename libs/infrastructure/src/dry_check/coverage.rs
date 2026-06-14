//! `FsDryCheckCoverageAdapter` — filesystem adapter for the D5 coverage manifest.

use std::collections::BTreeSet;
use std::path::PathBuf;

use domain::TrackId;
use domain::dry_check::{DryCheckCoverageRecord, FragmentContentHash, FragmentRef};
use domain::review_v2::FilePath;
use usecase::dry_check::{DryCheckCoveragePort, DryCheckCycleError};

use crate::track::atomic_write::atomic_write_file;
use crate::track::symlink_guard::reject_symlinks_below;

// ── On-disk schema (private serde DTO) ────────────────────────────────────────

/// Schema version written by this implementation.
const CURRENT_SCHEMA_VERSION: u32 = 1;

/// On-disk envelope: `{ "schema_version": 1, "fragment_refs": [{ "path": "...", "content_hash": "..." }] }`.
///
/// Serde lives only here per the hexagonal principle — the domain
/// `DryCheckCoverageRecord` stays serde-free.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct CoverageManifestV1 {
    schema_version: u32,
    fragment_refs: Vec<FragmentRefDto>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct FragmentRefDto {
    path: String,
    content_hash: String,
}

// ── FsDryCheckCoverageAdapter ─────────────────────────────────────────────────

/// Filesystem adapter implementing [`DryCheckCoveragePort`].
///
/// Persists a single coverage manifest at the configured `store_path`.
/// `read_coverage` returns `Ok(None)` when the file is absent (CN-08:
/// the caller treats that as Blocked / fail-closed). Writes are atomic
/// via the shared `atomic_write_file` helper.
/// Symlinks at `store_path` or below `trusted_root` are rejected before
/// reads and writes.
///
/// `read_coverage` / `write_coverage` ignore the `TrackId` argument:
/// each adapter instance is bound to one path / one track. Composition
/// builds a fresh instance per track via [`FsDryCheckCoverageAdapter::new`].
#[derive(Debug)]
pub struct FsDryCheckCoverageAdapter {
    store_path: PathBuf,
    trusted_root: PathBuf,
}

impl FsDryCheckCoverageAdapter {
    /// Construct a new adapter bound to `store_path`.
    #[must_use]
    pub fn new(store_path: PathBuf, trusted_root: PathBuf) -> FsDryCheckCoverageAdapter {
        FsDryCheckCoverageAdapter { store_path, trusted_root }
    }

    fn reject_symlinks(&self, path_str: &str) -> Result<(), DryCheckCycleError> {
        reject_symlinks_below(&self.store_path, &self.trusted_root)
            .map(|_| ())
            .map_err(|e| DryCheckCycleError::CoveragePort(format!("symlink guard {path_str}: {e}")))
    }
}

impl DryCheckCoveragePort for FsDryCheckCoverageAdapter {
    fn read_coverage(
        &self,
        _track_id: &TrackId,
    ) -> Result<Option<DryCheckCoverageRecord>, DryCheckCycleError> {
        let path_str = self.store_path.display().to_string();

        self.reject_symlinks(&path_str)?;

        let content = match std::fs::read_to_string(&self.store_path) {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => {
                return Err(DryCheckCycleError::CoveragePort(format!("read {path_str}: {e}")));
            }
        };

        if content.trim().is_empty() {
            // An empty file is treated like a missing manifest (Ok(None) →
            // Blocked at the caller). Persisted empty-set coverage records
            // always serialize to a non-empty JSON envelope.
            return Ok(None);
        }

        let manifest: CoverageManifestV1 = serde_json::from_str(&content)
            .map_err(|e| DryCheckCycleError::CoveragePort(format!("parse {path_str}: {e}")))?;

        if manifest.schema_version != CURRENT_SCHEMA_VERSION {
            return Err(DryCheckCycleError::CoveragePort(format!(
                "{path_str}: unsupported schema_version {}",
                manifest.schema_version
            )));
        }

        let mut fragment_refs = BTreeSet::new();
        for dto in manifest.fragment_refs {
            let path = FilePath::new(&dto.path).map_err(|e| {
                DryCheckCycleError::CoveragePort(format!("{path_str}: invalid path: {e}"))
            })?;
            let content_hash = FragmentContentHash::new(&dto.content_hash).map_err(|e| {
                DryCheckCycleError::CoveragePort(format!("{path_str}: invalid content_hash: {e}"))
            })?;
            fragment_refs.insert(FragmentRef::new(path, content_hash));
        }

        Ok(Some(DryCheckCoverageRecord::new(fragment_refs)))
    }

    fn write_coverage(
        &self,
        _track_id: &TrackId,
        record: DryCheckCoverageRecord,
    ) -> Result<(), DryCheckCycleError> {
        let path_str = self.store_path.display().to_string();

        self.reject_symlinks(&path_str)?;

        let manifest = CoverageManifestV1 {
            schema_version: CURRENT_SCHEMA_VERSION,
            fragment_refs: record
                .fragment_refs()
                .iter()
                .map(|fr| FragmentRefDto {
                    path: fr.path().as_str().to_owned(),
                    content_hash: fr.content_hash().as_str().to_owned(),
                })
                .collect(),
        };

        let json = serde_json::to_vec_pretty(&manifest)
            .map_err(|e| DryCheckCycleError::CoveragePort(format!("serialize {path_str}: {e}")))?;

        if let Some(parent) = self.store_path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    DryCheckCycleError::CoveragePort(format!(
                        "create parent {}: {e}",
                        parent.display()
                    ))
                })?;
            }
        }

        atomic_write_file(&self.store_path, &json)
            .map_err(|e| DryCheckCycleError::CoveragePort(format!("write {path_str}: {e}")))?;

        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn adapter_in_dir(dir: &tempfile::TempDir, filename: &str) -> FsDryCheckCoverageAdapter {
        FsDryCheckCoverageAdapter::new(dir.path().join(filename), dir.path().to_owned())
    }

    fn make_track_id() -> TrackId {
        TrackId::try_new("test-track-2026-06-13").unwrap()
    }

    fn make_fragment_ref(path: &str, hash_char: char) -> FragmentRef {
        let hash = hash_char.to_string().repeat(64);
        FragmentRef::new(FilePath::new(path).unwrap(), FragmentContentHash::new(hash).unwrap())
    }

    #[test]
    fn test_read_coverage_returns_none_when_file_missing() {
        let dir = tempfile::tempdir().unwrap();
        let adapter = adapter_in_dir(&dir, "coverage.json");
        let result = adapter.read_coverage(&make_track_id()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_write_then_read_round_trips_record() {
        let dir = tempfile::tempdir().unwrap();
        let adapter = adapter_in_dir(&dir, "coverage.json");

        let a = make_fragment_ref("src/a.rs", 'a');
        let b = make_fragment_ref("src/b.rs", 'b');
        let mut refs = BTreeSet::new();
        refs.insert(a.clone());
        refs.insert(b.clone());
        let record = DryCheckCoverageRecord::new(refs);

        adapter.write_coverage(&make_track_id(), record.clone()).unwrap();

        let read = adapter.read_coverage(&make_track_id()).unwrap();
        assert_eq!(read, Some(record));
    }

    #[test]
    fn test_write_then_read_round_trips_empty_record() {
        let dir = tempfile::tempdir().unwrap();
        let adapter = adapter_in_dir(&dir, "coverage.json");

        let record = DryCheckCoverageRecord::new(BTreeSet::new());
        adapter.write_coverage(&make_track_id(), record.clone()).unwrap();

        // A persisted empty-set coverage record serializes to a non-empty
        // JSON envelope, so it round-trips as Some(empty), distinct from
        // a missing file (None).
        let read = adapter.read_coverage(&make_track_id()).unwrap();
        assert_eq!(read, Some(record));
    }

    #[test]
    fn test_write_failure_on_invalid_parent_returns_coverage_port_error() {
        // store_path points into a non-existent root the adapter cannot create
        // (a path under a file rather than a directory).
        let dir = tempfile::tempdir().unwrap();
        let blocker = dir.path().join("blocker");
        std::fs::write(&blocker, b"not a dir").unwrap();
        // Use the file `blocker` as a parent — create_dir_all will fail (AlreadyExists / NotADirectory).
        let path = blocker.join("coverage.json");
        let adapter = FsDryCheckCoverageAdapter::new(path, dir.path().to_owned());

        let result =
            adapter.write_coverage(&make_track_id(), DryCheckCoverageRecord::new(BTreeSet::new()));
        assert!(matches!(result, Err(DryCheckCycleError::CoveragePort(_))));
    }

    #[test]
    fn test_read_coverage_with_unsupported_schema_version_returns_coverage_port_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("coverage.json");
        std::fs::write(&path, br#"{ "schema_version": 99, "fragment_refs": [] }"#).unwrap();
        let adapter = FsDryCheckCoverageAdapter::new(path, dir.path().to_owned());

        let result = adapter.read_coverage(&make_track_id());
        assert!(matches!(result, Err(DryCheckCycleError::CoveragePort(_))));
    }

    #[test]
    fn test_read_coverage_with_malformed_json_returns_coverage_port_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("coverage.json");
        std::fs::write(&path, b"not json").unwrap();
        let adapter = FsDryCheckCoverageAdapter::new(path, dir.path().to_owned());

        let result = adapter.read_coverage(&make_track_id());
        assert!(matches!(result, Err(DryCheckCycleError::CoveragePort(_))));
    }

    #[test]
    fn test_read_coverage_with_invalid_path_returns_coverage_port_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("coverage.json");
        std::fs::write(
            &path,
            br#"{
  "schema_version": 1,
  "fragment_refs": [
    { "path": "../src/a.rs", "content_hash": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" }
  ]
}"#,
        )
        .unwrap();
        let adapter = FsDryCheckCoverageAdapter::new(path, dir.path().to_owned());

        let result = adapter.read_coverage(&make_track_id());
        assert!(matches!(result, Err(DryCheckCycleError::CoveragePort(_))));
    }

    #[test]
    fn test_read_coverage_with_invalid_content_hash_returns_coverage_port_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("coverage.json");
        std::fs::write(
            &path,
            br#"{
  "schema_version": 1,
  "fragment_refs": [
    { "path": "src/a.rs", "content_hash": "not-a-sha256" }
  ]
}"#,
        )
        .unwrap();
        let adapter = FsDryCheckCoverageAdapter::new(path, dir.path().to_owned());

        let result = adapter.read_coverage(&make_track_id());
        assert!(matches!(result, Err(DryCheckCycleError::CoveragePort(_))));
    }

    #[test]
    fn test_read_coverage_preserves_fragment_ref_identity_under_same_hash_different_path() {
        let dir = tempfile::tempdir().unwrap();
        let adapter = adapter_in_dir(&dir, "coverage.json");

        // Two refs sharing the same content_hash at different paths.
        let same_hash = 'a';
        let a = make_fragment_ref("src/a.rs", same_hash);
        let b = make_fragment_ref("src/b.rs", same_hash);
        assert_eq!(a.content_hash(), b.content_hash());
        let mut refs = BTreeSet::new();
        refs.insert(a.clone());
        refs.insert(b.clone());

        let record = DryCheckCoverageRecord::new(refs);
        adapter.write_coverage(&make_track_id(), record.clone()).unwrap();

        let read = adapter.read_coverage(&make_track_id()).unwrap().unwrap();
        // Both distinct (path + hash) refs survive the round-trip.
        assert!(read.covers(&a));
        assert!(read.covers(&b));
        assert_eq!(read.fragment_refs().len(), 2);
    }

    #[cfg(unix)]
    #[test]
    fn test_write_coverage_with_symlink_parent_returns_coverage_port_error() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real");
        std::fs::create_dir_all(&real).unwrap();
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let adapter =
            FsDryCheckCoverageAdapter::new(link.join("coverage.json"), dir.path().to_owned());

        let result =
            adapter.write_coverage(&make_track_id(), DryCheckCoverageRecord::new(BTreeSet::new()));
        assert!(matches!(result, Err(DryCheckCycleError::CoveragePort(_))));
    }
}
