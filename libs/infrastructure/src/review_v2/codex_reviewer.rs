//! Codex-backed implementation of the `Reviewer` usecase port.

use std::ffi::OsString;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use domain::review_v2::{FastVerdict, Finding, LogInfo, ReviewTarget, Verdict, VerdictError};
use usecase::review_v2::{ReviewerError, ports::Reviewer};
use usecase::review_workflow::{
    REVIEW_OUTPUT_SCHEMA_JSON, ReviewFinalMessageState, ReviewPayloadVerdict, ReviewVerdict,
    classify_review_verdict, normalize_final_message, parse_review_final_message,
    render_review_payload,
};

const REVIEW_RUNTIME_DIR: &str = "tmp/reviewer-runtime";
const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Environment variable for overriding the `codex` binary path in tests.
#[cfg(test)]
pub(crate) const CODEX_BIN_ENV: &str = "SOTP_CODEX_BIN";

/// Codex-backed reviewer implementation for the `Reviewer` usecase port.
///
/// Spawns a `codex exec --sandbox read-only` subprocess, feeds it a review
/// prompt (base prompt + scope file list), polls for completion, and parses
/// the structured JSON verdict written to `--output-last-message`.
pub struct CodexReviewer {
    /// Codex model name (e.g., `"gpt-5.4"` or `"gpt-5.4-mini"`).
    model: String,
    /// Maximum time to wait for the Codex subprocess.
    timeout: Duration,
    /// Base review prompt to send to Codex (before the file list is appended).
    base_prompt: String,
    /// Scope label injected into the prompt (e.g., `"cli"`, `"infrastructure"`).
    scope_label: String,
    /// Test-only: override the Codex binary path (avoids unsafe env var mutation).
    #[cfg(test)]
    bin_override: Option<std::ffi::OsString>,
}

impl CodexReviewer {
    /// Constructs a new `CodexReviewer`.
    ///
    /// # Arguments
    /// - `model`: Codex model name.
    /// - `timeout`: Maximum time allowed for the review subprocess.
    /// - `base_prompt`: Review instructions without the scope file list.
    pub fn new(
        model: impl Into<String>,
        timeout: Duration,
        base_prompt: impl Into<String>,
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
    pub fn with_scope_label(mut self, label: impl Into<String>) -> Self {
        self.scope_label = label.into();
        self
    }

    /// Test-only: set a custom binary path instead of the default `codex`.
    #[cfg(test)]
    pub(crate) fn with_bin(mut self, bin: impl Into<std::ffi::OsString>) -> Self {
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

    /// Runs the Codex review and returns a `(verdict_str, log_info)` pair.
    ///
    /// `verdict_str` is the raw JSON string of the final verdict.
    fn run_review(
        &self,
        target: &ReviewTarget,
        scope_label: &str,
    ) -> Result<ReviewOutcomeRaw, ReviewerError> {
        let prompt = self.build_full_prompt(target, scope_label);

        let output_last_message =
            prepare_output_last_message_path(None).map_err(ReviewerError::Unexpected)?;
        let output_schema = prepare_output_schema_path().map_err(ReviewerError::Unexpected)?;
        let session_log = prepare_session_log_path().map_err(ReviewerError::Unexpected)?;

        // Auto-managed: output-last-message and output-schema are cleaned up on drop.
        // Session log is NOT auto-managed — it persists for post-run debugging.
        let _cleanup = AutoManagedArtifacts::new([&output_last_message, &output_schema]);

        // Reset output-last-message to empty so stale content cannot be mistaken for a verdict.
        std::fs::write(&output_last_message, "").map_err(|e| {
            ReviewerError::Unexpected(format!("failed to initialize output-last-message: {e}"))
        })?;

        // Write output schema file.
        std::fs::write(&output_schema, REVIEW_OUTPUT_SCHEMA_JSON).map_err(|e| {
            ReviewerError::Unexpected(format!("failed to write output-schema: {e}"))
        })?;

        #[cfg(test)]
        let bin = self.bin_override.clone().unwrap_or_else(codex_bin);
        #[cfg(not(test))]
        let bin = codex_bin();

        let invocation =
            build_codex_invocation(&self.model, &prompt, &output_last_message, &output_schema);

        let (child, io_handles) =
            spawn_codex(&bin, &invocation, &session_log).map_err(ReviewerError::Unexpected)?;

        run_codex_child(child, io_handles, self.timeout, output_last_message, &session_log)
    }
}

impl Reviewer for CodexReviewer {
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

/// Raw outcome from the Codex subprocess — parsed but not yet converted to domain types.
struct ReviewOutcomeRaw {
    verdict: ReviewVerdict,
    final_message: Option<String>,
    session_log_path: PathBuf,
}

/// Converts a raw Codex outcome to a final `(Verdict, LogInfo)`.
///
/// # Errors
/// Returns `ReviewerError` if the verdict indicates failure or the payload cannot be parsed.
fn convert_raw_to_final(raw: ReviewOutcomeRaw) -> Result<(Verdict, LogInfo), ReviewerError> {
    let payload = require_successful_payload(&raw)?;
    let log_info = LogInfo::new(raw.session_log_path.display().to_string());

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

/// Converts a raw Codex outcome to a fast `(FastVerdict, LogInfo)`.
///
/// # Errors
/// Returns `ReviewerError` if the verdict indicates failure or the payload cannot be parsed.
fn convert_raw_to_fast(raw: ReviewOutcomeRaw) -> Result<(FastVerdict, LogInfo), ReviewerError> {
    let payload = require_successful_payload(&raw)?;
    let log_info = LogInfo::new(raw.session_log_path.display().to_string());

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

/// Converts `usecase::review_workflow::ReviewFinding` slice to domain `Finding` vec.
fn convert_findings_to_domain(
    findings: &[usecase::review_workflow::ReviewFinding],
) -> Vec<Finding> {
    findings
        .iter()
        .filter_map(|f| {
            Finding::new(&f.message, f.severity.clone(), f.file.clone(), f.line, f.category.clone())
                .ok()
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Process management (internal helpers)
// ---------------------------------------------------------------------------

/// Builds a timestamped path inside `REVIEW_RUNTIME_DIR`.
fn runtime_path(prefix: &str, ext: &str) -> Result<PathBuf, String> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| format!("failed to compute timestamp: {e}"))?
        .as_nanos();
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = PathBuf::from(REVIEW_RUNTIME_DIR)
        .join(format!("{prefix}-{}-{timestamp}-{seq}.{ext}", std::process::id()));
    let parent = path
        .parent()
        .ok_or_else(|| format!("runtime path must have a parent directory: {}", path.display()))?;
    std::fs::create_dir_all(parent)
        .map_err(|e| format!("failed to create {}: {e}", parent.display()))?;
    Ok(path)
}

fn prepare_output_last_message_path(explicit: Option<&Path>) -> Result<PathBuf, String> {
    match explicit {
        Some(p) => {
            let parent = p.parent().ok_or_else(|| {
                format!("output-last-message path has no parent: {}", p.display())
            })?;
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("failed to create {}: {e}", parent.display()))?;
            Ok(p.to_path_buf())
        }
        None => runtime_path("codex-last-message", "txt"),
    }
}

fn prepare_output_schema_path() -> Result<PathBuf, String> {
    runtime_path("codex-output-schema", "json")
}

fn prepare_session_log_path() -> Result<PathBuf, String> {
    runtime_path("codex-session", "log")
}

struct AutoManagedArtifacts {
    paths: Vec<PathBuf>,
}

impl AutoManagedArtifacts {
    fn new<'a>(artifacts: impl IntoIterator<Item = &'a PathBuf>) -> Self {
        Self { paths: artifacts.into_iter().cloned().collect() }
    }
}

impl Drop for AutoManagedArtifacts {
    fn drop(&mut self) {
        for path in &self.paths {
            let _ = std::fs::remove_file(path);
        }
    }
}

fn codex_bin() -> OsString {
    #[cfg(test)]
    if let Some(value) = std::env::var_os(CODEX_BIN_ENV).filter(|v| !v.is_empty()) {
        return value;
    }
    OsString::from("codex")
}

fn build_codex_invocation(
    model: &str,
    prompt: &str,
    output_last_message: &Path,
    output_schema: &Path,
) -> Vec<OsString> {
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
    args
}

fn spawn_codex(
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
fn drain_pipe(pipe: std::process::ChildStdout) {
    let reader = BufReader::new(pipe);
    for line in reader.lines() {
        if line.is_err() {
            break;
        }
    }
}

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

fn run_codex_child(
    mut child: Child,
    io_handles: Vec<thread::JoinHandle<()>>,
    timeout: Duration,
    output_last_message: PathBuf,
    session_log_path: &Path,
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
                    // Ignore kill error: the child may have exited between
                    // try_wait() returning None and this kill() call.
                    let _ = terminate_reviewer_child(&mut child);
                    child.wait().map_err(|e| {
                        ReviewerError::Unexpected(format!("failed to reap reviewer child: {e}"))
                    })?;
                    break;
                }
                thread::sleep(POLL_INTERVAL);
            }
        }
    }

    if !timed_out {
        // Only join drain threads when the child exited normally.
        // On timeout, descendant processes may still hold the pipe FDs open,
        // causing the drain threads to block indefinitely. Dropping the
        // JoinHandles detaches the threads — they will terminate when all
        // FD holders close their end or when the process exits.
        for handle in io_handles {
            let _ = handle.join();
        }
    }

    let raw_content = match std::fs::read_to_string(&output_last_message) {
        Ok(content) => normalize_final_message(&content),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            return Err(ReviewerError::Unexpected(format!(
                "failed to read output-last-message {}: {e}",
                output_last_message.display()
            )));
        }
    };

    let final_message_state = parse_review_final_message(raw_content.as_deref());

    // No session-log fallback: the --output-last-message file is the sole
    // authoritative verdict source. The session log contains stderr output
    // which is a diagnostic channel, not a verdict channel. Parsing it as
    // a fallback would turn a non-authoritative stream into an approval
    // source, breaking the fail-closed contract.

    let final_message = match &final_message_state {
        ReviewFinalMessageState::Parsed(payload) => Some(
            render_review_payload(payload).map_err(|e| ReviewerError::Unexpected(e.to_string()))?,
        ),
        _ => raw_content,
    };

    let verdict = classify_review_verdict(timed_out, exit_success, &final_message_state);

    Ok(ReviewOutcomeRaw {
        verdict,
        final_message,
        session_log_path: session_log_path.to_path_buf(),
    })
}

/// Terminates the reviewer child process.
///
/// Uses `child.kill()` (safe cross-platform API) to kill the direct child only.
/// Descendant processes spawned by the child are NOT terminated here.
///
/// # Why no process group kill
///
/// `killpg(2)` requires `unsafe` which is `#[forbid(unsafe_code)]` in this crate.
/// Process group termination is intentionally deferred to the CLI layer
/// (`apps/cli`) where `unsafe` is permitted. This is an accepted architectural
/// constraint — see `#[forbid(unsafe_code)]` policy for infrastructure crate.
fn terminate_reviewer_child(child: &mut Child) -> Result<(), String> {
    child.kill().map_err(|e| format!("failed to kill reviewer child: {e}"))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_codex_reviewer_build_full_prompt_with_files() {
        let reviewer = CodexReviewer::new("gpt-5.4", Duration::from_secs(600), "Review this code.");
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
    fn test_codex_reviewer_build_full_prompt_empty_target_returns_base_prompt() {
        let reviewer = CodexReviewer::new("gpt-5.4", Duration::from_secs(600), "Review this code.");
        let target = ReviewTarget::new(vec![]);
        let prompt = reviewer.build_full_prompt(&target, "domain");

        assert_eq!(prompt, "Review this code.");
    }

    #[test]
    fn test_convert_findings_to_domain_skips_empty_message() {
        // FindingError::EmptyMessage causes filter_map to skip the item
        let findings = vec![usecase::review_workflow::ReviewFinding {
            message: "  ".to_owned(), // whitespace-only → empty after trim
            severity: None,
            file: None,
            line: None,
            category: None,
        }];
        let result = convert_findings_to_domain(&findings);
        assert!(result.is_empty(), "empty-message findings must be filtered out: {result:?}");
    }

    #[test]
    fn test_runtime_path_is_unique_across_calls() {
        let p1 = runtime_path("test-unique", "txt").unwrap();
        let p2 = runtime_path("test-unique", "txt").unwrap();
        assert_ne!(p1, p2, "sequential runtime_path calls must produce unique names");
        // Cleanup
        let _ = std::fs::remove_file(&p1);
        let _ = std::fs::remove_file(&p2);
    }

    #[cfg(unix)]
    #[test]
    fn test_review_with_fake_codex_zero_findings() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("fake-codex.sh");
        std::fs::write(
            &script,
            r#"#!/bin/sh
out=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --output-last-message) out="$2"; shift 2 ;;
    *) shift ;;
  esac
done
if [ -n "$out" ]; then
  printf '{"verdict":"zero_findings","findings":[]}\n' > "$out"
fi
exit 0
"#,
        )
        .unwrap();
        let mut perms = std::fs::metadata(&script).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script, perms).unwrap();

        let reviewer = CodexReviewer::new("gpt-5.4-mini", Duration::from_secs(10), "Review.")
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
}
