//! Subprocess management for the local Codex-backed planner.
//!
//! Simpler than the reviewer counterpart: no output-schema, no verdict parsing,
//! no auto-record. Just build the prompt, spawn Codex, forward output, and
//! return the exit code.

use std::ffi::OsString;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, Command, ExitCode, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use std::os::unix::process::CommandExt;

use super::{CodexInvocation, PLAN_RUNTIME_DIR, POLL_INTERVAL, PlanCodexLocalArgs, PlanRunResult};

#[cfg(test)]
use super::CODEX_BIN_ENV;

/// Execute the local Codex-backed planner.
///
/// Builds the prompt, spawns Codex, forwards stdout/stderr, waits with timeout,
/// and returns the Codex process exit code. No verdict parsing is performed.
pub(super) fn execute_codex_local(args: &PlanCodexLocalArgs) -> ExitCode {
    let prompt = match build_prompt(args) {
        Ok(p) => p,
        Err(err) => {
            eprintln!("[ERROR] {err}");
            return ExitCode::from(1);
        }
    };

    let invocation = build_codex_invocation(&args.model, &prompt);
    let timeout = Duration::from_secs(args.timeout_seconds);

    match run_codex_local_invocation(&invocation, timeout) {
        Ok(result) => ExitCode::from(result.exit_code),
        Err(err) => {
            eprintln!("[ERROR] {err}");
            ExitCode::from(1)
        }
    }
}

/// Build the prompt string from either a briefing file or an inline prompt.
///
/// # Errors
///
/// Returns an error string if the briefing file is not found or neither
/// `--briefing-file` nor `--prompt` was provided.
pub(super) fn build_prompt(args: &PlanCodexLocalArgs) -> Result<String, String> {
    if let Some(path) = &args.briefing_file {
        if !path.is_file() {
            return Err(format!("briefing file not found: {}", path.display()));
        }
        Ok(format!("Read {} and perform the task described there.", path.display()))
    } else {
        args.prompt
            .clone()
            .ok_or_else(|| "either --briefing-file or --prompt is required".to_owned())
    }
}

/// Build the Codex invocation for the planner.
///
/// Always uses `--sandbox read-only`. Never uses `--full-auto` because Codex CLI
/// treats it as an alias for `--sandbox workspace-write`, which would override
/// our read-only constraint.
pub(super) fn build_codex_invocation(model: &str, prompt: &str) -> CodexInvocation {
    let mut args = vec![OsString::from("exec"), OsString::from("--model"), OsString::from(model)];
    args.extend([OsString::from("--sandbox"), OsString::from("read-only")]);
    args.push(OsString::from(prompt));

    CodexInvocation { bin: codex_bin(), args }
}

fn codex_bin() -> OsString {
    #[cfg(test)]
    if let Some(value) = std::env::var_os(CODEX_BIN_ENV).filter(|value| !value.is_empty()) {
        return value;
    }

    OsString::from("codex")
}

fn prepare_session_log_path() -> Result<PathBuf, String> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| format!("failed to compute timestamp: {err}"))?
        .as_millis();
    let path = PathBuf::from(PLAN_RUNTIME_DIR)
        .join(format!("codex-session-{}-{timestamp}.log", std::process::id()));
    let parent = path.parent().ok_or_else(|| {
        format!("session log path must have a parent directory: {}", path.display())
    })?;
    std::fs::create_dir_all(parent)
        .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
    Ok(path)
}

fn spawn_codex(
    invocation: &CodexInvocation,
    session_log_path: &std::path::Path,
) -> Result<(Child, Option<thread::JoinHandle<()>>), String> {
    let mut command = Command::new(&invocation.bin);
    command.args(&invocation.args).stdin(Stdio::null()).stdout(Stdio::inherit());

    // Capture stderr to a session log file while also forwarding to inherited stderr.
    let log_file = std::fs::File::create(session_log_path).map_err(|err| {
        format!("failed to create session log {}: {err}", session_log_path.display())
    })?;

    command.stderr(Stdio::piped());
    configure_child_process_group(&mut command);

    let mut child = command
        .spawn()
        .map_err(|err| format!("failed to spawn {}: {err}", invocation.bin.to_string_lossy()))?;

    // Spawn a tee thread that copies stderr to both the log file and the real stderr.
    let stderr_pipe = child.stderr.take();
    let tee_handle = stderr_pipe.map(|pipe| {
        thread::spawn(move || {
            tee_stderr_to_file(pipe, log_file);
        })
    });

    Ok((child, tee_handle))
}

/// Copies lines from a pipe to both a log file and inherited stderr.
fn tee_stderr_to_file(pipe: std::process::ChildStderr, mut log_file: std::fs::File) {
    let reader = BufReader::new(pipe);
    for line in reader.lines() {
        match line {
            Ok(line) => {
                let _ = writeln!(log_file, "{line}");
                eprintln!("{line}");
            }
            Err(_) => break,
        }
    }
    let _ = log_file.flush();
}

pub(super) fn run_codex_local_invocation(
    invocation: &CodexInvocation,
    timeout: Duration,
) -> Result<PlanRunResult, String> {
    let session_log_path = prepare_session_log_path()?;
    let (child, tee_handle) = spawn_codex(invocation, &session_log_path)?;
    run_codex_child(child, tee_handle, timeout)
}

fn run_codex_child(
    mut child: Child,
    tee_handle: Option<thread::JoinHandle<()>>,
    timeout: Duration,
) -> Result<PlanRunResult, String> {
    let start = Instant::now();
    let mut timed_out = false;
    let mut raw_exit_code: u8 = 0;

    loop {
        match child.try_wait().map_err(|err| format!("failed to poll planner child: {err}"))? {
            Some(status) => {
                raw_exit_code = u8::try_from(status.code().unwrap_or(1)).unwrap_or(1);
                break;
            }
            None => {
                if start.elapsed() >= timeout {
                    timed_out = true;
                    terminate_planner_child(&mut child)?;
                    child.wait().map_err(|err| format!("failed to reap planner child: {err}"))?;
                    break;
                }
                thread::sleep(POLL_INTERVAL);
            }
        }
    }

    // Wait for the tee thread to finish flushing the log file.
    if let Some(handle) = tee_handle {
        let _ = handle.join();
    }

    if timed_out {
        eprintln!("[TIMEOUT] Local planner exceeded {}s", timeout.as_secs());
        return Ok(PlanRunResult { exit_code: 1 });
    }

    Ok(PlanRunResult { exit_code: raw_exit_code })
}

#[cfg(unix)]
fn configure_child_process_group(command: &mut Command) {
    command.process_group(0);
}

#[cfg(not(unix))]
fn configure_child_process_group(_command: &mut Command) {}

#[cfg(unix)]
fn terminate_planner_child(child: &mut Child) -> Result<(), String> {
    let process_group = i32::try_from(child.id())
        .map_err(|_| format!("planner child pid does not fit into i32: {}", child.id()))?;
    // Safety: `killpg` is called with the child process group id created by `process_group(0)`.
    let result = unsafe { libc::killpg(process_group, libc::SIGKILL) };
    if result == 0 {
        Ok(())
    } else {
        let err = std::io::Error::last_os_error();
        if err.raw_os_error() == Some(libc::ESRCH) {
            Ok(())
        } else {
            Err(format!("failed to terminate planner child process group {process_group}: {err}"))
        }
    }
}

#[cfg(windows)]
fn terminate_planner_child(child: &mut Child) -> Result<(), String> {
    if child.try_wait().map_err(|err| format!("failed to poll planner child: {err}"))?.is_some() {
        return Ok(());
    }

    let status = Command::new("taskkill")
        .args(["/PID", &child.id().to_string(), "/T", "/F"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|err| {
            format!("failed to spawn taskkill for planner child {}: {err}", child.id())
        })?;
    if status.success() {
        Ok(())
    } else if child
        .try_wait()
        .map_err(|err| format!("failed to poll planner child after taskkill: {err}"))?
        .is_some()
    {
        Ok(())
    } else {
        Err(format!("failed to terminate planner child process tree {} via taskkill", child.id()))
    }
}

#[cfg(all(not(unix), not(windows)))]
fn terminate_planner_child(child: &mut Child) -> Result<(), String> {
    child.kill().map_err(|err| format!("failed to terminate planner child: {err}"))
}
