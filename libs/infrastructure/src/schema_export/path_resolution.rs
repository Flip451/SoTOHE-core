//! Target-directory and path-safety helpers for the schema export infrastructure adapter.
//!
//! Functions here resolve the Cargo target directory (respecting `CARGO_TARGET_DIR` and
//! workspace config), guard all resolved paths against escape outside the workspace root,
//! and reject any symlinks beneath the trusted root — preventing path-traversal attacks
//! where a crafted `CARGO_TARGET_DIR` or symlink redirects rustdoc JSON output to an
//! arbitrary location on disk.

use std::path::{Path, PathBuf};
use std::process::Command;

use domain::schema::SchemaExportError;

/// Resolves the Cargo target directory, respecting `CARGO_TARGET_DIR` and workspace config.
pub(super) fn resolve_target_dir(workspace_root: &Path) -> Result<PathBuf, SchemaExportError> {
    if let Ok(dir) = std::env::var("CARGO_TARGET_DIR") {
        let path = PathBuf::from(dir);
        return resolve_configured_target_dir(workspace_root, path, "CARGO_TARGET_DIR");
    }
    let output = Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .current_dir(workspace_root)
        .output()
        .map_err(|e| SchemaExportError::RustdocFailed(format!("cargo metadata failed: {e}")))?;

    if !output.status.success() {
        return Ok(workspace_root.join("target"));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if let Ok(meta) = serde_json::from_str::<serde_json::Value>(&stdout) {
        if let Some(dir) = meta.get("target_directory").and_then(|v| v.as_str()) {
            return resolve_configured_target_dir(
                workspace_root,
                PathBuf::from(dir),
                "cargo metadata target_directory",
            );
        }
    }

    Ok(workspace_root.join("target"))
}

pub(super) fn resolve_configured_target_dir(
    workspace_root: &Path,
    configured_dir: PathBuf,
    source: &str,
) -> Result<PathBuf, SchemaExportError> {
    let allow_outside_workspace = source == "CARGO_TARGET_DIR" && configured_dir.is_absolute();
    let target_dir = if configured_dir.is_relative() {
        workspace_root.join(configured_dir)
    } else {
        configured_dir
    };
    ensure_target_dir_within_workspace(workspace_root, &target_dir, source, allow_outside_workspace)
}

/// Validate a resolved Cargo target directory.
///
/// The `allow_outside_workspace` flag is `true` only for an explicit absolute
/// `CARGO_TARGET_DIR` (e.g., `/cargo-target` in CI containers — see the
/// Dockerfile's `IMAGE_CARGO_TARGET_DIR`). Cargo itself accepts arbitrary
/// `--target-dir` locations, and rejecting the raw environment configuration
/// here would make the new TDDD-enabled CLI crates unusable in supported CI
/// configurations.
///
/// Behavior matrix:
/// - **In-workspace target dirs** (the default `<workspace>/target`, or a relative
///   `CARGO_TARGET_DIR` like `target-w1`) go through the full symlink guard
///   relative to `trusted_root`. This catches silent tamper attempts where an
///   in-workspace symlink would redirect rustdoc JSON output.
/// - **Relative paths that escape the workspace** (e.g., `CARGO_TARGET_DIR=../outside`)
///   are rejected: a relative escape is a path-traversal attack pattern, not a
///   legitimate CI configuration.
/// - **Absolute paths outside the workspace** are honored when explicitly
///   configured. As a minimal defensive measure, the target directory's leaf
///   is still rejected if it is itself a symlink.
fn ensure_target_dir_within_workspace(
    workspace_root: &Path,
    target_dir: &Path,
    source: &str,
    allow_outside_workspace: bool,
) -> Result<PathBuf, SchemaExportError> {
    let trusted_root = checked_workspace_root(workspace_root)?;
    let target_abs = absolutize_for_target_guard(target_dir)?;
    let normalized_target = crate::verify::path_safety::lexical_normalize(&target_abs);

    if normalized_target.starts_with(&trusted_root) {
        reject_symlinks_for_rustdoc_path(&normalized_target, &trusted_root, source)?;
        Ok(normalized_target)
    } else if allow_outside_workspace {
        if let Ok(meta) = normalized_target.symlink_metadata() {
            if meta.file_type().is_symlink() {
                return Err(SchemaExportError::RustdocFailed(format!(
                    "{source} target directory is a symlink (rejected for tamper-resistance): {}",
                    normalized_target.display()
                )));
            }
        }
        Ok(normalized_target)
    } else {
        Err(SchemaExportError::RustdocFailed(format!(
            "{source} resolves target directory outside workspace root: {} (workspace root: {})",
            target_dir.display(),
            workspace_root.display()
        )))
    }
}

pub(super) fn checked_workspace_root(workspace_root: &Path) -> Result<PathBuf, SchemaExportError> {
    let workspace_abs = absolutize_for_target_guard(workspace_root)?;
    let normalized_workspace = crate::verify::path_safety::lexical_normalize(&workspace_abs);
    crate::verify::trusted_root::ensure_not_symlink_root(normalized_workspace).map_err(|e| {
        SchemaExportError::RustdocFailed(format!(
            "workspace_root symlink guard rejected '{}': {e}",
            workspace_root.display()
        ))
    })
}

pub(super) fn reject_symlinks_for_rustdoc_path(
    path: &Path,
    trusted_root: &Path,
    source: &str,
) -> Result<(), SchemaExportError> {
    crate::track::symlink_guard::reject_symlinks_below(path, trusted_root).map_err(|e| {
        SchemaExportError::RustdocFailed(format!("{source} symlink guard rejected path: {e}"))
    })?;
    Ok(())
}

pub(super) fn absolutize_for_target_guard(path: &Path) -> Result<PathBuf, SchemaExportError> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    std::env::current_dir()
        .map(|cwd| cwd.join(path))
        .map_err(|e| SchemaExportError::RustdocFailed(format!("target-dir guard: {e}")))
}

/// Parse a rustdoc JSON file into a `rustdoc_types::Crate`.
///
/// # Errors
/// Returns `SchemaExportError::ParseFailed` on I/O or JSON parse errors.
pub(super) fn parse_rustdoc_json(path: &Path) -> Result<rustdoc_types::Crate, SchemaExportError> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| SchemaExportError::ParseFailed(format!("read error: {e}")))?;
    serde_json::from_str(&content)
        .map_err(|e| SchemaExportError::ParseFailed(format!("JSON parse error: {e}")))
}
