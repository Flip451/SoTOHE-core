//! Target-directory and path-safety helpers for the schema export infrastructure adapter.
//!
//! Functions here resolve the Cargo target directory (respecting `CARGO_TARGET_DIR` and
//! workspace config), guard all resolved paths against escape outside the workspace root,
//! and reject any symlinks beneath the trusted root — preventing path-traversal attacks
//! where a crafted `CARGO_TARGET_DIR` or symlink redirects rustdoc JSON output to an
//! arbitrary location on disk.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use domain::schema::SchemaExportError;
use domain::tddd::CargoFeatureName;
use domain::tddd::catalogue_v2::CrateName;
use sha2::Digest as _;

const MAX_CARGO_METADATA_OUTPUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_CARGO_METADATA_DURATION: Duration = Duration::from_secs(120);

/// Resolves the target directory for snapshot reuse.
///
/// This never guesses `<workspace>/target` when Cargo metadata cannot establish
/// the configured target directory. Reusing a stale snapshot is safe only when
/// its location is known exactly.
pub(super) fn resolve_target_dir_strict(
    workspace_root: &Path,
) -> Result<PathBuf, SchemaExportError> {
    if let Ok(dir) = std::env::var("CARGO_TARGET_DIR") {
        return resolve_configured_target_dir(
            workspace_root,
            PathBuf::from(dir),
            "CARGO_TARGET_DIR",
        );
    }
    let output = run_cargo_metadata(workspace_root, "target directory for snapshot reuse")?;
    if !output.status.success() {
        return Err(SchemaExportError::RustdocFailed(format!(
            "cannot resolve target directory for snapshot reuse: cargo metadata exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout).map_err(|error| {
        SchemaExportError::RustdocFailed(format!(
            "cannot resolve target directory for snapshot reuse: cargo metadata output is invalid: {error}"
        ))
    })?;
    let target_directory = metadata
        .get("target_directory")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            SchemaExportError::RustdocFailed(
                "cannot resolve target directory for snapshot reuse: cargo metadata has no target_directory"
                    .to_owned(),
            )
        })?;
    resolve_configured_target_dir(
        workspace_root,
        PathBuf::from(target_directory),
        "cargo metadata target_directory",
    )
}

/// Resolves the private Cargo target area used by one rustdoc selection.
///
/// Cargo's ordinary target directory is shared with unrelated commands. The
/// rustdoc adapter therefore places each stable workspace/package/feature
/// selection below a hidden, no-follow-created subtree. Ordinary Cargo
/// rustdoc invocations continue to use the parent target directory and cannot
/// overwrite this adapter's expected JSON by accident; cooperating adapter
/// invocations are serialized by the lock in that subtree.
pub(super) fn resolve_exclusive_target_dir(
    workspace_root: &Path,
    crate_name: &CrateName,
    features: &[CargoFeatureName],
    use_default_features: bool,
) -> Result<PathBuf, SchemaExportError> {
    let cargo_target_dir = resolve_target_dir_strict(workspace_root)?;
    let trusted_workspace = checked_workspace_root(workspace_root)?;
    let mut identity = Vec::new();
    append_len_prefixed_bytes(&mut identity, &path_bytes(&trusted_workspace));
    append_len_prefixed_bytes(&mut identity, crate_name.as_str().as_bytes());
    append_len_prefixed_bytes(
        &mut identity,
        if use_default_features { b"default-features" } else { b"declared-features" },
    );
    for feature in features {
        append_len_prefixed_bytes(&mut identity, feature.as_str().as_bytes());
    }
    let directory_name = hex_digest(&sha2::Sha256::digest(&identity));
    let exclusive = cargo_target_dir.join(".sotp-rustdoc").join(directory_name);
    crate::track::symlink_guard::reject_symlinks_up_to_root(&exclusive).map_err(|error| {
        SchemaExportError::RustdocFailed(format!(
            "exclusive rustdoc target directory symlink guard rejected '{}': {error}",
            exclusive.display()
        ))
    })?;
    Ok(exclusive)
}

fn run_cargo_metadata(
    workspace_root: &Path,
    purpose: &str,
) -> Result<crate::capability_exec::process::BoundedCommandOutput, SchemaExportError> {
    let mut command = Command::new("cargo");
    command.args(["metadata", "--format-version", "1", "--no-deps"]).current_dir(workspace_root);
    crate::capability_exec::process::run_command_with_bounded_output(
        &mut command,
        MAX_CARGO_METADATA_OUTPUT_BYTES,
        MAX_CARGO_METADATA_DURATION,
        purpose,
    )
    .map_err(|error| SchemaExportError::RustdocFailed(format!("cargo metadata failed: {error}")))
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
///   configured only when every path component is non-symlinked. A symlinked
///   ancestor would otherwise redirect the trusted target root outside its
///   lexical location.
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
        crate::track::symlink_guard::reject_symlinks_up_to_root(&normalized_target).map_err(
            |error| {
                SchemaExportError::RustdocFailed(format!(
                    "{source} target directory symlink guard rejected '{}': {error}",
                    normalized_target.display()
                ))
            },
        )?;
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
    crate::track::symlink_guard::reject_symlinks_up_to_root(&normalized_workspace).map_err(
        |error| {
            SchemaExportError::RustdocFailed(format!(
                "workspace_root symlink guard rejected '{}': {error}",
                workspace_root.display()
            ))
        },
    )?;
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

fn append_len_prefixed_bytes(output: &mut Vec<u8>, bytes: &[u8]) {
    output.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
    output.extend_from_slice(bytes);
}

fn path_bytes(path: &Path) -> Vec<u8> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt as _;
        path.as_os_str().as_bytes().to_vec()
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt as _;
        path.as_os_str().encode_wide().flat_map(u16::to_be_bytes).collect()
    }
    #[cfg(not(any(unix, windows)))]
    {
        path.to_string_lossy().into_owned().into_bytes()
    }
}

fn hex_digest(digest: &[u8]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}
