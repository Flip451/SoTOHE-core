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

use std::collections::VecDeque;
use std::ffi::OsString;
use std::io::{Read, Write};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use domain::review_v2::{
    FastVerdict, LogInfo, ReviewTarget, ReviewerFinding, RoundType, ScopeName, Verdict,
    VerdictError,
};
use domain::{CommitHash, TrackId};
use std::sync::Arc;
use usecase::capability_exec::{CLAUDE_PROVIDER_NAME, ModelName, ReasoningEffort};
use usecase::provider_session::{ProviderSessionCachePort, ReviewerPrompt};
use usecase::review_v2::{
    ResolvedReviewer, ResolvedReviewerAssignment, ReviewerError, ports::Reviewer,
};
use usecase::review_workflow::{
    REVIEW_OUTPUT_SCHEMA_JSON, ReviewFinalMessageState, ReviewPayloadVerdict, ReviewVerdict,
    classify_review_verdict, normalize_final_message, parse_review_final_message,
    render_review_payload,
};

use super::session::{ReviewerSession, effort_value};
use crate::codex_common::POLL_INTERVAL;

/// Return type of `spawn_claude`: child process, stderr collector handle, and stdout collector handle.
///
/// Both handles collect the respective streams into `String` in memory (no files written — CN-05).
type SpawnClaudeResult =
    Result<(Child, thread::JoinHandle<String>, thread::JoinHandle<Option<String>>), String>;

/// Environment variable for overriding the `claude` binary path in tests.
#[cfg(any(test, feature = "test-helpers"))]
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
    model: ModelName,
    /// Maximum time to wait for the Claude subprocess.
    timeout: Duration,
    /// Base review prompt to send to Claude (before the file list is appended).
    base_prompt: String,
    /// Scope label injected into the prompt (e.g., `"cli"`, `"infrastructure"`).
    scope_label: String,
    /// Scope-bound cache identity and resolved execution profile.
    session: ReviewerSession,
    /// Validated assignment owned by this reviewer adapter.
    assignment: ResolvedReviewerAssignment,
    /// Test-only: override the Claude binary path (avoids unsafe env var mutation).
    #[cfg(any(test, feature = "test-helpers"))]
    bin_override: Option<OsString>,
}

impl ClaudeReviewer {
    /// Constructs a new `ClaudeReviewer`.
    ///
    /// # Arguments
    /// - `diff_base`: optional persisted review-cycle base that scopes session reuse.
    /// - `model`: Claude model name.
    /// - `timeout`: Maximum time allowed for the review subprocess.
    /// - `base_prompt`: Review instructions without the scope file list.
    #[allow(clippy::too_many_arguments)] // signature is the catalogue-declared contract
    pub fn new(
        track_id: TrackId,
        scope: ScopeName,
        round_type: RoundType,
        diff_base: Option<CommitHash>,
        model: ModelName,
        effort: ReasoningEffort,
        timeout: Duration,
        base_prompt: ReviewerPrompt,
        session_cache: Arc<dyn ProviderSessionCachePort>,
    ) -> Self {
        let assignment = ResolvedReviewerAssignment::new(
            track_id.clone(),
            scope.clone(),
            CLAUDE_PROVIDER_NAME.clone(),
            model.clone(),
            effort,
        );
        Self {
            session: ReviewerSession::new(
                track_id,
                scope.clone(),
                round_type,
                diff_base,
                "claude",
                model.clone(),
                effort,
                session_cache,
            ),
            model,
            timeout,
            base_prompt: base_prompt.as_str().to_owned(),
            scope_label: scope.to_string(),
            assignment,
            #[cfg(any(test, feature = "test-helpers"))]
            bin_override: None,
        }
    }

    /// Sets the scope label injected into the review prompt.
    pub fn with_scope_label<S: Into<String>>(mut self, label: S) -> Self {
        self.scope_label = label.into();
        self
    }

    /// Test-only: set a custom binary path instead of the default `claude`.
    ///
    /// Available in test builds and when the `test-helpers` feature is enabled.
    /// Used by integration tests in dependent crates (e.g. `cli_composition`) that
    /// need a fake Claude binary.
    #[cfg(any(test, feature = "test-helpers"))]
    pub fn with_bin(mut self, bin: impl Into<OsString>) -> Self {
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
             Re-read the CURRENT file list and CURRENT diff, then fully re-adjudicate this entire scope.\n\n\
             You have read-only access to the working tree — use `git diff` to see changes.\n\
             Read the listed files and the current diff before deciding. A verdict produced\n\
             without reading them is invalid — in particular, returning `zero_findings` without\n\
             having inspected the diff is not a pass.\n\n\
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

        #[cfg(any(test, feature = "test-helpers"))]
        let bin = self.bin_override.clone().unwrap_or_else(claude_bin);
        #[cfg(not(any(test, feature = "test-helpers")))]
        let bin = claude_bin();

        let resume_id = self.session.resumable_id();
        let run = |resume_id: Option<&str>| {
            let (child, stderr_collector, stdout_collector) = spawn_claude(
                &bin,
                self.model.as_str(),
                effort_value(self.session.effort()),
                resume_id,
                &prompt,
            )
            .map_err(ReviewerError::Unexpected)?;
            run_claude_child(child, stderr_collector, stdout_collector, self.timeout)
        };
        let attempted = run(resume_id.as_deref());
        if resume_id.is_some()
            && !matches!(
                attempted.as_ref().map(|raw| &raw.verdict),
                Ok(ReviewVerdict::ZeroFindings | ReviewVerdict::FindingsRemain)
            )
        {
            return run(None);
        }
        attempted
    }
}

impl ResolvedReviewer for ClaudeReviewer {
    fn resolved_assignment(&self) -> &ResolvedReviewerAssignment {
        &self.assignment
    }
}

impl Reviewer for ClaudeReviewer {
    fn review(&self, target: &ReviewTarget) -> Result<(Verdict, LogInfo), ReviewerError> {
        let raw = self.run_review(target, &self.scope_label)?;
        let session_id = raw.session_id.clone();
        let (verdict, log_info) = convert_raw_to_final(raw)?;
        self.session.save(session_id);
        Ok((verdict, log_info))
    }

    fn fast_review(&self, target: &ReviewTarget) -> Result<(FastVerdict, LogInfo), ReviewerError> {
        let raw = self.run_review(target, &self.scope_label)?;
        let session_id = raw.session_id.clone();
        let (verdict, log_info) = convert_raw_to_fast(raw)?;
        self.session.save(session_id);
        Ok((verdict, log_info))
    }
}

/// Raw outcome from the Claude subprocess — parsed but not yet converted to domain types.
struct ReviewOutcomeRaw {
    verdict: ReviewVerdict,
    final_message: Option<String>,
    /// Captured stderr output (in-memory; no files written — CN-05).
    session_stderr: String,
    session_id: Option<String>,
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
fn build_claude_args(
    model: &str,
    effort: &str,
    resume_id: Option<&str>,
    prompt: &str,
) -> Vec<OsString> {
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
        OsString::from("--effort"),
        OsString::from(effort),
    ]);
    if let Some(session_id) = resume_id {
        args.extend([OsString::from("--resume"), OsString::from(session_id)]);
    }
    args.push(OsString::from(prompt));
    args
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
fn spawn_claude(
    bin: &std::ffi::OsStr,
    model: &str,
    effort: &str,
    resume_id: Option<&str>,
    prompt: &str,
) -> SpawnClaudeResult {
    let args = build_claude_args(model, effort, resume_id, prompt);

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

fn run_claude_child(
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

    struct StaticSessionCache {
        entry: Option<usecase::provider_session::ProviderSessionCacheEntry>,
    }

    impl ProviderSessionCachePort for StaticSessionCache {
        fn load(
            &self,
            _: &usecase::provider_session::ProviderSessionCacheKey,
        ) -> Result<
            Option<usecase::provider_session::ProviderSessionCacheEntry>,
            usecase::provider_session::ProviderSessionCacheError,
        > {
            Ok(self.entry.clone())
        }

        fn save(
            &self,
            _: &usecase::provider_session::ProviderSessionCacheKey,
            _: &usecase::provider_session::ProviderSessionCacheEntry,
        ) -> Result<(), usecase::provider_session::ProviderSessionCacheError> {
            Ok(())
        }

        fn remove(
            &self,
            _: &usecase::provider_session::ProviderSessionCacheKey,
        ) -> Result<(), usecase::provider_session::ProviderSessionCacheError> {
            Ok(())
        }
    }

    struct KeyedSessionCache {
        entry: Option<usecase::provider_session::ProviderSessionCacheEntry>,
        expected_key: usecase::provider_session::ProviderSessionCacheKey,
    }

    impl ProviderSessionCachePort for KeyedSessionCache {
        fn load(
            &self,
            key: &usecase::provider_session::ProviderSessionCacheKey,
        ) -> Result<
            Option<usecase::provider_session::ProviderSessionCacheEntry>,
            usecase::provider_session::ProviderSessionCacheError,
        > {
            Ok((key == &self.expected_key).then(|| self.entry.clone()).flatten())
        }

        fn save(
            &self,
            _: &usecase::provider_session::ProviderSessionCacheKey,
            _: &usecase::provider_session::ProviderSessionCacheEntry,
        ) -> Result<(), usecase::provider_session::ProviderSessionCacheError> {
            Ok(())
        }

        fn remove(
            &self,
            _: &usecase::provider_session::ProviderSessionCacheKey,
        ) -> Result<(), usecase::provider_session::ProviderSessionCacheError> {
            Ok(())
        }
    }

    struct EmptySessionCache;

    impl ProviderSessionCachePort for EmptySessionCache {
        fn load(
            &self,
            _: &usecase::provider_session::ProviderSessionCacheKey,
        ) -> Result<
            Option<usecase::provider_session::ProviderSessionCacheEntry>,
            usecase::provider_session::ProviderSessionCacheError,
        > {
            Ok(None)
        }
        fn save(
            &self,
            _: &usecase::provider_session::ProviderSessionCacheKey,
            _: &usecase::provider_session::ProviderSessionCacheEntry,
        ) -> Result<(), usecase::provider_session::ProviderSessionCacheError> {
            Ok(())
        }
        fn remove(
            &self,
            _: &usecase::provider_session::ProviderSessionCacheKey,
        ) -> Result<(), usecase::provider_session::ProviderSessionCacheError> {
            Ok(())
        }
    }

    fn test_reviewer(timeout: Duration, prompt: &str) -> ClaudeReviewer {
        test_reviewer_with_cache(timeout, prompt, Arc::new(EmptySessionCache))
    }

    fn test_reviewer_with_cache(
        timeout: Duration,
        prompt: &str,
        cache: Arc<dyn ProviderSessionCachePort>,
    ) -> ClaudeReviewer {
        ClaudeReviewer::new(
            TrackId::try_new("test-track").unwrap(),
            ScopeName::Other,
            RoundType::Fast,
            Some(CommitHash::try_new("a1b2c3d").unwrap()),
            ModelName::try_new("claude-opus-4-7").unwrap(),
            ReasoningEffort::High,
            timeout,
            ReviewerPrompt::try_new(prompt.to_owned()).unwrap(),
            cache,
        )
    }

    #[test]
    fn test_claude_reviewer_resolved_assignment_returns_adapter_values() {
        let mut reviewer: ClaudeReviewer = test_reviewer(Duration::from_secs(10), "Review.");
        let assignment_before =
            <ClaudeReviewer as usecase::review_v2::ResolvedReviewer>::resolved_assignment(
                &reviewer,
            )
            .clone();

        // `model` and `scope_label` are mutable invocation configuration. The
        // constructor stores a separate resolved assignment snapshot, so a
        // later configuration change must not alter values persisted to telemetry.
        reviewer.model = ModelName::try_new("profile-mutated-model").unwrap();
        reviewer.scope_label = "profile-mutated-scope".to_owned();
        let assignment =
            <ClaudeReviewer as usecase::review_v2::ResolvedReviewer>::resolved_assignment(
                &reviewer,
            );

        assert_eq!(reviewer.model.as_str(), "profile-mutated-model");
        assert_eq!(reviewer.scope_label, "profile-mutated-scope");
        assert_eq!(assignment, &assignment_before);
        assert_eq!(assignment.track_id().as_ref(), "test-track");
        assert_eq!(assignment.scope(), &ScopeName::Other);
        assert_eq!(assignment.provider().as_str(), "claude");
        assert_eq!(assignment.model().as_str(), "claude-opus-4-7");
        assert_eq!(assignment.reasoning_effort(), ReasoningEffort::High);
    }

    fn session_entry(provider: &str) -> usecase::provider_session::ProviderSessionCacheEntry {
        session_entry_with_model(provider, "claude-opus-4-7")
    }

    fn session_entry_with_model(
        provider: &str,
        model: &str,
    ) -> usecase::provider_session::ProviderSessionCacheEntry {
        usecase::provider_session::ProviderSessionCacheEntry::new(
            usecase::provider_session::ProviderSessionId::try_new("prior-session".to_owned())
                .unwrap(),
            usecase::capability_exec::ProviderName::try_new(provider.to_owned()).unwrap(),
            ModelName::try_new(model.to_owned()).unwrap(),
            ReasoningEffort::High,
        )
    }

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
        let args = build_claude_args(model, "high", None, prompt);

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
    fn test_build_claude_resume_args_reinject_all_execution_flags() {
        let args = build_claude_args("claude-model", "max", Some("prior-session"), "Review.");
        let args: Vec<&str> = args.iter().filter_map(|arg| arg.to_str()).collect();
        assert!(args.windows(2).any(|pair| pair == ["--resume", "prior-session"]));
        assert!(args.windows(2).any(|pair| pair == ["--model", "claude-model"]));
        assert!(args.windows(2).any(|pair| pair == ["--effort", "max"]));
        assert!(args.windows(2).any(|pair| pair == ["--permission-mode", "dontAsk"]));
    }

    #[test]
    fn test_claude_reviewer_build_full_prompt_with_files() {
        let reviewer = test_reviewer(Duration::from_secs(600), "Review this code.");
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
        assert!(prompt.contains("CURRENT file list"));
        assert!(prompt.contains("CURRENT diff"));
        assert!(prompt.contains("fully re-adjudicate this entire scope"));
    }

    #[cfg(unix)]
    #[test]
    fn test_claude_reviewer_resume_and_fresh_rounds_reinject_flags_reread_scope_and_preserve_verdict_unit()
     {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let write_bin = |name: &str, expected_resume: bool| {
            let script = dir.path().join(name);
            let resume_check = if expected_resume {
                "[ \"$has_resume\" -eq 1 ]"
            } else {
                "[ \"$has_resume\" -eq 0 ]"
            };
            std::fs::write(
                &script,
                format!(
                    r#"#!/bin/sh
has_model=0
has_effort=0
has_permission=0
has_resume=0
last=""
previous=""
for argument in "$@"; do
  [ "$previous" = "--model" ] && [ "$argument" = "claude-opus-4-7" ] && has_model=1
  [ "$previous" = "--effort" ] && [ "$argument" = "high" ] && has_effort=1
  [ "$previous" = "--permission-mode" ] && [ "$argument" = "dontAsk" ] && has_permission=1
  [ "$argument" = "--resume" ] && has_resume=1
  previous="$argument"
  last="$argument"
done
[ "$has_model" -eq 1 ] && [ "$has_effort" -eq 1 ] && [ "$has_permission" -eq 1 ] && {resume_check} || exit 9
case "$last" in
  *"CURRENT file list"*"CURRENT diff"*"fully re-adjudicate this entire scope"*"src/lib.rs"*) ;;
  *) exit 10 ;;
esac
printf '{{"type":"result","session_id":"new-session","structured_output":{{"verdict":"zero_findings","findings":[]}}}}\n'
"#
                ),
            )
            .unwrap();
            let mut permissions = std::fs::metadata(&script).unwrap().permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(&script, permissions).unwrap();
            script
        };

        let target =
            ReviewTarget::new(vec![domain::review_v2::FilePath::new("src/lib.rs").unwrap()]);
        let resumed = test_reviewer_with_cache(
            Duration::from_secs(10),
            "Review this code.",
            Arc::new(StaticSessionCache { entry: Some(session_entry("claude")) }),
        )
        .with_bin(write_bin("resume-claude.sh", true))
        .review(&target)
        .unwrap();
        let model_mismatch = test_reviewer_with_cache(
            Duration::from_secs(10),
            "Review this code.",
            Arc::new(StaticSessionCache {
                entry: Some(session_entry_with_model("claude", "previous-model")),
            }),
        )
        .with_bin(write_bin("model-mismatch-claude.sh", false))
        .review(&target)
        .unwrap();
        let provider_mismatch = test_reviewer_with_cache(
            Duration::from_secs(10),
            "Review this code.",
            Arc::new(StaticSessionCache { entry: Some(session_entry("codex")) }),
        )
        .with_bin(write_bin("provider-mismatch-claude.sh", false))
        .review(&target)
        .unwrap();
        let first_round = test_reviewer_with_cache(
            Duration::from_secs(10),
            "Review this code.",
            Arc::new(EmptySessionCache),
        )
        .with_bin(write_bin("first-round-claude.sh", false))
        .review(&target)
        .unwrap();

        assert_eq!(resumed.0, model_mismatch.0);
        assert_eq!(resumed.0, provider_mismatch.0);
        assert_eq!(resumed.0, first_round.0);
        assert!(matches!(resumed.0, Verdict::ZeroFindings));
        // A resume changes only provider context. Every invocation still returns
        // the same single-round review record shape: verdict plus log info.
        assert_eq!(resumed.1, model_mismatch.1);
        assert_eq!(resumed.1, provider_mismatch.1);
        assert_eq!(resumed.1, first_round.1);
    }

    #[cfg(unix)]
    #[test]
    fn test_claude_reviewer_resume_failure_retries_fresh_invocation() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let attempts = dir.path().join("attempts.log");
        let script = dir.path().join("resume-fallback-claude.sh");
        std::fs::write(
            &script,
            format!(
                r#"#!/bin/sh
has_resume=0
has_model=0
has_effort=0
has_permission=0
previous=""
for argument in "$@"; do
  [ "$argument" = "--resume" ] && has_resume=1
  [ "$previous" = "--model" ] && [ "$argument" = "claude-opus-4-7" ] && has_model=1
  [ "$previous" = "--effort" ] && [ "$argument" = "high" ] && has_effort=1
  [ "$previous" = "--permission-mode" ] && [ "$argument" = "dontAsk" ] && has_permission=1
  previous="$argument"
done
[ "$has_model" -eq 1 ] && [ "$has_effort" -eq 1 ] && [ "$has_permission" -eq 1 ] || exit 9
if [ "$has_resume" -eq 1 ]; then
  printf 'resume\n' >> '{}'
  exit 7
fi
printf 'fresh\n' >> '{}'
printf '{{"type":"result","session_id":"new-session","structured_output":{{"verdict":"zero_findings","findings":[]}}}}\n'
"#,
                attempts.display(),
                attempts.display()
            ),
        )
        .unwrap();
        let mut permissions = std::fs::metadata(&script).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&script, permissions).unwrap();

        let target =
            ReviewTarget::new(vec![domain::review_v2::FilePath::new("src/lib.rs").unwrap()]);
        let record = test_reviewer_with_cache(
            Duration::from_secs(10),
            "Review this code.",
            Arc::new(StaticSessionCache { entry: Some(session_entry("claude")) }),
        )
        .with_bin(script)
        .review(&target)
        .unwrap();

        assert!(matches!(record.0, Verdict::ZeroFindings));
        assert_eq!(std::fs::read_to_string(attempts).unwrap(), "resume\nfresh\n");
    }

    #[cfg(unix)]
    #[test]
    fn test_claude_reviewer_expired_session_starts_fresh_with_explicit_flags() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let attempts = dir.path().join("attempts.log");
        let script = dir.path().join("expired-session-claude.sh");
        std::fs::write(
            &script,
            format!(
                r#"#!/bin/sh
has_resume=0
has_model=0
has_effort=0
has_permission=0
previous=""
for argument in "$@"; do
  [ "$argument" = "--resume" ] && has_resume=1
  [ "$previous" = "--model" ] && [ "$argument" = "claude-opus-4-7" ] && has_model=1
  [ "$previous" = "--effort" ] && [ "$argument" = "high" ] && has_effort=1
  [ "$previous" = "--permission-mode" ] && [ "$argument" = "dontAsk" ] && has_permission=1
  previous="$argument"
done
[ "$has_model" -eq 1 ] && [ "$has_effort" -eq 1 ] && [ "$has_permission" -eq 1 ] || exit 9
if [ "$has_resume" -eq 1 ]; then
  printf 'resume\n' >> '{}'
  printf 'expired or unknown session\n' >&2
  exit 7
fi
printf 'fresh\n' >> '{}'
printf '{{"type":"result","session_id":"new-session","structured_output":{{"verdict":"zero_findings","findings":[]}}}}\n'
"#,
                attempts.display(),
                attempts.display()
            ),
        )
        .unwrap();
        let mut permissions = std::fs::metadata(&script).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&script, permissions).unwrap();

        let target =
            ReviewTarget::new(vec![domain::review_v2::FilePath::new("src/lib.rs").unwrap()]);
        let record = test_reviewer_with_cache(
            Duration::from_secs(10),
            "Review this code.",
            Arc::new(StaticSessionCache { entry: Some(session_entry("claude")) }),
        )
        .with_bin(script)
        .review(&target)
        .unwrap();

        assert!(matches!(record.0, Verdict::ZeroFindings));
        assert_eq!(std::fs::read_to_string(attempts).unwrap(), "resume\nfresh\n");
    }

    #[cfg(unix)]
    #[test]
    fn test_claude_reviewer_resumes_only_matching_track_scope_and_round_key_unit() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let write_bin = |name: &str, expected_resume: bool| {
            let script = dir.path().join(name);
            let resume_check = if expected_resume {
                "[ \"$has_resume\" -eq 1 ]"
            } else {
                "[ \"$has_resume\" -eq 0 ]"
            };
            std::fs::write(
                &script,
                format!(
                    r#"#!/bin/sh
has_model=0
has_effort=0
has_permission=0
has_resume=0
previous=""
for argument in "$@"; do
  [ "$previous" = "--model" ] && [ "$argument" = "claude-opus-4-7" ] && has_model=1
  [ "$previous" = "--effort" ] && [ "$argument" = "high" ] && has_effort=1
  [ "$previous" = "--permission-mode" ] && [ "$argument" = "dontAsk" ] && has_permission=1
  [ "$argument" = "--resume" ] && has_resume=1
  previous="$argument"
done
[ "$has_model" -eq 1 ] && [ "$has_effort" -eq 1 ] && [ "$has_permission" -eq 1 ] && {resume_check} || exit 9
printf '{{"type":"result","session_id":"new-session","structured_output":{{"verdict":"zero_findings","findings":[]}}}}\n'
"#
                ),
            )
            .unwrap();
            let mut permissions = std::fs::metadata(&script).unwrap().permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(&script, permissions).unwrap();
            script
        };
        let target =
            ReviewTarget::new(vec![domain::review_v2::FilePath::new("src/lib.rs").unwrap()]);
        let track_id = TrackId::try_new("test-track").unwrap();
        let scope = ScopeName::Other;
        let diff_base = CommitHash::try_new("a1b2c3d").unwrap();
        let matched_key = usecase::provider_session::ProviderSessionCacheKey::Review {
            track_id: track_id.clone(),
            scope: scope.clone(),
            round_type: RoundType::Fast,
            diff_base: diff_base.clone(),
        };
        let wrong_track_key = usecase::provider_session::ProviderSessionCacheKey::Review {
            track_id: TrackId::try_new("other-track").unwrap(),
            scope: scope.clone(),
            round_type: RoundType::Fast,
            diff_base: diff_base.clone(),
        };
        let wrong_scope_key = usecase::provider_session::ProviderSessionCacheKey::Review {
            track_id: track_id.clone(),
            scope: ScopeName::Main(
                domain::review_v2::MainScopeName::new("infrastructure").unwrap(),
            ),
            round_type: RoundType::Fast,
            diff_base: diff_base.clone(),
        };

        let run = |round_type, expected_key, name, expects_resume| {
            ClaudeReviewer::new(
                track_id.clone(),
                scope.clone(),
                round_type,
                Some(diff_base.clone()),
                ModelName::try_new("claude-opus-4-7").unwrap(),
                ReasoningEffort::High,
                Duration::from_secs(10),
                ReviewerPrompt::try_new("Review this code.".to_owned()).unwrap(),
                Arc::new(KeyedSessionCache { entry: Some(session_entry("claude")), expected_key }),
            )
            .with_bin(write_bin(name, expects_resume))
            .review(&target)
            .unwrap()
        };

        assert!(matches!(
            run(RoundType::Fast, wrong_track_key, "wrong-track-claude.sh", false).0,
            Verdict::ZeroFindings
        ));
        assert!(matches!(
            run(RoundType::Fast, wrong_scope_key, "wrong-scope-claude.sh", false).0,
            Verdict::ZeroFindings
        ));
        assert!(matches!(
            run(RoundType::Final, matched_key.clone(), "fast-keyed-final-claude.sh", false).0,
            Verdict::ZeroFindings
        ));
        assert!(matches!(
            run(RoundType::Fast, matched_key, "matching-key-claude.sh", true).0,
            Verdict::ZeroFindings
        ));
    }

    #[test]
    fn test_claude_reviewer_build_full_prompt_empty_target_returns_base_prompt() {
        let reviewer = test_reviewer(Duration::from_secs(600), "Review this code.");
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

        let reviewer = test_reviewer(Duration::from_secs(10), "Review.").with_bin(&script);
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

        let reviewer = test_reviewer(Duration::from_secs(10), "Fast review.").with_bin(&script);
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

        let reviewer = test_reviewer(Duration::from_secs(10), "Fast review.").with_bin(&script);
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

        let reviewer = test_reviewer(Duration::from_secs(10), "Review.").with_bin(&script);
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

        let reviewer = test_reviewer(Duration::from_millis(200), "Review.").with_bin(&script);
        let target = ReviewTarget::new(vec![]);
        let result = reviewer.review(&target);

        assert!(
            matches!(result, Err(usecase::review_v2::ReviewerError::Timeout)),
            "hanging subprocess must yield Timeout, got: {result:?}"
        );
    }
}
