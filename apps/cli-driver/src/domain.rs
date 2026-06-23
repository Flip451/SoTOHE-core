//! `domain` command family — primary adapter driver.
//!
//! `DomainDriver` holds an injected [`usecase::export_schema::ExportSchemaService`]
//! and exposes `handle(input) -> CommandOutcome`.

use std::path::PathBuf;
use std::sync::Arc;

use usecase::export_schema::{ExportSchemaCommand, ExportSchemaService};

use crate::render::CommandOutcome;

// ---------------------------------------------------------------------------
// Input type
// ---------------------------------------------------------------------------

/// Input DTO for the `domain export-schema` command.
#[derive(Debug, Clone)]
pub struct ExportSchemaInput {
    /// Crate name within the workspace.
    pub crate_name: String,
    /// Use indented JSON output.
    pub pretty: bool,
    /// Write output to a file instead of stdout.
    pub output: Option<PathBuf>,
}

/// Typed input for the `domain` command family.
pub enum DomainInput {
    /// Export the public API schema of a crate as JSON.
    ExportSchema(ExportSchemaInput),
}

// ---------------------------------------------------------------------------
// Driver
// ---------------------------------------------------------------------------

/// Primary adapter driver for the `domain` command family.
///
/// Holds an injected [`ExportSchemaService`]; exposes `handle(input) -> CommandOutcome`.
pub struct DomainDriver {
    export_schema_service: Arc<dyn ExportSchemaService>,
}

impl DomainDriver {
    /// Create a new `DomainDriver` with the given export schema service.
    pub fn new(export_schema_service: Arc<dyn ExportSchemaService>) -> Self {
        Self { export_schema_service }
    }

    /// Handle a domain command.
    pub fn handle(&self, input: DomainInput) -> CommandOutcome {
        match input {
            DomainInput::ExportSchema(export_input) => self.domain_export_schema(export_input),
        }
    }

    fn domain_export_schema(&self, input: ExportSchemaInput) -> CommandOutcome {
        let output_path = input.output.clone();

        let raw_json = match self
            .export_schema_service
            .export(ExportSchemaCommand { crate_name: input.crate_name, output_path: input.output })
        {
            Ok(json) => json,
            Err(e) => return CommandOutcome::failure(Some(e.to_string())),
        };

        // When output_path was set the service wrote the file and returned "".
        if let Some(path) = output_path {
            return CommandOutcome {
                stdout: None,
                stderr: Some(format!("[OK] Schema written to {}", path.display())),
                exit_code: 0,
            };
        }

        // No output path: compact if not pretty, then print to stdout.
        let json = if input.pretty {
            raw_json
        } else {
            match serde_json::from_str::<serde_json::Value>(&raw_json)
                .and_then(|v| serde_json::to_string(&v))
            {
                Ok(compact) => compact,
                Err(e) => {
                    return CommandOutcome::failure(Some(format!(
                        "failed to compact schema JSON: {e}"
                    )));
                }
            }
        };

        CommandOutcome::success(Some(json))
    }
}
