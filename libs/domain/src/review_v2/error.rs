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
