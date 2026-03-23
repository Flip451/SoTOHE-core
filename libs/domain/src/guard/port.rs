//! Shell parser port (domain boundary).

use super::types::SimpleCommand;
use super::verdict::ParseError;

/// Port for shell command parsing.
///
/// Implementations split a raw shell command string into structured
/// [`SimpleCommand`] values. The domain layer consumes the parsed output
/// without depending on any particular parser library.
///
/// # Errors
///
/// Returns `ParseError` on nesting depth exceeded or parse failures.
pub trait ShellParser: Send + Sync {
    /// Splits a shell command string into individual simple commands.
    fn split_shell(&self, input: &str) -> Result<Vec<SimpleCommand>, ParseError>;
}
