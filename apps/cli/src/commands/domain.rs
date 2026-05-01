//! `sotp domain` subcommands.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use clap::{Args, Subcommand};

use infrastructure::schema_export::RustdocSchemaExporter;
use usecase::export_schema::{ExportSchemaCommand, ExportSchemaInteractor, ExportSchemaService};

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

    // Composition root: wire RustdocSchemaExporter (infrastructure) as
    // Arc<dyn SchemaExporterPort> into ExportSchemaInteractor.
    // CLI never imports domain::schema::SchemaExporter (CN-01 satisfied).
    let exporter = Arc::new(RustdocSchemaExporter::new(workspace_root));
    let service = ExportSchemaInteractor::new(exporter);

    let raw_json = service
        .export(ExportSchemaCommand { crate_name: args.crate_name.clone() })
        .map_err(|e| CliError::Message(e.to_string()))?;

    // The infrastructure adapter always serializes with pretty formatting.
    // When --pretty=false (the default), compact the output by re-parsing
    // and re-serializing without indentation so the flag has its advertised
    // effect.
    let json = if args.pretty {
        raw_json
    } else {
        let value: serde_json::Value = serde_json::from_str(&raw_json)
            .map_err(|e| CliError::Message(format!("failed to parse schema JSON: {e}")))?;
        serde_json::to_string(&value)
            .map_err(|e| CliError::Message(format!("failed to compact schema JSON: {e}")))?
    };

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
