//! Shared helpers for building Codex CLI argument vectors and managing
//! Codex subprocess I/O.
//!
//! Both the `DryCheckAgentPort` adapter (`codex_dry_checker`) and the
//! `Reviewer` adapter (`codex_reviewer`) build the same `codex exec`
//! argument pattern: model, read-only sandbox, reasoning-effort config,
//! output schema/last-message, and prompt.  This module centralises that
//! construction so future changes to Codex CLI flags only need to happen
//! in one place.
//!
//! It also hosts the shared subprocess-management helpers extracted under
//! ADR D3: `drain_pipe`, `tee_stderr_to_file`, `spawn_codex`, and
//! `runtime_path`.

use std::ffi::OsString;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

/// Build the argument vector for a `codex exec --sandbox read-only` invocation.
///
/// Produces: `exec --model <model> --sandbox read-only --config
/// model_reasoning_effort="<reasoning_effort>" --output-schema <schema>
/// --output-last-message <last_msg> <prompt>`.
///
/// # Arguments
/// - `model`: Codex model name (e.g. `"gpt-5.5"`).
/// - `reasoning_effort`: `model_reasoning_effort` value (e.g. `"high"`).
/// - `prompt`: Full prompt string passed as the final positional argument.
/// - `output_last_message`: Path where Codex writes the last message JSON.
/// - `output_schema`: Path to the JSON schema file for structured output.
pub fn build_codex_read_only_invocation(
    model: &str,
    reasoning_effort: &str,
    prompt: &str,
    output_last_message: &Path,
    output_schema: &Path,
) -> Vec<OsString> {
    let mut args = vec![OsString::from("exec"), OsString::from("--model"), OsString::from(model)];
    // MUST use read-only sandbox. Do NOT use --full-auto here because it
    // implies --sandbox workspace-write and Codex CLI applies it after our
    // explicit --sandbox read-only, overriding the safety constraint.
    args.extend([OsString::from("--sandbox"), OsString::from("read-only")]);
    args.extend([
        OsString::from("--config"),
        OsString::from(format!("model_reasoning_effort=\"{reasoning_effort}\"")),
    ]);
    args.extend([
        OsString::from("--output-schema"),
        output_schema.as_os_str().to_os_string(),
        OsString::from("--output-last-message"),
        output_last_message.as_os_str().to_os_string(),
        OsString::from(prompt),
    ]);
    args
}

// ---------------------------------------------------------------------------
// Subprocess-management helpers (ADR D3: extracted from codex_reviewer and
// codex_dry_checker — these were byte-identical in both adapters).
// ---------------------------------------------------------------------------

/// Environment variable for overriding the `codex` binary path in tests.
///
/// Set to a non-empty value to substitute a fake `codex` executable.
/// Only active in test builds (`#[cfg(test)]`).
#[cfg(test)]
pub(crate) const CODEX_BIN_ENV: &str = "SOTP_CODEX_BIN";

/// Returns the path to the `codex` binary.
///
/// In test builds, the `SOTP_CODEX_BIN` environment variable may override
/// the default `"codex"` to allow injecting a fake executable.
pub(crate) fn codex_bin() -> OsString {
    #[cfg(test)]
    if let Some(value) = std::env::var_os(CODEX_BIN_ENV).filter(|v| !v.is_empty()) {
        return value;
    }
    OsString::from("codex")
}

/// Builds a timestamped, process-unique path inside `base_dir`.
///
/// Creates `base_dir` (and any missing ancestors) before returning.
///
/// # Arguments
/// - `base_dir`: Runtime directory constant (e.g. `REVIEW_RUNTIME_DIR` or
///   `DRY_CHECK_RUNTIME_DIR`). Callers pass their own constant so T012 can
///   consolidate the values without touching this function.
/// - `prefix`: File-name prefix (e.g. `"codex-last-message"`).
/// - `ext`: File extension without leading dot (e.g. `"txt"`).
pub(crate) fn runtime_path(base_dir: &str, prefix: &str, ext: &str) -> Result<PathBuf, String> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| format!("failed to compute timestamp: {e}"))?
        .as_nanos();
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = PathBuf::from(base_dir)
        .join(format!("{prefix}-{}-{timestamp}-{seq}.{ext}", std::process::id()));
    let parent = path
        .parent()
        .ok_or_else(|| format!("runtime path must have a parent directory: {}", path.display()))?;
    std::fs::create_dir_all(parent)
        .map_err(|e| format!("failed to create {}: {e}", parent.display()))?;
    Ok(path)
}

/// Spawns the `codex` binary with the given arguments and wires up I/O threads.
///
/// Returns `(child, io_handles)` where `io_handles` contains threads that drain
/// stdout and tee stderr to `session_log_path`. The caller is responsible for
/// waiting on the child and (when appropriate) joining the handles.
///
/// Stdout is captured and drained (not forwarded) to prevent the child from
/// blocking on a full pipe buffer and to uphold the fail-closed contract: the
/// sole authoritative verdict source is the `--output-last-message` file.
pub(crate) fn spawn_codex(
    bin: &std::ffi::OsStr,
    args: &[OsString],
    session_log_path: &Path,
) -> Result<(Child, Vec<thread::JoinHandle<()>>), String> {
    let mut command = Command::new(bin);
    // Capture stdout instead of inheriting: the wrapper is the sole code path
    // that emits authoritative verdict JSON. Inherited stdout would let the
    // reviewer child leak verdict-like content before persistence succeeds,
    // breaking the fail-closed contract for unrecorded rounds.
    command.args(args).stdin(Stdio::null()).stdout(Stdio::piped());

    let log_file = std::fs::File::create(session_log_path)
        .map_err(|e| format!("failed to create session log {}: {e}", session_log_path.display()))?;

    command.stderr(Stdio::piped());

    let mut child =
        command.spawn().map_err(|e| format!("failed to spawn {}: {e}", bin.to_string_lossy()))?;

    let mut io_handles = Vec::new();

    if let Some(pipe) = child.stderr.take() {
        io_handles.push(thread::spawn(move || {
            tee_stderr_to_file(pipe, log_file);
        }));
    }

    // Drain stdout to prevent the child from blocking on a full pipe buffer.
    // Content is intentionally not forwarded to the parent process.
    if let Some(pipe) = child.stdout.take() {
        io_handles.push(thread::spawn(move || {
            drain_pipe(pipe);
        }));
    }

    Ok((child, io_handles))
}

/// Drains a pipe to prevent the child process from blocking on a full buffer.
/// Content is intentionally discarded.
pub(crate) fn drain_pipe(pipe: std::process::ChildStdout) {
    let reader = BufReader::new(pipe);
    for line in reader.lines() {
        if line.is_err() {
            break;
        }
    }
}

/// Tees the child's stderr to a log file while also printing to the current
/// process's stderr.
pub fn tee_stderr_to_file(pipe: std::process::ChildStderr, mut log_file: std::fs::File) {
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
