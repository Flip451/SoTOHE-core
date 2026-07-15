//! Codex-backed implementation of the `Reviewer` usecase port.

use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

use domain::TrackId;
use domain::review_v2::{
    FastVerdict, LogInfo, ReviewTarget, ReviewerFinding, RoundType, ScopeName, Verdict,
    VerdictError,
};
use usecase::capability_exec::{ModelName, ReasoningEffort};
use usecase::provider_session::{ProviderSessionCachePort, ReviewerPrompt};
use usecase::review_v2::{ReviewerError, ports::Reviewer};
use usecase::review_workflow::{
    REVIEW_OUTPUT_SCHEMA_JSON, ReviewFinalMessageState, ReviewPayloadVerdict, ReviewVerdict,
    classify_review_verdict, normalize_final_message, parse_review_final_message,
    render_review_payload,
};

use super::session::{ReviewerSession, effort_value};
use crate::codex_common::{
    POLL_INTERVAL, REVIEW_RUNTIME_DIR, codex_bin, runtime_path, tee_stderr_to_file,
};
use crate::track::symlink_guard::reject_symlinks_up_to_root;

type SpawnCodexReviewerResult =
    Result<(Child, thread::JoinHandle<()>, thread::JoinHandle<Option<String>>), String>;

/// Codex-backed reviewer implementation for the `Reviewer` usecase port.
///
/// Spawns a `codex exec --sandbox read-only` subprocess, feeds it a review
/// prompt (base prompt + scope file list), polls for completion, and parses
/// the structured JSON verdict written to `--output-last-message`.
pub struct CodexReviewer {
    /// Codex model name (e.g., `"gpt-5.4"` or `"gpt-5.4-mini"`).
    model: ModelName,
    /// Maximum time to wait for the Codex subprocess.
    timeout: Duration,
    /// Base review prompt to send to Codex (before the file list is appended).
    base_prompt: String,
    /// Scope label injected into the prompt (e.g., `"cli"`, `"infrastructure"`).
    scope_label: String,
    session: ReviewerSession,
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
    #[allow(clippy::too_many_arguments)] // signature is the catalogue-declared contract
    pub fn new(
        track_id: TrackId,
        scope: ScopeName,
        round_type: RoundType,
        model: ModelName,
        effort: ReasoningEffort,
        timeout: Duration,
        base_prompt: ReviewerPrompt,
        session_cache: Arc<dyn ProviderSessionCachePort>,
    ) -> Self {
        Self {
            session: ReviewerSession::new(
                track_id,
                scope.clone(),
                round_type,
                "codex",
                model.clone(),
                effort,
                session_cache,
            ),
            model,
            timeout,
            base_prompt: base_prompt.as_str().to_owned(),
            scope_label: scope.to_string(),
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
             Re-read the CURRENT file list and CURRENT diff, then fully re-adjudicate this entire scope.\n\
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
        let output_schema = runtime_path(REVIEW_RUNTIME_DIR, "codex-output-schema", "json")
            .map_err(ReviewerError::Unexpected)?;
        let session_log = runtime_path(REVIEW_RUNTIME_DIR, "codex-session", "log")
            .map_err(ReviewerError::Unexpected)?;

        // Auto-managed: output-last-message and output-schema are cleaned up on drop.
        // Session log is NOT auto-managed — it persists for post-run debugging.
        let _cleanup = AutoManagedArtifacts::new([&output_last_message, &output_schema]);

        // Write output schema file.
        std::fs::write(&output_schema, REVIEW_OUTPUT_SCHEMA_JSON).map_err(|e| {
            ReviewerError::Unexpected(format!("failed to write output-schema: {e}"))
        })?;

        #[cfg(test)]
        let bin = self.bin_override.clone().unwrap_or_else(codex_bin);
        #[cfg(not(test))]
        let bin = codex_bin();

        let resume_id = self.session.resumable_id();
        let run = |resume_id: Option<&str>| {
            // Both resumed and fresh attempts share this path. Reset the
            // authoritative output before every child so a failed resume
            // cannot donate its verdict to the fresh retry.
            initialize_output_last_message(&output_last_message).map_err(|e| {
                ReviewerError::Unexpected(format!("failed to initialize output-last-message: {e}"))
            })?;
            let invocation = build_codex_reviewer_invocation(
                self.model.as_str(),
                effort_value(self.session.effort()),
                resume_id,
                &prompt,
                &output_last_message,
                &output_schema,
            );
            let (child, stderr, stdout) = spawn_codex_reviewer(&bin, &invocation, &session_log)
                .map_err(ReviewerError::Unexpected)?;
            run_codex_child(
                child,
                stderr,
                stdout,
                self.timeout,
                output_last_message.clone(),
                &session_log,
            )
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

impl Reviewer for CodexReviewer {
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

/// Raw outcome from the Codex subprocess — parsed but not yet converted to domain types.
struct ReviewOutcomeRaw {
    verdict: ReviewVerdict,
    final_message: Option<String>,
    session_log_path: PathBuf,
    session_id: Option<String>,
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
        None => runtime_path(REVIEW_RUNTIME_DIR, "codex-last-message", "txt"),
    }
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

fn run_codex_child(
    mut child: Child,
    stderr_collector: thread::JoinHandle<()>,
    stdout_collector: thread::JoinHandle<Option<String>>,
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

    let session_id = if timed_out { None } else { stdout_collector.join().unwrap_or_default() };
    if !timed_out {
        // Only join drain threads when the child exited normally.
        // On timeout, descendant processes may still hold the pipe FDs open,
        // causing the drain threads to block indefinitely. Dropping the
        // JoinHandles detaches the threads — they will terminate when all
        // FD holders close their end or when the process exits.
        let _ = stderr_collector.join();
    }

    let raw_content = match read_bounded_output_last_message(
        &output_last_message,
        MAX_CODEX_LAST_MESSAGE_BYTES,
    ) {
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
        session_id,
    })
}

/// Maximum size accepted for Codex's authoritative final-message file.
const MAX_CODEX_LAST_MESSAGE_BYTES: u64 = 4 * 1024 * 1024;

/// Reads an authoritative Codex final-message file within an explicit byte limit.
///
/// The extra byte detects overflow while avoiding an unbounded allocation. Callers
/// must treat an overflow as an error because this file is the verdict source.
fn read_bounded_output_last_message(path: &Path, max_bytes: u64) -> std::io::Result<String> {
    let metadata = std::fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("refusing to read symlinked output-last-message: {}", path.display()),
        ));
    }
    if !metadata.file_type().is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("output-last-message is not a regular file: {}", path.display()),
        ));
    }
    let file = std::fs::File::open(path)?;
    let mut bytes = Vec::new();
    file.take(max_bytes.saturating_add(1)).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > max_bytes {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!(
                "output-last-message exceeds maximum size of {max_bytes} bytes: {} bytes",
                bytes.len()
            ),
        ));
    }
    String::from_utf8(bytes).map_err(|error| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("output-last-message is not valid UTF-8: {error}"),
        )
    })
}

/// Empties the authoritative Codex final-message file without following symlinks.
///
/// The file is reset before every invocation so a failed resumed attempt cannot
/// donate a stale verdict to its fresh retry. On Unix, `O_NOFOLLOW` closes the
/// check-to-open race; truncation occurs only after the opened handle is verified
/// to be a regular file.
fn initialize_output_last_message(path: &Path) -> std::io::Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("refusing to initialize symlinked output-last-message: {}", path.display()),
            ));
        }
        Ok(metadata) if !metadata.file_type().is_file() => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("output-last-message is not a regular file: {}", path.display()),
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }

    // The runtime path is relative to the workspace. Check every existing
    // ancestor before opening the leaf so a symlinked `tmp/` or
    // `reviewer-runtime/` cannot redirect the authoritative verdict outside
    // the runtime tree.
    reject_symlinks_up_to_root(path)?;

    let mut options = OpenOptions::new();
    options.write(true).create(true);
    #[cfg(unix)]
    options.custom_flags(libc::O_NOFOLLOW);
    let file = options.open(path)?;
    if !file.metadata()?.file_type().is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("output-last-message is not a regular file: {}", path.display()),
        ));
    }
    file.set_len(0)
}

fn build_codex_reviewer_invocation(
    model: &str,
    effort: &str,
    resume_id: Option<&str>,
    prompt: &str,
    output_last_message: &Path,
    output_schema: &Path,
) -> Vec<std::ffi::OsString> {
    let mut args = vec![
        "exec".into(),
        "--model".into(),
        model.into(),
        "--sandbox".into(),
        "read-only".into(),
        "--config".into(),
        format!("model_reasoning_effort=\"{effort}\"").into(),
    ];
    if let Some(session_id) = resume_id {
        args.extend(["resume".into(), session_id.into()]);
    }
    args.extend([
        "--json".into(),
        "--output-schema".into(),
        output_schema.as_os_str().to_os_string(),
        "--output-last-message".into(),
        output_last_message.as_os_str().to_os_string(),
        prompt.into(),
    ]);
    args
}

fn spawn_codex_reviewer(
    bin: &std::ffi::OsStr,
    args: &[std::ffi::OsString],
    session_log_path: &Path,
) -> SpawnCodexReviewerResult {
    let mut command = Command::new(bin);
    command.args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|error| format!("failed to spawn {}: {error}", bin.to_string_lossy()))?;
    let log = std::fs::File::create(session_log_path).map_err(|error| {
        format!("failed to create session log {}: {error}", session_log_path.display())
    })?;
    let stderr = child
        .stderr
        .take()
        .map(|pipe| thread::spawn(move || tee_stderr_to_file(pipe, log)))
        .unwrap_or_else(|| thread::spawn(|| {}));
    let stdout = child
        .stdout
        .take()
        .map(|pipe| thread::spawn(move || collect_codex_session_id(pipe)))
        .unwrap_or_else(|| thread::spawn(|| None));
    Ok((child, stderr, stdout))
}

/// Maximum bytes retained for a single Codex JSON event while looking up `thread_id`.
///
/// Codex emits newline-delimited events. Larger or malformed events are discarded so a
/// malfunctioning child cannot make the reviewer retain an unbounded stdout stream.
const MAX_CODEX_EVENT_BYTES: usize = 64 * 1024;

/// Drains Codex's JSON event stream while retaining only the first bounded `thread_id` event.
fn collect_codex_session_id<R: Read>(pipe: R) -> Option<String> {
    let mut reader = BufReader::new(pipe);
    let mut event = Vec::with_capacity(MAX_CODEX_EVENT_BYTES);
    let mut discarding_event = false;
    let mut session_id = None;

    while let Ok(buffer) = reader.fill_buf() {
        if buffer.is_empty() {
            break;
        }

        if session_id.is_some() {
            let consumed = buffer.len();
            reader.consume(consumed);
            continue;
        }

        let newline = buffer.iter().position(|byte| *byte == b'\n');
        let event_bytes = newline.unwrap_or(buffer.len());
        if !discarding_event {
            let remaining = MAX_CODEX_EVENT_BYTES.saturating_sub(event.len());
            if event_bytes <= remaining {
                if let Some(event_part) = buffer.get(..event_bytes) {
                    event.extend_from_slice(event_part);
                } else {
                    discarding_event = true;
                }
            } else {
                discarding_event = true;
            }
        }

        let consumed = newline.map_or(buffer.len(), |index| index.saturating_add(1));
        reader.consume(consumed);

        if newline.is_some() {
            if !discarding_event {
                session_id = extract_codex_session_id_event(&event);
            }
            event.clear();
            discarding_event = false;
        }
    }

    session_id
}

fn extract_codex_session_id_event(event: &[u8]) -> Option<String> {
    let event = std::str::from_utf8(event).ok()?;
    let value = serde_json::from_str::<serde_json::Value>(event.trim()).ok()?;
    value.get("thread_id")?.as_str().filter(|id| !id.trim().is_empty()).map(str::to_owned)
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

    fn test_reviewer(timeout: Duration, prompt: &str) -> CodexReviewer {
        test_reviewer_with_cache(timeout, prompt, Arc::new(EmptySessionCache))
    }

    fn test_reviewer_with_cache(
        timeout: Duration,
        prompt: &str,
        cache: Arc<dyn ProviderSessionCachePort>,
    ) -> CodexReviewer {
        CodexReviewer::new(
            TrackId::try_new("test-track").unwrap(),
            ScopeName::Other,
            RoundType::Fast,
            ModelName::try_new("gpt-5.4").unwrap(),
            ReasoningEffort::High,
            timeout,
            ReviewerPrompt::try_new(prompt.to_owned()).unwrap(),
            cache,
        )
    }

    fn session_entry(provider: &str) -> usecase::provider_session::ProviderSessionCacheEntry {
        session_entry_with_model(provider, "gpt-5.4")
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

    #[test]
    fn test_codex_reviewer_build_full_prompt_with_files() {
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
    fn test_codex_reviewer_resume_and_fresh_rounds_reinject_flags_reread_scope_and_preserve_verdict_unit()
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
has_sandbox=0
has_effort=0
has_resume=0
output=""
last=""
previous=""
for argument in "$@"; do
  [ "$previous" = "--model" ] && [ "$argument" = "gpt-5.4" ] && has_model=1
  [ "$previous" = "--sandbox" ] && [ "$argument" = "read-only" ] && has_sandbox=1
  [ "$previous" = "--output-last-message" ] && output="$argument"
  [ "$argument" = "model_reasoning_effort=\"high\"" ] && has_effort=1
  [ "$argument" = "resume" ] && has_resume=1
  previous="$argument"
  last="$argument"
done
[ "$has_model" -eq 1 ] && [ "$has_sandbox" -eq 1 ] && [ "$has_effort" -eq 1 ] && [ -n "$output" ] && {resume_check} || exit 9
case "$last" in
  *"CURRENT file list"*"CURRENT diff"*"fully re-adjudicate this entire scope"*"src/lib.rs"*) ;;
  *) exit 10 ;;
esac
printf '{{"verdict":"zero_findings","findings":[]}}\n' > "$output"
printf '{{"thread_id":"new-session"}}\n'
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
            Arc::new(StaticSessionCache { entry: Some(session_entry("codex")) }),
        )
        .with_bin(write_bin("resume-codex.sh", true))
        .review(&target)
        .unwrap();
        let model_mismatch = test_reviewer_with_cache(
            Duration::from_secs(10),
            "Review this code.",
            Arc::new(StaticSessionCache {
                entry: Some(session_entry_with_model("codex", "previous-model")),
            }),
        )
        .with_bin(write_bin("model-mismatch-codex.sh", false))
        .review(&target)
        .unwrap();
        let provider_mismatch = test_reviewer_with_cache(
            Duration::from_secs(10),
            "Review this code.",
            Arc::new(StaticSessionCache { entry: Some(session_entry("claude")) }),
        )
        .with_bin(write_bin("provider-mismatch-codex.sh", false))
        .review(&target)
        .unwrap();
        let first_round = test_reviewer_with_cache(
            Duration::from_secs(10),
            "Review this code.",
            Arc::new(EmptySessionCache),
        )
        .with_bin(write_bin("first-round-codex.sh", false))
        .review(&target)
        .unwrap();

        assert_eq!(resumed.0, model_mismatch.0);
        assert_eq!(resumed.0, provider_mismatch.0);
        assert_eq!(resumed.0, first_round.0);
        assert!(matches!(resumed.0, Verdict::ZeroFindings));
        // Codex uses distinct log paths per subprocess, but resumed and fresh
        // results both preserve the record's verdict + non-empty log-info unit.
        assert!(!resumed.1.as_str().is_empty());
        assert!(!model_mismatch.1.as_str().is_empty());
        assert!(!provider_mismatch.1.as_str().is_empty());
        assert!(!first_round.1.as_str().is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn test_codex_reviewer_resume_failure_retries_fresh_invocation() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let attempts = dir.path().join("attempts.log");
        let script = dir.path().join("resume-fallback-codex.sh");
        std::fs::write(
            &script,
            format!(
                r#"#!/bin/sh
has_resume=0
has_model=0
has_sandbox=0
has_effort=0
output=""
previous=""
for argument in "$@"; do
  [ "$argument" = "resume" ] && has_resume=1
  [ "$previous" = "--model" ] && [ "$argument" = "gpt-5.4" ] && has_model=1
  [ "$previous" = "--sandbox" ] && [ "$argument" = "read-only" ] && has_sandbox=1
  [ "$previous" = "--output-last-message" ] && output="$argument"
  [ "$argument" = "model_reasoning_effort=\"high\"" ] && has_effort=1
  previous="$argument"
done
[ "$has_model" -eq 1 ] && [ "$has_sandbox" -eq 1 ] && [ "$has_effort" -eq 1 ] || exit 9
if [ "$has_resume" -eq 1 ]; then
  printf 'resume\n' >> '{}'
  exit 7
fi
printf 'fresh\n' >> '{}'
printf '{{"verdict":"zero_findings","findings":[]}}\n' > "$output"
printf '{{"thread_id":"new-session"}}\n'
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
            Arc::new(StaticSessionCache { entry: Some(session_entry("codex")) }),
        )
        .with_bin(script)
        .review(&target)
        .unwrap();

        assert!(matches!(record.0, Verdict::ZeroFindings));
        assert_eq!(std::fs::read_to_string(attempts).unwrap(), "resume\nfresh\n");
    }

    #[cfg(unix)]
    #[test]
    fn test_codex_reviewer_fresh_retry_does_not_adopt_failed_resume_verdict() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let attempts = dir.path().join("attempts.log");
        let script = dir.path().join("stale-resume-verdict-codex.sh");
        std::fs::write(
            &script,
            format!(
                r#"#!/bin/sh
has_resume=0
output=""
previous=""
for argument in "$@"; do
  [ "$argument" = "resume" ] && has_resume=1
  [ "$previous" = "--output-last-message" ] && output="$argument"
  previous="$argument"
done
if [ "$has_resume" -eq 1 ]; then
  printf 'resume\n' >> '{}'
  printf '{{"verdict":"zero_findings","findings":[]}}\n' > "$output"
  exit 7
fi
printf 'fresh\n' >> '{}'
exit 0
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
        let result = test_reviewer_with_cache(
            Duration::from_secs(10),
            "Review this code.",
            Arc::new(StaticSessionCache { entry: Some(session_entry("codex")) }),
        )
        .with_bin(script)
        .review(&target);

        assert!(matches!(result, Err(ReviewerError::IllegalVerdict)));
        assert_eq!(std::fs::read_to_string(attempts).unwrap(), "resume\nfresh\n");
    }

    #[cfg(unix)]
    #[test]
    fn test_codex_reviewer_expired_session_starts_fresh_with_explicit_flags() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let attempts = dir.path().join("attempts.log");
        let script = dir.path().join("expired-session-codex.sh");
        std::fs::write(
            &script,
            format!(
                r#"#!/bin/sh
has_resume=0
has_model=0
has_sandbox=0
has_effort=0
output=""
previous=""
for argument in "$@"; do
  [ "$argument" = "resume" ] && has_resume=1
  [ "$previous" = "--model" ] && [ "$argument" = "gpt-5.4" ] && has_model=1
  [ "$previous" = "--sandbox" ] && [ "$argument" = "read-only" ] && has_sandbox=1
  [ "$previous" = "--output-last-message" ] && output="$argument"
  [ "$argument" = "model_reasoning_effort=\"high\"" ] && has_effort=1
  previous="$argument"
done
[ "$has_model" -eq 1 ] && [ "$has_sandbox" -eq 1 ] && [ "$has_effort" -eq 1 ] && [ -n "$output" ] || exit 9
if [ "$has_resume" -eq 1 ]; then
  printf 'resume\n' >> '{}'
  printf 'expired or unknown session\n' >&2
  exit 7
fi
printf 'fresh\n' >> '{}'
printf '{{"verdict":"zero_findings","findings":[]}}\n' > "$output"
printf '{{"thread_id":"new-session"}}\n'
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
            Arc::new(StaticSessionCache { entry: Some(session_entry("codex")) }),
        )
        .with_bin(script)
        .review(&target)
        .unwrap();

        assert!(matches!(record.0, Verdict::ZeroFindings));
        assert_eq!(std::fs::read_to_string(attempts).unwrap(), "resume\nfresh\n");
    }

    #[cfg(unix)]
    #[test]
    fn test_codex_reviewer_resumes_only_matching_track_scope_and_round_key_unit() {
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
has_resume=0
has_model=0
has_sandbox=0
has_effort=0
output=""
previous=""
for argument in "$@"; do
  [ "$argument" = "resume" ] && has_resume=1
  [ "$previous" = "--model" ] && [ "$argument" = "gpt-5.4" ] && has_model=1
  [ "$previous" = "--sandbox" ] && [ "$argument" = "read-only" ] && has_sandbox=1
  [ "$previous" = "--output-last-message" ] && output="$argument"
  [ "$argument" = "model_reasoning_effort=\"high\"" ] && has_effort=1
  previous="$argument"
done
[ "$has_model" -eq 1 ] && [ "$has_sandbox" -eq 1 ] && [ "$has_effort" -eq 1 ] && [ -n "$output" ] && {resume_check} || exit 9
printf '{{"verdict":"zero_findings","findings":[]}}\n' > "$output"
printf '{{"thread_id":"new-session"}}\n'
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
        let matched_key = usecase::provider_session::ProviderSessionCacheKey::Review {
            track_id: track_id.clone(),
            scope: scope.clone(),
            round_type: RoundType::Fast,
        };
        let wrong_track_key = usecase::provider_session::ProviderSessionCacheKey::Review {
            track_id: TrackId::try_new("other-track").unwrap(),
            scope: scope.clone(),
            round_type: RoundType::Fast,
        };
        let wrong_scope_key = usecase::provider_session::ProviderSessionCacheKey::Review {
            track_id: track_id.clone(),
            scope: ScopeName::Main(
                domain::review_v2::MainScopeName::new("infrastructure").unwrap(),
            ),
            round_type: RoundType::Fast,
        };

        let run = |round_type, expected_key, name, expects_resume| {
            CodexReviewer::new(
                track_id.clone(),
                scope.clone(),
                round_type,
                ModelName::try_new("gpt-5.4").unwrap(),
                ReasoningEffort::High,
                Duration::from_secs(10),
                ReviewerPrompt::try_new("Review this code.".to_owned()).unwrap(),
                Arc::new(KeyedSessionCache { entry: Some(session_entry("codex")), expected_key }),
            )
            .with_bin(write_bin(name, expects_resume))
            .review(&target)
            .unwrap()
        };

        assert!(matches!(
            run(RoundType::Fast, wrong_track_key, "wrong-track-codex.sh", false).0,
            Verdict::ZeroFindings
        ));
        assert!(matches!(
            run(RoundType::Fast, wrong_scope_key, "wrong-scope-codex.sh", false).0,
            Verdict::ZeroFindings
        ));
        assert!(matches!(
            run(RoundType::Final, matched_key.clone(), "fast-keyed-final-codex.sh", false).0,
            Verdict::ZeroFindings
        ));
        assert!(matches!(
            run(RoundType::Fast, matched_key, "matching-key-codex.sh", true).0,
            Verdict::ZeroFindings
        ));
    }

    #[test]
    fn test_codex_reviewer_build_full_prompt_empty_target_returns_base_prompt() {
        let reviewer = test_reviewer(Duration::from_secs(600), "Review this code.");
        let target = ReviewTarget::new(vec![]);
        let prompt = reviewer.build_full_prompt(&target, "domain");

        assert_eq!(prompt, "Review this code.");
    }

    #[test]
    fn test_codex_resume_args_reinject_all_execution_flags() {
        let args = build_codex_reviewer_invocation(
            "codex-model",
            "xhigh",
            Some("prior-session"),
            "Review.",
            Path::new("tmp/last-message.json"),
            Path::new("tmp/schema.json"),
        );
        let args: Vec<&str> = args.iter().filter_map(|arg| arg.to_str()).collect();
        assert!(args.windows(2).any(|pair| pair == ["resume", "prior-session"]));
        assert!(args.windows(2).any(|pair| pair == ["--model", "codex-model"]));
        assert!(args.windows(2).any(|pair| pair == ["--sandbox", "read-only"]));
        assert!(args.contains(&"model_reasoning_effort=\"xhigh\""));
    }

    #[test]
    fn test_collect_codex_session_id_discards_oversized_event_and_keeps_draining() {
        let mut stdout = vec![b'x'; MAX_CODEX_EVENT_BYTES.saturating_add(1)];
        stdout.push(b'\n');
        stdout.extend_from_slice(br#"{"thread_id":"captured-session"}"#);
        stdout.push(b'\n');
        stdout.extend_from_slice(b"discarded-after-session\n");

        assert_eq!(
            collect_codex_session_id(std::io::Cursor::new(stdout)),
            Some("captured-session".to_owned())
        );
    }

    #[test]
    fn test_read_bounded_output_last_message_over_limit_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("last-message.json");
        std::fs::write(&path, b"abcde").unwrap();

        let error = read_bounded_output_last_message(&path, 4).unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("exceeds maximum size of 4 bytes"));
    }

    #[cfg(unix)]
    #[test]
    fn test_read_bounded_output_last_message_symlink_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("real-last-message.json");
        let link = dir.path().join("last-message.json");
        std::fs::write(&target, b"{}\n").unwrap();
        std::os::unix::fs::symlink(&target, &link).unwrap();

        let error = read_bounded_output_last_message(&link, 4).unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        assert!(error.to_string().contains("symlinked output-last-message"));
    }

    #[cfg(unix)]
    #[test]
    fn test_initialize_output_last_message_symlink_returns_error_without_truncating_target() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("verdict.json");
        let link = dir.path().join("last-message.json");
        std::fs::write(&target, b"preserve this verdict").unwrap();
        std::os::unix::fs::symlink(&target, &link).unwrap();

        let error = initialize_output_last_message(&link).unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        assert!(error.to_string().contains("symlinked output-last-message"));
        assert_eq!(std::fs::read(&target).unwrap(), b"preserve this verdict");
    }

    #[cfg(unix)]
    #[test]
    fn test_initialize_output_last_message_symlinked_parent_preserves_target() {
        let dir = tempfile::tempdir().unwrap();
        let redirected_parent = dir.path().join("redirected");
        let link_parent = dir.path().join("runtime");
        std::fs::create_dir_all(&redirected_parent).unwrap();
        std::fs::write(redirected_parent.join("last-message.json"), b"preserve this verdict")
            .unwrap();
        std::os::unix::fs::symlink(&redirected_parent, &link_parent).unwrap();

        let error =
            initialize_output_last_message(&link_parent.join("last-message.json")).unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        assert!(error.to_string().contains("refusing to follow symlink"));
        assert_eq!(
            std::fs::read(redirected_parent.join("last-message.json")).unwrap(),
            b"preserve this verdict"
        );
    }

    #[test]
    fn test_convert_findings_to_domain_skips_empty_message() {
        // ReviewerFindingError::EmptyMessage causes filter_map to skip the item
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
        let p1 = runtime_path(REVIEW_RUNTIME_DIR, "test-unique", "txt").unwrap();
        let p2 = runtime_path(REVIEW_RUNTIME_DIR, "test-unique", "txt").unwrap();
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
}
