//! Capability-exec-specific path validation.
//!
//! Capability definitions and sources use this component walk instead of the
//! shared track guard because their trusted root may be reached through a
//! symlink, while a symlink below that root must still be rejected even when a
//! following `..` would otherwise hide it.

use std::path::{Component, Path, PathBuf};

/// Lexically normalizes a path without resolving or inspecting filesystem components.
pub(crate) fn lexically_normalize(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();

    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(Path::new("/")),
            Component::CurDir => {}
            Component::ParentDir => match normalized.components().next_back() {
                Some(Component::Normal(_)) => {
                    let _ = normalized.pop();
                }
                Some(Component::RootDir | Component::Prefix(_)) => {}
                Some(Component::ParentDir) | Some(Component::CurDir) | None => {
                    normalized.push(component.as_os_str());
                }
            },
            Component::Normal(part) => normalized.push(part),
        }
    }

    if normalized.as_os_str().is_empty() && !path.is_absolute() {
        PathBuf::from(".")
    } else {
        normalized
    }
}

/// Normalizes `path` after rejecting each symlinked component below `trusted_root`.
///
/// A parent component is processed only after its preceding component has been
/// inspected. This prevents `symlink/..` from concealing a path traversal. The
/// trusted root itself may be a symlink, which supports workspaces reached
/// through a symlinked checkout path.
pub(crate) fn normalize_path_rejecting_symlinked_components(
    path: &Path,
    trusted_root: &Path,
) -> Result<PathBuf, std::io::Error> {
    let trusted_root = lexically_normalize(trusted_root);
    let mut normalized = PathBuf::new();

    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(Path::new("/")),
            Component::CurDir => {}
            Component::ParentDir => {
                reject_symlinked_component(&normalized, &trusted_root)?;
                match normalized.components().next_back() {
                    Some(Component::Normal(_)) => {
                        let _ = normalized.pop();
                    }
                    Some(Component::RootDir | Component::Prefix(_)) => {}
                    Some(Component::ParentDir) | Some(Component::CurDir) | None => {
                        normalized.push(component.as_os_str());
                    }
                }
            }
            Component::Normal(part) => {
                normalized.push(part);
                reject_symlinked_component(&normalized, &trusted_root)?;
            }
        }
    }

    if normalized.as_os_str().is_empty() && !path.is_absolute() {
        Ok(PathBuf::from("."))
    } else {
        Ok(normalized)
    }
}

fn reject_symlinked_component(component: &Path, trusted_root: &Path) -> Result<(), std::io::Error> {
    if component.as_os_str().is_empty() || component == trusted_root {
        return Ok(());
    }

    match component.symlink_metadata() {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("refusing to follow symlink: {}", component.display()),
        )),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}
