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
}

/// Errors from `ReviewHashValue::new` construction.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ReviewHashError {
    #[error("review hash must start with 'rvw1:sha256:' and contain hex digits: {0}")]
    InvalidFormat(String),
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
