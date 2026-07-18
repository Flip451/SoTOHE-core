//! `sotp codex-runtime` subcommands.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Subcommand};
use cli_composition::CodexRuntimeCompositionRoot;
#[cfg(test)]
use cli_driver::codex_runtime::CodexRuntimeDriver;
use cli_driver::codex_runtime::CodexRuntimeInput;

use super::driver_outcome_to_exit;

/// Explicit Codex runtime maintenance operations.
#[derive(Subcommand)]
pub enum CodexRuntimeCommand {
    /// Resolve, verify, and provision the repository-local Codex runtime link.
    Provision(CodexRuntimeProvisionArgs),
}

/// Arguments for one-shot Codex runtime provisioning.
#[derive(Args)]
pub struct CodexRuntimeProvisionArgs {
    /// Repository root that receives the runtime link.
    #[arg(long, default_value = ".")]
    pub project_root: PathBuf,
}

/// Execute a Codex runtime command with its already-wired primary adapter.
///
/// Composition-root wiring is intentionally owned by the delivery composition layer.
#[cfg(test)]
pub fn execute_with_driver(cmd: CodexRuntimeCommand, driver: &CodexRuntimeDriver) -> ExitCode {
    match cmd {
        CodexRuntimeCommand::Provision(args) => driver_outcome_to_exit(
            driver.handle(CodexRuntimeInput { project_root: args.project_root }),
        ),
    }
}

/// Execute a Codex runtime command through the delivery composition root.
pub fn execute(cmd: CodexRuntimeCommand) -> ExitCode {
    match cmd {
        CodexRuntimeCommand::Provision(args) => driver_outcome_to_exit(
            CodexRuntimeCompositionRoot::new()
                .codex_runtime_driver()
                .handle(CodexRuntimeInput { project_root: args.project_root }),
        ),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::process::ExitCode;
    use std::sync::Arc;

    use clap::Parser;
    use cli_driver::codex_runtime::CodexRuntimeDriver;
    use usecase::{
        DiagnosticMessage,
        codex_runtime::{CodexRuntimeProvisionError, CodexRuntimeProvisionService},
    };

    use super::{CodexRuntimeCommand, execute_with_driver};

    #[derive(Parser)]
    struct TestCli {
        #[command(subcommand)]
        command: CodexRuntimeCommand,
    }

    struct SucceedingService;

    impl CodexRuntimeProvisionService for SucceedingService {
        fn provision(&self, _project_root: &Path) -> Result<(), CodexRuntimeProvisionError> {
            Ok(())
        }
    }

    struct FailingService;

    impl CodexRuntimeProvisionService for FailingService {
        fn provision(&self, _project_root: &Path) -> Result<(), CodexRuntimeProvisionError> {
            let detail = DiagnosticMessage::try_new("install Codex and rerun bootstrap".to_owned())
                .expect("test diagnostic must be valid");
            Err(CodexRuntimeProvisionError::NoUsableCandidate(detail))
        }
    }

    #[test]
    fn test_provision_parses_default_project_root() {
        let parsed =
            TestCli::try_parse_from(["test", "provision"]).expect("provision command must parse");

        match parsed.command {
            CodexRuntimeCommand::Provision(args) => {
                assert_eq!(args.project_root, PathBuf::from("."))
            }
        }
    }

    #[test]
    fn test_provision_parses_explicit_project_root() {
        let parsed = TestCli::try_parse_from(["test", "provision", "--project-root", "/repo"])
            .expect("provision command must parse");

        match parsed.command {
            CodexRuntimeCommand::Provision(args) => {
                assert_eq!(args.project_root, PathBuf::from("/repo"));
            }
        }
    }

    #[test]
    fn test_provision_dispatch_returns_success_for_provisioned_runtime() {
        let command = TestCli::try_parse_from(["test", "provision"])
            .expect("provision command must parse")
            .command;
        let driver = CodexRuntimeDriver::new(Arc::new(SucceedingService));

        let exit_code = execute_with_driver(command, &driver);

        assert_eq!(exit_code, ExitCode::SUCCESS);
    }

    #[test]
    fn test_provision_dispatch_returns_failure_for_unusable_candidates() {
        let command = TestCli::try_parse_from(["test", "provision"])
            .expect("provision command must parse")
            .command;
        let driver = CodexRuntimeDriver::new(Arc::new(FailingService));

        let exit_code = execute_with_driver(command, &driver);

        assert_eq!(exit_code, ExitCode::FAILURE);
    }
}
