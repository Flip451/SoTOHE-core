//! Environment and Cargo-configuration inputs for rustdoc cache identity.

use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::io::{Error, ErrorKind};
use std::path::{Component, Path, PathBuf};

use super::{
    MAX_RUSTDOC_INPUT_FILE_BYTES, MAX_RUSTDOC_INPUT_PATH_BYTES, RustdocInputFingerprintError,
    check_file_size, io_error,
};

const RUSTDOC_ENVIRONMENT_INPUTS: &[&str] = &[
    "CARGO_BUILD_TARGET",
    "CARGO_ENCODED_RUSTFLAGS",
    "CARGO_HOME",
    "CARGO_TARGET_DIR",
    "CARGO_NET_OFFLINE",
    "PATH",
    "RUSTC",
    "RUSTC_WRAPPER",
    "RUSTC_WORKSPACE_WRAPPER",
    "RUSTDOC",
    "RUSTDOCFLAGS",
    "RUSTFLAGS",
    "RUSTUP_HOME",
    "RUSTUP_TOOLCHAIN",
];

const TOOL_ENVIRONMENT_INPUTS: &[&str] =
    &["RUSTC", "RUSTDOC", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER"];
const MAX_RUSTDOC_ENVIRONMENT_VALUE_BYTES: usize = 64 * 1024;

/// Appends the complete, bounded environment/configuration identity.
pub(super) fn append_environment_identity(
    canonical: &mut Vec<u8>,
    workspace_root: &Path,
) -> Result<(), RustdocInputFingerprintError> {
    let trusted_root =
        workspace_root.canonicalize().map_err(|error| io_error(workspace_root, error))?;
    for name in RUSTDOC_ENVIRONMENT_INPUTS {
        super::append_len_prefixed_bytes(canonical, name.as_bytes());
        let Some(value) = std::env::var_os(name) else {
            canonical.push(0);
            continue;
        };
        let value_bytes = super::os_bytes(&value);
        if value_bytes.len() > MAX_RUSTDOC_ENVIRONMENT_VALUE_BYTES {
            return Err(RustdocInputFingerprintError::EnvironmentBytes {
                name: (*name).to_owned(),
                bytes: value_bytes.len(),
                maximum: MAX_RUSTDOC_ENVIRONMENT_VALUE_BYTES,
            });
        }
        canonical.push(1);
        super::append_len_prefixed_bytes(canonical, &value_bytes);
        if TOOL_ENVIRONMENT_INPUTS.contains(name) {
            let resolved = resolve_tool_path(workspace_root, &trusted_root, name, &value)?;
            super::append_len_prefixed_bytes(canonical, &super::path_bytes(&resolved.path));
            super::append_len_prefixed_bytes(canonical, &super::sha256_bytes(&resolved.bytes));
        }
    }
    append_actual_rustdoc_tool_identity(canonical, workspace_root)?;
    append_cargo_config_hierarchy(canonical, workspace_root)?;
    Ok(())
}

struct ResolvedTool {
    path: PathBuf,
    bytes: Vec<u8>,
}

fn resolve_tool_path(
    workspace_root: &Path,
    trusted_root: &Path,
    name: &str,
    value: &OsStr,
) -> Result<ResolvedTool, RustdocInputFingerprintError> {
    if value.is_empty() {
        return Err(io_error(
            Path::new(name),
            Error::new(ErrorKind::InvalidInput, "tool path is empty"),
        ));
    }
    let value_path = PathBuf::from(value);
    if is_bare_command(&value_path) {
        let candidate = resolve_bare_tool(workspace_root, name, value)?;
        let resolved = candidate.canonicalize().map_err(|error| io_error(&candidate, error))?;
        return snapshot_tool_file(&resolved, None, name);
    }
    if value_path.is_absolute() {
        let resolved = value_path.canonicalize().map_err(|error| io_error(&value_path, error))?;
        return snapshot_tool_file(&resolved, None, name);
    }
    let candidate = workspace_root.join(value_path);
    let resolved = candidate.canonicalize().map_err(|error| io_error(&candidate, error))?;
    if !resolved.starts_with(trusted_root) {
        return Err(io_error(
            &resolved,
            Error::other(format!("{name} resolves outside the trusted workspace")),
        ));
    }
    snapshot_tool_file(&resolved, Some(trusted_root), name)
}

fn append_actual_rustdoc_tool_identity(
    canonical: &mut Vec<u8>,
    workspace_root: &Path,
) -> Result<(), RustdocInputFingerprintError> {
    for tool in ["cargo", "rustc", "rustdoc"] {
        let resolved = resolve_path_command(workspace_root, tool)?;
        append_tool_snapshot(canonical, &format!("actual-{tool}"), &resolved);
    }
    Ok(())
}

fn resolve_path_command(
    workspace_root: &Path,
    name: &str,
) -> Result<ResolvedTool, RustdocInputFingerprintError> {
    let candidate = resolve_bare_tool(workspace_root, name, OsStr::new(name))?;
    let resolved = candidate.canonicalize().map_err(|error| io_error(&candidate, error))?;
    snapshot_tool_file(&resolved, None, name)
}

fn snapshot_tool_file(
    path: &Path,
    trusted_root: Option<&Path>,
    label: &str,
) -> Result<ResolvedTool, RustdocInputFingerprintError> {
    let resolved = path.canonicalize().map_err(|error| io_error(path, error))?;
    if let Some(trusted_root) = trusted_root {
        if !resolved.starts_with(trusted_root) {
            return Err(io_error(
                &resolved,
                Error::other(format!("{label} resolves outside the trusted workspace")),
            ));
        }
    }
    let path_length = super::path_bytes(&resolved).len();
    if path_length > MAX_RUSTDOC_INPUT_PATH_BYTES {
        return Err(RustdocInputFingerprintError::PathBytes {
            path: resolved,
            bytes: path_length,
            maximum: MAX_RUSTDOC_INPUT_PATH_BYTES,
        });
    }
    let metadata =
        std::fs::symlink_metadata(&resolved).map_err(|error| io_error(&resolved, error))?;
    if !metadata.is_file() {
        return Err(io_error(
            &resolved,
            Error::new(ErrorKind::InvalidInput, format!("{label} is not a regular file")),
        ));
    }
    check_file_size(&resolved, metadata.len())?;
    let bytes = crate::tddd::tddd_catalogue_document_loader::read_optional_regular_file_bytes(
        &resolved,
        trusted_root,
        MAX_RUSTDOC_INPUT_FILE_BYTES,
    )
    .map_err(|error| io_error(&resolved, error))?
    .ok_or_else(|| io_error(&resolved, Error::new(ErrorKind::NotFound, "tool disappeared")))?;
    let after = std::fs::symlink_metadata(&resolved).map_err(|error| io_error(&resolved, error))?;
    if super::metadata_generation(&metadata) != super::metadata_generation(&after)
        || after.len() != bytes.len() as u64
    {
        return Err(io_error(
            &resolved,
            Error::other(format!("{label} changed while it was being fingerprinted")),
        ));
    }
    Ok(ResolvedTool { path: resolved, bytes })
}

fn append_tool_snapshot(canonical: &mut Vec<u8>, label: &str, tool: &ResolvedTool) {
    super::append_len_prefixed_bytes(canonical, label.as_bytes());
    super::append_len_prefixed_bytes(canonical, &super::path_bytes(&tool.path));
    super::append_len_prefixed_bytes(canonical, &super::sha256_bytes(&tool.bytes));
}

fn is_bare_command(path: &Path) -> bool {
    path.components().count() == 1 && matches!(path.components().next(), Some(Component::Normal(_)))
}

fn resolve_bare_tool(
    workspace_root: &Path,
    name: &str,
    value: &OsStr,
) -> Result<PathBuf, RustdocInputFingerprintError> {
    let path = std::env::var_os("PATH").ok_or_else(|| {
        io_error(
            Path::new(name),
            Error::new(ErrorKind::NotFound, "PATH is unavailable for bare tool path"),
        )
    })?;
    let mut found = None;
    for directory in std::env::split_paths(&path) {
        let directory = if directory.as_os_str().is_empty() {
            workspace_root.to_path_buf()
        } else if directory.is_absolute() {
            directory
        } else {
            workspace_root.join(directory)
        };
        let candidate = directory.join(value);
        // PATH entries commonly point at rustup shims through a symlink. The
        // caller canonicalizes the selected candidate before taking its
        // bounded snapshot, so following the PATH entry here does not bypass
        // the final regular-file and generation checks.
        match std::fs::metadata(&candidate) {
            Ok(metadata) if metadata.is_file() => {
                found = Some(candidate);
                break;
            }
            Ok(_) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(io_error(&candidate, error)),
        }
    }
    found.ok_or_else(|| {
        io_error(
            Path::new(name),
            Error::new(ErrorKind::NotFound, "bare tool path was not found on PATH"),
        )
    })
}

fn append_cargo_config_hierarchy(
    canonical: &mut Vec<u8>,
    workspace_root: &Path,
) -> Result<(), RustdocInputFingerprintError> {
    let home = resolve_optional_environment_path(workspace_root, "HOME")?;
    let cargo_home = if let Some(value) = std::env::var_os("CARGO_HOME") {
        resolve_environment_path(workspace_root, "CARGO_HOME", &value)?
    } else if let Some(home) = home.as_ref() {
        home.join(".cargo")
    } else {
        return Err(io_error(
            Path::new("CARGO_HOME"),
            Error::other("CARGO_HOME cannot be resolved without HOME"),
        ));
    };
    append_resolved_path(canonical, "resolved-home", home.as_deref())?;
    append_resolved_path(canonical, "resolved-cargo-home", Some(&cargo_home))?;

    let mut config_paths = BTreeSet::new();
    let mut directory = workspace_root.to_path_buf();
    let mut depth = 0_usize;
    loop {
        if depth >= super::MAX_RUSTDOC_INPUT_DEPTH {
            return Err(RustdocInputFingerprintError::DirectoryDepth {
                path: directory,
                maximum: super::MAX_RUSTDOC_INPUT_DEPTH,
            });
        }
        let cargo_directory = directory.join(".cargo");
        match std::fs::symlink_metadata(&cargo_directory) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(RustdocInputFingerprintError::Symlink { path: cargo_directory });
            }
            Ok(metadata) if !metadata.is_dir() => {
                return Err(io_error(
                    &cargo_directory,
                    Error::new(ErrorKind::InvalidInput, "Cargo config parent is not a directory"),
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(io_error(&cargo_directory, error)),
        }
        for name in ["config.toml", "config"] {
            config_paths.insert(cargo_directory.join(name));
        }
        let Some(parent) = directory.parent() else {
            break;
        };
        if parent == directory {
            break;
        }
        directory = parent.to_path_buf();
        depth += 1;
    }
    for name in ["config.toml", "config"] {
        config_paths.insert(cargo_home.join(name));
    }
    for path in config_paths {
        append_config_file_snapshot(canonical, &path)?;
    }
    Ok(())
}

fn resolve_optional_environment_path(
    workspace_root: &Path,
    name: &str,
) -> Result<Option<PathBuf>, RustdocInputFingerprintError> {
    std::env::var_os(name)
        .map(|value| resolve_environment_path(workspace_root, name, &value))
        .transpose()
}

fn resolve_environment_path(
    workspace_root: &Path,
    name: &str,
    value: &OsStr,
) -> Result<PathBuf, RustdocInputFingerprintError> {
    let raw = PathBuf::from(value);
    let absolute = if raw.is_absolute() { raw } else { workspace_root.join(raw) };
    let normalized = crate::verify::path_safety::lexical_normalize(&absolute);
    if super::path_bytes(&normalized).len() > MAX_RUSTDOC_INPUT_PATH_BYTES {
        return Err(RustdocInputFingerprintError::PathBytes {
            path: normalized,
            bytes: super::path_bytes(&absolute).len(),
            maximum: MAX_RUSTDOC_INPUT_PATH_BYTES,
        });
    }
    crate::track::symlink_guard::reject_symlinks_up_to_root(&normalized)
        .map_err(|error| io_error(&normalized, error))?;
    match normalized.canonicalize() {
        Ok(path) => Ok(path),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(normalized),
        Err(error) => Err(io_error(Path::new(name), error)),
    }
}

fn append_resolved_path(
    canonical: &mut Vec<u8>,
    label: &str,
    path: Option<&Path>,
) -> Result<(), RustdocInputFingerprintError> {
    super::append_len_prefixed_bytes(canonical, label.as_bytes());
    match path {
        Some(path) => {
            let bytes = super::path_bytes(path);
            if bytes.len() > MAX_RUSTDOC_INPUT_PATH_BYTES {
                return Err(RustdocInputFingerprintError::PathBytes {
                    path: path.to_path_buf(),
                    bytes: bytes.len(),
                    maximum: MAX_RUSTDOC_INPUT_PATH_BYTES,
                });
            }
            canonical.push(1);
            super::append_len_prefixed_bytes(canonical, &bytes);
        }
        None => canonical.push(0),
    }
    Ok(())
}

fn append_config_file_snapshot(
    canonical: &mut Vec<u8>,
    path: &Path,
) -> Result<(), RustdocInputFingerprintError> {
    let bytes_path = super::path_bytes(path);
    if bytes_path.len() > MAX_RUSTDOC_INPUT_PATH_BYTES {
        return Err(RustdocInputFingerprintError::PathBytes {
            path: path.to_path_buf(),
            bytes: bytes_path.len(),
            maximum: MAX_RUSTDOC_INPUT_PATH_BYTES,
        });
    }
    super::append_len_prefixed_bytes(canonical, &bytes_path);
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            canonical.push(0);
            return Ok(());
        }
        Err(error) => return Err(io_error(path, error)),
    };
    if metadata.file_type().is_symlink() {
        return Err(RustdocInputFingerprintError::Symlink { path: path.to_path_buf() });
    }
    if !metadata.is_file() {
        return Err(io_error(
            path,
            Error::new(ErrorKind::InvalidInput, "Cargo config is not a regular file"),
        ));
    }
    check_file_size(path, metadata.len())?;
    crate::track::symlink_guard::reject_symlinks_up_to_root(path)
        .map_err(|error| io_error(path, error))?;
    let bytes = crate::tddd::tddd_catalogue_document_loader::read_optional_regular_file_bytes(
        path,
        None,
        MAX_RUSTDOC_INPUT_FILE_BYTES,
    )
    .map_err(|error| io_error(path, error))?
    .ok_or_else(|| io_error(path, Error::new(ErrorKind::NotFound, "Cargo config disappeared")))?;
    let after = std::fs::symlink_metadata(path).map_err(|error| io_error(path, error))?;
    if super::metadata_generation(&metadata) != super::metadata_generation(&after)
        || after.len() != bytes.len() as u64
    {
        return Err(io_error(path, Error::other("Cargo config changed while fingerprinting")));
    }
    canonical.push(1);
    super::append_len_prefixed_bytes(canonical, &super::sha256_bytes(&bytes));
    Ok(())
}
