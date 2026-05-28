//! `domain` command family — CliApp impl methods and input DTOs.

use std::path::PathBuf;

use crate::{CliApp, CommandOutcome};

/// Input DTO for `domain_export_schema`.
#[derive(Debug, Clone)]
pub struct ExportSchemaInput {
    /// Crate name within the workspace.
    pub crate_name: String,
    /// Use indented JSON output.
    pub pretty: bool,
    /// Write output to a file instead of stdout.
    pub output: Option<PathBuf>,
}

impl CliApp {
    /// Export the public API schema of a crate as JSON.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn domain_export_schema(&self, input: ExportSchemaInput) -> Result<CommandOutcome, String> {
        let _ = input;
        Err(String::from("not implemented"))
    }
}
