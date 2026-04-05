//! Subprocess management for the local Codex-backed reviewer.

use std::io::{self, Write};
use std::process::ExitCode;
use std::time::Duration;

use domain::review_v2::{ReviewOutcome, ReviewWriter};
use infrastructure::review_v2::CodexReviewer;
use usecase::review_workflow::{ReviewFinalPayload, ReviewPayloadVerdict, render_review_payload};

use super::{CodexLocalArgs, validate_auto_record_args};

pub(super) fn execute_codex_local(args: &CodexLocalArgs) -> ExitCode {
    match run_execute_codex_local(args) {
        Ok(code) => ExitCode::from(code),
        Err(msg) => {
            eprintln!("{msg}");
            ExitCode::from(1)
        }
    }
}

fn run_execute_codex_local(args: &CodexLocalArgs) -> Result<u8, String> {
    // Step 1: Validate record args before building composition (fail fast).
    let validated = validate_auto_record_args(args)?;

    // Step 2: Build base prompt. The scope file list is appended by
    // CodexReviewer::build_full_prompt when it receives the ReviewTarget.
    let base_prompt = build_base_prompt(args)?;

    // Step 3: Build v2 composition with real CodexReviewer.
    let track_id = domain::TrackId::try_new(validated.track_id.as_ref())
        .map_err(|e| format!("[ERROR] invalid track id: {e}"))?;
    let timeout = Duration::from_secs(args.timeout_seconds);
    let reviewer = CodexReviewer::new(&args.model, timeout, base_prompt)
        .with_scope_label(validated.group_name.as_ref());
    let comp =
        super::compose_v2::build_review_v2_with_reviewer(&track_id, &validated.items_dir, reviewer)
            .map_err(|e| format!("[ERROR] v2 composition failed: {e}"))?;

    // Step 4: Map --group to ScopeName.
    let scope = map_group_to_scope(validated.group_name.as_ref())?;

    // Step 5: Run review via ReviewCycle (hash_before → Codex → hash_after).
    match validated.round_type {
        domain::RoundType::Final => match comp.cycle.review(&scope) {
            Ok(ReviewOutcome::Skipped) => {
                emit_skip_output(validated.group_name.as_ref())?;
                Ok(0)
            }
            Ok(ReviewOutcome::Reviewed { verdict, hash, .. }) => {
                comp.review_store
                    .write_verdict(&scope, &verdict, &hash)
                    .map_err(|e| format!("[ERROR] record failed: {e}"))?;
                emit_verdict_output_final(&verdict)
            }
            Err(e) => Err(format!("[ERROR] {e}")),
        },
        domain::RoundType::Fast => match comp.cycle.fast_review(&scope) {
            Ok(ReviewOutcome::Skipped) => {
                emit_skip_output(validated.group_name.as_ref())?;
                Ok(0)
            }
            Ok(ReviewOutcome::Reviewed { verdict, hash, .. }) => {
                comp.review_store
                    .write_fast_verdict(&scope, &verdict, &hash)
                    .map_err(|e| format!("[ERROR] record failed: {e}"))?;
                emit_verdict_output_fast(&verdict)
            }
            Err(e) => Err(format!("[ERROR] {e}")),
        },
    }
}

/// Builds the base prompt from CLI args (briefing file or inline prompt).
///
/// The scope file list is NOT appended here — `CodexReviewer::build_full_prompt`
/// appends it when it receives the `ReviewTarget` from `ReviewCycle`.
///
/// # Errors
/// Returns an error if the briefing file does not exist or neither arg is provided.
pub(super) fn build_base_prompt(args: &CodexLocalArgs) -> Result<String, String> {
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

/// Alias for test compatibility — tests.rs imports `build_prompt` by this name.
#[cfg(test)]
pub(super) use build_base_prompt as build_prompt;

/// Maps a group name string to a `ScopeName`.
fn map_group_to_scope(group: &str) -> Result<domain::review_v2::ScopeName, String> {
    if group == "other" {
        Ok(domain::review_v2::ScopeName::Other)
    } else {
        domain::review_v2::MainScopeName::new(group)
            .map(domain::review_v2::ScopeName::Main)
            .map_err(|e| format!("[ERROR] invalid scope name: {e}"))
    }
}

/// Prints the skip message and zero_findings JSON for an empty scope.
fn emit_skip_output(scope: &str) -> Result<(), String> {
    eprintln!("[auto-record] Scope '{scope}' is empty, skipping");
    emit_stdout_line(r#"{"verdict":"zero_findings","findings":[]}"#)
}

/// Emits the final verdict JSON to stdout and returns the appropriate exit code.
fn emit_verdict_output_final(verdict: &domain::review_v2::Verdict) -> Result<u8, String> {
    let (payload, exit_code) = match verdict {
        domain::review_v2::Verdict::ZeroFindings => (
            ReviewFinalPayload { verdict: ReviewPayloadVerdict::ZeroFindings, findings: vec![] },
            0u8,
        ),
        domain::review_v2::Verdict::FindingsRemain(nef) => {
            let findings = nef.as_slice().iter().map(finding_to_review_finding).collect();
            (ReviewFinalPayload { verdict: ReviewPayloadVerdict::FindingsRemain, findings }, 2u8)
        }
    };
    let json = render_review_payload(&payload).map_err(|e| format!("[ERROR] {e}"))?;
    emit_stdout_line(&json)?;
    Ok(exit_code)
}

/// Emits the fast verdict JSON to stdout and returns the appropriate exit code.
fn emit_verdict_output_fast(verdict: &domain::review_v2::FastVerdict) -> Result<u8, String> {
    let (payload, exit_code) = match verdict {
        domain::review_v2::FastVerdict::ZeroFindings => (
            ReviewFinalPayload { verdict: ReviewPayloadVerdict::ZeroFindings, findings: vec![] },
            0u8,
        ),
        domain::review_v2::FastVerdict::FindingsRemain(nef) => {
            let findings = nef.as_slice().iter().map(finding_to_review_finding).collect();
            (ReviewFinalPayload { verdict: ReviewPayloadVerdict::FindingsRemain, findings }, 2u8)
        }
    };
    let json = render_review_payload(&payload).map_err(|e| format!("[ERROR] {e}"))?;
    emit_stdout_line(&json)?;
    Ok(exit_code)
}

/// Converts a domain `Finding` to a `ReviewFinding` for JSON serialization.
fn finding_to_review_finding(
    f: &domain::review_v2::Finding,
) -> usecase::review_workflow::ReviewFinding {
    usecase::review_workflow::ReviewFinding {
        message: f.message().to_owned(),
        severity: f.severity().map(str::to_owned),
        file: f.file().map(str::to_owned),
        line: f.line(),
        category: f.category().map(str::to_owned),
    }
}

fn emit_stdout_line(line: &str) -> Result<(), String> {
    writeln!(io::stdout(), "{line}").map_err(|e| format!("failed to write stdout: {e}"))
}

// ---------------------------------------------------------------------------
// Test shim — provides stable test surface for tests.rs
// ---------------------------------------------------------------------------
//
// Tests that call `run_codex_local` or `build_codex_invocation` directly
// continue to work. This shim reimplements the subprocess pipeline using the
// same types (`CodexInvocation`, `ReviewRunResult`, `OutputLastMessagePath`)
// that live in mod.rs under `#[cfg(test)]`.

#[cfg(test)]
pub(super) fn build_codex_invocation(
    model: &str,
    prompt: &str,
    output_last_message: &std::path::Path,
    output_schema: &std::path::Path,
) -> super::CodexInvocation {
    use std::ffi::OsString;
    let mut args = vec![OsString::from("exec"), OsString::from("--model"), OsString::from(model)];
    args.extend([OsString::from("--sandbox"), OsString::from("read-only")]);
    args.extend([OsString::from("--config"), OsString::from("model_reasoning_effort=\"high\"")]);
    args.extend([
        OsString::from("--output-schema"),
        output_schema.as_os_str().to_os_string(),
        OsString::from("--output-last-message"),
        output_last_message.as_os_str().to_os_string(),
        OsString::from(prompt),
    ]);
    super::CodexInvocation { bin: codex_bin(), args }
}

#[cfg(test)]
fn codex_bin() -> std::ffi::OsString {
    if let Some(v) = std::env::var_os(super::CODEX_BIN_ENV).filter(|v| !v.is_empty()) {
        return v;
    }
    std::ffi::OsString::from("codex")
}

#[cfg(test)]
pub(super) fn run_codex_local(args: &CodexLocalArgs) -> Result<super::ReviewRunResult, String> {
    let prompt = build_base_prompt(args)?;

    let output_last_message_path = prepare_output_last_message(args)?;
    let output_schema_path = prepare_timestamped_path("codex-output-schema", "json")?;
    let session_log_path = prepare_timestamped_path("codex-session", "log")?;

    // Only auto-managed paths are cleaned up on drop; explicit (test-provided) paths persist.
    let mut cleanup_paths = vec![output_schema_path.clone()];
    if output_last_message_path.auto_managed {
        cleanup_paths.push(output_last_message_path.path.clone());
    }
    let _cleanup = TestArtifactCleanup(cleanup_paths);

    std::fs::write(&output_last_message_path.path, "")
        .map_err(|e| format!("failed to init output-last-message: {e}"))?;
    std::fs::write(&output_schema_path, usecase::review_workflow::REVIEW_OUTPUT_SCHEMA_JSON)
        .map_err(|e| format!("failed to write output-schema: {e}"))?;

    let invocation = build_codex_invocation(
        &args.model,
        &prompt,
        &output_last_message_path.path,
        &output_schema_path,
    );
    run_codex_invocation(
        &invocation,
        Duration::from_secs(args.timeout_seconds),
        output_last_message_path,
        &session_log_path,
    )
}

#[cfg(test)]
fn prepare_output_last_message(
    args: &CodexLocalArgs,
) -> Result<super::OutputLastMessagePath, String> {
    if let Some(path) = &args.output_last_message {
        let parent =
            path.parent().ok_or_else(|| format!("path has no parent: {}", path.display()))?;
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create {}: {e}", parent.display()))?;
        return Ok(super::OutputLastMessagePath { path: path.clone(), auto_managed: false });
    }
    let path = prepare_timestamped_path("codex-last-message", "txt")?;
    Ok(super::OutputLastMessagePath { path, auto_managed: true })
}

#[cfg(test)]
fn prepare_timestamped_path(prefix: &str, ext: &str) -> Result<std::path::PathBuf, String> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| format!("timestamp error: {e}"))?
        .as_millis();
    let path = std::path::PathBuf::from(super::REVIEW_RUNTIME_DIR)
        .join(format!("{prefix}-{}-{ts}.{ext}", std::process::id()));
    let parent = path.parent().ok_or_else(|| format!("path has no parent: {}", path.display()))?;
    std::fs::create_dir_all(parent)
        .map_err(|e| format!("failed to create {}: {e}", parent.display()))?;
    Ok(path)
}

#[cfg(test)]
struct TestArtifactCleanup(Vec<std::path::PathBuf>);

#[cfg(test)]
impl Drop for TestArtifactCleanup {
    fn drop(&mut self) {
        for p in &self.0 {
            let _ = std::fs::remove_file(p);
        }
    }
}

#[cfg(test)]
fn run_codex_invocation(
    invocation: &super::CodexInvocation,
    timeout: Duration,
    output_last_message: super::OutputLastMessagePath,
    session_log_path: &std::path::Path,
) -> Result<super::ReviewRunResult, String> {
    use std::io::BufRead;
    use std::process::{Command, Stdio};
    use std::thread;

    let log_file = std::fs::File::create(session_log_path)
        .map_err(|e| format!("failed to create session log: {e}"))?;

    let mut command = Command::new(&invocation.bin);
    command
        .args(&invocation.args)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::piped());

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }

    let mut child = command
        .spawn()
        .map_err(|e| format!("failed to spawn {}: {e}", invocation.bin.to_string_lossy()))?;

    let stderr_pipe = child.stderr.take();
    let tee_handle = stderr_pipe.map(|pipe| {
        thread::spawn(move || {
            let reader = std::io::BufReader::new(pipe);
            let mut lf = log_file;
            for line in reader.lines() {
                match line {
                    Ok(l) => {
                        let _ = writeln!(lf, "{l}");
                        eprintln!("{l}");
                    }
                    Err(_) => break,
                }
            }
            let _ = lf.flush();
        })
    });

    let (timed_out, exit_success) =
        poll_child_with_timeout(&mut child, tee_handle, timeout, session_log_path)?;

    parse_codex_output(output_last_message, session_log_path, timed_out, exit_success)
}

/// Polls a child process until it exits or the timeout elapses, then joins the tee thread.
#[cfg(test)]
fn poll_child_with_timeout(
    child: &mut std::process::Child,
    tee_handle: Option<std::thread::JoinHandle<()>>,
    timeout: Duration,
    _session_log_path: &std::path::Path,
) -> Result<(bool, bool), String> {
    use std::thread;
    let start = std::time::Instant::now();
    let mut timed_out = false;
    let mut exit_success = false;

    loop {
        match child.try_wait().map_err(|e| format!("failed to poll reviewer child: {e}"))? {
            Some(status) => {
                exit_success = status.success();
                break;
            }
            None => {
                if start.elapsed() >= timeout {
                    timed_out = true;
                    terminate_child(child)?;
                    child.wait().map_err(|e| format!("failed to reap child: {e}"))?;
                    break;
                }
                thread::sleep(super::POLL_INTERVAL);
            }
        }
    }

    if let Some(h) = tee_handle {
        let _ = h.join();
    }
    Ok((timed_out, exit_success))
}

/// Reads and parses the Codex output files into a `ReviewRunResult`.
#[cfg(test)]
fn parse_codex_output(
    output_last_message: super::OutputLastMessagePath,
    session_log_path: &std::path::Path,
    timed_out: bool,
    exit_success: bool,
) -> Result<super::ReviewRunResult, String> {
    let raw_content = match std::fs::read_to_string(&output_last_message.path) {
        Ok(c) => usecase::review_workflow::normalize_final_message(&c),
        Err(e) if e.kind() == io::ErrorKind::NotFound => None,
        Err(e) => return Err(format!("failed to read output-last-message: {e}")),
    };

    let final_message_state =
        usecase::review_workflow::parse_review_final_message(raw_content.as_deref());

    let final_message_state = if matches!(
        final_message_state,
        usecase::review_workflow::ReviewFinalMessageState::Missing
    ) {
        let fallback = std::fs::read_to_string(session_log_path)
            .ok()
            .and_then(|c| usecase::review_workflow::extract_verdict_from_content(&c));
        match fallback {
            Some(s) => {
                eprintln!(
                    "[INFO] Verdict extracted from session log fallback: {}",
                    session_log_path.display()
                );
                s
            }
            None => final_message_state,
        }
    } else {
        final_message_state
    };

    let final_message = match &final_message_state {
        usecase::review_workflow::ReviewFinalMessageState::Parsed(p) => {
            Some(usecase::review_workflow::render_review_payload(p).map_err(|e| e.to_string())?)
        }
        _ => raw_content,
    };
    let verdict = usecase::review_workflow::classify_review_verdict(
        timed_out,
        exit_success,
        &final_message_state,
    );
    let verdict_detail = match &final_message_state {
        usecase::review_workflow::ReviewFinalMessageState::Invalid { reason } => {
            Some(format!("invalid reviewer final payload: {reason}"))
        }
        _ => None,
    };

    Ok(super::ReviewRunResult {
        verdict,
        final_message,
        output_last_message: output_last_message.path,
        output_last_message_auto_managed: output_last_message.auto_managed,
        verdict_detail,
    })
}

#[cfg(unix)]
#[cfg(test)]
fn terminate_child(child: &mut std::process::Child) -> Result<(), String> {
    let pid = i32::try_from(child.id())
        .map_err(|_| format!("child pid does not fit i32: {}", child.id()))?;
    // Safety: killpg sends SIGKILL to the process group created by process_group(0) above.
    let result = unsafe { libc::killpg(pid, libc::SIGKILL) };
    if result == 0 {
        Ok(())
    } else {
        let err = io::Error::last_os_error();
        if err.raw_os_error() == Some(libc::ESRCH) {
            Ok(())
        } else {
            Err(format!("failed to kill reviewer process group {pid}: {err}"))
        }
    }
}

#[cfg(windows)]
#[cfg(test)]
fn terminate_child(child: &mut std::process::Child) -> Result<(), String> {
    use std::process::{Command, Stdio};
    if child.try_wait().map_err(|e| format!("failed to poll child: {e}"))?.is_some() {
        return Ok(());
    }
    let status = Command::new("taskkill")
        .args(["/PID", &child.id().to_string(), "/T", "/F"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| format!("taskkill failed: {e}"))?;
    if status.success() {
        Ok(())
    } else if child.try_wait().map_err(|e| format!("poll after taskkill: {e}"))?.is_some() {
        Ok(())
    } else {
        Err(format!("failed to terminate child {} via taskkill", child.id()))
    }
}

#[cfg(all(not(unix), not(windows)))]
#[cfg(test)]
fn terminate_child(child: &mut std::process::Child) -> Result<(), String> {
    child.kill().map_err(|e| format!("failed to kill child: {e}"))
}
