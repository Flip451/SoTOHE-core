//! Subprocess-spawning concrete adapter for [`AgentExecutionRunner`].
//!
//! This module owns *all* OS-level subprocess invocation for `bin/sotp ref-verify`.
//!
//! ## Hexagonal layering
//!
//! `apps/cli-composition` only depends on the [`AgentExecutionRunner`] type
//! alias (the secondary-port shape) and the [`make_ref_verifier_process_runner`]
//! factory. The cli-composition layer never imports `std::process::Command`,
//! `std::env`, or any binary-name strings — those lived in
//! `apps/cli-composition/src/ref_verify/runner.rs` historically and were a
//! hexagonal architecture violation (orchestration spawning subprocesses).
//! That violation is removed by relocating the entire subprocess pipeline
//! into this file (D7 / CN-01).
//!
//! ## D11: separate Chain1 / Chain2 capabilities
//!
//! The provider dispatch routes through `resolved.provider` (already
//! capability-selected upstream in `AgentRefVerifierAdapter::verify_pair`).
//! Chain1 uses `ref-verifier-chain1` and Chain2 uses `ref-verifier-chain2`
//! capabilities; both can route through any of the three providers below.
//!
//! ## No timeout
//!
//! Per the user-stated constraint ("そもそもタイムアウト指定が不要です"),
//! the subprocess waits with `child.wait()` (no deadline). LLM verifier calls
//! are inherently long-running; an arbitrary timeout would cause spurious
//! escalation to human review. If the subprocess hangs, the user kills `sotp`
//! externally — there is no protection on this gate against runaway models.
//!
//! ## No env-var binary resolution
//!
//! Per the user-stated constraint ("env var自体アンチパターンです"), the
//! binary names are hardcoded literals (`"claude"`, `"codex"`, `"gemini"`)
//! and resolution is delegated to the OS `PATH`. No `CLAUDE_BIN` / `CODEX_BIN`
//! / `GEMINI_BIN` environment variable is consulted.
//!
//! ## stderr tail surfacing
//!
//! On non-zero subprocess exit, the **tail** (not the head) of stderr is
//! attached to [`RefVerifyError::VerifierPort`] up to 4 KB / 20 lines.
//! Long verbose headers (telemetry banners) must not bury the actual error.

use std::ffi::{OsStr, OsString};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;

use usecase::capability_exec::ReasoningEffort;
use usecase::ref_verify::RefVerifyError;

use crate::agent_profiles::ResolvedExecution;
use crate::codex_common::{REVIEW_RUNTIME_DIR, runtime_path};
use crate::ref_verify::AgentExecutionRunner;

mod transient_files;

use transient_files::AutoCleanupFile;

const MAX_REF_VERIFY_SESSION_LOG_BYTES: usize = 4 * 1024 * 1024;
const STDERR_SESSION_LOG_HEADING: &[u8] = b"=== STDERR ===\n";
const STDERR_SESSION_LOG_TRUNCATED_NOTICE: &[u8] = b"\n[stderr truncated]\n";

/// Codex structured-output JSON schema for the ref-verify [`VerdictResponseDto`].
///
/// Modelled as a flat object with a `kind` discriminator + nullable
/// `citation` / `reason` strings. OpenAI structured output rejects `oneOf`
/// and requires all properties to appear in `required`.
///
/// Exposed at `pub(crate)` so the shared semantic-verifier pipeline
/// (`test_obligation::semantic_verifier`) can reuse this exact schema for the
/// waiver verifier — whose verdict wire shape matches ref-verify's (D8).
pub(crate) const CODEX_OUTPUT_SCHEMA: &str = r#"{
  "type": "object",
  "additionalProperties": false,
  "required": ["kind", "citation", "reason"],
  "properties": {
    "kind": { "type": "string", "enum": ["pass", "fail", "pending"] },
    "citation": { "type": ["string", "null"] },
    "reason": { "type": ["string", "null"] }
  }
}
"#;

/// Codex structured-output JSON schema for the obligation-fulfillment verdict.
///
/// Extends [`CODEX_OUTPUT_SCHEMA`] with the D6 fail-taxonomy `category`
/// (`contradiction` / `substitution` / `central_unverified`), so a `fail`
/// verdict can propagate its real category to the calibration loop instead of
/// being flattened to `central_unverified` by the ref-verify schema's default.
///
/// The category is nullable — a `pass` or `pending` verdict emits
/// `"category": null`. Structured output requires every property in `required`
/// and forbids `oneOf`; the enum lists the three snake_case fail categories
/// alongside `null`, and the property `type` combines `string` with `null`.
pub(crate) const CODEX_FULFILLMENT_OUTPUT_SCHEMA: &str = r#"{
  "type": "object",
  "additionalProperties": false,
  "required": ["kind", "citation", "reason", "category"],
  "properties": {
    "kind": { "type": "string", "enum": ["pass", "fail", "pending"] },
    "citation": { "type": ["string", "null"] },
    "reason": { "type": ["string", "null"] },
    "category": {
      "type": ["string", "null"],
      "enum": ["contradiction", "substitution", "central_unverified", null]
    }
  }
}
"#;

/// Build the canonical [`AgentExecutionRunner`] that spawns the configured
/// provider binary (claude / codex / gemini) for ref-verifier execution.
///
/// `project_root` is the canonical project root path; it is captured into
/// the closure and used as the working directory for subprocess spawns and
/// as the parent for transient codex output files under
/// `tmp/reviewer-runtime/`.
#[must_use]
pub fn make_ref_verifier_process_runner(project_root: PathBuf) -> Arc<AgentExecutionRunner> {
    make_agent_process_runner(project_root, CODEX_OUTPUT_SCHEMA)
}

/// Build an [`AgentExecutionRunner`] that shares the same subprocess pipeline
/// as [`make_ref_verifier_process_runner`] but constrains the codex path with
/// a caller-supplied structured-output schema.
///
/// The claude / gemini paths do not enforce structural constraints on model
/// output (they pass the rendered prompt through unchanged), so the schema
/// only affects the codex `exec --output-schema` argument. This lets the
/// obligation-fulfillment verifier reach codex with a schema that admits its
/// D6 fail `category` while ref-verify and the waiver verifier keep the
/// narrower ref-verify schema.
#[must_use]
pub(crate) fn make_agent_process_runner(
    project_root: PathBuf,
    codex_output_schema: &'static str,
) -> Arc<AgentExecutionRunner> {
    Arc::new(move |resolved: ResolvedExecution, prompt: String| {
        run_ref_verifier_agent(&project_root, resolved, prompt, codex_output_schema)
    })
}

/// Provider-dispatching entry point — pure delegation to a per-provider runner.
///
/// `codex_output_schema` is the structured-output JSON schema forwarded to
/// codex's `--output-schema` (see [`make_agent_process_runner`]). The claude
/// and gemini paths ignore it: neither CLI enforces structural constraints on
/// model output at this pipeline layer.
fn run_ref_verifier_agent(
    project_root: &Path,
    resolved: ResolvedExecution,
    prompt: String,
    codex_output_schema: &str,
) -> Result<String, RefVerifyError> {
    let ResolvedExecution::ProviderCli { provider, model, effort } = resolved else {
        return Err(ref_verify_runner_error(
            "hosted-service execution cannot run a reference verifier".to_owned(),
        ));
    };
    match provider.as_str() {
        "claude" => run_claude_ref_verifier(project_root, model.as_str(), effort, &prompt),
        "codex" => run_codex_ref_verifier(
            project_root,
            model.as_str(),
            effort,
            &prompt,
            codex_output_schema,
        ),
        "gemini" => run_gemini_ref_verifier(project_root, model.as_str(), effort, &prompt),
        other => {
            Err(ref_verify_runner_error(format!("unsupported ref-verifier provider '{other}'")))
        }
    }
}

// ── claude ───────────────────────────────────────────────────────────────────

/// claude no-tools boundary: the semantic verifier MUST judge only the prompt
/// payload. `--tools ""` removes built-in tools and `--disallowedTools …`
/// explicitly forbids the Read/Grep/Glob/Bash/Edit/Write toolkit even if host
/// settings expose them as defaults.
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

fn run_claude_ref_verifier(
    project_root: &Path,
    model: &str,
    effort: ReasoningEffort,
    prompt: &str,
) -> Result<String, RefVerifyError> {
    let bin: OsString = "claude".into();
    let args = build_claude_ref_verifier_args(model, effort, prompt);
    let outcome = run_process_retryable(&bin, &args, project_root, "claude ref-verifier", None)?;
    extract_claude_ref_verifier_output(&outcome.stdout)
        .or_else(|| nonempty_trimmed(&outcome.stdout))
        .ok_or_else(|| ref_verify_runner_error("claude ref-verifier produced no output"))
}

/// Build the argv for the `claude` ref-verifier subprocess.
#[must_use]
pub fn build_claude_ref_verifier_args(
    model: &str,
    effort: ReasoningEffort,
    prompt: &str,
) -> Vec<OsString> {
    let mut args = CLAUDE_REF_VERIFIER_STATIC_ARGS.iter().map(OsString::from).collect::<Vec<_>>();
    args.extend(CLAUDE_REF_VERIFIER_DISALLOWED_TOOLS.iter().map(OsString::from));
    args.push(OsString::from("--output-format"));
    args.push(OsString::from("json"));
    args.push(OsString::from("--model"));
    args.push(OsString::from(model));
    args.push(OsString::from("--effort"));
    args.push(OsString::from(reasoning_effort_value(effort)));
    args.push(OsString::from(prompt));
    args
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

// ── codex ────────────────────────────────────────────────────────────────────

fn run_codex_ref_verifier(
    project_root: &Path,
    model: &str,
    effort: ReasoningEffort,
    prompt: &str,
    output_schema: &str,
) -> Result<String, RefVerifyError> {
    let schema = AutoCleanupFile::create(
        project_root,
        "codex-ref-verify-schema",
        "json",
        output_schema.as_bytes(),
    )?;
    let last_message = AutoCleanupFile::create(project_root, "codex-ref-verify-last", "txt", &[])?;

    let runtime = crate::codex_common::resolve_codex_runtime(project_root)
        .map_err(|error| ref_verify_runner_error(error.to_string()))?;
    let args =
        build_codex_ref_verifier_args(model, effort, prompt, schema.path(), last_message.path());
    run_process_retryable(
        runtime.executable(),
        &args,
        project_root,
        "codex ref-verifier",
        Some(&runtime),
    )?;

    // Codex writes the final structured message to `--output-last-message`.
    // Fail closed if the authoritative output file is absent or empty — do NOT fall back to
    // stdout, which is a best-effort stream that can contain partial writes, formatting noise,
    // or an incomplete verdict. A missing file indicates a wrapper regression or early exit
    // and must surface as an error rather than producing a potentially corrupt verdict.
    let last_text = std::fs::read_to_string(last_message.path()).map_err(|e| {
        ref_verify_runner_error(format!(
            "codex ref-verifier output-last-message read error at '{}': {e}",
            last_message.path().display()
        ))
    })?;
    nonempty_trimmed(&last_text)
        .ok_or_else(|| ref_verify_runner_error("codex ref-verifier output-last-message is empty"))
}

/// Build the argv for the `codex exec` ref-verifier subprocess.
///
/// Constraints:
/// - `--sandbox read-only` plus `--config sandbox_workspace_write_roots=[]`:
///   the verifier must not perform filesystem mutation.
/// - `--config model_reasoning_effort="<resolved-effort>"`: makes the profile
///   reasoning depth explicit rather than falling back to Codex's default.
/// - `--output-schema <path>` + `--output-last-message <path>`: structured
///   output is enforced and routed to a transient file instead of stdout
///   (so codex's tail-output formatting cannot corrupt the JSON).
#[must_use]
pub fn build_codex_ref_verifier_args(
    model: &str,
    effort: ReasoningEffort,
    prompt: &str,
    output_schema: &Path,
    output_last_message: &Path,
) -> Vec<OsString> {
    vec![
        OsString::from("exec"),
        OsString::from("--sandbox"),
        OsString::from("read-only"),
        OsString::from("--config"),
        OsString::from("sandbox_workspace_write_roots=[]"),
        OsString::from("--config"),
        OsString::from(format!("model_reasoning_effort=\"{}\"", reasoning_effort_value(effort))),
        OsString::from("--model"),
        OsString::from(model),
        OsString::from("--output-schema"),
        output_schema.as_os_str().to_owned(),
        OsString::from("--output-last-message"),
        output_last_message.as_os_str().to_owned(),
        OsString::from(prompt),
    ]
}

// ── gemini ───────────────────────────────────────────────────────────────────

fn run_gemini_ref_verifier(
    project_root: &Path,
    model: &str,
    effort: ReasoningEffort,
    prompt: &str,
) -> Result<String, RefVerifyError> {
    let bin: OsString = "gemini".into();
    let args = build_gemini_ref_verifier_args(model, effort, prompt)?;
    let outcome = run_process_retryable(&bin, &args, project_root, "gemini ref-verifier", None)?;
    nonempty_trimmed(&outcome.stdout)
        .ok_or_else(|| ref_verify_runner_error("gemini ref-verifier produced no output"))
}

/// Build the argv for the `gemini` ref-verifier subprocess.
///
/// Gemini CLI has no invocation-scoped reasoning-effort flag. Refusing this
/// execution is safer than launching with an implicit provider default.
///
/// # Errors
///
/// Always returns an error until Gemini CLI exposes an invocation-scoped
/// effort setting that can be passed without mutating persistent user config.
pub fn build_gemini_ref_verifier_args(
    model: &str,
    effort: ReasoningEffort,
    prompt: &str,
) -> Result<Vec<OsString>, RefVerifyError> {
    let _ = (model, prompt);
    Err(ref_verify_runner_error(format!(
        "Gemini ref-verifier cannot inject resolved reasoning effort '{}' without a provider default",
        reasoning_effort_value(effort)
    )))
}

fn reasoning_effort_value(effort: ReasoningEffort) -> &'static str {
    match effort {
        ReasoningEffort::Low => "low",
        ReasoningEffort::Medium => "medium",
        ReasoningEffort::High => "high",
        ReasoningEffort::XHigh => "xhigh",
        ReasoningEffort::Max => "max",
    }
}

// ── subprocess core ──────────────────────────────────────────────────────────

#[derive(Debug)]
struct ProcessOutcome {
    stdout: String,
}

struct RefVerifySessionLog {
    path: PathBuf,
    written_bytes: usize,
}

struct BoundedStderr {
    bytes: Vec<u8>,
    truncated: bool,
}

/// Spawn `bin args …` with `current_dir = project_root`, wait without timeout,
/// and return stdout. On non-zero exit, attach the **tail** of stderr (up to
/// 4 KB / 20 lines) to the error so a long verbose header cannot bury the
/// actual failure detail.
fn run_process(
    bin: &OsStr,
    args: &[OsString],
    current_dir: &Path,
    label: &str,
    runtime: Option<&crate::codex_common::ResolvedCodexRuntime>,
) -> Result<ProcessOutcome, RefVerifyError> {
    let mut session_log = runtime
        .map(|runtime| {
            let path = runtime_path(REVIEW_RUNTIME_DIR, "ref-verify-codex-session", "log")
                .map_err(ref_verify_runner_error)?;
            std::fs::File::create(&path).map_err(|error| {
                ref_verify_runner_error(format!(
                    "failed to create ref-verify session log {}: {error}",
                    path.display()
                ))
            })?;
            let mut session_log = RefVerifySessionLog { path, written_bytes: 0 };
            append_session_log(
                &mut session_log,
                crate::codex_common::runtime_log_header(runtime).as_bytes(),
            )?;
            Ok(session_log)
        })
        .transpose()?;
    let mut command = Command::new(bin);
    command
        .args(args)
        .current_dir(current_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(runtime) = runtime {
        crate::codex_common::configure_codex_command(&mut command, runtime)
            .map_err(ref_verify_runner_error)?;
    }
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
    let stdout_handle = spawn_read_to_string(stdout);
    let stderr_handle = spawn_bounded_stderr_reader(stderr, MAX_REF_VERIFY_SESSION_LOG_BYTES);

    let status = child.wait().map_err(|e| {
        ref_verify_runner_error(format!("failed to wait on {label} subprocess: {e}"))
    })?;

    let stdout_text = stdout_handle
        .join()
        .map_err(|_| ref_verify_runner_error(format!("{label} stdout reader thread panicked")))?
        .map_err(|e| ref_verify_runner_error(format!("failed to read {label} stdout: {e}")))?;
    let stderr = stderr_handle
        .join()
        .map_err(|_| ref_verify_runner_error(format!("{label} stderr reader thread panicked")))?
        .map_err(|e| ref_verify_runner_error(format!("failed to read {label} stderr: {e}")))?;
    if let Some(session_log) = session_log.as_mut() {
        append_session_log(session_log, STDERR_SESSION_LOG_HEADING)?;
        append_session_log(session_log, &stderr.bytes)?;
        if stderr.truncated {
            append_session_log(session_log, STDERR_SESSION_LOG_TRUNCATED_NOTICE)?;
        }
    }
    let stderr_text = String::from_utf8_lossy(&stderr.bytes).into_owned();

    if !status.success() {
        let tail = stderr_tail(&stderr_text, 20, 4096);
        return Err(ref_verify_runner_error(format!(
            "{label} exited with non-zero status: {status}; stderr tail: {tail}"
        )));
    }

    Ok(ProcessOutcome { stdout: stdout_text })
}

/// Retry-aware wrapper around [`run_process`]: the same subprocess call, plus
/// bounded backoff on transient provider failures.
///
/// The retry is intentionally at the subprocess seam (not per verifier lane)
/// so the ref-verify chain1/2, obligation-fulfillment, and waiver runners all
/// share the same resilience without each re-implementing it. Codex-specific
/// transient files (schema + last-message) are created once by the caller and
/// stay alive across retries because they live outside this function's scope.
fn run_process_retryable(
    bin: &OsStr,
    args: &[OsString],
    current_dir: &Path,
    label: &str,
    runtime: Option<&crate::codex_common::ResolvedCodexRuntime>,
) -> Result<ProcessOutcome, RefVerifyError> {
    super::retry::retry_transient(
        super::retry::DEFAULT_TRANSIENT_BACKOFFS,
        || run_process(bin, args, current_dir, label, runtime),
        std::thread::sleep,
    )
}

fn spawn_read_to_string<R>(mut pipe: R) -> std::thread::JoinHandle<std::io::Result<String>>
where
    R: Read + Send + 'static,
{
    std::thread::spawn(move || {
        let mut buf = String::new();
        pipe.read_to_string(&mut buf).map(|_| buf)
    })
}

fn spawn_bounded_stderr_reader<R>(
    pipe: R,
    maximum_bytes: usize,
) -> std::thread::JoinHandle<std::io::Result<BoundedStderr>>
where
    R: Read + Send + 'static,
{
    std::thread::spawn(move || collect_bounded_stderr(pipe, maximum_bytes))
}

fn collect_bounded_stderr<R>(mut pipe: R, maximum_bytes: usize) -> std::io::Result<BoundedStderr>
where
    R: Read,
{
    let mut buffer = [0_u8; 8192];
    let mut retained = Vec::new();
    let mut truncated = false;
    loop {
        let read = pipe.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        let bytes = buffer.get(..read).ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, "invalid stderr read count")
        })?;
        if bytes.len() >= maximum_bytes {
            retained.clear();
            retained.extend_from_slice(
                bytes.get(bytes.len().saturating_sub(maximum_bytes)..).unwrap_or_default(),
            );
            truncated = true;
            continue;
        }
        let overflow = retained.len().saturating_add(bytes.len()).saturating_sub(maximum_bytes);
        if overflow > 0 {
            retained.drain(..overflow);
            truncated = true;
        }
        retained.extend_from_slice(bytes);
    }
    Ok(BoundedStderr { bytes: retained, truncated })
}

fn append_session_log(
    session_log: &mut RefVerifySessionLog,
    bytes: &[u8],
) -> Result<(), RefVerifyError> {
    let remaining = MAX_REF_VERIFY_SESSION_LOG_BYTES.saturating_sub(session_log.written_bytes);
    let retained = bytes.get(..bytes.len().min(remaining)).unwrap_or_default();
    if retained.is_empty() {
        return Ok(());
    }
    let mut file =
        std::fs::OpenOptions::new().append(true).open(&session_log.path).map_err(|error| {
            ref_verify_runner_error(format!(
                "failed to open ref-verify session log {}: {error}",
                session_log.path.display()
            ))
        })?;
    file.write_all(retained).map_err(|error| {
        ref_verify_runner_error(format!(
            "failed to write ref-verify session log {}: {error}",
            session_log.path.display()
        ))
    })?;
    session_log.written_bytes = session_log.written_bytes.saturating_add(retained.len());
    Ok(())
}

fn stderr_tail(text: &str, max_lines: usize, max_bytes: usize) -> String {
    if text.is_empty() {
        return String::new();
    }
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.len().saturating_sub(max_lines);
    let mut tail = lines.get(start..).map(|s| s.join("\n")).unwrap_or_default();
    if tail.len() > max_bytes {
        // Trim from the front so the most recent bytes are preserved.
        let cut = tail.len() - max_bytes;
        // Find a char boundary at or after `cut` to avoid splitting a UTF-8 sequence.
        let mut idx = cut;
        while idx < tail.len() && !tail.is_char_boundary(idx) {
            idx += 1;
        }
        tail = tail.split_off(idx);
    }
    tail
}

#[cfg(unix)]
fn configure_verifier_process_group(command: &mut Command) {
    use std::os::unix::process::CommandExt as _;
    command.process_group(0);
}

#[cfg(not(unix))]
fn configure_verifier_process_group(_command: &mut Command) {}

#[cfg(unix)]
#[allow(dead_code)]
fn terminate_verifier_child(child: &mut Child, label: &str) -> Result<(), RefVerifyError> {
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
#[allow(dead_code)]
fn terminate_verifier_child(child: &mut Child, label: &str) -> Result<(), RefVerifyError> {
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
#[allow(dead_code)]
fn terminate_verifier_child(child: &mut Child, label: &str) -> Result<(), RefVerifyError> {
    child.kill().map_err(|e| ref_verify_runner_error(format!("failed to terminate {label}: {e}")))
}

fn nonempty_trimmed(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() { None } else { Some(trimmed.to_owned()) }
}

fn ref_verify_runner_error(message: impl Into<String>) -> RefVerifyError {
    RefVerifyError::VerifierPort { message: message.into() }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use usecase::capability_exec::{ModelName, ProviderName, ReasoningEffort};

    #[test]
    fn build_claude_ref_verifier_args_denies_local_tools() {
        let args =
            build_claude_ref_verifier_args("claude-opus-4-8", ReasoningEffort::Max, "the prompt");
        let strs: Vec<&str> = args.iter().filter_map(|s| s.to_str()).collect();
        assert!(strs.contains(&"--disallowedTools"));
        for tool in ["Read", "Grep", "Glob", "Bash", "Edit", "Write"] {
            assert!(strs.contains(&tool), "expected disallowed tool '{tool}'");
        }
        assert!(strs.contains(&"the prompt"));
        assert!(strs.contains(&"claude-opus-4-8"));
        assert!(strs.contains(&"--effort"));
        assert!(strs.contains(&"max"));
    }

    #[test]
    fn build_codex_ref_verifier_args_includes_schema_and_reasoning_effort() {
        let args = build_codex_ref_verifier_args(
            "gpt-5",
            ReasoningEffort::Low,
            "the prompt",
            Path::new("/tmp/schema.json"),
            Path::new("/tmp/last.txt"),
        );
        let strs: Vec<&str> = args.iter().filter_map(|s| s.to_str()).collect();
        assert!(strs.contains(&"exec"));
        assert!(strs.contains(&"--sandbox"));
        assert!(strs.contains(&"read-only"));
        assert!(strs.contains(&"sandbox_workspace_write_roots=[]"));
        assert!(strs.contains(&"model_reasoning_effort=\"low\""));
        assert!(strs.contains(&"--output-schema"));
        assert!(strs.contains(&"--output-last-message"));
        assert!(strs.contains(&"the prompt"));
    }

    #[test]
    fn build_gemini_ref_verifier_args_rejects_provider_default_effort() {
        let error =
            build_gemini_ref_verifier_args("gemini-2.5-pro", ReasoningEffort::Low, "the prompt")
                .expect_err("Gemini execution must fail closed when effort cannot be injected");
        let RefVerifyError::VerifierPort { message } = error else {
            panic!("expected VerifierPort, got {error:?}");
        };
        assert!(message.contains("reasoning effort 'low'"), "got: {message}");
        assert!(message.contains("provider default"), "got: {message}");
    }

    #[test]
    fn stderr_tail_preserves_last_lines() {
        let text = "line1\nline2\nline3\nline4\nline5";
        let tail = stderr_tail(text, 2, 4096);
        assert_eq!(tail, "line4\nline5");
    }

    #[test]
    fn stderr_tail_truncates_overlong_bytes_preserving_tail() {
        let text: String = (0..1000).map(|i| format!("line{i:04}\n")).collect();
        let tail = stderr_tail(&text, 50, 64);
        assert!(tail.len() <= 64);
        assert!(tail.contains("line0999"), "tail must include last line, got '{tail}'");
    }

    #[test]
    fn test_collect_bounded_stderr_retains_tail_and_marks_truncation() {
        let output = collect_bounded_stderr(std::io::Cursor::new(b"0123456789"), 4)
            .expect("stderr collector succeeds");

        assert_eq!(output.bytes, b"6789");
        assert!(output.truncated);
    }

    #[test]
    fn test_append_session_log_caps_persistent_log() {
        let directory = tempfile::tempdir().expect("temporary directory is created");
        let path = directory.path().join("session.log");
        std::fs::File::create(&path).expect("session log is created");
        let mut session_log = RefVerifySessionLog { path: path.clone(), written_bytes: 0 };

        append_session_log(&mut session_log, &vec![b'x'; MAX_REF_VERIFY_SESSION_LOG_BYTES + 1])
            .expect("bounded log append succeeds");

        assert_eq!(
            std::fs::metadata(path).expect("session log metadata is read").len(),
            MAX_REF_VERIFY_SESSION_LOG_BYTES as u64
        );
    }

    #[test]
    fn auto_cleanup_file_removes_path_on_drop() {
        let dir = tempfile::tempdir().unwrap();
        let path = {
            let f = AutoCleanupFile::create(dir.path(), "test-cleanup", "tmp", b"hello").unwrap();
            f.path().to_path_buf()
        };
        assert!(!path.exists(), "auto-cleanup file must be removed on drop");
    }

    #[test]
    fn run_ref_verifier_agent_rejects_unsupported_provider() {
        let dir = tempfile::tempdir().unwrap();
        let resolved = ResolvedExecution::ProviderCli {
            provider: ProviderName::try_new("unsupported").unwrap(),
            model: ModelName::try_new("m").unwrap(),
            effort: ReasoningEffort::High,
        };
        let err =
            run_ref_verifier_agent(dir.path(), resolved, "prompt".to_owned(), CODEX_OUTPUT_SCHEMA)
                .unwrap_err();
        let RefVerifyError::VerifierPort { message } = err else {
            panic!("expected VerifierPort, got {err:?}");
        };
        assert!(message.contains("unsupported ref-verifier provider"));
    }

    #[test]
    fn run_ref_verifier_agent_rejects_hosted_service_execution() {
        let dir = tempfile::tempdir().unwrap();
        let resolved =
            ResolvedExecution::HostedService { provider: ProviderName::try_new("claude").unwrap() };
        let err =
            run_ref_verifier_agent(dir.path(), resolved, "prompt".to_owned(), CODEX_OUTPUT_SCHEMA)
                .unwrap_err();
        let RefVerifyError::VerifierPort { message } = err else {
            panic!("expected VerifierPort, got {err:?}");
        };
        assert!(message.contains("hosted-service execution"));
    }

    // ── structured-output schema shape ────────────────────────────────────────

    /// The ref-verify codex schema stays byte-identical to the historical
    /// three-field contract: adding the fulfillment `category` here would
    /// leak the D6 fail taxonomy into ref-verify's verdict wire shape.
    #[test]
    fn codex_output_schema_matches_ref_verify_three_field_contract() {
        let schema: serde_json::Value = serde_json::from_str(CODEX_OUTPUT_SCHEMA).unwrap();
        assert_eq!(schema.get("additionalProperties"), Some(&serde_json::Value::Bool(false)));
        let required: Vec<&str> = schema
            .get("required")
            .and_then(serde_json::Value::as_array)
            .unwrap()
            .iter()
            .filter_map(serde_json::Value::as_str)
            .collect();
        assert_eq!(required, vec!["kind", "citation", "reason"]);
        assert!(
            schema.get("properties").and_then(|p| p.get("category")).is_none(),
            "ref-verify schema must not declare a category property"
        );
    }

    /// The fulfillment codex schema must include the nullable `category`
    /// alongside the three ref-verify fields, so codex's structured output can
    /// carry a real fail taxonomy back through calibration. Every property
    /// must appear in `required` and the enum must include `null` — those are
    /// OpenAI structured-output constraints.
    #[test]
    fn codex_fulfillment_output_schema_declares_category_enum_with_null() {
        let schema: serde_json::Value =
            serde_json::from_str(CODEX_FULFILLMENT_OUTPUT_SCHEMA).unwrap();
        assert_eq!(schema.get("additionalProperties"), Some(&serde_json::Value::Bool(false)));
        let required: std::collections::BTreeSet<&str> = schema
            .get("required")
            .and_then(serde_json::Value::as_array)
            .unwrap()
            .iter()
            .filter_map(serde_json::Value::as_str)
            .collect();
        for name in ["kind", "citation", "reason", "category"] {
            assert!(required.contains(name), "required missing '{name}'");
        }

        let category =
            schema.get("properties").and_then(|p| p.get("category")).expect("category property");
        let ty = category.get("type").and_then(serde_json::Value::as_array).expect("type array");
        let ty_variants: std::collections::BTreeSet<&str> =
            ty.iter().filter_map(serde_json::Value::as_str).collect();
        assert!(ty_variants.contains("string") && ty_variants.contains("null"));

        let enum_vals =
            category.get("enum").and_then(serde_json::Value::as_array).expect("category enum");
        let string_variants: std::collections::BTreeSet<&str> =
            enum_vals.iter().filter_map(serde_json::Value::as_str).collect();
        for value in ["contradiction", "substitution", "central_unverified"] {
            assert!(string_variants.contains(value), "category enum missing '{value}'");
        }
        assert!(
            enum_vals.iter().any(serde_json::Value::is_null),
            "category enum must include null for pass/pending verdicts"
        );
    }

    // ── success-path subprocess tests ─────────────────────────────────────────

    /// Verify that `run_process` captures stdout from a successfully-spawned subprocess.
    ///
    /// Uses `/usr/bin/echo` (universally available on Linux/macOS) as the subprocess;
    /// this exercises the subprocess-spawning path, stdout-capture thread, and
    /// `ProcessOutcome` assembly without requiring a real LLM binary.
    #[test]
    fn run_process_captures_stdout_on_success() {
        let dir = tempfile::tempdir().unwrap();
        let outcome = run_process(
            OsStr::new("echo"),
            &[OsString::from("hello subprocess")],
            dir.path(),
            "echo",
            None,
        )
        .unwrap();
        assert!(outcome.stdout.contains("hello subprocess"));
    }

    /// Verify that `run_process` attaches stderr tail on non-zero exit.
    ///
    /// Uses `sh -c "echo failure >&2; exit 1"` to exercise the stderr-capture
    /// thread and the stderr-tail formatting path.
    #[test]
    fn run_process_surfaces_stderr_tail_on_failure() {
        let dir = tempfile::tempdir().unwrap();
        let err = run_process(
            OsStr::new("sh"),
            &[OsString::from("-c"), OsString::from("echo failure-detail >&2; exit 1")],
            dir.path(),
            "sh-test",
            None,
        )
        .unwrap_err();
        let RefVerifyError::VerifierPort { message } = err else {
            panic!("expected VerifierPort, got {err:?}");
        };
        assert!(message.contains("non-zero status"), "expected status mention, got: {message}");
        assert!(message.contains("failure-detail"), "expected stderr tail, got: {message}");
    }

    /// Verify that `make_ref_verifier_process_runner` produces a callable runner
    /// and that the runner propagates a spawn failure to `RefVerifyError::VerifierPort`.
    ///
    /// Uses a deliberately non-existent binary name so the spawn itself fails,
    /// exercising the factory + closure wiring path without requiring LLM binaries.
    #[test]
    fn make_ref_verifier_process_runner_returns_callable_runner() {
        let dir = tempfile::tempdir().unwrap();
        let runner = make_ref_verifier_process_runner(dir.path().to_path_buf());
        let resolved = ResolvedExecution::ProviderCli {
            provider: ProviderName::try_new("claude").unwrap(),
            model: ModelName::try_new("claude-opus-4-8").unwrap(),
            effort: ReasoningEffort::Max,
        };
        // The "claude" binary is not available in unit-test CI, so the spawn will fail with
        // "No such file or directory" (or equivalent). That error path proves the runner
        // closure is wired correctly and surfaces errors via RefVerifyError::VerifierPort.
        let err = runner(resolved, "test prompt".to_owned()).unwrap_err();
        let RefVerifyError::VerifierPort { message } = err else {
            panic!("expected VerifierPort, got {err:?}");
        };
        // Spawn failure → the error must mention the binary name or "spawn"
        assert!(
            message.contains("claude") || message.contains("spawn"),
            "expected spawn-failure message, got: {message}"
        );
    }
}
