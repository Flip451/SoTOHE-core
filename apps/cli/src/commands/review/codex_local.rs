//! Subprocess management for the local Codex-backed reviewer.

use std::ffi::OsString;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitCode, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use std::os::unix::process::CommandExt;

use usecase::review_workflow::scope::{DiffScopeProvider, ScopeFilteredPayload};
use usecase::review_workflow::usecases::{RecordRoundProtocol, Verdict, record_round_typed};
use usecase::review_workflow::{
    ReviewFinalMessageState, ReviewPayloadVerdict, ReviewVerdict, classify_review_verdict,
    extract_verdict_from_content, findings_to_concerns, normalize_final_message,
    parse_review_final_message, render_review_payload,
};

use super::{
    AutoManagedArtifacts, CodexInvocation, CodexLocalArgs, OutputLastMessagePath, POLL_INTERVAL,
    REVIEW_RUNTIME_DIR, RenderedCommandResult, ReviewRunResult, ValidatedAutoRecordArgs,
    validate_auto_record_args,
};

#[cfg(test)]
use super::CODEX_BIN_ENV;

pub(super) fn execute_codex_local(args: &CodexLocalArgs) -> ExitCode {
    // Step 1: Validate auto-record args before spawning Codex (fail fast).
    let auto_record = match validate_auto_record_args(args) {
        Ok(ar) => ar,
        Err(err) => {
            eprintln!("[ERROR] {err}");
            return ExitCode::from(1);
        }
    };

    // Step 2: Run Codex (existing flow).
    let outcome = run_codex_local(args);

    // Step 3: If auto-record is enabled, apply scope filter and record.
    if let Some(validated) = auto_record {
        let scope_provider = infrastructure::review_adapters::GitDiffScopeProvider;
        let protocol = infrastructure::review_adapters::RecordRoundProtocolImpl {
            items_dir: validated.items_dir.clone(),
            group_display: validated.group_name.as_ref().to_owned(),
            base_ref: validated.diff_base.clone(),
        };
        return execute_with_auto_record(
            outcome,
            validated,
            args.timeout_seconds,
            &scope_provider,
            &protocol,
        );
    }

    // Step 4: Original flow (no auto-record).
    let rendered = render_codex_local_result(args, outcome);
    for line in rendered.stdout_lines {
        println!("{line}");
    }
    for line in rendered.stderr_lines {
        eprintln!("{line}");
    }
    ExitCode::from(rendered.exit_code)
}

/// Applies scope filtering, calls `record_round_typed`, then emits the filtered verdict JSON.
///
/// `timeout_seconds` is forwarded to the non-actionable verdict rendering path
/// (Timeout / ProcessFailed / LastMessageMissing) where it appears in the error message.
///
/// Generic over `DiffScopeProvider` and `RecordRoundProtocol` so tests can inject stubs.
pub(super) fn execute_with_auto_record(
    outcome: Result<ReviewRunResult, String>,
    validated: ValidatedAutoRecordArgs,
    timeout_seconds: u64,
    scope_provider: &impl DiffScopeProvider,
    protocol: &impl RecordRoundProtocol,
) -> ExitCode {
    // Handle non-success outcomes (spawn failure etc.) — skip auto-record.
    let result = match outcome {
        Ok(r) => r,
        Err(err) => {
            eprintln!("[ERROR] {err}");
            return ExitCode::from(1);
        }
    };

    match result.verdict {
        ReviewVerdict::ZeroFindings | ReviewVerdict::FindingsRemain => {
            record_auto_round(result, validated, scope_provider, protocol)
        }
        // Non-actionable verdicts (Timeout, ProcessFailed, LastMessageMissing):
        // skip auto-record, use inline rendering.
        ReviewVerdict::Timeout => render_non_actionable_timeout(&result, timeout_seconds),
        ReviewVerdict::ProcessFailed => render_non_actionable_process_failed(result),
        ReviewVerdict::LastMessageMissing => render_non_actionable_missing_message(&result),
    }
}

/// Performs the scope-filter → record → emit flow for actionable verdicts.
#[allow(clippy::too_many_lines)]
fn record_auto_round(
    result: ReviewRunResult,
    validated: ValidatedAutoRecordArgs,
    scope_provider: &impl DiffScopeProvider,
    protocol: &impl RecordRoundProtocol,
) -> ExitCode {
    // Parse the payload for scope filtering.
    let payload = match result.final_message.as_deref() {
        Some(json) => match parse_review_final_message(Some(json)) {
            ReviewFinalMessageState::Parsed(p) => p,
            _ => {
                eprintln!("[ERROR] auto-record: could not parse verdict payload");
                if let Some(msg) = result.final_message {
                    println!("{msg}");
                }
                return ExitCode::from(1);
            }
        },
        None => {
            eprintln!("[ERROR] auto-record: no verdict payload to record");
            return ExitCode::from(1);
        }
    };

    // Compute diff scope. On failure, skip filtering but still record the round.
    let filtered = match scope_provider.changed_files(&validated.diff_base) {
        Ok(scope) => {
            let f = usecase::review_workflow::apply_scope_filter(payload, &scope);
            let total = f.adjusted_payload.findings.len() + f.out_of_scope.len();
            if !f.out_of_scope.is_empty() || f.unknown_path_count > 0 {
                eprintln!(
                    "[scope-filter] {} of {} findings out of scope, {} unknown paths",
                    f.out_of_scope.len(),
                    total,
                    f.unknown_path_count
                );
            }
            f
        }
        Err(err) => {
            eprintln!("[WARNING] Failed to compute diff scope (recording unfiltered): {err}");
            ScopeFilteredPayload {
                adjusted_payload: payload,
                out_of_scope: Vec::new(),
                unknown_path_count: 0,
            }
        }
    };

    // Map ReviewPayloadVerdict → domain::Verdict.
    let domain_verdict = match filtered.adjusted_payload.verdict {
        ReviewPayloadVerdict::ZeroFindings => Verdict::ZeroFindings,
        ReviewPayloadVerdict::FindingsRemain => Verdict::FindingsRemain,
    };

    // Extract concerns from in-scope findings.
    let concerns = match findings_to_concerns(&filtered.adjusted_payload.findings) {
        Ok(c) => c,
        Err(err) => {
            eprintln!("[ERROR] Failed to extract concerns: {err}");
            Vec::new()
        }
    };

    // Build timestamp.
    let timestamp = match super::make_timestamp() {
        Ok(ts) => ts,
        Err(err) => {
            eprintln!("[ERROR] {err}");
            return ExitCode::from(1);
        }
    };

    // Capture display strings before moving validated fields.
    let group_display = validated.group_name.as_ref().to_owned();
    let round_type_display = validated.round_type;

    // Call record_round_typed.
    use usecase::review_workflow::usecases::RecordRoundError;
    match record_round_typed(
        validated.track_id,
        validated.round_type,
        validated.group_name,
        domain_verdict,
        concerns,
        validated.expected_groups,
        timestamp,
        protocol,
    ) {
        Ok(()) => {
            eprintln!(
                "[auto-record] Recorded {round_type_display} round for group \
                 '{group_display}' (verdict: {domain_verdict})"
            );
        }
        Err(RecordRoundError::EscalationBlocked(blocked)) => {
            eprintln!("[auto-record] Escalation blocked: {}", blocked.join(", "));
            // Do NOT print verdict JSON — recording failed, so the verdict is untrusted.
            return ExitCode::from(3);
        }
        Err(RecordRoundError::Other(msg)) => {
            // All recording failures are fatal — the core guarantee of auto-record
            // is that verdicts are always persisted internally.
            // Do NOT print verdict JSON — unrecorded verdicts must not be trusted.
            eprintln!("[auto-record] Record failed: {msg}");
            return ExitCode::from(1);
        }
    }

    // Output the filtered verdict JSON to stdout.
    if let Ok(json) = render_review_payload(&filtered.adjusted_payload) {
        println!("{json}");
    }

    match filtered.adjusted_payload.verdict {
        ReviewPayloadVerdict::ZeroFindings => ExitCode::from(0),
        ReviewPayloadVerdict::FindingsRemain => ExitCode::from(2),
    }
}

fn render_non_actionable_timeout(result: &ReviewRunResult, timeout_seconds: u64) -> ExitCode {
    let msg = if result.output_last_message_auto_managed {
        format!("[TIMEOUT] Local reviewer exceeded {timeout_seconds}s")
    } else {
        format!(
            "[TIMEOUT] Local reviewer exceeded {timeout_seconds}s: {}",
            result.output_last_message.display()
        )
    };
    eprintln!("{msg}");
    ExitCode::from(1)
}

fn render_non_actionable_process_failed(result: ReviewRunResult) -> ExitCode {
    eprintln!("[ERROR] Local reviewer process failed");
    if let Some(detail) = result.verdict_detail {
        eprintln!("{detail}");
    }
    if let Some(message) = result.final_message {
        eprintln!("{message}");
    }
    ExitCode::from(1)
}

fn render_non_actionable_missing_message(result: &ReviewRunResult) -> ExitCode {
    let msg = if result.output_last_message_auto_managed {
        "[ERROR] Local reviewer finished without a final message".to_owned()
    } else {
        format!(
            "[ERROR] Local reviewer finished without a final message: {}",
            result.output_last_message.display()
        )
    };
    eprintln!("{msg}");
    ExitCode::from(1)
}

pub(super) fn render_codex_local_result(
    args: &CodexLocalArgs,
    outcome: Result<ReviewRunResult, String>,
) -> RenderedCommandResult {
    match outcome {
        Ok(result) => match result.verdict {
            ReviewVerdict::ZeroFindings => render_final_json_or_failure(
                result,
                0,
                "[ERROR] Local reviewer reported zero findings without a final JSON payload",
            ),
            ReviewVerdict::FindingsRemain => render_final_json_or_failure(
                result,
                2,
                "[ERROR] Local reviewer reported findings without a final JSON payload",
            ),
            ReviewVerdict::Timeout => RenderedCommandResult {
                exit_code: 1,
                stdout_lines: Vec::new(),
                stderr_lines: vec![render_missing_message_failure(
                    &format!("[TIMEOUT] Local reviewer exceeded {}s", args.timeout_seconds),
                    &result,
                )],
            },
            ReviewVerdict::ProcessFailed => {
                let mut stderr_lines = vec!["[ERROR] Local reviewer process failed".to_owned()];
                if let Some(detail) = result.verdict_detail {
                    stderr_lines.push(detail);
                }
                if let Some(message) = result.final_message {
                    stderr_lines.push(message);
                }
                RenderedCommandResult { exit_code: 1, stdout_lines: Vec::new(), stderr_lines }
            }
            ReviewVerdict::LastMessageMissing => RenderedCommandResult {
                exit_code: 1,
                stdout_lines: Vec::new(),
                stderr_lines: vec![render_missing_message_failure(
                    "[ERROR] Local reviewer finished without a final message",
                    &result,
                )],
            },
        },
        Err(err) => RenderedCommandResult {
            exit_code: 1,
            stdout_lines: Vec::new(),
            stderr_lines: vec![format!("local reviewer failed: {err}")],
        },
    }
}

fn render_final_json_or_failure(
    result: ReviewRunResult,
    success_exit_code: u8,
    missing_payload_message: &str,
) -> RenderedCommandResult {
    match result.final_message {
        Some(message) => RenderedCommandResult {
            exit_code: success_exit_code,
            stdout_lines: vec![message],
            stderr_lines: Vec::new(),
        },
        None => RenderedCommandResult {
            exit_code: 1,
            stdout_lines: Vec::new(),
            stderr_lines: vec![render_missing_message_failure(missing_payload_message, &result)],
        },
    }
}

fn render_missing_message_failure(prefix: &str, result: &ReviewRunResult) -> String {
    if result.output_last_message_auto_managed {
        prefix.to_owned()
    } else {
        format!("{prefix}: {}", result.output_last_message.display())
    }
}

pub(super) fn run_codex_local(args: &CodexLocalArgs) -> Result<ReviewRunResult, String> {
    let prompt = build_prompt(args)?;
    #[cfg(test)]
    let explicit_output_last_message = args.output_last_message.as_deref();
    #[cfg(not(test))]
    let explicit_output_last_message: Option<&Path> = None;

    let output_last_message = prepare_output_last_message_path(explicit_output_last_message)?;
    let output_schema = prepare_output_schema_path()?;
    let session_log = prepare_session_log_path()?;
    // Session log is NOT auto-managed — it persists for post-run traceability/debugging.
    let _cleanup = AutoManagedArtifacts::new([&output_last_message, &output_schema]);
    reset_output_last_message(&output_last_message.path)?;
    write_output_schema(&output_schema.path)?;
    let invocation = build_codex_invocation(
        &args.model,
        &prompt,
        &output_last_message.path,
        &output_schema.path,
    );
    run_codex_invocation(
        &invocation,
        Duration::from_secs(args.timeout_seconds),
        output_last_message,
        &session_log.path,
    )
}

pub(super) fn build_prompt(args: &CodexLocalArgs) -> Result<String, String> {
    let prompt = if let Some(path) = &args.briefing_file {
        if !path.is_file() {
            return Err(format!("briefing file not found: {}", path.display()));
        }
        format!("Read {} and perform the task described there.", path.display())
    } else {
        args.prompt
            .clone()
            .ok_or_else(|| "either --briefing-file or --prompt is required".to_owned())?
    };

    Ok(prompt)
}

fn prepare_output_last_message_path(path: Option<&Path>) -> Result<OutputLastMessagePath, String> {
    let (path, auto_managed) = match path {
        Some(path) => (path.to_path_buf(), false),
        None => {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|err| format!("failed to compute timestamp: {err}"))?
                .as_millis();
            (
                PathBuf::from(REVIEW_RUNTIME_DIR)
                    .join(format!("codex-last-message-{}-{timestamp}.txt", std::process::id())),
                true,
            )
        }
    };

    let parent = path.parent().ok_or_else(|| {
        format!("output-last-message path must have a parent directory: {}", path.display())
    })?;
    std::fs::create_dir_all(parent)
        .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;

    Ok(OutputLastMessagePath { path, auto_managed })
}

fn reset_output_last_message(path: &Path) -> Result<(), String> {
    std::fs::write(path, "").map_err(|err| {
        format!("failed to initialize reviewer final message {}: {err}", path.display())
    })
}

fn prepare_output_schema_path() -> Result<OutputLastMessagePath, String> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| format!("failed to compute timestamp: {err}"))?
        .as_millis();
    let path = PathBuf::from(REVIEW_RUNTIME_DIR)
        .join(format!("codex-output-schema-{}-{timestamp}.json", std::process::id()));
    let parent = path.parent().ok_or_else(|| {
        format!("output-schema path must have a parent directory: {}", path.display())
    })?;
    std::fs::create_dir_all(parent)
        .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;

    Ok(OutputLastMessagePath { path, auto_managed: true })
}

fn write_output_schema(path: &Path) -> Result<(), String> {
    use usecase::review_workflow::REVIEW_OUTPUT_SCHEMA_JSON;
    std::fs::write(path, REVIEW_OUTPUT_SCHEMA_JSON)
        .map_err(|err| format!("failed to write reviewer output schema {}: {err}", path.display()))
}

pub(super) fn build_codex_invocation(
    model: &str,
    prompt: &str,
    output_last_message: &Path,
    output_schema: &Path,
) -> CodexInvocation {
    let mut args = vec![OsString::from("exec"), OsString::from("--model"), OsString::from(model)];
    // Reviewers MUST use read-only sandbox. Do NOT use --full-auto here because it
    // implies --sandbox workspace-write and Codex CLI applies it after our explicit
    // --sandbox read-only, overriding the safety constraint.
    args.extend([OsString::from("--sandbox"), OsString::from("read-only")]);
    args.extend([OsString::from("--config"), OsString::from("model_reasoning_effort=\"high\"")]);
    args.extend([
        OsString::from("--output-schema"),
        output_schema.as_os_str().to_os_string(),
        OsString::from("--output-last-message"),
        output_last_message.as_os_str().to_os_string(),
        OsString::from(prompt),
    ]);

    CodexInvocation { bin: codex_bin(), args }
}

fn codex_bin() -> OsString {
    #[cfg(test)]
    if let Some(value) = std::env::var_os(CODEX_BIN_ENV).filter(|value| !value.is_empty()) {
        return value;
    }

    OsString::from("codex")
}

fn spawn_codex(
    invocation: &CodexInvocation,
    session_log_path: &Path,
) -> Result<(Child, Option<thread::JoinHandle<()>>), String> {
    let mut command = Command::new(&invocation.bin);
    command.args(&invocation.args).stdin(Stdio::null()).stdout(Stdio::inherit());

    // Capture stderr to a session log file while also forwarding to inherited stderr.
    // Open the log file before spawning so we fail early on I/O errors.
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

fn run_codex_invocation(
    invocation: &CodexInvocation,
    timeout: Duration,
    output_last_message: OutputLastMessagePath,
    session_log_path: &Path,
) -> Result<ReviewRunResult, String> {
    let (child, tee_handle) = spawn_codex(invocation, session_log_path)?;
    run_codex_child(child, tee_handle, timeout, output_last_message, session_log_path)
}

fn run_codex_child(
    mut child: Child,
    tee_handle: Option<thread::JoinHandle<()>>,
    timeout: Duration,
    output_last_message: OutputLastMessagePath,
    session_log_path: &Path,
) -> Result<ReviewRunResult, String> {
    let start = Instant::now();
    let mut timed_out = false;
    let mut exit_success = false;

    loop {
        match child.try_wait().map_err(|err| format!("failed to poll reviewer child: {err}"))? {
            Some(status) => {
                exit_success = status.success();
                break;
            }
            None => {
                if start.elapsed() >= timeout {
                    timed_out = true;
                    terminate_reviewer_child(&mut child)?;
                    child.wait().map_err(|err| format!("failed to reap reviewer child: {err}"))?;
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

    let raw_final_message = read_final_message(&output_last_message.path)?;
    let final_message_state = parse_review_final_message(raw_final_message.as_deref());

    // Fallback: if codex-last-message is empty, try extracting verdict from session log.
    let final_message_state = if matches!(final_message_state, ReviewFinalMessageState::Missing) {
        let fallback = std::fs::read_to_string(session_log_path)
            .ok()
            .and_then(|content| extract_verdict_from_content(&content));
        match fallback {
            Some(fallback_state) => {
                eprintln!(
                    "[INFO] Verdict extracted from session log fallback: {}",
                    session_log_path.display()
                );
                fallback_state
            }
            None => final_message_state,
        }
    } else {
        final_message_state
    };

    let final_message = match &final_message_state {
        ReviewFinalMessageState::Parsed(payload) => {
            Some(render_review_payload(payload).map_err(|e| e.to_string())?)
        }
        _ => raw_final_message,
    };
    let verdict = classify_review_verdict(timed_out, exit_success, &final_message_state);
    let verdict_detail = match &final_message_state {
        ReviewFinalMessageState::Invalid { reason } => {
            Some(format!("invalid reviewer final payload: {reason}"))
        }
        _ => None,
    };

    Ok(ReviewRunResult {
        verdict,
        final_message,
        output_last_message: output_last_message.path,
        output_last_message_auto_managed: output_last_message.auto_managed,
        verdict_detail,
    })
}

pub(super) fn prepare_session_log_path() -> Result<OutputLastMessagePath, String> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| format!("failed to compute timestamp: {err}"))?
        .as_millis();
    let path = PathBuf::from(REVIEW_RUNTIME_DIR)
        .join(format!("codex-session-{}-{timestamp}.log", std::process::id()));
    let parent = path.parent().ok_or_else(|| {
        format!("session log path must have a parent directory: {}", path.display())
    })?;
    std::fs::create_dir_all(parent)
        .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;

    Ok(OutputLastMessagePath { path, auto_managed: true })
}

fn read_final_message(path: &Path) -> Result<Option<String>, String> {
    match std::fs::read_to_string(path) {
        Ok(content) => Ok(normalize_final_message(&content)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(format!("failed to read reviewer final message {}: {err}", path.display())),
    }
}

#[cfg(unix)]
fn configure_child_process_group(command: &mut Command) {
    command.process_group(0);
}

#[cfg(not(unix))]
fn configure_child_process_group(_command: &mut Command) {}

#[cfg(unix)]
fn terminate_reviewer_child(child: &mut Child) -> Result<(), String> {
    let process_group = i32::try_from(child.id())
        .map_err(|_| format!("reviewer child pid does not fit into i32: {}", child.id()))?;
    // Safety: `killpg` is called with the child process group id created by `process_group(0)`.
    let result = unsafe { libc::killpg(process_group, libc::SIGKILL) };
    if result == 0 {
        Ok(())
    } else {
        let err = std::io::Error::last_os_error();
        if err.raw_os_error() == Some(libc::ESRCH) {
            Ok(())
        } else {
            Err(format!("failed to terminate reviewer child process group {process_group}: {err}"))
        }
    }
}

#[cfg(windows)]
fn terminate_reviewer_child(child: &mut Child) -> Result<(), String> {
    if child.try_wait().map_err(|err| format!("failed to poll reviewer child: {err}"))?.is_some() {
        return Ok(());
    }

    let status = Command::new("taskkill")
        .args(["/PID", &child.id().to_string(), "/T", "/F"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|err| {
            format!("failed to spawn taskkill for reviewer child {}: {err}", child.id())
        })?;
    if status.success() {
        Ok(())
    } else if child
        .try_wait()
        .map_err(|err| format!("failed to poll reviewer child after taskkill: {err}"))?
        .is_some()
    {
        Ok(())
    } else {
        Err(format!("failed to terminate reviewer child process tree {} via taskkill", child.id()))
    }
}

#[cfg(all(not(unix), not(windows)))]
fn terminate_reviewer_child(child: &mut Child) -> Result<(), String> {
    child.kill().map_err(|err| format!("failed to terminate reviewer child: {err}"))
}
