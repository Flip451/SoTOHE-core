//! Freshness-input helpers for the type-signal evaluator.

use std::io::{self, Read as _};
use std::path::Path;
use std::process::{Child, ExitStatus, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use domain::CommitHash;
use domain::tddd::type_signals_doc::TypeSignalsCacheKey;

use super::EvaluateSignalsError;
use crate::tddd::type_signals_codec;

pub(super) fn read_bytes_file_limited(
    path: &Path,
    maximum_bytes: usize,
) -> Result<Vec<u8>, io::Error> {
    use std::io::Read as _;

    let metadata = std::fs::metadata(path)?;
    if metadata.len() > maximum_bytes as u64 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "file exceeds maximum size"));
    }
    // The take-bound caps the allocation even if the file grows between the
    // stat above and this read; reading one extra byte detects that race.
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take((maximum_bytes as u64).saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() > maximum_bytes {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "file exceeds maximum size"));
    }
    Ok(bytes)
}

pub(super) fn read_utf8_file_limited(
    path: &Path,
    maximum_bytes: usize,
) -> Result<String, io::Error> {
    let bytes = read_bytes_file_limited(path, maximum_bytes)?;
    String::from_utf8(bytes).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

const MAX_GIT_OUTPUT_BYTES: usize = 8 * 1024;
const GIT_PROBE_TIMEOUT: Duration = Duration::from_secs(10);
const GIT_PROBE_POLL_INTERVAL: Duration = Duration::from_millis(10);

pub(crate) fn read_head_commit(workspace_root: &Path) -> Result<CommitHash, EvaluateSignalsError> {
    let output = crate::git_cli::isolation::isolated_bounded_git_output(
        workspace_root,
        &["rev-parse", "--verify", "HEAD^{commit}"],
        MAX_GIT_OUTPUT_BYTES,
    )
    .map_err(|error| {
        EvaluateSignalsError::authoritative_input(format!("cannot read HEAD: {error}"))
    })?;
    if !output.status.success() {
        return Err(EvaluateSignalsError::authoritative_input(
            "cannot resolve repository HEAD".to_owned(),
        ));
    }
    CommitHash::try_new(
        std::str::from_utf8(&output.stdout)
            .map_err(|error| {
                EvaluateSignalsError::authoritative_input(format!("HEAD is not UTF-8: {error}"))
            })?
            .trim()
            .to_owned(),
    )
    .map_err(|error| EvaluateSignalsError::authoritative_input(format!("HEAD is invalid: {error}")))
}

pub(crate) fn worktree_is_clean(workspace_root: &Path) -> Result<bool, EvaluateSignalsError> {
    drain_worktree_status(workspace_root).map_err(|error| {
        EvaluateSignalsError::authoritative_input(format!(
            "cannot inspect worktree status: {error}"
        ))
    })
}

struct StatusReader {
    receiver: Receiver<io::Result<bool>>,
    handle: JoinHandle<()>,
}

fn drain_worktree_status(workspace_root: &Path) -> io::Result<bool> {
    let args = ["status", "--porcelain=v1", "--untracked-files=all", "--"];
    let mut command = crate::git_cli::isolation::isolated_git_command(workspace_root, &args);
    command.stderr(Stdio::null());
    let mut child = crate::git_cli::spawn_bounded_git_child(&mut command)?;
    let stdout = match child.stdout.take() {
        Some(stdout) => stdout,
        None => {
            return cleanup_status_probe(
                io::Error::other("git status stdout was not captured"),
                &mut child,
                None,
            );
        }
    };
    let (sender, receiver) = mpsc::sync_channel(1);
    let handle = match thread::Builder::new().name("streaming-git-status-reader".to_owned()).spawn(
        move || {
            let mut dirty = false;
            let mut buffer = [0_u8; 8 * 1024];
            let mut stdout = stdout;
            let result = loop {
                match stdout.read(&mut buffer) {
                    Ok(0) => break Ok(dirty),
                    Ok(_) => dirty = true,
                    Err(error) => break Err(error),
                }
            };
            let _ = sender.send(result);
        },
    ) {
        Ok(handle) => handle,
        Err(error) => return cleanup_status_probe(error, &mut child, None),
    };
    let reader = StatusReader { receiver, handle };
    let started = Instant::now();
    let status = match wait_for_status_child(&mut child, started) {
        Ok(status) => status,
        Err(error) => return cleanup_status_probe(error, &mut child, Some(reader)),
    };
    let dirty = match receive_status_reader(&reader, started) {
        Ok(dirty) => dirty,
        Err(error) => return cleanup_status_probe(error, &mut child, Some(reader)),
    };
    reader.handle.join().map_err(|_| io::Error::other("git status reader panicked"))?;
    if !status.success() {
        return Err(io::Error::other("git status failed"));
    }
    Ok(!dirty)
}

fn wait_for_status_child(child: &mut Child, started: Instant) -> io::Result<ExitStatus> {
    loop {
        match child.try_wait()? {
            Some(status) => return Ok(status),
            None if started.elapsed() >= GIT_PROBE_TIMEOUT => {
                return Err(io::Error::new(io::ErrorKind::TimedOut, "git status timed out"));
            }
            None => thread::sleep(GIT_PROBE_POLL_INTERVAL),
        }
    }
}

fn receive_status_reader(reader: &StatusReader, started: Instant) -> io::Result<bool> {
    let remaining = GIT_PROBE_TIMEOUT
        .checked_sub(started.elapsed())
        .ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "git status timed out"))?;
    match reader.receiver.recv_timeout(remaining) {
        Ok(result) => result,
        Err(RecvTimeoutError::Timeout) => {
            Err(io::Error::new(io::ErrorKind::TimedOut, "git status timed out"))
        }
        Err(RecvTimeoutError::Disconnected) => {
            Err(io::Error::other("git status reader disconnected"))
        }
    }
}

fn cleanup_status_probe<T>(
    error: io::Error,
    child: &mut Child,
    reader: Option<StatusReader>,
) -> io::Result<T> {
    let cleanup = crate::git_cli::terminate_bounded_git_child(child);
    let join = reader.map_or(Ok(()), |reader| {
        reader.handle.join().map_err(|_| io::Error::other("git status reader panicked"))
    });
    match (cleanup, join) {
        (Ok(()), Ok(())) => Err(error),
        (Err(cleanup), Ok(())) => Err(io::Error::new(
            error.kind(),
            format!("{error}; status probe cleanup failed: {cleanup}"),
        )),
        (Ok(()), Err(_)) => {
            Err(io::Error::new(error.kind(), format!("{error}; status probe reader join failed")))
        }
        (Err(cleanup), Err(_)) => Err(io::Error::new(
            error.kind(),
            format!("{error}; status probe cleanup failed: {cleanup}; reader join failed"),
        )),
    }
}

/// Rejects persistence when inputs change while rustdoc or evaluation is running.
pub(super) fn verify_evaluation_inputs_unchanged(
    workspace_root: &Path,
    catalogue_path: &Path,
    baseline_path: &Path,
    initial_key: &TypeSignalsCacheKey,
) -> Result<(), EvaluateSignalsError> {
    if read_head_commit(workspace_root)? != *initial_key.head_commit() {
        return Err(EvaluateSignalsError::authoritative_input(
            "repository HEAD changed during type-signal evaluation".to_owned(),
        ));
    }
    let catalogue =
        read_bytes_file_limited(catalogue_path, super::MAX_CATALOGUE_BYTES).map_err(|error| {
            EvaluateSignalsError::authoritative_input(format!(
                "cannot re-read catalogue '{}': {error}",
                catalogue_path.display()
            ))
        })?;
    if type_signals_codec::declaration_hash(&catalogue) != *initial_key.declaration_hash() {
        return Err(EvaluateSignalsError::authoritative_input(
            "catalogue changed during type-signal evaluation".to_owned(),
        ));
    }
    let baseline =
        read_bytes_file_limited(baseline_path, super::MAX_RUSTDOC_JSON_BYTES).map_err(|error| {
            EvaluateSignalsError::authoritative_input(format!(
                "cannot re-read baseline '{}': {error}",
                baseline_path.display()
            ))
        })?;
    if type_signals_codec::baseline_hash(&baseline) != *initial_key.baseline_hash() {
        return Err(EvaluateSignalsError::authoritative_input(
            "baseline changed during type-signal evaluation".to_owned(),
        ));
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::{read_utf8_file_limited, worktree_is_clean};
    use crate::verify::test_support::run_git;
    use std::fs;

    #[test]
    fn test_read_utf8_file_limited_rejects_missing_file() {
        assert!(read_utf8_file_limited(std::path::Path::new("missing"), 1).is_err());
    }

    #[test]
    fn test_worktree_is_clean_drains_large_status_output_without_retaining_it() {
        let repository = tempfile::tempdir().unwrap();
        run_git(repository.path(), &["init", "-q", "-b", "main"]);
        run_git(repository.path(), &["config", "user.email", "test@example.com"]);
        run_git(repository.path(), &["config", "user.name", "test"]);
        fs::write(repository.path().join("tracked.txt"), "base\n").unwrap();
        run_git(repository.path(), &["add", "tracked.txt"]);
        run_git(repository.path(), &["commit", "-q", "-m", "initial"]);
        let suffix = "x".repeat(48);
        for index in 0..320 {
            fs::write(repository.path().join(format!("untracked-{index:04}-{suffix}")), "saved\n")
                .unwrap();
        }

        assert!(!worktree_is_clean(repository.path()).unwrap());
    }
}
