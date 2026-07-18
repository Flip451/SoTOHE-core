use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::codex_common::REVIEW_RUNTIME_DIR;

use super::dry_fix_exit_detail;
use super::session_log::{dry_fix_redaction_values, redact_dry_fix_sensitive_text};

const DRY_FIX_PIPE_READ_BUF_BYTES: usize = 8192;
const DRY_FIX_CAPTURE_LIMIT_BYTES: usize = 1024 * 1024;

pub(super) struct DryFixRuntimeFile {
    path: PathBuf,
    parent: File,
    name: OsString,
}

impl DryFixRuntimeFile {
    pub(super) fn path(&self) -> &Path {
        &self.path
    }
}

pub(super) fn dry_fix_runtime_file(
    trusted_root: &Path,
    prefix: &str,
    ext: &str,
) -> Result<DryFixRuntimeFile, String> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| format!("failed to compute timestamp: {e}"))?
        .as_nanos();
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    let root = trusted_root.canonicalize().map_err(|e| {
        format!("failed to canonicalize trusted repository root {}: {e}", trusted_root.display())
    })?;
    let parent = open_or_create_dry_fix_runtime_directory(&root)?;
    let name = OsString::from(format!("{prefix}-{}-{timestamp}-{seq}.{ext}", std::process::id()));
    Ok(DryFixRuntimeFile { path: root.join(REVIEW_RUNTIME_DIR).join(&name), parent, name })
}

pub(super) fn dry_fix_initialize_runtime_file(file: &DryFixRuntimeFile) -> Result<(), String> {
    let mut leaf = rustix::fs::openat(
        &file.parent,
        &file.name,
        rustix::fs::OFlags::WRONLY
            | rustix::fs::OFlags::CREATE
            | rustix::fs::OFlags::EXCL
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::from_raw_mode(0o600),
    )
    .map(File::from)
    .map_err(|e| format!("failed to initialize runtime file {}: {e}", file.path.display()))?;
    leaf.write_all(b"")
        .map_err(|e| format!("failed to initialize runtime file {}: {e}", file.path.display()))
}

fn open_or_create_dry_fix_runtime_directory(root: &Path) -> Result<File, String> {
    let mut directory = open_dry_fix_directory_nofollow(root)
        .map_err(|e| format!("failed to open trusted repository root {}: {e}", root.display()))?;
    for component in Path::new(REVIEW_RUNTIME_DIR).components() {
        let std::path::Component::Normal(name) = component else {
            return Err("runtime directory contains an invalid component".to_owned());
        };
        directory = open_or_create_dry_fix_directory_at(&directory, name).map_err(|error| {
            let detail =
                match rustix::fs::statat(&directory, name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW) {
                    Ok(metadata)
                        if rustix::fs::FileType::from_raw_mode(metadata.st_mode).is_symlink() =>
                    {
                        "refusing to follow symlink".to_owned()
                    }
                    _ => error.to_string(),
                };
            format!(
                "failed to open runtime directory component {}: {detail}",
                name.to_string_lossy()
            )
        })?;
    }
    Ok(directory)
}

fn open_or_create_dry_fix_directory_at(
    parent: &File,
    name: &OsStr,
) -> Result<File, std::io::Error> {
    match open_dry_fix_directory_at_nofollow(parent, name) {
        Ok(directory) => Ok(directory),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            match rustix::fs::mkdirat(parent, name, rustix::fs::Mode::from_raw_mode(0o700)) {
                Ok(()) | Err(rustix::io::Errno::EXIST) => {}
                Err(error) => return Err(error.into()),
            }
            open_dry_fix_directory_at_nofollow(parent, name)
        }
        Err(error) => Err(error),
    }
}

fn open_dry_fix_directory_nofollow(path: &Path) -> Result<File, std::io::Error> {
    rustix::fs::open(
        path,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::DIRECTORY
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .map(File::from)
    .map_err(Into::into)
}

fn open_dry_fix_directory_at_nofollow(parent: &File, name: &OsStr) -> Result<File, std::io::Error> {
    rustix::fs::openat(
        parent,
        name,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::DIRECTORY
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .map(File::from)
    .map_err(Into::into)
}

pub(super) fn dry_fix_spawn_and_collect(
    bin: &OsString,
    args: &[OsString],
    safe_env: &[(OsString, OsString)],
    prompt: &str,
    trusted_root: &Path,
    runtime: Option<&crate::codex_common::ResolvedCodexRuntime>,
) -> Result<(String, ExitStatus, PathBuf), String> {
    let log_file = dry_fix_runtime_file(trusted_root, "dry-fix-codex-session", "log")?;
    dry_fix_initialize_runtime_file(&log_file)?;
    let log_path = log_file.path().to_path_buf();
    let mut command = Command::new(bin);
    command.args(args);
    command.env_clear();
    for (k, v) in safe_env {
        command.env(k, v);
    }
    command.stdin(Stdio::piped());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|e| format!("failed to spawn Codex fixer: {e}"))?;
    let redactions = dry_fix_redaction_values(safe_env);
    let stdout_pipe = child.stdout.take();
    let stdout_redactions = redactions.clone();
    let stdout_handle = thread::spawn(move || collect_pipe(stdout_pipe, false, &stdout_redactions));
    let stderr_pipe = child.stderr.take();
    let stderr_handle = thread::spawn(move || collect_pipe(stderr_pipe, true, &redactions));
    let prompt_result = match child.stdin.take() {
        Some(mut stdin) => stdin
            .write_all(prompt.as_bytes())
            .map_err(|e| format!("failed to write prompt to Codex fixer stdin: {e}")),
        None => Err("failed to open Codex fixer stdin pipe".to_owned()),
    };
    if let Err(msg) = prompt_result {
        let _ = child.kill();
        let stdout = stdout_handle.join().ok().and_then(|r| r.ok()).unwrap_or_default();
        let stderr = stderr_handle.join().ok().and_then(|r| r.ok()).unwrap_or_default();
        let exit_status = match child.wait() {
            Ok(status) => status,
            Err(error) => {
                let detail = format!("failed to wait after prompt write error: {error}");
                write_dry_fix_log_at(&log_file, bin, &detail, &stdout, &stderr, runtime);
                return Err(format!("{msg}; {detail}; log: {}", log_path.display()));
            }
        };
        write_dry_fix_log_at(&log_file, bin, &exit_status.to_string(), &stdout, &stderr, runtime);
        return Err(format!(
            "{msg}; Codex fixer {}; log: {}",
            dry_fix_exit_detail(exit_status),
            log_path.display()
        ));
    }
    let exit_status = child.wait().map_err(|e| format!("failed to wait for Codex fixer: {e}"))?;
    let status_str = exit_status.to_string();
    let (stdout, stdout_error) =
        dry_fix_collector_result_for_log(join_dry_fix_collector(stdout_handle, "stdout"), "stdout");
    let (stderr, stderr_error) =
        dry_fix_collector_result_for_log(join_dry_fix_collector(stderr_handle, "stderr"), "stderr");
    write_dry_fix_log_at(&log_file, bin, &status_str, &stdout, &stderr, runtime);
    if let Some(error) = stdout_error.or(stderr_error) {
        return Err(format!(
            "{error}; Codex fixer {}; log: {}",
            dry_fix_exit_detail(exit_status),
            log_path.display()
        ));
    }
    Ok((stdout, exit_status, log_path))
}

fn write_dry_fix_log_at(
    log_file: &DryFixRuntimeFile,
    bin: &OsString,
    status: &str,
    stdout: &str,
    stderr: &str,
    runtime: Option<&crate::codex_common::ResolvedCodexRuntime>,
) {
    let file = rustix::fs::openat(
        &log_file.parent,
        &log_file.name,
        rustix::fs::OFlags::WRONLY
            | rustix::fs::OFlags::TRUNC
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::NONBLOCK
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .map(File::from);
    let Ok(mut file) = file else {
        return;
    };
    let Ok(metadata) = file.metadata() else {
        return;
    };
    if !metadata.is_file() {
        return;
    }
    let runtime_header = runtime.map(crate::codex_common::runtime_log_header).unwrap_or_default();
    let content = format!(
        "bin: {}\n{}status: {}\n--- stdout ---\n{}\n--- stderr ---\n{}",
        bin.to_string_lossy(),
        runtime_header,
        status,
        stdout,
        stderr
    );
    let _ = file.write_all(content.as_bytes());
}

fn dry_fix_collector_result_for_log(
    result: Result<String, String>,
    stream_name: &str,
) -> (String, Option<String>) {
    match result {
        Ok(output) => (output, None),
        Err(error) => (format!("[failed to collect {stream_name}: {error}]\n"), Some(error)),
    }
}

fn join_dry_fix_collector(
    handle: std::thread::JoinHandle<Result<String, String>>,
    stream_name: &str,
) -> Result<String, String> {
    handle
        .join()
        .map_err(|_| format!("{stream_name} collector thread panicked"))?
        .map_err(|e| format!("{stream_name} collection error: {e}"))
}

fn collect_pipe<R: std::io::Read>(
    pipe: Option<R>,
    echo_to_stderr: bool,
    redactions: &[(String, String)],
) -> Result<String, String> {
    let mut collected = BoundedTextCapture::new(DRY_FIX_CAPTURE_LIMIT_BYTES);
    if let Some(mut pipe) = pipe {
        let mut read_buf = [0_u8; DRY_FIX_PIPE_READ_BUF_BYTES];
        let mut redactor = StreamingRedactor::new(redactions, echo_to_stderr);

        loop {
            let read = pipe
                .read(&mut read_buf)
                .map_err(|e| format!("failed to read Codex fixer output: {e}"))?;
            if read == 0 {
                break;
            }
            let chunk = read_buf
                .get(..read)
                .ok_or_else(|| "Codex fixer output read exceeded buffer".to_owned())?;
            redactor.push(&String::from_utf8_lossy(chunk), &mut collected);
        }

        redactor.finish(&mut collected);
    }
    Ok(collected.into_string())
}

struct RedactionMatch {
    start: usize,
    len: usize,
    placeholder: String,
}

struct StreamingRedactor<'a> {
    redactions: &'a [(String, String)],
    echo_to_stderr: bool,
    pending: String,
    max_secret_len: usize,
}

impl<'a> StreamingRedactor<'a> {
    fn new(redactions: &'a [(String, String)], echo_to_stderr: bool) -> Self {
        Self {
            redactions,
            echo_to_stderr,
            pending: String::new(),
            max_secret_len: max_redaction_secret_len(redactions),
        }
    }

    fn push(&mut self, text: &str, collected: &mut BoundedTextCapture) {
        self.pending.push_str(text);
        self.drain(false, collected);
    }

    fn finish(&mut self, collected: &mut BoundedTextCapture) {
        self.drain(true, collected);
    }

    fn drain(&mut self, final_chunk: bool, collected: &mut BoundedTextCapture) {
        loop {
            let keep_suffix = if final_chunk { 0 } else { self.max_secret_len.saturating_sub(1) };
            if self.pending.len() <= keep_suffix {
                break;
            }

            let safe_len = previous_char_boundary(&self.pending, self.pending.len() - keep_suffix);
            if safe_len == 0 {
                break;
            }

            match earliest_redaction_match(&self.pending, self.redactions) {
                Some(redaction) if redaction.start < safe_len => {
                    let prefix = self.take_prefix(redaction.start);
                    self.emit(&prefix, collected);
                    let _secret = self.take_prefix(redaction.len);
                    self.emit(&redaction.placeholder, collected);
                }
                _ => {
                    let prefix = self.take_prefix(safe_len);
                    self.emit(&prefix, collected);
                }
            }
        }

        if final_chunk && !self.pending.is_empty() {
            if let Some(redaction) = earliest_redaction_match(&self.pending, self.redactions) {
                let prefix = self.take_prefix(redaction.start);
                self.emit(&prefix, collected);
                let _secret = self.take_prefix(redaction.len);
                self.emit(&redaction.placeholder, collected);
                self.drain(true, collected);
            } else {
                let rest = std::mem::take(&mut self.pending);
                self.emit(&rest, collected);
            }
        }
    }

    fn take_prefix(&mut self, len: usize) -> String {
        let take_len = previous_char_boundary(&self.pending, len);
        let prefix = self.pending.get(..take_len).unwrap_or_default().to_owned();
        self.pending = self.pending.get(take_len..).unwrap_or_default().to_owned();
        prefix
    }

    fn emit(&self, text: &str, collected: &mut BoundedTextCapture) {
        let text = redact_dry_fix_sensitive_text(text, self.redactions);
        if self.echo_to_stderr {
            eprint!("{text}");
        }
        collected.push_str(&text);
    }
}

fn max_redaction_secret_len(redactions: &[(String, String)]) -> usize {
    redactions.iter().map(|(_, secret)| secret.len()).max().unwrap_or(0)
}

fn earliest_redaction_match(text: &str, redactions: &[(String, String)]) -> Option<RedactionMatch> {
    redactions
        .iter()
        .filter(|(_, secret)| !secret.is_empty())
        .filter_map(|(var, secret)| {
            text.find(secret).map(|start| RedactionMatch {
                start,
                len: secret.len(),
                placeholder: format!("[REDACTED:{var}]"),
            })
        })
        .min_by(|a, b| a.start.cmp(&b.start).then_with(|| b.len.cmp(&a.len)))
}

struct BoundedTextCapture {
    limit: usize,
    text: String,
    dropped_bytes: usize,
}

impl BoundedTextCapture {
    fn new(limit: usize) -> Self {
        Self { limit, text: String::new(), dropped_bytes: 0 }
    }

    fn push_str(&mut self, text: &str) {
        if text.is_empty() || self.limit == 0 {
            self.dropped_bytes = self.dropped_bytes.saturating_add(text.len());
            return;
        }

        if text.len() >= self.limit {
            self.dropped_bytes = self
                .dropped_bytes
                .saturating_add(self.text.len())
                .saturating_add(text.len().saturating_sub(self.limit));
            self.text.clear();
            let start = next_char_boundary(text, text.len() - self.limit);
            if let Some(tail) = text.get(start..) {
                self.text.push_str(tail);
            }
            return;
        }

        let overflow = self.text.len().saturating_add(text.len()).saturating_sub(self.limit);
        if overflow > 0 {
            let drop_len = next_char_boundary(&self.text, overflow);
            self.dropped_bytes = self.dropped_bytes.saturating_add(drop_len);
            self.text = self.text.get(drop_len..).unwrap_or_default().to_owned();
        }
        self.text.push_str(text);
    }

    fn into_string(self) -> String {
        if self.dropped_bytes == 0 {
            return self.text;
        }
        format!(
            "[dry-fix output truncated: dropped {} bytes; kept last {} bytes]\n{}",
            self.dropped_bytes,
            self.text.len(),
            self.text
        )
    }
}

fn next_char_boundary(text: &str, min_index: usize) -> usize {
    let mut idx = min_index.min(text.len());
    while idx < text.len() && !text.is_char_boundary(idx) {
        idx += 1;
    }
    idx
}

fn previous_char_boundary(text: &str, max_index: usize) -> usize {
    let mut idx = max_index.min(text.len());
    while idx > 0 && !text.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use std::path::Path;

    #[cfg(unix)]
    fn make_executable(script: &Path) {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(script).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(script, perms).unwrap();
    }

    #[cfg(unix)]
    fn write_fake_codex_runner(dir: &Path, body: &str) -> PathBuf {
        let script_content = format!(
            "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then echo \"codex 0.125.0\"; exit 0; fi\n{body}"
        );
        let script = dir.join("fake-codex.sh");
        std::fs::write(&script, script_content).unwrap();
        make_executable(&script);
        script
    }

    #[cfg(unix)]
    #[test]
    fn test_dry_fix_spawn_and_collect_redacts_sensitive_values_from_output_and_log() {
        let dir = tempfile::tempdir().unwrap();
        let short_secret = "sk-dry-fix";
        let long_secret = "sk-dry-fix-secret";
        let org_id = "org-dry-fix";
        let base_url = "https://token@example.invalid/v1";
        let fake_codex = write_fake_codex_runner(
            dir.path(),
            &format!(
                "while IFS= read -r _line; do :; done\nprintf 'stdout {short_secret} {long_secret} {org_id} {base_url}\\n'\nprintf 'stderr {short_secret} {long_secret} {org_id} {base_url}\\n' >&2\nexit 0\n"
            ),
        );
        let safe_env = vec![
            (OsString::from("OPENAI_API_KEY"), OsString::from(short_secret)),
            (OsString::from("CODEX_API_KEY"), OsString::from(long_secret)),
            (OsString::from("OPENAI_ORG_ID"), OsString::from(org_id)),
            (OsString::from("OPENAI_BASE_URL"), OsString::from(base_url)),
        ];

        let (stdout, status, log_path) = dry_fix_spawn_and_collect(
            &fake_codex.as_os_str().to_os_string(),
            &[],
            &safe_env,
            "prompt",
            dir.path(),
            None,
        )
        .unwrap();
        let log = std::fs::read_to_string(log_path).unwrap();

        assert!(status.success());

        for secret in [short_secret, long_secret, org_id, base_url] {
            assert!(!stdout.contains(secret), "stdout must redact {secret}");
            assert!(!log.contains(secret), "session log must redact {secret}");
        }
        assert!(!stdout.contains("-secret"), "overlapping secret suffix must not leak");
        assert!(!log.contains("-secret"), "overlapping secret suffix must not leak");
        assert!(stdout.contains("[REDACTED:OPENAI_API_KEY]"));
        assert!(stdout.contains("[REDACTED:CODEX_API_KEY]"));
        assert!(stdout.contains("[REDACTED:OPENAI_ORG_ID]"));
        assert!(stdout.contains("[REDACTED:OPENAI_BASE_URL]"));
        assert!(log.contains("[REDACTED:OPENAI_API_KEY]"));
        assert!(log.contains("[REDACTED:CODEX_API_KEY]"));
        assert!(log.contains("[REDACTED:OPENAI_ORG_ID]"));
        assert!(log.contains("[REDACTED:OPENAI_BASE_URL]"));
    }

    #[cfg(unix)]
    #[test]
    fn test_dry_fix_runtime_file_rejects_symlinked_runtime_parent() {
        let repository = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), repository.path().join("tmp")).unwrap();

        let error = match dry_fix_runtime_file(repository.path(), "last-message", "txt") {
            Ok(_) => panic!("a symlinked runtime parent must be rejected"),
            Err(error) => error,
        };

        assert!(error.contains("refusing to follow symlink"), "got: {error}");
    }

    #[cfg(unix)]
    #[test]
    fn test_write_dry_fix_log_at_does_not_follow_replaced_symlink() {
        let repository = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let log_file = dry_fix_runtime_file(repository.path(), "session", "log").unwrap();
        dry_fix_initialize_runtime_file(&log_file).unwrap();
        let outside_file = outside.path().join("outside.log");
        std::fs::write(&outside_file, "outside content").unwrap();
        std::fs::remove_file(log_file.path()).unwrap();
        std::os::unix::fs::symlink(&outside_file, log_file.path()).unwrap();

        write_dry_fix_log_at(
            &log_file,
            &OsString::from("codex"),
            "exit status: 1",
            "child output",
            "child error",
            None,
        );

        assert_eq!(std::fs::read_to_string(&outside_file).unwrap(), "outside content");
    }

    #[test]
    fn test_collect_pipe_truncates_unbounded_output_but_keeps_tail() {
        let mut output = vec![b'x'; DRY_FIX_CAPTURE_LIMIT_BYTES + 128];
        output.extend_from_slice(b"\nDRY_FIX_STATUS: completed\n");

        let collected = collect_pipe(Some(std::io::Cursor::new(output)), false, &[]).unwrap();

        assert!(collected.contains("dry-fix output truncated"));
        assert!(collected.contains("DRY_FIX_STATUS: completed"));
        assert!(
            collected.len() <= DRY_FIX_CAPTURE_LIMIT_BYTES + 128,
            "bounded capture must not grow with subprocess output"
        );
    }
}
