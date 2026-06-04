//! Port traits and their error types:
//! `DryCheckReaderError`, `DryCheckWriterError`, `DryCheckReader`, `DryCheckWriter`.

use thiserror::Error;

use super::record::{DryCheckEntry, DryCheckRecord};

// ── DryCheckReaderError ───────────────────────────────────────────────────────

/// Errors from [`DryCheckReader`] port operations.
#[derive(Debug, Error)]
pub enum DryCheckReaderError {
    /// File system I/O failure.
    #[error("dry-check reader I/O error: {path}: {detail}")]
    Io {
        /// The file path involved.
        path: String,
        /// Human-readable description of the failure.
        detail: String,
    },
    /// The target path is a symlink (rejected for security).
    #[error("dry-check reader: symlink detected at {path}")]
    SymlinkDetected {
        /// The symlink path.
        path: String,
    },
    /// JSON codec failure.
    #[error("dry-check reader codec error: {path}: {detail}")]
    Codec {
        /// The file path involved.
        path: String,
        /// Human-readable description of the failure.
        detail: String,
    },
    /// A record could not be deserialized to valid domain types.
    #[error("dry-check reader invalid data: {0}")]
    InvalidData(String),
    /// The on-disk schema version is newer than what this implementation supports.
    #[error("dry-check reader incompatible schema version: {version}")]
    IncompatibleSchema {
        /// The unsupported schema version found on disk.
        version: u64,
    },
}

// ── DryCheckWriterError ───────────────────────────────────────────────────────

/// Errors from [`DryCheckWriter`] port operations.
#[derive(Debug, Error)]
pub enum DryCheckWriterError {
    /// File system I/O failure.
    #[error("dry-check writer I/O error: {path}: {detail}")]
    Io {
        /// The file path involved.
        path: String,
        /// Human-readable description of the failure.
        detail: String,
    },
    /// The target path is a symlink (rejected for security).
    #[error("dry-check writer: symlink detected at {path}")]
    SymlinkDetected {
        /// The symlink path.
        path: String,
    },
    /// JSON codec failure.
    #[error("dry-check writer codec error: {detail}")]
    Codec {
        /// Human-readable description of the failure.
        detail: String,
    },
    /// The on-disk schema version is newer than what this implementation supports.
    #[error("dry-check writer incompatible schema version: {version}")]
    IncompatibleSchema {
        /// The unsupported schema version found on disk.
        version: u64,
    },
}

// ── DryCheckReader ────────────────────────────────────────────────────────────

/// Read-only port for dry-check history retrieval.
///
/// Returns the full history array of [`DryCheckRecord`] entries. The caller is
/// responsible for latest-per-pair derivation (last occurrence per pair key
/// wins). Persistence port — defined in domain layer (mirrors `ReviewReader`).
pub trait DryCheckReader: Send + Sync {
    /// Read all recorded dry-check history entries.
    ///
    /// # Errors
    ///
    /// Returns [`DryCheckReaderError`] on I/O, codec, invalid data, or schema
    /// incompatibility failures.
    fn read_records(&self) -> Result<Vec<DryCheckRecord>, DryCheckReaderError>;
}

// ── DryCheckWriter ────────────────────────────────────────────────────────────

/// Write port for dry-check history persistence.
///
/// Receives a [`DryCheckEntry`] (7 fields, no `recorded_at`). The adapter
/// (`FsDryCheckStore`) stamps a `Timestamp` internally to produce a
/// [`DryCheckRecord`] before writing. init-on-first-write: if the file is
/// absent the implementation creates a fresh envelope before appending.
pub trait DryCheckWriter: Send + Sync {
    /// Append a verdict record for the given entry.
    ///
    /// The infra adapter calls `infrastructure::timestamp_now()?` to obtain a
    /// `Timestamp` directly — no `Timestamp::new` re-wrap is needed because
    /// `timestamp_now()` already returns `Result<Timestamp, ValidationError>`.
    /// The interactor never produces a `Timestamp`.
    ///
    /// # Errors
    ///
    /// Returns [`DryCheckWriterError`] on I/O, codec, or schema incompatibility
    /// failures.
    fn append_record(&self, entry: &DryCheckEntry) -> Result<(), DryCheckWriterError>;
}
