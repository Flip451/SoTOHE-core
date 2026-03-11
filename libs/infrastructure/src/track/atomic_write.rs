//! Atomic file write utility using tmp-in-same-dir + fsync + rename + parent fsync.

use std::fs;
use std::io::Write;
use std::path::Path;

/// Atomically writes content to a file using tmp-in-same-dir + fsync + rename + parent fsync.
///
/// The pattern:
/// 1. Create a temporary file in the same directory as the target
/// 2. Write content and fsync the file
/// 3. Rename (atomic on POSIX when same filesystem)
/// 4. Fsync the parent directory to persist the rename
///
/// # Errors
/// Returns `std::io::Error` on any I/O failure. Cleans up temp file on error.
pub fn atomic_write_file(path: &Path, content: &[u8]) -> std::io::Result<()> {
    let parent = path.parent().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "path has no parent directory")
    })?;

    // Create temp file in the same directory to ensure same-filesystem rename.
    let tmp_path = parent.join(format!(
        ".tmp-{}-{}",
        path.file_name().and_then(|n| n.to_str()).unwrap_or("file"),
        std::process::id()
    ));

    // Write + fsync, then rename. Clean up on any error.
    match write_and_rename(&tmp_path, path, parent, content) {
        Ok(()) => Ok(()),
        Err(e) => {
            // Best-effort cleanup of temp file.
            let _ = fs::remove_file(&tmp_path);
            Err(e)
        }
    }
}

fn write_and_rename(
    tmp_path: &Path,
    target_path: &Path,
    parent: &Path,
    content: &[u8],
) -> std::io::Result<()> {
    // Step 1: Write content to temp file.
    let mut file = fs::File::create(tmp_path)?;
    file.write_all(content)?;

    // Step 2: Fsync the file to ensure content is on disk.
    file.sync_all()?;
    drop(file);

    // Step 3: Atomic rename.
    fs::rename(tmp_path, target_path)?;

    // Step 4: Fsync parent directory to persist the directory entry.
    let dir = fs::File::open(parent)?;
    dir.sync_all()?;

    Ok(())
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_atomic_write_creates_file_with_correct_content() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.json");

        atomic_write_file(&path, b"hello world").unwrap();

        assert_eq!(fs::read_to_string(&path).unwrap(), "hello world");
    }

    #[test]
    fn test_atomic_write_overwrites_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.json");

        fs::write(&path, "old content").unwrap();
        atomic_write_file(&path, b"new content").unwrap();

        assert_eq!(fs::read_to_string(&path).unwrap(), "new content");
    }

    #[test]
    fn test_atomic_write_cleans_up_on_invalid_parent() {
        let result = atomic_write_file(Path::new("/nonexistent/dir/file.json"), b"data");
        assert!(result.is_err());
    }

    #[test]
    fn test_atomic_write_no_temp_file_left_on_success() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.json");

        atomic_write_file(&path, b"content").unwrap();

        // Only the target file should exist, no .tmp- files.
        let entries: Vec<_> = fs::read_dir(dir.path()).unwrap().filter_map(|e| e.ok()).collect();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].file_name(), "test.json");
    }
}
