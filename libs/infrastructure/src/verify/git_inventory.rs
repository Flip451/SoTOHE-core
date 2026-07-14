//! Git-index inventory helpers shared by repository-wide verification checks.

use std::ffi::OsString;
use std::io::{BufRead, BufReader};
use std::path::{Component, Path, PathBuf};
use std::process::Stdio;

use crate::git_cli::guarded_git_command;
use crate::track::symlink_guard::reject_symlinks_below;

/// The largest Git index inventory accepted by these repository-wide checks.
const MAX_TRACKED_INVENTORY_BYTES: usize = 16 * 1024 * 1024;
/// The largest number of tracked paths accepted by these repository-wide checks.
const MAX_TRACKED_FILE_COUNT: usize = 100_000;
/// The largest accepted NUL-delimited Git path, excluding its delimiter.
const MAX_TRACKED_PATH_BYTES: usize = 16 * 1024;

/// Lists every tracked repository file using the Git index.
///
/// The NUL-delimited form preserves file names containing whitespace and new
/// lines.  Invalid or non-workspace-relative paths are rejected rather than
/// being silently ignored.
///
/// # Errors
///
/// Returns an error when the repository's `.git` entry is symlinked, Git cannot
/// list the index, or the index contains an invalid repository-relative path.
pub(crate) fn tracked_files(project_root: &Path) -> Result<Vec<PathBuf>, String> {
    reject_symlinked_git_entry(project_root)?;

    let mut child = guarded_git_command()
        .args(["ls-files", "-z"])
        .current_dir(project_root)
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|error| format!("cannot list Git-tracked files: {error}"))?;
    let stdout =
        child.stdout.take().ok_or_else(|| "cannot read Git-tracked file inventory".to_owned())?;
    let mut stdout = BufReader::new(stdout);
    let files = parse_tracked_file_inventory(&mut stdout);
    drop(stdout);

    let status = child.wait().map_err(|error| format!("cannot list Git-tracked files: {error}"))?;
    let files = files?;

    if !status.success() {
        return Err("cannot list Git-tracked files".to_owned());
    }

    Ok(files)
}

/// Parses Git's NUL-delimited `ls-files` output with bounded memory use.
fn parse_tracked_file_inventory<R: BufRead>(reader: &mut R) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    let mut inventory_bytes = 0_usize;
    let mut path_bytes = Vec::new();

    while read_nul_delimited_path(reader, &mut path_bytes)? {
        inventory_bytes = inventory_bytes
            .checked_add(path_bytes.len().saturating_add(1))
            .ok_or_else(|| "Git-tracked file inventory exceeds the size limit".to_owned())?;
        if inventory_bytes > MAX_TRACKED_INVENTORY_BYTES {
            return Err("Git-tracked file inventory exceeds the size limit".to_owned());
        }
        if files.len() >= MAX_TRACKED_FILE_COUNT {
            return Err("Git-tracked file inventory exceeds the path-count limit".to_owned());
        }
        files.push(path_from_git_bytes(&path_bytes)?);
    }

    Ok(files)
}

/// Reads the next NUL-delimited Git path without allocating past its limit.
fn read_nul_delimited_path<R: BufRead>(reader: &mut R, path: &mut Vec<u8>) -> Result<bool, String> {
    path.clear();

    loop {
        let (consumed, completed) = {
            let available = reader
                .fill_buf()
                .map_err(|error| format!("cannot read Git-tracked file inventory: {error}"))?;
            if available.is_empty() {
                if path.is_empty() {
                    return Ok(false);
                }
                return Err("Git-tracked file inventory ended before a path delimiter".to_owned());
            }

            if let Some(delimiter) = available.iter().position(|byte| *byte == b'\0') {
                let path_bytes = available.get(..delimiter).ok_or_else(|| {
                    "cannot parse Git-tracked file inventory delimiter".to_owned()
                })?;
                append_path_bytes(path, path_bytes)?;
                (delimiter.saturating_add(1), true)
            } else {
                append_path_bytes(path, available)?;
                (available.len(), false)
            }
        };
        reader.consume(consumed);
        if completed {
            return Ok(true);
        }
    }
}

fn append_path_bytes(path: &mut Vec<u8>, bytes: &[u8]) -> Result<(), String> {
    let length = path
        .len()
        .checked_add(bytes.len())
        .ok_or_else(|| "Git-tracked file path exceeds the size limit".to_owned())?;
    if length > MAX_TRACKED_PATH_BYTES {
        return Err("Git-tracked file path exceeds the size limit".to_owned());
    }
    path.extend_from_slice(bytes);
    Ok(())
}

/// Returns a tracked path that is safe to read without following symlinks.
///
/// # Errors
///
/// Returns an error when the tracked file is absent, is symlinked, or has a
/// symlinked parent below `project_root`.
pub(crate) fn checked_tracked_file_path(
    project_root: &Path,
    relative_path: &Path,
) -> Result<PathBuf, String> {
    let path = project_root.join(relative_path);
    match reject_symlinks_below(&path, project_root) {
        Ok(true) => Ok(path),
        Ok(false) => Err(format!("tracked file does not exist: {}", relative_path.display())),
        Err(error) => {
            Err(format!("refusing to read tracked file {}: {error}", relative_path.display()))
        }
    }
}

fn reject_symlinked_git_entry(project_root: &Path) -> Result<(), String> {
    let git_dir = project_root.join(".git");
    match std::fs::symlink_metadata(&git_dir) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err("refusing to follow a symlinked .git entry".to_owned())
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("cannot inspect .git entry: {error}")),
    }
}

#[cfg(unix)]
fn path_from_git_bytes(bytes: &[u8]) -> Result<PathBuf, String> {
    use std::os::unix::ffi::OsStringExt as _;

    validate_workspace_relative_path(PathBuf::from(OsString::from_vec(bytes.to_vec())))
}

#[cfg(not(unix))]
fn path_from_git_bytes(bytes: &[u8]) -> Result<PathBuf, String> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| "Git returned a non-UTF-8 tracked-file path".to_owned())?;
    validate_workspace_relative_path(PathBuf::from(text))
}

fn validate_workspace_relative_path(path: PathBuf) -> Result<PathBuf, String> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(component, Component::ParentDir | Component::RootDir | Component::Prefix(_))
        })
    {
        return Err("Git returned an invalid repository-relative file path".to_owned());
    }
    Ok(path)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use std::io::Cursor;
    use std::path::Path;
    use std::process::Command;

    use tempfile::TempDir;

    use super::{
        MAX_TRACKED_FILE_COUNT, MAX_TRACKED_PATH_BYTES, parse_tracked_file_inventory, tracked_files,
    };

    fn run_git(root: &Path, args: &[&str]) {
        let output = Command::new("git").args(args).current_dir(root).output().unwrap();
        assert!(
            output.status.success(),
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn initialize_repository(project_root: &Path) {
        run_git(project_root, &["init", "--quiet", "--initial-branch=main"]);
    }

    #[cfg(unix)]
    #[test]
    fn test_tracked_files_symlinked_git_entry_returns_error() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path().join("project");
        let other_repository = temp_dir.path().join("other-repository");
        std::fs::create_dir_all(&project_root).unwrap();
        std::fs::create_dir_all(&other_repository).unwrap();
        initialize_repository(&project_root);
        initialize_repository(&other_repository);
        std::fs::write(project_root.join("README.md"), "fixture\n").unwrap();
        run_git(&project_root, &["add", "README.md"]);

        let git_dir = project_root.join(".git");
        std::fs::remove_dir_all(&git_dir).unwrap();
        std::os::unix::fs::symlink(other_repository.join(".git"), &git_dir).unwrap();

        assert!(tracked_files(&project_root).is_err());
    }

    #[test]
    fn test_parse_tracked_file_inventory_oversized_path_returns_error() {
        let mut output = vec![b'a'; MAX_TRACKED_PATH_BYTES.saturating_add(1)];
        output.push(b'\0');

        assert!(parse_tracked_file_inventory(&mut Cursor::new(output)).is_err());
    }

    #[test]
    fn test_parse_tracked_file_inventory_excessive_path_count_returns_error() {
        let mut output = Vec::with_capacity(MAX_TRACKED_FILE_COUNT.saturating_add(1) * 2);
        for _ in 0..=MAX_TRACKED_FILE_COUNT {
            output.extend_from_slice(b"a\0");
        }

        assert!(parse_tracked_file_inventory(&mut Cursor::new(output)).is_err());
    }
}
