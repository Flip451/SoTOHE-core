//! Environment inputs for rustdoc cache identity.

use std::ffi::OsStr;
use std::io::{Error, ErrorKind, Read};
use std::path::{Component, Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use super::{
    FingerprintBudget, MAX_RUSTDOC_INPUT_FILE_BYTES, MAX_RUSTDOC_INPUT_PATH_BYTES,
    RustdocInputFingerprintError, check_file_size, io_error,
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
const MAX_RUSTUP_WHICH_OUTPUT_BYTES: usize = 16 * 1024 * 1024;
const RUSTUP_WHICH_TIMEOUT: Duration = Duration::from_secs(120);
const RUSTUP_WHICH_POLL_INTERVAL: Duration = Duration::from_millis(50);
const RUSTUP_WHICH_DRAIN_TIMEOUT: Duration = Duration::from_secs(1);

/// Appends the complete, bounded environment identity.
pub(super) fn append_environment_identity(
    canonical: &mut Vec<u8>,
    workspace_root: &Path,
    budget: &mut FingerprintBudget,
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
        budget.reserve_bytes(value_bytes.len() as u64)?;
        canonical.push(1);
        super::append_len_prefixed_bytes(canonical, &value_bytes);
        if TOOL_ENVIRONMENT_INPUTS.contains(name) {
            let resolved = resolve_tool_path(workspace_root, &trusted_root, name, &value, budget)?;
            super::append_len_prefixed_bytes(canonical, &super::path_bytes(&resolved.path));
            super::append_len_prefixed_bytes(canonical, &super::sha256_bytes(&resolved.bytes));
        }
    }
    append_actual_rustdoc_tool_identity(canonical, workspace_root, budget)?;
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
        return snapshot_tool_file(&resolved, None, name, budget);
    }
    if value_path.is_absolute() {
        let resolved = value_path.canonicalize().map_err(|error| io_error(&value_path, error))?;
        return snapshot_tool_file(&resolved, None, name, budget);
    }
    let candidate = workspace_root.join(value_path);
    let resolved = candidate.canonicalize().map_err(|error| io_error(&candidate, error))?;
    if !resolved.starts_with(trusted_root) {
        return Err(io_error(
            &resolved,
            Error::other(format!("{name} resolves outside the trusted workspace")),
        ));
    }
    snapshot_tool_file(&resolved, Some(trusted_root), name, budget)
}

fn append_actual_rustdoc_tool_identity(
    canonical: &mut Vec<u8>,
    workspace_root: &Path,
    budget: &mut FingerprintBudget,
) -> Result<(), RustdocInputFingerprintError> {
    let rustup = resolve_bare_tool(workspace_root, "rustup", OsStr::new("rustup"))?
        .canonicalize()
        .map_err(|error| io_error(Path::new("rustup"), error))?;
    for tool in ["cargo", "rustc", "rustdoc"] {
        let resolved = resolve_nightly_tool(&rustup, workspace_root, tool, budget)?;
        append_tool_snapshot(canonical, &format!("nightly-{tool}"), &resolved);
    }
    Ok(())
}

struct NightlyToolResolutionOutput {
    status: ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

type NightlyOutputResult = Result<Vec<u8>, Error>;
type NightlyOutputSlot = Arc<Mutex<Option<NightlyOutputResult>>>;

fn run_bounded_command_with_total(
    command: &mut Command,
    maximum_stream_bytes: usize,
    maximum_total_bytes: u64,
) -> Result<NightlyToolResolutionOutput, Error> {
    command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    crate::capability_exec::process::configure_process_group(command);
    let mut child = command.spawn()?;
    let process_id = child.id();
    let stdout = child.stdout.take().ok_or_else(|| {
        Error::new(ErrorKind::BrokenPipe, "rustup nightly tool resolution stdout was not captured")
    });
    let stdout = match stdout {
        Ok(stdout) => stdout,
        Err(error) => {
            let _ = terminate_nightly_process(&mut child, process_id);
            return Err(error);
        }
    };
    let stderr = child.stderr.take().ok_or_else(|| {
        Error::new(ErrorKind::BrokenPipe, "rustup nightly tool resolution stderr was not captured")
    });
    let stderr = match stderr {
        Ok(stderr) => stderr,
        Err(error) => {
            let _ = terminate_nightly_process(&mut child, process_id);
            return Err(error);
        }
    };
    let shared_total = Arc::new(AtomicU64::new(0));
    let stdout_result = Arc::new(Mutex::new(None));
    let stderr_result = Arc::new(Mutex::new(None));
    let stdout_reader = match spawn_nightly_output_reader(
        stdout,
        Arc::clone(&stdout_result),
        Arc::clone(&shared_total),
        maximum_stream_bytes,
        maximum_total_bytes,
    ) {
        Ok(handle) => handle,
        Err(error) => {
            let _ = terminate_nightly_process(&mut child, process_id);
            return Err(error);
        }
    };
    let stderr_reader = match spawn_nightly_output_reader(
        stderr,
        Arc::clone(&stderr_result),
        shared_total,
        maximum_stream_bytes,
        maximum_total_bytes,
    ) {
        Ok(handle) => handle,
        Err(error) => {
            let _ = terminate_nightly_process(&mut child, process_id);
            let _ = stdout_reader.join();
            return Err(error);
        }
    };

    let started = Instant::now();
    let mut exited_at = None;
    let mut status = None;
    let mut stdout = None;
    let mut stderr = None;
    loop {
        if stdout.is_none() {
            if let Some(result) = take_nightly_output_result(&stdout_result) {
                stdout = Some(handle_nightly_output_result(result, &mut child, process_id)?);
            }
        }
        if stderr.is_none() {
            if let Some(result) = take_nightly_output_result(&stderr_result) {
                stderr = Some(handle_nightly_output_result(result, &mut child, process_id)?);
            }
        }

        if status.is_none() {
            match child.try_wait() {
                Ok(Some(child_status)) => {
                    status = Some(child_status);
                    exited_at = Some(Instant::now());
                }
                Ok(None) => {}
                Err(error) => {
                    let termination_detail = terminate_nightly_process(&mut child, process_id)
                        .err()
                        .map(|error| format!("; process termination also failed: {error}"))
                        .unwrap_or_default();
                    return Err(Error::new(
                        error.kind(),
                        format!(
                            "cannot poll rustup nightly tool resolution: {error}{termination_detail}"
                        ),
                    ));
                }
            }
        }

        if status.is_some() && stdout.is_some() && stderr.is_some() {
            let status = match status.take() {
                Some(status) => status,
                None => {
                    return Err(Error::other(
                        "rustup nightly tool resolution lost its exit status",
                    ));
                }
            };
            let stdout = match stdout.take() {
                Some(stdout) => stdout,
                None => return Err(Error::other("rustup nightly tool resolution lost stdout")),
            };
            let stderr = match stderr.take() {
                Some(stderr) => stderr,
                None => return Err(Error::other("rustup nightly tool resolution lost stderr")),
            };
            let stdout_reader_panicked = stdout_reader.join().is_err();
            let stderr_reader_panicked = stderr_reader.join().is_err();
            if stdout_reader_panicked || stderr_reader_panicked {
                return Err(Error::other(
                    "rustup nightly tool resolution output reader thread panicked",
                ));
            }
            return Ok(NightlyToolResolutionOutput { status, stdout, stderr });
        }
        if started.elapsed() >= RUSTUP_WHICH_TIMEOUT {
            let termination_detail = terminate_nightly_process(&mut child, process_id)
                .err()
                .map(|error| format!("; process termination also failed: {error}"))
                .unwrap_or_default();
            return Err(Error::new(
                ErrorKind::TimedOut,
                format!(
                    "rustup nightly tool resolution timed out after {} seconds{termination_detail}",
                    RUSTUP_WHICH_TIMEOUT.as_secs()
                ),
            ));
        }
        if exited_at.is_some_and(|exited| exited.elapsed() >= RUSTUP_WHICH_DRAIN_TIMEOUT) {
            let termination_detail = terminate_nightly_process(&mut child, process_id)
                .err()
                .map(|error| format!("; process termination also failed: {error}"))
                .unwrap_or_default();
            return Err(Error::new(
                ErrorKind::TimedOut,
                format!(
                    "rustup nightly tool resolution output did not close within {} seconds after the subprocess exited{termination_detail}",
                    RUSTUP_WHICH_DRAIN_TIMEOUT.as_secs()
                ),
            ));
        }
        thread::sleep(RUSTUP_WHICH_POLL_INTERVAL);
    }
}

#[cfg(test)]
fn run_nightly_tool_resolution(
    command: &mut Command,
) -> Result<NightlyToolResolutionOutput, Error> {
    run_bounded_command_with_total(command, MAX_RUSTUP_WHICH_OUTPUT_BYTES, u64::MAX)
}

pub(super) fn run_cargo_metadata_with_early_bounded_output(
    command: &mut Command,
    maximum_stream_bytes: usize,
    maximum_total_bytes: u64,
) -> Result<crate::capability_exec::process::BoundedCommandOutput, Error> {
    let output =
        run_bounded_command_with_total(command, maximum_stream_bytes, maximum_total_bytes)?;
    Ok(crate::capability_exec::process::BoundedCommandOutput {
        status: output.status,
        stdout: output.stdout,
        stderr: output.stderr,
    })
}

fn spawn_nightly_output_reader(
    pipe: impl Read + Send + 'static,
    result_slot: NightlyOutputSlot,
    shared_total: Arc<AtomicU64>,
    maximum_stream_bytes: usize,
    maximum_total_bytes: u64,
) -> Result<JoinHandle<()>, Error> {
    let reader_label = "rustup nightly tool resolution output reader";
    let handle = thread::Builder::new()
        .name(reader_label.to_owned())
        .spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                collect_nightly_output(
                    pipe,
                    shared_total,
                    maximum_stream_bytes,
                    maximum_total_bytes,
                )
            }))
            .unwrap_or_else(|_| {
                Err(Error::other("rustup nightly tool resolution output reader panicked"))
            });
            match result_slot.lock() {
                Ok(mut slot) => *slot = Some(result),
                Err(poisoned) => *poisoned.into_inner() = Some(result),
            }
        })
        .map_err(|error| {
            Error::new(error.kind(), format!("cannot spawn {reader_label}: {error}"))
        })?;
    Ok(handle)
}

fn take_nightly_output_result(
    result_slot: &Mutex<Option<NightlyOutputResult>>,
) -> Option<NightlyOutputResult> {
    match result_slot.lock() {
        Ok(mut slot) => slot.take(),
        Err(poisoned) => poisoned.into_inner().take(),
    }
}

fn handle_nightly_output_result(
    result: Result<Vec<u8>, Error>,
    child: &mut Child,
    process_id: u32,
) -> Result<Vec<u8>, Error> {
    result.map_err(|error| {
        let termination_detail = terminate_nightly_process(child, process_id)
            .err()
            .map(|error| format!("; process termination also failed: {error}"))
            .unwrap_or_default();
        Error::new(
            error.kind(),
            format!("rustup nightly tool resolution output failed: {error}{termination_detail}"),
        )
    })
}

fn collect_nightly_output(
    mut pipe: impl Read,
    shared_total: Arc<AtomicU64>,
    maximum_stream_bytes: usize,
    maximum_total_bytes: u64,
) -> Result<Vec<u8>, Error> {
    let mut buffer = [0_u8; 8192];
    let mut bytes = Vec::new();
    loop {
        let stream_remaining = maximum_stream_bytes.saturating_sub(bytes.len());
        let total_remaining =
            maximum_total_bytes.saturating_sub(shared_total.load(Ordering::Acquire));
        let total_remaining = usize::try_from(total_remaining).unwrap_or(usize::MAX);
        let available = stream_remaining.min(total_remaining);
        let read_limit = available.min(buffer.len().saturating_sub(1)).saturating_add(1);
        let read_buffer = buffer.get_mut(..read_limit).ok_or_else(|| {
            Error::new(
                ErrorKind::InvalidData,
                "rustup nightly tool resolution returned an invalid read limit",
            )
        })?;
        let read = pipe.read(read_buffer)?;
        if read == 0 {
            return Ok(bytes);
        }
        let next_len = bytes.len().checked_add(read).ok_or_else(|| {
            Error::new(
                ErrorKind::InvalidData,
                "rustup nightly tool resolution output length overflowed",
            )
        })?;
        if next_len > maximum_stream_bytes {
            return Err(Error::new(
                ErrorKind::InvalidData,
                format!("subprocess output exceeds {maximum_stream_bytes} bytes per stream"),
            ));
        }
        let chunk = buffer.get(..read).ok_or_else(|| {
            Error::new(
                ErrorKind::InvalidData,
                "rustup nightly tool resolution returned an invalid byte count",
            )
        })?;
        if !reserve_shared_output_bytes(&shared_total, read, maximum_total_bytes) {
            return Err(Error::new(
                ErrorKind::InvalidData,
                format!(
                    "rustup nightly tool resolution output exceeds {maximum_total_bytes} aggregate bytes"
                ),
            ));
        }
        bytes.extend_from_slice(chunk);
    }
}

fn reserve_shared_output_bytes(total: &AtomicU64, bytes: usize, maximum: u64) -> bool {
    let bytes = bytes as u64;
    total
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |used| {
            used.checked_add(bytes).filter(|next| *next <= maximum)
        })
        .is_ok()
}

fn terminate_nightly_process(child: &mut Child, process_id: u32) -> Result<(), Error> {
    if crate::capability_exec::process::terminate_bounded_process_group(process_id).is_err() {
        if let Err(error) = child.kill() {
            if error.kind() != ErrorKind::InvalidInput {
                return Err(error);
            }
        }
    }
    child.wait().map(|_| ())
}

fn resolve_nightly_tool(
    rustup: &Path,
    workspace_root: &Path,
    tool: &str,
    budget: &mut FingerprintBudget,
) -> Result<ResolvedTool, RustdocInputFingerprintError> {
    let mut command = Command::new(rustup);
    command.args(["which", "--toolchain", "nightly", tool]).current_dir(workspace_root);
    let output = run_bounded_command_with_total(
        &mut command,
        MAX_RUSTUP_WHICH_OUTPUT_BYTES,
        budget.remaining_bytes(),
    )
    .map_err(|error| io_error(rustup, error))?;
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
    let resolved = selected.canonicalize().map_err(|error| io_error(&selected, error))?;
    snapshot_tool_file(&resolved, None, &format!("nightly {tool}"), budget)
}

fn snapshot_tool_file(
    path: &Path,
    trusted_root: Option<&Path>,
    label: &str,
    budget: &mut FingerprintBudget,
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
    budget.reserve_file(metadata.len())?;
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

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    #[cfg(unix)]
    use std::io::ErrorKind;
    #[cfg(unix)]
    use std::process::Command;
    #[cfg(unix)]
    use std::time::{Duration, Instant};

    #[cfg(unix)]
    #[test]
    fn test_chatty_nightly_resolution_terminates_at_the_output_cap() {
        let blocks = super::MAX_RUSTUP_WHICH_OUTPUT_BYTES / 1024 + 1;
        let command_line =
            format!("/bin/dd if=/dev/zero bs=1024 count={blocks} 2>/dev/null; /bin/sleep 120");
        let mut command = Command::new("/bin/sh");
        command.args(["-c", command_line.as_str()]);
        let started = Instant::now();

        let error = match super::run_nightly_tool_resolution(&mut command) {
            Ok(_) => panic!("chatty nightly resolution must fail at its output cap"),
            Err(error) => error,
        };

        assert_eq!(error.kind(), ErrorKind::InvalidData);
        assert!(
            error.to_string().contains("exceeds"),
            "the output-cap failure must identify the bound: {error}"
        );
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "output-cap termination must not wait for the 120-second command: {:?}",
            started.elapsed()
        );
    }
}
