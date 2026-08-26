//! Trusted launch context for every review-fix provider.
//!
//! This is the sole review-fix boundary that accepts a resolver-proven root
//! and a caller-supplied briefing path. It defends against path escape,
//! symlink traversal, bounded output handling, and runtime-directory substitution.
//! It cannot defend against a provider that intentionally ignores its prompt
//! or writes arbitrary content inside the already trusted runtime directory.

use std::collections::VecDeque;
use std::ffi::{OsStr, OsString};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use usecase::review_v2::run_review_fix::ReviewFixRunnerError;

use super::session_log::{
    credential_redaction_overlap_bytes, credential_values, redact_credential_values,
};
use super::spawn::{RuntimeFile, create_runtime_file, read_runtime_file_bounded};

const MAX_RETAINED_CHILD_OUTPUT_BYTES: usize = 64 * 1024;
/// Maximum output retained when a child stream is used as validation input.
///
/// Diagnostic review-fix streams use the separate tail collector below and do
/// not apply this fail-closed limit.
pub(super) const MAX_CHILD_TOTAL_OUTPUT_BYTES: usize = 1024 * 1024;
pub(super) const VERSION_PROBE_TIMEOUT: Duration = Duration::from_secs(30);
pub(super) const FIXER_RUNTIME_TIMEOUT: Duration = Duration::from_secs(60 * 60);
const CHILD_OUTPUT_CHUNK_BYTES: usize = 8 * 1024;
const TRUNCATION_MARKER: &[u8] = b"\n[review-fix child output truncated]\n";
const RETAINED_CHILD_OUTPUT_SUFFIX_BYTES: usize = 16 * 1024;
const RETAINED_CHILD_OUTPUT_PREFIX_BYTES: usize =
    MAX_RETAINED_CHILD_OUTPUT_BYTES - TRUNCATION_MARKER.len() - RETAINED_CHILD_OUTPUT_SUFFIX_BYTES;
const TAIL_RING_CONTENT_BYTES: usize = MAX_RETAINED_CHILD_OUTPUT_BYTES - TRUNCATION_MARKER.len();
const COLLECTOR_DRAIN_TIMEOUT: Duration = Duration::from_secs(1);

pub(crate) struct TrustedLaunchContext {
    pub(super) repository_root: PathBuf,
}

impl TrustedLaunchContext {
    pub(crate) fn for_repository(repository_root: &Path) -> Result<Self, ReviewFixRunnerError> {
        let repository_root = repository_root.canonicalize().map_err(|error| {
            unexpected(format!(
                "failed to canonicalize resolver-proven repository root {}: {error}",
                repository_root.display()
            ))
        })?;
        Ok(Self { repository_root })
    }

    pub(super) fn create_runtime_file(
        &self,
        prefix: &str,
        extension: &str,
    ) -> Result<RuntimeFile, ReviewFixRunnerError> {
        create_runtime_file(&self.repository_root, prefix, extension)
    }

    pub(super) fn read_runtime_file_bounded(
        &self,
        file: &RuntimeFile,
        maximum_bytes: u64,
    ) -> Result<String, ReviewFixRunnerError> {
        read_runtime_file_bounded(file, maximum_bytes)
    }

    /// Runs a provider version probe while retaining only bounded diagnostics.
    ///
    /// Both output pipes are drained to completion so a noisy provider cannot
    /// deadlock the probe, but only a capped prefix is kept for semver parsing
    /// and error reporting.
    pub(super) fn run_version_probe(
        &self,
        bin: &OsStr,
        safe_env: &[(OsString, OsString)],
    ) -> Result<(ExitStatus, String), ReviewFixRunnerError> {
        let mut command = Command::new(bin);
        command.arg("--version");
        command.current_dir(&self.repository_root);
        command.env_clear();
        for (key, value) in safe_env {
            command.env(key, value);
        }
        command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        crate::capability_exec::process::configure_process_group(&mut command);
        let mut child = command
            .spawn()
            .map_err(|error| unexpected(format!("failed to start codex --version: {error}")))?;
        let process_id = child.id();
        let stdout = child.stdout.take();
        let stdout_collector = spawn_child_output_collector(stdout, false, "stdout");
        let stderr = child.stderr.take();
        let stderr_collector = spawn_child_output_collector(stderr, false, "stderr");
        let status =
            wait_for_child_with_timeout(&mut child, VERSION_PROBE_TIMEOUT, "codex --version");
        let stdout = receive_child_output_collector(
            stdout_collector,
            process_id,
            "codex --version",
            "stdout",
        )?;
        let stderr = receive_child_output_collector(
            stderr_collector,
            process_id,
            "codex --version",
            "stderr",
        )?;
        let status = status.map_err(unexpected)?;
        Ok((status, format!("{stdout}{stderr}")))
    }
}

/// Waits for a child with a hard deadline, killing it before returning a timeout.
pub(super) fn wait_for_child_with_timeout(
    child: &mut Child,
    timeout: Duration,
    label: &str,
) -> Result<ExitStatus, String> {
    wait_for_child_with_timeout_or_cancellation(child, timeout, label, None)
}

/// Waits for a child with a hard deadline or a collector cancellation signal.
pub(super) fn wait_for_child_with_timeout_or_cancellation(
    child: &mut Child,
    timeout: Duration,
    label: &str,
    cancellation: Option<&mpsc::Receiver<String>>,
) -> Result<ExitStatus, String> {
    let deadline = Instant::now().checked_add(timeout).unwrap_or_else(Instant::now);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                // A successful direct child can leave descendants in its
                // process group even after every output pipe closes. They
                // must not outlive this adapter and mutate the repository.
                // Windows has no process-group handle here; dead-PID cleanup is tolerated, so post-exit orphan cleanup is best effort.
                crate::capability_exec::process::terminate_bounded_process_group(child.id())
                    .map_err(|error| {
                        format!(
                            "{label} direct child exited but its process group could not be terminated: {error}"
                        )
                    })?;
                return Ok(status);
            }
            Ok(None) => {
                if let Some(reason) = cancellation.and_then(|receiver| receiver.try_recv().ok()) {
                    let (kill_result, wait_result) = terminate_child_process_group(child);
                    return Err(format!(
                        "{label} was terminated after {reason}; kill={kill_result:?}; wait={wait_result:?}"
                    ));
                }
                if Instant::now() < deadline {
                    thread::sleep(Duration::from_millis(10));
                    continue;
                }
                let (kill_result, wait_result) = terminate_child_process_group(child);
                return Err(format!(
                    "{label} exceeded its {}-second runtime limit; kill={kill_result:?}; wait={wait_result:?}",
                    timeout.as_secs()
                ));
            }
            Err(error) => {
                let (kill_result, wait_result) = terminate_child_process_group(child);
                return Err(format!(
                    "failed to poll {label}: {error}; kill={kill_result:?}; wait={wait_result:?}"
                ));
            }
        }
    }
}

fn terminate_child_process_group(
    child: &mut Child,
) -> (Result<(), std::io::Error>, std::io::Result<ExitStatus>) {
    let process_id = child.id();
    let kill_result = crate::capability_exec::process::terminate_bounded_process_group(process_id)
        .or_else(|group_error| {
            child.kill().map_err(|child_error| {
                std::io::Error::other(format!(
                    "process-group termination failed: {group_error}; child termination failed: {child_error}"
                ))
            })
        });
    let wait_result = child.wait();
    (kill_result, wait_result)
}

/// Drains one child stream with the fail-closed bound used for validation input.
#[cfg(test)]
pub(super) fn collect_child_output_bounded<R: Read>(
    pipe: Option<R>,
    echo_to_stderr: bool,
    stream_name: &str,
) -> Result<String, String> {
    collect_child_output_fail_closed(pipe, echo_to_stderr, stream_name)
}

/// Starts a pipe collector for output that is consumed as validation input.
pub(super) fn spawn_child_output_collector<R: Read + Send + 'static>(
    pipe: Option<R>,
    echo_to_stderr: bool,
    stream_name: &'static str,
) -> mpsc::Receiver<Result<String, String>> {
    let (sender, receiver) = mpsc::sync_channel(1);
    thread::spawn(move || {
        let _ = sender.send(collect_child_output_fail_closed(pipe, echo_to_stderr, stream_name));
    });
    receiver
}

/// Starts a pipe collector for review-fix diagnostic output.
pub(super) fn spawn_child_output_tail_collector<R: Read + Send + 'static>(
    pipe: Option<R>,
    echo_to_stderr: bool,
    stream_name: &'static str,
) -> mpsc::Receiver<Result<String, String>> {
    let (sender, receiver) = mpsc::sync_channel(1);
    thread::spawn(move || {
        let _ = sender.send(collect_child_output_tail_impl(pipe, echo_to_stderr, stream_name));
    });
    receiver
}

/// Drains one child stream, rejecting excess output because it is validation input.
fn collect_child_output_fail_closed<R: Read>(
    mut pipe: Option<R>,
    echo_to_stderr: bool,
    stream_name: &str,
) -> Result<String, String> {
    let mut collected = Vec::with_capacity(MAX_CHILD_TOTAL_OUTPUT_BYTES);
    let mut buffer = [0_u8; CHILD_OUTPUT_CHUNK_BYTES];
    let mut total_read = 0_usize;
    // Retain the full next pipe chunk as well as the longest secret. This
    // ensures a credential that ends before a chunk's final delimiter is
    // still redacted together with its complete value before any prefix is
    // echoed.
    let redaction_overlap =
        credential_redaction_overlap_bytes().saturating_add(CHILD_OUTPUT_CHUNK_BYTES);
    let credentials = credential_values();
    let mut echo_pending = Vec::with_capacity(redaction_overlap);
    while let Some(reader) = pipe.as_mut() {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| format!("failed to read codex fixer {stream_name}: {error}"))?;
        if read == 0 {
            break;
        }
        total_read = total_read.saturating_add(read);
        if total_read > MAX_CHILD_TOTAL_OUTPUT_BYTES {
            return Err(format!(
                "codex fixer {stream_name} exceeded the {}-byte output limit",
                MAX_CHILD_TOTAL_OUTPUT_BYTES
            ));
        }
        let chunk = buffer.get(..read).ok_or_else(|| {
            format!(
                "codex fixer {stream_name} reader returned {read} bytes for a {}-byte buffer",
                buffer.len()
            )
        })?;
        collected.extend_from_slice(chunk);
        if echo_to_stderr {
            redact_and_emit_prefix(
                &mut echo_pending,
                chunk,
                redaction_overlap,
                &credentials,
                |text| eprint!("{text}"),
            );
        }
    }
    if echo_to_stderr {
        flush_redacted(&mut echo_pending, &credentials, |text| eprint!("{text}"));
    }
    // Redact the complete validation-bounded stream before choosing retained
    // prefix and suffix bytes. Otherwise a credential spanning the truncation
    // point can leave an unredactable fragment in a persistent session log.
    Ok(redact_and_retain_child_output(&collected, &credentials))
}

/// Drains one diagnostic child stream while retaining only its recent tail.
///
/// The collector always drains the pipe to EOF. Reaching the retention
/// capacity records truncation in the returned diagnostic, but never signals
/// the supervisor to terminate the child.
#[cfg(test)]
pub(super) fn collect_child_output_tail<R: Read>(
    pipe: Option<R>,
    echo_to_stderr: bool,
    stream_name: &str,
) -> Result<String, String> {
    collect_child_output_tail_impl(pipe, echo_to_stderr, stream_name)
}

fn collect_child_output_tail_impl<R: Read>(
    mut pipe: Option<R>,
    echo_to_stderr: bool,
    stream_name: &str,
) -> Result<String, String> {
    let mut retained = TailRing::new(TAIL_RING_CONTENT_BYTES);
    let mut buffer = [0_u8; CHILD_OUTPUT_CHUNK_BYTES];
    let redaction_overlap =
        credential_redaction_overlap_bytes().saturating_add(CHILD_OUTPUT_CHUNK_BYTES);
    let credentials = credential_values();
    let mut pending = Vec::with_capacity(redaction_overlap);
    let mut capture = |text: &str| {
        retained.push_bytes(text.as_bytes());
        if echo_to_stderr {
            eprint!("{text}");
        }
    };

    while let Some(reader) = pipe.as_mut() {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| format!("failed to read codex fixer {stream_name}: {error}"))?;
        if read == 0 {
            break;
        }
        let chunk = buffer.get(..read).ok_or_else(|| {
            format!(
                "codex fixer {stream_name} reader returned {read} bytes for a {}-byte buffer",
                buffer.len()
            )
        })?;
        redact_and_emit_prefix(&mut pending, chunk, redaction_overlap, &credentials, &mut capture);
    }
    flush_redacted(&mut pending, &credentials, &mut capture);
    Ok(retained.finish())
}

fn redact_and_retain_child_output(output: &[u8], credentials: &[(&str, String)]) -> String {
    let redacted = redact_credential_values(
        &String::from_utf8_lossy(output),
        credentials.iter().map(|(name, value)| (*name, value.clone())),
    );
    retain_bounded_child_output(redacted.as_bytes())
}

fn retain_bounded_child_output(output: &[u8]) -> String {
    if output.len() <= MAX_RETAINED_CHILD_OUTPUT_BYTES {
        return String::from_utf8_lossy(output).into_owned();
    }
    let prefix = output.get(..RETAINED_CHILD_OUTPUT_PREFIX_BYTES).unwrap_or_default();
    let suffix_start = output.len().saturating_sub(RETAINED_CHILD_OUTPUT_SUFFIX_BYTES);
    let suffix = output.get(suffix_start..).unwrap_or_default();
    let mut retained = Vec::with_capacity(MAX_RETAINED_CHILD_OUTPUT_BYTES);
    retained.extend_from_slice(prefix);
    retained.extend_from_slice(TRUNCATION_MARKER);
    retained.extend_from_slice(suffix);
    String::from_utf8_lossy(&retained).into_owned()
}

struct TailRing {
    bytes: VecDeque<u8>,
    capacity: usize,
    truncated: bool,
}

impl TailRing {
    fn new(capacity: usize) -> Self {
        Self { bytes: VecDeque::with_capacity(capacity), capacity, truncated: false }
    }

    fn push_bytes(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        if self.capacity == 0 {
            self.truncated = true;
            return;
        }
        if bytes.len() >= self.capacity {
            self.bytes.clear();
            let start = bytes.len().saturating_sub(self.capacity);
            if let Some(tail) = bytes.get(start..) {
                self.bytes.extend(tail.iter().copied());
            }
            self.truncated = true;
            return;
        }
        let overflow = self.bytes.len().saturating_add(bytes.len()).saturating_sub(self.capacity);
        for _ in 0..overflow {
            let _ = self.bytes.pop_front();
        }
        if overflow > 0 {
            self.truncated = true;
        }
        self.bytes.extend(bytes.iter().copied());
    }

    fn finish(self) -> String {
        let marker_bytes = if self.truncated { TRUNCATION_MARKER.len() } else { 0 };
        let mut output = Vec::with_capacity(self.bytes.len() + marker_bytes);
        if self.truncated {
            output.extend_from_slice(TRUNCATION_MARKER);
        }
        output.extend(self.bytes);
        String::from_utf8_lossy(&output).into_owned()
    }
}

fn redact_and_emit_prefix(
    pending: &mut Vec<u8>,
    bytes: &[u8],
    overlap: usize,
    credentials: &[(&str, String)],
    mut emit: impl FnMut(&str),
) {
    pending.extend_from_slice(bytes);
    let emitted_len = pending.len().saturating_sub(overlap);
    if emitted_len == 0 {
        return;
    }
    let safe_len = safe_redaction_emission_boundary(pending, emitted_len, credentials);
    let prefix = pending.get(..safe_len).unwrap_or_default();
    let prefix_redacted = redact_credential_values(
        &String::from_utf8_lossy(prefix),
        credentials.iter().map(|(name, value)| (*name, value.clone())),
    );
    emit(&prefix_redacted);
    let tail = pending.split_off(safe_len.min(pending.len()));
    *pending = tail;
}

fn flush_redacted(
    pending: &mut Vec<u8>,
    credentials: &[(&str, String)],
    mut emit: impl FnMut(&str),
) {
    if pending.is_empty() {
        return;
    }
    let emitted = redact_credential_values(
        &String::from_utf8_lossy(pending),
        credentials.iter().map(|(name, value)| (*name, value.clone())),
    );
    emit(&emitted);
    pending.clear();
}

/// Chooses an emission point that cannot split a credential value. Besides a
/// complete value crossing the requested boundary, this also catches a prefix
/// that reaches the currently buffered tail. The latter is crucial: values
/// beginning with a literal redaction placeholder can otherwise pass a
/// prefix-comparison check and leak before their final byte arrives.
fn safe_redaction_emission_boundary(
    pending: &[u8],
    requested: usize,
    credentials: &[(&str, String)],
) -> usize {
    let mut safe = requested.min(pending.len());
    for (_, credential) in credentials {
        let secret = credential.as_bytes();
        if secret.is_empty() {
            continue;
        }
        let scan_limit = safe;
        for start in 0..scan_limit {
            let available = pending.len().saturating_sub(start).min(secret.len());
            let Some(candidate) = pending.get(start..start.saturating_add(available)) else {
                continue;
            };
            let Some(prefix) = secret.get(..available) else {
                continue;
            };
            if available == 0 || candidate != prefix {
                continue;
            }
            let ends_at_tail = start.saturating_add(available) == pending.len();
            let crosses_boundary =
                available == secret.len() && start.saturating_add(secret.len()) > safe;
            if crosses_boundary || (available < secret.len() && ends_at_tail) {
                safe = safe.min(start);
            }
        }
    }
    safe
}

pub(super) fn receive_child_output_collector(
    receiver: mpsc::Receiver<Result<String, String>>,
    process_id: u32,
    label: &str,
    stream_name: &str,
) -> Result<String, ReviewFixRunnerError> {
    match receiver.recv_timeout(COLLECTOR_DRAIN_TIMEOUT) {
        Ok(Ok(output)) => Ok(output),
        Ok(Err(error)) => Err(unexpected(error)),
        Err(mpsc::RecvTimeoutError::Timeout) => {
            let termination_detail =
                crate::capability_exec::process::terminate_bounded_process_group(process_id)
                    .err()
                    .map(|error| format!("; process-group termination also failed: {error}"))
                    .unwrap_or_default();
            Err(unexpected(format!(
                "{label} {stream_name} collector did not close within {} seconds after the direct child exited{termination_detail}",
                COLLECTOR_DRAIN_TIMEOUT.as_secs()
            )))
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            Err(unexpected(format!("{label} {stream_name} collector thread disconnected")))
        }
    }
}

pub(super) fn prompt_path_string(path: &Path, label: &str) -> Result<String, ReviewFixRunnerError> {
    let raw =
        path.to_str().ok_or_else(|| unexpected(format!("{label} path is not valid UTF-8")))?;
    if raw.is_empty()
        || raw.chars().any(|c| c == '`' || c.is_control() || matches!(c, '\u{2028}' | '\u{2029}'))
    {
        return Err(unexpected(format!(
            "{label} path contains characters that are unsafe in the fixer prompt"
        )));
    }
    Ok(raw.to_owned())
}

fn unexpected(detail: impl Into<String>) -> ReviewFixRunnerError {
    ReviewFixRunnerError::Unexpected(usecase::git_workflow::DiagnosticText::new(detail.into()))
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use std::ffi::OsString;
    use std::io::Cursor;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    #[cfg(unix)]
    use std::process::{Command, Stdio};
    use std::sync::mpsc;
    #[cfg(unix)]
    use std::time::{Duration, Instant};

    use super::{
        MAX_CHILD_TOTAL_OUTPUT_BYTES, MAX_RETAINED_CHILD_OUTPUT_BYTES,
        RETAINED_CHILD_OUTPUT_PREFIX_BYTES, TrustedLaunchContext, collect_child_output_bounded,
        collect_child_output_tail, flush_redacted, redact_and_emit_prefix,
        redact_and_retain_child_output, wait_for_child_with_timeout_or_cancellation,
    };

    #[test]
    fn test_collect_child_output_bounded_caps_newline_free_stream() {
        let input = vec![b'x'; MAX_RETAINED_CHILD_OUTPUT_BYTES * 2];

        let output = collect_child_output_bounded(Some(Cursor::new(input)), false, "stdout")
            .expect("bounded collector must drain a large newline-free stream");

        assert!(output.len() <= MAX_RETAINED_CHILD_OUTPUT_BYTES);
        assert!(output.contains("output truncated"));
    }

    #[test]
    fn test_collect_child_output_bounded_retains_final_status_after_cap() {
        let sentinel = b"\nREVIEW_FIX_STATUS: completed\n";
        let mut input = vec![b'x'; MAX_RETAINED_CHILD_OUTPUT_BYTES + 1];
        input.extend_from_slice(sentinel);

        let output = collect_child_output_bounded(Some(Cursor::new(input)), false, "stdout")
            .expect("bounded collector must retain the final status sentinel");

        assert!(output.len() <= MAX_RETAINED_CHILD_OUTPUT_BYTES);
        assert!(output.contains("output truncated"));
        assert!(output.contains("REVIEW_FIX_STATUS: completed"));
    }

    #[test]
    fn test_retained_output_redacts_credential_crossing_truncation_boundary() {
        let secret = "sk-crosses-retained-output-boundary";
        let prefix_len = RETAINED_CHILD_OUTPUT_PREFIX_BYTES.saturating_sub(secret.len() / 2);
        let mut output = vec![b'x'; prefix_len];
        output.extend_from_slice(secret.as_bytes());
        output.extend(std::iter::repeat_n(b'y', MAX_RETAINED_CHILD_OUTPUT_BYTES));

        let retained =
            redact_and_retain_child_output(&output, &[("OPENAI_API_KEY", secret.to_owned())]);

        assert!(!retained.contains(secret), "retained output must not leak a secret fragment");
        assert!(retained.contains("output truncated"));
    }

    #[test]
    fn test_collect_child_output_bounded_rejects_total_output_above_limit() {
        let input = vec![b'x'; MAX_CHILD_TOTAL_OUTPUT_BYTES + 1];

        let result = collect_child_output_bounded(Some(Cursor::new(input)), false, "stdout");

        assert!(result.is_err(), "total child output above the limit must be rejected");
    }

    #[test]
    fn test_collect_child_output_tail_drains_past_validation_limit() {
        let mut input = b"early diagnostic output\n".to_vec();
        input.extend(std::iter::repeat_n(b'x', MAX_CHILD_TOTAL_OUTPUT_BYTES + 1));
        input.extend_from_slice(b"latest diagnostic output\n");

        let output = collect_child_output_tail(Some(Cursor::new(input)), false, "stdout")
            .expect("diagnostic output must continue past the validation limit");

        assert!(output.len() <= MAX_RETAINED_CHILD_OUTPUT_BYTES);
        assert!(output.starts_with("\n[review-fix child output truncated]\n"));
        assert!(output.contains("latest diagnostic output"));
        assert!(!output.contains("early diagnostic output"));
    }

    #[cfg(unix)]
    #[test]
    fn test_wait_for_child_cancellation_terminates_process_group() {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "sleep 60"]);
        command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        crate::capability_exec::process::configure_process_group(&mut command);
        let mut child = command.spawn().expect("spawn child process group");
        let (sender, receiver) = mpsc::channel();
        sender.send("stdout output limit".to_owned()).expect("send cancellation");
        let started = Instant::now();

        let error = wait_for_child_with_timeout_or_cancellation(
            &mut child,
            Duration::from_secs(60),
            "test child",
            Some(&receiver),
        )
        .expect_err("cancellation must terminate the child process group");

        assert!(started.elapsed() < Duration::from_secs(1));
        assert!(error.contains("stdout output limit"));
    }

    #[test]
    fn test_streaming_redaction_hides_credential_straddling_chunks() {
        let secret = "sk-straddles-a-pipe-boundary";
        let credentials = [("OPENAI_API_KEY", secret.to_owned())];
        let mut pending = Vec::new();
        let mut echoed = String::new();

        redact_and_emit_prefix(
            &mut pending,
            b"provider output: sk-straddles-",
            secret.len().saturating_add(64),
            &credentials,
            |text| echoed.push_str(text),
        );
        redact_and_emit_prefix(
            &mut pending,
            b"a-pipe-boundary\n",
            secret.len().saturating_add(64),
            &credentials,
            |text| echoed.push_str(text),
        );
        flush_redacted(&mut pending, &credentials, |text| echoed.push_str(text));

        assert!(!echoed.contains(secret));
        assert!(echoed.contains("[REDACTED:OPENAI_API_KEY]"));
    }

    #[test]
    fn test_streaming_redaction_hides_credential_crossing_emission_boundary() {
        let secret = "sk-crosses-emission-boundary";
        let credentials = [("OPENAI_API_KEY", secret.to_owned())];
        let mut pending = Vec::new();
        let mut echoed = String::new();

        redact_and_emit_prefix(
            &mut pending,
            b"prefix sk-crosses-",
            secret.len(),
            &credentials,
            |text| echoed.push_str(text),
        );
        redact_and_emit_prefix(
            &mut pending,
            b"emission-boundary suffix\n",
            secret.len(),
            &credentials,
            |text| echoed.push_str(text),
        );
        flush_redacted(&mut pending, &credentials, |text| echoed.push_str(text));

        assert!(!echoed.contains(secret));
        assert!(echoed.contains("[REDACTED:OPENAI_API_KEY]"));
    }

    #[test]
    fn test_streaming_redaction_withholds_placeholder_shaped_secret_prefix() {
        let secret = "[REDACTED:OPENAI_API_KEY]suffix";
        let credentials = [("OPENAI_API_KEY", secret.to_owned())];
        let mut pending = Vec::new();
        let mut echoed = String::new();

        redact_and_emit_prefix(
            &mut pending,
            b"provider: [REDACTED:OPENAI_API_KEY]",
            0,
            &credentials,
            |text| echoed.push_str(text),
        );
        redact_and_emit_prefix(&mut pending, b"suffix\n", 0, &credentials, |text| {
            echoed.push_str(text)
        });
        flush_redacted(&mut pending, &credentials, |text| echoed.push_str(text));

        assert!(!echoed.contains(secret), "no secret prefix may reach stderr");
        assert!(echoed.contains("[REDACTED:OPENAI_API_KEY]"));
    }

    #[cfg(unix)]
    #[test]
    fn test_version_probe_uses_safe_env_and_repository_root() {
        let directory = tempfile::tempdir().expect("temporary probe directory");
        let repository_root = directory.path().join("repository");
        std::fs::create_dir(&repository_root).expect("repository root");
        let probe = directory.path().join("version-probe.sh");
        std::fs::write(
            &probe,
            "#!/bin/sh\nprintf '%s|%s\\n' \"$PWD\" \"${HOME+x}\" > \"${0%/*}/probe-output.txt\"\nprintf 'codex 0.125.0\\n'\n",
        )
        .expect("version probe script");
        std::fs::set_permissions(&probe, std::fs::Permissions::from_mode(0o755))
            .expect("make probe executable");
        let context = TrustedLaunchContext { repository_root: repository_root.clone() };
        let safe_env = vec![(OsString::from("SAFE_TEST"), OsString::from("1"))];

        let (status, output) =
            context.run_version_probe(probe.as_os_str(), &safe_env).expect("version probe");

        assert!(status.success());
        assert_eq!(output, "codex 0.125.0\n");
        let captured = std::fs::read_to_string(directory.path().join("probe-output.txt"))
            .expect("probe output");
        assert_eq!(captured, format!("{}|\n", repository_root.display()));
    }

    #[cfg(unix)]
    #[test]
    fn test_version_probe_terminates_descendant_holding_output_pipes_open() {
        let directory = tempfile::tempdir().expect("temporary probe directory");
        let repository_root = directory.path().join("repository");
        std::fs::create_dir(&repository_root).expect("repository root");
        let probe = directory.path().join("orphaned-output-probe.sh");
        std::fs::write(&probe, "#!/bin/sh\nsleep 60 &\nexit 0\n").expect("version probe script");
        std::fs::set_permissions(&probe, std::fs::Permissions::from_mode(0o755))
            .expect("make probe executable");
        let context = TrustedLaunchContext { repository_root };
        let started = Instant::now();

        let (status, output) = context
            .run_version_probe(probe.as_os_str(), &[])
            .expect("descendant retaining a pipe must be terminated");

        assert!(started.elapsed() < Duration::from_secs(3));
        assert!(status.success());
        assert!(output.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn test_version_probe_terminates_descendant_after_clean_pipe_drain() {
        let directory = tempfile::tempdir().expect("temporary probe directory");
        let repository_root = directory.path().join("repository");
        std::fs::create_dir(&repository_root).expect("repository root");
        let probe = directory.path().join("detached-output-probe.sh");
        let descendant_pid = directory.path().join("descendant.pid");
        #[cfg(target_os = "linux")]
        let descendant_stat = directory.path().join("descendant.stat");
        #[cfg(target_os = "linux")]
        let probe_script = "#!/bin/sh\nsleep 60 </dev/null >/dev/null 2>&1 &\ndescendant_pid=$!\ndescendant_stat=\nIFS= read -r descendant_stat < \"/proc/$descendant_pid/stat\"\nprintf '%s' \"$descendant_pid\" > \"${0%/*}/descendant.pid\"\nprintf '%s' \"$descendant_stat\" > \"${0%/*}/descendant.stat\"\nprintf 'codex 0.125.0\\n'\nexit 0\n";
        #[cfg(not(target_os = "linux"))]
        let probe_script = "#!/bin/sh\nsleep 60 </dev/null >/dev/null 2>&1 &\nprintf '%s' \"$!\" > \"${0%/*}/descendant.pid\"\nprintf 'codex 0.125.0\\n'\nexit 0\n";
        std::fs::write(&probe, probe_script).expect("version probe script");
        std::fs::set_permissions(&probe, std::fs::Permissions::from_mode(0o755))
            .expect("make probe executable");
        let context = TrustedLaunchContext { repository_root };

        let (status, _) =
            context.run_version_probe(probe.as_os_str(), &[]).expect("version probe must succeed");
        let process_identity = std::fs::read_to_string(descendant_pid).expect("descendant pid");
        let process_id = process_identity.split_whitespace().next().expect("descendant pid");
        #[cfg(target_os = "linux")]
        fn parse_process_stat(process_stat: &str) -> Option<(String, String)> {
            let (_, fields) = process_stat.rsplit_once(") ")?;
            let mut fields = fields.split_whitespace();
            let process_state = fields.next()?.to_owned();
            let process_start_time = fields.nth(18)?.to_owned();
            Some((process_state, process_start_time))
        }

        #[cfg(target_os = "linux")]
        let expected_start_time = std::fs::read_to_string(descendant_stat)
            .ok()
            .and_then(|process_stat| parse_process_stat(&process_stat))
            .map(|(_, process_start_time)| process_start_time);
        const DESCENDANT_TERMINATION_TIMEOUT: Duration = Duration::from_secs(5);
        const DESCENDANT_REOBSERVATION_INTERVAL: Duration = Duration::from_millis(25);

        let observation_started = Instant::now();
        let mut last_observed_state = String::from("not observed");
        let descendant_terminated = loop {
            let kill_output =
                match Command::new("/bin/kill").args(["-0", process_id.trim()]).output() {
                    Ok(output) => output,
                    Err(error) => {
                        last_observed_state = format!("kill probe unavailable: {error}");
                        if observation_started.elapsed() >= DESCENDANT_TERMINATION_TIMEOUT {
                            break false;
                        }
                        std::thread::sleep(DESCENDANT_REOBSERVATION_INTERVAL);
                        continue;
                    }
                };

            if !kill_output.status.success() {
                break true;
            }

            #[cfg(target_os = "linux")]
            {
                let process_observation =
                    std::fs::read_to_string(format!("/proc/{process_id}/stat"))
                        .ok()
                        .and_then(|process_stat| parse_process_stat(&process_stat));

                if let Some((process_state, process_start_time)) = process_observation {
                    if let Some(expected_start_time) = expected_start_time.as_deref() {
                        if process_start_time != expected_start_time {
                            last_observed_state = format!(
                                "pid identity changed (start time {process_start_time} != expected {expected_start_time})"
                            );
                            break true;
                        }
                    }

                    last_observed_state = process_state.clone();
                    if process_state == "Z" {
                        break true;
                    }
                } else {
                    last_observed_state = String::from("identity unavailable");
                }
            }

            #[cfg(not(target_os = "linux"))]
            {
                last_observed_state = String::from("running");
            }

            if observation_started.elapsed() >= DESCENDANT_TERMINATION_TIMEOUT {
                break false;
            }
            std::thread::sleep(DESCENDANT_REOBSERVATION_INTERVAL);
        };

        assert!(status.success());
        assert!(
            descendant_terminated,
            "descendant termination exceeded DESCENDANT_TERMINATION_TIMEOUT ({DESCENDANT_TERMINATION_TIMEOUT:?}); last observed state: {last_observed_state}"
        );
    }
}
