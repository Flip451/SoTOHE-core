//! CLI error type that unifies errors from all layers.

use std::process::ExitCode;

use thiserror::Error;

/// Unified error type for CLI commands.
///
/// All composition logic (infrastructure/usecase errors) is handled inside
/// `cli_composition::CliApp` and converted to `String` before reaching this
/// layer. Only `Message` and `Io` are needed in production; the other variants
/// are preserved (via `#[from]`) in test builds only for test-helper ergonomics.
///
/// The `Display` impl (via thiserror) produces user-facing messages suitable
/// for `eprintln!`.
#[derive(Debug, Error)]
pub enum CliError {
    /// Generic message for errors that don't fit a specific variant.
    #[error("{0}")]
    Message(String),

    /// I/O errors from the CLI layer itself.
    #[error("{0}")]
    Io(#[from] std::io::Error),
}

impl CliError {
    /// Converts this error into an `ExitCode`.
    ///
    /// All errors map to `ExitCode::FAILURE` (exit code 1).
    #[must_use]
    pub fn exit_code(&self) -> ExitCode {
        ExitCode::FAILURE
    }
}
