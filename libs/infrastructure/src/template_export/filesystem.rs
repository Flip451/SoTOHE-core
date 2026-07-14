//! Filesystem safety checks and deterministic directory access for template export.

use std::path::{Component, Path, PathBuf};

use domain::FreeText;
use usecase::template_export::{TemplateExportCommand, TemplateExportPortError};

use crate::track::symlink_guard::reject_symlinks_below;

/// Reads metadata for `path`, rejecting symlinks before any operation can follow
/// them.
pub(super) fn non_symlink_metadata(
    path: &Path,
) -> Result<std::fs::Metadata, TemplateExportPortError> {
    if !reject_export_path_symlinks(path)? {
        let error = std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("path not found: {}", path.display()),
        );
        return Err(io_error(path, &error));
    }
    std::fs::symlink_metadata(path).map_err(|error| io_error(path, &error))
}

/// Reads entries in file-name order so traversal remains deterministic.
pub(super) fn sorted_dir_entries(
    dir: &Path,
) -> Result<Vec<std::fs::DirEntry>, TemplateExportPortError> {
    let mut entries = std::fs::read_dir(dir)
        .map_err(|error| io_error(dir, &error))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| io_error(dir, &error))?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    Ok(entries)
}

/// Verifies an overlay source exists without following symlinks.
pub(super) fn ensure_overlay_source_exists(
    overlay_source: &Path,
) -> Result<bool, TemplateExportPortError> {
    reject_export_path_symlinks(overlay_source)
}

pub(super) fn reject_existing_export_path_symlinks(
    path: &Path,
) -> Result<(), TemplateExportPortError> {
    reject_export_path_symlinks(path).map(|_| ())
}

pub(super) fn ensure_output_dir_absent(output_dir: &Path) -> Result<(), TemplateExportPortError> {
    if reject_export_path_symlinks(output_dir)? {
        return Err(TemplateExportPortError::OutputDirExists { path: output_dir.to_path_buf() });
    }
    Ok(())
}

pub(super) fn ensure_output_dir_outside_source_roots(
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

pub(super) fn reject_export_path_symlinks(path: &Path) -> Result<bool, TemplateExportPortError> {
    reject_symlinks_below(path, symlink_guard_root(path)).map_err(|error| io_error(path, &error))
}

pub(super) fn symlink_guard_root(path: &Path) -> &Path {
    path.ancestors().last().unwrap_or_else(|| Path::new(""))
}

/// Wraps a [`std::io::Error`] into a [`TemplateExportPortError::Io`] carrying the
/// offending path.
pub(super) fn io_error(path: &Path, error: &std::io::Error) -> TemplateExportPortError {
    TemplateExportPortError::Io {
        path: path.to_path_buf(),
        reason: FreeText::new(error.to_string()),
    }
}
