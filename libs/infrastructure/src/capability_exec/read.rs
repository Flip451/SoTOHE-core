//! Bounded, symlink-safe reads of repository-authored capability definitions.

use std::fs::File;
use std::io::{Error, ErrorKind, Read};
use std::path::Path;

use super::{MAX_CAPABILITY_EXEC_TEXT_BYTES, path_guard};

pub(crate) fn bounded_read_utf8_file(path: &Path) -> Result<String, std::io::Error> {
    let metadata = path.symlink_metadata()?;
    if !metadata.file_type().is_file() {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            format!("{} is not a regular file", path.display()),
        ));
    }
    let file = File::open(path)?;
    let opened_metadata = file.metadata()?;
    if !opened_metadata.is_file() {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            format!("{} is not a regular file", path.display()),
        ));
    }
    if opened_metadata.len() > MAX_CAPABILITY_EXEC_TEXT_BYTES {
        return Err(Error::new(
            ErrorKind::InvalidData,
            format!(
                "{} exceeds the maximum allowed size of {MAX_CAPABILITY_EXEC_TEXT_BYTES} bytes",
                path.display()
            ),
        ));
    }

    // The metadata check is only a snapshot. Read one byte past the limit as well so a file
    // that grows between inspection and reading is rejected without an unbounded allocation.
    let mut reader = file.take(MAX_CAPABILITY_EXEC_TEXT_BYTES.saturating_add(1));
    let mut content = String::new();
    reader.read_to_string(&mut content)?;
    if content.len() > MAX_CAPABILITY_EXEC_TEXT_BYTES as usize {
        return Err(Error::new(
            ErrorKind::InvalidData,
            format!(
                "{} exceeds the maximum allowed size of {MAX_CAPABILITY_EXEC_TEXT_BYTES} bytes",
                path.display()
            ),
        ));
    }
    Ok(content)
}

pub(crate) fn read_utf8_file(path: &Path, repo_root: &Path) -> Result<String, String> {
    let normalized_root = path_guard::lexically_normalize(repo_root);
    let normalized_path = path_guard::normalize_path_rejecting_symlinked_components(
        path, repo_root,
    )
    .map_err(|error| format!("refusing to follow symlink at {}: {error}", path.display()))?;
    if !normalized_path.starts_with(&normalized_root) {
        return Err(format!(
            "path {} escapes repository root {}",
            path.display(),
            normalized_root.display()
        ));
    }
    let canonical_root = normalized_root.canonicalize().map_err(|error| {
        format!("cannot canonicalize repository root {}: {error}", repo_root.display())
    })?;
    let canonical_path = normalized_path
        .canonicalize()
        .map_err(|error| format!("cannot canonicalize {}: {error}", path.display()))?;
    if !canonical_path.starts_with(&canonical_root) {
        return Err(format!(
            "path {} escapes repository root {}",
            path.display(),
            canonical_root.display()
        ));
    }
    bounded_read_utf8_file(&normalized_path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))
}
