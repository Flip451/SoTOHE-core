//! Hook dispatch subcommand for security-critical hooks.
//!
//! Reads Claude Code hook JSON from stdin, dispatches to the appropriate
//! handler via `cli_composition::CliApp`, and exits with the correct code:
//! - Exit 0 = allow
//! - Exit 2 = block (Claude Code hook protocol)
//!
//! PreToolUse hooks: any internal error → exit 2 (fail-closed).

use std::process::ExitCode;

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
    fn hook_name(self) -> &'static str {
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

/// Executes a hook subcommand.
pub fn execute(cmd: HookCommand) -> ExitCode {
    match cmd {
        HookCommand::Dispatch { hook } => {
            let hook_name = hook.hook_name().to_owned();
            match CliApp::new().hook_dispatch(hook_name) {
                Ok(outcome) => {
                    if let Some(stdout) = outcome.stdout {
                        println!("{stdout}");
                    }
                    if let Some(stderr) = outcome.stderr {
                        eprintln!("{stderr}");
                    }
                    ExitCode::from(outcome.exit_code)
                }
                Err(msg) => {
                    eprintln!("{msg}");
                    // Fail-closed: hook error → block (exit 2)
                    ExitCode::from(2u8)
                }
            }
        }
    }
}
