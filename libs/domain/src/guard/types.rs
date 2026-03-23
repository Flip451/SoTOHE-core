//! Shell command types used by guard policy and parsing.

/// A parsed simple command (argv list + redirect texts + output redirect flag).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimpleCommand {
    /// The argument vector of the command.
    pub argv: Vec<String>,
    /// Flattened text from redirect targets (including heredoc bodies).
    /// Used by policy to detect git references hidden in heredocs.
    pub redirect_texts: Vec<String>,
    /// Whether this command has any output redirect (Write/Append/Clobber).
    /// Does NOT include DupWrite (`>&fd`) or Read (`<`).
    pub has_output_redirect: bool,
}
