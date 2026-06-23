//! `domain` command family — `DomainCompositionRoot` impl methods and input DTOs.

use std::path::PathBuf;
use std::sync::Arc;

use crate::{CommandOutcome, error::CompositionError};

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

// ---------------------------------------------------------------------------
// Per-context composition root
// ---------------------------------------------------------------------------

/// Composition root for the `domain` command family.
///
/// This family has no injectable adapter dependencies; the schema exporter
/// is constructed inline from the discovered workspace root.
pub struct DomainCompositionRoot;

impl DomainCompositionRoot {
    /// Create a new `DomainCompositionRoot`.
    pub fn new() -> Self {
        Self
    }
}

impl Default for DomainCompositionRoot {
    fn default() -> Self {
        Self::new()
    }
}

impl DomainCompositionRoot {
    /// Build a wired [`cli_driver::domain::DomainDriver`] for the domain family.
    ///
    /// # Errors
    /// Returns `Err` when workspace root discovery fails.
    pub fn domain_driver(
        &self,
    ) -> Result<cli_driver::domain::DomainDriver, crate::error::CompositionError> {
        use infrastructure::file_port::FsFileWriteAdapter;
        use infrastructure::schema_export::RustdocSchemaExporter;
        use usecase::export_schema::ExportSchemaInteractor;

        let workspace_root =
            discover_workspace_root().map_err(crate::error::CompositionError::AdapterInit)?;
        let exporter = Arc::new(RustdocSchemaExporter::new(workspace_root));
        let file_port = Arc::new(FsFileWriteAdapter::new());
        let service = Arc::new(ExportSchemaInteractor::new(exporter, file_port));
        Ok(cli_driver::domain::DomainDriver::new(service))
    }

    /// Export the public API schema of a crate as JSON.
    ///
    /// Delegates to `domain_driver()` so that the file-write side effect is
    /// handled by the usecase layer via [`FileWritePort`].
    ///
    /// # Errors
    /// Returns `Err` when workspace root discovery or schema export fails.
    pub fn domain_export_schema(
        &self,
        input: ExportSchemaInput,
    ) -> Result<CommandOutcome, CompositionError> {
        use cli_driver::domain::{DomainInput, ExportSchemaInput as DriverInput};

        let driver = self.domain_driver()?;
        let out = driver.handle(DomainInput::ExportSchema(DriverInput {
            crate_name: input.crate_name,
            pretty: input.pretty,
            output: input.output,
        }));
        // Convert cli_driver::CommandOutcome → cli_composition::CommandOutcome.
        Ok(CommandOutcome { stdout: out.stdout, stderr: out.stderr, exit_code: out.exit_code })
    }
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

fn discover_workspace_root() -> Result<PathBuf, String> {
    let output = std::process::Command::new("cargo")
        .args(["locate-project", "--workspace", "--message-format", "plain"])
        .output()
        .map_err(|e| format!("cargo locate-project failed: {e}"))?;

    if !output.status.success() {
        return Err("failed to locate workspace root via cargo".to_owned());
    }

    let manifest = String::from_utf8_lossy(&output.stdout);
    let manifest_path = PathBuf::from(manifest.trim());
    manifest_path
        .parent()
        .map(|p| p.to_owned())
        .ok_or_else(|| "workspace manifest has no parent directory".to_owned())
}
