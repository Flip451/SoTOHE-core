//! Infrastructure adapter for the Codex-backed planner subprocess.
//!
//! `CodexPlannerAdapter` implements `usecase::planner::PlannerPort`.
//! All subprocess I/O (spawn, tee-stderr, timeout, kill) is handled here,
//! using the shared helpers in `crate::codex_common` where possible.
//!
//! The `runtime_dir` (session log directory) is supplied at construction time
//! by the composition root. It does not cross the usecase port boundary.
//!
//! Unlike the reviewer adapters, the planner uses `stdout = Stdio::inherit()`
//! (the user watches Codex output in real time) and has no output-schema /
//! output-last-message contract.

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use usecase::planner::{PlanRunOutput, PlannerPort, PlannerPortError};

use crate::codex_common::{POLL_INTERVAL, codex_bin, tee_stderr_to_file};

/// Infrastructure adapter that runs the Codex planner as a subprocess.
///
/// Implements `PlannerPort`. The CLI composition root constructs this adapter
/// with a `runtime_dir` and injects it into `PlanDriver`.
///
/// stdout is forwarded (inherited) so the user sees Codex output in real time.
/// stderr is teed to a timestamped session log file inside `runtime_dir`.
pub struct CodexPlannerAdapter {
    /// Directory used for session log files. Set at construction by the composition root.
    runtime_dir: PathBuf,
}

impl CodexPlannerAdapter {
    /// Create a new `CodexPlannerAdapter`.
    ///
    /// `runtime_dir` is the directory where session log files are written.
    /// The directory will be created if it does not exist when the first run is invoked.
    pub fn new(runtime_dir: PathBuf) -> Self {
        Self { runtime_dir }
    }
}

impl PlannerPort for CodexPlannerAdapter {
    fn run(
        &self,
        model: &str,
        prompt: &str,
        timeout_seconds: u64,
    ) -> Result<PlanRunOutput, PlannerPortError> {
        let args = build_planner_args(model, prompt);
        let timeout = Duration::from_secs(timeout_seconds);
        let session_log_path = prepare_session_log_path(&self.runtime_dir)?;
        let (child, tee_handle) = spawn_planner(codex_bin().as_ref(), &args, &session_log_path)?;
        run_planner_child(child, tee_handle, timeout)
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Build the Codex argument vector for the planner.
///
/// Always uses `--sandbox read-only`. Never uses `--full-auto` because Codex CLI
/// treats it as an alias for `--sandbox workspace-write`, which would override the
/// read-only constraint.
fn build_planner_args(model: &str, prompt: &str) -> Vec<OsString> {
    let mut args = vec![OsString::from("exec"), OsString::from("--model"), OsString::from(model)];
    args.extend([OsString::from("--sandbox"), OsString::from("read-only")]);
    args.push(OsString::from(prompt));
    args
}

/// Prepare a timestamped session log path inside `runtime_dir`.
fn prepare_session_log_path(runtime_dir: &std::path::Path) -> Result<PathBuf, PlannerPortError> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SESSION_LOG_COUNTER: AtomicU64 = AtomicU64::new(0);

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| PlannerPortError::PlannerUnavailable {
            reason: format!("failed to compute timestamp: {e}"),
        })?
        .as_nanos();
    let seq = SESSION_LOG_COUNTER.fetch_add(1, Ordering::Relaxed);

    let path =
        runtime_dir.join(format!("codex-session-{}-{timestamp}-{seq}.log", std::process::id()));
    let parent = path.parent().ok_or_else(|| PlannerPortError::PlannerUnavailable {
        reason: "session log path must have a parent directory".to_owned(),
    })?;
    std::fs::create_dir_all(parent).map_err(|e| PlannerPortError::PlannerUnavailable {
        reason: format!("failed to create session log directory: {e}"),
    })?;
    Ok(path)
}

/// Spawn the Codex planner subprocess, wiring stdout (inherited) and stderr (teed).
fn spawn_planner(
    bin: &std::ffi::OsStr,
    args: &[OsString],
    session_log_path: &std::path::Path,
) -> Result<(Child, Option<thread::JoinHandle<()>>), PlannerPortError> {
    let mut command = Command::new(bin);
    // stdout is inherited so the user sees Codex output in real time.
    command.args(args).stdin(Stdio::null()).stdout(Stdio::inherit());

    let log_file = std::fs::File::create(session_log_path).map_err(|e| {
        PlannerPortError::PlannerUnavailable {
            reason: format!("failed to create session log: {e}"),
        }
    })?;

    command.stderr(Stdio::piped());

    configure_child_process_group(&mut command);

    let mut child = command.spawn().map_err(|e| PlannerPortError::PlannerUnavailable {
        reason: format!("failed to start planner: {e}"),
    })?;

    let tee_handle = child.stderr.take().map(|pipe| {
        thread::spawn(move || {
            tee_stderr_to_file(pipe, log_file);
        })
    });

    Ok((child, tee_handle))
}

/// Poll the planner child until exit or timeout, then join the tee thread when safe.
fn run_planner_child(
    mut child: Child,
    tee_handle: Option<thread::JoinHandle<()>>,
    timeout: Duration,
) -> Result<PlanRunOutput, PlannerPortError> {
    let start = Instant::now();
    let mut timed_out = false;
    let mut raw_exit_code: u8 = 0;

    loop {
        match child.try_wait().map_err(|e| PlannerPortError::PlannerUnavailable {
            reason: format!("failed to poll planner: {e}"),
        })? {
            Some(status) => {
                raw_exit_code = u8::try_from(status.code().unwrap_or(1)).unwrap_or(1);
                break;
            }
            None => {
                if start.elapsed() >= timeout {
                    timed_out = true;
                    if let Err(kill_err) = terminate_planner_child(&mut child) {
                        match child.try_wait().map_err(|e| {
                            PlannerPortError::PlannerUnavailable {
                                reason: format!(
                                    "failed to poll planner after termination failure: {e}"
                                ),
                            }
                        })? {
                            Some(_) => {}
                            None => return Err(kill_err),
                        }
                    } else {
                        child.wait().map_err(|e| PlannerPortError::PlannerUnavailable {
                            reason: format!("failed to reap planner process: {e}"),
                        })?;
                    }
                    break;
                }
                thread::sleep(POLL_INTERVAL);
            }
        }
    }

    if !timed_out {
        // On timeout, descendant processes may still hold the stderr pipe open,
        // so joining the tee thread can block indefinitely. Dropping the handle
        // detaches it; it exits when the pipe is eventually closed.
        if let Some(handle) = tee_handle {
            let _ = handle.join();
        }
    }

    if timed_out {
        let elapsed_seconds = timeout.as_secs();
        eprintln!("[TIMEOUT] Local planner exceeded {elapsed_seconds}s");
        return Err(PlannerPortError::PlannerTimeout { elapsed_seconds });
    }

    Ok(PlanRunOutput { exit_code: raw_exit_code })
}

#[cfg(unix)]
fn configure_child_process_group(command: &mut Command) {
    // `std::os::unix::process::CommandExt::process_group` is a stable safe API
    // (Rust 1.64+) that wraps `setpgid(2)`. Setting pgroup=0 makes the child the
    // leader of a new process group, so killing `-child.id()` terminates the
    // child plus every descendant it spawns. Restores the process-tree
    // termination guarantee that the pre-refactor CLI helper provided.
    use std::os::unix::process::CommandExt;
    command.process_group(0);
}

#[cfg(not(unix))]
fn configure_child_process_group(_command: &mut Command) {
    // Windows uses `taskkill /T /F` for process-tree termination; no pre-spawn
    // configuration is required.
}

/// Terminates the planner child process and (on supported platforms) every
/// descendant it spawned.
///
/// On Unix, sends `SIGKILL` to the process group `-child.id()` by shelling out
/// to `/bin/kill`. This avoids the `unsafe` required by `libc::killpg(2)`
/// (`#[forbid(unsafe_code)]` is set on this crate) while still tearing down the
/// entire process tree the Codex planner may have spawned. On Windows, uses
/// `taskkill /T /F` for equivalent process-tree termination.
///
/// If the group / taskkill path fails for any reason, falls back to
/// `child.kill()` so the direct child is still terminated.
#[cfg(unix)]
fn terminate_planner_child(child: &mut Child) -> Result<(), PlannerPortError> {
    let pid = child.id();
    let group_target = format!("-{pid}");
    // `--` separates `-KILL` (signal spec) from the negative pid operand. Without
    // it, `kill` parses `-<pid>` as another signal flag and silently no-ops; the
    // existing ref-verify runner uses the same pattern.
    let status = Command::new("kill")
        .args(["-KILL", "--", &group_target])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    match status {
        Ok(s) if s.success() => Ok(()),
        _ => child.kill().map_err(|e| PlannerPortError::PlannerUnavailable {
            reason: format!("failed to terminate planner process: {e}"),
        }),
    }
}

#[cfg(not(unix))]
fn terminate_planner_child(child: &mut Child) -> Result<(), PlannerPortError> {
    if child
        .try_wait()
        .map_err(|e| PlannerPortError::PlannerUnavailable {
            reason: format!("failed to poll planner child: {e}"),
        })?
        .is_some()
    {
        return Ok(());
    }

    let status = Command::new("taskkill")
        .args(["/PID", &child.id().to_string(), "/T", "/F"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    match status {
        Ok(s) if s.success() => Ok(()),
        _ => child.kill().map_err(|e| PlannerPortError::PlannerUnavailable {
            reason: format!("failed to terminate planner process: {e}"),
        }),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_prepare_session_log_path_multiple_calls_returns_unique_paths() {
        let dir = tempfile::tempdir().unwrap();

        let first = prepare_session_log_path(dir.path()).unwrap();
        let second = prepare_session_log_path(dir.path()).unwrap();

        assert_ne!(first, second);
        assert_eq!(first.parent(), Some(dir.path()));
        assert_eq!(second.parent(), Some(dir.path()));
    }
}
