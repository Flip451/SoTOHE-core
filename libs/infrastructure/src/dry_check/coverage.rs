//! `FsDryCheckCoverageAdapter` — filesystem adapter for the D5 coverage manifest.

use std::collections::BTreeSet;
use std::path::PathBuf;

use domain::TrackId;
use domain::dry_check::{
    DryCheckConfigFingerprint, DryCheckCoverageRecord, DryCheckPairKey, FragmentContentHash,
    FragmentRef,
};
use domain::review_v2::FilePath;
use usecase::dry_check::{DryCheckCoveragePort, DryCheckCycleError};

use crate::track::atomic_write::atomic_write_file;
use crate::track::symlink_guard::reject_symlinks_below;

// ── On-disk schema (private serde DTO) ────────────────────────────────────────

/// Schema version written by this implementation.
///
/// Schema v3 adds the `config_fingerprint` field alongside the existing
/// `fragment_refs` and `processed_pair_keys` arrays. Files written with
/// schema_version 2 (which lack `config_fingerprint`) are rejected as
/// unsupported: the gate cannot detect config changes on v2 files, so we
/// fail-closed and require a fresh `dry write` to regenerate in v3 format.
/// Schema v1 files (which lack `processed_pair_keys`) are likewise rejected.
const CURRENT_SCHEMA_VERSION: u32 = 3;

/// On-disk envelope (schema v3):
/// ```json
/// {
///   "schema_version": 3,
///   "config_fingerprint": "<64-char hex>",
///   "fragment_refs": [{ "path": "...", "content_hash": "..." }],
///   "processed_pair_keys": [{ "low": { "path": "...", "content_hash": "..." }, "high": { ... } }]
/// }
/// ```
///
/// Serde lives only here per the hexagonal principle — the domain
/// `DryCheckCoverageRecord` stays serde-free.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct CoverageManifestV3 {
    schema_version: u32,
    /// SHA-256 fingerprint of the `dry-check.json` fields that affect `dry write`
    /// semantics (threshold, max_parallelism, reasoning efforts, known-bad percents).
    config_fingerprint: String,
    fragment_refs: Vec<FragmentRefDto>,
    processed_pair_keys: Vec<PairKeyDto>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct FragmentRefDto {
    path: String,
    content_hash: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct PairKeyDto {
    low: FragmentRefDto,
    high: FragmentRefDto,
}

// ── DTO helpers ───────────────────────────────────────────────────────────────

/// Convert a [`FragmentRef`] to its on-disk DTO representation.
fn fragment_ref_to_dto(fr: &FragmentRef) -> FragmentRefDto {
    FragmentRefDto {
        path: fr.path().as_str().to_owned(),
        content_hash: fr.content_hash().as_str().to_owned(),
    }
}

/// Parse a [`FragmentRefDto`] into a domain [`FragmentRef`], returning a
/// [`DryCheckCycleError::CoveragePort`] on invalid field values.
fn parse_fragment_ref_dto(
    dto: FragmentRefDto,
    path_str: &str,
) -> Result<FragmentRef, DryCheckCycleError> {
    let path = FilePath::new(&dto.path)
        .map_err(|e| DryCheckCycleError::CoveragePort(format!("{path_str}: invalid path: {e}")))?;
    let content_hash = FragmentContentHash::new(&dto.content_hash).map_err(|e| {
        DryCheckCycleError::CoveragePort(format!("{path_str}: invalid content_hash: {e}"))
    })?;
    Ok(FragmentRef::new(path, content_hash))
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

        let manifest: CoverageManifestV3 = serde_json::from_str(&content)
            .map_err(|e| DryCheckCycleError::CoveragePort(format!("parse {path_str}: {e}")))?;

        if manifest.schema_version != CURRENT_SCHEMA_VERSION {
            // schema_version 1 files (pre-v2 format) lack processed_pair_keys.
            // schema_version 2 files (pre-v3 format) lack config_fingerprint.
            // Treating them as having an empty pair-key set or zero fingerprint
            // would cause the gate to behave incorrectly.
            // Fail-closed: reject and require a fresh `dry write` to regenerate
            // a v3 manifest.
            return Err(DryCheckCycleError::CoveragePort(format!(
                "{path_str}: unsupported schema_version {} (expected {}); \
                 run `dry write` to regenerate the coverage manifest in the current format",
                manifest.schema_version, CURRENT_SCHEMA_VERSION
            )));
        }

        let config_fingerprint = DryCheckConfigFingerprint::new(&manifest.config_fingerprint)
            .map_err(|e| {
                DryCheckCycleError::CoveragePort(format!(
                    "{path_str}: invalid config_fingerprint: {e}"
                ))
            })?;

        let mut fragment_refs = BTreeSet::new();
        for dto in manifest.fragment_refs {
            let fr = parse_fragment_ref_dto(dto, &path_str)?;
            fragment_refs.insert(fr);
        }

        let mut processed_pair_keys = BTreeSet::new();
        for dto in manifest.processed_pair_keys {
            let low = parse_fragment_ref_dto(dto.low, &path_str)?;
            let high = parse_fragment_ref_dto(dto.high, &path_str)?;
            let pair_key = DryCheckPairKey::new(low, high).map_err(|e| {
                DryCheckCycleError::CoveragePort(format!(
                    "{path_str}: invalid pair key in processed_pair_keys: {e}"
                ))
            })?;
            processed_pair_keys.insert(pair_key);
        }

        Ok(Some(DryCheckCoverageRecord::new(
            fragment_refs,
            processed_pair_keys,
            config_fingerprint,
        )))
    }

    fn write_coverage(
        &self,
        _track_id: &TrackId,
        record: DryCheckCoverageRecord,
    ) -> Result<(), DryCheckCycleError> {
        let path_str = self.store_path.display().to_string();

        self.reject_symlinks(&path_str)?;

        let manifest = CoverageManifestV3 {
            schema_version: CURRENT_SCHEMA_VERSION,
            config_fingerprint: record.config_fingerprint().as_str().to_owned(),
            fragment_refs: record.fragment_refs().iter().map(fragment_ref_to_dto).collect(),
            processed_pair_keys: record
                .processed_pair_keys()
                .iter()
                .map(|pk| PairKeyDto {
                    low: fragment_ref_to_dto(pk.low()),
                    high: fragment_ref_to_dto(pk.high()),
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

    fn test_fingerprint() -> DryCheckConfigFingerprint {
        DryCheckConfigFingerprint::new("a".repeat(64)).unwrap()
    }

    /// Write `record` and read it back via the adapter.
    ///
    /// Centralises the write + read round-trip scaffolding so that individual
    /// tests only vary the record under test and their specific assertions.
    fn round_trip_coverage(
        record: DryCheckCoverageRecord,
        adapter: &FsDryCheckCoverageAdapter,
    ) -> DryCheckCoverageRecord {
        adapter.write_coverage(&make_track_id(), record).unwrap();
        adapter.read_coverage(&make_track_id()).unwrap().unwrap()
    }

    #[test]
    fn test_read_coverage_returns_none_when_file_missing() {
        let dir = tempfile::tempdir().unwrap();
        let adapter = adapter_in_dir(&dir, "coverage.json");
        let result = adapter.read_coverage(&make_track_id()).unwrap();
        assert!(result.is_none());
    }

    fn make_pair_key(path_a: &str, hash_a: char, path_b: &str, hash_b: char) -> DryCheckPairKey {
        let a = make_fragment_ref(path_a, hash_a);
        let b = make_fragment_ref(path_b, hash_b);
        DryCheckPairKey::new(a, b).unwrap()
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
        let record = DryCheckCoverageRecord::new(refs, BTreeSet::new(), test_fingerprint());

        let read = round_trip_coverage(record.clone(), &adapter);
        assert_eq!(read, record);
    }

    #[test]
    fn test_write_then_read_round_trips_record_with_pair_keys() {
        // Schema v3: fragment_refs, processed_pair_keys, and config_fingerprint
        // all survive the round-trip.
        let dir = tempfile::tempdir().unwrap();
        let adapter = adapter_in_dir(&dir, "coverage.json");

        let a = make_fragment_ref("src/a.rs", 'a');
        let b = make_fragment_ref("src/b.rs", 'b');
        let pair = make_pair_key("src/a.rs", 'a', "src/b.rs", 'b');

        let mut refs = BTreeSet::new();
        refs.insert(a.clone());
        refs.insert(b.clone());
        let mut pairs = BTreeSet::new();
        pairs.insert(pair.clone());
        let record = DryCheckCoverageRecord::new(refs, pairs, test_fingerprint());

        let read = round_trip_coverage(record.clone(), &adapter);
        assert_eq!(read, record);
        assert!(read.covers(&a));
        assert!(read.covers(&b));
        assert!(read.contains_pair(&pair));
        assert_eq!(read.config_fingerprint(), &test_fingerprint());
    }

    #[test]
    fn test_write_then_read_round_trips_config_fingerprint() {
        // Schema v3: the config_fingerprint survives the round-trip unchanged.
        let dir = tempfile::tempdir().unwrap();
        let adapter = adapter_in_dir(&dir, "coverage.json");

        let fp = DryCheckConfigFingerprint::new("b".repeat(64)).unwrap();
        let record = DryCheckCoverageRecord::new(BTreeSet::new(), BTreeSet::new(), fp.clone());
        adapter.write_coverage(&make_track_id(), record.clone()).unwrap();

        let read = adapter.read_coverage(&make_track_id()).unwrap().unwrap();
        assert_eq!(read.config_fingerprint(), &fp);
    }

    #[test]
    fn test_write_then_read_round_trips_empty_record() {
        let dir = tempfile::tempdir().unwrap();
        let adapter = adapter_in_dir(&dir, "coverage.json");

        let record =
            DryCheckCoverageRecord::new(BTreeSet::new(), BTreeSet::new(), test_fingerprint());
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

        let result = adapter.write_coverage(
            &make_track_id(),
            DryCheckCoverageRecord::new(BTreeSet::new(), BTreeSet::new(), test_fingerprint()),
        );
        assert!(matches!(result, Err(DryCheckCycleError::CoveragePort(_))));
    }

    #[test]
    fn test_read_coverage_with_unsupported_schema_version_returns_coverage_port_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("coverage.json");
        std::fs::write(
            &path,
            br#"{ "schema_version": 99, "config_fingerprint": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "fragment_refs": [], "processed_pair_keys": [] }"#,
        )
        .unwrap();
        let adapter = FsDryCheckCoverageAdapter::new(path, dir.path().to_owned());

        let result = adapter.read_coverage(&make_track_id());
        assert!(matches!(result, Err(DryCheckCycleError::CoveragePort(_))));
    }

    #[test]
    fn test_read_coverage_with_schema_version_1_returns_coverage_port_error() {
        // schema_version 1 files lack processed_pair_keys; the adapter must
        // reject them (fail-closed) so the user is prompted to run `dry write`.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("coverage.json");
        std::fs::write(&path, br#"{ "schema_version": 1, "fragment_refs": [] }"#).unwrap();
        let adapter = FsDryCheckCoverageAdapter::new(path, dir.path().to_owned());

        let result = adapter.read_coverage(&make_track_id());
        assert!(matches!(result, Err(DryCheckCycleError::CoveragePort(_))));
    }

    #[test]
    fn test_read_coverage_with_schema_version_2_returns_coverage_port_error() {
        // schema_version 2 files lack config_fingerprint; the adapter must
        // reject them (fail-closed) so the user is prompted to run `dry write`.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("coverage.json");
        std::fs::write(
            &path,
            br#"{ "schema_version": 2, "fragment_refs": [], "processed_pair_keys": [] }"#,
        )
        .unwrap();
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
  "schema_version": 3,
  "config_fingerprint": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  "fragment_refs": [
    { "path": "../src/a.rs", "content_hash": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" }
  ],
  "processed_pair_keys": []
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
  "schema_version": 3,
  "config_fingerprint": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  "fragment_refs": [
    { "path": "src/a.rs", "content_hash": "not-a-sha256" }
  ],
  "processed_pair_keys": []
}"#,
        )
        .unwrap();
        let adapter = FsDryCheckCoverageAdapter::new(path, dir.path().to_owned());

        let result = adapter.read_coverage(&make_track_id());
        assert!(matches!(result, Err(DryCheckCycleError::CoveragePort(_))));
    }

    #[test]
    fn test_read_coverage_with_invalid_config_fingerprint_returns_coverage_port_error() {
        // The config_fingerprint field must be a valid 64-char lowercase hex string.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("coverage.json");
        std::fs::write(
            &path,
            br#"{
  "schema_version": 3,
  "config_fingerprint": "not-a-valid-fingerprint",
  "fragment_refs": [],
  "processed_pair_keys": []
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

        let record = DryCheckCoverageRecord::new(refs, BTreeSet::new(), test_fingerprint());
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

        let result = adapter.write_coverage(
            &make_track_id(),
            DryCheckCoverageRecord::new(BTreeSet::new(), BTreeSet::new(), test_fingerprint()),
        );
        assert!(matches!(result, Err(DryCheckCycleError::CoveragePort(_))));
    }
}
