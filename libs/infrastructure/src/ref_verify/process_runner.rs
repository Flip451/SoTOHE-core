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
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use usecase::ref_verify::RefVerifyError;

use crate::agent_profiles::ResolvedExecution;
use crate::ref_verify::AgentExecutionRunner;

/// Codex structured-output JSON schema for [`VerdictResponseDto`].
///
/// Modelled as a flat object with a `kind` discriminator + nullable
/// `citation` / `reason` strings. OpenAI structured output rejects `oneOf`
/// and requires all properties to appear in `required`.
const CODEX_OUTPUT_SCHEMA: &str = r#"{
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

/// Build the canonical [`AgentExecutionRunner`] that spawns the configured
/// provider binary (claude / codex / gemini) for ref-verifier execution.
///
/// `project_root` is the canonical project root path; it is captured into
/// the closure and used as the working directory for subprocess spawns and
/// as the parent for transient codex output files under
/// `tmp/reviewer-runtime/`.
#[must_use]
pub fn make_ref_verifier_process_runner(project_root: PathBuf) -> Arc<AgentExecutionRunner> {
    Arc::new(move |resolved: ResolvedExecution, prompt: String| {
        run_ref_verifier_agent(&project_root, resolved, prompt)
    })
}

/// Provider-dispatching entry point — pure delegation to a per-provider runner.
fn run_ref_verifier_agent(
    project_root: &Path,
    resolved: ResolvedExecution,
    prompt: String,
) -> Result<String, RefVerifyError> {
    match resolved.provider.as_str() {
        "claude" => {
            let model = require_ref_verifier_model("claude", resolved.model.as_deref())?;
            run_claude_ref_verifier(project_root, model, &prompt)
        }
        "codex" => {
            let model = require_ref_verifier_model("codex", resolved.model.as_deref())?;
            run_codex_ref_verifier(project_root, model, &prompt)
        }
        "gemini" => {
            let model = require_ref_verifier_model("gemini", resolved.model.as_deref())?;
            run_gemini_ref_verifier(project_root, model, &prompt)
        }
        other => {
            Err(ref_verify_runner_error(format!("unsupported ref-verifier provider '{other}'")))
        }
    }
}

fn require_ref_verifier_model<'a>(
    provider: &str,
    model: Option<&'a str>,
) -> Result<&'a str, RefVerifyError> {
    model.ok_or_else(|| {
        ref_verify_runner_error(format!(
            "ref-verifier provider '{provider}' requires a configured model"
        ))
    })
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
    prompt: &str,
) -> Result<String, RefVerifyError> {
    let bin: OsString = "claude".into();
    let args = build_claude_ref_verifier_args(model, prompt);
    let outcome = run_process(&bin, &args, project_root, "claude ref-verifier")?;
    extract_claude_ref_verifier_output(&outcome.stdout)
        .or_else(|| nonempty_trimmed(&outcome.stdout))
        .ok_or_else(|| ref_verify_runner_error("claude ref-verifier produced no output"))
}

/// Build the argv for the `claude` ref-verifier subprocess.
#[must_use]
pub fn build_claude_ref_verifier_args(model: &str, prompt: &str) -> Vec<OsString> {
    let mut args = CLAUDE_REF_VERIFIER_STATIC_ARGS.iter().map(OsString::from).collect::<Vec<_>>();
    args.extend(CLAUDE_REF_VERIFIER_DISALLOWED_TOOLS.iter().map(OsString::from));
    args.push(OsString::from("--output-format"));
    args.push(OsString::from("json"));
    args.push(OsString::from("--model"));
    args.push(OsString::from(model));
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
    prompt: &str,
) -> Result<String, RefVerifyError> {
    let schema = AutoCleanupFile::create(
        project_root,
        "codex-ref-verify-schema",
        "json",
        CODEX_OUTPUT_SCHEMA.as_bytes(),
    )?;
    let last_message = AutoCleanupFile::create(project_root, "codex-ref-verify-last", "txt", &[])?;

    let bin: OsString = "codex".into();
    let args = build_codex_ref_verifier_args(model, prompt, schema.path(), last_message.path());
    run_process(&bin, &args, project_root, "codex ref-verifier")?;

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
/// - `--config model_reasoning_effort="high"`: match the dry-checker
///   pattern so the verifier reasons hard before emitting Pass.
/// - `--output-schema <path>` + `--output-last-message <path>`: structured
///   output is enforced and routed to a transient file instead of stdout
///   (so codex's tail-output formatting cannot corrupt the JSON).
#[must_use]
pub fn build_codex_ref_verifier_args(
    model: &str,
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
        OsString::from("model_reasoning_effort=\"high\""),
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
    prompt: &str,
) -> Result<String, RefVerifyError> {
    let bin: OsString = "gemini".into();
    let args = build_gemini_ref_verifier_args(model, prompt);
    let outcome = run_process(&bin, &args, project_root, "gemini ref-verifier")?;
    nonempty_trimmed(&outcome.stdout)
        .ok_or_else(|| ref_verify_runner_error("gemini ref-verifier produced no output"))
}

/// Build the argv for the `gemini` ref-verifier subprocess.
#[must_use]
pub fn build_gemini_ref_verifier_args(model: &str, prompt: &str) -> Vec<OsString> {
    vec![OsString::from("-m"), OsString::from(model), OsString::from("-p"), OsString::from(prompt)]
}

// ── subprocess core ──────────────────────────────────────────────────────────

#[derive(Debug)]
struct ProcessOutcome {
    stdout: String,
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
) -> Result<ProcessOutcome, RefVerifyError> {
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
    let stdout_handle = spawn_read_to_string(stdout);
    let stderr_handle = spawn_read_to_string(stderr);

    let status = child.wait().map_err(|e| {
        ref_verify_runner_error(format!("failed to wait on {label} subprocess: {e}"))
    })?;

    let stdout_text = stdout_handle
        .join()
        .map_err(|_| ref_verify_runner_error(format!("{label} stdout reader thread panicked")))?
        .map_err(|e| ref_verify_runner_error(format!("failed to read {label} stdout: {e}")))?;
    let stderr_text = stderr_handle
        .join()
        .map_err(|_| ref_verify_runner_error(format!("{label} stderr reader thread panicked")))?
        .map_err(|e| ref_verify_runner_error(format!("failed to read {label} stderr: {e}")))?;

    if !status.success() {
        let tail = stderr_tail(&stderr_text, 20, 4096);
        return Err(ref_verify_runner_error(format!(
            "{label} exited with non-zero status: {status}; stderr tail: {tail}"
        )));
    }

    Ok(ProcessOutcome { stdout: stdout_text })
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

// ── transient files (codex --output-schema / --output-last-message) ─────────

/// RAII handle for a transient file under `tmp/reviewer-runtime/`.
///
/// On drop, the file is removed best-effort. Errors during removal are
/// swallowed because the dropper has no way to surface them and a stale
/// transient file is harmless.
struct AutoCleanupFile {
    path: PathBuf,
}

impl AutoCleanupFile {
    fn create(
        project_root: &Path,
        prefix: &str,
        ext: &str,
        content: &[u8],
    ) -> Result<Self, RefVerifyError> {
        let canon_root = project_root.canonicalize().map_err(|e| {
            ref_verify_runner_error(format!(
                "cannot canonicalize project root '{}': {e}",
                project_root.display()
            ))
        })?;
        let path = ref_verify_runtime_path(project_root, prefix, ext)?;
        // Use `create_new` so a raced symlink planted after the directory guard cannot redirect
        // the file write to an existing path outside the tree.
        let mut f =
            std::fs::OpenOptions::new().write(true).create_new(true).open(&path).map_err(|e| {
                ref_verify_runner_error(format!(
                    "failed to create transient file '{}': {e}",
                    path.display()
                ))
            })?;
        // Post-creation guard: verify the opened file resolves within the project root.
        // `canonicalize` on the newly-created path cannot follow a symlink placed after
        // `create_new` succeeded (the file now exists at the inode we created), but it does
        // resolve any symlink in the parent-directory ancestry.
        let canon_path = path.canonicalize().map_err(|e| {
            ref_verify_runner_error(format!(
                "cannot canonicalize transient file '{}': {e}",
                path.display()
            ))
        })?;
        if !canon_path.starts_with(&canon_root) {
            let _ = std::fs::remove_file(&path);
            return Err(ref_verify_runner_error(format!(
                "transient file '{}' resolves to '{}' which escapes project root '{}'",
                path.display(),
                canon_path.display(),
                canon_root.display()
            )));
        }
        if !content.is_empty() {
            f.write_all(content).map_err(|e| {
                ref_verify_runner_error(format!(
                    "failed to write transient file '{}': {e}",
                    path.display()
                ))
            })?;
        }
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for AutoCleanupFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn ref_verify_runtime_path(
    project_root: &Path,
    prefix: &str,
    ext: &str,
) -> Result<PathBuf, RefVerifyError> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let canon_root = project_root.canonicalize().map_err(|e| {
        ref_verify_runner_error(format!(
            "cannot canonicalize project root '{}': {e}",
            project_root.display()
        ))
    })?;

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
    // Guard: verify the created directory resolves within the canonical project root.
    // This catches pre-existing symlinks on `tmp` or `reviewer-runtime` that would redirect
    // writes outside the trusted tree.
    let canon_parent = parent.canonicalize().map_err(|e| {
        ref_verify_runner_error(format!(
            "cannot canonicalize runtime dir '{}': {e}",
            parent.display()
        ))
    })?;
    if !canon_parent.starts_with(&canon_root) {
        return Err(ref_verify_runner_error(format!(
            "runtime dir '{}' resolves to '{}' which escapes project root '{}'",
            parent.display(),
            canon_parent.display(),
            canon_root.display()
        )));
    }
    Ok(path)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn build_claude_ref_verifier_args_denies_local_tools() {
        let args = build_claude_ref_verifier_args("claude-opus-4-8", "the prompt");
        let strs: Vec<&str> = args.iter().filter_map(|s| s.to_str()).collect();
        assert!(strs.contains(&"--disallowedTools"));
        for tool in ["Read", "Grep", "Glob", "Bash", "Edit", "Write"] {
            assert!(strs.contains(&tool), "expected disallowed tool '{tool}'");
        }
        assert!(strs.contains(&"the prompt"));
        assert!(strs.contains(&"claude-opus-4-8"));
    }

    #[test]
    fn build_codex_ref_verifier_args_includes_schema_and_reasoning_effort() {
        let args = build_codex_ref_verifier_args(
            "gpt-5",
            "the prompt",
            Path::new("/tmp/schema.json"),
            Path::new("/tmp/last.txt"),
        );
        let strs: Vec<&str> = args.iter().filter_map(|s| s.to_str()).collect();
        assert!(strs.contains(&"exec"));
        assert!(strs.contains(&"--sandbox"));
        assert!(strs.contains(&"read-only"));
        assert!(strs.contains(&"sandbox_workspace_write_roots=[]"));
        assert!(strs.contains(&"model_reasoning_effort=\"high\""));
        assert!(strs.contains(&"--output-schema"));
        assert!(strs.contains(&"--output-last-message"));
        assert!(strs.contains(&"the prompt"));
    }

    #[test]
    fn build_gemini_ref_verifier_args_uses_short_flags() {
        let args = build_gemini_ref_verifier_args("gemini-2.5-pro", "the prompt");
        let strs: Vec<&str> = args.iter().filter_map(|s| s.to_str()).collect();
        assert_eq!(strs, vec!["-m", "gemini-2.5-pro", "-p", "the prompt"]);
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
        let resolved =
            ResolvedExecution { provider: "unsupported".to_owned(), model: Some("m".to_owned()) };
        let err = run_ref_verifier_agent(dir.path(), resolved, "prompt".to_owned()).unwrap_err();
        let RefVerifyError::VerifierPort { message } = err else {
            panic!("expected VerifierPort, got {err:?}");
        };
        assert!(message.contains("unsupported ref-verifier provider"));
    }

    #[test]
    fn run_ref_verifier_agent_rejects_provider_without_model() {
        let dir = tempfile::tempdir().unwrap();
        let resolved = ResolvedExecution { provider: "claude".to_owned(), model: None };
        let err = run_ref_verifier_agent(dir.path(), resolved, "prompt".to_owned()).unwrap_err();
        let RefVerifyError::VerifierPort { message } = err else {
            panic!("expected VerifierPort, got {err:?}");
        };
        assert!(message.contains("requires a configured model"));
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
        let resolved = ResolvedExecution {
            provider: "claude".to_owned(),
            model: Some("claude-opus-4-8".to_owned()),
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
