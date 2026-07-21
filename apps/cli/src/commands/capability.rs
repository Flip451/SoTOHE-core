//! `sotp capability` command family.

use std::process::ExitCode;

use clap::{Args, Subcommand};
use cli_composition::CapabilityCompositionRoot;
use cli_driver::capability::{
    CapabilityExecDriverInput, CapabilityFilePathArg, CapabilityNameArg, CapabilityResumeArg,
    ProviderNameArg, TargetArtifactPathArg, TimeoutSecondsArg,
};

use crate::commands::driver_outcome_to_exit;

/// Generic capability dispatch subcommands.
#[derive(Debug, Subcommand)]
pub enum CapabilityCommand {
    /// Resolve a capability profile and run its provider-native dispatch path.
    Exec(CapabilityExecArgs),
}

/// Arguments for `sotp capability exec`.
#[derive(Debug, Args)]
pub struct CapabilityExecArgs {
    /// Capability name resolved from `.harness/config/agent-profiles.json`.
    pub capability: CapabilityNameArg,
    /// Actual provider of the host invoking this command.
    #[arg(long)]
    pub host: ProviderNameArg,
    /// Path to a non-empty UTF-8 briefing file.
    #[arg(long)]
    pub briefing_file: CapabilityFilePathArg,
    /// Provider-process timeout in seconds. When omitted, the provider process
    /// runs without a time limit.
    #[arg(long)]
    pub timeout_seconds: Option<TimeoutSecondsArg>,
    /// Resume the matching prior provider session.  With no target artifacts,
    /// track resolution is required; otherwise dispatch falls back fresh.
    #[arg(long)]
    pub resume: bool,
    /// Repository-relative artifact identity for a workspace resume.
    #[arg(long = "target-artifact", requires = "resume")]
    pub target_artifacts: Vec<TargetArtifactPathArg>,
}

/// Executes a generic capability command.
pub fn execute(command: CapabilityCommand) -> ExitCode {
    execute_with(command, execute_exec)
}

fn execute_with(
    command: CapabilityCommand,
    execute_exec: impl FnOnce(CapabilityExecArgs) -> ExitCode,
) -> ExitCode {
    match command {
        CapabilityCommand::Exec(args) => execute_exec(args),
    }
}

fn execute_exec(args: CapabilityExecArgs) -> ExitCode {
    let root = match CapabilityCompositionRoot::discover() {
        Ok(root) => root,
        Err(error) => {
            eprintln!("failed to initialize capability command: {error}");
            return ExitCode::FAILURE;
        }
    };
    let driver = root.capability_driver();
    driver_outcome_to_exit(driver.handle(into_driver_input(args)))
}

fn into_driver_input(args: CapabilityExecArgs) -> CapabilityExecDriverInput {
    CapabilityExecDriverInput {
        capability: args.capability,
        host: args.host,
        briefing_file: args.briefing_file,
        timeout_seconds: args.timeout_seconds,
        resume: if !args.resume {
            CapabilityResumeArg::Fresh
        } else if args.target_artifacts.is_empty() {
            CapabilityResumeArg::ResumeWithoutTarget
        } else {
            CapabilityResumeArg::Resume(args.target_artifacts)
        },
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use clap::Parser;

    use super::{
        CapabilityCommand, CapabilityExecArgs, CapabilityFilePathArg, CapabilityNameArg,
        CapabilityResumeArg, ProviderNameArg, TargetArtifactPathArg, TimeoutSecondsArg, execute,
        execute_with, into_driver_input,
    };

    #[derive(Parser)]
    struct TestCli {
        #[command(subcommand)]
        command: CapabilityCommand,
    }

    #[test]
    fn test_capability_exec_parses_required_generic_dispatch_inputs() {
        let cli = TestCli::try_parse_from([
            "sotp",
            "exec",
            "implementer",
            "--host",
            "codex",
            "--briefing-file",
            "tmp/briefing.md",
        ])
        .expect("valid capability command parses");

        match cli.command {
            CapabilityCommand::Exec(CapabilityExecArgs {
                capability,
                host,
                briefing_file,
                timeout_seconds,
                resume: _,
                target_artifacts: _,
            }) => {
                assert_eq!(
                    capability,
                    "implementer".parse::<CapabilityNameArg>().expect("valid test capability")
                );
                assert_eq!(host, "codex".parse::<ProviderNameArg>().expect("valid test provider"));
                assert_eq!(
                    briefing_file,
                    "tmp/briefing.md"
                        .parse::<CapabilityFilePathArg>()
                        .expect("valid test briefing path")
                );
                assert_eq!(timeout_seconds, None, "omitted timeout parses as no limit");
            }
        }
    }

    #[test]
    fn test_capability_exec_parses_targeted_resume_input() {
        let cli = TestCli::try_parse_from([
            "sotp",
            "exec",
            "implementer",
            "--host",
            "codex",
            "--briefing-file",
            "tmp/briefing.md",
            "--resume",
            "--target-artifact",
            "track/items/a/spec.json",
        ])
        .expect("valid resume command parses");

        match cli.command {
            CapabilityCommand::Exec(args) => {
                assert!(args.resume);
                assert_eq!(args.target_artifacts.len(), 1);
            }
        }
    }

    #[test]
    fn test_capability_exec_maps_fresh_targetless_and_targeted_resume_to_distinct_driver_states() {
        let fresh = TestCli::try_parse_from([
            "sotp",
            "exec",
            "implementer",
            "--host",
            "codex",
            "--briefing-file",
            "tmp/briefing.md",
        ])
        .expect("fresh command parses");
        let targetless = TestCli::try_parse_from([
            "sotp",
            "exec",
            "implementer",
            "--host",
            "codex",
            "--briefing-file",
            "tmp/briefing.md",
            "--resume",
        ])
        .expect("targetless resume command parses");
        let targeted = TestCli::try_parse_from([
            "sotp",
            "exec",
            "implementer",
            "--host",
            "codex",
            "--briefing-file",
            "tmp/briefing.md",
            "--resume",
            "--target-artifact",
            "track/items/a/./spec.json",
        ])
        .expect("targeted resume command parses");

        let CapabilityCommand::Exec(fresh) = fresh.command;
        let CapabilityCommand::Exec(targetless) = targetless.command;
        let CapabilityCommand::Exec(targeted) = targeted.command;
        assert_eq!(into_driver_input(fresh).resume, CapabilityResumeArg::Fresh);
        assert!(targetless.target_artifacts.is_empty());
        assert_eq!(into_driver_input(targetless).resume, CapabilityResumeArg::ResumeWithoutTarget);
        assert_eq!(
            into_driver_input(targeted).resume,
            CapabilityResumeArg::Resume(vec![
                "track/items/a/spec.json"
                    .parse::<TargetArtifactPathArg>()
                    .expect("normalized target")
            ])
        );
    }

    #[test]
    fn test_capability_exec_timeout_seconds_parses_and_rejects_zero() {
        let cli = TestCli::try_parse_from([
            "sotp",
            "exec",
            "implementer",
            "--host",
            "codex",
            "--briefing-file",
            "tmp/briefing.md",
            "--timeout-seconds",
            "1800",
        ])
        .expect("valid timeout parses");
        match cli.command {
            CapabilityCommand::Exec(args) => {
                assert_eq!(
                    args.timeout_seconds,
                    Some("1800".parse::<TimeoutSecondsArg>().expect("valid test timeout"))
                );
            }
        }

        assert!(
            TestCli::try_parse_from([
                "sotp",
                "exec",
                "implementer",
                "--host",
                "codex",
                "--briefing-file",
                "tmp/briefing.md",
                "--timeout-seconds",
                "0",
            ])
            .is_err()
        );
    }

    #[test]
    fn test_capability_exec_requires_host_and_briefing_file() {
        assert!(TestCli::try_parse_from(["sotp", "exec", "implementer"]).is_err());
        assert!(
            TestCli::try_parse_from([
                "sotp",
                "exec",
                " ",
                "--host",
                "codex",
                "--briefing-file",
                "tmp/briefing.md",
            ])
            .is_err()
        );
    }

    #[test]
    fn test_capability_exec_missing_briefing_file_is_rejected() {
        assert!(
            TestCli::try_parse_from(["sotp", "exec", "implementer", "--host", "codex",]).is_err()
        );
    }

    #[test]
    fn test_capability_exec_invalid_values_are_rejected_at_parse_boundary() {
        assert!(
            TestCli::try_parse_from([
                "sotp",
                "exec",
                "implementer",
                "--host",
                " ",
                "--briefing-file",
                "tmp/briefing.md",
            ])
            .is_err()
        );
        assert!(
            TestCli::try_parse_from([
                "sotp",
                "exec",
                "implementer",
                "--host",
                "codex",
                "--briefing-file",
                "../briefing.md",
            ])
            .is_err()
        );
    }

    #[test]
    fn test_capability_execute_routes_exec_to_concrete_execution_helper() {
        let command = CapabilityCommand::Exec(CapabilityExecArgs {
            capability: "implementer".parse().expect("valid test capability"),
            host: "codex".parse().expect("valid test provider"),
            briefing_file: "tmp/briefing.md".parse().expect("valid test briefing path"),
            timeout_seconds: None,
            resume: false,
            target_artifacts: Vec::new(),
        });
        let mut forwarded = None;

        let exit = execute_with(command, |args| {
            forwarded = Some(args);
            std::process::ExitCode::SUCCESS
        });

        assert_eq!(exit, std::process::ExitCode::SUCCESS);
        let args = forwarded.expect("Exec variant is forwarded to its execution helper");
        assert_eq!(
            args.capability,
            "implementer".parse::<CapabilityNameArg>().expect("valid test capability")
        );
        assert_eq!(args.host, "codex".parse::<ProviderNameArg>().expect("valid test provider"));
        assert_eq!(
            args.briefing_file,
            "tmp/briefing.md".parse::<CapabilityFilePathArg>().expect("valid test briefing path")
        );
    }

    #[test]
    fn test_capability_execute_missing_briefing_returns_failure_without_provider_dispatch() {
        let cli = TestCli::try_parse_from([
            "sotp",
            "exec",
            "implementer",
            "--host",
            "codex",
            "--briefing-file",
            "tmp/obligation-test-missing-briefing.md",
        ])
        .expect("valid capability command parses");

        assert_eq!(execute(cli.command), std::process::ExitCode::FAILURE);
    }
}
