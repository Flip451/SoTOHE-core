use std::ffi::OsString;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};
use usecase::review_v2::run_review_fix::ReviewFixRunnerError;

use crate::codex_common::REVIEW_RUNTIME_DIR;

use super::session_log::{redact_credentials, write_session_log};

pub(super) fn fixer_runtime_path(prefix: &str, ext: &str) -> Result<PathBuf, ReviewFixRunnerError> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| ReviewFixRunnerError::Unexpected(format!("failed to compute timestamp: {e}")))?
        .as_nanos();
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = PathBuf::from(REVIEW_RUNTIME_DIR)
        .join(format!("{prefix}-{}-{timestamp}-{seq}.{ext}", std::process::id()));
    let parent = path.parent().ok_or_else(|| {
        ReviewFixRunnerError::Unexpected(format!(
            "runtime path must have a parent directory: {}",
            path.display()
        ))
    })?;
    std::fs::create_dir_all(parent).map_err(|e| {
        ReviewFixRunnerError::Unexpected(format!("failed to create {}: {e}", parent.display()))
    })?;
    Ok(path)
}

pub(super) fn spawn_and_collect_codex(
    bin: &std::ffi::OsStr,
    args: &[OsString],
    safe_env: &[(OsString, OsString)],
    prompt: &str,
) -> Result<(String, PathBuf), ReviewFixRunnerError> {
    let log_path = fixer_runtime_path("review-fix-codex-session", "log")?;
    let mut command = Command::new(bin);
    command.args(args);
    command.env_clear();
    for (k, v) in safe_env {
        command.env(k, v);
    }
    command.stdin(Stdio::piped());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|e| {
        ReviewFixRunnerError::SpawnFailed(format!("failed to spawn codex fixer: {e}"))
    })?;
    let stdout_pipe = child.stdout.take();
    let stdout_handle = thread::spawn(move || collect_output_pipe(stdout_pipe, false, "stdout"));
    let stderr_pipe = child.stderr.take();
    let stderr_handle = thread::spawn(move || collect_output_pipe(stderr_pipe, true, "stderr"));
    let prompt_write_result = match child.stdin.take() {
        Some(mut stdin) => stdin
            .write_all(prompt.as_bytes())
            .map_err(|e| format!("failed to write prompt to codex fixer stdin: {e}")),
        None => Err("failed to open codex fixer stdin pipe".to_owned()),
    };
    if let Err(message) = prompt_write_result {
        let _ = child.kill();
        let exit_status = child.wait().map_or_else(
            |e| format!("failed to wait after prompt write error: {e}"),
            |s| s.to_string(),
        );
        let (stdout, _) =
            collector_result_for_log(join_output_collector(stdout_handle, "stdout"), "stdout");
        let (stderr, _) =
            collector_result_for_log(join_output_collector(stderr_handle, "stderr"), "stderr");
        write_session_log(&log_path, bin, &exit_status, &stdout, &stderr);
        return Err(ReviewFixRunnerError::SpawnFailed(format!(
            "{message}; session log: {}",
            log_path.display()
        )));
    }
    let exit_status = child.wait().map_err(|e| {
        ReviewFixRunnerError::SpawnFailed(format!("failed to wait for codex fixer: {e}"))
    })?;
    let exit_status = exit_status.to_string();
    let (stdout, stdout_error) =
        collector_result_for_log(join_output_collector(stdout_handle, "stdout"), "stdout");
    let (stderr, stderr_error) =
        collector_result_for_log(join_output_collector(stderr_handle, "stderr"), "stderr");
    write_session_log(&log_path, bin, &exit_status, &stdout, &stderr);
    if let Some(error) = stdout_error.or(stderr_error) {
        return Err(ReviewFixRunnerError::Unexpected(format!(
            "{error}; session log: {}",
            log_path.display()
        )));
    }
    Ok((stdout, log_path))
}

pub(super) fn collector_result_for_log(
    result: Result<String, ReviewFixRunnerError>,
    stream_name: &str,
) -> (String, Option<ReviewFixRunnerError>) {
    match result {
        Ok(output) => (output, None),
        Err(error) => (format!("[failed to collect {stream_name}: {error}]\n"), Some(error)),
    }
}

pub(super) fn collect_output_pipe<R: std::io::Read>(
    pipe: Option<R>,
    echo_to_stderr: bool,
    stream_name: &str,
) -> Result<String, String> {
    let mut collected = String::new();
    if let Some(pipe) = pipe {
        let reader = BufReader::new(pipe);
        for line in reader.lines() {
            let line =
                line.map_err(|e| format!("failed to read codex fixer {stream_name}: {e}"))?;
            if echo_to_stderr {
                eprintln!("{}", redact_credentials(&line));
            }
            collected.push_str(&line);
            collected.push('\n');
        }
    }
    Ok(collected)
}

pub(super) fn join_output_collector(
    handle: thread::JoinHandle<Result<String, String>>,
    stream_name: &str,
) -> Result<String, ReviewFixRunnerError> {
    handle
        .join()
        .map_err(|_| {
            ReviewFixRunnerError::Unexpected(format!(
                "codex fixer {stream_name} collector thread panicked"
            ))
        })?
        .map_err(ReviewFixRunnerError::Unexpected)
}
