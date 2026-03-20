//! CLI subcommands for local reviewer workflow wrappers.

use std::ffi::OsString;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitCode, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use clap::{ArgGroup, Args, Subcommand};
use usecase::review_workflow::{
    ModelProfile, REVIEW_OUTPUT_SCHEMA_JSON, ReviewFinalMessageState, ReviewVerdict,
    classify_review_verdict, normalize_final_message, parse_review_final_message,
    render_review_payload, resolve_full_auto,
};

#[cfg(unix)]
use std::os::unix::process::CommandExt;

const DEFAULT_TIMEOUT_SECONDS: u64 = 600;
const REVIEW_RUNTIME_DIR: &str = "tmp/reviewer-runtime";
const POLL_INTERVAL: Duration = Duration::from_millis(50);
#[cfg(test)]
const CODEX_BIN_ENV: &str = "SOTP_CODEX_BIN";

#[derive(Debug, Subcommand)]
pub enum ReviewCommand {
    /// Run the local Codex-backed reviewer through a repo-owned wrapper.
    CodexLocal(CodexLocalArgs),
    /// Record a review round result into metadata.json.
    RecordRound(RecordRoundArgs),
    /// Check if review is approved for commit.
    CheckApproved(CheckApprovedArgs),
    /// Resolve an active review escalation block.
    ResolveEscalation(ResolveEscalationArgs),
}

#[derive(Debug, Args)]
#[command(group(
    ArgGroup::new("review_input")
        .required(true)
        .args(["briefing_file", "prompt"])
))]
pub struct CodexLocalArgs {
    /// Model name resolved from `.claude/agent-profiles.json`.
    #[arg(long)]
    model: String,

    /// Timeout for the reviewer subprocess in seconds.
    #[arg(long, default_value_t = DEFAULT_TIMEOUT_SECONDS)]
    timeout_seconds: u64,

    /// Path to a briefing file that the reviewer should read.
    #[arg(long)]
    briefing_file: Option<PathBuf>,

    /// Inline prompt for the reviewer.
    #[arg(long)]
    prompt: Option<String>,

    /// Test-only explicit path where the wrapper should ask Codex to write the final message.
    #[cfg(test)]
    #[arg(long, hide = true)]
    output_last_message: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct RecordRoundArgs {
    /// Round type: fast or final.
    #[arg(long)]
    round_type: String,

    /// Review group name (e.g., "infra-domain").
    #[arg(long)]
    group: String,

    /// Verdict JSON string (e.g., '{"verdict":"zero_findings","findings":[]}').
    #[arg(long)]
    verdict: String,

    /// Comma-separated list of expected group names.
    #[arg(long)]
    expected_groups: String,

    /// Comma-separated list of concern slugs for escalation tracking.
    /// Extracted from reviewer findings. Empty for zero_findings rounds.
    #[arg(long, default_value = "")]
    concerns: String,

    /// Path to the track items directory.
    #[arg(long, default_value = "track/items")]
    items_dir: PathBuf,

    /// Track ID.
    #[arg(long)]
    track_id: String,
}

#[derive(Debug, Args)]
pub struct ResolveEscalationArgs {
    /// Track ID.
    #[arg(long)]
    track_id: String,

    /// Comma-separated list of blocked concerns to resolve.
    /// Must match the concerns currently blocking escalation.
    #[arg(long)]
    blocked_concerns: String,

    /// Path to workspace search artifact.
    #[arg(long)]
    workspace_search_ref: String,

    /// Path to reinvention check artifact.
    #[arg(long)]
    reinvention_check_ref: String,

    /// Decision: adopt_workspace, adopt_crate, or continue_self.
    #[arg(long)]
    decision: String,

    /// Summary of the decision rationale.
    #[arg(long)]
    summary: String,

    /// Path to the track items directory.
    #[arg(long, default_value = "track/items")]
    items_dir: PathBuf,
}

#[derive(Debug, Args)]
pub struct CheckApprovedArgs {
    /// Path to the track items directory.
    #[arg(long, default_value = "track/items")]
    items_dir: PathBuf,

    /// Track ID.
    #[arg(long)]
    track_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReviewRunResult {
    verdict: ReviewVerdict,
    final_message: Option<String>,
    output_last_message: PathBuf,
    output_last_message_auto_managed: bool,
    verdict_detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CodexInvocation {
    bin: OsString,
    args: Vec<OsString>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RenderedCommandResult {
    exit_code: u8,
    stdout_lines: Vec<String>,
    stderr_lines: Vec<String>,
}

pub fn execute(cmd: ReviewCommand) -> ExitCode {
    match cmd {
        ReviewCommand::CodexLocal(args) => execute_codex_local(&args),
        ReviewCommand::RecordRound(args) => execute_record_round(&args),
        ReviewCommand::CheckApproved(args) => execute_check_approved(&args),
        ReviewCommand::ResolveEscalation(args) => execute_resolve_escalation(&args),
    }
}

fn execute_codex_local(args: &CodexLocalArgs) -> ExitCode {
    let rendered = render_codex_local_result(args, run_codex_local(args));
    for line in rendered.stdout_lines {
        println!("{line}");
    }
    for line in rendered.stderr_lines {
        eprintln!("{line}");
    }
    ExitCode::from(rendered.exit_code)
}

fn render_codex_local_result(
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

fn run_codex_local(args: &CodexLocalArgs) -> Result<ReviewRunResult, String> {
    let prompt = build_prompt(args)?;
    let full_auto = resolve_full_auto_from_profiles(&args.model);
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
        full_auto,
    );
    run_codex_invocation(
        &invocation,
        Duration::from_secs(args.timeout_seconds),
        output_last_message,
        &session_log.path,
    )
}

fn build_prompt(args: &CodexLocalArgs) -> Result<String, String> {
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct OutputLastMessagePath {
    path: PathBuf,
    auto_managed: bool,
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
    std::fs::write(path, REVIEW_OUTPUT_SCHEMA_JSON)
        .map_err(|err| format!("failed to write reviewer output schema {}: {err}", path.display()))
}

#[derive(Debug)]
struct AutoManagedArtifacts {
    paths: Vec<PathBuf>,
}

impl AutoManagedArtifacts {
    fn new<'a>(artifacts: impl IntoIterator<Item = &'a OutputLastMessagePath>) -> Self {
        Self {
            paths: artifacts
                .into_iter()
                .filter(|artifact| artifact.auto_managed)
                .map(|artifact| artifact.path.clone())
                .collect(),
        }
    }
}

impl Drop for AutoManagedArtifacts {
    fn drop(&mut self) {
        for path in &self.paths {
            let _ = std::fs::remove_file(path);
        }
    }
}

fn build_codex_invocation(
    model: &str,
    prompt: &str,
    output_last_message: &Path,
    output_schema: &Path,
    full_auto: bool,
) -> CodexInvocation {
    let mut args = vec![OsString::from("exec"), OsString::from("--model"), OsString::from(model)];
    if full_auto {
        // --full-auto is required for full models (gpt-5.4 etc.) to produce
        // JSON verdicts reliably (see GitHub Issue #4181).
        // However, --full-auto implicitly sets --sandbox workspace-write.
        // We re-apply --sandbox read-only AFTER --full-auto so the last-wins
        // CLI semantics enforce read-only sandbox for reviewers.
        args.push(OsString::from("--full-auto"));
    }
    args.extend([OsString::from("--sandbox"), OsString::from("read-only")]);
    args.extend([
        OsString::from("--output-schema"),
        output_schema.as_os_str().to_os_string(),
        OsString::from("--output-last-message"),
        output_last_message.as_os_str().to_os_string(),
        OsString::from(prompt),
    ]);

    CodexInvocation { bin: codex_bin(), args }
}

const AGENT_PROFILES_PATH: &str = ".claude/agent-profiles.json";

/// Reads `agent-profiles.json` and resolves `full_auto` for the given model.
///
/// Falls back to `true` (fail-closed) when the file is missing, unreadable,
/// or does not contain `model_profiles` for the codex provider.
fn resolve_full_auto_from_profiles(model: &str) -> bool {
    #[derive(serde::Deserialize)]
    struct AgentProfiles {
        #[serde(default)]
        providers: std::collections::HashMap<String, ProviderConfig>,
    }

    #[derive(serde::Deserialize)]
    struct ProviderConfig {
        #[serde(default)]
        model_profiles: Option<std::collections::HashMap<String, ModelProfile>>,
    }

    let content = match std::fs::read_to_string(AGENT_PROFILES_PATH) {
        Ok(c) => c,
        Err(_) => return true,
    };
    let profiles: AgentProfiles = match serde_json::from_str(&content) {
        Ok(p) => p,
        Err(_) => return true,
    };
    let codex = match profiles.providers.get("codex") {
        Some(p) => p,
        None => return true,
    };
    resolve_full_auto(model, codex.model_profiles.as_ref())
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
        match extract_verdict_from_session_log(session_log_path) {
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

/// Attempts to extract a verdict JSON from the session log file.
///
/// Handles both single-line and pretty-printed multi-line JSON.
/// Scans backward for JSON objects containing `"verdict"` and `"findings"` keys.
/// Returns `None` if no valid verdict is found.
fn extract_verdict_from_session_log(path: &Path) -> Option<ReviewFinalMessageState> {
    let content = std::fs::read_to_string(path).ok()?;

    // Strategy 1: Check single lines (compact JSON)
    for line in content.lines().rev() {
        let trimmed = line.trim();
        if trimmed.starts_with('{') && trimmed.contains("\"verdict\"") {
            let state = parse_review_final_message(Some(trimmed));
            if matches!(state, ReviewFinalMessageState::Parsed(_)) {
                return Some(state);
            }
        }
    }

    // Strategy 2: Extract multi-line JSON blocks (pretty-printed)
    // Scan backward for '{' ... '}' blocks that contain "verdict"
    let bytes = content.as_bytes();
    let mut end = bytes.len();
    while let Some(close) = content.get(..end).and_then(|s| s.rfind('}')) {
        // Find the matching opening brace by counting brace depth
        let mut depth = 0i32;
        let mut start = None;
        for (i, &b) in bytes.get(..=close).iter().flat_map(|s| s.iter().enumerate().rev()) {
            match b {
                b'}' => depth += 1,
                b'{' => {
                    depth -= 1;
                    if depth == 0 {
                        start = Some(i);
                        break;
                    }
                }
                _ => {}
            }
        }
        if let Some(start) = start {
            if let Some(block) = content.get(start..=close) {
                if block.contains("\"verdict\"") {
                    let state = parse_review_final_message(Some(block));
                    if matches!(state, ReviewFinalMessageState::Parsed(_)) {
                        return Some(state);
                    }
                }
            }
        }
        end = close;
    }

    None
}

fn prepare_session_log_path() -> Result<OutputLastMessagePath, String> {
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

// ---------------------------------------------------------------------------
// record-round: Write review round results to metadata.json
// ---------------------------------------------------------------------------

/// Exit code used when a review escalation block prevents recording a round.
const EXIT_CODE_ESCALATION_BLOCKED: u8 = 3;

fn execute_record_round(args: &RecordRoundArgs) -> ExitCode {
    match run_record_round(args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(RecordRoundError::EscalationBlocked(concerns)) => {
            eprintln!(
                "[BLOCKED] Review escalation active for concerns: {concerns:?}\n\
                 Required actions:\n\
                 \x20 1. Workspace Search: use Grep to check if existing code solves this problem\n\
                 \x20 2. Reinvention Check: invoke researcher capability to survey crates.io\n\
                 \x20 3. Decision: run `sotp review resolve-escalation` with evidence"
            );
            ExitCode::from(EXIT_CODE_ESCALATION_BLOCKED)
        }
        Err(RecordRoundError::Other(msg)) => {
            eprintln!("{msg}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Debug)]
enum RecordRoundError {
    EscalationBlocked(Vec<String>),
    Other(String),
}

impl From<String> for RecordRoundError {
    fn from(s: String) -> Self {
        Self::Other(s)
    }
}

#[allow(clippy::too_many_lines)]
fn run_record_round(args: &RecordRoundArgs) -> Result<(), RecordRoundError> {
    use domain::{ReviewRoundResult, ReviewState, RoundType};
    use infrastructure::git_cli::{GitRepository, SystemGitRepo};
    use infrastructure::track::fs_store::FsTrackStore;

    let round_type = match args.round_type.as_str() {
        "fast" => RoundType::Fast,
        "final" => RoundType::Final,
        other => {
            return Err(RecordRoundError::Other(format!(
                "unknown round type: {other} (expected 'fast' or 'final')"
            )));
        }
    };

    let expected_groups_raw: Vec<String> =
        args.expected_groups.split(',').map(|s| s.trim().to_owned()).collect();
    if expected_groups_raw.is_empty() || expected_groups_raw.iter().any(|g| g.is_empty()) {
        return Err(RecordRoundError::Other(
            "--expected-groups must be a non-empty comma-separated list".to_owned(),
        ));
    }
    let expected_groups: Vec<domain::ReviewGroupName> = expected_groups_raw
        .iter()
        .map(|s| {
            domain::ReviewGroupName::try_new(s.as_str())
                .map_err(|e| RecordRoundError::Other(format!("invalid group name: {e}")))
        })
        .collect::<Result<_, _>>()?;
    let group_name = domain::ReviewGroupName::try_new(args.group.as_str())
        .map_err(|e| RecordRoundError::Other(format!("invalid group name: {e}")))?;

    // Parse concerns from the comma-separated --concerns argument.
    // Dedup and sort via BTreeSet so the stored list is canonical regardless of input order.
    let concerns: Vec<domain::ReviewConcern> = if args.concerns.trim().is_empty() {
        Vec::new()
    } else {
        let parsed: Result<std::collections::BTreeSet<domain::ReviewConcern>, _> =
            args.concerns.split(',').map(|s| domain::ReviewConcern::try_new(s.trim())).collect();
        parsed
            .map_err(|e| RecordRoundError::Other(format!("invalid concern: {e}")))?
            .into_iter()
            .collect()
    };

    // Parse and semantically validate the verdict JSON.
    // parse_review_final_message applies both structural and semantic checks
    // (e.g., zero_findings must have empty findings, findings_remain must have entries).
    let final_message_state = parse_review_final_message(Some(&args.verdict));
    let verdict = match &final_message_state {
        ReviewFinalMessageState::Parsed(payload) => match payload.verdict {
            usecase::review_workflow::ReviewPayloadVerdict::ZeroFindings => {
                domain::Verdict::ZeroFindings
            }
            usecase::review_workflow::ReviewPayloadVerdict::FindingsRemain => {
                domain::Verdict::FindingsRemain
            }
        },
        ReviewFinalMessageState::Missing => {
            return Err(RecordRoundError::Other("--verdict is required".to_owned()));
        }
        ReviewFinalMessageState::Invalid { reason } => {
            return Err(RecordRoundError::Other(format!("invalid --verdict: {reason}")));
        }
    };

    let git = SystemGitRepo::discover()
        .map_err(|e| RecordRoundError::Other(format!("git error: {e}")))?;

    // Compute repo-relative metadata path for normalized hash.
    let metadata_abs = args.items_dir.join(&args.track_id).join("metadata.json");
    let metadata_rel = metadata_abs
        .strip_prefix(git.root())
        .unwrap_or(&metadata_abs)
        .to_string_lossy()
        .into_owned();

    // Open track store.
    let track_id = domain::TrackId::try_new(&args.track_id)
        .map_err(|e| RecordRoundError::Other(format!("invalid track id: {e}")))?;

    let store = FsTrackStore::new(&args.items_dir);

    let timestamp_str = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let timestamp = domain::Timestamp::new(timestamp_str.clone())
        .map_err(|e| RecordRoundError::Other(format!("invalid timestamp: {e}")))?;

    // Step 1: Copy the current git index to a private temp file.
    // All staging operations go through the private index; the real index is
    // only updated at the very end via an atomic fs::rename.  This eliminates
    // the TOCTOU window between the two `store.update` calls in the old design.
    let private_index = PrivateIndex::from_current(&git).map_err(RecordRoundError::Other)?;

    // Step 2: Compute pre-update normalized hash from the private index.
    let pre_update_hash = private_index
        .normalized_tree_hash(&git, &metadata_rel)
        .map_err(|e| RecordRoundError::Other(format!("normalized hash error: {e}")))?;

    // Steps 3–7 are a single atomic read-modify-write via with_locked_document.
    // Correctness relies on single-process sequential execution (no concurrent
    // sotp record-round). Parallel access will use worktree isolation (SPEC-04).
    //
    // Captured variables used inside the closure:
    //   - git, private_index, metadata_rel (git index staging)
    //   - round_type, args.group, verdict_str, timestamp, expected_groups, pre_update_hash
    let mut stale_error: Option<String> = None;
    let mut escalation_error: Option<Vec<String>> = None;
    let with_locked_result = store.with_locked_document(&track_id, |track, meta| {
        // Step 3: Record round with code_hash="PENDING".
        let review = track.review_mut().get_or_insert_with(ReviewState::new);
        let round_num = review
            .groups()
            .get(&group_name)
            .and_then(|g| match round_type {
                RoundType::Fast => g.fast().map(|r| r.round()),
                RoundType::Final => g.final_round().map(|r| r.round()),
            })
            .map(|n| n.saturating_add(1))
            .unwrap_or(1);

        let result = if concerns.is_empty() {
            ReviewRoundResult::new(round_num, verdict, timestamp.clone())
        } else {
            ReviewRoundResult::new_with_concerns(
                round_num,
                verdict,
                timestamp.clone(),
                concerns.clone(),
            )
        };
        match review.record_round_with_pending(
            round_type,
            &group_name,
            result,
            &expected_groups,
            &pre_update_hash,
        ) {
            Ok(()) => {}
            Err(domain::ReviewError::EscalationActive { concerns: blocked }) => {
                // Escalation is active — signal via sentinel and abort the closure
                // with an Err so that with_locked_document does NOT write metadata.json.
                escalation_error = Some(blocked);
                return Err(domain::DomainError::Validation(
                    domain::ValidationError::InvalidTaskId(
                        "escalation-blocked-sentinel".to_owned(),
                    ),
                ));
            }
            Err(domain::ReviewError::StaleCodeHash { expected, actual }) => {
                // Persist the invalidated state, then signal the caller.
                // Returning Ok here causes with_locked_document to write
                // the (now-invalidated) track to disk.
                stale_error = Some(format!(
                    "code hash mismatch: review recorded against {expected}, \
                         but current code is {actual} — review.status set to invalidated"
                ));
                meta.updated_at = timestamp.to_string();
                return Ok(());
            }
            Err(e) => {
                return Err(domain::DomainError::Validation(
                    domain::ValidationError::InvalidTaskId(e.to_string()),
                ));
            }
        }

        // Step 4: Set updated_at (not auto-set by with_locked_document).
        meta.updated_at = timestamp.to_string();

        // Step 5: Serialize PENDING state and stage into private index.
        let pending_json = infrastructure::track::codec::encode(track, meta).map_err(|e| {
            domain::DomainError::Validation(domain::ValidationError::InvalidTaskId(format!(
                "codec encode error: {e}"
            )))
        })?;
        // Append trailing newline for POSIX compatibility, matching write_track.
        let pending_content = format!("{pending_json}\n");
        private_index.stage_bytes(&git, &metadata_rel, pending_content.as_bytes()).map_err(
            |e| domain::DomainError::Validation(domain::ValidationError::InvalidTaskId(e)),
        )?;

        // Step 6: Compute normalized hash H1 from the private index (PENDING state staged).
        let h1 = private_index.normalized_tree_hash(&git, &metadata_rel).map_err(|e| {
            domain::DomainError::Validation(domain::ValidationError::InvalidTaskId(format!(
                "post-pending normalized hash error: {e}"
            )))
        })?;

        // Step 7: Replace PENDING with the real code_hash.
        if let Some(r) = track.review_mut().as_mut() {
            r.set_code_hash(h1).map_err(|e| {
                domain::DomainError::Validation(domain::ValidationError::InvalidTaskId(format!(
                    "set_code_hash error: {e}"
                )))
            })?;
        }

        // Step 8: Serialize final state and stage into private index.
        // with_locked_document will write this same state to disk.
        let final_json = infrastructure::track::codec::encode(track, meta).map_err(|e| {
            domain::DomainError::Validation(domain::ValidationError::InvalidTaskId(format!(
                "codec encode error (final): {e}"
            )))
        })?;
        let final_content = format!("{final_json}\n");
        private_index.stage_bytes(&git, &metadata_rel, final_content.as_bytes()).map_err(|e| {
            domain::DomainError::Validation(domain::ValidationError::InvalidTaskId(e))
        })?;

        Ok(())
    });

    // Escalation block — the closure returned Err (sentinel), so with_locked_document
    // did NOT write metadata.json. Check escalation_error before propagating other errors.
    if let Some(blocked_concerns) = escalation_error {
        return Err(RecordRoundError::EscalationBlocked(blocked_concerns));
    }

    // Propagate any real (non-escalation) error from with_locked_document.
    // Extract inner message from InvalidTaskId sentinel wrapping (CLI-02 will fix properly).
    with_locked_result.map_err(|e| {
        let msg = e.to_string();
        let cleaned = if let Some(inner) = msg.strip_prefix("task id '") {
            inner.strip_suffix("' must match the pattern T<digits>").unwrap_or(inner).to_owned()
        } else {
            msg
        };
        RecordRoundError::Other(format!("record-round failed: {cleaned}"))
    })?;

    if let Some(err_msg) = stale_error {
        // private_index is dropped without swap — real index is untouched.
        return Err(RecordRoundError::Other(format!("[BLOCKED] {err_msg}")));
    }

    // Step 9: Atomically rename private index over the real index.
    // If any earlier step failed, private_index would be dropped without swap,
    // leaving the real index entirely untouched.
    private_index.swap_into_real().map_err(RecordRoundError::Other)?;

    eprintln!("[OK] Recorded {round_type} round for group '{}' (verdict: {verdict})", args.group);
    Ok(())
}

// ---------------------------------------------------------------------------
// resolve-escalation: Resolve an active review escalation block
// ---------------------------------------------------------------------------

fn execute_resolve_escalation(args: &ResolveEscalationArgs) -> ExitCode {
    match run_resolve_escalation(args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            eprintln!("{msg}");
            ExitCode::FAILURE
        }
    }
}

fn run_resolve_escalation(args: &ResolveEscalationArgs) -> Result<(), String> {
    use domain::{ReviewEscalationDecision, ReviewEscalationResolution};
    use infrastructure::track::fs_store::FsTrackStore;

    let decision = match args.decision.as_str() {
        "adopt_workspace" => ReviewEscalationDecision::AdoptWorkspaceSolution,
        "adopt_crate" => ReviewEscalationDecision::AdoptExternalCrate,
        "continue_self" => ReviewEscalationDecision::ContinueSelfImplementation,
        other => {
            return Err(format!(
                "unknown decision: {other}. Use: adopt_workspace, adopt_crate, or continue_self"
            ));
        }
    };

    // Parse caller-supplied blocked concerns. Deduplicate and sort via BTreeSet so
    // that order-insensitive comparisons in the domain layer are stable.
    // The domain layer validates the match against the active escalation block via
    // ReviewEscalationResolution::new.
    let blocked_concerns: Vec<domain::ReviewConcern> = {
        let parsed: Result<std::collections::BTreeSet<_>, _> = args
            .blocked_concerns
            .split(',')
            .map(|s| {
                domain::ReviewConcern::try_new(s.trim())
                    .map_err(|e| format!("invalid blocked concern: {e}"))
            })
            .collect();
        parsed?.into_iter().collect()
    };

    let track_id =
        domain::TrackId::try_new(&args.track_id).map_err(|e| format!("invalid track id: {e}"))?;

    let store = FsTrackStore::new(&args.items_dir);

    let timestamp_str = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let timestamp =
        domain::Timestamp::new(timestamp_str).map_err(|e| format!("invalid timestamp: {e}"))?;

    store
        .with_locked_document(&track_id, |track, meta| {
            let review = track.review_mut().as_mut().ok_or_else(|| {
                domain::DomainError::Validation(domain::ValidationError::InvalidTaskId(
                    "no review section in metadata.json".to_owned(),
                ))
            })?;

            // Check escalation is active before verifying evidence artifacts.
            // This ensures callers get "EscalationNotActive" rather than
            // "artifact not found" when no escalation is in progress.
            if !review.escalation().is_blocked() {
                return Err(domain::DomainError::Validation(
                    domain::ValidationError::InvalidTaskId(
                        "no active escalation block; cannot resolve".to_owned(),
                    ),
                ));
            }

            // Verify evidence artifacts exist before committing to the resolution.
            if !std::path::Path::new(&args.workspace_search_ref).exists() {
                return Err(domain::DomainError::Validation(
                    domain::ValidationError::InvalidTaskId(format!(
                        "workspace search artifact not found: {}",
                        args.workspace_search_ref
                    )),
                ));
            }
            if !std::path::Path::new(&args.reinvention_check_ref).exists() {
                return Err(domain::DomainError::Validation(
                    domain::ValidationError::InvalidTaskId(format!(
                        "reinvention check artifact not found: {}",
                        args.reinvention_check_ref
                    )),
                ));
            }

            let resolution = ReviewEscalationResolution::new(
                blocked_concerns.clone(),
                args.workspace_search_ref.clone(),
                args.reinvention_check_ref.clone(),
                decision,
                args.summary.clone(),
                timestamp.clone(),
            )
            .map_err(|e| {
                domain::DomainError::Validation(domain::ValidationError::InvalidTaskId(
                    e.to_string(),
                ))
            })?;

            review.resolve_escalation(resolution).map_err(|e| {
                domain::DomainError::Validation(domain::ValidationError::InvalidTaskId(
                    e.to_string(),
                ))
            })?;

            meta.updated_at = timestamp.to_string();
            Ok(())
        })
        .map_err(|e| {
            // Extract the inner message from InvalidTaskId sentinel wrapping.
            // The closure uses InvalidTaskId as a generic error carrier because
            // with_locked_document constrains the return type to DomainError.
            // This will be cleaned up in CLI-02 (usecase layer extraction).
            let msg = e.to_string();
            if let Some(inner) = msg.strip_prefix("task id '") {
                if let Some(inner) = inner.strip_suffix("' must match the pattern T<digits>") {
                    return format!("resolve-escalation failed: {inner}");
                }
            }
            format!("resolve-escalation failed: {msg}")
        })?;

    println!("[OK] Escalation resolved: {}", args.decision);
    Ok(())
}

// ---------------------------------------------------------------------------
// PrivateIndex: POSIX-atomic index update via private copy + rename
// ---------------------------------------------------------------------------

/// A private copy of the git index used to stage files without touching the
/// real `.git/index` until all operations succeed.
///
/// All git commands that read or modify the index operate on `temp_path`
/// via `GIT_INDEX_FILE`.  When every step has succeeded, [`swap_into_real`]
/// atomically renames `temp_path` over `real_index_path`.  If the
/// `PrivateIndex` is dropped before [`swap_into_real`] is called, the temp
/// file is removed and the real index is left completely untouched.
///
/// [`swap_into_real`]: PrivateIndex::swap_into_real
struct PrivateIndex {
    temp_path: PathBuf,
    real_index_path: PathBuf,
    swapped: bool,
}

impl PrivateIndex {
    /// Copy the current git index to a temp file in the same directory as the
    /// real index (required for `fs::rename` to be atomic on POSIX).
    ///
    /// Uses `git rev-parse --git-path index` to find the real index path so
    /// that linked worktrees are handled correctly.
    fn from_current(git: &impl infrastructure::git_cli::GitRepository) -> Result<Self, String> {
        // Resolve the real index path (worktree-safe).
        let real_index_path = resolve_real_index_path(git)?;

        // Place the temp file in the same directory so rename is atomic.
        let temp_dir = real_index_path.parent().ok_or_else(|| {
            format!("git index path has no parent directory: {}", real_index_path.display())
        })?;
        let temp_path = temp_dir.join(format!(
            "sotp-private-index-{}-{}.tmp",
            std::process::id(),
            // Pointer address of a stack local used as a secondary disambiguator.
            // This is stable within a single call site and avoids collisions
            // when multiple PrivateIndex values are alive concurrently.
            {
                let marker: u8 = 0;
                std::ptr::from_ref(&marker) as usize
            }
        ));

        std::fs::copy(&real_index_path, &temp_path).map_err(|e| {
            format!(
                "failed to copy git index {} -> {}: {e}",
                real_index_path.display(),
                temp_path.display()
            )
        })?;

        Ok(Self { temp_path, real_index_path, swapped: false })
    }

    /// Compute the normalized tree hash from the private index.
    ///
    /// The normalization is identical to `index_tree_hash_normalizing` in
    /// `git_cli.rs`: `review.code_hash` is set to `"PENDING"` and
    /// `updated_at` is set to the Unix epoch.  A second temp copy of the
    /// private index is used for the write-tree operation so the private
    /// index itself is not modified.
    #[allow(clippy::too_many_lines)]
    fn normalized_tree_hash(
        &self,
        git: &impl infrastructure::git_cli::GitRepository,
        metadata_path: &str,
    ) -> Result<String, infrastructure::git_cli::GitError> {
        use infrastructure::git_cli::GitError;
        use std::io::Write as _;

        // Step 1: Read the metadata.json blob from the private index.
        let show_output = std::process::Command::new("git")
            .args(["show", &format!(":{metadata_path}")])
            .current_dir(git.root())
            .env("GIT_INDEX_FILE", &self.temp_path)
            .output()
            .map_err(|source| GitError::Spawn {
                command: format!("show :{metadata_path}"),
                source,
            })?;
        if !show_output.status.success() {
            let stderr = String::from_utf8_lossy(&show_output.stderr).trim().to_owned();
            let code = show_output.status.code().unwrap_or(-1);
            return Err(GitError::CommandFailed {
                command: format!("show :{metadata_path}"),
                code,
                stderr,
            });
        }
        let blob_content = String::from_utf8_lossy(&show_output.stdout);

        // Step 2: Parse as JSON.
        let mut json: serde_json::Value =
            serde_json::from_str(&blob_content).map_err(|e| GitError::CommandFailed {
                command: format!("parse {metadata_path}"),
                code: -1,
                stderr: e.to_string(),
            })?;

        // Step 3: Normalize volatile fields.
        if let serde_json::Value::Object(obj) = &mut json {
            obj.insert(
                "updated_at".to_owned(),
                serde_json::Value::String("1970-01-01T00:00:00Z".to_owned()),
            );
            let review = obj
                .entry("review")
                .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
            if let serde_json::Value::Object(review_obj) = review {
                review_obj.insert(
                    "code_hash".to_owned(),
                    serde_json::Value::String("PENDING".to_owned()),
                );
            }
        }

        // Step 4: Serialize deterministically.
        let normalized =
            serde_json::to_string_pretty(&json).map_err(|e| GitError::CommandFailed {
                command: format!("serialize {metadata_path}"),
                code: -1,
                stderr: e.to_string(),
            })?;

        // Step 5: Write normalized blob to object store.
        let mut hash_object_child = std::process::Command::new("git")
            .args(["hash-object", "-w", "--stdin"])
            .current_dir(git.root())
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|source| GitError::Spawn {
                command: "hash-object -w --stdin".to_owned(),
                source,
            })?;
        if let Some(ref mut stdin) = hash_object_child.stdin {
            stdin.write_all(normalized.as_bytes()).map_err(|source| GitError::Spawn {
                command: "hash-object write stdin".to_owned(),
                source,
            })?;
        }
        let hash_object_output = hash_object_child.wait_with_output().map_err(|source| {
            GitError::Spawn { command: "hash-object -w --stdin (wait)".to_owned(), source }
        })?;
        if !hash_object_output.status.success() {
            let stderr = String::from_utf8_lossy(&hash_object_output.stderr).trim().to_owned();
            let code = hash_object_output.status.code().unwrap_or(-1);
            return Err(GitError::CommandFailed {
                command: "hash-object -w --stdin".to_owned(),
                code,
                stderr,
            });
        }
        let blob_hash = String::from_utf8_lossy(&hash_object_output.stdout).trim().to_owned();

        // Step 6: Copy private index to a second temp file for write-tree
        // (write-tree modifies internal index state, so we must not use
        // self.temp_path directly).
        let norm_temp = self.temp_path.with_extension("norm.tmp");
        std::fs::copy(&self.temp_path, &norm_temp).map_err(|source| GitError::Spawn {
            command: "copy private index for write-tree".to_owned(),
            source,
        })?;

        // Step 7: Update the norm copy with the normalized blob.
        let update_output = std::process::Command::new("git")
            .args(["update-index", "--cacheinfo", &format!("100644,{blob_hash},{metadata_path}")])
            .current_dir(git.root())
            .env("GIT_INDEX_FILE", &norm_temp)
            .output()
            .map_err(|source| GitError::Spawn {
                command: "update-index --cacheinfo (norm)".to_owned(),
                source,
            })?;
        if !update_output.status.success() {
            let _ = std::fs::remove_file(&norm_temp);
            let stderr = String::from_utf8_lossy(&update_output.stderr).trim().to_owned();
            let code = update_output.status.code().unwrap_or(-1);
            return Err(GitError::CommandFailed {
                command: "update-index --cacheinfo (norm)".to_owned(),
                code,
                stderr,
            });
        }

        // Step 8: Write tree from the norm copy.
        let write_tree_output = std::process::Command::new("git")
            .args(["write-tree"])
            .current_dir(git.root())
            .env("GIT_INDEX_FILE", &norm_temp)
            .output()
            .map_err(|source| GitError::Spawn {
                command: "write-tree (private-norm)".to_owned(),
                source,
            })?;

        // Step 9: Clean up the norm copy unconditionally.
        let _ = std::fs::remove_file(&norm_temp);

        if !write_tree_output.status.success() {
            let stderr = String::from_utf8_lossy(&write_tree_output.stderr).trim().to_owned();
            let code = write_tree_output.status.code().unwrap_or(-1);
            return Err(GitError::CommandFailed {
                command: "write-tree (private-norm)".to_owned(),
                code,
                stderr,
            });
        }

        Ok(String::from_utf8_lossy(&write_tree_output.stdout).trim().to_owned())
    }

    /// Stage raw bytes as a blob for the given repo-relative path in the private index.
    ///
    /// Feeds `content` to `git hash-object -w --stdin` to write a blob to the
    /// object store, then updates the private index entry with
    /// `git update-index --cacheinfo`.
    fn stage_bytes(
        &self,
        git: &impl infrastructure::git_cli::GitRepository,
        rel_path: &str,
        content: &[u8],
    ) -> Result<(), String> {
        use std::io::Write as _;

        // Step 1: Write blob to object store via stdin.
        let mut child = std::process::Command::new("git")
            .args(["hash-object", "-w", "--stdin"])
            .current_dir(git.root())
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("failed to spawn git hash-object for {rel_path}: {e}"))?;

        if let Some(ref mut stdin) = child.stdin {
            stdin
                .write_all(content)
                .map_err(|e| format!("failed to write content to hash-object stdin: {e}"))?;
        }

        let hash_output = child
            .wait_with_output()
            .map_err(|e| format!("failed to wait for git hash-object: {e}"))?;
        if !hash_output.status.success() {
            let stderr = String::from_utf8_lossy(&hash_output.stderr).trim().to_owned();
            return Err(format!("git hash-object failed for {rel_path}: {stderr}"));
        }
        let blob_hash = String::from_utf8_lossy(&hash_output.stdout).trim().to_owned();

        // Step 2: Update the private index entry.
        let update_output = std::process::Command::new("git")
            .args(["update-index", "--cacheinfo", &format!("100644,{blob_hash},{rel_path}")])
            .current_dir(git.root())
            .env("GIT_INDEX_FILE", &self.temp_path)
            .output()
            .map_err(|e| format!("failed to update-index for {rel_path}: {e}"))?;
        if !update_output.status.success() {
            let stderr = String::from_utf8_lossy(&update_output.stderr).trim().to_owned();
            return Err(format!("git update-index failed for {rel_path}: {stderr}"));
        }

        Ok(())
    }

    /// Atomically replace the real index using git's own lockfile protocol.
    ///
    /// 1. Create `<index>.lock` with `O_CREAT|O_EXCL` — blocks concurrent
    ///    git operations (they also use this lock before touching the index).
    /// 2. Copy our private index content into the lock file.
    /// 3. Rename `<index>.lock` → `<index>` — atomic on POSIX, and this is
    ///    exactly how git itself commits index changes.
    ///
    /// The lock is held only for the copy+rename duration (microseconds),
    /// so it does not interfere with our earlier `git hash-object` /
    /// `git update-index` calls which operate on the private index.
    fn swap_into_real(mut self) -> Result<(), String> {
        let lock_path = self.real_index_path.with_extension("lock");
        // Acquire git's index lock — fails if another git operation holds it.
        std::fs::OpenOptions::new().write(true).create_new(true).open(&lock_path).map_err(|e| {
            format!(
                "failed to acquire index lock at {}: {e}. \
                     A concurrent git operation may be in progress.",
                lock_path.display()
            )
        })?;
        // Copy private index content into the lock file.
        if let Err(e) = std::fs::copy(&self.temp_path, &lock_path) {
            let _ = std::fs::remove_file(&lock_path);
            return Err(format!("failed to write index lock: {e}"));
        }
        // Atomic rename: lock file becomes the real index.
        // This is git's own commit protocol for index updates.
        if let Err(e) = std::fs::rename(&lock_path, &self.real_index_path) {
            let _ = std::fs::remove_file(&lock_path);
            return Err(format!("failed to rename index lock to index: {e}"));
        }
        // Clean up the temp private index.
        let _ = std::fs::remove_file(&self.temp_path);
        self.swapped = true;
        Ok(())
    }
}

impl Drop for PrivateIndex {
    fn drop(&mut self) {
        if !self.swapped {
            let _ = std::fs::remove_file(&self.temp_path);
        }
    }
}

/// Resolve the absolute path of the real git index, worktree-safe.
///
/// Uses `git rev-parse --git-path index` which correctly resolves the index
/// path even in linked worktrees where `.git` is a pointer file.
fn resolve_real_index_path(
    git: &impl infrastructure::git_cli::GitRepository,
) -> Result<PathBuf, String> {
    let output = git
        .output(&["rev-parse", "--git-path", "index"])
        .map_err(|e| format!("failed to resolve git index path: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        return Err(format!("git rev-parse --git-path index failed: {stderr}"));
    }
    let resolved = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let path = if std::path::Path::new(&resolved).is_absolute() {
        PathBuf::from(resolved)
    } else {
        git.root().join(resolved)
    };
    Ok(path)
}

// ---------------------------------------------------------------------------
// check-approved: Verify review.status == approved with current code hash
// ---------------------------------------------------------------------------

fn execute_check_approved(args: &CheckApprovedArgs) -> ExitCode {
    match run_check_approved(args) {
        Ok(()) => {
            eprintln!("[OK] Review is approved and code hash is current");
            ExitCode::SUCCESS
        }
        Err(msg) => {
            eprintln!("{msg}");
            ExitCode::FAILURE
        }
    }
}

fn run_check_approved(args: &CheckApprovedArgs) -> Result<(), String> {
    use infrastructure::git_cli::{GitRepository, SystemGitRepo};
    use infrastructure::track::fs_store::FsTrackStore;

    let git = SystemGitRepo::discover().map_err(|e| format!("git error: {e}"))?;

    // Compute repo-relative metadata path for normalized hash.
    let metadata_abs = args.items_dir.join(&args.track_id).join("metadata.json");
    let metadata_rel = metadata_abs
        .strip_prefix(git.root())
        .unwrap_or(&metadata_abs)
        .to_string_lossy()
        .into_owned();

    // Use normalized hash: review.code_hash → "PENDING", updated_at → epoch.
    let code_hash = git
        .index_tree_hash_normalizing(&metadata_rel)
        .map_err(|e| format!("normalized hash error: {e}"))?;

    let track_id =
        domain::TrackId::try_new(&args.track_id).map_err(|e| format!("invalid track id: {e}"))?;

    let store = FsTrackStore::new(&args.items_dir);

    // Phase 1: Read-only check. On success, return without writing metadata.json.
    use domain::TrackReader;
    let track = store
        .find(&track_id)
        .map_err(|e| format!("failed to read track: {e}"))?
        .ok_or_else(|| format!("track '{}' not found", args.track_id))?;

    let review = track.review().ok_or("[BLOCKED] no review section in metadata.json")?;

    // WF-54: Allow commit when review is in its initial state (no rounds recorded).
    // NotStarted + empty groups = freshly created track (planning artifacts only).
    // NotStarted + non-empty groups = demoted after findings_remain — must re-review.
    if review.status() == domain::ReviewStatus::NotStarted && review.groups().is_empty() {
        return Ok(());
    }

    let mut review_check = review.clone();
    match review_check.check_commit_ready(&code_hash) {
        Ok(()) => return Ok(()),
        Err(domain::ReviewError::StaleCodeHash { .. }) => {
            // Phase 2: Persist invalidation under lock with re-check to prevent TOCTOU.
            use domain::TrackWriter;
            let mut invalidation_msg: Option<String> = None;
            store
                .update(&track_id, |track| {
                    if let Some(r) = track.review_mut().as_mut() {
                        match r.check_commit_ready(&code_hash) {
                            Ok(()) => {} // Refreshed by another process — no invalidation
                            Err(domain::ReviewError::StaleCodeHash { expected, actual }) => {
                                invalidation_msg = Some(format!(
                                    "[BLOCKED] code hash mismatch: recorded against {expected}, \
                                     current is {actual} — review.status set to invalidated"
                                ));
                            }
                            Err(e) => {
                                invalidation_msg =
                                    Some(format!("[BLOCKED] Review guard failed: {e}"));
                            }
                        }
                    }
                    Ok(())
                })
                .map_err(|e| format!("failed to persist invalidation: {e}"))?;

            if let Some(msg) = invalidation_msg {
                return Err(msg);
            }
        }
        Err(e) => return Err(format!("[BLOCKED] Review guard failed: {e}")),
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::{
        CODEX_BIN_ENV, CodexLocalArgs, ReviewRunResult, build_codex_invocation, build_prompt,
        render_codex_local_result, run_codex_local,
    };
    use std::env;
    use std::ffi::OsString;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::{Mutex, OnceLock};
    #[cfg(unix)]
    use std::time::Duration;
    use usecase::review_workflow::ReviewVerdict;

    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    struct EnvVarGuard {
        key: &'static str,
        original: Option<OsString>,
    }

    impl EnvVarGuard {
        fn set(key: &'static str, value: &std::ffi::OsStr) -> Self {
            let original = env::var_os(key);
            // SAFETY: tests serialize access via env_lock(), so mutating process env here is safe.
            unsafe { env::set_var(key, value) };
            Self { key, original }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            match &self.original {
                Some(value) => {
                    // SAFETY: tests serialize access via env_lock(), so mutating process env here is safe.
                    unsafe { env::set_var(self.key, value) };
                }
                None => {
                    // SAFETY: tests serialize access via env_lock(), so mutating process env here is safe.
                    unsafe { env::remove_var(self.key) };
                }
            }
        }
    }

    fn fake_args(
        prompt: Option<String>,
        briefing_file: Option<PathBuf>,
        output_last_message: PathBuf,
        timeout_seconds: u64,
    ) -> CodexLocalArgs {
        CodexLocalArgs {
            model: "gpt-5.4".to_owned(),
            timeout_seconds,
            briefing_file,
            prompt,
            output_last_message: Some(output_last_message),
        }
    }

    struct CurrentDirGuard {
        original: PathBuf,
    }

    impl CurrentDirGuard {
        fn change_to(path: &Path) -> Self {
            let original = env::current_dir().unwrap();
            env::set_current_dir(path).unwrap();
            Self { original }
        }
    }

    impl Drop for CurrentDirGuard {
        fn drop(&mut self) {
            env::set_current_dir(&self.original).unwrap();
        }
    }

    #[cfg(unix)]
    fn process_is_gone_or_zombie(pid: i32) -> bool {
        // Safety: kill with signal 0 only probes whether the process still exists.
        let status = unsafe { libc::kill(pid, 0) };
        let err = std::io::Error::last_os_error();
        if status != 0 && err.raw_os_error() == Some(libc::ESRCH) {
            return true;
        }

        let stat_path = PathBuf::from(format!("/proc/{pid}/stat"));
        let Ok(stat) = fs::read_to_string(stat_path) else {
            return false;
        };
        stat.split_whitespace().nth(2) == Some("Z")
    }

    fn write_fake_codex_script(root: &Path) -> PathBuf {
        let script = root.join("fake-codex.sh");
        fs::write(
            &script,
            r#"#!/bin/sh
set -eu
args_file="${SOTP_FAKE_CODEX_ARGS_FILE:-}"
if [ -n "$args_file" ]; then
  printf '%s\n' "$@" > "$args_file"
fi
out=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --output-last-message)
      out="$2"
      shift 2
      ;;
    *)
      shift
      ;;
  esac
done
sleep_seconds="${SOTP_FAKE_CODEX_SLEEP_SECONDS:-0}"
if [ "$sleep_seconds" != "0" ]; then
  sleep "$sleep_seconds"
fi
message="${SOTP_FAKE_CODEX_MESSAGE:-}"
if [ -n "$message" ] && [ -n "$out" ]; then
  printf '%s\n' "$message" > "$out"
fi
exit "${SOTP_FAKE_CODEX_EXIT_CODE:-0}"
"#,
        )
        .unwrap();
        #[cfg(unix)]
        {
            let mut perms = fs::metadata(&script).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&script, perms).unwrap();
        }
        script
    }

    #[test]
    fn build_prompt_uses_briefing_file_reference() {
        let dir = tempfile::tempdir().unwrap();
        let briefing = dir.path().join("briefing.md");
        fs::write(&briefing, "# Task\n").unwrap();
        let args = fake_args(None, Some(briefing.clone()), dir.path().join("out.txt"), 1);

        let prompt = build_prompt(&args).unwrap();

        assert_eq!(
            prompt,
            format!("Read {} and perform the task described there.", briefing.display())
        );
    }

    #[test]
    fn build_codex_invocation_omits_full_auto_when_false() {
        let invocation = build_codex_invocation(
            "gpt-5.3-codex-spark",
            "Review this change.",
            Path::new("tmp/reviewer-runtime/out.txt"),
            Path::new("tmp/reviewer-runtime/schema.json"),
            false,
        );
        let rendered =
            invocation.args.iter().map(|arg| arg.to_string_lossy().to_string()).collect::<Vec<_>>();

        assert_eq!(rendered.first().map(String::as_str), Some("exec"));
        assert!(rendered.windows(2).any(|pair| pair == ["--sandbox", "read-only"]));
        assert!(
            rendered
                .windows(2)
                .any(|pair| pair == ["--output-schema", "tmp/reviewer-runtime/schema.json"])
        );
        assert!(rendered.windows(2).any(|pair| pair == ["--model", "gpt-5.3-codex-spark"]));
        assert!(!rendered.iter().any(|arg| arg == "--full-auto"));
    }

    #[test]
    fn build_codex_invocation_includes_full_auto_then_read_only_when_true() {
        let invocation = build_codex_invocation(
            "gpt-5.4",
            "Review this change.",
            Path::new("tmp/reviewer-runtime/out.txt"),
            Path::new("tmp/reviewer-runtime/schema.json"),
            true,
        );
        let rendered =
            invocation.args.iter().map(|arg| arg.to_string_lossy().to_string()).collect::<Vec<_>>();

        assert_eq!(rendered.first().map(String::as_str), Some("exec"));
        assert!(rendered.iter().any(|arg| arg == "--full-auto"));
        // --sandbox read-only must appear AFTER --full-auto to override workspace-write
        let full_auto_pos = rendered.iter().position(|a| a == "--full-auto").unwrap();
        let sandbox_pos =
            rendered.windows(2).position(|p| p == ["--sandbox", "read-only"]).unwrap();
        assert!(
            sandbox_pos > full_auto_pos,
            "--sandbox read-only must come after --full-auto to override its implicit workspace-write"
        );
        assert!(rendered.windows(2).any(|pair| pair == ["--model", "gpt-5.4"]));
    }

    #[test]
    fn render_codex_local_result_emits_zero_findings_json_to_stdout() {
        let rendered = render_codex_local_result(
            &fake_args(
                Some("Review this implementation.".to_owned()),
                None,
                PathBuf::from("tmp/reviewer-runtime/out.txt"),
                1,
            ),
            Ok(ReviewRunResult {
                verdict: ReviewVerdict::ZeroFindings,
                final_message: Some("{\"verdict\":\"zero_findings\",\"findings\":[]}".to_owned()),
                output_last_message: PathBuf::from("tmp/reviewer-runtime/out.txt"),
                output_last_message_auto_managed: false,
                verdict_detail: None,
            }),
        );

        assert_eq!(rendered.exit_code, 0);
        assert_eq!(
            rendered.stdout_lines,
            vec!["{\"verdict\":\"zero_findings\",\"findings\":[]}".to_owned()]
        );
        assert!(rendered.stderr_lines.is_empty());
    }

    #[test]
    fn render_codex_local_result_emits_findings_json_to_stdout() {
        let rendered = render_codex_local_result(
            &fake_args(
                Some("Review this implementation.".to_owned()),
                None,
                PathBuf::from("tmp/reviewer-runtime/out.txt"),
                1,
            ),
            Ok(ReviewRunResult {
                verdict: ReviewVerdict::FindingsRemain,
                final_message: Some(
                    "{\"verdict\":\"findings_remain\",\"findings\":[{\"message\":\"P1: finding\",\"severity\":\"P1\",\"file\":null,\"line\":null}]}".to_owned(),
                ),
                output_last_message: PathBuf::from("tmp/reviewer-runtime/out.txt"),
                output_last_message_auto_managed: false,
                verdict_detail: None,
            }),
        );

        assert_eq!(rendered.exit_code, 2);
        assert_eq!(
            rendered.stdout_lines,
            vec![
                "{\"verdict\":\"findings_remain\",\"findings\":[{\"message\":\"P1: finding\",\"severity\":\"P1\",\"file\":null,\"line\":null}]}"
                    .to_owned(),
            ]
        );
        assert!(rendered.stderr_lines.is_empty());
    }

    #[test]
    fn render_codex_local_result_hides_auto_managed_path_for_timeout() {
        let rendered = render_codex_local_result(
            &fake_args(
                Some("Review this implementation.".to_owned()),
                None,
                PathBuf::from("tmp/reviewer-runtime/out.txt"),
                1,
            ),
            Ok(ReviewRunResult {
                verdict: ReviewVerdict::Timeout,
                final_message: None,
                output_last_message: PathBuf::from("tmp/reviewer-runtime/out.txt"),
                output_last_message_auto_managed: true,
                verdict_detail: None,
            }),
        );

        assert_eq!(rendered.exit_code, 1);
        assert_eq!(rendered.stderr_lines, vec!["[TIMEOUT] Local reviewer exceeded 1s".to_owned()]);
    }

    #[test]
    fn render_codex_local_result_keeps_explicit_path_for_missing_message() {
        let rendered = render_codex_local_result(
            &fake_args(
                Some("Review this implementation.".to_owned()),
                None,
                PathBuf::from("tmp/reviewer-runtime/out.txt"),
                1,
            ),
            Ok(ReviewRunResult {
                verdict: ReviewVerdict::LastMessageMissing,
                final_message: None,
                output_last_message: PathBuf::from("tmp/reviewer-runtime/out.txt"),
                output_last_message_auto_managed: false,
                verdict_detail: None,
            }),
        );

        assert_eq!(rendered.exit_code, 1);
        assert_eq!(
            rendered.stderr_lines,
            vec![
                "[ERROR] Local reviewer finished without a final message: tmp/reviewer-runtime/out.txt"
                    .to_owned()
            ]
        );
    }

    #[test]
    fn run_codex_local_reports_zero_findings() {
        let _lock = env_lock().lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let script = write_fake_codex_script(dir.path());
        let output = dir.path().join("last.txt");
        let _bin = EnvVarGuard::set(CODEX_BIN_ENV, script.as_os_str());
        let _message = EnvVarGuard::set(
            "SOTP_FAKE_CODEX_MESSAGE",
            std::ffi::OsStr::new("{\"verdict\":\"zero_findings\",\"findings\":[]}"),
        );

        let result = run_codex_local(&fake_args(
            Some("Review this implementation.".to_owned()),
            None,
            output.clone(),
            1,
        ))
        .unwrap();

        assert_eq!(result.verdict, ReviewVerdict::ZeroFindings);
        assert_eq!(
            result.final_message.as_deref(),
            Some("{\"verdict\":\"zero_findings\",\"findings\":[]}")
        );
        assert_eq!(result.output_last_message, output);
    }

    #[test]
    fn run_codex_local_reports_findings_when_final_message_is_present() {
        let _lock = env_lock().lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let script = write_fake_codex_script(dir.path());
        let output = dir.path().join("last.txt");
        let _bin = EnvVarGuard::set(CODEX_BIN_ENV, script.as_os_str());
        let _message = EnvVarGuard::set(
            "SOTP_FAKE_CODEX_MESSAGE",
            std::ffi::OsStr::new(
                "{\"verdict\":\"findings_remain\",\"findings\":[{\"message\":\"P1: review finding\",\"severity\":\"P1\",\"file\":null,\"line\":null}]}",
            ),
        );

        let result = run_codex_local(&fake_args(
            Some("Review this implementation.".to_owned()),
            None,
            output,
            1,
        ))
        .unwrap();

        assert_eq!(result.verdict, ReviewVerdict::FindingsRemain);
        assert_eq!(
            result.final_message.as_deref(),
            Some(
                "{\"verdict\":\"findings_remain\",\"findings\":[{\"message\":\"P1: review finding\",\"severity\":\"P1\",\"file\":null,\"line\":null}]}"
            )
        );
    }

    #[test]
    fn run_codex_local_reports_process_failed_when_findings_payload_has_nonzero_exit() {
        let _lock = env_lock().lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let script = write_fake_codex_script(dir.path());
        let output = dir.path().join("last.txt");
        let _bin = EnvVarGuard::set(CODEX_BIN_ENV, script.as_os_str());
        let _message = EnvVarGuard::set(
            "SOTP_FAKE_CODEX_MESSAGE",
            std::ffi::OsStr::new(
                "{\"verdict\":\"findings_remain\",\"findings\":[{\"message\":\"P1: review finding\",\"severity\":\"P1\",\"file\":null,\"line\":null}]}",
            ),
        );
        let _code = EnvVarGuard::set("SOTP_FAKE_CODEX_EXIT_CODE", std::ffi::OsStr::new("1"));

        let result = run_codex_local(&fake_args(
            Some("Review this implementation.".to_owned()),
            None,
            output,
            1,
        ))
        .unwrap();

        assert_eq!(result.verdict, ReviewVerdict::ProcessFailed);
    }

    #[test]
    fn run_codex_local_canonicalizes_pretty_printed_final_json() {
        let _lock = env_lock().lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let script = write_fake_codex_script(dir.path());
        let output = dir.path().join("last.txt");
        let _bin = EnvVarGuard::set(CODEX_BIN_ENV, script.as_os_str());
        let _message = EnvVarGuard::set(
            "SOTP_FAKE_CODEX_MESSAGE",
            std::ffi::OsStr::new("{\n  \"verdict\": \"zero_findings\",\n  \"findings\": []\n}"),
        );

        let result = run_codex_local(&fake_args(
            Some("Review this implementation.".to_owned()),
            None,
            output,
            1,
        ))
        .unwrap();

        assert_eq!(result.verdict, ReviewVerdict::ZeroFindings);
        assert_eq!(
            result.final_message.as_deref(),
            Some("{\"verdict\":\"zero_findings\",\"findings\":[]}")
        );
    }

    #[test]
    fn run_codex_local_clears_stale_explicit_output_file_before_invocation() {
        let _lock = env_lock().lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let script = write_fake_codex_script(dir.path());
        let output = dir.path().join("last.txt");
        fs::write(&output, "{\"verdict\":\"zero_findings\",\"findings\":[]}").unwrap();
        let _bin = EnvVarGuard::set(CODEX_BIN_ENV, script.as_os_str());

        let result = run_codex_local(&fake_args(
            Some("Review this implementation.".to_owned()),
            None,
            output.clone(),
            1,
        ))
        .unwrap();

        assert_eq!(result.verdict, ReviewVerdict::LastMessageMissing);
        assert_eq!(result.final_message, None);
        assert_eq!(fs::read_to_string(output).unwrap(), "");
    }

    #[test]
    fn run_codex_local_cleans_auto_managed_artifacts_when_spawn_fails() {
        let _lock = env_lock().lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let _cwd = CurrentDirGuard::change_to(dir.path());
        let _bin =
            EnvVarGuard::set(CODEX_BIN_ENV, std::ffi::OsStr::new("definitely-missing-codex"));

        let args = CodexLocalArgs {
            model: "gpt-5.4".to_owned(),
            timeout_seconds: 1,
            briefing_file: None,
            prompt: Some("Review this implementation.".to_owned()),
            output_last_message: None,
        };

        let err = run_codex_local(&args).unwrap_err();
        assert!(err.contains("failed to spawn"));

        // Auto-managed artifacts (output-last-message, output-schema) should be cleaned up.
        // Session log persists intentionally for post-run debugging.
        let runtime_dir = dir.path().join("tmp/reviewer-runtime");
        if runtime_dir.is_dir() {
            let remaining: Vec<_> = fs::read_dir(&runtime_dir)
                .unwrap()
                .filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().to_string())
                .collect();
            // Only session log files should remain
            assert!(
                remaining.iter().all(|name| name.starts_with("codex-session-")),
                "unexpected non-session-log artifacts remain: {remaining:?}"
            );
        }
    }

    #[test]
    fn run_codex_local_reports_last_message_missing_on_success() {
        let _lock = env_lock().lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let script = write_fake_codex_script(dir.path());
        let output = dir.path().join("last.txt");
        let _bin = EnvVarGuard::set(CODEX_BIN_ENV, script.as_os_str());

        let result = run_codex_local(&fake_args(
            Some("Review this implementation.".to_owned()),
            None,
            output,
            1,
        ))
        .unwrap();

        assert_eq!(result.verdict, ReviewVerdict::LastMessageMissing);
        assert_eq!(result.final_message, None);
    }

    #[test]
    fn run_codex_local_reports_process_failed_for_invalid_json_payload() {
        let _lock = env_lock().lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let script = write_fake_codex_script(dir.path());
        let output = dir.path().join("last.txt");
        let _bin = EnvVarGuard::set(CODEX_BIN_ENV, script.as_os_str());
        let _message =
            EnvVarGuard::set("SOTP_FAKE_CODEX_MESSAGE", std::ffi::OsStr::new("NO_FINDINGS"));

        let result = run_codex_local(&fake_args(
            Some("Review this implementation.".to_owned()),
            None,
            output,
            1,
        ))
        .unwrap();

        assert_eq!(result.verdict, ReviewVerdict::ProcessFailed);
        assert!(
            result
                .verdict_detail
                .as_deref()
                .is_some_and(|detail| detail.contains("invalid reviewer final payload"))
        );
    }

    #[test]
    fn run_codex_local_reports_timeout() {
        let _lock = env_lock().lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let script = write_fake_codex_script(dir.path());
        let output = dir.path().join("last.txt");
        let _bin = EnvVarGuard::set(CODEX_BIN_ENV, script.as_os_str());
        let _sleep = EnvVarGuard::set("SOTP_FAKE_CODEX_SLEEP_SECONDS", std::ffi::OsStr::new("1"));

        let result = run_codex_local(&fake_args(
            Some("Review this implementation.".to_owned()),
            None,
            output,
            0,
        ))
        .unwrap();

        assert_eq!(result.verdict, ReviewVerdict::Timeout);
        assert_eq!(result.final_message, None);
    }

    #[cfg(unix)]
    #[test]
    fn run_codex_local_kills_reviewer_process_group_on_timeout() {
        let _lock = env_lock().lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("fake-codex-tree.sh");
        let child_pid_file = dir.path().join("child.pid");
        fs::write(
            &script,
            r#"#!/bin/sh
set -eu
pid_file="${SOTP_FAKE_CODEX_CHILD_PID_FILE:-}"
if [ -n "$pid_file" ]; then
  sleep 30 &
  echo "$!" > "$pid_file"
fi
sleep "${SOTP_FAKE_CODEX_SLEEP_SECONDS:-30}"
"#,
        )
        .unwrap();
        let mut perms = fs::metadata(&script).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script, perms).unwrap();

        let output = dir.path().join("last.txt");
        let _bin = EnvVarGuard::set(CODEX_BIN_ENV, script.as_os_str());
        let _pid_file =
            EnvVarGuard::set("SOTP_FAKE_CODEX_CHILD_PID_FILE", child_pid_file.as_os_str());
        let _sleep = EnvVarGuard::set("SOTP_FAKE_CODEX_SLEEP_SECONDS", std::ffi::OsStr::new("30"));

        let result = run_codex_local(&fake_args(
            Some("Review this implementation.".to_owned()),
            None,
            output,
            1,
        ))
        .unwrap();

        assert_eq!(result.verdict, ReviewVerdict::Timeout);

        for _ in 0..20 {
            if child_pid_file.is_file() {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let child_pid = fs::read_to_string(&child_pid_file).unwrap();
        let child_pid = child_pid.trim().parse::<i32>().unwrap();
        let mut child_gone = false;
        for _ in 0..40 {
            if process_is_gone_or_zombie(child_pid) {
                child_gone = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(
            child_gone,
            "expected timed-out reviewer descendant {child_pid} to be gone or zombie"
        );
    }

    fn write_agent_profiles(root: &Path, model_profiles_json: &str) {
        let claude_dir = root.join(".claude");
        fs::create_dir_all(&claude_dir).unwrap();
        fs::write(
            claude_dir.join("agent-profiles.json"),
            format!(
                r#"{{
  "version": 1,
  "providers": {{
    "codex": {{
      "default_model": "gpt-5.4",
      "model_profiles": {model_profiles_json}
    }}
  }},
  "profiles": {{
    "default": {{ "reviewer": "codex" }}
  }}
}}"#
            ),
        )
        .unwrap();
    }

    #[test]
    fn run_codex_local_passes_full_auto_for_full_model() {
        let _lock = env_lock().lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let _cwd = CurrentDirGuard::change_to(dir.path());
        let script = write_fake_codex_script(dir.path());
        let output = dir.path().join("last.txt");
        let args_file = dir.path().join("codex-args.txt");
        write_agent_profiles(
            dir.path(),
            r#"{"gpt-5.4": {"full_auto": true}, "gpt-5.3-codex-spark": {"full_auto": false}}"#,
        );
        let _bin = EnvVarGuard::set(CODEX_BIN_ENV, script.as_os_str());
        let _args_env = EnvVarGuard::set("SOTP_FAKE_CODEX_ARGS_FILE", args_file.as_os_str());
        let _message = EnvVarGuard::set(
            "SOTP_FAKE_CODEX_MESSAGE",
            std::ffi::OsStr::new("{\"verdict\":\"zero_findings\",\"findings\":[]}"),
        );

        let result = run_codex_local(&fake_args(
            Some("Review this implementation.".to_owned()),
            None,
            output,
            1,
        ))
        .unwrap();

        assert_eq!(result.verdict, ReviewVerdict::ZeroFindings);
        let args_content = fs::read_to_string(&args_file).unwrap();
        assert!(
            args_content.contains("--full-auto"),
            "expected --full-auto in args for full model, got: {args_content}"
        );
    }

    #[test]
    fn run_codex_local_omits_full_auto_for_spark_model() {
        let _lock = env_lock().lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let _cwd = CurrentDirGuard::change_to(dir.path());
        let script = write_fake_codex_script(dir.path());
        let output = dir.path().join("last.txt");
        let args_file = dir.path().join("codex-args.txt");
        write_agent_profiles(
            dir.path(),
            r#"{"gpt-5.4": {"full_auto": true}, "gpt-5.3-codex-spark": {"full_auto": false}}"#,
        );
        let _bin = EnvVarGuard::set(CODEX_BIN_ENV, script.as_os_str());
        let _args_env = EnvVarGuard::set("SOTP_FAKE_CODEX_ARGS_FILE", args_file.as_os_str());
        let _message = EnvVarGuard::set(
            "SOTP_FAKE_CODEX_MESSAGE",
            std::ffi::OsStr::new("{\"verdict\":\"zero_findings\",\"findings\":[]}"),
        );

        let mut args = fake_args(Some("Review this implementation.".to_owned()), None, output, 1);
        args.model = "gpt-5.3-codex-spark".to_owned();

        let result = run_codex_local(&args).unwrap();

        assert_eq!(result.verdict, ReviewVerdict::ZeroFindings);
        let args_content = fs::read_to_string(&args_file).unwrap();
        assert!(
            !args_content.contains("--full-auto"),
            "expected no --full-auto in args for spark model, got: {args_content}"
        );
    }

    #[test]
    fn run_codex_local_defaults_to_full_auto_when_profiles_missing() {
        let _lock = env_lock().lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let _cwd = CurrentDirGuard::change_to(dir.path());
        // No agent-profiles.json written — file read should fail, fall back to full_auto=true
        let script = write_fake_codex_script(dir.path());
        let output = dir.path().join("last.txt");
        let args_file = dir.path().join("codex-args.txt");
        let _bin = EnvVarGuard::set(CODEX_BIN_ENV, script.as_os_str());
        let _args_env = EnvVarGuard::set("SOTP_FAKE_CODEX_ARGS_FILE", args_file.as_os_str());
        let _message = EnvVarGuard::set(
            "SOTP_FAKE_CODEX_MESSAGE",
            std::ffi::OsStr::new("{\"verdict\":\"zero_findings\",\"findings\":[]}"),
        );

        let result = run_codex_local(&fake_args(
            Some("Review this implementation.".to_owned()),
            None,
            output,
            1,
        ))
        .unwrap();

        assert_eq!(result.verdict, ReviewVerdict::ZeroFindings);
        let args_content = fs::read_to_string(&args_file).unwrap();
        assert!(
            args_content.contains("--full-auto"),
            "expected --full-auto (fail-closed) when profiles missing, got: {args_content}"
        );
    }
}
