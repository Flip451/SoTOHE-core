//! `sotp domain` subcommands.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Subcommand};
use cli_composition::DomainCompositionRoot;
use cli_driver::domain::{DomainInput, ExportSchemaInput as DriverExportSchemaInput};

use crate::commands::driver_outcome_to_exit;

#[derive(Debug, Subcommand)]
pub enum DomainCommand {
    /// Export the public API schema of a crate as JSON (requires nightly toolchain).
    ExportSchema(ExportSchemaArgs),
}

#[derive(Debug, Args)]
pub struct ExportSchemaArgs {
    /// Crate name within the workspace.
    #[arg(long = "crate", value_name = "NAME")]
    pub crate_name: String,

    /// Use indented JSON output.
    #[arg(long, default_value_t = false)]
    pub pretty: bool,

    /// Write output to a file instead of stdout.
    #[arg(long, value_name = "PATH")]
    pub output: Option<PathBuf>,
}

pub fn execute(cmd: DomainCommand) -> ExitCode {
    match cmd {
        DomainCommand::ExportSchema(args) => {
            let driver = match DomainCompositionRoot::new().domain_driver() {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("{e}");
                    return ExitCode::FAILURE;
                }
            };
            driver_outcome_to_exit(driver.handle(DomainInput::ExportSchema(
                DriverExportSchemaInput {
                    crate_name: args.crate_name,
                    pretty: args.pretty,
                    output: args.output,
                },
            )))
        }
    }
}
