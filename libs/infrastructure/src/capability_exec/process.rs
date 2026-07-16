//! Provider subprocess execution, bounded logging, and process-tree cleanup.

use std::ffi::OsString;
use std::fs::{File, OpenOptions};
use std::io::{Error, ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use usecase::capability_exec::{CapabilityExecError, ProviderName};

use super::{MAX_CAPABILITY_EXEC_LOG_BYTES, dispatch_error, path_guard};

const PROVIDER_PROCESS_POLL_INTERVAL: Duration = Duration::from_millis(50);
const PROVIDER_LOG_DRAIN_TIMEOUT: Duration = Duration::from_secs(1);
const BOUNDED_COMMAND_POLL_INTERVAL: Duration = Duration::from_millis(50);
const BOUNDED_COMMAND_DRAIN_TIMEOUT: Duration = Duration::from_secs(1);
const LOG_TRUNCATION_NOTICE: &[u8] = b"\n[provider stderr truncated]\n";
static PROVIDER_LOG_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Bounded output collected from a short-lived subprocess.
pub(crate) struct BoundedCommandOutput {
    pub(crate) status: ExitStatus,
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: Vec<u8>,
}

struct BoundedPipeOutput {
    bytes: Vec<u8>,
    exceeded_limit: bool,
}

/// Runs a subprocess while draining both output streams concurrently.
///
/// Each stream retains at most `maximum_bytes`, but continues draining after
/// reaching that cap so the child cannot block on a full pipe. An over-limit
/// stream is reported only after the child exits and both readers are joined.
pub(crate) fn run_command_with_bounded_output(
    command: &mut Command,
    maximum_bytes: usize,
    timeout: Duration,
    label: &str,
) -> Result<BoundedCommandOutput, std::io::Error> {
    command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    configure_process_group(command);
    let mut child = command.spawn()?;
    let process_id = child.id();
    let stdout = child.stdout.take().ok_or_else(|| {
        Error::new(ErrorKind::BrokenPipe, format!("{label} subprocess stdout was not captured"))
    })?;
    let stderr = child.stderr.take().ok_or_else(|| {
        Error::new(ErrorKind::BrokenPipe, format!("{label} subprocess stderr was not captured"))
    })?;
    let stdout_reader = spawn_bounded_pipe_reader(stdout, maximum_bytes);
    let stderr_reader = spawn_bounded_pipe_reader(stderr, maximum_bytes);
    let status = wait_for_bounded_command(&mut child, process_id, timeout, label)?;
    let stdout = receive_bounded_pipe(stdout_reader, process_id, label, "stdout")?;
    let stderr = receive_bounded_pipe(stderr_reader, process_id, label, "stderr")?;
    if stdout.exceeded_limit || stderr.exceeded_limit {
        return Err(Error::new(
            ErrorKind::InvalidData,
            format!("{label} subprocess output exceeds {maximum_bytes} bytes per stream"),
        ));
    }
    Ok(BoundedCommandOutput { status, stdout: stdout.bytes, stderr: stderr.bytes })
}

fn spawn_bounded_pipe_reader(
    pipe: impl Read + Send + 'static,
    maximum_bytes: usize,
) -> Receiver<Result<BoundedPipeOutput, std::io::Error>> {
    let (sender, receiver) = mpsc::sync_channel(1);
    thread::spawn(move || {
        let _ = sender.send(collect_bounded_pipe(pipe, maximum_bytes));
    });
    receiver
}

fn collect_bounded_pipe(
    mut pipe: impl Read,
    maximum_bytes: usize,
) -> Result<BoundedPipeOutput, std::io::Error> {
    let mut buffer = [0_u8; 8192];
    let mut bytes = Vec::new();
    let mut exceeded_limit = false;
    loop {
        let read = pipe.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        let remaining = maximum_bytes.saturating_sub(bytes.len());
        let retained = remaining.min(read);
        let retained_bytes = buffer.get(..retained).ok_or_else(|| {
            Error::new(
                ErrorKind::InvalidData,
                "subprocess pipe reader returned an invalid byte count",
            )
        })?;
        bytes.extend_from_slice(retained_bytes);
        exceeded_limit |= retained < read;
    }
    Ok(BoundedPipeOutput { bytes, exceeded_limit })
}

fn receive_bounded_pipe(
    receiver: Receiver<Result<BoundedPipeOutput, std::io::Error>>,
    process_id: u32,
    label: &str,
    stream: &str,
) -> Result<BoundedPipeOutput, std::io::Error> {
    match receiver.recv_timeout(BOUNDED_COMMAND_DRAIN_TIMEOUT) {
        Ok(Ok(output)) => Ok(output),
        Ok(Err(error)) => {
            Err(Error::new(error.kind(), format!("cannot read {label} {stream}: {error}")))
        }
        Err(RecvTimeoutError::Timeout) => {
            let termination_detail = terminate_bounded_process_group(process_id)
                .err()
                .map(|error| format!("; process-tree termination also failed: {error}"))
                .unwrap_or_default();
            Err(Error::new(
                ErrorKind::TimedOut,
                format!(
                    "{label} {stream} reader did not close within {} seconds after the subprocess exited{termination_detail}",
                    BOUNDED_COMMAND_DRAIN_TIMEOUT.as_secs()
                ),
            ))
        }
        Err(RecvTimeoutError::Disconnected) => {
            Err(Error::other(format!("{label} {stream} reader thread disconnected")))
        }
    }
}

fn wait_for_bounded_command(
    child: &mut Child,
    process_id: u32,
    timeout: Duration,
    label: &str,
) -> Result<ExitStatus, std::io::Error> {
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) if started.elapsed() >= timeout => {
                terminate_bounded_command(child, process_id)?;
                return Err(Error::new(
                    ErrorKind::TimedOut,
                    format!("{label} subprocess timed out after {} seconds", timeout.as_secs()),
                ));
            }
            Ok(None) => thread::sleep(BOUNDED_COMMAND_POLL_INTERVAL),
            Err(error) => {
                let termination_detail = terminate_bounded_command(child, process_id)
                    .err()
                    .map(|termination_error| {
                        format!("; process-tree termination also failed: {termination_error}")
                    })
                    .unwrap_or_default();
                return Err(Error::new(
                    error.kind(),
                    format!("cannot poll {label} subprocess: {error}{termination_detail}"),
                ));
            }
        }
    }
}

fn terminate_bounded_command(child: &mut Child, process_id: u32) -> Result<(), std::io::Error> {
    if terminate_bounded_process_group(process_id).is_err() {
        child.kill()?;
    }
    child.wait().map(|_| ())
}

pub(crate) trait ProviderProcessRunner: Send + Sync {
    #[allow(clippy::too_many_arguments)]
    fn run(
        &self,
        binary: &str,
        args: &[OsString],
        repo_root: &Path,
        runtime_dir: &Path,
        provider: &ProviderName,
        timeout: Option<Duration>,
        output_last_message: Option<&Path>,
    ) -> Result<ProviderProcessOutput, CapabilityExecError>;
}

/// Observable, bounded result of a provider subprocess.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProviderProcessOutput {
    pub(crate) exit_code: u8,
    pub(crate) session_id: Option<String>,
    /// The bounded provider result retained until the adapter adopts this attempt.
    pub(crate) final_message: Option<Vec<u8>>,
}

pub(crate) struct SystemProviderProcessRunner;

impl ProviderProcessRunner for SystemProviderProcessRunner {
    fn run(
        &self,
        binary: &str,
        args: &[OsString],
        repo_root: &Path,
        runtime_dir: &Path,
        provider: &ProviderName,
        timeout: Option<Duration>,
        output_last_message: Option<&Path>,
    ) -> Result<ProviderProcessOutput, CapabilityExecError> {
        run_provider_process_with_timeout(
            binary,
            args,
            repo_root,
            runtime_dir,
            provider,
            timeout,
            output_last_message,
        )
    }
}

pub(crate) fn system_process_runner() -> Arc<dyn ProviderProcessRunner> {
    Arc::new(SystemProviderProcessRunner)
}

pub(crate) fn run_provider_process_with_timeout(
    binary: &str,
    args: &[OsString],
    repo_root: &Path,
    runtime_dir: &Path,
    provider: &ProviderName,
    timeout: Option<Duration>,
    output_last_message: Option<&Path>,
) -> Result<ProviderProcessOutput, CapabilityExecError> {
    run_provider_process_with_timeout_inner(
        binary,
        args,
        repo_root,
        runtime_dir,
        provider,
        timeout,
        output_last_message,
    )
}

#[allow(clippy::too_many_arguments)]
fn run_provider_process_with_timeout_inner(
    binary: &str,
    args: &[OsString],
    repo_root: &Path,
    runtime_dir: &Path,
    provider: &ProviderName,
    timeout: Option<Duration>,
    output_last_message: Option<&Path>,
) -> Result<ProviderProcessOutput, CapabilityExecError> {
    let runtime_dir = prepare_runtime_dir(repo_root, runtime_dir, provider)?;
    let output_last_message = output_last_message
        .map(|path| initialize_output_last_message(path, &runtime_dir, provider))
        .transpose()?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| {
            dispatch_error(provider, format!("cannot create session log timestamp: {error}"))
        })?
        .as_nanos();
    let log_path = runtime_dir.join(format!(
        "capability-exec-{}-{}-{timestamp}-{}.log",
        provider.as_str(),
        std::process::id(),
        PROVIDER_LOG_SEQUENCE.fetch_add(1, Ordering::Relaxed),
    ));
    let log_file =
        OpenOptions::new().write(true).create_new(true).open(&log_path).map_err(|error| {
            dispatch_error(
                provider,
                format!("cannot create session log {}: {error}", log_path.display()),
            )
        })?;

    let mut command = Command::new(binary);
    command
        .args(args)
        .current_dir(repo_root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_process_group(&mut command);
    let mut child = command
        .spawn()
        .map_err(|error| dispatch_error(provider, format!("cannot start {binary}: {error}")))?;
    let process_id = child.id();
    let stderr = child.stderr.take().ok_or_else(|| {
        dispatch_error(provider, format!("cannot capture stderr for {binary} provider subprocess"))
    })?;
    let stdout = child.stdout.take().ok_or_else(|| {
        dispatch_error(provider, format!("cannot capture stdout for {binary} provider subprocess"))
    })?;
    let log_writer = spawn_bounded_log_writer(stderr, log_file);
    let session_collector = spawn_provider_session_collector(stdout, provider.clone());
    let status = wait_for_provider_process(&mut child, provider, binary, timeout)?;
    wait_for_bounded_log_writer(log_writer, process_id, provider, binary, &log_path)?;
    let collected = receive_provider_output(session_collector, process_id, provider, binary)?;
    // The provider may mutate its writable runtime directory while it runs. Re-check every
    // component after it exits before opening provider-controlled output so a replaced parent
    // cannot redirect the leaf read outside the repository runtime.
    let runtime_dir = prepare_runtime_dir(repo_root, &runtime_dir, provider)?;
    let final_message = match output_last_message {
        Some(path) => {
            validate_output_last_message_parent(&path, &runtime_dir, provider)?;
            read_output_last_message(&path, provider)?
        }
        None => collected.final_message,
    };
    Ok(ProviderProcessOutput {
        exit_code: status.code().and_then(|code| u8::try_from(code).ok()).unwrap_or(1),
        session_id: collected.session_id,
        final_message,
    })
}

/// Emits the final message of an attempt the adapter has selected for adoption.
pub(crate) fn emit_provider_final_message(
    output: &ProviderProcessOutput,
    provider: &ProviderName,
    binary: &str,
    passthrough: &mut impl Write,
) -> Result<(), CapabilityExecError> {
    let Some(message) = output.final_message.as_deref() else {
        return Ok(());
    };
    passthrough.write_all(message).map_err(|error| {
        dispatch_error(provider, format!("cannot write final message from {binary}: {error}"))
    })?;
    passthrough.flush().map_err(|error| {
        dispatch_error(provider, format!("cannot flush final message from {binary}: {error}"))
    })
}

#[path = "process/output_collector.rs"]
mod output_collector;

use output_collector::{
    initialize_output_last_message, read_output_last_message, receive_provider_output,
    spawn_provider_session_collector, validate_output_last_message_parent,
};
fn wait_for_bounded_log_writer(
    log_writer: Receiver<Result<(), std::io::Error>>,
    process_id: u32,
    provider: &ProviderName,
    binary: &str,
    log_path: &Path,
) -> Result<(), CapabilityExecError> {
    match log_writer.recv_timeout(PROVIDER_LOG_DRAIN_TIMEOUT) {
        Ok(Ok(())) => Ok(()),
        Ok(Err(error)) => {
            let termination_detail = terminate_provider_process_group(process_id, provider, binary)
                .err()
                .map(|termination_error| {
                    format!("; process-group termination also failed: {termination_error}")
                })
                .unwrap_or_default();
            Err(dispatch_error(
                provider,
                format!(
                    "cannot write bounded session log {}: {error}{termination_detail}",
                    log_path.display()
                ),
            ))
        }
        Err(RecvTimeoutError::Timeout) => {
            let termination_detail = terminate_provider_process_group(process_id, provider, binary)
                .err()
                .map(|termination_error| {
                    format!("; process-group termination also failed: {termination_error}")
                })
                .unwrap_or_default();
            Err(dispatch_error(
                provider,
                format!(
                    "stderr drain did not close within {} seconds after {binary} exited{termination_detail}",
                    PROVIDER_LOG_DRAIN_TIMEOUT.as_secs(),
                ),
            ))
        }
        Err(RecvTimeoutError::Disconnected) => {
            Err(dispatch_error(provider, format!("stderr logger thread disconnected for {binary}")))
        }
    }
}

fn prepare_runtime_dir(
    repo_root: &Path,
    runtime_dir: &Path,
    provider: &ProviderName,
) -> Result<PathBuf, CapabilityExecError> {
    let normalized_root = path_guard::lexically_normalize(repo_root);
    let normalized_runtime =
        path_guard::normalize_path_rejecting_symlinked_components(runtime_dir, repo_root).map_err(
            |error| {
                dispatch_error(
                    provider,
                    format!(
                        "refusing to follow symlink while resolving runtime directory {}: {error}",
                        runtime_dir.display()
                    ),
                )
            },
        )?;
    if !normalized_runtime.starts_with(&normalized_root) {
        return Err(dispatch_error(
            provider,
            format!(
                "runtime directory {} escapes repository root {}",
                runtime_dir.display(),
                normalized_root.display()
            ),
        ));
    }
    let canonical_root = normalized_root.canonicalize().map_err(|error| {
        dispatch_error(
            provider,
            format!("cannot canonicalize repository root {}: {error}", repo_root.display()),
        )
    })?;
    let root_metadata = canonical_root.metadata().map_err(|error| {
        dispatch_error(
            provider,
            format!("cannot inspect repository root {}: {error}", canonical_root.display()),
        )
    })?;
    if !root_metadata.is_dir() {
        return Err(dispatch_error(
            provider,
            format!("repository root {} is not a directory", canonical_root.display()),
        ));
    }
    let relative_runtime = normalized_runtime.strip_prefix(&normalized_root).map_err(|error| {
        dispatch_error(
            provider,
            format!(
                "cannot resolve runtime directory {} below repository root {}: {error}",
                normalized_runtime.display(),
                normalized_root.display()
            ),
        )
    })?;
    let mut current = canonical_root.clone();
    for component in relative_runtime.components() {
        let std::path::Component::Normal(name) = component else {
            return Err(dispatch_error(
                provider,
                format!(
                    "runtime directory {} contains an invalid normalized component",
                    normalized_runtime.display()
                ),
            ));
        };
        let next = current.join(name);
        match next.symlink_metadata() {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(dispatch_error(
                    provider,
                    format!("refusing to follow symlink: {}", next.display()),
                ));
            }
            Ok(metadata) if !metadata.is_dir() => {
                return Err(dispatch_error(
                    provider,
                    format!("runtime path component {} is not a directory", next.display()),
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {
                if let Err(error) = std::fs::create_dir(&next)
                    && error.kind() != ErrorKind::AlreadyExists
                {
                    return Err(dispatch_error(
                        provider,
                        format!("cannot create runtime directory {}: {error}", next.display()),
                    ));
                }
                let metadata = next.symlink_metadata().map_err(|error| {
                    dispatch_error(
                        provider,
                        format!(
                            "cannot inspect created runtime directory {}: {error}",
                            next.display()
                        ),
                    )
                })?;
                if metadata.file_type().is_symlink() {
                    return Err(dispatch_error(
                        provider,
                        format!("refusing to follow symlink: {}", next.display()),
                    ));
                }
                if !metadata.is_dir() {
                    return Err(dispatch_error(
                        provider,
                        format!("runtime path component {} is not a directory", next.display()),
                    ));
                }
            }
            Err(error) => {
                return Err(dispatch_error(
                    provider,
                    format!("cannot inspect runtime path component {}: {error}", next.display()),
                ));
            }
        }
        current = next;
    }
    let canonical_runtime = current.canonicalize().map_err(|error| {
        dispatch_error(
            provider,
            format!("cannot canonicalize runtime directory {}: {error}", current.display()),
        )
    })?;
    if !canonical_runtime.starts_with(&canonical_root) {
        return Err(dispatch_error(
            provider,
            format!(
                "runtime directory {} escapes repository root {}",
                canonical_runtime.display(),
                canonical_root.display()
            ),
        ));
    }
    Ok(canonical_runtime)
}

pub(crate) fn spawn_bounded_log_writer(
    mut stderr: impl Read + Send + 'static,
    mut log_file: File,
) -> Receiver<Result<(), std::io::Error>> {
    let (sender, receiver) = mpsc::sync_channel(1);
    // The receiver waits for a bounded period only. A provider descendant may retain stderr
    // after the direct child exits, so joining this drain thread would reintroduce an unbounded
    // wait. The caller terminates that process group on drain timeout.
    thread::spawn(move || {
        let result = (|| {
            let mut buffer = [0_u8; 8192];
            let mut written = 0_usize;
            let mut truncated = false;
            loop {
                let read = stderr.read(&mut buffer)?;
                if read == 0 {
                    break;
                }
                if written < MAX_CAPABILITY_EXEC_LOG_BYTES {
                    let remaining = MAX_CAPABILITY_EXEC_LOG_BYTES - written;
                    if read <= remaining {
                        let captured = buffer.get(..read).ok_or_else(|| {
                            Error::new(
                                ErrorKind::InvalidData,
                                "stderr reader returned an invalid byte count",
                            )
                        })?;
                        log_file.write_all(captured)?;
                        written += read;
                    } else {
                        let content_budget = remaining.saturating_sub(LOG_TRUNCATION_NOTICE.len());
                        if content_budget > 0 {
                            let captured = buffer.get(..content_budget).ok_or_else(|| {
                                Error::new(
                                    ErrorKind::InvalidData,
                                    "stderr reader returned an invalid byte count",
                                )
                            })?;
                            log_file.write_all(captured)?;
                        }
                        if remaining >= LOG_TRUNCATION_NOTICE.len() {
                            log_file.write_all(LOG_TRUNCATION_NOTICE)?;
                        }
                        written = MAX_CAPABILITY_EXEC_LOG_BYTES;
                        truncated = true;
                    }
                } else {
                    truncated = true;
                }
            }
            if truncated {
                log_file.flush()?;
            }
            Ok(())
        })();
        let _ = sender.send(result);
    });
    receiver
}

fn wait_for_provider_process(
    child: &mut Child,
    provider: &ProviderName,
    binary: &str,
    timeout: Option<Duration>,
) -> Result<ExitStatus, CapabilityExecError> {
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) => {
                if let Some(limit) = timeout
                    && started.elapsed() >= limit
                {
                    terminate_provider_process(child, provider, binary)?;
                    return Err(dispatch_error(
                        provider,
                        format!(
                            "{binary} provider process timed out after {} seconds",
                            limit.as_secs()
                        ),
                    ));
                }
                thread::sleep(PROVIDER_PROCESS_POLL_INTERVAL);
            }
            Err(error) => {
                let poll_detail = format!("cannot poll {binary} provider process: {error}");
                let termination_detail = terminate_provider_process(child, provider, binary)
                    .err()
                    .map(|termination_error| {
                        format!("; provider termination also failed: {termination_error}")
                    })
                    .unwrap_or_default();
                return Err(dispatch_error(provider, format!("{poll_detail}{termination_detail}")));
            }
        }
    }
}

#[path = "process/termination.rs"]
mod termination;

use termination::{
    configure_process_group, terminate_bounded_process_group, terminate_provider_process,
    terminate_provider_process_group,
};

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::io::{Cursor, Error};

    use super::output_collector::{
        MAX_PROVIDER_FINAL_MESSAGE_BYTES, MAX_PROVIDER_SESSION_EVENT_BYTES,
        collect_provider_output, initialize_output_last_message, read_bounded_output_last_message,
        read_output_last_message,
    };
    use super::{collect_bounded_pipe, run_provider_process_with_timeout_inner};
    use usecase::capability_exec::ProviderName;

    #[test]
    fn test_collect_bounded_pipe_discards_excess_after_limit() -> Result<(), std::io::Error> {
        let collected = collect_bounded_pipe(Cursor::new(b"abcdef"), 3)?;

        assert_eq!(collected.bytes, b"abc");
        assert!(collected.exceeded_limit);
        Ok(())
    }

    #[test]
    fn test_provider_session_capture_discards_oversized_event_and_keeps_draining()
    -> Result<(), Error> {
        let mut stdout = vec![b'x'; MAX_PROVIDER_SESSION_EVENT_BYTES.saturating_add(1)];
        stdout.push(b'\n');
        stdout.extend_from_slice(br#"{"thread_id":"captured-session"}"#);
        stdout.push(b'\n');

        let provider = ProviderName::try_new("codex".to_owned())
            .map_err(|error| Error::new(std::io::ErrorKind::InvalidInput, error.to_string()))?;
        assert_eq!(
            collect_provider_output(Cursor::new(stdout), &provider)?.session_id,
            Some("captured-session".to_owned())
        );
        Ok(())
    }

    #[test]
    fn test_provider_process_emits_only_last_message_while_capturing_session_id()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let provider = ProviderName::try_new("codex".to_owned())?;
        let output_last_message = directory.path().join("tmp/runtime/final-message.txt");
        let args = vec![
            OsString::from("-c"),
            OsString::from(
                "printf '{\"session_id\":\"captured-session\",\"event\":\"envelope\"}\\n'; printf 'specialist final report' > \"$1\"",
            ),
            OsString::from("sh"),
            output_last_message.as_os_str().to_owned(),
        ];

        let result = run_provider_process_with_timeout_inner(
            "sh",
            &args,
            directory.path(),
            &directory.path().join("tmp/runtime"),
            &provider,
            None,
            Some(&output_last_message),
        )?;

        assert_eq!(result.exit_code, 0);
        assert_eq!(result.session_id.as_deref(), Some("captured-session"));
        assert_eq!(result.final_message.as_deref(), Some(b"specialist final report".as_slice()));
        Ok(())
    }

    #[test]
    fn test_provider_process_missing_last_message_retains_no_fabricated_output()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let provider = ProviderName::try_new("codex".to_owned())?;
        let output_last_message = directory.path().join("tmp/runtime/final-message.txt");
        let result = run_provider_process_with_timeout_inner(
            "sh",
            &[OsString::from("-c"), OsString::from("printf '{\"event\":\"envelope\"}\\n'")],
            directory.path(),
            &directory.path().join("tmp/runtime"),
            &provider,
            None,
            Some(&output_last_message),
        )?;

        assert_eq!(result.session_id, None);
        assert_eq!(result.final_message, None);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn test_provider_process_rejects_runtime_dir_replaced_with_symlink()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let provider = ProviderName::try_new("codex".to_owned())?;
        let runtime_dir = directory.path().join("tmp/runtime");
        let output_last_message = runtime_dir.join("final-message.txt");
        let outside_dir = directory.path().join("outside");
        std::fs::create_dir_all(&outside_dir)?;
        let args = vec![
            OsString::from("-c"),
            OsString::from(
                "rm -rf \"$1\"; ln -s \"$2\" \"$1\"; printf 'outside runtime' > \"$1/final-message.txt\"",
            ),
            OsString::from("sh"),
            runtime_dir.as_os_str().to_owned(),
            outside_dir.as_os_str().to_owned(),
        ];

        let result = run_provider_process_with_timeout_inner(
            "sh",
            &args,
            directory.path(),
            &runtime_dir,
            &provider,
            None,
            Some(&output_last_message),
        );
        let error = match result {
            Ok(_) => {
                return Err(
                    "replaced runtime directory must be rejected before reading output".into()
                );
            }
            Err(error) => error,
        };

        assert!(error.to_string().contains("refusing to follow symlink"));
        Ok(())
    }

    #[test]
    fn test_read_bounded_output_last_message_over_limit_returns_error()
    -> Result<(), Box<dyn std::error::Error>> {
        let provider = ProviderName::try_new("codex".to_owned())?;
        let path = std::path::Path::new("final-message.txt");
        let reader = Cursor::new(vec![b'x'; MAX_PROVIDER_FINAL_MESSAGE_BYTES as usize + 1]);

        let error = match read_bounded_output_last_message(reader, path, &provider) {
            Ok(_) => return Err("oversized message must be rejected".into()),
            Err(error) => error,
        };

        assert!(error.to_string().contains("exceeds"));
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn test_read_output_last_message_symlink_returns_error()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let target = directory.path().join("provider-output.txt");
        let link = directory.path().join("final-message.txt");
        let provider = ProviderName::try_new("codex".to_owned())?;
        std::fs::write(&target, b"specialist final report")?;
        std::os::unix::fs::symlink(&target, &link)?;

        let error = match read_output_last_message(&link, &provider) {
            Ok(_) => return Err("symlinked message must be rejected".into()),
            Err(error) => error,
        };

        assert!(error.to_string().contains("not a regular file"));
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn test_initialize_output_last_message_symlink_preserves_target()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let runtime_dir = directory.path().join("runtime");
        let target = directory.path().join("provider-output.txt");
        let link = runtime_dir.join("final-message.txt");
        let provider = ProviderName::try_new("codex".to_owned())?;
        std::fs::create_dir_all(&runtime_dir)?;
        std::fs::write(&target, b"preserve this message")?;
        std::os::unix::fs::symlink(&target, &link)?;

        let error = match initialize_output_last_message(&link, &runtime_dir, &provider) {
            Ok(_) => return Err("symlinked message must be rejected".into()),
            Err(error) => error,
        };

        assert!(error.to_string().contains("refusing to follow symlink"));
        assert_eq!(std::fs::read(&target)?, b"preserve this message");
        Ok(())
    }

    #[test]
    fn test_claude_json_envelope_emits_only_result_and_captures_session_id()
    -> Result<(), Box<dyn std::error::Error>> {
        let provider = ProviderName::try_new("claude".to_owned())?;
        let collected = collect_provider_output(
            Cursor::new(
                b"{\"session_id\":\"captured-session\",\"result\":\"specialist final report\"}",
            ),
            &provider,
        )?;

        assert_eq!(collected.session_id.as_deref(), Some("captured-session"));
        assert_eq!(collected.final_message.as_deref(), Some(b"specialist final report".as_slice()));
        Ok(())
    }
}
