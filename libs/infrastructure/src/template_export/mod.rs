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

use std::path::{Component, Path, PathBuf};

use domain::{FreeText, TemplateBoundaryManifest, TemplatePathClassification, TemplatePathPattern};
use usecase::template_export::{
    TemplateBoundaryManifestPort, TemplateBoundaryManifestReadError, TemplateExportCommand,
    TemplateExportPort, TemplateExportPortError, TemplateExportReport,
};

use crate::track::symlink_guard::reject_symlinks_below;

pub mod codec;

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

        let content = std::fs::read_to_string(manifest_path).map_err(|error| {
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

/// Rejects a symlinked manifest before `read_to_string` can follow it.
fn reject_manifest_symlink(manifest_path: &Path) -> Result<(), TemplateBoundaryManifestReadError> {
    match reject_symlinks_below(manifest_path, symlink_guard_root(manifest_path)) {
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

// ---------------------------------------------------------------------------
// FsTemplateExportAdapter
// ---------------------------------------------------------------------------

/// `TemplateExportPort` filesystem adapter (spec IN-01, IN-03, AC-01, AC-03,
/// CN-03).
///
/// Before walking, it preflights every manifest entry against the workspace so a
/// pattern that no longer resolves to a real path fails closed with
/// [`TemplateExportPortError::SourceMissing`] instead of being silently dropped
/// (spec IN-12). It then walks the workspace tree top-down and applies the
/// boundary manifest classifications: `include` copies the classified subtree
/// as-is, `exclude` skips it, and `overlay` copies the overlay directory's
/// template version in its place. Traversal descends only into directories the
/// manifest does not classify; a file with no classifying ancestor is a
/// fail-closed [`TemplateExportPortError::UnclassifiedPath`] (spec IN-12). The
/// export never
/// parses or rewrites file contents (spec CN-03) and reads directory entries in
/// sorted order so identical inputs yield identical output (spec AC-01).
#[derive(Debug, Default)]
pub struct FsTemplateExportAdapter;

impl FsTemplateExportAdapter {
    /// Constructor.
    #[must_use]
    pub fn new() -> Self {
        Self
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
    ///   path has no file in the overlay directory.
    /// - [`TemplateExportPortError::SourceMissing`] if a manifest-classified
    ///   pattern has no corresponding path in the workspace (fail-closed drift
    ///   detection, spec IN-12).
    /// - [`TemplateExportPortError::UnclassifiedPath`] if a workspace file is not
    ///   classified by the manifest (fail-closed, spec IN-12).
    /// - [`TemplateExportPortError::Io`] if any filesystem operation fails.
    fn export(
        &self,
        command: &TemplateExportCommand,
        manifest: &TemplateBoundaryManifest,
    ) -> Result<TemplateExportReport, TemplateExportPortError> {
        reject_existing_export_path_symlinks(&command.workspace_root)?;
        reject_existing_export_path_symlinks(&command.overlay_dir)?;
        ensure_output_dir_absent(&command.output_dir)?;
        ensure_output_dir_outside_source_roots(command)?;
        preflight_manifest_sources(command, manifest)?;

        std::fs::create_dir_all(&command.output_dir)
            .map_err(|error| io_error(&command.output_dir, &error))?;

        let mut counts = ExportCounts::default();
        walk_and_export(Path::new(""), command, manifest, &mut counts)?;

        Ok(TemplateExportReport {
            included_count: counts.included,
            excluded_count: counts.excluded,
            overlay_applied_count: counts.overlay,
            output_dir: command.output_dir.clone(),
        })
    }
}

/// Running tally of applied classifications, mapped onto the export report.
#[derive(Debug, Default)]
struct ExportCounts {
    included: usize,
    excluded: usize,
    overlay: usize,
}

/// Preflights every manifest entry against `command.workspace_root` before the
/// export walk begins (fail-closed drift detection, spec IN-12).
///
/// The walk only visits paths that exist in the workspace, so a manifest row
/// whose source has been renamed or removed would otherwise be silently skipped,
/// yielding an incomplete template rather than a failure. Requiring each
/// classified pattern to resolve to a real workspace path — for `overlay` rows as
/// well, whose overlay-file requirement is still enforced during the walk — turns
/// that drift into a [`TemplateExportPortError::SourceMissing`]. Symlinked
/// sources are rejected in the same style as the rest of the export.
fn preflight_manifest_sources(
    command: &TemplateExportCommand,
    manifest: &TemplateBoundaryManifest,
) -> Result<(), TemplateExportPortError> {
    for entry in manifest.entries() {
        let source = command.workspace_root.join(entry.pattern().as_str());
        if !reject_export_path_symlinks(&source)? {
            return Err(TemplateExportPortError::SourceMissing { path: source });
        }
    }
    Ok(())
}

/// Recursively walks `rel` (relative to `command.workspace_root`) and applies
/// the manifest classification of each child, descending only into directories
/// the manifest leaves unclassified.
fn walk_and_export(
    rel: &Path,
    command: &TemplateExportCommand,
    manifest: &TemplateBoundaryManifest,
    counts: &mut ExportCounts,
) -> Result<(), TemplateExportPortError> {
    let source_dir = command.workspace_root.join(rel);
    for entry in sorted_dir_entries(&source_dir)? {
        let child_rel = rel.join(entry.file_name());
        let pattern = pattern_for(&child_rel)?;

        match manifest.classify(&pattern) {
            Some(TemplatePathClassification::Include) => {
                copy_tree(
                    &command.workspace_root.join(&child_rel),
                    &command.output_dir.join(&child_rel),
                )?;
                counts.included += 1;
            }
            Some(TemplatePathClassification::Exclude) => {
                counts.excluded += 1;
            }
            Some(TemplatePathClassification::Overlay) => {
                let overlay_source = command.overlay_dir.join(&child_rel);
                if !ensure_overlay_source_exists(&overlay_source)? {
                    return Err(TemplateExportPortError::OverlayMissing {
                        pattern,
                        overlay_path: overlay_source,
                    });
                }
                copy_tree(&overlay_source, &command.output_dir.join(&child_rel))?;
                counts.overlay += 1;
            }
            None => {
                let file_type = entry.file_type().map_err(|error| io_error(&child_rel, &error))?;
                if file_type.is_dir() {
                    walk_and_export(&child_rel, command, manifest, counts)?;
                } else {
                    return Err(TemplateExportPortError::UnclassifiedPath { path: pattern });
                }
            }
        }
    }
    Ok(())
}

/// Reads the entries of `dir` in file-name order so traversal is deterministic
/// (spec AC-01); `read_dir` itself yields an unspecified order.
fn sorted_dir_entries(dir: &Path) -> Result<Vec<std::fs::DirEntry>, TemplateExportPortError> {
    let mut entries = std::fs::read_dir(dir)
        .map_err(|error| io_error(dir, &error))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| io_error(dir, &error))?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    Ok(entries)
}

/// Verifies an overlay source exists without following symlinks.
fn ensure_overlay_source_exists(overlay_source: &Path) -> Result<bool, TemplateExportPortError> {
    reject_export_path_symlinks(overlay_source)
}

/// Builds a [`TemplatePathPattern`] for a workspace-relative path, mapping the
/// (practically unreachable) validation failure onto an I/O-style error.
fn pattern_for(rel: &Path) -> Result<TemplatePathPattern, TemplateExportPortError> {
    let raw = rel.to_str().ok_or_else(|| TemplateExportPortError::Io {
        path: rel.to_path_buf(),
        reason: FreeText::new("workspace path is not valid UTF-8".to_owned()),
    })?;
    TemplatePathPattern::try_new(raw.to_owned()).map_err(|error| TemplateExportPortError::Io {
        path: rel.to_path_buf(),
        reason: FreeText::new(error.to_string()),
    })
}

/// Copies `source` to `destination` verbatim, recursing into directories.
///
/// File contents are copied byte-for-byte via [`std::fs::copy`]; the export
/// never rewrites contents (spec CN-03). Directory entries are copied in sorted
/// order so the output is deterministic (spec AC-01).
fn copy_tree(source: &Path, destination: &Path) -> Result<(), TemplateExportPortError> {
    let metadata = non_symlink_metadata(source)?;

    if metadata.is_dir() {
        std::fs::create_dir_all(destination).map_err(|error| io_error(destination, &error))?;
        for entry in sorted_dir_entries(source)? {
            copy_tree(&entry.path(), &destination.join(entry.file_name()))?;
        }
    } else {
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent).map_err(|error| io_error(parent, &error))?;
        }
        std::fs::copy(source, destination).map_err(|error| io_error(source, &error))?;
    }
    Ok(())
}

/// Reads metadata for `path`, rejecting symlinks before any operation can follow
/// them.
fn non_symlink_metadata(path: &Path) -> Result<std::fs::Metadata, TemplateExportPortError> {
    if !reject_export_path_symlinks(path)? {
        let error = std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("path not found: {}", path.display()),
        );
        return Err(io_error(path, &error));
    }
    std::fs::symlink_metadata(path).map_err(|error| io_error(path, &error))
}

fn reject_existing_export_path_symlinks(path: &Path) -> Result<(), TemplateExportPortError> {
    reject_export_path_symlinks(path).map(|_| ())
}

fn ensure_output_dir_absent(output_dir: &Path) -> Result<(), TemplateExportPortError> {
    if reject_export_path_symlinks(output_dir)? {
        return Err(TemplateExportPortError::OutputDirExists { path: output_dir.to_path_buf() });
    }
    Ok(())
}

fn ensure_output_dir_outside_source_roots(
    command: &TemplateExportCommand,
) -> Result<(), TemplateExportPortError> {
    let output_dir = absolute_lexical_path(&command.output_dir)?;
    reject_nested_output_dir(&command.output_dir, &output_dir, &command.workspace_root)?;
    reject_nested_output_dir(&command.output_dir, &output_dir, &command.overlay_dir)?;
    Ok(())
}

fn reject_nested_output_dir(
    original_output_dir: &Path,
    output_dir: &Path,
    source_root: &Path,
) -> Result<(), TemplateExportPortError> {
    let source_root = absolute_lexical_path(source_root)?;
    if output_dir.starts_with(&source_root) {
        let error = std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("output directory must not be inside source root {}", source_root.display()),
        );
        return Err(io_error(original_output_dir, &error));
    }
    Ok(())
}

fn absolute_lexical_path(path: &Path) -> Result<PathBuf, TemplateExportPortError> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().map_err(|error| io_error(path, &error))?.join(path)
    };
    Ok(lexical_normalize(&absolute))
}

fn lexical_normalize(path: &Path) -> PathBuf {
    let mut components: Vec<Component<'_>> = Vec::new();
    for component in path.components() {
        match component {
            Component::ParentDir => match components.last() {
                Some(Component::Normal(_)) => {
                    components.pop();
                }
                _ => {
                    components.push(component);
                }
            },
            Component::CurDir => {}
            _ => {
                components.push(component);
            }
        }
    }
    components.iter().collect()
}

fn reject_export_path_symlinks(path: &Path) -> Result<bool, TemplateExportPortError> {
    reject_symlinks_below(path, symlink_guard_root(path)).map_err(|error| io_error(path, &error))
}

fn symlink_guard_root(path: &Path) -> &Path {
    path.ancestors().last().unwrap_or_else(|| Path::new(""))
}

/// Wraps a [`std::io::Error`] into a [`TemplateExportPortError::Io`] carrying the
/// offending path.
fn io_error(path: &Path, error: &std::io::Error) -> TemplateExportPortError {
    TemplateExportPortError::Io {
        path: path.to_path_buf(),
        reason: FreeText::new(error.to_string()),
    }
}
