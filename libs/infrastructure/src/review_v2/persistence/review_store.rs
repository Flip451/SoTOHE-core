//! `FsReviewStore` — filesystem adapter for review.json v2.

use std::collections::HashMap;
use std::path::PathBuf;

use domain::review_v2::{
    FastVerdict, Finding, MainScopeName, ReviewHash, ReviewReader, ReviewReaderError, ReviewWriter,
    ReviewWriterError, ScopeName, Verdict,
};

use super::{
    FindingEntry, PersistenceError, ReviewJsonV2, ScopeEntry, WriteGuard, reject_symlinks_below,
    write_atomic,
};

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
        // Reject symlinks on the read path to prevent review state injection
        // (e.g., a symlinked review.json pointing to an external zero_findings file).
        reject_symlinks_below(&self.path, &self.trusted_root).map_err(|source| {
            if source.kind() == std::io::ErrorKind::InvalidInput {
                PersistenceError::Symlink { path: self.path.clone() }
            } else {
                PersistenceError::Io { operation: "symlink check", path: self.path.clone(), source }
            }
        })?;

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

        // Empty file (e.g., interrupted write, accidental truncation) → treat as empty
        // rather than failing with a codec error. This preserves the fail-closed invariant
        // (all scopes need review) without blocking status/approval queries.
        if content.trim().is_empty() {
            return Ok(ReviewJsonV2::empty());
        }

        let envelope: serde_json::Value =
            serde_json::from_str(&content).map_err(|e| PersistenceError::Codec {
                operation: "parse",
                path: Some(self.path.clone()),
                detail: e.to_string(),
            })?;

        // Distinguish explicit schema_version from missing/malformed.
        // Missing or non-numeric schema_version in an existing file is malformed,
        // not legacy — silently treating it as empty could drop scope history on writes.
        let version_field = envelope.get("schema_version");
        let version = match version_field.and_then(serde_json::Value::as_u64) {
            Some(v) => v,
            None if version_field.is_none() || content.trim().is_empty() => {
                // File exists but is empty or has no schema_version field at all.
                // On write path this is malformed; on read path treat as empty (fail-closed).
                if reject_future {
                    return Err(PersistenceError::Codec {
                        operation: "validate schema_version",
                        path: Some(self.path.clone()),
                        detail: "missing or non-numeric schema_version".to_owned(),
                    });
                }
                return Ok(ReviewJsonV2::empty());
            }
            None => {
                // schema_version present but not a valid u64 (e.g., string, float, null)
                return Err(PersistenceError::Codec {
                    operation: "validate schema_version",
                    path: Some(self.path.clone()),
                    detail: format!("schema_version is not a valid integer: {:?}", version_field),
                });
            }
        };

        if version == 2 {
            let doc: ReviewJsonV2 =
                serde_json::from_value(envelope).map_err(|e| PersistenceError::Codec {
                    operation: "parse v2",
                    path: Some(self.path.clone()),
                    detail: e.to_string(),
                })?;
            return Ok(doc);
        }

        // v1 (known legacy, explicit schema_version: 1 or 0) → treat as empty
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
        entry.rounds.push(super::RoundEntry {
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
            let pid = std::process::id();
            let archive_name = format!("review-{now}-{pid}.json");
            if let Some(parent) = self.path.parent() {
                let archive_path = parent.join(archive_name);
                std::fs::rename(&self.path, &archive_path).map_err(|e| ReviewWriterError::Io {
                    path: self.path.display().to_string(),
                    detail: format!("archive → {}: {e}", archive_path.display()),
                })?;
            }
        }

        // Create fresh review.json (does NOT clear .commit_hash per ADR)
        write_atomic(&self.path, &ReviewJsonV2::empty())
            .map_err(PersistenceError::into_writer_error)
    }
}

// ── Helpers ───────────────────────────────────────────────────────────

fn parse_scope_name(key: &str) -> Result<ScopeName, ReviewReaderError> {
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
        "zero_findings" if findings.is_empty() => Ok(Verdict::ZeroFindings),
        "zero_findings" => Err(ReviewReaderError::InvalidData(format!(
            "zero_findings verdict has {} findings attached (expected 0)",
            findings.len()
        ))),
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
