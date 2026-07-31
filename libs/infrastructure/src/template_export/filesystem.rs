//! Filesystem safety checks and deterministic directory access for template export.

use std::path::{Path, PathBuf};

use domain::FreeText;
use usecase::template_export::{TemplateExportCommand, TemplateExportPortError};

use crate::lexical_path::lexical_normalize;
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

/// The most entries one exported directory may hold.
///
/// Orders of magnitude above any directory in this workspace — the largest holds
/// a few hundred files — so an honest export never meets it, while a directory
/// large enough to exhaust memory stops the export instead of being collected.
const MAX_TEMPLATE_DIR_ENTRIES: usize = 10_000;

/// Reads entries in file-name order so traversal remains deterministic.
///
/// # Errors
///
/// Returns [`TemplateExportPortError::Io`] when the directory cannot be read, or
/// when it holds more entries than an export will collect.
pub(super) fn sorted_dir_entries(
    dir: &Path,
) -> Result<Vec<std::fs::DirEntry>, TemplateExportPortError> {
    sorted_dir_entries_within(dir, MAX_TEMPLATE_DIR_ENTRIES)
}

/// The body of [`sorted_dir_entries`], with the budget supplied so the refusal can
/// be exercised without building a directory of the production size.
fn sorted_dir_entries_within(
    dir: &Path,
    budget: usize,
) -> Result<Vec<std::fs::DirEntry>, TemplateExportPortError> {
    let mut entries = Vec::new();
    // Counted while reading rather than collected and measured: the point of the
    // budget is that an oversized directory never occupies memory in the first
    // place.
    for entry in std::fs::read_dir(dir).map_err(|error| io_error(dir, &error))? {
        if entries.len() >= budget {
            return Err(TemplateExportPortError::Io {
                path: dir.to_path_buf(),
                reason: FreeText::new(format!("directory holds more than {budget} entries")),
            });
        }
        entries.push(entry.map_err(|error| io_error(dir, &error))?);
    }
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_a_directory_within_the_budget_is_read_in_file_name_order() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["c.txt", "a.txt", "b.txt"] {
            std::fs::write(dir.path().join(name), "x").unwrap();
        }

        let entries = sorted_dir_entries_within(dir.path(), 3).unwrap();

        let names: Vec<String> =
            entries.iter().map(|entry| entry.file_name().to_string_lossy().into_owned()).collect();
        assert_eq!(names, vec!["a.txt", "b.txt", "c.txt"], "order is deterministic");
    }

    #[test]
    fn test_a_directory_past_the_budget_stops_the_export_instead_of_being_collected() {
        let dir = tempfile::tempdir().unwrap();
        for index in 0..4 {
            std::fs::write(dir.path().join(format!("entry-{index}.txt")), "x").unwrap();
        }

        let error = sorted_dir_entries_within(dir.path(), 3)
            .expect_err("a directory past the budget must be refused");

        let TemplateExportPortError::Io { reason, .. } = error else {
            panic!("expected an I/O refusal: {error:?}");
        };
        assert!(reason.as_str().contains("more than 3 entries"), "got: {reason}");
    }
}
