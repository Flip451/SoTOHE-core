//! Claude-backed implementation of the `Reviewer` usecase port.
//!
//! Invokes `claude -p --bare --permission-mode dontAsk --allowedTools Read Grep Glob
//! "Bash(git diff:*)" "Bash(git show:*)" "Bash(git log:*)" "Bash(git ls-files:*)"
//! --disallowedTools Edit Write --output-format json --json-schema '<schema>' --model <model>
//! <prompt>` as a subprocess and parses the `structured_output` field from the JSON envelope
//! written to stdout (CN-01 / CN-05 / CN-06).
//!
//! Best-effort, permission-based read-only invocation for the reviewer role (CN-05):
//! 1. `--bare`: skips auto-discovery of host hooks, skills, MCP servers, and CLAUDE.md files,
//!    giving a reproducible invocation environment (CN-01).
//! 2. `--permission-mode dontAsk`: auto-denies tool calls not on the allow list — in standard
//!    environments (no permissive host `permissions.allow` overrides) this prevents unlisted
//!    tools from being invoked.
//! 3. `--allowedTools <read-only-set>`: pre-approves only file inspection and read-only git tools.
//! 4. `--disallowedTools Edit Write`: removes write tools from the model's context entirely
//!    (defense in depth — they cannot be invoked even if the allow set were bypassed).
//!
//! Note: unlike `codex exec --sandbox read-only`, `claude -p` has no kernel-level sandbox flag.
//! Read-only behavior rests on the reviewer role + headless output-only (`-p`) form; a permissive
//! host `.claude/settings.json` could in principle broaden the tool surface.
//!
//! stderr is captured in memory — no session log or output files are written to the workspace
//! (CN-05).

use std::ffi::OsString;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use domain::review_v2::{
    FastVerdict, LogInfo, ReviewTarget, ReviewerFinding, Verdict, VerdictError,
};
use usecase::review_v2::{ReviewerError, ports::Reviewer};
use usecase::review_workflow::{
    REVIEW_OUTPUT_SCHEMA_JSON, ReviewFinalMessageState, ReviewPayloadVerdict, ReviewVerdict,
    classify_review_verdict, normalize_final_message, parse_review_final_message,
    render_review_payload,
};

const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Return type of `spawn_claude`: child process, stderr collector handle, and stdout collector handle.
///
/// Both handles collect the respective streams into `String` in memory (no files written — CN-05).
type SpawnClaudeResult =
    Result<(Child, thread::JoinHandle<String>, thread::JoinHandle<String>), String>;

/// Environment variable for overriding the `claude` binary path in tests.
#[cfg(test)]
pub(crate) const CLAUDE_BIN_ENV: &str = "SOTP_CLAUDE_BIN";

/// Claude-backed reviewer implementation for the `Reviewer` usecase port.
///
/// Spawns a `claude -p --bare --permission-mode dontAsk --allowedTools <read-only-set>
/// --disallowedTools Edit Write --output-format json --json-schema` subprocess, feeds it a review
/// prompt (base prompt + scope file list), polls for completion, and parses the structured JSON
/// verdict from the `structured_output` field of the JSON envelope on stdout.
///
/// Best-effort, permission-based read-only invocation for the reviewer role (CN-05).
/// In standard environments (no permissive host `permissions.allow` overrides) write/edit tools
/// are denied:
/// - `--bare`: skips auto-discovery of host hooks, skills, MCP servers, and CLAUDE.md files for
///   a reproducible invocation environment (CN-01).
/// - `--permission-mode dontAsk`: auto-denies tool calls not on the allow list, preventing
///   unlisted tools from falling through to a permissive mode.
/// - `--allowedTools`: pre-approves only read-only inspection tools (`Read`, `Grep`, `Glob`,
///   `Bash(git diff:*)`, etc.). Each tool is passed as a separate argument.
/// - `--disallowedTools Edit Write`: removes `Edit` and `Write` from the model's context
///   entirely (defense in depth).
///
/// This is NOT a kernel-level sandbox (unlike `codex exec --sandbox read-only`). `claude -p`
/// has no sandbox flag; read-only behavior rests on the reviewer role + headless (`-p`) form.
pub struct ClaudeReviewer {
    /// Claude model name (e.g., `"claude-opus-4-7"`).
    model: String,
    /// Maximum time to wait for the Claude subprocess.
    timeout: Duration,
    /// Base review prompt to send to Claude (before the file list is appended).
    base_prompt: String,
    /// Scope label injected into the prompt (e.g., `"cli"`, `"infrastructure"`).
    scope_label: String,
    /// Test-only: override the Claude binary path (avoids unsafe env var mutation).
    #[cfg(test)]
    bin_override: Option<OsString>,
}

impl ClaudeReviewer {
    /// Constructs a new `ClaudeReviewer`.
    ///
    /// # Arguments
    /// - `model`: Claude model name.
    /// - `timeout`: Maximum time allowed for the review subprocess.
    /// - `base_prompt`: Review instructions without the scope file list.
    pub fn new<M: Into<String>, B: Into<String>>(
        model: M,
        timeout: Duration,
        base_prompt: B,
    ) -> Self {
        Self {
            model: model.into(),
            timeout,
            base_prompt: base_prompt.into(),
            scope_label: "scope".to_owned(),
            #[cfg(test)]
            bin_override: None,
        }
    }

    /// Sets the scope label injected into the review prompt.
    pub fn with_scope_label<S: Into<String>>(mut self, label: S) -> Self {
        self.scope_label = label.into();
        self
    }

    /// Test-only: set a custom binary path instead of the default `claude`.
    #[cfg(test)]
    pub(crate) fn with_bin(mut self, bin: impl Into<OsString>) -> Self {
        self.bin_override = Some(bin.into());
        self
    }

    /// Builds the full prompt by appending the scope file list to the base prompt.
    fn build_full_prompt(&self, target: &ReviewTarget, scope_label: &str) -> String {
        if target.is_empty() {
            return self.base_prompt.clone();
        }
        let file_list = target
            .files()
            .iter()
            .map(|f| format!("- {}", f.as_str()))
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            "{base}\n\n\
             ## Review scope: `{scope}`\n\n\
             Review ONLY the following files (this is the `{scope}` scope).\n\
             You have read-only access to the repo — use `git diff` to see changes.\n\n\
             Files:\n{file_list}",
            base = self.base_prompt,
            scope = scope_label,
        )
    }

    /// Runs the Claude review and returns a `ReviewOutcomeRaw`.
    ///
    /// `verdict_str` is the raw JSON string of the final verdict, extracted from the
    /// `structured_output` field of the `--output-format json` stdout envelope.
    /// Stderr is captured in memory; no files are written to the workspace (CN-05).
    fn run_review(
        &self,
        target: &ReviewTarget,
        scope_label: &str,
    ) -> Result<ReviewOutcomeRaw, ReviewerError> {
        let prompt = self.build_full_prompt(target, scope_label);

        #[cfg(test)]
        let bin = self.bin_override.clone().unwrap_or_else(claude_bin);
        #[cfg(not(test))]
        let bin = claude_bin();

        let (child, stderr_collector, stdout_collector) =
            spawn_claude(&bin, &self.model, &prompt).map_err(ReviewerError::Unexpected)?;

        run_claude_child(child, stderr_collector, stdout_collector, self.timeout)
    }
}

impl Reviewer for ClaudeReviewer {
    fn review(&self, target: &ReviewTarget) -> Result<(Verdict, LogInfo), ReviewerError> {
        let raw = self.run_review(target, &self.scope_label)?;
        let (verdict, log_info) = convert_raw_to_final(raw)?;
        Ok((verdict, log_info))
    }

    fn fast_review(&self, target: &ReviewTarget) -> Result<(FastVerdict, LogInfo), ReviewerError> {
        let raw = self.run_review(target, &self.scope_label)?;
        let (verdict, log_info) = convert_raw_to_fast(raw)?;
        Ok((verdict, log_info))
    }
}

/// Raw outcome from the Claude subprocess — parsed but not yet converted to domain types.
struct ReviewOutcomeRaw {
    verdict: ReviewVerdict,
    final_message: Option<String>,
    /// Captured stderr output (in-memory; no files written — CN-05).
    session_stderr: String,
}

/// Converts a raw Claude outcome to a final `(Verdict, LogInfo)`.
///
/// # Errors
/// Returns `ReviewerError` if the verdict indicates failure or the payload cannot be parsed.
fn convert_raw_to_final(raw: ReviewOutcomeRaw) -> Result<(Verdict, LogInfo), ReviewerError> {
    let payload = require_successful_payload(&raw)?;
    let log_info = LogInfo::new(raw.session_stderr);

    let verdict = match payload.verdict {
        ReviewPayloadVerdict::ZeroFindings => Verdict::ZeroFindings,
        ReviewPayloadVerdict::FindingsRemain => {
            let findings = convert_findings_to_domain(&payload.findings);
            Verdict::findings_remain(findings).map_err(|e: VerdictError| {
                ReviewerError::Unexpected(format!("verdict construction: {e}"))
            })?
        }
    };
    Ok((verdict, log_info))
}

/// Converts a raw Claude outcome to a fast `(FastVerdict, LogInfo)`.
///
/// # Errors
/// Returns `ReviewerError` if the verdict indicates failure or the payload cannot be parsed.
fn convert_raw_to_fast(raw: ReviewOutcomeRaw) -> Result<(FastVerdict, LogInfo), ReviewerError> {
    let payload = require_successful_payload(&raw)?;
    let log_info = LogInfo::new(raw.session_stderr);

    let verdict = match payload.verdict {
        ReviewPayloadVerdict::ZeroFindings => FastVerdict::ZeroFindings,
        ReviewPayloadVerdict::FindingsRemain => {
            let findings = convert_findings_to_domain(&payload.findings);
            FastVerdict::findings_remain(findings).map_err(|e: VerdictError| {
                ReviewerError::Unexpected(format!("verdict construction: {e}"))
            })?
        }
    };
    Ok((verdict, log_info))
}

/// Extracts the parsed payload from the raw outcome, mapping error variants to `ReviewerError`.
fn require_successful_payload(
    raw: &ReviewOutcomeRaw,
) -> Result<usecase::review_workflow::ReviewFinalPayload, ReviewerError> {
    match raw.verdict {
        ReviewVerdict::ZeroFindings | ReviewVerdict::FindingsRemain => {}
        ReviewVerdict::Timeout => return Err(ReviewerError::Timeout),
        ReviewVerdict::ProcessFailed => return Err(ReviewerError::ReviewerAbort),
        ReviewVerdict::LastMessageMissing => return Err(ReviewerError::IllegalVerdict),
    }

    let json = raw.final_message.as_deref().ok_or(ReviewerError::IllegalVerdict)?;
    match parse_review_final_message(Some(json)) {
        ReviewFinalMessageState::Parsed(p) => Ok(p),
        _ => Err(ReviewerError::IllegalVerdict),
    }
}

/// Converts `usecase::review_workflow::ReviewFinding` slice to domain `ReviewerFinding` vec.
fn convert_findings_to_domain(
    findings: &[usecase::review_workflow::ReviewFinding],
) -> Vec<ReviewerFinding> {
    findings
        .iter()
        .filter_map(|f| {
            ReviewerFinding::new(
                &f.message,
                f.severity.clone(),
                f.file.clone(),
                f.line,
                f.category.clone(),
            )
            .ok()
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Process management (internal helpers)
// ---------------------------------------------------------------------------

fn claude_bin() -> OsString {
    #[cfg(test)]
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

/// Builds the argument list for the `claude -p` invocation.
///
/// Best-effort, permission-based read-only invocation (CN-05). This is NOT a kernel-level sandbox
/// (unlike `codex exec --sandbox read-only`); `claude -p` has no sandbox flag.
/// 1. `--bare`: skips auto-discovery of host hooks, skills, MCP servers, and CLAUDE.md files
///    (reproducible invocation environment — CN-01).
/// 2. `--permission-mode dontAsk`: auto-denies tool calls not on the allow list — in standard
///    environments (no permissive host `permissions.allow` overrides) this prevents unlisted tools
///    from being invoked.
/// 3. `--allowedTools <tools...>`: each tool passed as a separate `OsString` argument (not
///    space-joined) to pre-approve only read-only inspection tools.
/// 4. `--disallowedTools Edit Write`: removes write tools from the model's context entirely
///    (defense in depth).
///
/// Read-only behavior rests on the reviewer role + headless output-only (`-p`) form; a permissive
/// host `.claude/settings.json` could in principle broaden the tool surface.
///
/// Uses `--output-format json` so the verdict appears in the `structured_output` field on stdout.
/// Uses `--json-schema` for API-level schema enforcement (grammar-compiled, CN-01).
fn build_claude_args(model: &str, prompt: &str) -> Vec<OsString> {
    let mut args = vec![
        OsString::from("-p"),
        OsString::from("--bare"),
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
        OsString::from(REVIEW_OUTPUT_SCHEMA_JSON),
        OsString::from("--model"),
        OsString::from(model),
        OsString::from(prompt),
    ]);
    args
}

/// Spawns the Claude subprocess, capturing stdout and stderr in memory (no files written — CN-05).
///
/// Returns `(child, stderr_collector_handle, stdout_collector_handle)`.
/// Both handles return the collected stream content as a `String` via their `JoinHandle`.
fn spawn_claude(bin: &std::ffi::OsStr, model: &str, prompt: &str) -> SpawnClaudeResult {
    let args = build_claude_args(model, prompt);

    let mut command = Command::new(bin);
    command.args(&args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child =
        command.spawn().map_err(|e| format!("failed to spawn {}: {e}", bin.to_string_lossy()))?;

    // Collect stderr in memory (echoed to the process stderr for observability).
    let stderr_collector = match child.stderr.take() {
        Some(pipe) => thread::spawn(move || {
            let mut buf = String::new();
            let reader = BufReader::new(pipe);
            for line in reader.lines() {
                match line {
                    Ok(l) => {
                        eprintln!("{l}");
                        buf.push_str(&l);
                        buf.push('\n');
                    }
                    Err(_) => break,
                }
            }
            buf
        }),
        None => thread::spawn(String::new),
    };

    // Collect stdout for later parsing.
    let stdout_collector = match child.stdout.take() {
        Some(pipe) => thread::spawn(move || {
            let mut buf = String::new();
            let reader = BufReader::new(pipe);
            for line in reader.lines() {
                match line {
                    Ok(l) => {
                        buf.push_str(&l);
                        buf.push('\n');
                    }
                    Err(_) => break,
                }
            }
            buf
        }),
        None => thread::spawn(String::new),
    };

    Ok((child, stderr_collector, stdout_collector))
}

fn run_claude_child(
    mut child: Child,
    stderr_collector: thread::JoinHandle<String>,
    stdout_collector: thread::JoinHandle<String>,
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
    let stdout_raw =
        if timed_out { String::new() } else { stdout_collector.join().unwrap_or_default() };

    // Collect stderr similarly — skip join on timeout to avoid blocking.
    let session_stderr =
        if timed_out { String::new() } else { stderr_collector.join().unwrap_or_default() };

    // Parse the --output-format json envelope from stdout and extract structured_output.
    let final_message = if timed_out || stdout_raw.trim().is_empty() {
        None
    } else {
        extract_structured_output(&stdout_raw)
    };

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

    Ok(ReviewOutcomeRaw { verdict, final_message: rendered_message, session_stderr })
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
    /// `--bare`, `--permission-mode dontAsk`, each read-only tool as a separate token after
    /// `--allowedTools`, and each disallowed tool as a separate token after `--disallowedTools`.
    ///
    /// This test is the canonical guard for the security-critical subprocess argv; the
    /// fake-binary integration tests cannot catch regressions here because the fake binaries
    /// ignore their arguments.
    #[test]
    fn test_build_claude_args_encodes_read_only_contract() {
        let model = "claude-opus-4-7";
        let prompt = "Review this.";
        let args = build_claude_args(model, prompt);

        // Collect as &str slices for readable assertions.
        let strs: Vec<&str> = args.iter().filter_map(|a| a.to_str()).collect();

        // Required positional prefix flags.
        assert!(strs.contains(&"-p"), "must pass -p");
        assert!(strs.contains(&"--bare"), "must pass --bare (CN-05 layer 1)");

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
    fn test_claude_reviewer_build_full_prompt_with_files() {
        let reviewer =
            ClaudeReviewer::new("claude-opus-4-7", Duration::from_secs(600), "Review this code.");
        let files = vec![
            domain::review_v2::FilePath::new("src/lib.rs").unwrap(),
            domain::review_v2::FilePath::new("src/main.rs").unwrap(),
        ];
        let target = ReviewTarget::new(files);
        let prompt = reviewer.build_full_prompt(&target, "domain");

        assert!(prompt.starts_with("Review this code."));
        assert!(prompt.contains("## Review scope: `domain`"));
        assert!(prompt.contains("- src/lib.rs"));
        assert!(prompt.contains("- src/main.rs"));
    }

    #[test]
    fn test_claude_reviewer_build_full_prompt_empty_target_returns_base_prompt() {
        let reviewer =
            ClaudeReviewer::new("claude-opus-4-7", Duration::from_secs(600), "Review this code.");
        let target = ReviewTarget::new(vec![]);
        let prompt = reviewer.build_full_prompt(&target, "domain");

        assert_eq!(prompt, "Review this code.");
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
    fn test_convert_findings_to_domain_skips_empty_message() {
        let findings = vec![usecase::review_workflow::ReviewFinding {
            message: "  ".to_owned(),
            severity: None,
            file: None,
            line: None,
            category: None,
        }];
        let result = convert_findings_to_domain(&findings);
        assert!(result.is_empty(), "empty-message findings must be filtered out: {result:?}");
    }

    #[cfg(unix)]
    #[test]
    fn test_review_with_fake_claude_zero_findings() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("fake-claude.sh");
        // The fake binary outputs a JSON envelope with structured_output on stdout.
        std::fs::write(
            &script,
            r#"#!/bin/sh
printf '{"type":"result","structured_output":{"verdict":"zero_findings","findings":[]}}\n'
exit 0
"#,
        )
        .unwrap();
        let mut perms = std::fs::metadata(&script).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script, perms).unwrap();

        let reviewer = ClaudeReviewer::new("claude-opus-4-7", Duration::from_secs(10), "Review.")
            .with_bin(&script);
        let target =
            ReviewTarget::new(vec![domain::review_v2::FilePath::new("src/lib.rs").unwrap()]);
        let result = reviewer.review(&target);

        let (verdict, _log) = result.expect("review should succeed");
        assert!(
            matches!(verdict, domain::review_v2::Verdict::ZeroFindings),
            "expected ZeroFindings, got: {verdict:?}"
        );
    }

    #[test]
    fn test_convert_findings_to_domain_converts_valid_finding() {
        let findings = vec![usecase::review_workflow::ReviewFinding {
            message: "Missing error handling".to_owned(),
            severity: Some("P1".to_owned()),
            file: Some("src/lib.rs".to_owned()),
            line: Some(42),
            category: Some("error_handling".to_owned()),
        }];
        let result = convert_findings_to_domain(&findings);
        assert_eq!(result.len(), 1);
        let finding = result.first().expect("expected one finding");
        assert_eq!(finding.message(), "Missing error handling");
        assert_eq!(finding.severity(), Some("P1"));
        assert_eq!(finding.file(), Some("src/lib.rs"));
        assert_eq!(finding.line(), Some(42));
        assert_eq!(finding.category(), Some("error_handling"));
    }

    #[cfg(unix)]
    #[test]
    fn test_fast_review_with_fake_claude_zero_findings() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("fake-claude-fast.sh");
        std::fs::write(
            &script,
            r#"#!/bin/sh
printf '{"type":"result","structured_output":{"verdict":"zero_findings","findings":[]}}\n'
exit 0
"#,
        )
        .unwrap();
        let mut perms = std::fs::metadata(&script).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script, perms).unwrap();

        let reviewer =
            ClaudeReviewer::new("claude-opus-4-7", Duration::from_secs(10), "Fast review.")
                .with_bin(&script);
        let target =
            ReviewTarget::new(vec![domain::review_v2::FilePath::new("src/lib.rs").unwrap()]);
        let result = reviewer.fast_review(&target);

        let (verdict, _log) = result.expect("fast_review should succeed");
        assert!(
            matches!(verdict, domain::review_v2::FastVerdict::ZeroFindings),
            "expected FastVerdict::ZeroFindings, got: {verdict:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_fast_review_with_fake_claude_findings_remain() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("fake-claude-fast-findings.sh");
        std::fs::write(
            &script,
            r#"#!/bin/sh
printf '{"type":"result","structured_output":{"verdict":"findings_remain","findings":[{"message":"A finding","severity":"P2","file":"src/lib.rs","line":10,"category":"style"}]}}\n'
exit 0
"#,
        )
        .unwrap();
        let mut perms = std::fs::metadata(&script).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script, perms).unwrap();

        let reviewer =
            ClaudeReviewer::new("claude-opus-4-7", Duration::from_secs(10), "Fast review.")
                .with_bin(&script);
        let target =
            ReviewTarget::new(vec![domain::review_v2::FilePath::new("src/lib.rs").unwrap()]);
        let result = reviewer.fast_review(&target);

        let (verdict, _log) = result.expect("fast_review should succeed");
        assert!(
            matches!(verdict, domain::review_v2::FastVerdict::FindingsRemain(_)),
            "expected FastVerdict::FindingsRemain, got: {verdict:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_review_subprocess_failure_returns_reviewer_abort() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("fake-claude-fail.sh");
        // Exit non-zero with no output — simulates subprocess crash.
        std::fs::write(&script, "#!/bin/sh\nexit 1\n").unwrap();
        let mut perms = std::fs::metadata(&script).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script, perms).unwrap();

        let reviewer = ClaudeReviewer::new("claude-opus-4-7", Duration::from_secs(10), "Review.")
            .with_bin(&script);
        let target =
            ReviewTarget::new(vec![domain::review_v2::FilePath::new("src/lib.rs").unwrap()]);
        let result = reviewer.review(&target);

        assert!(
            matches!(result, Err(usecase::review_v2::ReviewerError::ReviewerAbort)),
            "non-zero exit with no output must yield ReviewerAbort, got: {result:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_review_subprocess_timeout_returns_timeout_error() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("fake-claude-hang.sh");
        // Sleep longer than the reviewer timeout. Use a short sleep (2s) so the test
        // completes quickly even if the child process outlives the kill signal.
        std::fs::write(&script, "#!/bin/sh\nsleep 2\n").unwrap();
        let mut perms = std::fs::metadata(&script).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script, perms).unwrap();

        let reviewer =
            ClaudeReviewer::new("claude-opus-4-7", Duration::from_millis(200), "Review.")
                .with_bin(&script);
        let target = ReviewTarget::new(vec![]);
        let result = reviewer.review(&target);

        assert!(
            matches!(result, Err(usecase::review_v2::ReviewerError::Timeout)),
            "hanging subprocess must yield Timeout, got: {result:?}"
        );
    }
}
