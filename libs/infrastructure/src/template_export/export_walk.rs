//! Manifest-driven filesystem traversal and materialization for template export.

use std::path::Path;

use domain::{FreeText, TemplateBoundaryManifest, TemplatePathClassification, TemplatePathPattern};
use usecase::template_export::{TemplateExportCommand, TemplateExportPortError};

use super::filesystem::{
    ensure_overlay_source_exists, io_error, non_symlink_metadata, reject_export_path_symlinks,
    sorted_dir_entries,
};
use super::gitignore_classification::is_gitignored_untracked;

/// Running tally of classifications applied while materializing the export.
#[derive(Debug, Default)]
pub(super) struct ExportCounts {
    pub(super) included: usize,
    pub(super) excluded: usize,
    pub(super) overlay: usize,
}

/// Preflights, creates, and materializes the manifest-resolved output tree.
pub(super) fn export_worktree(
    command: &TemplateExportCommand,
    manifest: &TemplateBoundaryManifest,
) -> Result<ExportCounts, TemplateExportPortError> {
    preflight_manifest_sources(command, manifest)?;
    std::fs::create_dir_all(&command.output_dir)
        .map_err(|error| io_error(&command.output_dir, &error))?;

    let mut counts = ExportCounts::default();
    walk_and_export(Path::new(""), command, manifest, &mut counts)?;
    emit_overlay_only_entries(command, manifest, &mut counts)?;
    Ok(counts)
}

/// Preflights every manifest entry before the export walk begins so drift is
/// caught up front rather than silently producing an incomplete template
/// (fail-closed, spec IN-12). The requirement is per-classification, because the
/// walk only ever visits paths that exist in the workspace:
///
/// - `include`: the workspace path MUST exist — otherwise the walk would never
///   reach it and the subtree would be silently dropped
///   ([`TemplateExportPortError::SourceMissing`]).
/// - `exclude`: absence is acceptable — an excluded path that is already gone
///   just means there is nothing to skip, so no existence is required.
/// - `overlay`: the workspace anchor OR the overlay content must exist. Only when
///   *neither* is present is the row drift ([`TemplateExportPortError::SourceMissing`]).
///   A present anchor with absent overlay content stays an `OverlayMissing`
///   during the walk; an absent anchor with present overlay content is emitted by
///   [`emit_overlay_only_entries`].
///
/// Symlinked sources are rejected in the same style as the rest of the export.
fn preflight_manifest_sources(
    command: &TemplateExportCommand,
    manifest: &TemplateBoundaryManifest,
) -> Result<(), TemplateExportPortError> {
    for entry in manifest.entries() {
        let workspace_source = command.workspace_root.join(entry.pattern().as_str());
        match entry.classification() {
            TemplatePathClassification::Include => {
                if !reject_export_path_symlinks(&workspace_source)? {
                    return Err(TemplateExportPortError::SourceMissing { path: workspace_source });
                }
            }
            // Nothing to ship: an absent excluded path is a no-op, not drift.
            TemplatePathClassification::Exclude => {}
            TemplatePathClassification::Overlay => {
                let workspace_exists = reject_export_path_symlinks(&workspace_source)?;
                let overlay_source = command.overlay_dir.join(entry.pattern().as_str());
                let overlay_exists = reject_export_path_symlinks(&overlay_source)?;
                if !workspace_exists && !overlay_exists {
                    return Err(TemplateExportPortError::SourceMissing { path: workspace_source });
                }
            }
        }
    }
    Ok(())
}

/// Emits `overlay` rows whose workspace anchor is absent but whose overlay
/// content exists (spec IN-01, AC-03).
///
/// The walk classifies only paths that exist in the workspace, so an overlay row
/// backed solely by overlay content (e.g. a gitignored generated view that lives
/// only under `overlay/`) is invisible to it. This pass copies that overlay
/// content to the output path directly — creating parent directories as needed
/// and counting it toward the overlay-applied tally — so such rows still ship.
/// Rows whose workspace anchor exists are left to the walk, avoiding a double
/// emission. Preflight already guaranteed overlay content is present when the
/// anchor is absent, but the presence is re-checked to stay fail-closed.
fn emit_overlay_only_entries(
    command: &TemplateExportCommand,
    manifest: &TemplateBoundaryManifest,
    counts: &mut ExportCounts,
) -> Result<(), TemplateExportPortError> {
    for entry in manifest.entries() {
        if !matches!(entry.classification(), TemplatePathClassification::Overlay) {
            continue;
        }
        let workspace_source = command.workspace_root.join(entry.pattern().as_str());
        // A present anchor is handled by the walk; only anchorless rows land here.
        if reject_export_path_symlinks(&workspace_source)? {
            continue;
        }
        let overlay_source = command.overlay_dir.join(entry.pattern().as_str());
        if !ensure_overlay_source_exists(&overlay_source)? {
            return Err(TemplateExportPortError::OverlayMissing {
                pattern: entry.pattern().clone(),
                overlay_path: overlay_source,
            });
        }
        copy_tree(&overlay_source, &command.output_dir.join(entry.pattern().as_str()))?;
        counts.overlay += 1;
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
                if copy_workspace_tree(
                    &command.workspace_root,
                    &child_rel,
                    &command.output_dir.join(&child_rel),
                )? {
                    counts.included += 1;
                }
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
                if is_gitignored_untracked(&command.workspace_root, pattern.as_str())? {
                    continue;
                }
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

/// Copies a workspace include entry to `destination`, skipping untracked
/// gitignored entries before recursing into their subtrees.
///
/// Returns `false` when `relative_source` itself is skipped. This lets the
/// caller avoid counting an ignored include root as materialized.
fn copy_workspace_tree(
    workspace_root: &Path,
    relative_source: &Path,
    destination: &Path,
) -> Result<bool, TemplateExportPortError> {
    let pattern = pattern_for(relative_source)?;
    if is_gitignored_untracked(workspace_root, pattern.as_str())? {
        return Ok(false);
    }

    let source = workspace_root.join(relative_source);
    let metadata = non_symlink_metadata(&source)?;
    if metadata.is_dir() {
        std::fs::create_dir_all(destination).map_err(|error| io_error(destination, &error))?;
        for entry in sorted_dir_entries(&source)? {
            let child_rel = relative_source.join(entry.file_name());
            copy_workspace_tree(workspace_root, &child_rel, &destination.join(entry.file_name()))?;
        }
    } else {
        copy_file(&source, destination)?;
    }

    Ok(true)
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
        copy_file(source, destination)?;
    }
    Ok(())
}

/// Copies one file and creates its output parent when necessary.
fn copy_file(source: &Path, destination: &Path) -> Result<(), TemplateExportPortError> {
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent).map_err(|error| io_error(parent, &error))?;
    }
    std::fs::copy(source, destination).map_err(|error| io_error(source, &error))?;
    Ok(())
}
