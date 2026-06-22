//! Hook dispatch subcommand for security-critical hooks.
//!
//! Dispatches to the appropriate handler via `cli_composition::CliApp`
//! and exits with the correct code:
//! - Exit 0 = allow
//! - Exit 2 = block (Claude Code hook protocol)
//!
//! PreToolUse hooks: any internal error → exit 2 (fail-closed).

use cli_composition::HookCompositionRoot;

/// Hook names as CLI value enum (clap layer only — DIP).
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum CliHookName {
    /// Preflight: require local git hooks setup before Bash execution.
    HooksPathSetup,
    /// Guard: block direct git operations.
    BlockDirectGitOps,
    /// Guard: block `rm` commands targeting test files (PreToolUse).
    BlockTestFileDeletion,
    /// Process-level git hook: reference transaction.
    GitRefUpdate,
    /// Process-level git hook: pre-push.
    GitPrePush,
    /// Advisory: skill compliance check for UserPromptSubmit.
    SkillCompliance,
}

impl CliHookName {
    /// Returns the hook name string used by `CliApp::hook_dispatch`.
    pub fn hook_name(self) -> &'static str {
        match self {
            Self::HooksPathSetup => "hooks-path-setup",
            Self::BlockDirectGitOps => "block-direct-git-ops",
            Self::BlockTestFileDeletion => "block-test-file-deletion",
            Self::GitRefUpdate => "git-ref-update",
            Self::GitPrePush => "git-pre-push",
            Self::SkillCompliance => "skill-compliance",
        }
    }

    /// Returns whether this hook is invoked by git with positional hook arguments.
    fn accepts_git_hook_args(self) -> bool {
        matches!(self, Self::GitRefUpdate | Self::GitPrePush)
    }
}

/// Hook subcommands.
#[derive(Debug, clap::Subcommand)]
pub enum HookCommand {
    /// Dispatch a security-critical hook via Rust logic.
    /// Claude Code hooks read hook JSON from stdin.
    /// Git process hooks may receive positional hook arguments.
    /// Exit 0 = allow, exit 2 = block (Claude Code hook protocol).
    /// PreToolUse hooks: any internal error → exit 2 (fail-closed).
    Dispatch {
        /// The hook to dispatch.
        #[arg(value_enum)]
        hook: CliHookName,
        /// Positional arguments supplied by git process hooks.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        git_hook_args: Vec<String>,
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
        HookCommand::Dispatch { hook, git_hook_args } => {
            if !git_hook_args.is_empty() && !hook.accepts_git_hook_args() {
                return Ok(cli_composition::CommandOutcome {
                    stdout: None,
                    stderr: Some(
                        "extra hook arguments are only supported for git process hooks".to_owned(),
                    ),
                    exit_code: 2,
                });
            }

            let hook_name = hook.hook_name().to_owned();
            HookCompositionRoot::new()
                .hook_dispatch(hook_name, git_hook_args)
                .map_err(|e| e.to_string())
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use clap::Parser;

    use super::{CliHookName, HookCommand};

    #[derive(Parser)]
    struct TestCli {
        #[command(subcommand)]
        cmd: HookCommand,
    }

    #[test]
    fn test_dispatch_hooks_path_setup_parses() {
        let cli = TestCli::try_parse_from(["hook", "dispatch", "hooks-path-setup"]).unwrap();

        match cli.cmd {
            HookCommand::Dispatch { hook, git_hook_args } => {
                assert!(matches!(hook, CliHookName::HooksPathSetup));
                assert!(git_hook_args.is_empty());
            }
        }
    }

    #[test]
    fn test_dispatch_git_ref_update_with_prepared_arg_parses() {
        let cli =
            TestCli::try_parse_from(["hook", "dispatch", "git-ref-update", "prepared"]).unwrap();

        match cli.cmd {
            HookCommand::Dispatch { hook, git_hook_args } => {
                assert!(matches!(hook, CliHookName::GitRefUpdate));
                assert_eq!(git_hook_args, vec!["prepared".to_owned()]);
            }
        }
    }

    #[test]
    fn test_dispatch_git_pre_push_with_remote_args_parses() {
        let cli = TestCli::try_parse_from([
            "hook",
            "dispatch",
            "git-pre-push",
            "origin",
            "https://example.com",
        ])
        .unwrap();

        match cli.cmd {
            HookCommand::Dispatch { hook, git_hook_args } => {
                assert!(matches!(hook, CliHookName::GitPrePush));
                assert_eq!(
                    git_hook_args,
                    vec!["origin".to_owned(), "https://example.com".to_owned()]
                );
            }
        }
    }

    #[test]
    fn test_execute_block_direct_git_ops_with_extra_args_returns_exit_2() {
        let outcome = super::execute_inner(HookCommand::Dispatch {
            hook: CliHookName::BlockDirectGitOps,
            git_hook_args: vec!["extra".to_owned()],
        })
        .unwrap();

        assert_eq!(outcome.exit_code, 2);
        assert_eq!(
            outcome.stderr.as_deref(),
            Some("extra hook arguments are only supported for git process hooks")
        );
    }

    #[test]
    fn test_execute_block_test_file_deletion_with_extra_args_returns_exit_2() {
        let outcome = super::execute_inner(HookCommand::Dispatch {
            hook: CliHookName::BlockTestFileDeletion,
            git_hook_args: vec!["extra".to_owned()],
        })
        .unwrap();

        assert_eq!(outcome.exit_code, 2);
        assert_eq!(
            outcome.stderr.as_deref(),
            Some("extra hook arguments are only supported for git process hooks")
        );
    }
}
