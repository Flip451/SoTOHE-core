//! Use-case layer for the `sotp template export` command (spec IN-01, AC-01).
//!
//! This module owns the behavioural contract for exporting the workspace as a
//! reusable template. The [`TemplateExportService`] primary port drives two
//! secondary ports:
//!
//! - [`TemplateBoundaryManifestPort`] loads the boundary manifest SSoT that
//!   classifies every workspace path (spec IN-02, CN-02).
//! - [`TemplateExportPort`] performs the filesystem export driven purely by
//!   those classifications; it never parses or rewrites file contents
//!   (spec IN-03, CN-03).
//!
//! Ports live in the use-case layer (hexagonal architecture): the domain
//! carries no concept of filesystem traversal, so both secondary ports abstract
//! infrastructure concerns behind traits the interactor depends on.
//! See ADR `2026-07-06-1717-template-extraction-boundary.md` D1/D4.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use domain::tddd::catalogue_linter::FreeText;
use domain::template_export::{
    TemplateBoundaryManifest, TemplateBoundaryManifestError, TemplatePathPattern,
    TemplatePathPatternError,
};
use thiserror::Error;

// ---------------------------------------------------------------------------
// Command / report DTOs
// ---------------------------------------------------------------------------

/// `sotp template export` command (IN-01, AC-01).
///
/// Carries the four filesystem locations the export operates over: the
/// workspace to read from, the boundary manifest to classify paths, the overlay
/// directory holding template replacements, and the output directory the
/// template is written to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateExportCommand {
    /// Root of the workspace being exported as a template.
    pub workspace_root: PathBuf,
    /// Path to the boundary manifest that classifies every workspace path.
    pub manifest_path: PathBuf,
    /// Directory holding overlay (template-replacement) files.
    pub overlay_dir: PathBuf,
    /// Directory the exported template is written to.
    pub output_dir: PathBuf,
}

/// `sotp template export` report (IN-01, AC-01).
///
/// Summarises the outcome of a successful export: how many paths were included,
/// excluded, and replaced by an overlay, plus the directory the template was
/// written to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateExportReport {
    /// Number of paths copied into the template unchanged.
    pub included_count: usize,
    /// Number of paths dropped from the template.
    pub excluded_count: usize,
    /// Number of paths replaced by their overlay version.
    pub overlay_applied_count: usize,
    /// Directory the exported template was written to.
    pub output_dir: PathBuf,
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// `TemplateExportService::export` error (IN-01, IN-02, IN-12, AC-01, AC-02, CN-03).
///
/// Distinguishes a manifest-read failure from an export failure so callers can
/// report the failing stage precisely.
#[derive(Debug, Error)]
pub enum TemplateExportError {
    /// The boundary manifest could not be read or validated.
    #[error("failed to read template boundary manifest: {source}")]
    ManifestRead {
        /// Underlying manifest-read failure.
        #[from]
        source: TemplateBoundaryManifestReadError,
    },
    /// The filesystem export failed.
    #[error("failed to export template: {source}")]
    Export {
        /// Underlying export-port failure.
        #[from]
        source: TemplateExportPortError,
    },
}

/// `TemplateBoundaryManifestPort::read` error (IN-02, AC-02).
///
/// Enumerates every way loading the boundary manifest can fail, from a missing
/// file to an invalid pattern or a manifest-invariant violation.
#[derive(Debug, Error)]
pub enum TemplateBoundaryManifestReadError {
    /// The manifest file does not exist at the given path.
    #[error("template boundary manifest not found: {path}")]
    NotFound {
        /// Path the manifest was expected at.
        path: PathBuf,
    },
    /// The manifest file could not be parsed.
    #[error("failed to parse template boundary manifest at {path}: {reason}")]
    Parse {
        /// Path of the manifest that failed to parse.
        path: PathBuf,
        /// Human-readable diagnostic describing the parse failure.
        reason: FreeText,
    },
    /// A pattern in the manifest is not workspace-relative.
    #[error("invalid template path pattern in manifest: {source}")]
    InvalidPattern {
        /// Underlying pattern-validation failure.
        #[from]
        source: TemplatePathPatternError,
    },
    /// The manifest violates a construction invariant (empty or duplicate).
    #[error("invalid template boundary manifest: {source}")]
    InvalidManifest {
        /// Underlying manifest-invariant failure.
        #[from]
        source: TemplateBoundaryManifestError,
    },
    /// An I/O error occurred while reading the manifest.
    #[error("i/o error reading template boundary manifest at {path}: {reason}")]
    Io {
        /// Path involved in the I/O failure.
        path: PathBuf,
        /// Human-readable diagnostic describing the I/O failure.
        reason: FreeText,
    },
}

/// `TemplateExportPort::export` error (IN-01, IN-03, AC-01, AC-03, CN-03).
///
/// Enumerates every fail-closed condition the filesystem export can hit: a
/// pre-existing output directory, a missing overlay or source path, an
/// unclassified path, or an I/O failure.
#[derive(Debug, Error)]
pub enum TemplateExportPortError {
    /// The output directory already exists (export refuses to overwrite).
    #[error("output directory already exists: {path}")]
    OutputDirExists {
        /// The pre-existing output directory.
        path: PathBuf,
    },
    /// An overlay-classified pattern has no corresponding overlay file.
    #[error("overlay file missing for pattern {pattern:?}: {overlay_path}")]
    OverlayMissing {
        /// The overlay-classified pattern.
        pattern: TemplatePathPattern,
        /// The overlay path that was expected to exist.
        overlay_path: PathBuf,
    },
    /// A manifest-referenced source path is missing from the workspace.
    #[error("source path missing from workspace: {path}")]
    SourceMissing {
        /// The missing source path.
        path: PathBuf,
    },
    /// A workspace path is not classified by the manifest (fail-closed, IN-12).
    #[error("unclassified path (not covered by manifest): {path:?}")]
    UnclassifiedPath {
        /// The pattern that has no manifest classification.
        path: TemplatePathPattern,
    },
    /// An I/O error occurred during export.
    #[error("i/o error during template export at {path}: {reason}")]
    Io {
        /// Path involved in the I/O failure.
        path: PathBuf,
        /// Human-readable diagnostic describing the I/O failure.
        reason: FreeText,
    },
}

// ---------------------------------------------------------------------------
// Ports
// ---------------------------------------------------------------------------

/// `sotp template export` primary port (IN-01, AC-01).
///
/// Entry point for the export use case; implemented by
/// [`TemplateExportInteractor`].
pub trait TemplateExportService: Send + Sync {
    /// Primary port entrypoint (IN-01, AC-01).
    ///
    /// Reads the boundary manifest and performs the filesystem export.
    ///
    /// # Errors
    ///
    /// Returns [`TemplateExportError::ManifestRead`] if the boundary manifest
    /// cannot be read or validated.
    /// Returns [`TemplateExportError::Export`] if the filesystem export fails.
    fn export(
        &self,
        command: TemplateExportCommand,
    ) -> Result<TemplateExportReport, TemplateExportError>;
}

/// Boundary manifest secondary port (IN-02, AC-02, CN-02).
///
/// Abstracts loading and validating the boundary manifest SSoT from disk.
pub trait TemplateBoundaryManifestPort: Send + Sync {
    /// Manifest-read operation (IN-02, AC-02, CN-02).
    ///
    /// # Errors
    ///
    /// Returns a [`TemplateBoundaryManifestReadError`] variant when the manifest
    /// is missing, unparseable, contains an invalid pattern, violates a
    /// manifest invariant, or an I/O error occurs.
    fn read(
        &self,
        manifest_path: &Path,
    ) -> Result<TemplateBoundaryManifest, TemplateBoundaryManifestReadError>;
}

/// Template export secondary port (IN-01, IN-03, AC-01, AC-03, CN-03).
///
/// Abstracts the filesystem export driven purely by manifest classifications.
pub trait TemplateExportPort: Send + Sync {
    /// Filesystem export operation (IN-01, IN-03, AC-01, AC-03, CN-03).
    ///
    /// # Errors
    ///
    /// Returns a [`TemplateExportPortError`] variant when the output directory
    /// already exists, an overlay or source path is missing, a path is
    /// unclassified, or an I/O error occurs.
    fn export(
        &self,
        command: &TemplateExportCommand,
        manifest: &TemplateBoundaryManifest,
    ) -> Result<TemplateExportReport, TemplateExportPortError>;
}

// ---------------------------------------------------------------------------
// Interactor
// ---------------------------------------------------------------------------

/// `TemplateExportService` interactor (IN-01, AC-01).
///
/// Orchestrates [`TemplateBoundaryManifestPort`] (load manifest) and
/// [`TemplateExportPort`] (perform export).
pub struct TemplateExportInteractor {
    manifest_port: Arc<dyn TemplateBoundaryManifestPort>,
    export_port: Arc<dyn TemplateExportPort>,
}

impl TemplateExportInteractor {
    /// Constructor.
    #[must_use]
    pub fn new(
        manifest_port: Arc<dyn TemplateBoundaryManifestPort>,
        export_port: Arc<dyn TemplateExportPort>,
    ) -> Self {
        Self { manifest_port, export_port }
    }
}

impl TemplateExportService for TemplateExportInteractor {
    /// Reads the boundary manifest, then performs the filesystem export.
    ///
    /// # Errors
    ///
    /// Returns [`TemplateExportError::ManifestRead`] if the manifest cannot be
    /// read, or [`TemplateExportError::Export`] if the export fails.
    fn export(
        &self,
        command: TemplateExportCommand,
    ) -> Result<TemplateExportReport, TemplateExportError> {
        let manifest = self.manifest_port.read(&command.manifest_path)?;
        let report = self.export_port.export(&command, &manifest)?;
        Ok(report)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use domain::template_export::{TemplatePathClassification, TemplatePathEntry};
    use mockall::mock;

    use super::*;

    // Use `mock!` (not `#[automock]`) so the generated mock types stay confined
    // to the test module and do not leak into the crate's public API surface
    // (which the type-signals evaluator compares against the type catalogue).
    mock! {
        ManifestPort {}
        impl TemplateBoundaryManifestPort for ManifestPort {
            fn read(
                &self,
                manifest_path: &Path,
            ) -> Result<TemplateBoundaryManifest, TemplateBoundaryManifestReadError>;
        }
    }

    mock! {
        ExportPort {}
        impl TemplateExportPort for ExportPort {
            fn export(
                &self,
                command: &TemplateExportCommand,
                manifest: &TemplateBoundaryManifest,
            ) -> Result<TemplateExportReport, TemplateExportPortError>;
        }
    }

    fn sample_command() -> TemplateExportCommand {
        TemplateExportCommand {
            workspace_root: PathBuf::from("/ws"),
            manifest_path: PathBuf::from("/ws/boundary.json"),
            overlay_dir: PathBuf::from("/ws/overlay"),
            output_dir: PathBuf::from("/out"),
        }
    }

    fn sample_manifest() -> TemplateBoundaryManifest {
        let entry = TemplatePathEntry::new(
            TemplatePathPattern::try_new("libs/domain".to_owned()).unwrap(),
            TemplatePathClassification::Include,
        );
        TemplateBoundaryManifest::try_new(vec![entry]).unwrap()
    }

    fn sample_report() -> TemplateExportReport {
        TemplateExportReport {
            included_count: 1,
            excluded_count: 0,
            overlay_applied_count: 0,
            output_dir: PathBuf::from("/out"),
        }
    }

    #[test]
    fn test_export_with_successful_ports_returns_report() {
        let mut manifest_port = MockManifestPort::new();
        manifest_port.expect_read().times(1).returning(|_| Ok(sample_manifest()));

        let mut export_port = MockExportPort::new();
        export_port.expect_export().times(1).returning(|_, _| Ok(sample_report()));

        let interactor =
            TemplateExportInteractor::new(Arc::new(manifest_port), Arc::new(export_port));
        let result = interactor.export(sample_command());

        assert_eq!(result.unwrap(), sample_report());
    }

    #[test]
    fn test_export_with_manifest_read_failure_returns_manifest_read_error() {
        let mut manifest_port = MockManifestPort::new();
        manifest_port.expect_read().times(1).returning(|_| {
            Err(TemplateBoundaryManifestReadError::NotFound {
                path: PathBuf::from("/ws/boundary.json"),
            })
        });

        // Export port must never be called when the manifest read fails.
        let mut export_port = MockExportPort::new();
        export_port.expect_export().times(0);

        let interactor =
            TemplateExportInteractor::new(Arc::new(manifest_port), Arc::new(export_port));
        let result = interactor.export(sample_command());

        assert!(matches!(
            result,
            Err(TemplateExportError::ManifestRead {
                source: TemplateBoundaryManifestReadError::NotFound { .. }
            })
        ));
    }

    #[test]
    fn test_export_with_export_port_failure_returns_export_error() {
        let mut manifest_port = MockManifestPort::new();
        manifest_port.expect_read().times(1).returning(|_| Ok(sample_manifest()));

        let mut export_port = MockExportPort::new();
        export_port.expect_export().times(1).returning(|_, _| {
            Err(TemplateExportPortError::OutputDirExists { path: PathBuf::from("/out") })
        });

        let interactor =
            TemplateExportInteractor::new(Arc::new(manifest_port), Arc::new(export_port));
        let result = interactor.export(sample_command());

        assert!(matches!(
            result,
            Err(TemplateExportError::Export {
                source: TemplateExportPortError::OutputDirExists { .. }
            })
        ));
    }

    #[test]
    fn test_manifest_read_error_from_pattern_error_maps_to_invalid_pattern() {
        let pattern_err = TemplatePathPattern::try_new(String::new()).unwrap_err();
        let read_err = TemplateBoundaryManifestReadError::from(pattern_err);
        assert!(matches!(read_err, TemplateBoundaryManifestReadError::InvalidPattern { .. }));
    }

    #[test]
    fn test_manifest_read_error_from_manifest_error_maps_to_invalid_manifest() {
        let manifest_err = TemplateBoundaryManifest::try_new(vec![]).unwrap_err();
        let read_err = TemplateBoundaryManifestReadError::from(manifest_err);
        assert!(matches!(read_err, TemplateBoundaryManifestReadError::InvalidManifest { .. }));
    }

    #[test]
    fn test_export_error_from_manifest_read_error_maps_to_manifest_read_variant() {
        let read_err = TemplateBoundaryManifestReadError::NotFound {
            path: PathBuf::from("/ws/boundary.json"),
        };
        let export_err = TemplateExportError::from(read_err);
        assert!(matches!(export_err, TemplateExportError::ManifestRead { .. }));
    }

    #[test]
    fn test_export_error_from_port_error_maps_to_export_variant() {
        let port_err =
            TemplateExportPortError::SourceMissing { path: PathBuf::from("/ws/missing") };
        let export_err = TemplateExportError::from(port_err);
        assert!(matches!(export_err, TemplateExportError::Export { .. }));
    }
}
