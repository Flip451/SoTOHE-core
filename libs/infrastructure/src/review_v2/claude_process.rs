//! Process, argv, and envelope helpers for the Claude reviewer adapter.
//!
//! Invokes `claude -p --permission-mode dontAsk --allowedTools Read Grep Glob
//! "Bash(git diff:*)" "Bash(git show:*)" "Bash(git log:*)" "Bash(git ls-files:*)"
//! --disallowedTools Edit Write --output-format json --json-schema '<schema>' --model <model>
//! <prompt>` as a subprocess and parses the `structured_output` field from the JSON envelope
//! written to stdout (CN-01 / CN-05 / CN-06).
//!
//! `--bare` is omitted: Claude Code `--bare` refuses OAuth/keychain and accepts only
//! `ANTHROPIC_API_KEY` or `apiKeyHelper`, which makes `sotp review local` unusable
//! for the normal logged-in host session.
//!
//! Best-effort, permission-based read-only invocation for the reviewer role (CN-05):
//! 1. `--permission-mode dontAsk`: auto-denies tool calls not on the allow list — in standard
//!    environments (no permissive host `permissions.allow` overrides) this prevents unlisted
//!    tools from being invoked.
//! 2. `--allowedTools <read-only-set>`: pre-approves only file inspection and read-only git tools.
//! 3. `--disallowedTools Edit Write`: removes write tools from the model's context entirely
//!    (defense in depth — they cannot be invoked even if the allow set were bypassed).
//!
//! Note: unlike `codex exec --sandbox read-only`, `claude -p` has no kernel-level sandbox flag.
//! Read-only behavior rests on the reviewer role + headless output-only (`-p`) form; a permissive
//! host `.claude/settings.json` could in principle broaden the tool surface.
//!
//! stderr is captured in memory — no session log or output files are written to the workspace
//! (CN-05).

use std::collections::VecDeque;
use std::ffi::OsString;
use std::io::{Read, Write};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use usecase::review_v2::ReviewerError;
use usecase::review_workflow::{
    REVIEW_OUTPUT_SCHEMA_JSON, ReviewFinalMessageState, ReviewVerdict, classify_review_verdict,
    normalize_final_message, parse_review_final_message, render_review_payload,
};

use crate::codex_common::POLL_INTERVAL;

/// Return type of `spawn_claude`: child process, stderr collector handle, and stdout collector handle.
///
/// Both handles collect the respective streams into `String` in memory (no files written — CN-05).
type SpawnClaudeResult =
    Result<(Child, thread::JoinHandle<String>, thread::JoinHandle<Option<String>>), String>;

/// Environment variable for overriding the `claude` binary path in tests.
#[cfg(any(test, feature = "test-helpers"))]
pub(crate) const CLAUDE_BIN_ENV: &str = "SOTP_CLAUDE_BIN";

/// Raw outcome from the Claude subprocess — parsed but not yet converted to domain types.
pub(super) struct ReviewOutcomeRaw {
    pub(super) verdict: ReviewVerdict,
    pub(super) final_message: Option<String>,
    /// Captured stderr output (in-memory; no files written — CN-05).
    pub(super) session_stderr: String,
    pub(super) session_id: Option<String>,
}

pub(super) fn claude_bin() -> OsString {
    #[cfg(any(test, feature = "test-helpers"))]
    if let Some(value) = std::env::var_os(CLAUDE_BIN_ENV).filter(|v| !v.is_empty()) {
        return value;
    }
    OsString::from("claude")
}

/// Read-only tools pre-approved for the Claude reviewer subprocess (CN-05).
///
/// Each entry is passed as a **separate argument** after `--allowedTools` (NOT space-joined into
/// one string). This matches the `claude` CLI's expected argument format where each tool name is
/// its own positional value.
///
/// Best-effort read-only scope (CN-05 — permission-based, NOT a kernel sandbox):
/// - `Read`, `Grep`, `Glob`: file content inspection tools without write capability.
/// - `Bash(git diff:*)`, `Bash(git show:*)`, `Bash(git log:*)`, `Bash(git ls-files:*)`:
///   git queries for diff and history inspection. Note: these Bash-wrapped git commands could
///   in principle be invoked with write-capable options (e.g., `git diff --output=<path>`) or
///   shell redirection, so they do not constitute a hard no-write guarantee. This is accepted
///   under CN-05's best-effort, permission-based framing.
///
/// `Edit`, `Write`, and all other `Bash(...)` forms are denied by `--permission-mode dontAsk`
/// and `--disallowedTools Edit Write` (context removal) (CN-05).
const REVIEWER_ALLOWED_TOOLS: &[&str] = &[
    "Read",
    "Grep",
    "Glob",
    "Bash(git diff:*)",
    "Bash(git show:*)",
    "Bash(git log:*)",
    "Bash(git ls-files:*)",
];

/// Write tools explicitly removed from the Claude reviewer's context (CN-05, defense in depth).
///
/// Passed as separate arguments after `--disallowedTools`. Even if `--permission-mode dontAsk`
/// were bypassed, these tools are unavailable to the model.
const REVIEWER_DISALLOWED_TOOLS: &[&str] = &["Edit", "Write"];

/// Strips `$schema` from the shared review schema for Claude Code `--json-schema`.
///
/// The shared `REVIEW_OUTPUT_SCHEMA_JSON` carries a `$schema` dialect URI. Claude Code's
/// `--json-schema` validator treats that key as a remote `$ref` and rejects it with
/// `no schema with key or ref "https://json-schema.org/draft/2020-12/schema"`. The adapter
/// strips `$schema` only for the Claude argv; the shared codec is unchanged.
///
/// # Errors
///
/// Returns an error when the shared schema is not JSON or cannot be re-serialized.
fn claude_review_json_schema() -> Result<String, String> {
    let mut value: serde_json::Value = serde_json::from_str(REVIEW_OUTPUT_SCHEMA_JSON)
        .map_err(|error| format!("review output schema is not JSON: {error}"))?;
    if let Some(object) = value.as_object_mut() {
        object.remove("$schema");
    }
    serde_json::to_string(&value)
        .map_err(|error| format!("failed to serialize claude json schema: {error}"))
}

/// Builds the argument list for the `claude -p` invocation.
///
/// Best-effort, permission-based read-only invocation (CN-05). This is NOT a kernel-level sandbox
/// (unlike `codex exec --sandbox read-only`); `claude -p` has no sandbox flag.
/// 1. `--permission-mode dontAsk`: auto-denies tool calls not on the allow list — in standard
///    environments (no permissive host `permissions.allow` overrides) this prevents unlisted tools
///    from being invoked.
/// 2. `--allowedTools <tools...>`: each tool passed as a separate `OsString` argument (not
///    space-joined) to pre-approve only read-only inspection tools.
/// 3. `--disallowedTools Edit Write`: removes write tools from the model's context entirely
///    (defense in depth).
///
/// `--bare` is omitted so the subprocess can use the host OAuth session.
///
/// Read-only behavior rests on the reviewer role + headless output-only (`-p`) form; a permissive
/// host `.claude/settings.json` could in principle broaden the tool surface.
///
/// Uses `--output-format json` so the verdict appears in the `structured_output` field on stdout.
/// Uses `--json-schema` for API-level schema enforcement (grammar-compiled, CN-01).
///
/// # Errors
///
/// Returns an error when the Claude `--json-schema` payload cannot be built.
fn build_claude_args(
    model: &str,
    effort: &str,
    resume_id: Option<&str>,
    prompt: &str,
) -> Result<Vec<OsString>, String> {
    let json_schema = claude_review_json_schema()?;
    let mut args = vec![
        OsString::from("-p"),
        OsString::from("--permission-mode"),
        OsString::from("dontAsk"),
        OsString::from("--allowedTools"),
    ];
    for tool in REVIEWER_ALLOWED_TOOLS {
        args.push(OsString::from(*tool));
    }
    args.push(OsString::from("--disallowedTools"));
    for tool in REVIEWER_DISALLOWED_TOOLS {
        args.push(OsString::from(*tool));
    }
    args.extend([
        OsString::from("--output-format"),
        OsString::from("json"),
        OsString::from("--json-schema"),
        OsString::from(json_schema),
        OsString::from("--model"),
        OsString::from(model),
        OsString::from("--effort"),
        OsString::from(effort),
    ]);
    if let Some(session_id) = resume_id {
        args.extend([OsString::from("--resume"), OsString::from(session_id)]);
    }
    args.push(OsString::from(prompt));
    Ok(args)
}

/// Maximum stdout retained for Claude's one final JSON envelope.
const MAX_CLAUDE_STDOUT_BYTES: u64 = 4 * 1024 * 1024;

/// Maximum stderr bytes retained for Claude diagnostics.
///
/// The collector keeps the first and last halves of this limit, continuing to
/// drain the pipe after the limit is reached so a verbose child cannot block.
const MAX_CLAUDE_STDERR_BYTES: usize = 4 * 1024 * 1024;
const CLAUDE_STDERR_PREFIX_BYTES: usize = MAX_CLAUDE_STDERR_BYTES / 2;
const CLAUDE_STDERR_TRUNCATION_NOTICE: &str = "\n[Claude stderr truncated]\n";

/// Spawns the Claude subprocess, capturing bounded stdout and stderr in memory (no files written — CN-05).
///
/// Returns `(child, stderr_collector_handle, stdout_collector_handle)`.
/// The stdout handle returns `None` when the final envelope exceeds its explicit byte bound.
pub(super) fn spawn_claude(
    bin: &std::ffi::OsStr,
    model: &str,
    effort: &str,
    resume_id: Option<&str>,
    prompt: &str,
) -> SpawnClaudeResult {
    let args = build_claude_args(model, effort, resume_id, prompt)?;

    let mut command = Command::new(bin);
    command.args(&args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child =
        command.spawn().map_err(|e| format!("failed to spawn {}: {e}", bin.to_string_lossy()))?;

    // Collect bounded stderr in memory (echoed to the process stderr for observability).
    let stderr_collector = match child.stderr.take() {
        Some(pipe) => thread::spawn(move || collect_claude_stderr(pipe)),
        None => thread::spawn(String::new),
    };

    // Claude emits one final JSON envelope on stdout. Retain it only within the
    // explicit cap, but always drain the pipe through EOF so the child cannot block.
    let stdout_collector = match child.stdout.take() {
        Some(pipe) => thread::spawn(move || collect_claude_stdout(pipe)),
        None => thread::spawn(|| None),
    };

    Ok((child, stderr_collector, stdout_collector))
}

pub(super) fn run_claude_child(
    mut child: Child,
    stderr_collector: thread::JoinHandle<String>,
    stdout_collector: thread::JoinHandle<Option<String>>,
    timeout: Duration,
) -> Result<ReviewOutcomeRaw, ReviewerError> {
    let start = Instant::now();
    let mut timed_out = false;
    let mut exit_success = false;

    loop {
        match child
            .try_wait()
            .map_err(|e| ReviewerError::Unexpected(format!("failed to poll reviewer child: {e}")))?
        {
            Some(status) => {
                exit_success = status.success();
                break;
            }
            None => {
                if start.elapsed() >= timeout {
                    timed_out = true;
                    let _ = child.kill();
                    child.wait().map_err(|e| {
                        ReviewerError::Unexpected(format!("failed to reap reviewer child: {e}"))
                    })?;
                    break;
                }
                thread::sleep(POLL_INTERVAL);
            }
        }
    }

    // Collect stdout. On timeout we skip the join: a grandchild process may still
    // hold the stdout pipe open, causing join() to block indefinitely. The thread
    // is left detached and will complete once the pipe closes naturally.
    let stdout_raw = if timed_out { None } else { stdout_collector.join().unwrap_or_default() };

    // Collect stderr similarly — skip join on timeout to avoid blocking.
    let session_stderr =
        if timed_out { String::new() } else { stderr_collector.join().unwrap_or_default() };

    // Parse the --output-format json envelope from stdout and extract structured_output.
    let final_message = stdout_raw
        .as_deref()
        .filter(|stdout| !timed_out && !stdout.trim().is_empty())
        .and_then(extract_structured_output);

    let normalized = final_message.as_deref().and_then(normalize_final_message);
    let final_message_state = parse_review_final_message(normalized.as_deref());

    // Re-render to canonical form if successfully parsed.
    let rendered_message = match &final_message_state {
        ReviewFinalMessageState::Parsed(payload) => Some(
            render_review_payload(payload).map_err(|e| ReviewerError::Unexpected(e.to_string()))?,
        ),
        _ => normalized.or(final_message),
    };

    let verdict = classify_review_verdict(timed_out, exit_success, &final_message_state);

    let session_id = stdout_raw.as_deref().and_then(extract_claude_session_id);
    Ok(ReviewOutcomeRaw { verdict, final_message: rendered_message, session_stderr, session_id })
}

/// Reads at most one Claude final envelope into memory, then drains any remaining stdout.
fn collect_claude_stdout<R: Read>(mut pipe: R) -> Option<String> {
    let mut bytes = Vec::new();
    let read_result =
        pipe.by_ref().take(MAX_CLAUDE_STDOUT_BYTES.saturating_add(1)).read_to_end(&mut bytes);
    let drain_result = std::io::copy(&mut pipe, &mut std::io::sink());
    if read_result.is_err() || drain_result.is_err() || bytes.len() as u64 > MAX_CLAUDE_STDOUT_BYTES
    {
        return None;
    }
    String::from_utf8(bytes).ok()
}

/// Drains Claude stderr while retaining only a bounded diagnostic prefix and suffix.
fn collect_claude_stderr<R: Read>(pipe: R) -> String {
    collect_claude_stderr_with_limits(
        pipe,
        MAX_CLAUDE_STDERR_BYTES,
        CLAUDE_STDERR_PREFIX_BYTES,
        true,
    )
}

fn collect_claude_stderr_with_limits<R: Read>(
    mut pipe: R,
    max_bytes: usize,
    prefix_bytes: usize,
    echo_to_stderr: bool,
) -> String {
    let prefix_bytes = prefix_bytes.min(max_bytes);
    let suffix_bytes = max_bytes.saturating_sub(prefix_bytes);
    let mut prefix = Vec::with_capacity(prefix_bytes);
    let mut suffix = VecDeque::with_capacity(suffix_bytes);
    let mut total_bytes = 0usize;
    let mut chunk = [0_u8; 8 * 1024];

    loop {
        let read = match pipe.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(read) => read,
        };
        let Some(bytes) = chunk.get(..read) else {
            break;
        };
        if echo_to_stderr {
            let _ = std::io::stderr().write_all(bytes);
        }
        total_bytes = total_bytes.saturating_add(read);

        let prefix_remaining = prefix_bytes.saturating_sub(prefix.len());
        let prefix_part = prefix_remaining.min(bytes.len());
        if let Some(prefix_part) = bytes.get(..prefix_part) {
            prefix.extend_from_slice(prefix_part);
        }
        append_bounded_suffix(
            &mut suffix,
            bytes.get(prefix_part..).unwrap_or_default(),
            suffix_bytes,
        );
    }

    let mut retained = prefix;
    retained.extend(suffix);
    if total_bytes > max_bytes {
        let Some(prefix) = retained.get(..prefix_bytes) else {
            return String::from_utf8_lossy(&retained).into_owned();
        };
        let Some(suffix) = retained.get(prefix_bytes..) else {
            return String::from_utf8_lossy(&retained).into_owned();
        };
        let mut rendered = String::from_utf8_lossy(prefix).into_owned();
        rendered.push_str(CLAUDE_STDERR_TRUNCATION_NOTICE);
        rendered.push_str(&String::from_utf8_lossy(suffix));
        rendered
    } else {
        String::from_utf8_lossy(&retained).into_owned()
    }
}

fn append_bounded_suffix(suffix: &mut VecDeque<u8>, bytes: &[u8], max_bytes: usize) {
    if max_bytes == 0 {
        return;
    }
    if bytes.len() >= max_bytes {
        suffix.clear();
        suffix.extend(
            bytes.get(bytes.len().saturating_sub(max_bytes)..).unwrap_or_default().iter().copied(),
        );
        return;
    }
    let to_drop = suffix.len().saturating_add(bytes.len()).saturating_sub(max_bytes);
    suffix.drain(..to_drop);
    suffix.extend(bytes.iter().copied());
}

fn extract_claude_session_id(stdout: &str) -> Option<String> {
    stdout.lines().find_map(|line| {
        serde_json::from_str::<serde_json::Value>(line.trim())
            .ok()?
            .get("session_id")?
            .as_str()
            .filter(|value| !value.trim().is_empty())
            .map(str::to_owned)
    })
}

/// Extracts the `structured_output` field from the `--output-format json` envelope.
///
/// The Claude `--output-format json` stdout envelope has the form:
/// `{"type": "result", ..., "structured_output": {...}, ...}`
///
/// Returns `Some(json_string)` where `json_string` is the serialized `structured_output`
/// object, or `None` if parsing fails or the field is absent.
fn extract_structured_output(stdout: &str) -> Option<String> {
    // Claude may emit the JSON across multiple lines; find the JSON object.
    // Try each non-empty line as a potential JSON envelope.
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
            if let Some(structured) = value.get("structured_output") {
                return serde_json::to_string(structured).ok();
            }
        }
    }
    // Fallback: try to parse the entire stdout as a single JSON object.
    let trimmed = stdout.trim();
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
        if let Some(structured) = value.get("structured_output") {
            return serde_json::to_string(structured).ok();
        }
    }
    None
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    /// Verifies that `build_claude_args` encodes the CN-05 best-effort read-only contract:
    /// `--permission-mode dontAsk`, each read-only tool as a separate token after
    /// `--allowedTools`, and each disallowed tool as a separate token after `--disallowedTools`.
    /// `--bare` is intentionally absent so host OAuth remains usable.
    ///
    /// This test is the canonical guard for the security-critical subprocess argv; the
    /// fake-binary integration tests cannot catch regressions here because the fake binaries
    /// ignore their arguments.
    #[test]
    fn test_build_claude_args_encodes_read_only_contract() {
        let model = "claude-opus-4-7";
        let prompt = "Review this.";
        let args = build_claude_args(model, "high", None, prompt).expect("schema strip");

        // Collect as &str slices for readable assertions.
        let strs: Vec<&str> = args.iter().filter_map(|a| a.to_str()).collect();

        // Required positional prefix flags.
        assert!(strs.contains(&"-p"), "must pass -p");
        assert!(
            !strs.contains(&"--bare"),
            "--bare must not be passed; it rejects OAuth and blocks host-logged-in review"
        );

        // --permission-mode dontAsk (CN-05 layer 2: auto-deny unlisted tools in standard environments).
        let pm_idx = strs
            .iter()
            .position(|&s| s == "--permission-mode")
            .expect("--permission-mode must be present");
        assert_eq!(
            strs.get(pm_idx + 1).copied(),
            Some("dontAsk"),
            "--permission-mode must be followed immediately by dontAsk"
        );

        // --allowedTools followed by each read-only token as a separate argument (CN-05 layer 3).
        let at_idx = strs
            .iter()
            .position(|&s| s == "--allowedTools")
            .expect("--allowedTools must be present");
        for tool in REVIEWER_ALLOWED_TOOLS {
            assert!(
                strs[at_idx + 1..].contains(tool),
                "read-only tool `{tool}` must appear as a separate token after --allowedTools"
            );
        }

        // --disallowedTools followed by write tools (CN-05 layer 4: defense in depth).
        let dt_idx = strs
            .iter()
            .position(|&s| s == "--disallowedTools")
            .expect("--disallowedTools must be present");
        for tool in REVIEWER_DISALLOWED_TOOLS {
            assert!(
                strs[dt_idx + 1..].contains(tool),
                "disallowed write tool `{tool}` must appear as a separate token after --disallowedTools"
            );
        }

        // Model and prompt are present.
        let model_idx = strs.iter().position(|&s| s == "--model").expect("--model must be present");
        assert_eq!(
            strs.get(model_idx + 1).copied(),
            Some(model),
            "--model must be followed by the model name"
        );
        assert!(strs.contains(&prompt), "prompt must appear as the last argument");

        let schema_idx =
            strs.iter().position(|&s| s == "--json-schema").expect("--json-schema must be present");
        let schema = strs.get(schema_idx + 1).copied().expect("schema payload");
        assert!(
            !schema.contains("https://json-schema.org/draft/2020-12/schema"),
            "Claude --json-schema must not carry the $schema dialect URI"
        );

        // Write tools must NOT appear before --disallowedTools (they must only be values of it).
        for tool in REVIEWER_DISALLOWED_TOOLS {
            let first_occurrence = strs.iter().position(|&s| s == *tool);
            assert!(
                first_occurrence.is_none_or(|i| i > dt_idx),
                "write tool `{tool}` must not appear before --disallowedTools"
            );
        }
    }

    #[test]
    fn test_build_claude_resume_args_reinject_all_execution_flags() {
        let args = build_claude_args("claude-model", "max", Some("prior-session"), "Review.")
            .expect("schema strip");
        let args: Vec<&str> = args.iter().filter_map(|arg| arg.to_str()).collect();
        assert!(args.windows(2).any(|pair| pair == ["--resume", "prior-session"]));
        assert!(args.windows(2).any(|pair| pair == ["--model", "claude-model"]));
        assert!(args.windows(2).any(|pair| pair == ["--effort", "max"]));
        assert!(args.windows(2).any(|pair| pair == ["--permission-mode", "dontAsk"]));
    }

    #[test]
    fn test_extract_structured_output_single_line_envelope() {
        let stdout =
            r#"{"type":"result","structured_output":{"verdict":"zero_findings","findings":[]}}"#;
        let result = extract_structured_output(stdout).unwrap();
        let v: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(v.get("verdict").and_then(|v| v.as_str()), Some("zero_findings"));
    }

    #[test]
    fn test_extract_structured_output_missing_field_returns_none() {
        let stdout = r#"{"type":"result","content":"no structured output here"}"#;
        assert!(extract_structured_output(stdout).is_none());
    }

    #[test]
    fn test_extract_structured_output_invalid_json_returns_none() {
        assert!(extract_structured_output("not json at all").is_none());
        assert!(extract_structured_output("").is_none());
    }

    #[test]
    fn test_collect_claude_stdout_rejects_oversized_envelope() {
        let stdout = vec![b'x'; MAX_CLAUDE_STDOUT_BYTES.saturating_add(1) as usize];

        assert_eq!(collect_claude_stdout(std::io::Cursor::new(stdout)), None);
    }

    #[test]
    fn test_collect_claude_stderr_over_limit_keeps_bounded_prefix_and_suffix() {
        let stderr =
            collect_claude_stderr_with_limits(std::io::Cursor::new(b"abcdefghij"), 8, 4, false);

        assert_eq!(stderr, "abcd\n[Claude stderr truncated]\nghij");
    }
}
