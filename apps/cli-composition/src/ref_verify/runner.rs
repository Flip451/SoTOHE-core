use std::ffi::OsString;
use std::io::Read;
use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};
#[cfg(test)]
use std::time::{SystemTime, UNIX_EPOCH};

struct ProcessOutcome {
    timed_out: bool,
    exit_success: bool,
    stdout: String,
}

pub(super) struct PipeCollector {
    receiver: mpsc::Receiver<std::io::Result<String>>,
}

// The semantic verifier must judge only the prompt payload. `--tools ""` removes
// built-in tools from the Claude session, and `--allowedTools ""` keeps the
// permission whitelist empty even if host settings define broader defaults.
const CLAUDE_REF_VERIFIER_STATIC_ARGS: &[&str] = &[
    "-p",
    "--bare",
    "--permission-mode",
    "dontAsk",
    "--tools",
    "",
    "--allowedTools",
    "",
    "--disallowedTools",
];
const CLAUDE_REF_VERIFIER_DISALLOWED_TOOLS: &[&str] =
    &["Read", "Grep", "Glob", "Bash", "Edit", "Write"];

pub(super) fn run_ref_verifier_agent(
    project_root: &Path,
    resolved: infrastructure::agent_profiles::ResolvedExecution,
    prompt: String,
    timeout_secs: u64,
) -> Result<String, usecase::ref_verify::RefVerifyError> {
    if timeout_secs == 0 {
        return Err(ref_verify_runner_error("ref-verifier timeout_seconds must be nonzero"));
    }
    let timeout = Duration::from_secs(timeout_secs);
    match resolved.provider.as_str() {
        "claude" => {
            let model = require_ref_verifier_model("claude", resolved.model.as_deref())?;
            run_claude_ref_verifier(project_root, model, &prompt, timeout)
        }
        "codex" | "gemini" => Err(ref_verify_runner_error(format!(
            "ref-verifier provider '{}' cannot enforce the required no-tools boundary; configure provider 'claude'",
            resolved.provider
        ))),
        other => {
            Err(ref_verify_runner_error(format!("unsupported ref-verifier provider '{other}'")))
        }
    }
}

fn require_ref_verifier_model<'a>(
    provider: &str,
    model: Option<&'a str>,
) -> Result<&'a str, usecase::ref_verify::RefVerifyError> {
    model.ok_or_else(|| {
        ref_verify_runner_error(format!(
            "ref-verifier provider '{provider}' requires a configured model"
        ))
    })
}

fn run_claude_ref_verifier(
    project_root: &Path,
    model: &str,
    prompt: &str,
    timeout: Duration,
) -> Result<String, usecase::ref_verify::RefVerifyError> {
    let bin = provider_bin("CLAUDE_BIN", "claude");
    let args = build_claude_ref_verifier_args(model, prompt);
    let outcome =
        run_process_with_timeout(&bin, &args, project_root, timeout, "claude ref-verifier")?;
    process_success_or_error(outcome, "claude ref-verifier").and_then(|outcome| {
        extract_claude_ref_verifier_output(&outcome.stdout)
            .or_else(|| nonempty_trimmed(&outcome.stdout))
            .ok_or_else(|| ref_verify_runner_error("claude ref-verifier produced no output"))
    })
}

fn provider_bin(env_var: &str, default_bin: &str) -> OsString {
    std::env::var_os(env_var)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| OsString::from(default_bin))
}

pub(super) fn build_claude_ref_verifier_args(model: &str, prompt: &str) -> Vec<OsString> {
    let mut args = CLAUDE_REF_VERIFIER_STATIC_ARGS.iter().map(OsString::from).collect::<Vec<_>>();
    args.extend(CLAUDE_REF_VERIFIER_DISALLOWED_TOOLS.iter().map(OsString::from));
    args.push(OsString::from("--output-format"));
    args.push(OsString::from("json"));
    args.push(OsString::from("--model"));
    args.push(OsString::from(model));
    args.push(OsString::from(prompt));
    args
}

fn run_process_with_timeout(
    bin: &std::ffi::OsStr,
    args: &[OsString],
    current_dir: &Path,
    timeout: Duration,
    label: &str,
) -> Result<ProcessOutcome, usecase::ref_verify::RefVerifyError> {
    let deadline = Instant::now()
        .checked_add(timeout)
        .ok_or_else(|| ref_verify_runner_error(format!("{label} timeout is too large")))?;

    let mut command = Command::new(bin);
    command
        .args(args)
        .current_dir(current_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_verifier_process_group(&mut command);
    let mut child = command.spawn().map_err(|e| {
        ref_verify_runner_error(format!("failed to spawn {label} '{}': {e}", bin.to_string_lossy()))
    })?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| ref_verify_runner_error(format!("{label} stdout was not captured")))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| ref_verify_runner_error(format!("{label} stderr was not captured")))?;
    let stdout_handle = collect_pipe(stdout);
    let stderr_handle = collect_pipe(stderr);

    let (timed_out, exit_success) = wait_for_process(&mut child, deadline, label)?;
    if !timed_out {
        terminate_verifier_child(&mut child, label)?;
    }
    let stdout = if timed_out {
        String::new()
    } else {
        join_collector_until(stdout_handle, deadline, label, "stdout")?
    };
    if !timed_out {
        let _stderr = join_collector_until(stderr_handle, deadline, label, "stderr")?;
    }

    Ok(ProcessOutcome { timed_out, exit_success, stdout })
}

#[cfg(test)]
pub(super) fn run_test_ref_verifier_process(
    bin: &std::ffi::OsStr,
    current_dir: &Path,
    timeout: Duration,
) -> Result<String, usecase::ref_verify::RefVerifyError> {
    let outcome = run_process_with_timeout(bin, &[], current_dir, timeout, "test ref-verifier")?;
    process_success_or_error(outcome, "test ref-verifier").and_then(|outcome| {
        nonempty_trimmed(&outcome.stdout)
            .ok_or_else(|| ref_verify_runner_error("test ref-verifier produced no output"))
    })
}

fn wait_for_process(
    child: &mut Child,
    deadline: Instant,
    label: &str,
) -> Result<(bool, bool), usecase::ref_verify::RefVerifyError> {
    loop {
        match child
            .try_wait()
            .map_err(|e| ref_verify_runner_error(format!("failed to poll {label}: {e}")))?
        {
            Some(status) => return Ok((false, status.success())),
            None if Instant::now() >= deadline => {
                terminate_verifier_child(child, label)?;
                child.wait().map_err(|e| {
                    ref_verify_runner_error(format!("failed to reap timed-out {label}: {e}"))
                })?;
                return Ok((true, false));
            }
            None => thread::sleep(Duration::from_millis(50)),
        }
    }
}

#[cfg(unix)]
fn configure_verifier_process_group(command: &mut Command) {
    use std::os::unix::process::CommandExt as _;

    command.process_group(0);
}

#[cfg(not(unix))]
fn configure_verifier_process_group(_command: &mut Command) {}

#[cfg(unix)]
fn terminate_verifier_child(
    child: &mut Child,
    label: &str,
) -> Result<(), usecase::ref_verify::RefVerifyError> {
    let process_group_arg = format!("-{}", child.id());
    let status = Command::new("kill")
        .args(["-KILL", "--", process_group_arg.as_str()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| {
            ref_verify_runner_error(format!(
                "failed to spawn kill for {label} process group {}: {e}",
                child.id()
            ))
        })?;
    if status.success()
        || child
            .try_wait()
            .map_err(|e| ref_verify_runner_error(format!("failed to poll {label}: {e}")))?
            .is_some()
    {
        Ok(())
    } else {
        Err(ref_verify_runner_error(format!(
            "failed to terminate {label} process group {} with kill",
            child.id()
        )))
    }
}

#[cfg(windows)]
fn terminate_verifier_child(
    child: &mut Child,
    label: &str,
) -> Result<(), usecase::ref_verify::RefVerifyError> {
    if child
        .try_wait()
        .map_err(|e| ref_verify_runner_error(format!("failed to poll {label}: {e}")))?
        .is_some()
    {
        return Ok(());
    }

    let status = Command::new("taskkill")
        .args(["/PID", &child.id().to_string(), "/T", "/F"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| {
            ref_verify_runner_error(format!(
                "failed to spawn taskkill for {label} child {}: {e}",
                child.id()
            ))
        })?;
    if status.success()
        || child
            .try_wait()
            .map_err(|e| ref_verify_runner_error(format!("failed to poll {label}: {e}")))?
            .is_some()
    {
        Ok(())
    } else {
        Err(ref_verify_runner_error(format!(
            "failed to terminate {label} child tree {} with taskkill",
            child.id()
        )))
    }
}

#[cfg(not(any(unix, windows)))]
fn terminate_verifier_child(
    child: &mut Child,
    label: &str,
) -> Result<(), usecase::ref_verify::RefVerifyError> {
    child.kill().map_err(|e| ref_verify_runner_error(format!("failed to terminate {label}: {e}")))
}

pub(super) fn collect_pipe<R>(mut pipe: R) -> PipeCollector
where
    R: Read + Send + 'static,
{
    let (sender, receiver) = mpsc::sync_channel(1);
    thread::spawn(move || {
        let mut buf = String::new();
        let result = pipe.read_to_string(&mut buf).map(|_| buf);
        let _ = sender.send(result);
    });
    PipeCollector { receiver }
}

#[cfg(test)]
pub(super) fn join_collector(
    collector: PipeCollector,
    label: &str,
    stream: &str,
) -> Result<String, usecase::ref_verify::RefVerifyError> {
    collector
        .receiver
        .recv()
        .map_err(|_| ref_verify_runner_error(format!("{label} {stream} reader disconnected")))?
        .map_err(|e| ref_verify_runner_error(format!("failed to read {label} {stream}: {e}")))
}

fn join_collector_until(
    collector: PipeCollector,
    deadline: Instant,
    label: &str,
    stream: &str,
) -> Result<String, usecase::ref_verify::RefVerifyError> {
    let now = Instant::now();
    let remaining = deadline.saturating_duration_since(now);
    collector
        .receiver
        .recv_timeout(remaining)
        .map_err(|e| match e {
            mpsc::RecvTimeoutError::Timeout => {
                ref_verify_runner_error(format!("{label} {stream} reader timed out"))
            }
            mpsc::RecvTimeoutError::Disconnected => {
                ref_verify_runner_error(format!("{label} {stream} reader disconnected"))
            }
        })?
        .map_err(|e| ref_verify_runner_error(format!("failed to read {label} {stream}: {e}")))
}

fn process_success_or_error(
    outcome: ProcessOutcome,
    label: &str,
) -> Result<ProcessOutcome, usecase::ref_verify::RefVerifyError> {
    if outcome.timed_out {
        return Err(ref_verify_runner_error(format!("{label} timed out")));
    }
    if !outcome.exit_success {
        return Err(ref_verify_runner_error(format!("{label} failed")));
    }
    Ok(outcome)
}

fn extract_claude_ref_verifier_output(stdout: &str) -> Option<String> {
    for line in stdout.lines().map(str::trim).filter(|line| !line.is_empty()) {
        if let Some(output) = extract_claude_output_from_json(line) {
            return Some(output);
        }
    }
    extract_claude_output_from_json(stdout.trim())
}

fn extract_claude_output_from_json(text: &str) -> Option<String> {
    let value = serde_json::from_str::<serde_json::Value>(text).ok()?;
    if let Some(structured) = value.get("structured_output") {
        return serde_json::to_string(structured).ok();
    }
    value.get("result").and_then(|result| {
        result.as_str().and_then(nonempty_trimmed).or_else(|| serde_json::to_string(result).ok())
    })
}

fn nonempty_trimmed(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() { None } else { Some(trimmed.to_owned()) }
}

#[cfg(test)]
pub(super) fn ref_verify_runtime_path(
    project_root: &Path,
    prefix: &str,
    ext: &str,
) -> Result<PathBuf, usecase::ref_verify::RefVerifyError> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| ref_verify_runner_error(format!("failed to compute timestamp: {e}")))?
        .as_nanos();
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = project_root
        .join("tmp/reviewer-runtime")
        .join(format!("{prefix}-{}-{timestamp}-{seq}.{ext}", std::process::id()));
    let parent = path.parent().ok_or_else(|| {
        ref_verify_runner_error(format!("runtime path has no parent: '{}'", path.display()))
    })?;
    std::fs::create_dir_all(parent).map_err(|e| {
        ref_verify_runner_error(format!("failed to create '{}': {e}", parent.display()))
    })?;
    Ok(path)
}

fn ref_verify_runner_error(message: impl Into<String>) -> usecase::ref_verify::RefVerifyError {
    usecase::ref_verify::RefVerifyError::VerifierPort { message: message.into() }
}
