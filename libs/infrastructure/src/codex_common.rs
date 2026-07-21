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

use std::ffi::{OsStr, OsString};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use domain::tddd::test_obligation::ids::DiagnosticMessage;

use crate::git_cli::SystemGitRepo;

const REPO_LOCAL_CODEX_LINK: &str = ".harness/tools/bin/codex";
const CODEX_RUNTIME_PROBE_TIMEOUT: Duration = Duration::from_secs(5);
const CODEX_RUNTIME_PROBE_MAX_OUTPUT_BYTES: usize = 64 * 1024;

/// A Codex command resolved for one repository spawn.
///
/// The path prefix intentionally records the public launcher directory, rather
/// than the canonical package-internal executable directory.  The launcher
/// needs its colocated runtime to be discoverable by the sanitized child.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedCodexRuntime {
    executable: OsString,
    path_prefix: Option<PathBuf>,
    real_path: PathBuf,
    version: String,
}

impl ResolvedCodexRuntime {
    #[must_use]
    pub fn executable(&self) -> &OsStr {
        &self.executable
    }

    #[must_use]
    pub fn path_prefix(&self) -> Option<&Path> {
        self.path_prefix.as_deref()
    }

    #[must_use]
    pub fn real_path(&self) -> &Path {
        &self.real_path
    }

    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }
}

/// Explains why Codex cannot be resolved for a repository spawn.
#[derive(Debug)]
pub enum CodexRuntimeResolveError {
    ProjectRootInvalid(DiagnosticMessage),
    RepoLocalLinkInvalid(DiagnosticMessage),
    PathFallbackUnavailable(DiagnosticMessage),
    ProbeFailed(DiagnosticMessage),
}

impl std::fmt::Display for CodexRuntimeResolveError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let detail = match self {
            Self::ProjectRootInvalid(detail)
            | Self::RepoLocalLinkInvalid(detail)
            | Self::PathFallbackUnavailable(detail)
            | Self::ProbeFailed(detail) => detail,
        };
        write!(formatter, "{}", detail.as_str())
    }
}

impl std::error::Error for CodexRuntimeResolveError {}

/// Resolves the Codex runtime using the repository-local bootstrap link first.
///
/// A dangling or non-executable link is treated exactly like an absent link:
/// resolution falls back to the process PATH.  When neither source is usable,
/// the diagnostic tells the caller how to repair the repository-local runtime.
pub fn resolve_codex_runtime(
    project_root: &Path,
) -> Result<ResolvedCodexRuntime, CodexRuntimeResolveError> {
    if !project_root.is_dir() {
        return Err(CodexRuntimeResolveError::ProjectRootInvalid(diagnostic(format!(
            "project root is not a directory: {}",
            project_root.display()
        ))));
    }

    #[cfg(test)]
    if let Some(override_bin) = std::env::var_os(CODEX_BIN_ENV).filter(|value| !value.is_empty()) {
        return resolve_runtime_candidate(override_bin, None, "test override");
    }

    let link = project_root.join(REPO_LOCAL_CODEX_LINK);
    if let Ok(metadata) = std::fs::symlink_metadata(&link) {
        if metadata.file_type().is_symlink() {
            if let Ok(real_path) = link.canonicalize() {
                if is_executable(&real_path) {
                    let public_entry = std::fs::read_link(&link).map_err(|error| {
                        CodexRuntimeResolveError::RepoLocalLinkInvalid(diagnostic(format!(
                            "cannot read repository Codex link {}: {error}",
                            link.display()
                        )))
                    })?;
                    let public_entry = if public_entry.is_absolute() {
                        public_entry
                    } else {
                        link.parent().unwrap_or(project_root).join(public_entry)
                    };
                    let prefix = public_entry.parent().map(PathBuf::from);
                    // Execute the verified canonical target, never the mutable
                    // repository symlink. The public entry's parent remains the
                    // PATH prefix so launcher-side runtime discovery keeps its
                    // documented layout.
                    return resolve_runtime_candidate(
                        real_path.into_os_string(),
                        prefix,
                        "repository link",
                    );
                }
            }
        }
    }

    // PATH entries may be relative to the caller's working directory. Capture
    // that directory before locating and probing the candidate so the stored
    // runtime remains valid when a later spawn changes its working directory.
    let probe_directory = std::env::current_dir().map_err(|error| {
        CodexRuntimeResolveError::PathFallbackUnavailable(diagnostic(format!(
            "cannot determine current directory for PATH fallback: {error}; rerun `cargo make bootstrap`"
        )))
    })?;
    let fallback = find_on_path(Path::new("codex")).ok_or_else(|| {
        CodexRuntimeResolveError::PathFallbackUnavailable(diagnostic(
            "no usable repository Codex link or PATH fallback was found; rerun `cargo make bootstrap` to provision Codex".to_owned(),
        ))
    })?;
    resolve_path_fallback(fallback, &probe_directory)
}

/// Resolves Codex after discovering the repository root from the caller's
/// current working directory.
///
/// Codex's repository-local runtime link belongs at the git root, not at an
/// arbitrary subdirectory from which a CLI command was invoked.
pub(crate) fn resolve_codex_runtime_for_current_repository() -> Result<ResolvedCodexRuntime, String>
{
    let current_dir = std::env::current_dir()
        .map_err(|error| format!("failed to determine current directory: {error}"))?;
    resolve_codex_runtime_for_repository_start(&current_dir)
}

/// Resolves Codex after locating the git root that contains `start_dir`.
///
/// This is shared by Codex spawn paths and mirrors the dry-fix runner's
/// `SystemGitRepo`-based root discovery.
pub(crate) fn resolve_codex_runtime_for_repository_start(
    start_dir: &Path,
) -> Result<ResolvedCodexRuntime, String> {
    let repo = SystemGitRepo::discover_from(start_dir)
        .map_err(|error| format!("failed to discover git repository root: {error}"))?;
    resolve_codex_runtime(repo.root()).map_err(|error| error.to_string())
}

fn resolve_runtime_candidate(
    executable: OsString,
    path_prefix: Option<PathBuf>,
    source: &str,
) -> Result<ResolvedCodexRuntime, CodexRuntimeResolveError> {
    let real_path = Path::new(&executable).canonicalize().map_err(|error| {
        CodexRuntimeResolveError::RepoLocalLinkInvalid(diagnostic(format!(
            "cannot canonicalize Codex {source} {}: {error}",
            Path::new(&executable).display()
        )))
    })?;
    let mut command = Command::new(&executable);
    command.arg("--version");
    if let Some(prefix) = path_prefix.as_deref() {
        command.env(
            "PATH",
            prepend_dir_to_current_path(prefix)
                .map_err(|error| CodexRuntimeResolveError::ProbeFailed(diagnostic(error)))?,
        );
    }
    let output = crate::capability_exec::process::run_command_with_bounded_output(
        &mut command,
        CODEX_RUNTIME_PROBE_MAX_OUTPUT_BYTES,
        CODEX_RUNTIME_PROBE_TIMEOUT,
        "Codex runtime probe",
    )
    .map_err(|error| {
        CodexRuntimeResolveError::ProbeFailed(diagnostic(format!(
            "Codex {source} probe could not complete: {error}; rerun `cargo make bootstrap`"
        )))
    })?;
    if !output.status.success() {
        return Err(CodexRuntimeResolveError::ProbeFailed(diagnostic(format!(
            "Codex {source} probe exited with {}; rerun `cargo make bootstrap`",
            output.status
        ))));
    }
    let mut version = String::from_utf8_lossy(&output.stdout).into_owned();
    version.push_str(&String::from_utf8_lossy(&output.stderr));
    Ok(ResolvedCodexRuntime { executable, path_prefix, real_path, version })
}

fn resolve_path_fallback(
    fallback: PathBuf,
    probe_directory: &Path,
) -> Result<ResolvedCodexRuntime, CodexRuntimeResolveError> {
    let fallback = if fallback.is_absolute() { fallback } else { probe_directory.join(fallback) };
    let path_prefix = fallback.parent().map(PathBuf::from);
    resolve_runtime_candidate(fallback.into_os_string(), path_prefix, "PATH fallback")
}

fn find_on_path(executable: &Path) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|directory| directory.join(executable))
        .find(|candidate| is_executable(candidate))
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt as _;
    path.is_file()
        && std::fs::metadata(path)
            .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

fn diagnostic(message: String) -> DiagnosticMessage {
    let mut candidate = format!("Codex runtime: {message}");
    loop {
        match DiagnosticMessage::try_new(candidate) {
            Ok(diagnostic) => return diagnostic,
            Err(_) => candidate = "Codex runtime resolution failed".to_owned(),
        }
    }
}

pub(crate) fn prepend_dir_to_current_path(dir: &Path) -> Result<OsString, String> {
    let mut paths = vec![dir.to_path_buf()];
    if let Some(existing) = std::env::var_os("PATH") {
        if !existing.is_empty() {
            paths.extend(std::env::split_paths(&existing));
        }
    }
    std::env::join_paths(paths).map_err(|error| {
        format!("failed to prepend {} to Codex child PATH: {error}", dir.display())
    })
}

pub(crate) fn configure_codex_command(
    command: &mut Command,
    runtime: &ResolvedCodexRuntime,
) -> Result<(), String> {
    if let Some(prefix) = runtime.path_prefix() {
        command.env("PATH", prepend_dir_to_current_path(prefix)?);
    }
    Ok(())
}

/// Renders resolution diagnostics for persistent Codex session logs.
#[must_use]
pub(crate) fn runtime_log_header(runtime: &ResolvedCodexRuntime) -> String {
    format!(
        "resolved_real_path: {}\ncodex_version: {}\n",
        runtime.real_path().display(),
        runtime.version().trim_end()
    )
}

/// Polling interval for subprocess completion checks across all infrastructure adapters.
///
/// Both Codex and Claude adapters (`codex_dry_checker`, `codex_reviewer`, `claude_reviewer`)
/// share this value (ADR D4 / AC-05).
pub(crate) const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Runtime directory used by all infrastructure-layer reviewer and dry-check adapters.
///
/// Consolidates the four previously separate `"tmp/reviewer-runtime"` occurrences
/// (ADR D4 / AC-05 / T012).  Cross-layer usages in `apps/cli` and
/// `apps/cli-composition` are coincidental constants and remain in their
/// respective layers (CN-03 minimization; no cross-layer dep may be added).
pub(crate) const REVIEW_RUNTIME_DIR: &str = "tmp/reviewer-runtime";

/// Maximum stderr bytes retained in a persistent Codex session log.
///
/// Stderr is still drained and echoed after this limit, but no additional data
/// is written to the workspace log file.
pub(crate) const MAX_CODEX_SESSION_LOG_BYTES: u64 = 4 * 1024 * 1024;

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

/// Builds a timestamped, process-unique path inside `base_dir`.
///
/// Creates `base_dir` (and any missing ancestors) before returning.
///
/// # Arguments
/// - `base_dir`: Runtime directory constant (e.g. `REVIEW_RUNTIME_DIR`).
///   Callers pass the constant so the function remains independent of the
///   specific directory choice.
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
    runtime: Option<&ResolvedCodexRuntime>,
) -> Result<(Child, Vec<thread::JoinHandle<()>>), String> {
    let mut command = Command::new(bin);
    // Capture stdout instead of inheriting: the wrapper is the sole code path
    // that emits authoritative verdict JSON. Inherited stdout would let the
    // reviewer child leak verdict-like content before persistence succeeds,
    // breaking the fail-closed contract for unrecorded rounds.
    command.args(args).stdin(Stdio::null()).stdout(Stdio::piped());
    if let Some(runtime) = runtime {
        configure_codex_command(&mut command, runtime)?;
    }

    let mut log_file = std::fs::File::create(session_log_path)
        .map_err(|e| format!("failed to create session log {}: {e}", session_log_path.display()))?;
    if let Some(runtime) = runtime {
        log_file.write_all(runtime_log_header(runtime).as_bytes()).map_err(|e| {
            format!("failed to write session log {}: {e}", session_log_path.display())
        })?;
    }

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
    let mut buffer = [0_u8; 8 * 1024];
    drain_reader(pipe, &mut buffer);
}

/// Drains a reader in fixed-size byte chunks, discarding all content.
///
/// This intentionally avoids line-oriented reads: a newline-free child stream
/// must not cause the drain thread to allocate a buffer proportional to input.
fn drain_reader<R: Read>(mut reader: R, buffer: &mut [u8]) {
    loop {
        match reader.read(buffer) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
    }
}

/// Tees the child's stderr to a bounded log file while also printing it to the
/// current process's stderr.
pub fn tee_stderr_to_file(pipe: std::process::ChildStderr, log_file: std::fs::File) {
    tee_stderr_to_writer_with_limit(pipe, log_file, MAX_CODEX_SESSION_LOG_BYTES, true);
}

fn tee_stderr_to_writer_with_limit<R: Read, W: Write>(
    mut pipe: R,
    mut log_file: W,
    max_log_bytes: u64,
    echo_to_stderr: bool,
) {
    let mut retained = 0_u64;
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

        let write_len = max_log_bytes.saturating_sub(retained).min(read as u64) as usize;
        if write_len > 0 {
            let _ = log_file.write_all(bytes.get(..write_len).unwrap_or_default());
            retained = retained.saturating_add(write_len as u64);
        }
    }
    let _ = log_file.flush();
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn test_resolve_codex_runtime_prefers_repository_link_and_public_entry_parent() {
        use std::os::unix::fs::{PermissionsExt as _, symlink};

        let workspace = tempfile::tempdir().expect("workspace is created");
        let public_bin = workspace.path().join("public-bin");
        std::fs::create_dir_all(&public_bin).expect("public bin is created");
        let launcher = public_bin.join("codex");
        std::fs::write(&launcher, "#!/bin/sh\nprintf 'codex test-version\\n'\n")
            .expect("launcher is written");
        let mut permissions =
            std::fs::metadata(&launcher).expect("launcher metadata").permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&launcher, permissions).expect("launcher is executable");

        let link = workspace.path().join(REPO_LOCAL_CODEX_LINK);
        std::fs::create_dir_all(link.parent().expect("link has parent"))
            .expect("link dir is created");
        symlink(&launcher, &link).expect("repository link is created");

        let runtime = resolve_codex_runtime(workspace.path()).expect("runtime resolves from link");

        assert_eq!(runtime.executable(), launcher.canonicalize().expect("launcher canonical path"));
        assert_eq!(runtime.path_prefix(), Some(public_bin.as_path()));
        assert_eq!(runtime.real_path(), launcher.canonicalize().expect("launcher canonical path"));
        assert!(runtime.version().contains("codex test-version"));
    }

    #[cfg(unix)]
    #[test]
    fn test_resolve_codex_runtime_executes_verified_target_after_link_is_replaced() {
        use std::os::unix::fs::{PermissionsExt as _, symlink};

        let workspace = tempfile::tempdir().expect("workspace is created");
        let public_bin = workspace.path().join("public-bin");
        std::fs::create_dir_all(&public_bin).expect("public bin is created");
        let verified_launcher = public_bin.join("verified-codex");
        let replacement_launcher = public_bin.join("replacement-codex");
        std::fs::write(&verified_launcher, "#!/bin/sh\nprintf 'verified\\n'\n")
            .expect("verified launcher is written");
        std::fs::write(&replacement_launcher, "#!/bin/sh\nprintf 'replacement\\n'\n")
            .expect("replacement launcher is written");
        for launcher in [&verified_launcher, &replacement_launcher] {
            let mut permissions =
                std::fs::metadata(launcher).expect("launcher metadata").permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(launcher, permissions).expect("launcher is executable");
        }

        let link = workspace.path().join(REPO_LOCAL_CODEX_LINK);
        std::fs::create_dir_all(link.parent().expect("link has parent"))
            .expect("link dir is created");
        symlink(&verified_launcher, &link).expect("verified link is created");
        let runtime = resolve_codex_runtime(workspace.path()).expect("runtime resolves from link");

        std::fs::remove_file(&link).expect("verified link is removed");
        symlink(&replacement_launcher, &link).expect("replacement link is created");
        let output = Command::new(runtime.executable())
            .arg("--version")
            .output()
            .expect("verified target executes");

        assert!(output.status.success());
        assert_eq!(String::from_utf8_lossy(&output.stdout), "verified\n");
    }

    #[cfg(unix)]
    #[test]
    fn test_path_fallback_from_relative_entry_spawns_from_different_child_directory() {
        use std::os::unix::fs::PermissionsExt as _;

        let workspace = tempfile::tempdir().expect("workspace is created");
        let probe_directory = workspace.path().join("caller");
        let child_directory = workspace.path().join("repository");
        let launcher = probe_directory.join("node_modules/.bin/codex");
        std::fs::create_dir_all(launcher.parent().expect("launcher has parent"))
            .expect("launcher directory is created");
        std::fs::create_dir_all(&child_directory).expect("child directory is created");
        std::fs::write(&launcher, "#!/bin/sh\nprintf 'codex fallback-version\\n'\n")
            .expect("launcher is written");
        let mut permissions =
            std::fs::metadata(&launcher).expect("launcher metadata").permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&launcher, permissions).expect("launcher is executable");

        let runtime =
            resolve_path_fallback(PathBuf::from("node_modules/.bin/codex"), &probe_directory)
                .expect("relative fallback resolves");

        assert_eq!(runtime.executable(), launcher.as_os_str());
        assert_eq!(runtime.path_prefix(), launcher.parent());
        let mut command = Command::new(runtime.executable());
        command.arg("--version").current_dir(&child_directory);
        configure_codex_command(&mut command, &runtime).expect("runtime configures child");
        let output = command.output().expect("fallback executes from child directory");

        assert!(output.status.success());
        assert_eq!(String::from_utf8_lossy(&output.stdout), "codex fallback-version\n");
    }

    struct CountingReader {
        inner: std::io::Cursor<Vec<u8>>,
        read_calls: usize,
        bytes_read: usize,
    }

    impl CountingReader {
        fn new(bytes: Vec<u8>) -> Self {
            Self { inner: std::io::Cursor::new(bytes), read_calls: 0, bytes_read: 0 }
        }
    }

    impl Read for CountingReader {
        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            self.read_calls = self.read_calls.saturating_add(1);
            let read = self.inner.read(buffer)?;
            self.bytes_read = self.bytes_read.saturating_add(read);
            Ok(read)
        }
    }

    #[test]
    fn test_drain_reader_newline_free_stream_uses_fixed_chunks() {
        let mut reader = CountingReader::new(vec![b'x'; 10]);
        let mut small_buffer = [0_u8; 3];

        drain_reader(&mut reader, &mut small_buffer);

        assert_eq!(reader.bytes_read, 10);
        assert_eq!(reader.read_calls, 5);
    }

    #[test]
    fn test_tee_stderr_to_writer_with_limit_over_cap_retains_prefix_and_drains() {
        let mut session_log = Vec::new();

        tee_stderr_to_writer_with_limit(
            std::io::Cursor::new(b"0123456789"),
            &mut session_log,
            4,
            false,
        );

        assert_eq!(session_log, b"0123");
    }
}
