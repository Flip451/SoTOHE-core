//! Fail-closed scanning of exported files for work-machine home paths.

use std::fs::File;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use domain::FreeText;
use domain::review_v2::FilePath;
use usecase::template_export::TemplateExportPortError;

use super::filesystem::{io_error, non_symlink_metadata, sorted_dir_entries};

/// Rejects output containing the current work machine's home directory.
pub(super) fn ensure_exported_output_has_no_machine_paths(
    output_dir: &Path,
    machine_home_dir: Option<&Path>,
) -> Result<(), TemplateExportPortError> {
    let home_dir = machine_home_dir.ok_or_else(|| {
        let error = std::io::Error::other(
            "machine home directory must be resolved to scan exported output for work-machine paths",
        );
        io_error(output_dir, &error)
    })?;
    let home_dir = normalized_machine_home_path_bytes(output_dir, home_dir)?;
    scan_exported_output_for_machine_paths(output_dir, output_dir, &home_dir)
}

/// Returns whether output scanning is required for the injected machine home.
///
/// A machine home below the workspace denotes a shipped container path, not
/// work-machine identity (CN-03). Validate the path before making that
/// exception so a lexical `..` component cannot bypass the export scan.
pub(super) fn exported_output_scan_is_required(
    output_dir: &Path,
    machine_home_dir: Option<&Path>,
    workspace_root: &Path,
) -> Result<bool, TemplateExportPortError> {
    let Some(machine_home_dir) = machine_home_dir else {
        return Ok(true);
    };

    // Keep this validation aligned with the scanner's representation. An
    // invalid home fails closed even when its raw prefix appears to be in the
    // workspace.
    normalized_machine_home_path_bytes(output_dir, machine_home_dir)?;

    // Resolve both sides before containment testing so a relative workspace
    // root and a symlinked container mount agree with the absolute home path.
    // If either path cannot be resolved, retain the scan: a failed exception
    // check must not weaken the exported-output protection.
    let workspace_root = std::fs::canonicalize(workspace_root);
    let machine_home_dir = std::fs::canonicalize(machine_home_dir);
    match (workspace_root, machine_home_dir) {
        (Ok(workspace_root), Ok(machine_home_dir)) => {
            Ok(!machine_home_dir.starts_with(workspace_root))
        }
        _ => Ok(true),
    }
}

/// Returns the normalized, lossless representation used to scan machine paths.
fn normalized_machine_home_path_bytes(
    output_dir: &Path,
    machine_home_dir: &Path,
) -> Result<Vec<u8>, TemplateExportPortError> {
    if !machine_home_dir.is_absolute()
        || machine_home_dir.components().any(|component| matches!(component, Component::ParentDir))
    {
        let error = std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "machine home directory must be an absolute path without parent components",
        );
        return Err(io_error(output_dir, &error));
    }

    let normalized: PathBuf = machine_home_dir.components().collect();
    if normalized.file_name().is_none() {
        let error = std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "machine home directory must name a directory below its filesystem root",
        );
        return Err(io_error(output_dir, &error));
    }

    platform_path_bytes(output_dir, &normalized)
}

/// Returns the bytes that a machine-home path would have in exported text.
#[cfg(unix)]
fn platform_path_bytes(
    _output_dir: &Path,
    machine_home_dir: &Path,
) -> Result<Vec<u8>, TemplateExportPortError> {
    use std::os::unix::ffi::OsStrExt as _;

    Ok(machine_home_dir.as_os_str().as_bytes().to_vec())
}

/// Returns the bytes that a machine-home path would have in exported text.
#[cfg(not(unix))]
fn platform_path_bytes(
    output_dir: &Path,
    machine_home_dir: &Path,
) -> Result<Vec<u8>, TemplateExportPortError> {
    let machine_home_dir = machine_home_dir.to_str().ok_or_else(|| {
        let error = std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "machine home directory cannot be represented as UTF-8 for export scanning",
        );
        io_error(output_dir, &error)
    })?;
    Ok(machine_home_dir.as_bytes().to_vec())
}

/// Recursively reports the first output file containing the machine home path.
fn scan_exported_output_for_machine_paths(
    output_dir: &Path,
    directory: &Path,
    home_dir: &[u8],
) -> Result<(), TemplateExportPortError> {
    for entry in sorted_dir_entries(directory)? {
        let path = entry.path();
        let metadata = non_symlink_metadata(&path)?;
        if metadata.is_dir() {
            scan_exported_output_for_machine_paths(output_dir, &path, home_dir)?;
            continue;
        }

        if file_contains_machine_home_path(&path, home_dir)? {
            let relative_path = path.strip_prefix(output_dir).map_err(|error| {
                let error = std::io::Error::other(error);
                io_error(&path, &error)
            })?;
            let relative_path =
                relative_path.to_str().ok_or_else(|| TemplateExportPortError::Io {
                    path: relative_path.to_path_buf(),
                    reason: FreeText::new("exported output path is not valid UTF-8".to_owned()),
                })?;
            let path = FilePath::new(relative_path.to_owned()).map_err(|error| {
                TemplateExportPortError::Io {
                    path: relative_path.into(),
                    reason: FreeText::new(format!("invalid exported output path: {error}")),
                }
            })?;
            return Err(TemplateExportPortError::MachinePathDetected { path });
        }
    }
    Ok(())
}

/// Number of bytes read at once while scanning an exported file.
pub(super) const MACHINE_PATH_SCAN_CHUNK_SIZE: usize = 8 * 1024;

/// Reports whether `path` contains the machine home directory at path boundaries.
///
/// The scanner keeps only one fixed-size chunk plus the home-path overlap, so
/// export validation cannot allocate in proportion to the size of an exported
/// file. Retaining a full home-path overlap also detects a path that spans two
/// chunks.
fn file_contains_machine_home_path(
    path: &Path,
    home_dir: &[u8],
) -> Result<bool, TemplateExportPortError> {
    let mut file = File::open(path).map_err(|error| io_error(path, &error))?;
    let mut chunk = [0_u8; MACHINE_PATH_SCAN_CHUNK_SIZE];
    let mut buffered = Vec::new();
    // When the overlap is trimmed, retain the byte that preceded it so a
    // candidate at offset zero keeps its original path-boundary context.
    let mut buffered_preceding_byte = None;

    loop {
        let read = file.read(&mut chunk).map_err(|error| io_error(path, &error))?;
        if read == 0 {
            return Ok(contains_machine_home_path(
                &buffered,
                home_dir,
                true,
                buffered_preceding_byte,
            ));
        }

        let read_bytes = chunk.get(..read).ok_or_else(|| {
            let error = std::io::Error::other("file reader returned an invalid byte count");
            io_error(path, &error)
        })?;
        buffered.extend_from_slice(read_bytes);
        if contains_machine_home_path(&buffered, home_dir, false, buffered_preceding_byte) {
            return Ok(true);
        }

        let retained = buffered.len().min(home_dir.len().saturating_add(1));
        let retained_start = buffered.len().saturating_sub(retained);
        if retained_start > 0 {
            buffered_preceding_byte = buffered.get(retained_start.saturating_sub(1)).copied();
            buffered.drain(..retained_start);
        }
    }
}

fn contains_machine_home_path(
    bytes: &[u8],
    home_dir: &[u8],
    at_end_of_file: bool,
    preceding_buffered_byte: Option<u8>,
) -> bool {
    if home_dir.is_empty() {
        return false;
    }

    bytes.windows(home_dir.len()).enumerate().any(|(index, window)| {
        let preceding_byte = index
            .checked_sub(1)
            .and_then(|preceding| bytes.get(preceding))
            .copied()
            .or(preceding_buffered_byte);
        window == home_dir
            && preceding_byte.is_none_or(is_machine_path_boundary)
            && match bytes.get(index + home_dir.len()) {
                Some(following) => is_machine_path_boundary(*following),
                None => at_end_of_file,
            }
    })
}

/// Returns whether a byte separates a path token from surrounding text.
fn is_machine_path_boundary(byte: u8) -> bool {
    matches!(
        byte,
        b'/' | b'\\'
            | b' '
            | b'\t'
            | b'\r'
            | b'\n'
            | b'\''
            | b'"'
            | b'`'
            | b'='
            | b':'
            | b','
            | b';'
            | b'('
            | b')'
            | b'['
            | b']'
            | b'{'
            | b'}'
            | b'<'
            | b'>'
    )
}
