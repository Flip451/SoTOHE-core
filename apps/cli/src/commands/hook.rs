//! Hook dispatch subcommand for security-critical hooks.
//!
//! Dispatches to the appropriate handler via `cli_composition::CliApp`
//! and exits with the correct code:
//! - Exit 0 = allow
//! - Exit 2 = block (Claude Code hook protocol)
//!
//! PreToolUse hooks: any internal error → exit 2 (fail-closed).

use cli_composition::HookCompositionRoot;
use cli_driver::CommandOutcome;
use cli_driver::hook::{HookExecution as DriverHookExecution, HookHost, HookInput, HookName};

/// CLI-owned finite hook-host boundary value.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum CliHookHost {
    /// Claude Code's snake-case hook envelope.
    Claude,
    /// Codex's snake-case hook envelope.
    Codex,
    /// Grok's camel-case hook envelope.
    Grok,
}

impl From<CliHookHost> for HookHost {
    fn from(host: CliHookHost) -> Self {
        match host {
            CliHookHost::Claude => Self::Claude,
            CliHookHost::Codex => Self::Codex,
            CliHookHost::Grok => Self::Grok,
        }
    }
}

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

    fn driver_name(self) -> HookName {
        match self {
            Self::HooksPathSetup => HookName::HooksPathSetup,
            Self::BlockDirectGitOps => HookName::BlockDirectGitOps,
            Self::BlockTestFileDeletion => HookName::BlockTestFileDeletion,
            Self::GitRefUpdate => HookName::GitRefUpdate,
            Self::GitPrePush => HookName::GitPrePush,
            Self::SkillCompliance => HookName::SkillCompliance,
        }
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
        /// The agent host that owns the hook input contract.
        #[arg(long, value_enum)]
        host: Option<CliHookHost>,
        /// Positional arguments supplied by git process hooks.
        #[arg(num_args = 0..)]
        git_hook_args: Vec<String>,
    },
}

/// CLI telemetry classification for hook execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookExecutionDisposition {
    InputError,
    HookBlock,
    AdvisoryFired,
    Allow,
}

/// CLI emission sum type for hook execution.
pub enum CliHookExecution {
    InputError(CommandOutcome),
    HookBlock(CommandOutcome),
    AdvisoryFired(CommandOutcome),
    Allow(CommandOutcome),
}

impl CliHookExecution {
    /// Returns the rendered command outcome without changing its disposition.
    pub fn outcome(&self) -> &CommandOutcome {
        match self {
            Self::InputError(outcome)
            | Self::HookBlock(outcome)
            | Self::AdvisoryFired(outcome)
            | Self::Allow(outcome) => outcome,
        }
    }

    /// Returns the typed classification used by telemetry.
    pub fn disposition(&self) -> HookExecutionDisposition {
        match self {
            Self::InputError(_) => HookExecutionDisposition::InputError,
            Self::HookBlock(_) => HookExecutionDisposition::HookBlock,
            Self::AdvisoryFired(_) => HookExecutionDisposition::AdvisoryFired,
            Self::Allow(_) => HookExecutionDisposition::Allow,
        }
    }
}

impl From<DriverHookExecution> for CliHookExecution {
    fn from(execution: DriverHookExecution) -> Self {
        match execution {
            DriverHookExecution::InputError(outcome) => Self::InputError(outcome),
            DriverHookExecution::HookBlock(outcome) => Self::HookBlock(outcome),
            DriverHookExecution::AdvisoryFired(outcome) => Self::AdvisoryFired(outcome),
            DriverHookExecution::Allow(outcome) => Self::Allow(outcome),
        }
    }
}

impl From<HookCommand> for HookInput {
    fn from(command: HookCommand) -> Self {
        match command {
            HookCommand::Dispatch { hook, host, git_hook_args } => {
                Self { hook: hook.driver_name(), host: host.map(Into::into), git_hook_args }
            }
        }
    }
}

/// Executes a hook subcommand and retains the driver's typed result without
/// printing or converting to `ExitCode`.
///
/// Used by the telemetry wrapper in `main.rs` to observe the verdict before
/// printing (T005 / AC-04).
///
/// # Errors
/// Returns `Err` when the underlying composition logic fails.
pub fn execute_inner(cmd: HookCommand) -> Result<CliHookExecution, crate::CliError> {
    let execution = HookCompositionRoot::new().hook_driver().handle(cmd.into());
    Ok(execution.into())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use clap::Parser;

    use cli_driver::hook::{HookHost, HookInput};

    use super::{
        CliHookExecution, CliHookHost, CliHookName, HookCommand, HookExecutionDisposition,
    };

    #[derive(Debug, Parser)]
    struct TestCli {
        #[command(subcommand)]
        cmd: HookCommand,
    }

    #[test]
    fn test_dispatch_hooks_path_setup_parses() {
        let cli =
            TestCli::try_parse_from(["hook", "dispatch", "--host", "claude", "hooks-path-setup"])
                .unwrap();

        match cli.cmd {
            HookCommand::Dispatch { hook, host, git_hook_args } => {
                assert!(matches!(hook, CliHookName::HooksPathSetup));
                assert!(matches!(host, Some(CliHookHost::Claude)));
                assert!(git_hook_args.is_empty());
            }
        }
    }

    #[test]
    fn test_dispatch_git_ref_update_with_prepared_arg_parses() {
        let cli =
            TestCli::try_parse_from(["hook", "dispatch", "git-ref-update", "prepared"]).unwrap();

        match cli.cmd {
            HookCommand::Dispatch { hook, host, git_hook_args } => {
                assert!(matches!(hook, CliHookName::GitRefUpdate));
                assert!(host.is_none());
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
            HookCommand::Dispatch { hook, host, git_hook_args } => {
                assert!(matches!(hook, CliHookName::GitPrePush));
                assert!(host.is_none());
                assert_eq!(
                    git_hook_args,
                    vec!["origin".to_owned(), "https://example.com".to_owned()]
                );
            }
        }
    }

    #[test]
    fn test_dispatch_git_host_after_argument_remains_a_clap_option() {
        let cli = TestCli::try_parse_from([
            "hook",
            "dispatch",
            "git-ref-update",
            "committed",
            "--host",
            "claude",
        ])
        .unwrap();

        match cli.cmd {
            HookCommand::Dispatch { hook, host, git_hook_args } => {
                assert!(matches!(hook, CliHookName::GitRefUpdate));
                assert!(matches!(host, Some(CliHookHost::Claude)));
                assert_eq!(git_hook_args, vec!["committed".to_owned()]);
            }
        }
    }

    #[test]
    fn test_dispatch_git_arguments_after_delimiter_are_opaque() {
        let cli = TestCli::try_parse_from([
            "hook",
            "dispatch",
            "git-ref-update",
            "committed",
            "--",
            "--host",
            "claude",
        ])
        .unwrap();

        match cli.cmd {
            HookCommand::Dispatch { hook, host, git_hook_args } => {
                assert!(matches!(hook, CliHookName::GitRefUpdate));
                assert!(host.is_none());
                assert_eq!(
                    git_hook_args,
                    vec!["committed".to_owned(), "--host".to_owned(), "claude".to_owned()]
                );
            }
        }
    }

    #[test]
    fn test_dispatch_agent_host_parses_as_optional_clap_field() {
        let cli =
            TestCli::try_parse_from(["hook", "dispatch", "--host", "grok", "skill-compliance"])
                .unwrap();

        match cli.cmd {
            HookCommand::Dispatch { hook, host, git_hook_args } => {
                assert!(matches!(hook, CliHookName::SkillCompliance));
                assert!(matches!(host, Some(CliHookHost::Grok)));
                assert!(git_hook_args.is_empty());
            }
        }
    }

    #[test]
    fn test_dispatch_agent_host_parses_all_cli_values() {
        for (raw_host, expected_host) in [
            ("claude", CliHookHost::Claude),
            ("codex", CliHookHost::Codex),
            ("grok", CliHookHost::Grok),
        ] {
            let cli = TestCli::try_parse_from([
                "hook",
                "dispatch",
                "--host",
                raw_host,
                "skill-compliance",
            ])
            .unwrap();

            match cli.cmd {
                HookCommand::Dispatch { hook, host, git_hook_args } => {
                    assert!(matches!(hook, CliHookName::SkillCompliance));
                    assert!(matches!(
                        (expected_host, host),
                        (CliHookHost::Claude, Some(CliHookHost::Claude))
                            | (CliHookHost::Codex, Some(CliHookHost::Codex))
                            | (CliHookHost::Grok, Some(CliHookHost::Grok))
                    ));
                    assert!(git_hook_args.is_empty());
                }
            }
        }
    }

    #[test]
    fn test_dispatch_invalid_host_value_is_rejected_by_clap() {
        for hook in ["block-direct-git-ops", "skill-compliance"] {
            let result = TestCli::try_parse_from(["hook", "dispatch", "--host", "unknown", hook]);

            let error = result.expect_err("invalid host must be rejected by clap");
            assert_eq!(error.exit_code(), 2);
        }
    }

    #[test]
    fn test_cli_hook_host_converts_exhaustively_to_driver_host() {
        assert!(matches!(HookHost::from(CliHookHost::Claude), HookHost::Claude));
        assert!(matches!(HookHost::from(CliHookHost::Codex), HookHost::Codex));
        assert!(matches!(HookHost::from(CliHookHost::Grok), HookHost::Grok));
    }

    #[test]
    fn test_hook_command_converts_to_driver_input_without_cli_policy() {
        let input = HookInput::from(HookCommand::Dispatch {
            hook: CliHookName::GitPrePush,
            host: None,
            git_hook_args: vec!["origin".to_owned(), "https://example.com".to_owned()],
        });

        assert!(matches!(input.hook, cli_driver::hook::HookName::GitPrePush));
        assert!(input.host.is_none());
        assert_eq!(input.git_hook_args, ["origin", "https://example.com"]);
    }

    #[test]
    fn test_execute_agent_hook_without_host_returns_exit_2() {
        for hook in [CliHookName::BlockDirectGitOps, CliHookName::SkillCompliance] {
            let execution = super::execute_inner(HookCommand::Dispatch {
                hook,
                host: None,
                git_hook_args: vec![],
            })
            .unwrap();

            assert!(matches!(&execution, CliHookExecution::InputError(_)));
            assert_eq!(execution.outcome().exit_code, 2);
            assert_eq!(execution.disposition(), HookExecutionDisposition::InputError);
            assert_eq!(
                execution.outcome().stderr.as_deref(),
                Some("error: agent hooks require --host claude|codex|grok")
            );
        }
    }

    #[test]
    fn test_execute_git_hook_with_host_returns_exit_2() {
        let execution = super::execute_inner(HookCommand::Dispatch {
            hook: CliHookName::GitRefUpdate,
            host: Some(CliHookHost::Claude),
            git_hook_args: vec!["committed".to_owned()],
        })
        .unwrap();

        assert!(matches!(&execution, CliHookExecution::InputError(_)));
        assert_eq!(execution.outcome().exit_code, 2);
        assert_eq!(execution.disposition(), HookExecutionDisposition::InputError);
        assert_eq!(
            execution.outcome().stderr.as_deref(),
            Some("error: git process hooks must not specify --host")
        );
    }

    #[test]
    fn test_execute_block_direct_git_ops_with_extra_args_returns_exit_2() {
        let execution = super::execute_inner(HookCommand::Dispatch {
            hook: CliHookName::BlockDirectGitOps,
            host: Some(CliHookHost::Claude),
            git_hook_args: vec!["extra".to_owned()],
        })
        .unwrap();

        assert!(matches!(&execution, CliHookExecution::InputError(_)));
        assert_eq!(
            execution.outcome().stderr.as_deref(),
            Some("error: extra hook arguments are only supported for git process hooks")
        );
    }

    #[test]
    fn test_execute_block_test_file_deletion_with_extra_args_returns_exit_2() {
        let execution = super::execute_inner(HookCommand::Dispatch {
            hook: CliHookName::BlockTestFileDeletion,
            host: Some(CliHookHost::Claude),
            git_hook_args: vec!["extra".to_owned()],
        })
        .unwrap();

        assert!(matches!(&execution, CliHookExecution::InputError(_)));
        assert_eq!(
            execution.outcome().stderr.as_deref(),
            Some("error: extra hook arguments are only supported for git process hooks")
        );
    }
}
