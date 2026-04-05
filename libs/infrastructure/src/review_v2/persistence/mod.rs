//! Review v2 persistence adapters: review.json and .commit_hash file I/O.
//!
//! Split into sub-modules:
//! - `review_store`: `FsReviewStore` (ReviewReader + ReviewWriter)
//! - `commit_hash_store`: `FsCommitHashStore` (CommitHashReader + CommitHashWriter)

mod commit_hash_store;
mod review_store;
#[cfg(test)]
mod tests;

pub use commit_hash_store::FsCommitHashStore;
pub use review_store::FsReviewStore;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use domain::review_v2::{ReviewReaderError, ReviewWriterError};
use fs4::fs_std::FileExt;

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
    Symlink { path: PathBuf },
    Codec { operation: &'static str, path: Option<PathBuf>, detail: String },
    FutureSchema { version: u64 },
}

impl PersistenceError {
    fn into_writer_error(self) -> ReviewWriterError {
        match self {
            Self::Io { operation, path, source } => ReviewWriterError::Io {
                path: path.display().to_string(),
                detail: format!("{operation}: {source}"),
            },
            Self::Symlink { path } => {
                ReviewWriterError::SymlinkDetected { path: path.display().to_string() }
            }
            Self::Codec { operation, detail, .. } => {
                ReviewWriterError::Codec { detail: format!("{operation}: {detail}") }
            }
            Self::FutureSchema { version } => ReviewWriterError::IncompatibleSchema { version },
        }
    }

    fn into_reader_error(self) -> ReviewReaderError {
        match self {
            Self::Io { operation, path, source } => ReviewReaderError::Io {
                path: path.display().to_string(),
                detail: format!("{operation}: {source}"),
            },
            Self::Symlink { path } => {
                ReviewReaderError::SymlinkDetected { path: path.display().to_string() }
            }
            Self::Codec { operation, path, detail } => ReviewReaderError::Codec {
                path: path.map_or_else(String::new, |p| p.display().to_string()),
                detail: format!("{operation}: {detail}"),
            },
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
        reject_symlinks_below(path, trusted_root).map_err(|source| {
            if source.kind() == std::io::ErrorKind::InvalidInput {
                PersistenceError::Symlink { path: path.to_owned() }
            } else {
                PersistenceError::Io { operation: "symlink check", path: path.to_owned(), source }
            }
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
    let json = serde_json::to_string_pretty(doc).map_err(|e| PersistenceError::Codec {
        operation: "serialize",
        path: None,
        detail: e.to_string(),
    })?;
    atomic_write_file(path, json.as_bytes()).map_err(|source| PersistenceError::Io {
        operation: "atomic write",
        path: path.to_owned(),
        source,
    })
}
