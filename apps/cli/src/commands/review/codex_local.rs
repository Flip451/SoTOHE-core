//! Subprocess management for the local Codex-backed reviewer.
//!
//! Production code never imports `domain::` types directly (CN-01 / AC-03).
//! All domain conversions happen inside `infrastructure::review_v2`.

use std::io::{self, Write};
use std::process::ExitCode;
#[cfg(test)]
use std::time::Duration;

use cli_composition::ReviewRunCodexInput;

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
    // Step 1: Validate record args before delegating to CliApp (fail fast).
    let validated = validate_auto_record_args(args)?;

    // Step 2: Build DTO and delegate to CliApp.review_run_codex.
    let input = ReviewRunCodexInput {
        model: args.model.clone(),
        timeout_seconds: args.timeout_seconds,
        briefing_file: args.briefing_file.clone(),
        prompt: args.prompt.clone(),
        track_id: Some(validated.track_id),
        round_type: validated.round_type_str,
        group: validated.group_name,
        items_dir: validated.items_dir,
    };

    let outcome = cli_composition::ReviewCompositionRoot::new()
        .review_run_codex(input)
        .map_err(|e| e.to_string())?;

    emit_outcome_output(outcome.stdout.as_deref(), outcome.exit_code)
}

/// Builds the base prompt from CLI args (briefing file or inline prompt).
///
/// The scope file list is NOT appended here — `CodexReviewer::build_full_prompt`
/// appends it when it receives the `ReviewTarget` from `ReviewCycle`.
///
/// # Errors
/// Returns an error if the briefing file does not exist or neither arg is provided.
#[cfg(test)]
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

/// Appends a scope-specific severity policy reference section to `prompt`
/// when the given scope has a `briefing_file` configured and the path is safe
/// to inject.
///
/// No domain types involved — uses string-based lookup via infrastructure.
/// Becomes a no-op when the scope has no briefing configured, the scope is
/// "other", or the configured path fails `is_safe_briefing_path`.
///
/// # Errors
/// Returns an error if scope config cannot be loaded.
#[cfg(test)]
pub(super) fn append_scope_briefing_reference(
    prompt: &mut String,
    scope_name: &str,
    track_id: &str,
    items_dir: &std::path::Path,
) -> Result<(), String> {
    cli_composition::review_v2::append_scope_briefing_reference_str(
        prompt,
        scope_name,
        track_id,
        items_dir,
        is_safe_briefing_path,
    )
}

/// Returns `true` if `path` is safe to reference as a repo-relative briefing
/// file and to inject into the markdown prompt as a backtick-quoted path bullet.
///
/// Rejects strings that contain **any of the following**:
///
/// Prompt-injection class (would break out of the `` `path` `` markdown context
/// or smuggle additional prompt lines):
/// - Any Unicode control character (`char::is_control`, Unicode category Cc) —
///   covers ASCII C0 0x00–0x1F (including `\n`, `\r`, `\t`), DEL 0x7F,
///   and C1 controls 0x80–0x9F (including NEL U+0085)
/// - Line / paragraph separators U+2028 (Zl) and U+2029 (Zp) — not in category
///   Cc and therefore not caught by `is_control`, but both act as line breaks
/// - Backtick (`` ` ``)
///
/// Path-traversal class:
/// - Absolute paths starting with `/` or `\`
/// - Windows UNC and drive-letter prefixes (e.g. `\\server\share`, `C:\...`)
/// - Any `..` component (e.g. `track/../../etc/passwd`), split on either
///   `/` or `\`
///
/// Empty paths are also rejected.
#[cfg(test)]
pub(super) fn is_safe_briefing_path(path: &str) -> bool {
    if path.is_empty() {
        return false;
    }
    // Prompt-injection guard
    if path.chars().any(|c| c == '`' || c.is_control() || matches!(c, '\u{2028}' | '\u{2029}')) {
        return false;
    }
    // Absolute path (Unix root or Windows root / UNC)
    if path.starts_with('/') || path.starts_with('\\') {
        return false;
    }
    // Windows drive-letter prefix: `C:` / `c:` etc.
    if let (Some(first), Some(second)) = (path.as_bytes().first(), path.as_bytes().get(1)) {
        if *second == b':' && first.is_ascii_alphabetic() {
            return false;
        }
    }
    // Path-traversal: reject any `..` component (check both separators).
    if path.split(['/', '\\']).any(|component| component == "..") {
        return false;
    }
    true
}

/// Emits the review outcome to stdout and returns the exit code.
fn emit_outcome_output(stdout: Option<&str>, exit_code: u8) -> Result<u8, String> {
    if let Some(line) = stdout {
        writeln!(io::stdout(), "{line}").map_err(|e| format!("failed to write stdout: {e}"))?;
    }
    Ok(exit_code)
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
    let args = cli_composition::build_codex_read_only_invocation(
        model,
        "high",
        prompt,
        output_last_message,
        output_schema,
    );
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
                thread::sleep(crate::commands::POLL_INTERVAL);
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
