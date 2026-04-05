use thiserror::Error;

/// Errors from `MainScopeName` construction.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ScopeNameError {
    #[error("scope name must not be empty")]
    Empty,
    #[error("scope name must be ASCII")]
    NotAscii,
    #[error("scope name 'other' is reserved; use ScopeName::Other")]
    Reserved,
}

/// Errors from `Verdict::findings_remain` / `FastVerdict::findings_remain`.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum VerdictError {
    #[error("FindingsRemain requires at least one finding")]
    EmptyFindings,
}

/// Errors from `Finding::new` construction.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum FindingError {
    #[error("finding message must not be empty or whitespace-only")]
    EmptyMessage,
}

/// Errors from `FilePath::new` construction.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum FilePathError {
    #[error("file path must not be empty")]
    Empty,
    #[error("file path must be repo-relative, not absolute: {0}")]
    Absolute(String),
    #[error("file path must not contain '..' traversal: {0}")]
    Traversal(String),
}

/// Errors from `ReviewHashValue::new` construction.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ReviewHashError {
    #[error("review hash must start with 'rvw1:sha256:' and contain hex digits: {0}")]
    InvalidFormat(String),
}

/// Errors from `ReviewReader` port operations.
#[derive(Debug, Error)]
pub enum ReviewReaderError {
    /// File system I/O failure.
    #[error("review reader I/O error: {path}: {detail}")]
    Io { path: String, detail: String },
    /// Symlink detected on the review file path (security rejection).
    #[error("review reader symlink detected: {path}")]
    SymlinkDetected { path: String },
    /// JSON parsing failure (file is not valid JSON).
    #[error("review reader codec error: {path}: {detail}")]
    Codec { path: String, detail: String },
    /// Valid JSON but contains domain-invalid data (bad scope, hash, verdict, finding).
    #[error("review reader invalid data: {0}")]
    InvalidData(String),
}

/// Errors from `ReviewWriter` port operations.
#[derive(Debug, Error)]
pub enum ReviewWriterError {
    /// File system I/O failure.
    #[error("review writer I/O error: {path}: {detail}")]
    Io { path: String, detail: String },
    /// Symlink detected on the review file path (security rejection).
    #[error("review writer symlink detected: {path}")]
    SymlinkDetected { path: String },
    /// JSON serialization failure.
    #[error("review writer codec error: {detail}")]
    Codec { detail: String },
    /// Refusing to overwrite a `review.json` with a newer schema version.
    #[error(
        "review.json has schema_version {version} (unknown future version); \
         refusing to overwrite. Run init() to reset."
    )]
    IncompatibleSchema { version: u64 },
}

/// Errors from `CommitHashReader` / `CommitHashWriter` port operations.
#[derive(Debug, Error)]
pub enum CommitHashError {
    /// File system I/O failure.
    #[error("commit hash I/O error: {path}: {detail}")]
    Io { path: String, detail: String },
    /// Symlink detected on the commit hash file path (security rejection).
    #[error("commit hash symlink detected: {path}")]
    SymlinkDetected { path: String },
    /// Invalid commit hash format.
    #[error("commit hash format error: {0}")]
    Format(String),
}

/// Errors from `ReviewScopeConfig::new` construction.
#[derive(Debug, Error)]
pub enum ScopeConfigError {
    #[error("invalid scope name: {0}")]
    InvalidScopeName(#[from] ScopeNameError),
    #[error("invalid glob pattern '{pattern}': {source}")]
    InvalidPattern { pattern: String, source: globset::Error },
    #[error("invalid operational glob pattern '{pattern}': {source}")]
    InvalidOperationalPattern { pattern: String, source: globset::Error },
    #[error("invalid other_track glob pattern '{pattern}': {source}")]
    InvalidOtherTrackPattern { pattern: String, source: globset::Error },
}
