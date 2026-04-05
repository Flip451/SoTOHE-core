use std::collections::HashMap;
use std::path::PathBuf;

use domain::CommitHash;
use domain::review_v2::{
    CommitHashError, CommitHashReader, CommitHashWriter, FastVerdict, Finding, ReviewHash,
    ReviewReader, ReviewReaderError, ReviewWriter, ReviewWriterError, ScopeName, Verdict,
};

use crate::git_cli::{GitRepository, SystemGitRepo};

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

// ── FsReviewReader / FsReviewWriter ───────────────────────────────────

/// Filesystem-based review.json v2 reader/writer with fs4 file locking.
pub struct FsReviewStore {
    path: PathBuf,
}

impl FsReviewStore {
    #[must_use]
    pub fn new(review_json_path: PathBuf) -> Self {
        Self { path: review_json_path }
    }

    fn read_doc(&self) -> Result<ReviewJsonV2, ReviewReaderError> {
        match std::fs::read_to_string(&self.path) {
            Ok(content) => {
                let doc: ReviewJsonV2 = serde_json::from_str(&content).map_err(|e| {
                    ReviewReaderError::Codec(format!("parse {}: {e}", self.path.display()))
                })?;
                if doc.schema_version != 2 {
                    // v1 or unknown — treat as empty (ADR: v1 is ignored)
                    return Ok(ReviewJsonV2::empty());
                }
                Ok(doc)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(ReviewJsonV2::empty()),
            Err(e) => Err(ReviewReaderError::Io(format!("read {}: {e}", self.path.display()))),
        }
    }

    fn write_doc(&self, doc: &ReviewJsonV2) -> Result<(), ReviewWriterError> {
        use fs4::fs_std::FileExt;

        // Reject symlinks on the target path to prevent symlink traversal attacks
        reject_symlink_chain(&self.path)?;

        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                ReviewWriterError::Io(format!("create dir {}: {e}", parent.display()))
            })?;
        }

        let file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(&self.path)
            .map_err(|e| ReviewWriterError::Io(format!("open {}: {e}", self.path.display())))?;

        file.lock_exclusive()
            .map_err(|e| ReviewWriterError::Io(format!("lock {}: {e}", self.path.display())))?;

        let json = serde_json::to_string_pretty(doc)
            .map_err(|e| ReviewWriterError::Codec(format!("serialize review.json: {e}")))?;
        atomic_write(&self.path, &json)?;

        // Lock released on drop
        drop(file);

        Ok(())
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
        use fs4::fs_std::FileExt;

        // Reject symlinks on the target path and all ancestors
        reject_symlink_chain(&self.path)?;

        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                ReviewWriterError::Io(format!("create dir {}: {e}", parent.display()))
            })?;
        }

        // Acquire exclusive lock BEFORE reading to prevent TOCTOU
        let lock_file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(&self.path)
            .map_err(|e| ReviewWriterError::Io(format!("open {}: {e}", self.path.display())))?;
        lock_file
            .lock_exclusive()
            .map_err(|e| ReviewWriterError::Io(format!("lock {}: {e}", self.path.display())))?;

        // Read under lock
        let mut doc = self.read_doc().map_err(|e| ReviewWriterError::Io(e.to_string()))?;

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

        // Write under the same lock (atomic: tmp + rename)
        let json = serde_json::to_string_pretty(&doc)
            .map_err(|e| ReviewWriterError::Codec(format!("serialize review.json: {e}")))?;
        atomic_write(&self.path, &json)?;

        // Lock released on drop
        drop(lock_file);
        Ok(())
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
                        ReviewReaderError::Codec(format!("invalid hash in review.json: {e}"))
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
        // Archive existing file if present
        if self.path.exists() {
            let now = chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
            let archive_name = format!("review-{now}.json",);
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
        self.write_doc(&ReviewJsonV2::empty())
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
        // Atomic write: tmp file + rename
        let tmp_path = self.path.with_extension("tmp");
        std::fs::write(&tmp_path, hash.as_ref())
            .map_err(|e| CommitHashError::Io(format!("write {}: {e}", tmp_path.display())))?;
        std::fs::rename(&tmp_path, &self.path)
            .map_err(|e| CommitHashError::Io(format!("rename {}: {e}", self.path.display())))?;
        Ok(())
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
            .map_err(|e| ReviewReaderError::Codec(format!("invalid scope key '{key}': {e}")))
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
                        ReviewReaderError::Codec(format!("invalid finding in review.json: {e}"))
                    })
                })
                .collect();
            let domain_findings = domain_findings?;
            Verdict::findings_remain(domain_findings)
                .map_err(|e| ReviewReaderError::Codec(format!("verdict construction: {e}")))
        }
        other => Err(ReviewReaderError::Codec(format!("unknown verdict: {other}"))),
    }
}

/// Writes content to a file atomically via tmp + rename.
fn atomic_write(path: &std::path::Path, content: &str) -> Result<(), ReviewWriterError> {
    let tmp_path = path.with_extension("tmp");
    std::fs::write(&tmp_path, content)
        .map_err(|e| ReviewWriterError::Io(format!("write {}: {e}", tmp_path.display())))?;
    std::fs::rename(&tmp_path, path)
        .map_err(|e| ReviewWriterError::Io(format!("rename {}: {e}", path.display())))?;
    Ok(())
}

/// Rejects a path if it or any ancestor is a symlink.
fn reject_symlink_chain(path: &std::path::Path) -> Result<(), ReviewWriterError> {
    let mut current = std::path::PathBuf::new();
    for component in path.components() {
        current.push(component);
        match std::fs::symlink_metadata(&current) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(ReviewWriterError::Io(format!(
                    "refusing to write through symlink: {}",
                    current.display()
                )));
            }
            _ => {}
        }
    }
    Ok(())
}
