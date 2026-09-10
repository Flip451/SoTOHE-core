//! Provider subprocess execution, bounded logging, and process-tree cleanup.

use std::fs::File;
use std::io::{Error, ErrorKind, Read, Write};
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::AtomicU64;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use usecase::capability_exec::{CapabilityExecError, ProviderName};

use super::{MAX_CAPABILITY_EXEC_LOG_BYTES, dispatch_error, path_guard};

const PROVIDER_PROCESS_POLL_INTERVAL: Duration = Duration::from_millis(50);
const PROVIDER_LOG_DRAIN_TIMEOUT: Duration = Duration::from_secs(1);
const BOUNDED_COMMAND_POLL_INTERVAL: Duration = Duration::from_millis(50);
const BOUNDED_COMMAND_DRAIN_TIMEOUT: Duration = Duration::from_secs(1);
const CODEX_VERSION_PROBE_TIMEOUT: Duration = Duration::from_secs(5);
const CODEX_VERSION_PROBE_MAX_OUTPUT_BYTES: usize = 64 * 1024;
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

/// Runs a Grok direct-launch subprocess, draining both pipes to completion.
///
/// Stdout is stream-parsed for session metadata and a bounded final envelope
/// candidate. Cumulative stdout or stderr volume is not a failure.
pub(crate) fn run_grok_direct_command(
    command: &mut Command,
    stderr_maximum_bytes: usize,
    timeout: Duration,
    label: &str,
) -> Result<(std::process::ExitStatus, crate::grok_common::GrokStreamedStdout), std::io::Error> {
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
    let stdout_reader = spawn_grok_stdout_reader(stdout);
    let stderr_reader = spawn_bounded_pipe_reader(stderr, stderr_maximum_bytes);
    let status = wait_for_bounded_command(&mut child, process_id, timeout, label)?;
    let stdout = receive_grok_stdout(stdout_reader, process_id, label)?;
    let _stderr = receive_bounded_pipe(stderr_reader, process_id, label, "stderr")?;
    Ok((status, stdout))
}

fn spawn_grok_stdout_reader(
    pipe: impl Read + Send + 'static,
) -> Receiver<Result<crate::grok_common::GrokStreamedStdout, std::io::Error>> {
    let (sender, receiver) = mpsc::sync_channel(1);
    thread::spawn(move || {
        let _ = sender.send(crate::grok_common::collect_grok_stdout_stream(pipe));
    });
    receiver
}

fn receive_grok_stdout(
    receiver: Receiver<Result<crate::grok_common::GrokStreamedStdout, std::io::Error>>,
    process_id: u32,
    label: &str,
) -> Result<crate::grok_common::GrokStreamedStdout, std::io::Error> {
    match receiver.recv_timeout(BOUNDED_COMMAND_DRAIN_TIMEOUT) {
        Ok(Ok(output)) => Ok(output),
        Ok(Err(error)) => {
            Err(Error::new(error.kind(), format!("cannot read {label} stdout: {error}")))
        }
        Err(RecvTimeoutError::Timeout) => {
            let termination_detail = terminate_bounded_process_group(process_id)
                .err()
                .map(|error| format!("; process-tree termination also failed: {error}"))
                .unwrap_or_default();
            Err(Error::new(
                ErrorKind::TimedOut,
                format!(
                    "{label} stdout reader did not close within {} seconds after the subprocess exited{termination_detail}",
                    BOUNDED_COMMAND_DRAIN_TIMEOUT.as_secs()
                ),
            ))
        }
        Err(RecvTimeoutError::Disconnected) => {
            Err(Error::other(format!("{label} stdout reader thread disconnected")))
        }
    }
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

#[path = "process/reviewer_lifecycle.rs"]
mod reviewer_lifecycle;

pub(crate) use reviewer_lifecycle::{
    ProviderProcessFailure, ProviderProcessOutput, ProviderProcessRunner, ReviewerProcessOutput,
    system_process_runner,
};

#[cfg(test)]
pub(crate) use reviewer_lifecycle::{
    run_provider_process_with_timeout, run_provider_process_with_timeout_inner,
};

enum ProviderProcessWaitFailure {
    Timeout(CapabilityExecError),
    Failed(CapabilityExecError),
}

fn apply_path_prefix(
    command: &mut Command,
    path_prefix: Option<&Path>,
    provider: &ProviderName,
) -> Result<(), CapabilityExecError> {
    let Some(prefix) = path_prefix else {
        return Ok(());
    };

    let mut paths = vec![prefix.to_path_buf()];
    if let Some(existing) = std::env::var_os("PATH") {
        if !existing.is_empty() {
            paths.extend(std::env::split_paths(&existing));
        }
    }
    let path = std::env::join_paths(paths).map_err(|error| {
        dispatch_error(
            provider,
            format!("cannot prepend {} to provider PATH: {error}", prefix.display()),
        )
    })?;
    command.env("PATH", path);
    Ok(())
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
#[allow(dead_code)]
mod output_collector;

use output_collector::{receive_provider_output, spawn_provider_session_collector};

#[path = "process/runtime_dir.rs"]
mod runtime_dir;

use runtime_dir::{
    open_runtime_file, prepare_output_last_message, prepare_runtime_dir,
    read_output_last_message_at,
};

#[cfg(test)]
use runtime_dir::RuntimeOutputLastMessage;
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
            let maximum_capture = MAX_CAPABILITY_EXEC_LOG_BYTES.saturating_add(1);
            let mut captured = Vec::with_capacity(maximum_capture);
            let mut exceeded_limit = false;
            let mut invalid_read = false;
            loop {
                let read = match stderr.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(read) => read,
                    Err(_) => {
                        invalid_read = true;
                        break;
                    }
                };
                let Some(bytes) = buffer.get(..read) else {
                    invalid_read = true;
                    break;
                };
                let remaining = maximum_capture.saturating_sub(captured.len());
                let retained = remaining.min(bytes.len());
                if let Some(bytes) = bytes.get(..retained) {
                    captured.extend_from_slice(bytes);
                }
                exceeded_limit |= retained < bytes.len();
            }

            let rendered =
                if invalid_read || exceeded_limit || captured.len() > MAX_CAPABILITY_EXEC_LOG_BYTES
                {
                    "diagnostic_unavailable".to_owned()
                } else {
                    std::str::from_utf8(&captured)
                        .ok()
                        .map(crate::review_v2::review_fix_runner::redact_credentials)
                        .filter(|text| text.len() <= MAX_CAPABILITY_EXEC_LOG_BYTES)
                        .unwrap_or_else(|| "diagnostic_unavailable".to_owned())
                };
            log_file.write_all(rendered.as_bytes())?;
            log_file.flush()?;
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
) -> Result<ExitStatus, ProviderProcessWaitFailure> {
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) => {
                if let Some(limit) = timeout
                    && started.elapsed() >= limit
                {
                    let termination_detail = terminate_provider_process(child, provider, binary)
                        .err()
                        .map(|termination_error| {
                            format!("; provider termination also failed: {termination_error}")
                        })
                        .unwrap_or_default();
                    return Err(ProviderProcessWaitFailure::Timeout(dispatch_error(
                        provider,
                        format!(
                            "{binary} provider process timed out after {} seconds{termination_detail}",
                            limit.as_secs()
                        ),
                    )));
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
                return Err(ProviderProcessWaitFailure::Failed(dispatch_error(
                    provider,
                    format!("{poll_detail}{termination_detail}"),
                )));
            }
        }
    }
}

#[path = "process/termination.rs"]
mod termination;

pub(crate) use termination::{configure_process_group, terminate_bounded_process_group};
use termination::{terminate_provider_process, terminate_provider_process_group};

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use std::ffi::OsString;
    use std::fs::File;
    use std::io::{Cursor, Error};
    use std::time::Duration;

    use super::output_collector::{
        MAX_PROVIDER_FINAL_MESSAGE_BYTES, MAX_PROVIDER_SESSION_EVENT_BYTES,
        collect_provider_output, initialize_output_last_message, read_bounded_output_last_message,
        read_output_last_message,
    };
    use super::{
        MAX_CAPABILITY_EXEC_LOG_BYTES, ProviderProcessFailure, RuntimeOutputLastMessage,
        collect_bounded_pipe, prepare_runtime_dir, read_output_last_message_at,
        run_provider_process_with_timeout_inner, spawn_bounded_log_writer, system_process_runner,
    };
    use usecase::capability_exec::ProviderName;

    #[test]
    fn test_collect_bounded_pipe_discards_excess_after_limit() -> Result<(), std::io::Error> {
        let collected = collect_bounded_pipe(Cursor::new(b"abcdef"), 3)?;

        assert_eq!(collected.bytes, b"abc");
        assert!(collected.exceeded_limit);
        Ok(())
    }

    #[test]
    fn test_bounded_log_writer_redacts_credentials_before_persisting()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("provider.log");
        let secret = "xai-streaming-redaction-secret";

        temp_env::with_vars([("XAI_API_KEY", Some(secret))], || {
            let file = File::create(&path)?;
            let stderr = Cursor::new(format!("before {secret} after").into_bytes());
            spawn_bounded_log_writer(stderr, file)
                .recv_timeout(Duration::from_secs(1))
                .map_err(|_| "bounded log writer did not complete")??;

            let log = std::fs::read_to_string(&path)?;
            assert!(!log.contains(secret));
            assert!(log.contains("[REDACTED:XAI_API_KEY]"));
            Ok::<(), Box<dyn std::error::Error>>(())
        })?;
        Ok(())
    }

    #[test]
    fn test_bounded_log_writer_over_limit_fails_closed_to_unavailable_marker()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("provider.log");
        let file = File::create(&path)?;
        let stderr = Cursor::new(vec![b'x'; MAX_CAPABILITY_EXEC_LOG_BYTES + 1]);

        spawn_bounded_log_writer(stderr, file)
            .recv_timeout(Duration::from_secs(1))
            .map_err(|_| "bounded log writer did not complete")??;

        assert_eq!(std::fs::read_to_string(path)?, "diagnostic_unavailable");
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn test_system_reviewer_process_invalid_binary_is_pre_spawn_failure()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let provider = ProviderName::try_new("grok".to_owned())?;
        let binary = directory.path().join("missing-provider").display().to_string();
        let runtime_dir = directory.path().join("runtime");

        let failure = system_process_runner()
            .run_reviewer_with_lifecycle(
                &binary,
                None,
                &[],
                directory.path(),
                &runtime_dir,
                &provider,
                None,
                None,
            )
            .expect_err("invalid binary must fail before spawn");

        assert!(matches!(failure, ProviderProcessFailure::PreSpawn(_)));
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn test_system_reviewer_process_signal_exit_preserves_unavailable_status()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let provider = ProviderName::try_new("grok".to_owned())?;
        let runtime_dir = directory.path().join("runtime");
        let args = [OsString::from("-c"), OsString::from("kill -TERM $$")];

        let output = system_process_runner()
            .run_reviewer_with_lifecycle(
                "sh",
                None,
                &args,
                directory.path(),
                &runtime_dir,
                &provider,
                None,
                None,
            )
            .expect("signal-terminated reviewer process should return an output");

        assert_eq!(output.exit_code, None);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn test_system_reviewer_process_timeout_is_typed_post_spawn_failure()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let provider = ProviderName::try_new("grok".to_owned())?;
        let runtime_dir = directory.path().join("runtime");
        let args = [OsString::from("-c"), OsString::from("sleep 60")];

        let failure = system_process_runner()
            .run_reviewer_with_lifecycle(
                "sh",
                None,
                &args,
                directory.path(),
                &runtime_dir,
                &provider,
                Some(Duration::from_millis(100)),
                None,
            )
            .expect_err("timed-out child must be classified at the process boundary");

        assert!(matches!(failure, ProviderProcessFailure::Timeout(_)));
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
            None,
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
            None,
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
            None,
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

    #[cfg(unix)]
    #[test]
    fn test_prepare_runtime_dir_rejects_parent_component_before_normalization()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let provider = ProviderName::try_new("codex".to_owned())?;

        let error = match prepare_runtime_dir(
            directory.path(),
            &directory.path().join("ordinary/../runtime"),
            &provider,
        ) {
            Ok(_) => return Err("runtime path must reject a parent component".into()),
            Err(error) => error,
        };

        assert!(error.to_string().contains("parent traversal"));
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn test_read_output_last_message_fifo_returns_error_without_blocking()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let provider = ProviderName::try_new("codex".to_owned())?;
        let runtime_dir = directory.path().join("runtime");
        let output_path = runtime_dir.join("final-message.txt");
        let runtime = prepare_runtime_dir(directory.path(), &runtime_dir, &provider)?;
        rustix::fs::mkfifoat(
            &runtime.directory,
            "final-message.txt",
            rustix::fs::Mode::from_raw_mode(0o600),
        )?;
        let output = RuntimeOutputLastMessage {
            name: OsString::from("final-message.txt"),
            path: output_path,
        };

        let error = match read_output_last_message_at(&runtime, &output, &provider) {
            Ok(_) => return Err("a provider-created FIFO must be rejected".into()),
            Err(error) => error,
        };

        assert!(error.to_string().contains("not a regular file"));
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn test_provider_process_path_prefix_reaches_codex_child()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let provider = ProviderName::try_new("codex".to_owned())?;
        let prefix = directory.path().join("public-npm-bin");
        let output_last_message = directory.path().join("tmp/runtime/final-message.txt");
        std::fs::create_dir_all(&prefix)?;
        let args = [
            OsString::from("-c"),
            OsString::from("printf '%s' \"$PATH\" > \"$1\""),
            OsString::from("sh"),
            output_last_message.as_os_str().to_owned(),
        ];

        let result = run_provider_process_with_timeout_inner(
            "sh",
            Some(&prefix),
            &args,
            directory.path(),
            &directory.path().join("tmp/runtime"),
            &provider,
            None,
            Some(&output_last_message),
        )?;

        let path = std::ffi::OsString::from(String::from_utf8(
            result.final_message.ok_or("provider child must write its PATH")?,
        )?);
        let first = std::env::split_paths(&path).next();
        assert_eq!(first.as_deref(), Some(prefix.as_path()));
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

    #[test]
    fn test_grok_json_envelope_captures_structured_output_and_camel_case_session_id()
    -> Result<(), Box<dyn std::error::Error>> {
        let provider = ProviderName::try_new("grok".to_owned())?;
        let collected = collect_provider_output(
            Cursor::new(
                br#"{"sessionId":"grok-session","structured_output":{"status":"ok"},"text":"ignore-me"}"#,
            ),
            &provider,
        )?;

        assert_eq!(collected.session_id.as_deref(), Some("grok-session"));
        assert_eq!(
            collected.final_message.as_deref(),
            Some(br#"{"sessionId":"grok-session","structured_output":{"status":"ok"},"text":"ignore-me"}"#.as_slice()),
        );
        Ok(())
    }

    #[test]
    fn test_grok_session_id_from_earlier_event_is_preserved()
    -> Result<(), Box<dyn std::error::Error>> {
        let provider = ProviderName::try_new("grok".to_owned())?;
        let stdout = concat!(
            "{\"type\":\"start\",\"sessionId\":\"early-session\"}\n",
            "{\"structured_output\":{\"status\":\"ok\"}}\n",
        );
        let collected = collect_provider_output(Cursor::new(stdout.as_bytes()), &provider)?;
        assert_eq!(collected.session_id.as_deref(), Some("early-session"));
        assert_eq!(
            collected.final_message.as_deref(),
            Some(br#"{"structured_output":{"status":"ok"}}"#.as_slice()),
        );
        Ok(())
    }

    #[test]
    fn test_grok_pretty_printed_camel_case_structured_output_is_collected()
    -> Result<(), Box<dyn std::error::Error>> {
        let provider = ProviderName::try_new("grok".to_owned())?;
        let stdout = br#"{
  "text": "{\"result\": \"OK\"}",
  "sessionId": "grok-session",
  "structuredOutput": {
    "result": "OK"
  }
}
"#;
        let collected = collect_provider_output(Cursor::new(stdout.as_slice()), &provider)?;
        assert_eq!(collected.session_id.as_deref(), Some("grok-session"));
        let message = collected.final_message.ok_or("envelope")?;
        let envelope: crate::grok_common::GrokOutputEnvelope = serde_json::from_slice(&message)?;
        assert_eq!(envelope.into_structured_output()?, serde_json::json!({"result": "OK"}),);
        Ok(())
    }

    #[test]
    fn test_grok_json_envelope_above_session_event_limit_is_retained()
    -> Result<(), Box<dyn std::error::Error>> {
        let provider = ProviderName::try_new("grok".to_owned())?;
        let event = serde_json::json!({
            "structured_output": "x".repeat(MAX_PROVIDER_SESSION_EVENT_BYTES),
        });
        let encoded = serde_json::to_vec(&event)?;
        assert!(encoded.len() > MAX_PROVIDER_SESSION_EVENT_BYTES);

        let collected = collect_provider_output(Cursor::new(&encoded), &provider)?;

        assert_eq!(collected.final_message.as_deref(), Some(encoded.as_slice()));
        Ok(())
    }

    #[test]
    fn test_grok_stdout_over_limit_is_rejected_after_draining()
    -> Result<(), Box<dyn std::error::Error>> {
        let provider = ProviderName::try_new("grok".to_owned())?;
        let envelope = br#"{"sessionId":"grok-session","structured_output":{"status":"ok"}}"#;
        let mut stdout = vec![b'x'; MAX_PROVIDER_FINAL_MESSAGE_BYTES as usize + 64];
        stdout.splice(..envelope.len(), envelope.iter().copied());

        let mut cursor = Cursor::new(stdout);
        let error = match collect_provider_output(&mut cursor, &provider) {
            Ok(_) => return Err("oversized grok stdout must be rejected".into()),
            Err(error) => error,
        };

        assert!(error.to_string().contains("exceeds"));
        assert_eq!(cursor.position() as usize, cursor.get_ref().len());
        Ok(())
    }
}
