//! `sotp domain` subcommands.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Subcommand};
use cli_composition::{CliApp, ExportSchemaInput};

use crate::commands::outcome_to_exit;

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
            outcome_to_exit(CliApp::new().domain_export_schema(ExportSchemaInput {
                crate_name: args.crate_name,
                pretty: args.pretty,
                output: args.output,
            }))
        }
    }
}
