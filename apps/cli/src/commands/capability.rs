//! `sotp capability` command family.

use std::process::ExitCode;

use clap::{Args, Subcommand};
use cli_composition::CapabilityCompositionRoot;
use cli_driver::capability::{
    CapabilityExecDriverInput, CapabilityFilePathArg, CapabilityNameArg, ProviderNameArg,
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
    driver_outcome_to_exit(driver.handle(CapabilityExecDriverInput {
        capability: args.capability,
        host: args.host,
        briefing_file: args.briefing_file,
    }))
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use clap::Parser;

    use super::{
        CapabilityCommand, CapabilityExecArgs, CapabilityFilePathArg, CapabilityNameArg,
        ProviderNameArg, execute, execute_with,
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
            CapabilityCommand::Exec(CapabilityExecArgs { capability, host, briefing_file }) => {
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
            }
        }
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
