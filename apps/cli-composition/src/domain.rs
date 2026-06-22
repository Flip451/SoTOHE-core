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
    /// Export the public API schema of a crate as JSON.
    ///
    /// # Errors
    /// Returns `Err` when workspace root discovery or schema export fails.
    pub fn domain_export_schema(
        &self,
        input: ExportSchemaInput,
    ) -> Result<CommandOutcome, CompositionError> {
        use infrastructure::schema_export::RustdocSchemaExporter;
        use usecase::export_schema::{
            ExportSchemaCommand, ExportSchemaInteractor, ExportSchemaService,
        };

        let workspace_root = discover_workspace_root().map_err(CompositionError::AdapterInit)?;

        let exporter = Arc::new(RustdocSchemaExporter::new(workspace_root));
        let service = ExportSchemaInteractor::new(exporter);

        let raw_json = service
            .export(ExportSchemaCommand { crate_name: input.crate_name })
            .map_err(|e| CompositionError::Usecase(e.to_string()))?;

        let json = if input.pretty {
            raw_json
        } else {
            let value: serde_json::Value = serde_json::from_str(&raw_json).map_err(|e| {
                CompositionError::Infrastructure(format!("failed to parse schema JSON: {e}"))
            })?;
            serde_json::to_string(&value).map_err(|e| {
                CompositionError::Infrastructure(format!("failed to compact schema JSON: {e}"))
            })?
        };

        if let Some(path) = &input.output {
            std::fs::write(path, &json).map_err(|e| {
                CompositionError::Infrastructure(format!("failed to write {}: {e}", path.display()))
            })?;
            Ok(CommandOutcome {
                stdout: None,
                stderr: Some(format!("[OK] Schema written to {}", path.display())),
                exit_code: 0,
            })
        } else {
            Ok(CommandOutcome::success(Some(json)))
        }
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
