//! Shared symlink rejection utilities for infrastructure file I/O.
//!
//! All file loaders in the infrastructure layer should use these functions
//! to reject symlinks at any path component before reading or writing.

use std::path::Path;

/// Rejects symlinks at the given path AND every ancestor above it, walking up to
/// the filesystem root (or to the empty tail for a relative path).
///
/// Unlike [`reject_symlinks_below`], this helper has no trusted root: a symlink at
/// any ancestor would silently redirect the anchor to an untrusted location, so
/// every component from the leaf to the top of the path must be verified. Missing
/// components (leaf or ancestor) are treated as OK so a downstream save may still
/// create the directory. Intended for validating a "trusted anchor" directory
/// (e.g., a track-items root) before it is used as the base of subsequent
/// [`reject_symlinks_below`] checks.
///
/// Returns `Ok(())` when no component is a symlink,
/// `Err` if any component is a symlink or cannot be inspected.
pub(crate) fn reject_symlinks_up_to_root(path: &Path) -> Result<(), std::io::Error> {
    for ancestor in path.ancestors() {
        if ancestor.as_os_str().is_empty() {
            break;
        }
        match ancestor.symlink_metadata() {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(symlink_rejection(ancestor));
            }
            Ok(_) => {}
            // Missing components are OK — the caller may create them later, and a
            // still-unmaterialised ancestor cannot yet be a symlink.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(std::io::Error::new(
                    e.kind(),
                    format!("failed to stat {}: {e}", ancestor.display()),
                ));
            }
        }
    }
    Ok(())
}

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
                return Err(symlink_rejection(component));
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
        Ok(meta) if meta.file_type().is_symlink() => Err(symlink_rejection(path)),
        Ok(_) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => {
            Err(std::io::Error::new(e.kind(), format!("failed to stat {}: {e}", path.display())))
        }
    }
}

/// This module's refusal to follow a symlink, carried inside the
/// [`std::io::Error`] it raises.
///
/// A private type rather than a message or an error kind, because both of those
/// can come from something other than this module: `InvalidInput` is what the
/// filesystem returns for a malformed path, and an inspection failure renders
/// the inspected path into its own message, so a caller matching on either
/// could be handed a path that impersonates a refusal. Only this module can
/// construct this type, so a downcast answers the question exactly.
#[derive(Debug)]
struct SymlinkRejected {
    component: std::path::PathBuf,
}

impl std::fmt::Display for SymlinkRejected {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "refusing to follow symlink: {}", self.component.display())
    }
}

impl std::error::Error for SymlinkRejected {}

/// Reports whether `error` is this module's symlink refusal.
///
/// The downcast is the discrimination this module's payload exists for: neither
/// [`std::io::ErrorKind::InvalidInput`] nor the rendered message identifies a
/// refusal on its own, since the filesystem raises that kind for any malformed
/// path and an inspection failure renders the inspected path into its own text.
/// A caller that must tell a refusal apart — to report why without repeating the
/// path — asks here.
pub(crate) fn is_symlink_rejection(error: &std::io::Error) -> bool {
    error.get_ref().is_some_and(|inner| inner.is::<SymlinkRejected>())
}

/// Builds this module's rejection for the symlink at `component`.
fn symlink_rejection(component: &Path) -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        SymlinkRejected { component: component.to_path_buf() },
    )
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

    #[test]
    fn test_up_to_root_regular_leaf_accepted() {
        let dir = tempfile::tempdir().unwrap();
        let leaf = dir.path().join("real-leaf");
        std::fs::create_dir_all(&leaf).unwrap();

        assert!(reject_symlinks_up_to_root(&leaf).is_ok());
    }

    #[test]
    fn test_up_to_root_missing_leaf_accepted() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("does-not-exist");

        assert!(reject_symlinks_up_to_root(&missing).is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn test_up_to_root_symlinked_leaf_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real");
        let link = dir.path().join("link");
        std::fs::create_dir_all(&real).unwrap();
        std::os::unix::fs::symlink(&real, &link).unwrap();

        let err = reject_symlinks_up_to_root(&link).unwrap_err();
        assert!(err.to_string().contains("refusing to follow symlink"));
    }

    #[cfg(unix)]
    #[test]
    fn test_up_to_root_symlinked_parent_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let real_parent = dir.path().join("real-parent");
        std::fs::create_dir_all(real_parent.join("child")).unwrap();
        let link_parent = dir.path().join("link-parent");
        std::os::unix::fs::symlink(&real_parent, &link_parent).unwrap();

        // Leaf resolves to a real dir via the symlinked parent, but the ancestor
        // walk still detects the parent-level symlink.
        let leaf = link_parent.join("child");
        let err = reject_symlinks_up_to_root(&leaf).unwrap_err();
        assert!(err.to_string().contains("refusing to follow symlink"));
    }
}
