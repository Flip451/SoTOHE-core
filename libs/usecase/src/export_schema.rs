//! Domain schema export application service (usecase layer).
//!
//! Wraps `domain::schema::SchemaExporter` behind `SchemaExporterPort` so
//! the CLI never imports the domain port trait directly (CN-01 / D1).
//! The CLI injects `RustdocSchemaExporter` (infrastructure) as
//! `Arc<dyn SchemaExporterPort>` into `ExportSchemaInteractor` at the
//! composition root; domain types are hidden behind the usecase boundary.

use std::sync::Arc;

use thiserror::Error;

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
/// Carries the crate name to export. Owned by usecase so the CLI does not
/// import `domain::schema::SchemaExporter`.
pub struct ExportSchemaCommand {
    pub crate_name: String,
}

// ── ExportSchemaError ─────────────────────────────────────────────────────────

/// Error type for [`ExportSchemaService`].
///
/// Wraps failures from the rustdoc schema export and JSON serialization steps
/// without leaking `domain::schema::SchemaExportError` directly across the
/// usecase boundary.
#[derive(Debug, Error)]
pub enum ExportSchemaError {
    #[error("schema export failed: {0}")]
    ExportFailed(String),
    #[error("schema serialization failed: {0}")]
    SerializationFailed(String),
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
/// CLI sees only [`ExportSchemaService`] and [`ExportSchemaCommand`];
/// `domain::schema::SchemaExporter` is an implementation detail of the
/// infrastructure adapter.
pub struct ExportSchemaInteractor {
    port: Arc<dyn SchemaExporterPort>,
}

impl ExportSchemaInteractor {
    /// Creates a new interactor bound to the given schema exporter port.
    #[must_use]
    pub fn new(port: Arc<dyn SchemaExporterPort>) -> Self {
        Self { port }
    }
}

impl ExportSchemaService for ExportSchemaInteractor {
    fn export(&self, command: ExportSchemaCommand) -> Result<String, ExportSchemaError> {
        self.port.export_as_json(&command.crate_name).map_err(|e| {
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
        })
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
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

    #[test]
    fn test_export_schema_service_returns_json_string_on_success() {
        let port = Arc::new(OkPort { json: r#"{"types":[]}"#.to_owned() });
        let interactor = ExportSchemaInteractor::new(port);
        let result =
            interactor.export(ExportSchemaCommand { crate_name: "domain".to_owned() }).unwrap();
        assert_eq!(result, r#"{"types":[]}"#);
    }

    #[test]
    fn test_export_schema_service_returns_export_failed_on_port_error() {
        let port = Arc::new(FailPort { message: "nightly not found".to_owned() });
        let interactor = ExportSchemaInteractor::new(port);
        let err =
            interactor.export(ExportSchemaCommand { crate_name: "domain".to_owned() }).unwrap_err();
        assert!(matches!(err, ExportSchemaError::ExportFailed(_)));
    }

    #[test]
    fn test_export_schema_service_returns_serialization_failed_on_json_serialization_error() {
        // Simulates the error string emitted by schema_export_codec::SchemaExportCodecError.
        let port = Arc::new(FailPort {
            message: "JSON serialization failed: some serde error".to_owned(),
        });
        let interactor = ExportSchemaInteractor::new(port);
        let err =
            interactor.export(ExportSchemaCommand { crate_name: "domain".to_owned() }).unwrap_err();
        assert!(matches!(err, ExportSchemaError::SerializationFailed(_)));
    }
}
