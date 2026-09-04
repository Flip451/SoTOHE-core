//! Environment inputs for rustdoc cache identity.

use std::ffi::OsStr;
use std::io::{Error, ErrorKind};
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::Duration;

#[path = "environment_fingerprint/command_spawn.rs"]
mod command_spawn;
#[path = "environment_fingerprint/output_reader.rs"]
mod output_reader;

use super::{
    FingerprintBudget, FingerprintDeadline, MAX_RUSTDOC_INPUT_FILE_BYTES,
    MAX_RUSTDOC_INPUT_PATH_BYTES, RustdocInputFingerprintError, check_file_size, io_error,
};

pub(super) fn run_cargo_metadata_with_early_bounded_output(
    command: Command,
    maximum_stream_bytes: usize,
    maximum_total_bytes: u64,
    execution_timeout: Duration,
    drain_timeout: Duration,
) -> Result<crate::capability_exec::process::BoundedCommandOutput, Error> {
    command_spawn::run_cargo_metadata_with_early_bounded_output(
        command,
        maximum_stream_bytes,
        maximum_total_bytes,
        execution_timeout,
        drain_timeout,
    )
}

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
const MAX_RUSTUP_WHICH_OUTPUT_BYTES: usize = 16 * 1024 * 1024;
const RUSTUP_WHICH_POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Appends the complete, bounded environment identity.
pub(super) fn append_environment_identity(
    canonical: &mut Vec<u8>,
    workspace_root: &Path,
    budget: &mut FingerprintBudget,
    deadline: &FingerprintDeadline,
    drain_timeout: Duration,
) -> Result<(), RustdocInputFingerprintError> {
    deadline.check("environment identity")?;
    let workspace_root_path = workspace_root.to_owned();
    let trusted_root =
        deadline.run_io("environment identity", workspace_root_path.clone(), move || {
            workspace_root_path.canonicalize()
        })?;
    deadline.check("environment identity")?;
    for name in RUSTDOC_ENVIRONMENT_INPUTS {
        deadline.check("environment identity")?;
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
        budget.reserve_bytes(value_bytes.len() as u64)?;
        canonical.push(1);
        super::append_len_prefixed_bytes(canonical, &value_bytes);
        if TOOL_ENVIRONMENT_INPUTS.contains(name) {
            let resolved =
                resolve_tool_path(workspace_root, &trusted_root, name, &value, budget, deadline)?;
            super::append_len_prefixed_bytes(canonical, &super::path_bytes(&resolved.path));
            super::append_len_prefixed_bytes(canonical, &super::sha256_bytes(&resolved.bytes));
        }
    }
    append_actual_rustdoc_tool_identity(
        canonical,
        workspace_root,
        budget,
        deadline,
        drain_timeout,
    )?;
    deadline.check("environment identity")?;
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
    budget: &mut FingerprintBudget,
    deadline: &FingerprintDeadline,
) -> Result<ResolvedTool, RustdocInputFingerprintError> {
    deadline.check("tool path resolution")?;
    if value.is_empty() {
        return Err(io_error(
            Path::new(name),
            Error::new(ErrorKind::InvalidInput, "tool path is empty"),
        ));
    }
    let value_path = PathBuf::from(value);
    if is_bare_command(&value_path) {
        let candidate = resolve_bare_tool(workspace_root, name, value, deadline)?;
        deadline.check("tool path resolution")?;
        let candidate_path = candidate.clone();
        let resolved = deadline.run_io("tool path resolution", candidate.clone(), move || {
            candidate_path.canonicalize()
        })?;
        deadline.check("tool path resolution")?;
        return snapshot_tool_file(&resolved, None, name, budget, deadline);
    }
    if value_path.is_absolute() {
        deadline.check("tool path resolution")?;
        let value_path_for_io = value_path.clone();
        let resolved = deadline.run_io("tool path resolution", value_path.clone(), move || {
            value_path_for_io.canonicalize()
        })?;
        deadline.check("tool path resolution")?;
        return snapshot_tool_file(&resolved, None, name, budget, deadline);
    }
    let candidate = workspace_root.join(value_path);
    deadline.check("tool path resolution")?;
    let candidate_path = candidate.clone();
    let resolved = deadline
        .run_io("tool path resolution", candidate.clone(), move || candidate_path.canonicalize())?;
    deadline.check("tool path resolution")?;
    if !resolved.starts_with(trusted_root) {
        return Err(io_error(
            &resolved,
            Error::other(format!("{name} resolves outside the trusted workspace")),
        ));
    }
    snapshot_tool_file(&resolved, Some(trusted_root), name, budget, deadline)
}

fn append_actual_rustdoc_tool_identity(
    canonical: &mut Vec<u8>,
    workspace_root: &Path,
    budget: &mut FingerprintBudget,
    deadline: &FingerprintDeadline,
    drain_timeout: Duration,
) -> Result<(), RustdocInputFingerprintError> {
    deadline.check("nightly tool resolution")?;
    let rustup_path = resolve_bare_tool(workspace_root, "rustup", OsStr::new("rustup"), deadline)?;
    let rustup_path_for_io = rustup_path.clone();
    let rustup = deadline.run_io("nightly tool resolution", rustup_path, move || {
        rustup_path_for_io.canonicalize()
    })?;
    deadline.check("nightly tool resolution")?;
    for tool in ["cargo", "rustc", "rustdoc"] {
        let resolved =
            resolve_nightly_tool(&rustup, workspace_root, tool, budget, deadline, drain_timeout)?;
        append_tool_snapshot(canonical, &format!("nightly-{tool}"), &resolved);
    }
    Ok(())
}

fn resolve_nightly_tool(
    rustup: &Path,
    workspace_root: &Path,
    tool: &str,
    budget: &mut FingerprintBudget,
    deadline: &FingerprintDeadline,
    drain_timeout: Duration,
) -> Result<ResolvedTool, RustdocInputFingerprintError> {
    let execution_timeout = deadline.remaining("nightly tool resolution")?;
    let mut command = Command::new(rustup);
    command.args(["which", "--toolchain", "nightly", tool]).current_dir(workspace_root);
    let output = command_spawn::run_bounded_command_with_total(
        command,
        MAX_RUSTUP_WHICH_OUTPUT_BYTES,
        budget.remaining_bytes(),
        execution_timeout,
        drain_timeout,
        "rustup nightly tool resolution",
    )
    .map_err(|error| io_error(rustup, error))?;
    deadline.check("nightly tool resolution")?;
    let output_bytes = output.stdout.len().checked_add(output.stderr.len()).ok_or(
        RustdocInputFingerprintError::TotalBytes {
            bytes: u64::MAX,
            maximum: super::MAX_RUSTDOC_INPUT_TOTAL_BYTES,
        },
    )?;
    budget.reserve_bytes(output_bytes as u64)?;
    if !output.status.success() {
        return Err(io_error(
            rustup,
            Error::other(format!(
                "rustup which --toolchain nightly {tool} exited with {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            )),
        ));
    }
    let stdout = std::str::from_utf8(&output.stdout).map_err(|error| {
        io_error(
            rustup,
            Error::new(
                ErrorKind::InvalidData,
                format!("rustup which returned non-UTF-8 output for nightly {tool}: {error}"),
            ),
        )
    })?;
    let selected = stdout.trim();
    if selected.is_empty() || selected.lines().count() != 1 {
        return Err(io_error(
            rustup,
            Error::new(
                ErrorKind::InvalidData,
                format!("rustup which returned no single path for nightly {tool}"),
            ),
        ));
    }
    let selected = PathBuf::from(selected);
    let selected_path_bytes = super::path_bytes(&selected);
    if selected_path_bytes.len() > MAX_RUSTDOC_INPUT_PATH_BYTES {
        return Err(RustdocInputFingerprintError::PathBytes {
            path: selected,
            bytes: selected_path_bytes.len(),
            maximum: MAX_RUSTDOC_INPUT_PATH_BYTES,
        });
    }
    let selected = if selected.is_absolute() { selected } else { workspace_root.join(selected) };
    let selected_path = selected.clone();
    let resolved = deadline.run_io("nightly tool resolution", selected.clone(), move || {
        selected_path.canonicalize()
    })?;
    deadline.check("nightly tool resolution")?;
    snapshot_tool_file(&resolved, None, &format!("nightly {tool}"), budget, deadline)
}

fn snapshot_tool_file(
    path: &Path,
    trusted_root: Option<&Path>,
    label: &str,
    budget: &mut FingerprintBudget,
    deadline: &FingerprintDeadline,
) -> Result<ResolvedTool, RustdocInputFingerprintError> {
    deadline.check("tool snapshot")?;
    let path_for_io = path.to_owned();
    let resolved = deadline
        .run_io("tool snapshot", path_for_io.clone(), move || path_for_io.canonicalize())?;
    deadline.check("tool snapshot")?;
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
    let metadata_path = resolved.clone();
    let metadata = deadline.run_io("tool snapshot", resolved.clone(), move || {
        std::fs::symlink_metadata(&metadata_path)
    })?;
    deadline.check("tool snapshot")?;
    if !metadata.is_file() {
        return Err(io_error(
            &resolved,
            Error::new(ErrorKind::InvalidInput, format!("{label} is not a regular file")),
        ));
    }
    check_file_size(&resolved, metadata.len())?;
    budget.reserve_file(metadata.len())?;
    let read_path = resolved.clone();
    let trusted_root = trusted_root.map(Path::to_path_buf);
    let bytes = deadline
        .run_io("tool snapshot", resolved.clone(), move || {
            crate::tddd::tddd_catalogue_document_loader::read_optional_regular_file_bytes(
                &read_path,
                trusted_root.as_deref(),
                MAX_RUSTDOC_INPUT_FILE_BYTES,
            )
        })?
        .ok_or_else(|| io_error(&resolved, Error::new(ErrorKind::NotFound, "tool disappeared")))?;
    deadline.check("tool snapshot")?;
    let after_path = resolved.clone();
    let after = deadline.run_io("tool snapshot", resolved.clone(), move || {
        std::fs::symlink_metadata(&after_path)
    })?;
    deadline.check("tool snapshot")?;
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
    deadline: &FingerprintDeadline,
) -> Result<PathBuf, RustdocInputFingerprintError> {
    let path = std::env::var_os("PATH").ok_or_else(|| {
        io_error(
            Path::new(name),
            Error::new(ErrorKind::NotFound, "PATH is unavailable for bare tool path"),
        )
    })?;
    let mut found = None;
    for directory in std::env::split_paths(&path) {
        deadline.check("PATH tool resolution")?;
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
        let candidate_path = candidate.clone();
        let metadata = deadline.run_io("PATH tool resolution", candidate.clone(), move || {
            match std::fs::metadata(&candidate_path) {
                Ok(metadata) => Ok(Some(metadata)),
                Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
                Err(error) => Err(error),
            }
        })?;
        match metadata {
            Some(metadata) if metadata.is_file() => {
                found = Some(candidate);
                break;
            }
            Some(_) | None => {}
        }
        deadline.check("PATH tool resolution")?;
    }
    found.ok_or_else(|| {
        io_error(
            Path::new(name),
            Error::new(ErrorKind::NotFound, "bare tool path was not found on PATH"),
        )
    })
}
