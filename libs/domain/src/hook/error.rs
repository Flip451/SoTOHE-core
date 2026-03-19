//! Hook error types.

/// Errors that can occur during hook dispatch.
#[derive(Debug, thiserror::Error)]
pub enum HookError {
    /// Invalid or missing required hook input field.
    #[error("invalid hook input: {0}")]
    Input(String),

    /// Error from the guard command parser.
    #[error(transparent)]
    Guard(#[from] crate::guard::ParseError),

    /// The hook name is not supported by any handler.
    #[error("unsupported hook: {0:?}")]
    Unsupported(super::types::HookName),
}
