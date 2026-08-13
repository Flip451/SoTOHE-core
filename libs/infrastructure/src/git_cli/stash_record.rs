//! Repository-local persistence for the guarded stash pairing outcome.
//!
//! The record is infrastructure-owned operational state crossing the process
//! boundary between `stash push` and `stash pop`. This module also owns the
//! bounded readers and Git metadata lockfiles used by that transaction.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ExitStatus};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use domain::CommitHash;
use fs4::fs_std::FileExt as _;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use usecase::git_stash::GitStashPushOutcome;

use crate::git_cli::terminate_bounded_git_child;

/// Worktree Git-directory filename for the pending guarded stash pairing.
pub(crate) const STASH_RECORD_FILE: &str = ".sotp-guarded-stash.json";
/// Shared Git-common-directory filename for the guarded stash operation lock.
pub(crate) const STASH_LOCK_FILE: &str = ".sotp-guarded-stash.lock";

const MAX_STASH_RECORD_BYTES: u64 = 4 * 1024;
/// Bounded stdout retained by the guarded stash command reader.
pub(crate) const MAX_STASH_OUTPUT_BYTES: usize = 16 * 1024;
const STASH_COMMAND_TIMEOUT: Duration = Duration::from_secs(10);
const STASH_POLL_INTERVAL: Duration = Duration::from_millis(10);
const STASH_PIPE_BUFFER_BYTES: usize = 8 * 1024;

/// A Git pipe reader whose result is delivered without retaining unbounded
/// command output in the caller.
pub(crate) struct StashPipeReader<T> {
    receiver: Receiver<std::io::Result<T>>,
    handle: JoinHandle<()>,
}

fn spawn_reader<T, R, F>(
    pipe: R,
    name: &'static str,
    read: F,
) -> std::io::Result<StashPipeReader<T>>
where
    T: Send + 'static,
    R: Read + Send + 'static,
    F: FnOnce(R) -> std::io::Result<T> + Send + 'static,
{
    let (sender, receiver) = mpsc::sync_channel(1);
    let handle = thread::Builder::new().name(name.to_owned()).spawn(move || {
        let result = read(pipe);
        let _ = sender.send(result);
    })?;
    Ok(StashPipeReader { receiver, handle })
}

pub(crate) fn spawn_digest_reader(
    pipe: impl Read + Send + 'static,
) -> std::io::Result<StashPipeReader<Vec<u8>>> {
    spawn_reader(pipe, "streaming-git-ref-reader", |mut pipe| {
        let mut hasher = Sha256::new();
        let mut buffer = [0_u8; STASH_PIPE_BUFFER_BYTES];
        loop {
            match pipe.read(&mut buffer) {
                Ok(0) => return Ok(hasher.finalize().to_vec()),
                Ok(read) => {
                    let Some(chunk) = buffer.get(..read) else {
                        return Err(std::io::Error::other(
                            "git reference reader returned an invalid byte count",
                        ));
                    };
                    hasher.update(chunk);
                }
                Err(error) => return Err(error),
            }
        }
    })
}

pub(crate) fn spawn_bounded_stderr_reader(
    pipe: impl Read + Send + 'static,
) -> std::io::Result<StashPipeReader<Vec<u8>>> {
    spawn_reader(pipe, "bounded-git-stderr-reader", |mut pipe| {
        let mut retained = Vec::new();
        let mut buffer = [0_u8; STASH_PIPE_BUFFER_BYTES];
        loop {
            match pipe.read(&mut buffer) {
                Ok(0) => return Ok(retained),
                Ok(read) => {
                    let remaining = MAX_STASH_OUTPUT_BYTES.saturating_sub(retained.len());
                    let taken = read.min(remaining);
                    let Some(prefix) = buffer.get(..taken) else {
                        return Err(std::io::Error::other(
                            "git stderr reader returned an invalid byte count",
                        ));
                    };
                    retained.extend_from_slice(prefix);
                    if taken < read {
                        return Err(std::io::Error::other("git stderr exceeded its limit"));
                    }
                }
                Err(error) => return Err(error),
            }
        }
    })
}

pub(crate) fn wait_for_stash_child(
    child: &mut Child,
    started: Instant,
) -> std::io::Result<ExitStatus> {
    loop {
        match child.try_wait()? {
            Some(status) => return Ok(status),
            None if started.elapsed() >= STASH_COMMAND_TIMEOUT => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "git command timed out",
                ));
            }
            None => thread::sleep(STASH_POLL_INTERVAL),
        }
    }
}

pub(crate) fn receive_stash_reader<T>(
    reader: &StashPipeReader<T>,
    started: Instant,
) -> std::io::Result<T> {
    let remaining = STASH_COMMAND_TIMEOUT.checked_sub(started.elapsed()).ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::TimedOut, "git command timed out")
    })?;
    match reader.receiver.recv_timeout(remaining) {
        Ok(result) => result,
        Err(RecvTimeoutError::Timeout) => {
            Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "git command timed out"))
        }
        Err(RecvTimeoutError::Disconnected) => {
            Err(std::io::Error::other("git command reader disconnected"))
        }
    }
}

pub(crate) fn join_stash_readers(readers: Vec<StashPipeReader<Vec<u8>>>) -> std::io::Result<()> {
    let mut first_error = None;
    for reader in readers {
        if reader.handle.join().is_err() && first_error.is_none() {
            first_error = Some(std::io::Error::other("git command reader panicked"));
        }
    }
    first_error.map_or(Ok(()), Err)
}

pub(crate) fn cleanup_stash_child(
    child: &mut Child,
    readers: Vec<StashPipeReader<Vec<u8>>>,
) -> std::io::Result<()> {
    let termination = terminate_bounded_git_child(child);
    let readers = join_stash_readers(readers);
    match (termination, readers) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
        (Err(termination), Err(readers)) => Err(std::io::Error::other(format!(
            "git command cleanup failed ({termination}); reader cleanup failed ({readers})"
        ))),
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case", tag = "outcome")]
enum StoredStashOutcome {
    Created { commit: String },
    NothingToStash,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StashRecord {
    pub(crate) outcome: GitStashPushOutcome,
}

impl StashRecord {
    pub(crate) fn new(outcome: GitStashPushOutcome) -> Self {
        Self { outcome }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct StoredStashRecord {
    outcome: StoredStashOutcome,
}

fn record_path(repository_root: &Path) -> PathBuf {
    repository_root.join(STASH_RECORD_FILE)
}

fn reject_non_regular(path: &Path, metadata: &fs::Metadata) -> Result<(), String> {
    if metadata.file_type().is_symlink() {
        return Err(format!("refusing symlinked guarded stash record: {}", path.display()));
    }
    if !metadata.is_file() {
        return Err(format!("refusing non-regular guarded stash record: {}", path.display()));
    }
    Ok(())
}

fn read_record_bytes(path: &Path) -> Result<Option<Vec<u8>>, String> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("cannot inspect guarded stash record: {error}")),
    };
    reject_non_regular(path, &metadata)?;
    if metadata.len() > MAX_STASH_RECORD_BYTES {
        return Err("guarded stash record exceeds the read-size limit".to_owned());
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    let mut file =
        options.open(path).map_err(|error| format!("cannot open guarded stash record: {error}"))?;
    let opened_metadata = file
        .metadata()
        .map_err(|error| format!("cannot inspect opened guarded stash record: {error}"))?;
    if !opened_metadata.is_file() {
        return Err("guarded stash record is not a regular file".to_owned());
    }
    let mut content = Vec::new();
    Read::by_ref(&mut file)
        .take(MAX_STASH_RECORD_BYTES.saturating_add(1))
        .read_to_end(&mut content)
        .map_err(|error| format!("cannot read guarded stash record: {error}"))?;
    if u64::try_from(content.len()).map_or(true, |length| length > MAX_STASH_RECORD_BYTES) {
        return Err("guarded stash record exceeds the read-size limit".to_owned());
    }
    Ok(Some(content))
}

fn decode(content: &[u8]) -> Result<StashRecord, String> {
    let stored: StoredStashRecord = serde_json::from_slice(content)
        .map_err(|error| format!("guarded stash record is malformed: {error}"))?;
    let outcome = match stored.outcome {
        StoredStashOutcome::NothingToStash => Ok(GitStashPushOutcome::NothingToStash),
        StoredStashOutcome::Created { commit } => {
            CommitHash::try_new(commit).map(GitStashPushOutcome::Created).map_err(|error| {
                format!("guarded stash record has an invalid commit identity: {error}")
            })
        }
    }?;
    Ok(StashRecord { outcome })
}

fn encode(record: &StashRecord) -> Result<Vec<u8>, String> {
    let outcome = match &record.outcome {
        GitStashPushOutcome::NothingToStash => StoredStashOutcome::NothingToStash,
        GitStashPushOutcome::Created(commit) => {
            StoredStashOutcome::Created { commit: commit.as_ref().to_owned() }
        }
    };
    let encoded = serde_json::to_vec(&StoredStashRecord { outcome })
        .map_err(|error| format!("cannot encode guarded stash record: {error}"))?;
    if u64::try_from(encoded.len()).map_or(true, |length| length > MAX_STASH_RECORD_BYTES) {
        return Err("guarded stash record exceeds the write-size limit".to_owned());
    }
    if decode(&encoded)? != *record {
        return Err("guarded stash record failed round-trip validation".to_owned());
    }
    Ok(encoded)
}

/// Hold the repository-wide lock for one guarded stash transaction.
pub(crate) struct StashOperationLock {
    _file: File,
}

/// Acquire the cross-process lock that serializes guarded stash push/pop.
pub(crate) fn acquire_lock(lock_path: &Path) -> Result<StashOperationLock, String> {
    match fs::symlink_metadata(lock_path) {
        Ok(metadata) => reject_non_regular(lock_path, &metadata)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("cannot inspect guarded stash lock: {error}")),
    }
    let mut options = OpenOptions::new();
    options.create(true).read(true).write(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    let file = options
        .open(lock_path)
        .map_err(|error| format!("cannot open guarded stash lock: {error}"))?;
    if !file
        .metadata()
        .map_err(|error| format!("cannot inspect opened guarded stash lock: {error}"))?
        .is_file()
    {
        return Err("guarded stash lock is not a regular file".to_owned());
    }
    file.try_lock_exclusive()
        .map_err(|error| format!("cannot acquire guarded stash operation lock: {error}"))?;
    Ok(StashOperationLock { _file: file })
}

/// Read the pending guarded stash pairing from a Git state directory.
pub(crate) fn read(state_dir: &Path) -> Result<Option<StashRecord>, String> {
    read_record_bytes(&record_path(state_dir))?
        .map_or(Ok(None), |content| decode(&content).map(Some))
}

/// Persist one guarded stash pairing outcome in a Git state directory.
pub(crate) fn write(state_dir: &Path, record: &StashRecord) -> Result<(), String> {
    let path = record_path(state_dir);
    match fs::symlink_metadata(&path) {
        Ok(metadata) => reject_non_regular(&path, &metadata)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("cannot inspect guarded stash record: {error}")),
    }
    let encoded = encode(record)?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    let mut file = options
        .open(&path)
        .map_err(|error| format!("cannot create guarded stash record: {error}"))?;
    file.write_all(&encoded)
        .map_err(|error| format!("cannot write guarded stash record: {error}"))?;
    file.sync_all().map_err(|error| format!("cannot persist guarded stash record: {error}"))
}

/// Clear a pairing record only when it still contains the expected outcome.
pub(crate) fn clear(state_dir: &Path, expected: &StashRecord) -> Result<(), String> {
    let path = record_path(state_dir);
    let current = read(state_dir)?
        .ok_or_else(|| "guarded stash pairing record disappeared before clear".to_owned())?;
    if current != *expected {
        return Err("guarded stash pairing record changed before clear".to_owned());
    }
    let metadata = fs::symlink_metadata(&path)
        .map_err(|error| format!("cannot inspect guarded stash record before clear: {error}"))?;
    reject_non_regular(&path, &metadata)?;
    fs::remove_file(&path)
        .map_err(|error| format!("cannot clear guarded stash pairing record: {error}"))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::{STASH_RECORD_FILE, StashRecord, clear, read, write};
    use domain::CommitHash;
    use std::fs;
    use usecase::git_stash::GitStashPushOutcome;

    fn record() -> StashRecord {
        StashRecord::new(GitStashPushOutcome::Created(
            CommitHash::try_new("0123456789abcdef0123456789abcdef01234567").unwrap(),
        ))
    }

    #[test]
    fn test_stash_record_round_trip_and_clear_preserves_typed_outcome() {
        let repository = tempfile::tempdir().expect("repository fixture must exist");
        let expected = record();
        write(repository.path(), &expected).expect("record must be written");
        assert_eq!(read(repository.path()).expect("record must be read"), Some(expected.clone()));
        let entries: Vec<_> = fs::read_dir(repository.path())
            .expect("record directory must be readable")
            .filter_map(|entry| entry.ok())
            .collect();
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries.first().map(|entry| entry.file_name().to_string_lossy().into_owned()),
            Some(STASH_RECORD_FILE.to_owned())
        );
        clear(repository.path(), &expected).expect("record must be cleared");
        assert_eq!(read(repository.path()).expect("record absence must be readable"), None);
    }

    #[test]
    fn test_stash_record_rejects_malformed_content_without_clearing_it() {
        let repository = tempfile::tempdir().expect("repository fixture must exist");
        let path = repository.path().join(STASH_RECORD_FILE);
        fs::write(&path, b"not-json").expect("malformed record must be written");
        assert!(read(repository.path()).is_err());
        assert!(path.exists());
    }
}
