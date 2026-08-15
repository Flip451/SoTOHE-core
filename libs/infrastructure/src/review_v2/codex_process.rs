//! Process and runtime-artifact helpers for the Codex reviewer adapter.

use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

use usecase::review_v2::ReviewerError;
use usecase::review_workflow::{
    ReviewFinalMessageState, ReviewVerdict, classify_review_verdict, normalize_final_message,
    parse_review_final_message, render_review_payload,
};

use crate::codex_common::{
    POLL_INTERVAL, REVIEW_RUNTIME_DIR, configure_codex_command, runtime_path, tee_stderr_to_file,
};
use crate::track::symlink_guard::reject_symlinks_up_to_root;

/// Raw outcome from the Codex subprocess — parsed but not yet converted to domain types.
pub(super) struct ReviewOutcomeRaw {
    pub(super) verdict: ReviewVerdict,
    pub(super) final_message: Option<String>,
    pub(super) session_log_path: PathBuf,
    pub(super) session_id: Option<String>,
}

pub(super) fn prepare_output_last_message_path(explicit: Option<&Path>) -> Result<PathBuf, String> {
    match explicit {
        Some(path) => {
            let parent = path.parent().ok_or_else(|| {
                format!("output-last-message path has no parent: {}", path.display())
            })?;
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
            Ok(path.to_path_buf())
        }
        None => runtime_path(REVIEW_RUNTIME_DIR, "codex-last-message", "txt"),
    }
}

pub(super) struct AutoManagedArtifacts {
    paths: Vec<PathBuf>,
}

impl AutoManagedArtifacts {
    pub(super) fn new<'a>(artifacts: impl IntoIterator<Item = &'a PathBuf>) -> Self {
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

pub(super) fn run_codex_child(
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
pub(super) const MAX_CODEX_LAST_MESSAGE_BYTES: u64 = 4 * 1024 * 1024;

/// Opens a runtime artifact without following a symlink at its leaf.
///
/// The metadata checks performed by callers provide useful diagnostics, but are not
/// sufficient on their own because the path can be replaced between the check and
/// the open. Unsupported platforms fail closed instead of following the path.
fn open_no_follow(options: &mut OpenOptions, path: &Path) -> std::io::Result<File> {
    reject_symlinks_up_to_root(path)?;
    #[cfg(unix)]
    options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt as _;
        // FILE_FLAG_OPEN_REPARSE_POINT opens the reparse point itself so the
        // opened-handle metadata check can reject it.
        options.custom_flags(0x0020_0000);
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = path;
        return Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "atomic no-follow runtime-artifact open is unavailable on this platform",
        ));
    }
    options.open(path)
}

/// Writes a runtime artifact without following a symlink at any path component.
pub(super) fn write_runtime_artifact(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("refusing to write symlinked runtime artifact: {}", path.display()),
            ));
        }
        Ok(metadata) if !metadata.file_type().is_file() => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("runtime artifact is not a regular file: {}", path.display()),
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }

    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    let file = open_no_follow(&mut options, path)?;
    if !file.metadata()?.file_type().is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("runtime artifact is not a regular file: {}", path.display()),
        ));
    }
    use std::io::Write as _;
    let mut file = file;
    file.write_all(contents)
}

/// Reads an authoritative Codex final-message file within an explicit byte limit.
///
/// The extra byte detects overflow while avoiding an unbounded allocation. Callers
/// must treat an overflow as an error because this file is the verdict source.
pub(super) fn read_bounded_output_last_message(
    path: &Path,
    max_bytes: u64,
) -> std::io::Result<String> {
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
    let mut options = OpenOptions::new();
    options.read(true);
    let file = open_no_follow(&mut options, path)?;
    let opened_metadata = file.metadata()?;
    if opened_metadata.file_type().is_symlink() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("refusing to read symlinked output-last-message: {}", path.display()),
        ));
    }
    if !opened_metadata.file_type().is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("output-last-message is not a regular file: {}", path.display()),
        ));
    }
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
pub(super) fn initialize_output_last_message(path: &Path) -> std::io::Result<()> {
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

    let mut options = OpenOptions::new();
    options.write(true).create(true);
    let file = open_no_follow(&mut options, path)?;
    if !file.metadata()?.file_type().is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("output-last-message is not a regular file: {}", path.display()),
        ));
    }
    file.set_len(0)
}

pub(super) fn build_codex_reviewer_invocation(
    model: &str,
    effort: &str,
    resume_id: Option<&str>,
    prompt: &str,
    output_last_message: &Path,
    output_schema: &Path,
) -> Vec<std::ffi::OsString> {
    // All exec-level options must precede the `resume` subcommand: the documented
    // form is `codex exec [OPTIONS] resume [SESSION_ID] [PROMPT]`, and options
    // placed after `resume` are not guaranteed to bind to the run.
    let mut args = vec![
        "exec".into(),
        "--model".into(),
        model.into(),
        "--sandbox".into(),
        "read-only".into(),
        "--config".into(),
        format!("model_reasoning_effort=\"{effort}\"").into(),
        "--json".into(),
        "--output-schema".into(),
        output_schema.as_os_str().to_os_string(),
        "--output-last-message".into(),
        output_last_message.as_os_str().to_os_string(),
    ];
    if let Some(session_id) = resume_id {
        args.extend(["resume".into(), session_id.into()]);
    }
    args.push(prompt.into());
    args
}

pub(super) type SpawnCodexReviewerResult =
    Result<(Child, thread::JoinHandle<()>, thread::JoinHandle<Option<String>>), String>;

pub(super) fn spawn_codex_reviewer(
    bin: &std::ffi::OsStr,
    args: &[std::ffi::OsString],
    session_log_path: &Path,
    runtime: Option<&crate::codex_common::ResolvedCodexRuntime>,
) -> SpawnCodexReviewerResult {
    let mut command = Command::new(bin);
    command.args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    if let Some(runtime) = runtime {
        configure_codex_command(&mut command, runtime)?;
    }
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    let mut log = open_no_follow(&mut options, session_log_path).map_err(|error| {
        format!("failed to create session log {}: {error}", session_log_path.display())
    })?;
    if !log
        .metadata()
        .map_err(|error| {
            format!("failed to inspect session log {}: {error}", session_log_path.display())
        })?
        .file_type()
        .is_file()
    {
        return Err(format!(
            "refusing to create session log at non-regular file {}",
            session_log_path.display()
        ));
    }
    if let Some(runtime) = runtime {
        use std::io::Write as _;
        log.write_all(crate::codex_common::runtime_log_header(runtime).as_bytes()).map_err(
            |error| format!("failed to write session log {}: {error}", session_log_path.display()),
        )?;
    }
    let mut child = command
        .spawn()
        .map_err(|error| format!("failed to spawn {}: {error}", bin.to_string_lossy()))?;
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
pub(super) const MAX_CODEX_EVENT_BYTES: usize = 64 * 1024;

/// Drains Codex's JSON event stream while retaining only the first bounded `thread_id` event.
pub(super) fn collect_codex_session_id<R: Read>(pipe: R) -> Option<String> {
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
pub(super) fn terminate_reviewer_child(child: &mut Child) -> Result<(), String> {
    child.kill().map_err(|error| format!("failed to kill reviewer child: {error}"))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn test_open_no_follow_rejects_symlink_without_opening_target() {
        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("target.log");
        let link = directory.path().join("session.log");
        std::fs::write(&target, b"preserve this file").unwrap();
        std::os::unix::fs::symlink(&target, &link).unwrap();

        let mut options = OpenOptions::new();
        options.read(true);
        let result = open_no_follow(&mut options, &link);

        assert!(result.is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"preserve this file");
    }

    #[cfg(unix)]
    #[test]
    fn test_write_runtime_artifact_rejects_symlink_leaf_without_overwriting_target() {
        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("target.json");
        let link = directory.path().join("schema.json");
        std::fs::write(&target, b"preserve this schema").unwrap();
        std::os::unix::fs::symlink(&target, &link).unwrap();

        let result = write_runtime_artifact(&link, b"replacement");

        assert!(result.is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"preserve this schema");
    }

    #[cfg(unix)]
    #[test]
    fn test_write_runtime_artifact_rejects_symlink_parent_without_overwriting_target() {
        let directory = tempfile::tempdir().unwrap();
        let redirected_parent = directory.path().join("redirected");
        let link_parent = directory.path().join("runtime");
        std::fs::create_dir_all(&redirected_parent).unwrap();
        let target = redirected_parent.join("schema.json");
        std::fs::write(&target, b"preserve this schema").unwrap();
        std::os::unix::fs::symlink(&redirected_parent, &link_parent).unwrap();

        let result = write_runtime_artifact(&link_parent.join("schema.json"), b"replacement");

        assert!(result.is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"preserve this schema");
    }

    #[cfg(unix)]
    #[test]
    fn test_spawn_codex_reviewer_rejects_symlink_session_log_without_truncating_target() {
        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("target.log");
        let link = directory.path().join("session.log");
        std::fs::write(&target, b"preserve this file").unwrap();
        std::os::unix::fs::symlink(&target, &link).unwrap();

        let result = spawn_codex_reviewer(std::ffi::OsStr::new("true"), &[], &link, None);
        let error = match result {
            Ok(_) => panic!("symlinked session log must be rejected"),
            Err(error) => error,
        };

        assert!(error.contains("failed to create session log"));
        assert_eq!(std::fs::read(&target).unwrap(), b"preserve this file");
    }
}
