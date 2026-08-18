use std::ffi::OsString;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use usecase::review_v2::run_review_fix::ReviewFixRunnerError;

use crate::codex_common::REVIEW_RUNTIME_DIR;

use super::launch_context::{
    FIXER_RUNTIME_TIMEOUT, TrustedLaunchContext, receive_child_output_collector,
    spawn_child_output_tail_collector, wait_for_child_with_timeout,
};
use super::session_log::write_session_log;

pub(super) struct RuntimeFile {
    pub(super) path: PathBuf,
    pub(super) directory: File,
    pub(super) name: OsString,
    pub(super) file: File,
}

impl RuntimeFile {
    pub(super) fn path(&self) -> &Path {
        &self.path
    }

    pub(super) fn remove(&self) {
        let _ = rustix::fs::unlinkat(&self.directory, &self.name, rustix::fs::AtFlags::empty());
    }

    pub(super) fn verify_path_identity(&self) -> std::io::Result<()> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt as _;

            let replacement = rustix::fs::openat(
                &self.directory,
                &self.name,
                rustix::fs::OFlags::RDONLY
                    | rustix::fs::OFlags::NONBLOCK
                    | rustix::fs::OFlags::NOFOLLOW
                    | rustix::fs::OFlags::CLOEXEC,
                rustix::fs::Mode::empty(),
            )
            .map(File::from)
            .map_err(std::io::Error::from)?;
            let expected = self.file.metadata()?;
            let actual = replacement.metadata()?;
            if !actual.is_file() || expected.dev() != actual.dev() || expected.ino() != actual.ino()
            {
                return Err(std::io::Error::other(
                    "runtime session-log path no longer identifies its retained regular file",
                ));
            }
            Ok(())
        }
        #[cfg(not(unix))]
        {
            // The session log is written through the retained handle, never by
            // reopening this path. Non-Unix platforms do not expose the Unix
            // device/inode comparison above, so the held handle remains the
            // safe identity boundary for the write.
            Ok(())
        }
    }
}

pub(super) fn create_runtime_file(
    repository_root: &Path,
    prefix: &str,
    ext: &str,
) -> Result<RuntimeFile, ReviewFixRunnerError> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| {
            ReviewFixRunnerError::Unexpected(usecase::git_workflow::DiagnosticText::new(format!(
                "failed to compute timestamp: {e}"
            )))
        })?
        .as_nanos();
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    let root = repository_root.canonicalize().map_err(|error| {
        ReviewFixRunnerError::Unexpected(usecase::git_workflow::DiagnosticText::new(format!(
            "failed to canonicalize repository root {}: {error}",
            repository_root.display()
        )))
    })?;
    let mut directory = open_directory_nofollow(&root)?;
    let runtime = Path::new(REVIEW_RUNTIME_DIR);
    for component in runtime.components() {
        let std::path::Component::Normal(name) = component else {
            return Err(runtime_error("runtime directory contains an invalid component"));
        };
        directory = open_or_create_directory(&directory, name)?;
    }
    let name = OsString::from(format!("{prefix}-{}-{timestamp}-{seq}.{ext}", std::process::id()));
    let path = root.join(runtime).join(&name);
    let file = rustix::fs::openat(
        &directory,
        &name,
        rustix::fs::OFlags::WRONLY
            | rustix::fs::OFlags::CREATE
            | rustix::fs::OFlags::EXCL
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::from_raw_mode(0o600),
    )
    .map(File::from)
    .map_err(|error| runtime_error(format!("failed to create {}: {error}", path.display())))?;
    if !file.metadata().map_err(|error| runtime_error(error.to_string()))?.is_file() {
        return Err(runtime_error(format!("runtime file is not regular: {}", path.display())));
    }
    Ok(RuntimeFile { path, directory, name, file })
}

pub(super) fn read_runtime_file_bounded(
    file: &RuntimeFile,
    maximum_bytes: u64,
) -> Result<String, ReviewFixRunnerError> {
    let mut opened = rustix::fs::openat(
        &file.directory,
        &file.name,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::NONBLOCK
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .map(File::from)
    .map_err(|error| runtime_error(format!("failed to open {}: {error}", file.path.display())))?;
    let metadata = opened.metadata().map_err(|error| {
        runtime_error(format!("failed to inspect {}: {error}", file.path.display()))
    })?;
    if !metadata.is_file() {
        return Err(runtime_error(format!("runtime file is not regular: {}", file.path.display())));
    }
    if metadata.len() > maximum_bytes {
        return Err(runtime_error(format!(
            "runtime file {} exceeds {maximum_bytes} bytes",
            file.path.display()
        )));
    }
    let mut content = String::new();
    Read::by_ref(&mut opened)
        .take(maximum_bytes.saturating_add(1))
        .read_to_string(&mut content)
        .map_err(|error| {
            runtime_error(format!("failed to read {}: {error}", file.path.display()))
        })?;
    if content.len() as u64 > maximum_bytes {
        return Err(runtime_error(format!(
            "runtime file {} exceeds {maximum_bytes} bytes",
            file.path.display()
        )));
    }
    Ok(content)
}

fn open_directory_nofollow(path: &Path) -> Result<File, ReviewFixRunnerError> {
    rustix::fs::open(
        path,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::DIRECTORY
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .map(File::from)
    .map_err(|error| runtime_error(format!("failed to open {}: {error}", path.display())))
}

fn open_or_create_directory(
    parent: &File,
    name: &std::ffi::OsStr,
) -> Result<File, ReviewFixRunnerError> {
    let open = || {
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
    };
    match open() {
        Ok(directory) => Ok(directory),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            match rustix::fs::mkdirat(parent, name, rustix::fs::Mode::from_raw_mode(0o700)) {
                Ok(()) | Err(rustix::io::Errno::EXIST) => {}
                Err(error) => return Err(runtime_error(error.to_string())),
            }
            open().map_err(|error| {
                runtime_error(format!("failed to open runtime directory: {error}"))
            })
        }
        Err(error) => Err(runtime_error(format!("failed to open runtime directory: {error}"))),
    }
}

fn runtime_error(detail: impl Into<String>) -> ReviewFixRunnerError {
    ReviewFixRunnerError::Unexpected(usecase::git_workflow::DiagnosticText::new(detail.into()))
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod platform_tests {
    use super::create_runtime_file;

    #[test]
    fn test_verify_path_identity_retains_created_runtime_file() {
        let repository = tempfile::tempdir().expect("repository fixture");
        let runtime_file = create_runtime_file(repository.path(), "review-fix-test", "txt")
            .expect("create runtime file");

        runtime_file
            .verify_path_identity()
            .expect("created runtime file must retain a usable identity");
    }
}

#[cfg(all(test, unix))]
#[allow(clippy::expect_used)]
mod tests {
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    use super::{create_runtime_file, write_prompt_before_deadline};

    #[test]
    fn test_create_runtime_file_rejects_symlinked_tmp_directory() {
        let repository = tempfile::tempdir().expect("repository fixture");
        let outside = tempfile::tempdir().expect("outside fixture");
        std::os::unix::fs::symlink(outside.path(), repository.path().join("tmp"))
            .expect("tmp symlink");

        let result = create_runtime_file(repository.path(), "review-fix-test", "txt");

        assert!(result.is_err(), "symlinked tmp must be rejected");
    }

    #[test]
    fn test_write_prompt_deadline_detaches_a_potentially_blocked_writer() {
        let mut child = Command::new("sh")
            .args(["-c", "exec sleep 60"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("stdin-holding child fixture");
        let started = Instant::now();

        let result = write_prompt_before_deadline(
            child.stdin.take(),
            vec![b'x'; 16 * 1024 * 1024],
            Instant::now(),
        );

        assert!(result.is_err(), "expired deadline must reject the prompt write");
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "a blocked stdin writer must not delay the deadline failure"
        );
        child.kill().expect("terminate child fixture");
        child.wait().expect("reap child fixture");
    }
}

pub(super) fn spawn_and_collect_codex(
    bin: &std::ffi::OsStr,
    args: &[OsString],
    safe_env: &[(OsString, OsString)],
    prompt: &str,
    launch_context: &TrustedLaunchContext,
    runtime: Option<&crate::codex_common::ResolvedCodexRuntime>,
) -> Result<(ExitStatus, RuntimeFile), ReviewFixRunnerError> {
    let mut log_file = launch_context.create_runtime_file("review-fix-codex-session", "log")?;
    let mut command = Command::new(bin);
    command.args(args);
    command.current_dir(&launch_context.repository_root);
    command.env_clear();
    for (k, v) in safe_env {
        command.env(k, v);
    }
    command.stdin(Stdio::piped());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    crate::capability_exec::process::configure_process_group(&mut command);
    let mut child = command.spawn().map_err(|e| {
        ReviewFixRunnerError::SpawnFailed(usecase::git_workflow::DiagnosticText::new(format!(
            "failed to spawn codex fixer: {e}"
        )))
    })?;
    let process_id = child.id();
    let stdout_pipe = child.stdout.take();
    let stdout_collector = spawn_child_output_tail_collector(stdout_pipe, false, "stdout");
    let stderr_pipe = child.stderr.take();
    let stderr_collector = spawn_child_output_tail_collector(stderr_pipe, true, "stderr");
    let deadline = Instant::now().checked_add(FIXER_RUNTIME_TIMEOUT).unwrap_or_else(Instant::now);
    let prompt_write_result =
        write_prompt_before_deadline(child.stdin.take(), prompt.as_bytes().to_vec(), deadline);
    if let Err(message) = prompt_write_result {
        let exit_status = wait_for_child_with_timeout(&mut child, Duration::ZERO, "codex fixer")
            .map_or_else(|error| error, |status| status.to_string());
        let (stdout, _) = collector_result_for_log(
            receive_child_output_collector(stdout_collector, process_id, "codex fixer", "stdout"),
            "stdout",
        );
        let (stderr, _) = collector_result_for_log(
            receive_child_output_collector(stderr_collector, process_id, "codex fixer", "stderr"),
            "stderr",
        );
        if let Err(log_error) =
            write_session_log(&mut log_file, bin, &exit_status, &stdout, &stderr, runtime)
        {
            return Err(ReviewFixRunnerError::SpawnFailed(
                usecase::git_workflow::DiagnosticText::new(format!(
                    "{message}; additionally failed to write session log: {log_error}"
                )),
            ));
        }
        return Err(ReviewFixRunnerError::SpawnFailed(usecase::git_workflow::DiagnosticText::new(
            format!("{message}; session log: {}", log_file.path().display()),
        )));
    }
    let remaining = deadline.saturating_duration_since(Instant::now());
    let exit_status = wait_for_child_with_timeout(&mut child, remaining, "codex fixer");
    let (stdout, stdout_error) = collector_result_for_log(
        receive_child_output_collector(stdout_collector, process_id, "codex fixer", "stdout"),
        "stdout",
    );
    let (stderr, stderr_error) = collector_result_for_log(
        receive_child_output_collector(stderr_collector, process_id, "codex fixer", "stderr"),
        "stderr",
    );
    let exit_status_text = exit_status
        .as_ref()
        .map_or_else(|message| message.clone(), std::string::ToString::to_string);
    let log_result =
        write_session_log(&mut log_file, bin, &exit_status_text, &stdout, &stderr, runtime);
    let exit_status = match (exit_status, log_result) {
        (Ok(status), Ok(())) => status,
        (Ok(_), Err(log_error)) => return Err(log_error),
        (Err(message), Ok(())) => {
            return Err(ReviewFixRunnerError::SpawnFailed(
                usecase::git_workflow::DiagnosticText::new(format!(
                    "{message}; session log: {}",
                    log_file.path().display()
                )),
            ));
        }
        (Err(message), Err(log_error)) => {
            return Err(ReviewFixRunnerError::SpawnFailed(
                usecase::git_workflow::DiagnosticText::new(format!(
                    "{message}; additionally failed to write session log: {log_error}"
                )),
            ));
        }
    };
    if let Some(error) = stdout_error.or(stderr_error) {
        return Err(ReviewFixRunnerError::Unexpected(usecase::git_workflow::DiagnosticText::new(
            format!("{error}; session log: {}", log_file.path().display()),
        )));
    }
    Ok((exit_status, log_file))
}

fn write_prompt_before_deadline(
    stdin: Option<std::process::ChildStdin>,
    prompt: Vec<u8>,
    deadline: Instant,
) -> Result<(), String> {
    let (sender, receiver) = mpsc::sync_channel(1);
    let writer = thread::spawn(move || {
        let result = match stdin {
            Some(mut stdin) => stdin
                .write_all(&prompt)
                .map_err(|error| format!("failed to write prompt to codex fixer stdin: {error}")),
            None => Err("failed to open codex fixer stdin pipe".to_owned()),
        };
        let _ = sender.send(result);
    });
    let result = loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break Err(format!(
                "codex fixer exceeded its {}-second runtime limit while writing the prompt",
                FIXER_RUNTIME_TIMEOUT.as_secs()
            ));
        }
        match receiver.recv_timeout(remaining.min(Duration::from_millis(10))) {
            Ok(result) => break result,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                break Err("codex fixer prompt writer terminated unexpectedly".to_owned());
            }
        }
    };
    // A process-group termination can fall back to killing only the direct
    // child, leaving a descendant holding stdin open. Never join this writer:
    // dropping its handle detaches a blocked write and preserves the deadline.
    drop(writer);
    result
}

pub(super) fn collector_result_for_log(
    result: Result<String, ReviewFixRunnerError>,
    stream_name: &str,
) -> (String, Option<ReviewFixRunnerError>) {
    match result {
        Ok(output) => (output, None),
        Err(error) => (format!("[failed to collect {stream_name}: {error}]\n"), Some(error)),
    }
}
