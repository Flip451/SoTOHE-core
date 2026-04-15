//! `sotp domain` subcommands.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Subcommand};

use domain::schema::SchemaExporter;
use infrastructure::schema_export::RustdocSchemaExporter;
use infrastructure::schema_export_codec;

use crate::CliError;

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
        DomainCommand::ExportSchema(args) => match export_schema(&args) {
            Ok(code) => code,
            Err(err) => {
                eprintln!("{err}");
                err.exit_code()
            }
        },
    }
}

fn export_schema(args: &ExportSchemaArgs) -> Result<ExitCode, CliError> {
    let workspace_root = discover_workspace_root()?;
    let exporter = RustdocSchemaExporter::new(workspace_root);
    let schema = exporter.export(&args.crate_name).map_err(|e| CliError::Message(e.to_string()))?;

    let json = schema_export_codec::encode(&schema, args.pretty)
        .map_err(|e| CliError::Message(format!("JSON serialization failed: {e}")))?;

    if let Some(path) = &args.output {
        std::fs::write(path, &json)
            .map_err(|e| CliError::Message(format!("failed to write {}: {e}", path.display())))?;
        eprintln!("[OK] Schema written to {}", path.display());
    } else {
        println!("{json}");
    }

    Ok(ExitCode::SUCCESS)
}

fn discover_workspace_root() -> Result<PathBuf, CliError> {
    let output = std::process::Command::new("cargo")
        .args(["locate-project", "--workspace", "--message-format", "plain"])
        .output()
        .map_err(|e| CliError::Message(format!("cargo locate-project failed: {e}")))?;

    if !output.status.success() {
        return Err(CliError::Message("failed to locate workspace root via cargo".to_owned()));
    }

    let manifest = String::from_utf8_lossy(&output.stdout);
    let manifest_path = PathBuf::from(manifest.trim());
    manifest_path
        .parent()
        .map(|p| p.to_owned())
        .ok_or_else(|| CliError::Message("workspace manifest has no parent directory".to_owned()))
}
