//! Infrastructure adapters for the template extraction boundary (spec IN-01,
//! IN-02, IN-03, IN-12, AC-01, AC-02, AC-03, CN-02, CN-03).
//!
//! [`codec`] provides the fail-closed serde decoder for the boundary manifest
//! (the machine-readable boundary SSoT; ADR D4). This module adds the two
//! filesystem secondary adapters the export use case drives:
//!
//! - [`FsTemplateBoundaryManifestAdapter`] reads the manifest file from disk and
//!   decodes it via [`codec::decode_manifest`], failing closed on any I/O,
//!   parse, or domain-invariant error (spec IN-02, IN-12, AC-02, CN-02).
//! - [`FsTemplateExportAdapter`] walks the workspace tree and applies the
//!   manifest classifications — `include` copies a subtree as-is, `exclude`
//!   skips it, `overlay` copies the overlay directory's template version — and
//!   fails closed on any path the manifest does not classify (spec IN-01, IN-03,
//!   IN-12, AC-01, AC-03, CN-03). It never parses or rewrites file contents.

use std::path::{Path, PathBuf};

use domain::{FreeText, TemplateBoundaryManifest};
use usecase::template_export::{
    SelfBinaryTransplantError, SelfBinaryTransplantPort, TemplateBoundaryManifestPort,
    TemplateBoundaryManifestReadError, TemplateExportCommand, TemplateExportPort,
    TemplateExportPortError, TemplateExportReport,
};

use crate::capability_exec::bounded_read_utf8_file;

pub mod codec;

mod export_walk;
mod filesystem;
mod gitignore_classification;
pub(crate) mod machine_path_scan;

pub use codec::{
    MANIFEST_SCHEMA_VERSION, TemplateBoundaryManifestCodecError, TemplateBoundaryManifestDto,
    TemplatePathClassificationDto, TemplatePathEntryDto, decode_manifest,
};

#[cfg(test)]
mod tests;

// ---------------------------------------------------------------------------
// FsTemplateBoundaryManifestAdapter
// ---------------------------------------------------------------------------

/// `TemplateBoundaryManifestPort` filesystem adapter (spec IN-02, AC-02, CN-02).
///
/// Reads the boundary manifest file from disk and decodes it through
/// [`codec::decode_manifest`]. Every failure mode is fail-closed (spec IN-12):
/// a missing file, an I/O error, malformed JSON, an unsupported schema version,
/// an invalid pattern, or a manifest-invariant violation all surface as a
/// [`TemplateBoundaryManifestReadError`] rather than a degraded manifest.
#[derive(Debug, Default)]
pub struct FsTemplateBoundaryManifestAdapter;

impl FsTemplateBoundaryManifestAdapter {
    /// Constructor.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl TemplateBoundaryManifestPort for FsTemplateBoundaryManifestAdapter {
    /// Reads and decodes the boundary manifest at `manifest_path`.
    ///
    /// # Errors
    ///
    /// - [`TemplateBoundaryManifestReadError::NotFound`] if no file exists at
    ///   `manifest_path`.
    /// - [`TemplateBoundaryManifestReadError::Io`] if the file cannot be read.
    /// - [`TemplateBoundaryManifestReadError::Parse`] if the JSON is malformed or
    ///   declares an unsupported schema version.
    /// - [`TemplateBoundaryManifestReadError::InvalidPattern`] if an entry pattern
    ///   is not workspace-relative.
    /// - [`TemplateBoundaryManifestReadError::InvalidManifest`] if the entry set
    ///   is empty or contains a duplicate pattern.
    fn read(
        &self,
        manifest_path: &Path,
    ) -> Result<TemplateBoundaryManifest, TemplateBoundaryManifestReadError> {
        reject_manifest_symlink(manifest_path)?;

        let content = bounded_read_utf8_file(manifest_path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                TemplateBoundaryManifestReadError::NotFound { path: manifest_path.to_path_buf() }
            } else {
                TemplateBoundaryManifestReadError::Io {
                    path: manifest_path.to_path_buf(),
                    reason: FreeText::new(error.to_string()),
                }
            }
        })?;

        decode_manifest(&content).map_err(|error| decode_error_to_read_error(manifest_path, error))
    }
}

/// Rejects a symlinked manifest before the bounded reader can follow it.
fn reject_manifest_symlink(manifest_path: &Path) -> Result<(), TemplateBoundaryManifestReadError> {
    match crate::track::symlink_guard::reject_symlinks_below(
        manifest_path,
        filesystem::symlink_guard_root(manifest_path),
    ) {
        Ok(true) => Ok(()),
        Ok(false) => {
            Err(TemplateBoundaryManifestReadError::NotFound { path: manifest_path.to_path_buf() })
        }
        Err(error) => Err(TemplateBoundaryManifestReadError::Io {
            path: manifest_path.to_path_buf(),
            reason: FreeText::new(error.to_string()),
        }),
    }
}

/// Maps a [`TemplateBoundaryManifestCodecError`] onto the port-level read error,
/// attaching the manifest path to the parse/I/O-adjacent variants.
fn decode_error_to_read_error(
    manifest_path: &Path,
    error: TemplateBoundaryManifestCodecError,
) -> TemplateBoundaryManifestReadError {
    match error {
        TemplateBoundaryManifestCodecError::Json { reason } => {
            TemplateBoundaryManifestReadError::Parse { path: manifest_path.to_path_buf(), reason }
        }
        // A wrong schema version is a fail-closed parse rejection (spec IN-12).
        schema_error @ TemplateBoundaryManifestCodecError::SchemaVersion { .. } => {
            TemplateBoundaryManifestReadError::Parse {
                path: manifest_path.to_path_buf(),
                reason: FreeText::new(schema_error.to_string()),
            }
        }
        TemplateBoundaryManifestCodecError::Pattern { source } => {
            TemplateBoundaryManifestReadError::InvalidPattern { source }
        }
        TemplateBoundaryManifestCodecError::Manifest { source } => {
            TemplateBoundaryManifestReadError::InvalidManifest { source }
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod bounded_manifest_read_tests {
    use tempfile::TempDir;
    use usecase::template_export::{
        TemplateBoundaryManifestPort, TemplateBoundaryManifestReadError,
    };

    use super::FsTemplateBoundaryManifestAdapter;
    use crate::capability_exec::MAX_CAPABILITY_EXEC_TEXT_BYTES;

    #[test]
    fn test_manifest_read_oversized_file_returns_io_error() {
        let temp_dir = TempDir::new().unwrap();
        let manifest_path = temp_dir.path().join("template-boundary.json");
        std::fs::File::create(&manifest_path)
            .unwrap()
            .set_len(MAX_CAPABILITY_EXEC_TEXT_BYTES.saturating_add(1))
            .unwrap();

        let error = FsTemplateBoundaryManifestAdapter::new().read(&manifest_path).unwrap_err();

        assert!(matches!(error, TemplateBoundaryManifestReadError::Io { .. }));
    }
}

// ---------------------------------------------------------------------------
// FsTemplateExportAdapter
// ---------------------------------------------------------------------------

/// `TemplateExportPort` filesystem adapter (spec IN-01, IN-03, AC-01, AC-03,
/// CN-03).
///
/// Before walking, it preflights every manifest entry per classification so a
/// pattern that no longer resolves to a real source fails closed with
/// [`TemplateExportPortError::SourceMissing`] instead of being silently dropped
/// (spec IN-12): an `include` requires its workspace path, an `overlay` requires
/// either its workspace anchor or overlay content, and an `exclude` requires
/// nothing (an already-absent excluded path is a no-op). It then walks the
/// workspace tree top-down and applies the boundary manifest classifications:
/// `include` copies the classified subtree as-is, `exclude` skips it, and
/// `overlay` copies the overlay directory's template version in its place.
/// `overlay` rows whose workspace anchor is absent are emitted in a final pass.
/// Traversal descends only into directories the manifest does not classify; a
/// file with no classifying ancestor is a fail-closed
/// [`TemplateExportPortError::UnclassifiedPath`] (spec IN-12). The export never
/// parses or rewrites file contents (spec CN-03) and reads directory entries in
/// sorted order so identical inputs yield identical output (spec AC-01). Its
/// machine-home scan input is injected by the composition root; this adapter
/// never reads ambient environment variables (spec IN-06, AC-13).
#[derive(Debug)]
pub struct FsTemplateExportAdapter {
    machine_home_dir: Option<PathBuf>,
}

impl FsTemplateExportAdapter {
    /// Constructs the adapter with the machine home directory resolved by the
    /// composition root.
    ///
    /// `None` is preserved so exported-output scanning can fail closed when
    /// the composition root cannot resolve a machine home directory.
    #[must_use]
    pub fn new(machine_home_dir: Option<PathBuf>) -> Self {
        Self { machine_home_dir }
    }
}

impl TemplateExportPort for FsTemplateExportAdapter {
    /// Exports the workspace into `command.output_dir` driven by `manifest`.
    ///
    /// # Errors
    ///
    /// - [`TemplateExportPortError::OutputDirExists`] if the output directory
    ///   already exists (the export refuses to overwrite).
    /// - [`TemplateExportPortError::OverlayMissing`] if an `overlay`-classified
    ///   path with a present workspace anchor has no file in the overlay
    ///   directory.
    /// - [`TemplateExportPortError::SourceMissing`] if an `include` pattern (or an
    ///   `overlay` pattern with neither a workspace anchor nor overlay content)
    ///   has no source in the workspace — fail-closed drift detection (spec
    ///   IN-12). Absent `exclude` patterns are permitted.
    /// - [`TemplateExportPortError::UnclassifiedPath`] if a workspace file is not
    ///   classified by the manifest (fail-closed, spec IN-12).
    /// - [`TemplateExportPortError::Io`] if any filesystem operation fails.
    fn export(
        &self,
        command: &TemplateExportCommand,
        manifest: &TemplateBoundaryManifest,
    ) -> Result<TemplateExportReport, TemplateExportPortError> {
        filesystem::reject_existing_export_path_symlinks(&command.workspace_root)?;
        filesystem::reject_existing_export_path_symlinks(&command.overlay_dir)?;
        filesystem::ensure_output_dir_absent(&command.output_dir)?;
        filesystem::ensure_output_dir_outside_source_roots(command)?;

        let counts = export_walk::export_worktree(command, manifest)?;
        // A workspace-local home is container-local and must fail closed rather than skip a scan.
        if machine_path_scan::exported_output_scan_is_required(
            &command.output_dir,
            self.machine_home_dir.as_deref(),
            &command.workspace_root,
        )? {
            machine_path_scan::ensure_exported_output_has_no_machine_paths(
                &command.output_dir,
                self.machine_home_dir.as_deref(),
            )?;
        }

        Ok(TemplateExportReport {
            included_count: counts.included,
            excluded_count: counts.excluded,
            overlay_applied_count: counts.overlay,
            output_dir: command.output_dir.clone(),
        })
    }
}

// ---------------------------------------------------------------------------
// FsSelfBinaryTransplantAdapter
// ---------------------------------------------------------------------------

/// `SelfBinaryTransplantPort` filesystem adapter (spec IN-01, IN-02, CN-01,
/// CN-02, CN-06, AC-01, AC-03).
///
/// Resolves the running-process binary path via [`std::env::current_exe`],
/// copies it verbatim to the destination via [`std::fs::copy`] (byte-identity
/// is a property of the copy itself; the export never rewrites contents), and
/// preserves the executable permission bit on unix. Each failure mode surfaces
/// as a distinct [`SelfBinaryTransplantError`] variant so callers can report
/// the failing stage precisely (fail-closed, spec CN-06).
///
/// Non-unix platforms skip the permission-set step because the concept of an
/// executable bit does not apply the same way; the copy alone is sufficient
/// there.
#[derive(Debug, Default)]
pub struct FsSelfBinaryTransplantAdapter;

impl FsSelfBinaryTransplantAdapter {
    /// Constructor.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl SelfBinaryTransplantPort for FsSelfBinaryTransplantAdapter {
    /// Transplants the running binary to `destination` verbatim, preserving
    /// executable permission on unix.
    ///
    /// # Errors
    ///
    /// - [`SelfBinaryTransplantError::SourcePathUnavailable`] if
    ///   [`std::env::current_exe`] fails to resolve the running binary path.
    /// - [`SelfBinaryTransplantError::DestinationWriteFailure`] if the
    ///   destination's parent directory cannot be created or the byte-copy
    ///   fails.
    /// - [`SelfBinaryTransplantError::PermissionSetFailure`] (unix only) if the
    ///   source metadata cannot be read or the executable permission cannot be
    ///   applied to the destination.
    fn transplant(&self, destination: &Path) -> Result<(), SelfBinaryTransplantError> {
        let source = std::env::current_exe().map_err(|error| {
            SelfBinaryTransplantError::SourcePathUnavailable {
                reason: FreeText::new(error.to_string()),
            }
        })?;

        // Ensure the parent directory (`<output_dir>/bin/`) exists before the
        // copy. Both parent-creation failure and the copy itself surface as
        // `DestinationWriteFailure` — from the caller's perspective the write
        // simply cannot land at `destination`.
        if let Some(parent) = destination.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|error| {
                    SelfBinaryTransplantError::DestinationWriteFailure {
                        path: destination.to_path_buf(),
                        reason: FreeText::new(error.to_string()),
                    }
                })?;
            }
        }

        std::fs::copy(&source, destination).map_err(|error| {
            SelfBinaryTransplantError::DestinationWriteFailure {
                path: destination.to_path_buf(),
                reason: FreeText::new(error.to_string()),
            }
        })?;

        #[cfg(unix)]
        {
            let permissions = std::fs::metadata(&source)
                .map_err(|error| SelfBinaryTransplantError::PermissionSetFailure {
                    path: destination.to_path_buf(),
                    reason: FreeText::new(error.to_string()),
                })?
                .permissions();
            std::fs::set_permissions(destination, permissions).map_err(|error| {
                SelfBinaryTransplantError::PermissionSetFailure {
                    path: destination.to_path_buf(),
                    reason: FreeText::new(error.to_string()),
                }
            })?;
        }

        Ok(())
    }
}
