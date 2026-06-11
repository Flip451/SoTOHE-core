//! Hook dispatch subcommand for security-critical hooks.
//!
//! Reads Claude Code hook JSON from stdin, dispatches to the appropriate
//! handler via `cli_composition::CliApp`, and exits with the correct code:
//! - Exit 0 = allow
//! - Exit 2 = block (Claude Code hook protocol)
//!
//! PreToolUse hooks: any internal error → exit 2 (fail-closed).

use cli_composition::CliApp;

/// Hook names as CLI value enum (clap layer only — DIP).
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum CliHookName {
    /// Guard: block direct git operations.
    BlockDirectGitOps,
    /// Guard: block `rm` commands targeting test files (PreToolUse).
    BlockTestFileDeletion,
    /// Advisory: skill compliance check for UserPromptSubmit.
    SkillCompliance,
}

impl CliHookName {
    /// Returns the hook name string used by `CliApp::hook_dispatch`.
    pub fn hook_name(self) -> &'static str {
        match self {
            Self::BlockDirectGitOps => "block-direct-git-ops",
            Self::BlockTestFileDeletion => "block-test-file-deletion",
            Self::SkillCompliance => "skill-compliance",
        }
    }
}

/// Hook subcommands.
#[derive(Debug, clap::Subcommand)]
pub enum HookCommand {
    /// Dispatch a security-critical hook via Rust logic.
    /// Reads Claude Code hook JSON from stdin.
    /// Exit 0 = allow, exit 2 = block (Claude Code hook protocol).
    /// PreToolUse hooks: any internal error → exit 2 (fail-closed).
    Dispatch {
        /// The hook to dispatch.
        #[arg(value_enum)]
        hook: CliHookName,
    },
}

/// Executes a hook subcommand and returns the raw `CommandOutcome` without
/// printing or converting to `ExitCode`.
///
/// Used by the telemetry wrapper in `main.rs` to observe the verdict before
/// printing (T005 / AC-04).
///
/// # Errors
/// Returns `Err(msg)` when the underlying composition logic fails.
pub fn execute_inner(cmd: HookCommand) -> Result<cli_composition::CommandOutcome, String> {
    match cmd {
        HookCommand::Dispatch { hook } => {
            let hook_name = hook.hook_name().to_owned();
            CliApp::new().hook_dispatch(hook_name)
        }
    }
}
