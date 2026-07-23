//! CLI parsing and dispatch for disk maintenance.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Subcommand};
use cli_composition::MaintenanceCompositionRoot;
use cli_driver::maintenance::{MaintenanceCommandInput, MaintenanceQueryInput};

use crate::commands::driver_outcome_to_exit;

/// Shared project-root argument.
#[derive(Debug, Args)]
pub struct ProjectRootArgs {
    /// Repository root containing `.harness/config/disk-maintenance.toml`.
    #[arg(long, default_value = ".")]
    pub project_root: PathBuf,
}

/// Cleanup arguments.
#[derive(Debug, Args)]
pub struct CleanupArgs {
    #[command(flatten)]
    pub project_root: ProjectRootArgs,
    /// Actually remove contents of the configured cleanup roots.
    #[arg(long)]
    pub apply: bool,
}

/// Disk-maintenance subcommands.
#[derive(Debug, Subcommand)]
pub enum MaintenanceCommand {
    /// Write the configured sccache size into the compose environment file.
    ConfigureSccache(ProjectRootArgs),
    /// Print the cleanup plan, or apply it with `--apply`.
    Cleanup(CleanupArgs),
}

/// Dispatch a disk-maintenance command.
pub fn execute(command: MaintenanceCommand) -> ExitCode {
    let root = MaintenanceCompositionRoot::new();
    match command {
        MaintenanceCommand::ConfigureSccache(args) => {
            driver_outcome_to_exit(root.maintenance_command_driver().handle(
                MaintenanceCommandInput::ConfigureSccache { project_root: args.project_root },
            ))
        }
        MaintenanceCommand::Cleanup(args) if args.apply => driver_outcome_to_exit(
            root.maintenance_command_driver().handle(MaintenanceCommandInput::ApplyCleanup {
                project_root: args.project_root.project_root,
            }),
        ),
        MaintenanceCommand::Cleanup(args) => {
            driver_outcome_to_exit(root.maintenance_query_driver().handle(
                MaintenanceQueryInput::PlanCleanup { project_root: args.project_root.project_root },
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser as _;
    #[derive(clap::Parser)]
    struct TestCli {
        #[command(subcommand)]
        command: super::MaintenanceCommand,
    }
    #[test]
    fn test_cleanup_apply_flag_parses() -> Result<(), clap::Error> {
        let parsed = TestCli::try_parse_from(["test", "cleanup", "--apply"])?;
        assert!(matches!(parsed.command, super::MaintenanceCommand::Cleanup(args) if args.apply));
        Ok(())
    }

    #[test]
    fn test_cleanup_without_apply_parses_as_query_path() -> Result<(), clap::Error> {
        let parsed = TestCli::try_parse_from(["test", "cleanup", "--project-root", "project"])?;
        assert!(matches!(parsed.command, super::MaintenanceCommand::Cleanup(args) if !args.apply));
        Ok(())
    }
}
