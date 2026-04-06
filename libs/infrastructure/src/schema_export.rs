//! Infrastructure adapter for `SchemaExporter` port.
//!
//! Uses `cargo +nightly rustdoc` to generate rustdoc JSON, then parses the output
//! with `rustdoc_types` to build domain `SchemaExport` values.

use std::path::{Path, PathBuf};
use std::process::Command;

use domain::schema::{SchemaExport, SchemaExportError, SchemaExporter};

/// Adapter implementing `SchemaExporter` via rustdoc JSON.
pub struct RustdocSchemaExporter {
    workspace_root: PathBuf,
}

impl RustdocSchemaExporter {
    /// Creates a new exporter for the given workspace root.
    pub fn new(workspace_root: PathBuf) -> Self {
        Self { workspace_root }
    }
}

impl SchemaExporter for RustdocSchemaExporter {
    fn export(&self, crate_name: &str) -> Result<SchemaExport, SchemaExportError> {
        check_nightly_available()?;
        let json_path = run_rustdoc(&self.workspace_root, crate_name)?;
        let krate = parse_rustdoc_json(&json_path)?;
        Ok(build_schema_export(crate_name, &krate))
    }
}

fn check_nightly_available() -> Result<(), SchemaExportError> {
    let output = Command::new("rustup")
        .args(["run", "nightly", "rustc", "--version"])
        .output()
        .map_err(|_| SchemaExportError::NightlyNotFound)?;

    if !output.status.success() {
        return Err(SchemaExportError::NightlyNotFound);
    }
    Ok(())
}

fn run_rustdoc(workspace_root: &Path, crate_name: &str) -> Result<PathBuf, SchemaExportError> {
    let output = Command::new("cargo")
        .args([
            "+nightly",
            "rustdoc",
            "-p",
            crate_name,
            "--",
            "-Z",
            "unstable-options",
            "--output-format",
            "json",
        ])
        .current_dir(workspace_root)
        .output()
        .map_err(|e| SchemaExportError::RustdocFailed(e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("package(s) `") || stderr.contains("could not find") {
            return Err(SchemaExportError::CrateNotFound(crate_name.to_owned()));
        }
        return Err(SchemaExportError::RustdocFailed(stderr.into_owned()));
    }

    let json_path = workspace_root.join("target").join("doc").join(format!("{crate_name}.json"));

    if !json_path.is_file() {
        return Err(SchemaExportError::RustdocFailed(format!(
            "expected rustdoc JSON at {} but file not found",
            json_path.display()
        )));
    }

    Ok(json_path)
}

fn parse_rustdoc_json(path: &Path) -> Result<rustdoc_types::Crate, SchemaExportError> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| SchemaExportError::ParseFailed(format!("read error: {e}")))?;
    serde_json::from_str(&content)
        .map_err(|e| SchemaExportError::ParseFailed(format!("JSON parse error: {e}")))
}

fn build_schema_export(crate_name: &str, _krate: &rustdoc_types::Crate) -> SchemaExport {
    // Step 1: skeleton — returns empty export. Type extraction in next commit.
    SchemaExport::new(crate_name.to_owned(), Vec::new(), Vec::new(), Vec::new(), Vec::new())
}
