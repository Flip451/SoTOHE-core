//! Shared symlink rejection utilities for infrastructure file I/O.
//!
//! All file loaders in the infrastructure layer should use these functions
//! to reject symlinks at any path component before reading or writing.

use std::path::Path;

/// Rejects symlinks at the leaf path and every ancestor up to (but not including) the root.
///
/// `trusted_root` is assumed safe (e.g., CLI composition root). Only components
/// below it are verified.
///
/// Returns `Ok(true)` if the leaf exists and is not a symlink,
/// `Ok(false)` if the leaf does not exist,
/// `Err` if any checked component is a symlink or cannot be inspected.
pub fn reject_symlinks_below(path: &Path, trusted_root: &Path) -> Result<bool, std::io::Error> {
    // Collect ancestors between path and trusted_root, then check from root toward leaf.
    // This ensures parent directories are always verified even when the leaf is absent.
    let mut components: Vec<&Path> = Vec::new();
    for ancestor in path.ancestors() {
        if ancestor == trusted_root || ancestor.as_os_str().is_empty() {
            break;
        }
        components.push(ancestor);
    }
    // Reverse so we walk root → leaf (parents first)
    components.reverse();

    for component in &components {
        // Skip the leaf — handled separately below for the exists/not-exists return
        if *component == path {
            continue;
        }
        match component.symlink_metadata() {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("refusing to follow symlink: {}", component.display()),
                ));
            }
            Ok(_) => {}
            // Parent doesn't exist yet (e.g., track dir not created) — that's OK
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(std::io::Error::new(
                    e.kind(),
                    format!("failed to stat {}: {e}", component.display()),
                ));
            }
        }
    }

    // Check the leaf itself
    match path.symlink_metadata() {
        Ok(meta) if meta.file_type().is_symlink() => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("refusing to follow symlink: {}", path.display()),
        )),
        Ok(_) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => {
            Err(std::io::Error::new(e.kind(), format!("failed to stat {}: {e}", path.display())))
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_regular_file_accepted() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("test.json");
        std::fs::write(&file, "{}").unwrap();

        assert!(reject_symlinks_below(&file, dir.path()).unwrap());
    }

    #[test]
    fn test_nonexistent_returns_false() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("missing.json");

        assert!(!reject_symlinks_below(&file, dir.path()).unwrap());
    }

    #[test]
    fn test_nested_regular_file_accepted() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("sub/dir");
        std::fs::create_dir_all(&nested).unwrap();
        let file = nested.join("test.json");
        std::fs::write(&file, "{}").unwrap();

        assert!(reject_symlinks_below(&file, dir.path()).unwrap());
    }

    #[cfg(unix)]
    #[test]
    fn test_symlink_leaf_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("real.json");
        let link = dir.path().join("link.json");
        std::fs::write(&target, "{}").unwrap();
        std::os::unix::fs::symlink(&target, &link).unwrap();

        let result = reject_symlinks_below(&link, dir.path());
        assert!(result.is_err());
    }

    #[cfg(unix)]
    #[test]
    fn test_symlink_parent_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let real_sub = dir.path().join("real-sub");
        std::fs::create_dir_all(&real_sub).unwrap();
        std::fs::write(real_sub.join("test.json"), "{}").unwrap();

        let link_sub = dir.path().join("link-sub");
        std::os::unix::fs::symlink(&real_sub, &link_sub).unwrap();

        let result = reject_symlinks_below(&link_sub.join("test.json"), dir.path());
        assert!(result.is_err());
    }

    #[cfg(unix)]
    #[test]
    fn test_symlink_grandparent_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real");
        std::fs::create_dir_all(real.join("deep")).unwrap();
        std::fs::write(real.join("deep/test.json"), "{}").unwrap();

        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();

        let result = reject_symlinks_below(&link.join("deep/test.json"), dir.path());
        assert!(result.is_err());
    }
}
