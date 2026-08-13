//! Small, shared guarded reads for files stored beneath a trusted root.

#[cfg(target_os = "linux")]
use std::io::Read as _;
use std::path::{Path, PathBuf};

use crate::lexical_path::lexical_normalize;
use crate::track::symlink_guard::reject_symlinks_below;

/// Relationship between a possibly absent record and its trusted root.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum RecordLocation {
    /// The record's existing parent is inside the trusted root.
    Inside,
    /// No parent exists, but the requested location is still inside the root.
    NoParent,
    /// The record is outside the trusted root.
    Outside,
}

/// Determines whether `path` can be treated as a record under `trusted_root`.
pub(crate) fn locate_record(path: &Path, trusted_root: &Path) -> std::io::Result<RecordLocation> {
    if !path.is_absolute() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "trusted record path must be absolute",
        ));
    }
    let canonical_root = trusted_root.canonicalize()?;
    let Some(parent) = path.parent() else {
        return Ok(RecordLocation::Outside);
    };
    let canonical_parent = match parent.canonicalize() {
        Ok(parent) => parent,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let absolute_parent = if parent.is_absolute() {
                parent.to_path_buf()
            } else {
                canonical_root.join(parent)
            };
            let Some(existing) = nearest_existing_ancestor(&absolute_parent)? else {
                return Ok(RecordLocation::Outside);
            };
            if !existing.starts_with(&canonical_root) {
                return Ok(RecordLocation::Outside);
            }
            reject_symlinks_below(&absolute_parent, &canonical_root)?;
            return Ok(if lexical_normalize(&absolute_parent).starts_with(&canonical_root) {
                RecordLocation::NoParent
            } else {
                RecordLocation::Outside
            });
        }
        Err(error) => return Err(error),
    };
    Ok(if canonical_parent.starts_with(&canonical_root) {
        RecordLocation::Inside
    } else {
        RecordLocation::Outside
    })
}

/// Reads a small regular UTF-8 file without allocating more than `max_bytes`.
///
/// An absent file returns `Ok(None)`. On supported platforms the file is opened
/// no-follow and nonblocking before its *opened-handle* metadata is checked, so
/// a concurrent path replacement cannot turn a validated record into a symlink
/// or FIFO. The descriptor read is capped again to handle a file that grows.
#[cfg(target_os = "linux")]
pub(crate) fn read_bounded_regular_file(
    path: &Path,
    trusted_root: &Path,
    max_bytes: u64,
) -> std::io::Result<Option<String>> {
    use std::os::unix::fs::OpenOptionsExt as _;

    let file = match std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "not a regular file"));
    }
    if metadata.len() > max_bytes {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "larger than the configured bound",
        ));
    }
    verify_opened_file_within_root(&file, trusted_root)?;
    let mut content = String::new();
    file.take(max_bytes.saturating_add(1)).read_to_string(&mut content)?;
    if content.len() as u64 > max_bytes {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "larger than the configured bound",
        ));
    }
    Ok(Some(content))
}

/// Safe descriptor-pinned opening is mandatory for this helper. Platforms
/// without the Unix no-follow/nonblocking flags fail closed rather than falling
/// back to a path-based check-then-open sequence.
#[cfg(not(target_os = "linux"))]
pub(crate) fn read_bounded_regular_file(
    _path: &Path,
    _trusted_root: &Path,
    _max_bytes: u64,
) -> std::io::Result<Option<String>> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "bounded trusted-file reads require Linux descriptor containment verification",
    ))
}

/// Confirms that the descriptor opened for a trusted read still belongs below
/// the intended root, closing the race where an intermediate directory is
/// replaced after path validation but before `open`.
#[cfg(target_os = "linux")]
pub(crate) fn verify_opened_file_within_root(
    file: &std::fs::File,
    trusted_root: &Path,
) -> std::io::Result<()> {
    use std::os::unix::io::AsRawFd as _;

    let opened_path = std::fs::read_link(format!("/proc/self/fd/{}", file.as_raw_fd()))?;
    let canonical_root = trusted_root.canonicalize()?;
    if opened_path.starts_with(&canonical_root) {
        Ok(())
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "opened file resolves outside the trusted root",
        ))
    }
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn verify_opened_file_within_root(
    _file: &std::fs::File,
    _trusted_root: &Path,
) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "opened-file containment verification is unavailable on this platform",
    ))
}

fn nearest_existing_ancestor(path: &Path) -> std::io::Result<Option<PathBuf>> {
    for ancestor in path.ancestors() {
        match ancestor.canonicalize() {
            Ok(canonical) => return Ok(Some(canonical)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Ok(None)
}

#[cfg(all(test, target_os = "linux"))]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::read_bounded_regular_file;

    #[test]
    fn test_read_bounded_regular_file_rejects_a_directory() {
        let directory = tempfile::tempdir().unwrap();

        let error = read_bounded_regular_file(directory.path(), directory.path(), 256).unwrap_err();

        assert!(error.to_string().contains("not a regular file"));
    }

    #[test]
    fn test_read_bounded_regular_file_rejects_a_parent_symlink_outside_the_root() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("record"), "outside").unwrap();
        std::os::unix::fs::symlink(outside.path(), root.path().join("swapped-parent")).unwrap();

        let error =
            read_bounded_regular_file(&root.path().join("swapped-parent/record"), root.path(), 256)
                .unwrap_err();

        assert!(error.to_string().contains("outside the trusted root"));
    }
}
