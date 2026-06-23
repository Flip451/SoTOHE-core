//! Domain schema export application service (usecase layer).
//!
//! Wraps `domain::schema::SchemaExporter` behind `SchemaExporterPort` so
//! the CLI never imports the domain port trait directly (CN-01 / D1).
//! The CLI injects `RustdocSchemaExporter` (infrastructure) as
//! `Arc<dyn SchemaExporterPort>` into `ExportSchemaInteractor` at the
//! composition root; domain types are hidden behind the usecase boundary.

use std::path::PathBuf;
use std::sync::Arc;

use thiserror::Error;

use crate::file::{FilePortError, FileWritePort};

// ── SchemaExporterPort ────────────────────────────────────────────────────────

/// Secondary port (driven port) for domain schema export.
///
/// Accepts a crate name and returns the serialized JSON schema string, using
/// only primitive types so the CLI never imports `domain::schema::SchemaExporter`
/// or `domain::schema::SchemaExport` directly.
///
/// Infrastructure (`RustdocSchemaExporter`) implements both
/// `domain::schema::SchemaExporter` and this port; its `SchemaExporterPort`
/// impl calls the domain export method internally and serializes the result
/// before returning.
pub trait SchemaExporterPort: Send + Sync {
    /// Exports the schema for the named crate as a serialized JSON string.
    ///
    /// # Errors
    ///
    /// Returns an `Err(String)` describing the export or serialization failure.
    fn export_as_json(&self, crate_name: &str) -> Result<String, String>;
}

// ── ExportSchemaCommand ───────────────────────────────────────────────────────

/// CQRS command object for the schema export use case.
///
/// Carries the crate name to export and an optional output file path.
/// When `output_path` is `Some`, the interactor writes the JSON schema
/// to that file via the injected [`FileWritePort`] instead of returning
/// it as a string. Owned by usecase so the CLI does not import
/// `domain::schema::SchemaExporter`.
pub struct ExportSchemaCommand {
    pub crate_name: String,
    /// When `Some`, write the JSON output to this path via [`FileWritePort`].
    pub output_path: Option<PathBuf>,
}

// ── ExportSchemaError ─────────────────────────────────────────────────────────

/// Error type for [`ExportSchemaService`].
///
/// Wraps failures from the rustdoc schema export, JSON serialization, and
/// optional file write steps without leaking `domain::schema::SchemaExportError`
/// directly across the usecase boundary.
#[derive(Debug, Error)]
pub enum ExportSchemaError {
    #[error("schema export failed: {0}")]
    ExportFailed(String),
    #[error("schema serialization failed: {0}")]
    SerializationFailed(String),
    #[error("schema file write failed: {0}")]
    FileWriteFailed(#[from] FilePortError),
}

// ── ExportSchemaService ───────────────────────────────────────────────────────

/// Application service trait for the domain schema export use case
/// (`sotp domain export-schema`).
///
/// Driven by the CLI layer. Wraps `domain::schema::SchemaExporter` so the CLI
/// never imports the domain port trait directly. Returns the serialized JSON
/// schema as a `String` rather than exposing internal domain types.
pub trait ExportSchemaService: Send + Sync {
    /// Exports the schema for the crate named in `command`.
    ///
    /// # Errors
    ///
    /// Returns [`ExportSchemaError::ExportFailed`] if the underlying export
    /// fails, or [`ExportSchemaError::SerializationFailed`] if serialization
    /// fails.
    fn export(&self, command: ExportSchemaCommand) -> Result<String, ExportSchemaError>;
}

// ── ExportSchemaInteractor ────────────────────────────────────────────────────

/// Concrete struct implementing [`ExportSchemaService`].
///
/// Uses a [`SchemaExporterPort`] secondary port (`Arc<dyn SchemaExporterPort>`)
/// for schema export. Calls [`SchemaExporterPort::export_as_json`], which
/// returns an already-serialized JSON string, so the interactor never imports
/// `domain::schema::SchemaExporter` or `domain::schema::SchemaExport` directly.
///
/// When `command.output_path` is `Some`, writes the JSON to that file via
/// the injected [`FileWritePort`] and returns `Ok("")` (empty string — the
/// caller should check `output_path` to decide what to print). When
/// `output_path` is `None`, returns the full JSON string as before.
///
/// CLI sees only [`ExportSchemaService`] and [`ExportSchemaCommand`];
/// `domain::schema::SchemaExporter` is an implementation detail of the
/// infrastructure adapter.
pub struct ExportSchemaInteractor {
    port: Arc<dyn SchemaExporterPort>,
    file_port: Arc<dyn FileWritePort>,
}

impl ExportSchemaInteractor {
    /// Creates a new interactor bound to the given schema exporter port and file write port.
    #[must_use]
    pub fn new(port: Arc<dyn SchemaExporterPort>, file_port: Arc<dyn FileWritePort>) -> Self {
        Self { port, file_port }
    }
}

impl ExportSchemaService for ExportSchemaInteractor {
    fn export(&self, command: ExportSchemaCommand) -> Result<String, ExportSchemaError> {
        let json = self.port.export_as_json(&command.crate_name).map_err(|e| {
            // The `SchemaExporterPort` contract returns a single `String` error.
            // Infrastructure adapters that perform a two-step operation
            // (export then JSON-serialize) prefix serialization failures with
            // "JSON serialization failed:" per `schema_export_codec::SchemaExportCodecError`.
            // Detect that prefix to route to the correct error variant; all
            // other errors are considered export failures.
            if e.starts_with("JSON serialization failed") {
                ExportSchemaError::SerializationFailed(e)
            } else {
                ExportSchemaError::ExportFailed(e)
            }
        })?;

        if let Some(path) = command.output_path {
            self.file_port.write_atomic(&path, json.as_bytes())?;
            // Return empty string to signal "written to file" — caller renders
            // the success message rather than printing the JSON body.
            Ok(String::new())
        } else {
            Ok(json)
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::path::Path;

    use super::*;

    struct OkPort {
        json: String,
    }

    impl SchemaExporterPort for OkPort {
        fn export_as_json(&self, _crate_name: &str) -> Result<String, String> {
            Ok(self.json.clone())
        }
    }

    struct FailPort {
        message: String,
    }

    impl SchemaExporterPort for FailPort {
        fn export_as_json(&self, _crate_name: &str) -> Result<String, String> {
            Err(self.message.clone())
        }
    }

    /// Stub file write port that always succeeds (no-op).
    struct NoopFileWritePort;

    impl FileWritePort for NoopFileWritePort {
        fn write_atomic(&self, _path: &Path, _content: &[u8]) -> Result<(), FilePortError> {
            Ok(())
        }
    }

    fn noop_file_port() -> Arc<dyn FileWritePort> {
        Arc::new(NoopFileWritePort)
    }

    #[test]
    fn test_export_schema_service_returns_json_string_on_success() {
        let port = Arc::new(OkPort { json: r#"{"types":[]}"#.to_owned() });
        let interactor = ExportSchemaInteractor::new(port, noop_file_port());
        let result = interactor
            .export(ExportSchemaCommand { crate_name: "domain".to_owned(), output_path: None })
            .unwrap();
        assert_eq!(result, r#"{"types":[]}"#);
    }

    #[test]
    fn test_export_schema_service_returns_export_failed_on_port_error() {
        let port = Arc::new(FailPort { message: "nightly not found".to_owned() });
        let interactor = ExportSchemaInteractor::new(port, noop_file_port());
        let err = interactor
            .export(ExportSchemaCommand { crate_name: "domain".to_owned(), output_path: None })
            .unwrap_err();
        assert!(matches!(err, ExportSchemaError::ExportFailed(_)));
    }

    #[test]
    fn test_export_schema_service_returns_serialization_failed_on_json_serialization_error() {
        // Simulates the error string emitted by schema_export_codec::SchemaExportCodecError.
        let port = Arc::new(FailPort {
            message: "JSON serialization failed: some serde error".to_owned(),
        });
        let interactor = ExportSchemaInteractor::new(port, noop_file_port());
        let err = interactor
            .export(ExportSchemaCommand { crate_name: "domain".to_owned(), output_path: None })
            .unwrap_err();
        assert!(matches!(err, ExportSchemaError::SerializationFailed(_)));
    }

    #[test]
    fn test_export_schema_service_writes_to_file_when_output_path_set() {
        use std::sync::Mutex;

        struct CapturingPort {
            written: Arc<Mutex<Option<Vec<u8>>>>,
        }

        impl FileWritePort for CapturingPort {
            fn write_atomic(&self, _path: &Path, content: &[u8]) -> Result<(), FilePortError> {
                *self.written.lock().unwrap() = Some(content.to_vec());
                Ok(())
            }
        }

        let written = Arc::new(Mutex::new(None::<Vec<u8>>));
        let file_port = Arc::new(CapturingPort { written: Arc::clone(&written) });
        let schema_port = Arc::new(OkPort { json: r#"{"types":[]}"#.to_owned() });
        let interactor = ExportSchemaInteractor::new(schema_port, file_port);

        let result = interactor
            .export(ExportSchemaCommand {
                crate_name: "domain".to_owned(),
                output_path: Some(PathBuf::from("/tmp/schema.json")),
            })
            .unwrap();

        // Returns empty string when writing to file.
        assert_eq!(result, "");
        // Content was written.
        let captured = written.lock().unwrap();
        assert_eq!(captured.as_deref(), Some(r#"{"types":[]}"#.as_bytes()));
    }
}
