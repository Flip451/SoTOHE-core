//! `FsDryCheckStore` — filesystem adapter for dry-check.json.
//!
//! Implements [`DryCheckReader`] and [`DryCheckWriter`] using atomic write and
//! fs4 exclusive file locking. Mirrors `FsReviewStore` from `review_v2`.

use std::path::PathBuf;

use domain::Timestamp;
use domain::dry_check::{
    DryCheckEntry, DryCheckPairKey, DryCheckReader, DryCheckReaderError, DryCheckRecord,
    DryCheckRecordError, DryCheckVerdict, DryCheckWriter, DryCheckWriterError, FragmentContentHash,
    FragmentRef, Rationale, RefactorProposal,
};
use domain::review_v2::FilePath;
use fs4::fs_std::FileExt;

use crate::track::atomic_write::atomic_write_file;
use crate::track::symlink_guard::reject_symlinks_below;

use super::codec::{DryCheckJsonV1, DryCheckRecordDto, DryCheckVerdictDto};

// ── SCHEMA_VERSION ────────────────────────────────────────────────────────────

/// The current schema version written by this implementation.
const CURRENT_SCHEMA_VERSION: u32 = 1;

// ── FsDryCheckStore ───────────────────────────────────────────────────────────

/// Filesystem adapter implementing [`DryCheckReader`] and [`DryCheckWriter`] for
/// dry-check.json.
///
/// The on-disk format is a schema-versioned envelope object
/// `{ "schema_version": 1, "records": [...] }` (private serde DTO
/// `DryCheckJsonV1`). Uses atomic write and fs4 file locking, mirroring
/// `FsReviewStore` from `review_v2`.
#[derive(Debug)]
pub struct FsDryCheckStore {
    path: PathBuf,
    trusted_root: PathBuf,
}

impl FsDryCheckStore {
    /// Construct a new [`FsDryCheckStore`].
    #[must_use]
    pub fn new(path: PathBuf, trusted_root: PathBuf) -> FsDryCheckStore {
        Self { path, trusted_root }
    }

    /// Reads dry-check.json for read-only queries.
    ///
    /// File absent or empty → empty envelope.
    /// Future schema version (> 1) → returns all records but emits
    /// `IncompatibleSchema` when a future version field is encountered.
    fn read_doc(&self) -> Result<DryCheckJsonV1, DryCheckReaderError> {
        let path_str = self.path.display().to_string();

        reject_symlinks_below(&self.path, &self.trusted_root).map_err(|e| {
            if e.kind() == std::io::ErrorKind::InvalidInput {
                DryCheckReaderError::SymlinkDetected { path: path_str.clone() }
            } else {
                DryCheckReaderError::Io { path: path_str.clone(), detail: e.to_string() }
            }
        })?;

        let content = match std::fs::read_to_string(&self.path) {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(DryCheckJsonV1::empty());
            }
            Err(e) => {
                return Err(DryCheckReaderError::Io {
                    path: path_str,
                    detail: format!("read: {e}"),
                });
            }
        };

        if content.trim().is_empty() {
            return Ok(DryCheckJsonV1::empty());
        }

        // Parse envelope to check schema_version first.
        let envelope: serde_json::Value = serde_json::from_str(&content).map_err(|e| {
            DryCheckReaderError::Codec { path: path_str.clone(), detail: format!("parse: {e}") }
        })?;

        let version =
            envelope.get("schema_version").and_then(serde_json::Value::as_u64).unwrap_or(0);

        if version > u64::from(CURRENT_SCHEMA_VERSION) {
            return Err(DryCheckReaderError::IncompatibleSchema { version });
        }

        let doc: DryCheckJsonV1 = serde_json::from_value(envelope).map_err(|e| {
            DryCheckReaderError::Codec { path: path_str, detail: format!("parse v1: {e}") }
        })?;

        Ok(doc)
    }

    /// Reads dry-check.json under an exclusive lock for read-modify-write.
    ///
    /// File absent → empty envelope (init-on-first-write).
    /// Future schema version → writer error (refuse to overwrite unknown format).
    fn read_doc_for_write(&self) -> Result<DryCheckJsonV1, DryCheckWriterError> {
        let path_str = self.path.display().to_string();

        let content = match std::fs::read_to_string(&self.path) {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(DryCheckJsonV1::empty());
            }
            Err(e) => {
                return Err(DryCheckWriterError::Io {
                    path: path_str,
                    detail: format!("read-for-write: {e}"),
                });
            }
        };

        if content.trim().is_empty() {
            return Ok(DryCheckJsonV1::empty());
        }

        let envelope: serde_json::Value = serde_json::from_str(&content)
            .map_err(|e| DryCheckWriterError::Codec { detail: format!("parse: {e}") })?;

        let version =
            envelope.get("schema_version").and_then(serde_json::Value::as_u64).unwrap_or(0);

        if version > u64::from(CURRENT_SCHEMA_VERSION) {
            return Err(DryCheckWriterError::IncompatibleSchema { version });
        }

        let doc: DryCheckJsonV1 = serde_json::from_value(envelope)
            .map_err(|e| DryCheckWriterError::Codec { detail: format!("parse v1: {e}") })?;

        Ok(doc)
    }

    /// Acquire an exclusive fs4 lock on `<path>.lock`, ensuring the parent
    /// directory exists. Returns the lock file (held open for RAII).
    fn acquire_write_lock(&self) -> Result<std::fs::File, DryCheckWriterError> {
        let path_str = self.path.display().to_string();

        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| DryCheckWriterError::Io {
                path: path_str.clone(),
                detail: format!("create dir: {e}"),
            })?;
        }

        let lock_path = self.path.with_extension("json.lock");
        let lock_file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(&lock_path)
            .map_err(|e| DryCheckWriterError::Io {
                path: lock_path.display().to_string(),
                detail: format!("open lock: {e}"),
            })?;

        lock_file.lock_exclusive().map_err(|e| DryCheckWriterError::Io {
            path: lock_path.display().to_string(),
            detail: format!("acquire lock: {e}"),
        })?;

        Ok(lock_file)
    }

    /// Serialize `doc` to pretty JSON and write it atomically.
    fn write_doc(&self, doc: &DryCheckJsonV1) -> Result<(), DryCheckWriterError> {
        let json = serde_json::to_string_pretty(doc)
            .map_err(|e| DryCheckWriterError::Codec { detail: format!("serialize: {e}") })?;
        atomic_write_file(&self.path, json.as_bytes()).map_err(|e| DryCheckWriterError::Io {
            path: self.path.display().to_string(),
            detail: format!("atomic write: {e}"),
        })
    }
}

// ── DryCheckWriter ────────────────────────────────────────────────────────────

impl DryCheckWriter for FsDryCheckStore {
    fn append_record(&self, entry: &DryCheckEntry) -> Result<(), DryCheckWriterError> {
        let path_str = self.path.display().to_string();

        // Symlink check before acquiring lock.
        reject_symlinks_below(&self.path, &self.trusted_root).map_err(|e| {
            if e.kind() == std::io::ErrorKind::InvalidInput {
                DryCheckWriterError::SymlinkDetected { path: path_str.clone() }
            } else {
                DryCheckWriterError::Io { path: path_str.clone(), detail: e.to_string() }
            }
        })?;

        // Stamp the timestamp — adapter is sole owner of recorded_at.
        let ts: Timestamp = crate::timestamp_now()
            .map_err(|e| DryCheckWriterError::Codec { detail: format!("timestamp_now: {e}") })?;

        let recorded_at = ts.as_str().to_owned();

        // Acquire exclusive lock.
        let _guard = self.acquire_write_lock()?;

        // Read envelope under lock (init-on-first-write: absent → empty envelope).
        let mut doc = self.read_doc_for_write()?;

        // Build the DTO from the entry.
        let verdict_dto = match entry.verdict() {
            DryCheckVerdict::NotAViolation => DryCheckVerdictDto::NotAViolation,
            DryCheckVerdict::Accepted => DryCheckVerdictDto::Accepted,
            DryCheckVerdict::Violation { refactor_proposal } => DryCheckVerdictDto::Violation {
                refactor_proposal: refactor_proposal.as_str().to_owned(),
            },
        };

        let dto = DryCheckRecordDto {
            low_path: entry.pair_key().low().path().as_str().to_owned(),
            low_hash: entry.pair_key().low().content_hash().as_str().to_owned(),
            high_path: entry.pair_key().high().path().as_str().to_owned(),
            high_hash: entry.pair_key().high().content_hash().as_str().to_owned(),
            changed_path: entry.changed_path().as_str().to_owned(),
            verdict: verdict_dto,
            similarity_score: f64::from(entry.similarity_score().value()),
            threshold: f64::from(entry.threshold().value()),
            base_commit: entry.base_commit().as_ref().to_owned(),
            rationale: entry.rationale().as_str().to_owned(),
            recorded_at,
        };

        doc.records.push(dto);

        self.write_doc(&doc)
    }
}

// ── DryCheckReader ────────────────────────────────────────────────────────────

impl DryCheckReader for FsDryCheckStore {
    fn read_records(&self) -> Result<Vec<DryCheckRecord>, DryCheckReaderError> {
        let doc = self.read_doc()?;
        let mut records = Vec::with_capacity(doc.records.len());

        for dto in doc.records {
            let record = dto_to_domain(dto).map_err(DryCheckReaderError::InvalidData)?;
            records.push(record);
        }

        Ok(records)
    }
}

// ── DTO → domain conversion ───────────────────────────────────────────────────

/// Convert a [`DryCheckRecordDto`] to a [`DryCheckRecord`].
///
/// Any validation failure (invalid hash, self-match, empty proposal/rationale,
/// invalid timestamp, changed_path outside pair) returns `Err(String)` which
/// the caller maps to `DryCheckReaderError::InvalidData`.
fn dto_to_domain(dto: DryCheckRecordDto) -> Result<DryCheckRecord, String> {
    // Reconstruct the two FragmentRefs from the four flat fields.
    let low_content_hash =
        FragmentContentHash::new(&dto.low_hash).map_err(|e| format!("low_hash: {e}"))?;
    let low_file_path = FilePath::new(&dto.low_path).map_err(|e| format!("low_path: {e}"))?;
    let low_ref = FragmentRef::new(low_file_path, low_content_hash);

    let high_content_hash =
        FragmentContentHash::new(&dto.high_hash).map_err(|e| format!("high_hash: {e}"))?;
    let high_file_path = FilePath::new(&dto.high_path).map_err(|e| format!("high_path: {e}"))?;
    let high_ref = FragmentRef::new(high_file_path, high_content_hash);

    // DryCheckPairKey::new rejects self-match (both path AND hash equal).
    let pair_key = DryCheckPairKey::new(low_ref, high_ref).map_err(|e| format!("pair_key: {e}"))?;

    // Verdict reconstruction.
    let verdict = match dto.verdict {
        DryCheckVerdictDto::NotAViolation => DryCheckVerdict::NotAViolation,
        DryCheckVerdictDto::Accepted => DryCheckVerdict::Accepted,
        DryCheckVerdictDto::Violation { refactor_proposal } => {
            let proposal = RefactorProposal::new(refactor_proposal)
                .map_err(|e| format!("refactor_proposal: {e}"))?;
            DryCheckVerdict::Violation { refactor_proposal: proposal }
        }
    };

    let rationale = Rationale::new(&dto.rationale).map_err(|e| format!("rationale: {e}"))?;
    let recorded_at = Timestamp::new(&dto.recorded_at).map_err(|e| format!("recorded_at: {e}"))?;
    let changed_path =
        FilePath::new(&dto.changed_path).map_err(|e| format!("changed_path: {e}"))?;
    let base_commit =
        domain::CommitHash::try_new(&dto.base_commit).map_err(|e| format!("base_commit: {e}"))?;

    // Validate persisted f64 values before the lossy f32 conversion used by domain types.
    let similarity_score_value =
        validate_unit_interval_f64(dto.similarity_score, "similarity_score")?;
    let threshold_value = validate_unit_interval_f64(dto.threshold, "threshold")?;
    let similarity_score = domain::semantic_dup::SimilarityScore::new(similarity_score_value)
        .map_err(|e| format!("similarity_score: {e}"))?;
    let threshold = domain::semantic_dup::SimilarityThreshold::new(threshold_value)
        .map_err(|e| format!("threshold: {e}"))?;

    // Build entry first to validate changed_path-in-pair invariant.
    let entry = DryCheckEntry::new(
        pair_key,
        changed_path,
        verdict,
        similarity_score,
        threshold,
        base_commit,
        rationale,
    )
    .map_err(|e| format!("entry: {e}"))?;

    // Build record (stamps recorded_at; defence-in-depth validation).
    DryCheckRecord::from_entry_and_timestamp(entry, recorded_at)
        .map_err(|e: DryCheckRecordError| format!("record: {e}"))
}

fn validate_unit_interval_f64(value: f64, field: &str) -> Result<f32, String> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(format!("{field}: value must be finite and in [0, 1], got {value}"));
    }

    Ok(value as f32)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use domain::CommitHash;
    use domain::dry_check::{
        DryCheckEntry, DryCheckPairKey, DryCheckReader, DryCheckReaderError, DryCheckVerdict,
        DryCheckWriter, FragmentContentHash, FragmentRef, Rationale,
    };
    use domain::review_v2::FilePath;
    use domain::semantic_dup::{SimilarityScore, SimilarityThreshold};

    use super::FsDryCheckStore;

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn make_hash(hex: &str) -> FragmentContentHash {
        FragmentContentHash::new(hex).unwrap()
    }

    fn make_file_path(s: &str) -> FilePath {
        FilePath::new(s).unwrap()
    }

    fn make_fragment_ref(path: &str, hash: &str) -> FragmentRef {
        FragmentRef::new(make_file_path(path), make_hash(hash))
    }

    fn make_entry(
        low_path: &str,
        low_hash: &str,
        high_path: &str,
        high_hash: &str,
        changed: &str,
        verdict: DryCheckVerdict,
    ) -> DryCheckEntry {
        let low = make_fragment_ref(low_path, low_hash);
        let high = make_fragment_ref(high_path, high_hash);
        let pair_key = DryCheckPairKey::new(low, high).unwrap();
        let changed_path = make_file_path(changed);
        let score = SimilarityScore::new(0.9).unwrap();
        let threshold = SimilarityThreshold::new(0.8).unwrap();
        let commit = CommitHash::try_new("abcdef1234567").unwrap();
        let rationale = Rationale::new("Test rationale.").unwrap();

        DryCheckEntry::new(pair_key, changed_path, verdict, score, threshold, commit, rationale)
            .unwrap()
    }

    fn store_in(dir: &tempfile::TempDir) -> FsDryCheckStore {
        let path = dir.path().join("dry-check.json");
        FsDryCheckStore::new(path, dir.path().to_owned())
    }

    // ── Tests ─────────────────────────────────────────────────────────────────

    #[test]
    fn test_read_records_on_missing_file_returns_empty_vec() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);
        let records = store.read_records().unwrap();
        assert!(records.is_empty());
    }

    #[test]
    fn test_read_records_future_schema_version_returns_incompatible_schema() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("dry-check.json");
        std::fs::write(&path, r#"{"schema_version": 2, "records": []}"#).unwrap();

        let store = FsDryCheckStore::new(path, dir.path().to_owned());
        let result = store.read_records();

        assert!(matches!(result, Err(DryCheckReaderError::IncompatibleSchema { version: 2 })));
    }

    #[test]
    fn test_read_records_malformed_record_returns_invalid_data() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("dry-check.json");
        let json = serde_json::json!({
            "schema_version": 1,
            "records": [{
                "low_path": "src/a.rs",
                "low_hash": "not-a-sha256",
                "high_path": "src/b.rs",
                "high_hash": "b".repeat(64),
                "changed_path": "src/a.rs",
                "verdict": "not-a-violation",
                "similarity_score": 0.9,
                "threshold": 0.8,
                "base_commit": "abcdef1234567",
                "rationale": "test",
                "recorded_at": "2026-06-01T00:00:00Z"
            }]
        })
        .to_string();
        std::fs::write(&path, json).unwrap();

        let store = FsDryCheckStore::new(path, dir.path().to_owned());
        let result = store.read_records();

        assert!(matches!(result, Err(DryCheckReaderError::InvalidData(_))));
    }

    #[test]
    fn test_first_append_creates_file() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);
        let path = dir.path().join("dry-check.json");

        assert!(!path.exists(), "file should not exist before first append");

        let entry = make_entry(
            "src/a.rs",
            &"a".repeat(64),
            "src/b.rs",
            &"b".repeat(64),
            "src/a.rs",
            DryCheckVerdict::NotAViolation,
        );
        store.append_record(&entry).unwrap();

        assert!(path.exists(), "file should exist after first append");
    }

    #[test]
    fn test_round_trip_all_fields_survive() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);

        let low_hash = "a".repeat(64);
        let high_hash = "b".repeat(64);
        let entry = make_entry(
            "src/a.rs",
            &low_hash,
            "src/b.rs",
            &high_hash,
            "src/a.rs",
            DryCheckVerdict::NotAViolation,
        );

        store.append_record(&entry).unwrap();

        let persisted: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(dir.path().join("dry-check.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(persisted["records"][0]["verdict"], "not-a-violation");

        let records = store.read_records().unwrap();

        assert_eq!(records.len(), 1);
        let rec = &records[0];

        // pair_key fields
        assert_eq!(rec.pair_key().low().path().as_str(), "src/a.rs");
        assert_eq!(rec.pair_key().low().content_hash().as_str(), low_hash);
        assert_eq!(rec.pair_key().high().path().as_str(), "src/b.rs");
        assert_eq!(rec.pair_key().high().content_hash().as_str(), high_hash);

        // other fields
        assert_eq!(rec.changed_path().as_str(), "src/a.rs");
        assert_eq!(rec.verdict(), &DryCheckVerdict::NotAViolation);
        assert!((rec.similarity_score().value() - 0.9_f32).abs() < 0.001);
        assert!((rec.threshold().value() - 0.8_f32).abs() < 0.001);
        assert_eq!(rec.base_commit().as_ref(), "abcdef1234567");
        assert_eq!(rec.rationale().as_str(), "Test rationale.");
        // recorded_at must be a non-empty ISO-8601 string
        assert!(!rec.recorded_at().as_str().is_empty());
    }

    #[test]
    fn test_round_trip_violation_verdict_with_refactor_proposal() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);

        let proposal = domain::dry_check::RefactorProposal::new("Extract helper.").unwrap();
        let entry = make_entry(
            "src/a.rs",
            &"a".repeat(64),
            "src/b.rs",
            &"b".repeat(64),
            "src/a.rs",
            DryCheckVerdict::Violation { refactor_proposal: proposal.clone() },
        );

        store.append_record(&entry).unwrap();

        let persisted: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(dir.path().join("dry-check.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            persisted["records"][0]["verdict"]["violation"]["refactor_proposal"],
            "Extract helper."
        );

        let records = store.read_records().unwrap();

        assert_eq!(records.len(), 1);
        match records[0].verdict() {
            DryCheckVerdict::Violation { refactor_proposal } => {
                assert_eq!(refactor_proposal.as_str(), "Extract helper.");
            }
            other => panic!("expected Violation, got {other:?}"),
        }
    }

    #[test]
    fn test_recorded_at_stamped_by_append_record() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);

        let entry = make_entry(
            "src/a.rs",
            &"a".repeat(64),
            "src/b.rs",
            &"b".repeat(64),
            "src/a.rs",
            DryCheckVerdict::Accepted,
        );
        store.append_record(&entry).unwrap();

        let records = store.read_records().unwrap();
        assert_eq!(records.len(), 1);
        // recorded_at should be an ISO-8601 UTC timestamp (non-empty).
        let ts = records[0].recorded_at().as_str();
        assert!(!ts.is_empty());
        assert!(ts.contains('T'), "recorded_at should contain 'T' separator: {ts}");
        assert!(ts.ends_with('Z'), "recorded_at should end with 'Z': {ts}");
    }

    #[test]
    fn test_multiple_appends_accumulate_records() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);

        for i in 0..3_u8 {
            let low_hash = format!("{:0>64}", i);
            let high_hash = format!("{:0>64}", i + 1);
            let entry = make_entry(
                "src/a.rs",
                &low_hash,
                "src/b.rs",
                &high_hash,
                "src/a.rs",
                DryCheckVerdict::NotAViolation,
            );
            store.append_record(&entry).unwrap();
        }

        let records = store.read_records().unwrap();
        assert_eq!(records.len(), 3);
    }

    #[test]
    fn test_incompatible_schema_on_future_schema_version() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("dry-check.json");

        // Write a file with schema_version = 99 (future).
        let json = r#"{"schema_version": 99, "records": []}"#;
        std::fs::write(&path, json).unwrap();

        let store = FsDryCheckStore::new(path, dir.path().to_owned());
        let result = store.read_records();
        assert!(
            matches!(result, Err(DryCheckReaderError::IncompatibleSchema { version: 99 })),
            "expected IncompatibleSchema(99), got: {result:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_symlink_rejection() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real.json");
        std::fs::write(&real, "{}").unwrap();
        let link = dir.path().join("link.json");
        std::os::unix::fs::symlink(&real, &link).unwrap();

        let store = FsDryCheckStore::new(link, dir.path().to_owned());
        let result = store.read_records();
        assert!(
            matches!(result, Err(DryCheckReaderError::SymlinkDetected { .. })),
            "expected SymlinkDetected, got: {result:?}"
        );
    }

    #[test]
    fn test_read_returns_invalid_data_for_self_match() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("dry-check.json");

        // Craft a record where low_path==high_path AND low_hash==high_hash (self-match).
        let hash = "a".repeat(64);
        let json = serde_json::json!({
            "schema_version": 1,
            "records": [{
                "low_path": "src/a.rs",
                "low_hash": hash,
                "high_path": "src/a.rs",  // same as low_path
                "high_hash": hash,          // same as low_hash
                "changed_path": "src/a.rs",
                "verdict": "not-a-violation",
                "similarity_score": 0.9,
                "threshold": 0.8,
                "base_commit": "abcdef1234567",
                "rationale": "test",
                "recorded_at": "2026-06-01T00:00:00Z"
            }]
        })
        .to_string();
        std::fs::write(&path, json).unwrap();

        let store = FsDryCheckStore::new(path, dir.path().to_owned());
        let result = store.read_records();
        assert!(
            matches!(result, Err(DryCheckReaderError::InvalidData(_))),
            "expected InvalidData for self-match, got: {result:?}"
        );
    }

    #[test]
    fn test_read_returns_invalid_data_for_changed_path_outside_pair() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("dry-check.json");

        let low_hash = "a".repeat(64);
        let high_hash = "b".repeat(64);
        let json = serde_json::json!({
            "schema_version": 1,
            "records": [{
                "low_path": "src/a.rs",
                "low_hash": low_hash,
                "high_path": "src/b.rs",
                "high_hash": high_hash,
                "changed_path": "src/c.rs",  // NOT in pair
                "verdict": "not-a-violation",
                "similarity_score": 0.9,
                "threshold": 0.8,
                "base_commit": "abcdef1234567",
                "rationale": "test",
                "recorded_at": "2026-06-01T00:00:00Z"
            }]
        })
        .to_string();
        std::fs::write(&path, json).unwrap();

        let store = FsDryCheckStore::new(path, dir.path().to_owned());
        let result = store.read_records();
        assert!(
            matches!(result, Err(DryCheckReaderError::InvalidData(_))),
            "expected InvalidData for changed_path outside pair, got: {result:?}"
        );
    }

    #[test]
    fn test_read_returns_invalid_data_for_out_of_range_similarity_score() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("dry-check.json");

        let low_hash = "a".repeat(64);
        let high_hash = "b".repeat(64);
        let json = serde_json::json!({
            "schema_version": 1,
            "records": [{
                "low_path": "src/a.rs",
                "low_hash": low_hash,
                "high_path": "src/b.rs",
                "high_hash": high_hash,
                "changed_path": "src/a.rs",
                "verdict": "not-a-violation",
                "similarity_score": 1.0000000000000002_f64,
                "threshold": 0.8,
                "base_commit": "abcdef1234567",
                "rationale": "test",
                "recorded_at": "2026-06-01T00:00:00Z"
            }]
        })
        .to_string();
        std::fs::write(&path, json).unwrap();

        let store = FsDryCheckStore::new(path, dir.path().to_owned());
        let result = store.read_records();
        assert!(
            matches!(result, Err(DryCheckReaderError::InvalidData(_))),
            "expected InvalidData for out-of-range similarity_score, got: {result:?}"
        );
    }

    #[test]
    fn test_read_returns_invalid_data_for_out_of_range_threshold() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("dry-check.json");

        let low_hash = "a".repeat(64);
        let high_hash = "b".repeat(64);
        let json = serde_json::json!({
            "schema_version": 1,
            "records": [{
                "low_path": "src/a.rs",
                "low_hash": low_hash,
                "high_path": "src/b.rs",
                "high_hash": high_hash,
                "changed_path": "src/a.rs",
                "verdict": "not-a-violation",
                "similarity_score": 0.9,
                "threshold": -1e-50_f64,
                "base_commit": "abcdef1234567",
                "rationale": "test",
                "recorded_at": "2026-06-01T00:00:00Z"
            }]
        })
        .to_string();
        std::fs::write(&path, json).unwrap();

        let store = FsDryCheckStore::new(path, dir.path().to_owned());
        let result = store.read_records();
        assert!(
            matches!(result, Err(DryCheckReaderError::InvalidData(_))),
            "expected InvalidData for out-of-range threshold, got: {result:?}"
        );
    }

    #[test]
    fn test_read_returns_invalid_data_for_empty_rationale() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("dry-check.json");

        let low_hash = "a".repeat(64);
        let high_hash = "b".repeat(64);
        let json = serde_json::json!({
            "schema_version": 1,
            "records": [{
                "low_path": "src/a.rs",
                "low_hash": low_hash,
                "high_path": "src/b.rs",
                "high_hash": high_hash,
                "changed_path": "src/a.rs",
                "verdict": "not-a-violation",
                "similarity_score": 0.9,
                "threshold": 0.8,
                "base_commit": "abcdef1234567",
                "rationale": "",  // empty rationale
                "recorded_at": "2026-06-01T00:00:00Z"
            }]
        })
        .to_string();
        std::fs::write(&path, json).unwrap();

        let store = FsDryCheckStore::new(path, dir.path().to_owned());
        let result = store.read_records();
        assert!(
            matches!(result, Err(DryCheckReaderError::InvalidData(_))),
            "expected InvalidData for empty rationale, got: {result:?}"
        );
    }

    #[test]
    fn test_read_returns_invalid_data_for_empty_refactor_proposal_on_violation() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("dry-check.json");

        let low_hash = "a".repeat(64);
        let high_hash = "b".repeat(64);
        let json = serde_json::json!({
            "schema_version": 1,
            "records": [{
                "low_path": "src/a.rs",
                "low_hash": low_hash,
                "high_path": "src/b.rs",
                "high_hash": high_hash,
                "changed_path": "src/a.rs",
                "verdict": { "violation": { "refactor_proposal": "" } },  // empty proposal
                "similarity_score": 0.9,
                "threshold": 0.8,
                "base_commit": "abcdef1234567",
                "rationale": "test",
                "recorded_at": "2026-06-01T00:00:00Z"
            }]
        })
        .to_string();
        std::fs::write(&path, json).unwrap();

        let store = FsDryCheckStore::new(path, dir.path().to_owned());
        let result = store.read_records();
        assert!(
            matches!(result, Err(DryCheckReaderError::InvalidData(_))),
            "expected InvalidData for empty refactor_proposal, got: {result:?}"
        );
    }
}
