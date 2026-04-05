use std::collections::HashMap;
use std::path::{Path, PathBuf};

use domain::CommitHash;
use domain::review_v2::{
    CommitHashError, CommitHashReader, CommitHashWriter, FastVerdict, Finding, ReviewHash,
    ReviewReader, ReviewReaderError, ReviewWriter, ReviewWriterError, ScopeName, Verdict,
};
use fs4::fs_std::FileExt;

use crate::git_cli::{GitRepository, SystemGitRepo};
use crate::track::atomic_write::atomic_write_file;
use crate::track::symlink_guard::reject_symlinks_below;

// ── review.json v2 serde types ────────────────────────────────────────

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct ReviewJsonV2 {
    schema_version: u32,
    scopes: HashMap<String, ScopeEntry>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct ScopeEntry {
    rounds: Vec<RoundEntry>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct RoundEntry {
    #[serde(rename = "type")]
    round_type: String,
    verdict: String,
    findings: Vec<FindingEntry>,
    hash: String,
    at: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct FindingEntry {
    message: String,
    severity: Option<String>,
    file: Option<String>,
    line: Option<u64>,
    #[serde(default)]
    category: Option<String>,
}

impl ReviewJsonV2 {
    fn empty() -> Self {
        Self { schema_version: 2, scopes: HashMap::new() }
    }
}

// ── Internal error type ──────────────────────────────────────────────

/// Structured internal error for persistence operations.
/// Converted to domain error types (`ReviewReaderError` / `ReviewWriterError`)
/// at the port boundary — never exposed outside this module.
#[derive(Debug)]
enum PersistenceError {
    Io { operation: &'static str, path: PathBuf, source: std::io::Error },
    Codec { operation: &'static str, detail: String },
    FutureSchema { version: u64 },
}

impl PersistenceError {
    fn into_writer_error(self) -> ReviewWriterError {
        match self {
            Self::Io { operation, path, source } => {
                ReviewWriterError::Io(format!("{operation} {}: {source}", path.display()))
            }
            Self::Codec { operation, detail } => {
                ReviewWriterError::Codec(format!("{operation}: {detail}"))
            }
            Self::FutureSchema { version } => ReviewWriterError::IncompatibleSchema { version },
        }
    }

    fn into_reader_error(self) -> ReviewReaderError {
        match self {
            Self::Io { operation, path, source } => {
                ReviewReaderError::Io(format!("{operation} {}: {source}", path.display()))
            }
            Self::Codec { operation, detail } => {
                ReviewReaderError::Codec(format!("{operation}: {detail}"))
            }
            Self::FutureSchema { version } => ReviewReaderError::InvalidData(format!(
                "review.json has schema_version {version} (unknown future version)"
            )),
        }
    }
}

// ── Write guard (RAII lock holder) ───────────────────────────────────

/// RAII guard that validates the write path and holds an exclusive `fs4` lock
/// on `<path>.json.lock`. Lock is released on drop.
struct WriteGuard {
    _lock: std::fs::File,
}

impl WriteGuard {
    /// Validates the path for symlinks below `trusted_root`, ensures the parent
    /// directory exists, and acquires an exclusive lock on `<path>.json.lock`.
    fn acquire(path: &Path, trusted_root: &Path) -> Result<Self, PersistenceError> {
        reject_symlinks_below(path, trusted_root).map_err(|source| PersistenceError::Io {
            operation: "symlink check",
            path: path.to_owned(),
            source,
        })?;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| PersistenceError::Io {
                operation: "create dir",
                path: parent.to_owned(),
                source,
            })?;
        }

        let lock_path = path.with_extension("json.lock");
        let lock_file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(&lock_path)
            .map_err(|source| PersistenceError::Io {
                operation: "open lock",
                path: lock_path.clone(),
                source,
            })?;

        lock_file.lock_exclusive().map_err(|source| PersistenceError::Io {
            operation: "acquire lock",
            path: lock_path,
            source,
        })?;

        Ok(Self { _lock: lock_file })
    }
}

/// Serializes `doc` to pretty JSON and writes it atomically (tmp + fsync + rename).
/// Must be called while a `WriteGuard` is held.
fn write_atomic(path: &Path, doc: &ReviewJsonV2) -> Result<(), PersistenceError> {
    let json = serde_json::to_string_pretty(doc)
        .map_err(|e| PersistenceError::Codec { operation: "serialize", detail: e.to_string() })?;
    atomic_write_file(path, json.as_bytes()).map_err(|source| PersistenceError::Io {
        operation: "atomic write",
        path: path.to_owned(),
        source,
    })
}

// ── FsReviewReader / FsReviewWriter ───────────────────────────────────

/// Filesystem-based review.json v2 reader/writer with fs4 file locking.
pub struct FsReviewStore {
    path: PathBuf,
    trusted_root: PathBuf,
}

impl FsReviewStore {
    #[must_use]
    pub fn new(review_json_path: PathBuf, trusted_root: PathBuf) -> Self {
        Self { path: review_json_path, trusted_root }
    }

    /// Reads review.json for read-only queries.
    /// v1/missing → empty (fail-closed: all scopes need review).
    /// Future versions (>2) → empty (same rationale).
    fn read_doc(&self) -> Result<ReviewJsonV2, ReviewReaderError> {
        self.read_doc_inner(false).map_err(PersistenceError::into_reader_error)
    }

    /// Reads review.json for read-modify-write.
    /// v1/missing → empty (safe to init).
    /// Future versions (>2) → error (refuse to overwrite unknown format).
    fn read_doc_for_write(&self) -> Result<ReviewJsonV2, ReviewWriterError> {
        self.read_doc_inner(true).map_err(PersistenceError::into_writer_error)
    }

    fn read_doc_inner(&self, reject_future: bool) -> Result<ReviewJsonV2, PersistenceError> {
        let content = match std::fs::read_to_string(&self.path) {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(ReviewJsonV2::empty());
            }
            Err(e) => {
                return Err(PersistenceError::Io {
                    operation: "read",
                    path: self.path.clone(),
                    source: e,
                });
            }
        };

        let envelope: serde_json::Value =
            serde_json::from_str(&content).map_err(|e| PersistenceError::Codec {
                operation: "parse",
                detail: format!("{}: {e}", self.path.display()),
            })?;
        let version =
            envelope.get("schema_version").and_then(serde_json::Value::as_u64).unwrap_or(0);

        if version == 2 {
            let doc: ReviewJsonV2 =
                serde_json::from_value(envelope).map_err(|e| PersistenceError::Codec {
                    operation: "parse v2",
                    detail: format!("{}: {e}", self.path.display()),
                })?;
            return Ok(doc);
        }

        // v1 (known legacy) → treat as empty
        if version <= 1 {
            return Ok(ReviewJsonV2::empty());
        }

        // Future version (>2)
        if reject_future {
            return Err(PersistenceError::FutureSchema { version });
        }

        // Read-only path: treat as empty (fail-closed)
        Ok(ReviewJsonV2::empty())
    }

    fn write_doc(&self, doc: &ReviewJsonV2) -> Result<(), ReviewWriterError> {
        let _guard = WriteGuard::acquire(&self.path, &self.trusted_root)
            .map_err(PersistenceError::into_writer_error)?;
        write_atomic(&self.path, doc).map_err(PersistenceError::into_writer_error)
    }

    /// Atomically reads, appends a round, and writes under exclusive lock.
    fn append_round(
        &self,
        scope: &ScopeName,
        round_type: &str,
        verdict_str: &str,
        findings: &[Finding],
        hash: &ReviewHash,
    ) -> Result<(), ReviewWriterError> {
        let _guard = WriteGuard::acquire(&self.path, &self.trusted_root)
            .map_err(PersistenceError::into_writer_error)?;

        // Read under lock (reject future schema versions to prevent overwrite)
        let mut doc = self.read_doc_for_write()?;

        let scope_key = scope.to_string();
        let entry = doc.scopes.entry(scope_key).or_insert_with(|| ScopeEntry { rounds: vec![] });

        let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
        entry.rounds.push(RoundEntry {
            round_type: round_type.to_owned(),
            verdict: verdict_str.to_owned(),
            findings: findings
                .iter()
                .map(|f| FindingEntry {
                    message: f.message().to_owned(),
                    severity: f.severity().map(str::to_owned),
                    file: f.file().map(str::to_owned),
                    line: f.line(),
                    category: f.category().map(str::to_owned),
                })
                .collect(),
            hash: hash.as_str().unwrap_or("").to_owned(),
            at: now,
        });

        write_atomic(&self.path, &doc).map_err(PersistenceError::into_writer_error)
    }
}

impl ReviewReader for FsReviewStore {
    fn read_latest_finals(
        &self,
    ) -> Result<HashMap<ScopeName, (Verdict, ReviewHash)>, ReviewReaderError> {
        let doc = self.read_doc()?;
        let mut result = HashMap::new();

        for (scope_key, entry) in &doc.scopes {
            // Find the latest "final" round
            let latest_final = entry.rounds.iter().rev().find(|r| r.round_type == "final");
            if let Some(round) = latest_final {
                let scope = parse_scope_name(scope_key)?;
                let verdict = parse_verdict(&round.verdict, &round.findings)?;
                let hash = if round.hash.is_empty() {
                    ReviewHash::Empty
                } else {
                    ReviewHash::computed(&round.hash).map_err(|e| {
                        ReviewReaderError::InvalidData(format!("invalid hash in review.json: {e}"))
                    })?
                };
                result.insert(scope, (verdict, hash));
            }
        }

        Ok(result)
    }
}

impl ReviewWriter for FsReviewStore {
    fn write_verdict(
        &self,
        scope: &ScopeName,
        verdict: &Verdict,
        hash: &ReviewHash,
    ) -> Result<(), ReviewWriterError> {
        let (verdict_str, findings) = match verdict {
            Verdict::ZeroFindings => ("zero_findings", vec![]),
            Verdict::FindingsRemain(nef) => ("findings_remain", nef.as_slice().to_vec()),
        };
        self.append_round(scope, "final", verdict_str, &findings, hash)
    }

    fn write_fast_verdict(
        &self,
        scope: &ScopeName,
        verdict: &FastVerdict,
        hash: &ReviewHash,
    ) -> Result<(), ReviewWriterError> {
        let (verdict_str, findings) = match verdict {
            FastVerdict::ZeroFindings => ("zero_findings", vec![]),
            FastVerdict::FindingsRemain(nef) => ("findings_remain", nef.as_slice().to_vec()),
        };
        self.append_round(scope, "fast", verdict_str, &findings, hash)
    }

    fn init(&self) -> Result<(), ReviewWriterError> {
        self.write_doc(&ReviewJsonV2::empty())
    }

    fn reset(&self) -> Result<(), ReviewWriterError> {
        let _guard = WriteGuard::acquire(&self.path, &self.trusted_root)
            .map_err(PersistenceError::into_writer_error)?;

        // Archive existing file if present
        if self.path.exists() {
            let now = chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
            let archive_name = format!("review-{now}.json");
            if let Some(parent) = self.path.parent() {
                let archive_path = parent.join(archive_name);
                std::fs::rename(&self.path, &archive_path).map_err(|e| {
                    ReviewWriterError::Io(format!(
                        "archive {} → {}: {e}",
                        self.path.display(),
                        archive_path.display()
                    ))
                })?;
            }
        }

        // Create fresh review.json (does NOT clear .commit_hash per ADR)
        write_atomic(&self.path, &ReviewJsonV2::empty())
            .map_err(PersistenceError::into_writer_error)
    }
}

// ── FsCommitHashReader / FsCommitHashWriter ───────────────────────────

/// Filesystem-based .commit_hash reader with ancestry validation.
pub struct FsCommitHashStore {
    path: PathBuf,
}

impl FsCommitHashStore {
    #[must_use]
    pub fn new(commit_hash_path: PathBuf) -> Self {
        Self { path: commit_hash_path }
    }
}

impl CommitHashReader for FsCommitHashStore {
    fn read(&self) -> Result<Option<CommitHash>, CommitHashError> {
        let content = match std::fs::read_to_string(&self.path) {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => {
                return Err(CommitHashError::Io(format!("read {}: {e}", self.path.display())));
            }
        };

        let trimmed = content.trim();
        let hash = CommitHash::try_new(trimmed).map_err(|e| {
            CommitHashError::Format(format!("invalid commit hash in {}: {e}", self.path.display()))
        })?;

        // Ancestry validation (infra implementation detail, not trait contract)
        match SystemGitRepo::discover() {
            Ok(git) => {
                let output = git.output(&["merge-base", "--is-ancestor", trimmed, "HEAD"]);
                match output {
                    Ok(o) if o.status.success() => Ok(Some(hash)),
                    _ => Ok(None), // fail-closed: scope expands
                }
            }
            Err(_) => Ok(None), // git unavailable → fail-closed
        }
    }
}

impl CommitHashWriter for FsCommitHashStore {
    fn write(&self, hash: &CommitHash) -> Result<(), CommitHashError> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                CommitHashError::Io(format!("create dir {}: {e}", parent.display()))
            })?;
        }
        atomic_write_file(&self.path, hash.as_ref().as_bytes())
            .map_err(|e| CommitHashError::Io(format!("atomic write {}: {e}", self.path.display())))
    }

    fn clear(&self) -> Result<(), CommitHashError> {
        match std::fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(CommitHashError::Io(format!("remove {}: {e}", self.path.display()))),
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────

fn parse_scope_name(key: &str) -> Result<ScopeName, ReviewReaderError> {
    use domain::review_v2::MainScopeName;
    if key == "other" {
        Ok(ScopeName::Other)
    } else {
        MainScopeName::new(key)
            .map(ScopeName::Main)
            .map_err(|e| ReviewReaderError::InvalidData(format!("invalid scope key '{key}': {e}")))
    }
}

fn parse_verdict(
    verdict_str: &str,
    findings: &[FindingEntry],
) -> Result<Verdict, ReviewReaderError> {
    match verdict_str {
        "zero_findings" => Ok(Verdict::ZeroFindings),
        "findings_remain" => {
            let domain_findings: Result<Vec<Finding>, _> = findings
                .iter()
                .map(|f| {
                    Finding::new(
                        &f.message,
                        f.severity.clone(),
                        f.file.clone(),
                        f.line,
                        f.category.clone(),
                    )
                    .map_err(|e| {
                        ReviewReaderError::InvalidData(format!(
                            "invalid finding in review.json: {e}"
                        ))
                    })
                })
                .collect();
            let domain_findings = domain_findings?;
            Verdict::findings_remain(domain_findings)
                .map_err(|e| ReviewReaderError::InvalidData(format!("verdict construction: {e}")))
        }
        other => Err(ReviewReaderError::InvalidData(format!("unknown verdict: {other}"))),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use super::*;
    use domain::review_v2::{
        FastVerdict, Finding, MainScopeName, ReviewHash, ReviewReader, ReviewWriter, ScopeName,
        Verdict,
    };

    fn make_store(dir: &std::path::Path) -> FsReviewStore {
        FsReviewStore::new(dir.join("review.json"), dir.to_path_buf())
    }

    fn sample_scope() -> ScopeName {
        ScopeName::Main(MainScopeName::new("domain").unwrap())
    }

    fn sample_hash() -> ReviewHash {
        ReviewHash::computed("rvw1:sha256:abcdef0123456789").unwrap()
    }

    fn sample_finding() -> Finding {
        Finding::new(
            "test finding",
            Some("P2".to_owned()),
            Some("lib.rs".to_owned()),
            Some(42),
            Some("style".to_owned()),
        )
        .unwrap()
    }

    // ── init / read basics ──────────────────────────────────────────

    #[test]
    fn test_read_missing_file_returns_empty_map() {
        let dir = tempfile::tempdir().unwrap();
        let store = make_store(dir.path());
        let result = store.read_latest_finals().unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_init_creates_v2_empty_doc() {
        let dir = tempfile::tempdir().unwrap();
        let store = make_store(dir.path());
        store.init().unwrap();

        let content = std::fs::read_to_string(dir.path().join("review.json")).unwrap();
        let value: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(value["schema_version"], 2);
        assert!(value["scopes"].as_object().unwrap().is_empty());
    }

    #[test]
    fn test_read_after_init_returns_empty_map() {
        let dir = tempfile::tempdir().unwrap();
        let store = make_store(dir.path());
        store.init().unwrap();
        let result = store.read_latest_finals().unwrap();
        assert!(result.is_empty());
    }

    // ── write_verdict round trips ───────────────────────────────────

    #[test]
    fn test_write_zero_findings_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let store = make_store(dir.path());
        let scope = sample_scope();
        let hash = sample_hash();

        store.write_verdict(&scope, &Verdict::ZeroFindings, &hash).unwrap();
        let map = store.read_latest_finals().unwrap();

        assert_eq!(map.len(), 1);
        let (verdict, read_hash) = map.get(&scope).unwrap();
        assert!(matches!(verdict, Verdict::ZeroFindings));
        assert_eq!(read_hash, &hash);
    }

    #[test]
    fn test_write_findings_remain_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let store = make_store(dir.path());
        let scope = sample_scope();
        let hash = sample_hash();
        let finding = sample_finding();
        let verdict = Verdict::findings_remain(vec![finding.clone()]).unwrap();

        store.write_verdict(&scope, &verdict, &hash).unwrap();
        let map = store.read_latest_finals().unwrap();

        let (read_verdict, _) = map.get(&scope).unwrap();
        match read_verdict {
            Verdict::FindingsRemain(nef) => {
                assert_eq!(nef.as_slice().len(), 1);
                assert_eq!(nef.as_slice()[0].message(), "test finding");
                assert_eq!(nef.as_slice()[0].severity(), Some("P2"));
                assert_eq!(nef.as_slice()[0].file(), Some("lib.rs"));
                assert_eq!(nef.as_slice()[0].line(), Some(42));
                assert_eq!(nef.as_slice()[0].category(), Some("style"));
            }
            _ => panic!("expected FindingsRemain"),
        }
    }

    #[test]
    fn test_write_fast_verdict_not_in_latest_finals() {
        let dir = tempfile::tempdir().unwrap();
        let store = make_store(dir.path());
        let scope = sample_scope();
        let hash = sample_hash();

        store.write_fast_verdict(&scope, &FastVerdict::ZeroFindings, &hash).unwrap();
        let map = store.read_latest_finals().unwrap();
        // fast rounds are not "final" rounds, so should not appear
        assert!(map.is_empty());
    }

    #[test]
    fn test_multiple_rounds_returns_latest_final() {
        let dir = tempfile::tempdir().unwrap();
        let store = make_store(dir.path());
        let scope = sample_scope();
        let hash = sample_hash();

        // First final round: findings_remain
        let finding = sample_finding();
        let v1 = Verdict::findings_remain(vec![finding]).unwrap();
        store.write_verdict(&scope, &v1, &hash).unwrap();

        // Second final round: zero_findings
        store.write_verdict(&scope, &Verdict::ZeroFindings, &hash).unwrap();

        let map = store.read_latest_finals().unwrap();
        let (verdict, _) = map.get(&scope).unwrap();
        assert!(matches!(verdict, Verdict::ZeroFindings));
    }

    // ── reset ────────────────────────────────────────────────────────

    #[test]
    fn test_reset_archives_and_creates_fresh() {
        let dir = tempfile::tempdir().unwrap();
        let store = make_store(dir.path());
        let scope = sample_scope();
        let hash = sample_hash();

        // Write something
        store.write_verdict(&scope, &Verdict::ZeroFindings, &hash).unwrap();
        assert!(dir.path().join("review.json").exists());

        // Reset
        store.reset().unwrap();

        // Archive file should exist (review-<timestamp>.json)
        let archive_files: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_str()
                    .is_some_and(|n| n.starts_with("review-") && n.ends_with(".json"))
            })
            .collect();
        assert_eq!(archive_files.len(), 1, "expected exactly one archive file");

        // New review.json should be fresh (empty scopes)
        let map = store.read_latest_finals().unwrap();
        assert!(map.is_empty());
    }

    // ── empty hash round trip ───────────────────────────────────────

    #[test]
    fn test_write_empty_hash_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let store = make_store(dir.path());
        let scope = sample_scope();

        store.write_verdict(&scope, &Verdict::ZeroFindings, &ReviewHash::Empty).unwrap();
        let map = store.read_latest_finals().unwrap();
        let (_, read_hash) = map.get(&scope).unwrap();
        assert!(read_hash.is_empty());
    }

    // ── multiple scopes ─────────────────────────────────────────────

    #[test]
    fn test_multiple_scopes_independent() {
        let dir = tempfile::tempdir().unwrap();
        let store = make_store(dir.path());
        let scope1 = ScopeName::Main(MainScopeName::new("domain").unwrap());
        let scope2 = ScopeName::Other;
        let hash = sample_hash();

        store.write_verdict(&scope1, &Verdict::ZeroFindings, &hash).unwrap();
        let finding = sample_finding();
        let v2 = Verdict::findings_remain(vec![finding]).unwrap();
        store.write_verdict(&scope2, &v2, &hash).unwrap();

        let map = store.read_latest_finals().unwrap();
        assert_eq!(map.len(), 2);
        assert!(matches!(map.get(&scope1).unwrap().0, Verdict::ZeroFindings));
        assert!(matches!(map.get(&scope2).unwrap().0, Verdict::FindingsRemain(_)));
    }

    // ── init is idempotent ──────────────────────────────────────────

    #[test]
    fn test_init_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let store = make_store(dir.path());
        store.init().unwrap();
        store.init().unwrap(); // second call should not fail

        let content = std::fs::read_to_string(dir.path().join("review.json")).unwrap();
        let value: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(value["schema_version"], 2);
    }
}
