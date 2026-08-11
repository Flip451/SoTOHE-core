//! Repository-local persistence for the guarded stash pairing outcome.
//!
//! The record is infrastructure-owned operational state crossing the process
//! boundary between `stash push` and `stash pop`. This module also owns the
//! bounded readers and Git metadata lockfiles used by that transaction.

use std::fs::{self, File, OpenOptions};
use std::io::Read;
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
use crate::track::atomic_write::atomic_write_file;

/// Git-common-directory filename for the pending guarded stash pairing.
pub(crate) const STASH_RECORD_FILE: &str = ".sotp-guarded-stash.json";
/// Git-common-directory filename for the guarded stash operation lock.
pub(crate) const STASH_LOCK_FILE: &str = ".sotp-guarded-stash.lock";

const MAX_STASH_RECORD_BYTES: u64 = 4 * 1024;
const MAX_STASH_PATH_BYTES: usize = 16 * 1024;
const PATH_DIGEST_HEX_BYTES: usize = 64;
/// Bounded stdout retained by the guarded stash command reader.
pub(crate) const MAX_STASH_OUTPUT_BYTES: usize = 16 * 1024;
const STASH_COMMAND_TIMEOUT: Duration = Duration::from_secs(10);
const STASH_POLL_INTERVAL: Duration = Duration::from_millis(10);
const STASH_PIPE_BUFFER_BYTES: usize = 8 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StashListSummary {
    pub(crate) count: usize,
    pub(crate) expected_index: Option<usize>,
    pub(crate) expected_matches: usize,
    pub(crate) first_identity: Option<CommitHash>,
    pub(crate) digest: Vec<u8>,
    pub(crate) without_expected_digest: Vec<u8>,
}

pub(crate) enum StashReaderKind {
    Digest,
    Bounded,
    List(CommitHash),
}

pub(crate) enum StashReaderResult {
    Digest(Vec<u8>),
    Bounded(Vec<u8>),
    List(StashListSummary),
}

pub(crate) struct StashPipeReader {
    receiver: Receiver<std::io::Result<StashReaderResult>>,
    handle: JoinHandle<()>,
}

const MAX_STASH_LIST_LINE_BYTES: usize = 128;

struct StashListAccumulator<'a> {
    expected: &'a CommitHash,
    count: usize,
    expected_index: Option<usize>,
    expected_matches: usize,
    first_identity: Option<CommitHash>,
    digest: Sha256,
    without_expected_digest: Sha256,
}

impl<'a> StashListAccumulator<'a> {
    fn new(expected: &'a CommitHash) -> Self {
        Self {
            expected,
            count: 0,
            expected_index: None,
            expected_matches: 0,
            first_identity: None,
            digest: Sha256::new(),
            without_expected_digest: Sha256::new(),
        }
    }

    fn process_line(&mut self, line: &[u8]) -> std::io::Result<()> {
        let value = line.strip_suffix(b"\n").unwrap_or(line);
        if value.iter().all(u8::is_ascii_whitespace) {
            return Ok(());
        }
        let identity = value
            .split(|byte| byte.is_ascii_whitespace())
            .find(|field| !field.is_empty())
            .ok_or_else(|| {
                std::io::Error::other("git stash list returned an empty commit identity")
            })?;
        let identity = String::from_utf8(identity.to_vec()).map_err(|error| {
            std::io::Error::other(format!("git stash list returned invalid UTF-8: {error}"))
        })?;
        let identity = CommitHash::try_new(identity.trim().to_owned()).map_err(|error| {
            std::io::Error::other(format!(
                "git stash list returned an invalid commit identity: {error}"
            ))
        })?;
        let index = self.count;
        self.count = self
            .count
            .checked_add(1)
            .ok_or_else(|| std::io::Error::other("git stash list entry count overflowed"))?;
        if self.first_identity.is_none() {
            self.first_identity = Some(identity.clone());
        }
        self.digest.update(line);
        if identity == *self.expected {
            if self.expected_index.is_none() {
                self.expected_index = Some(index);
            }
            self.expected_matches = self
                .expected_matches
                .checked_add(1)
                .ok_or_else(|| std::io::Error::other("git stash identity count overflowed"))?;
        } else {
            self.without_expected_digest.update(line);
        }
        Ok(())
    }

    fn finish(self) -> StashListSummary {
        StashListSummary {
            count: self.count,
            expected_index: self.expected_index,
            expected_matches: self.expected_matches,
            first_identity: self.first_identity,
            digest: self.digest.finalize().to_vec(),
            without_expected_digest: self.without_expected_digest.finalize().to_vec(),
        }
    }
}

fn read_stash_list(
    pipe: &mut impl Read,
    expected: &CommitHash,
) -> std::io::Result<StashListSummary> {
    let mut accumulator = StashListAccumulator::new(expected);
    let mut line = Vec::with_capacity(64);
    let mut buffer = [0_u8; STASH_PIPE_BUFFER_BYTES];
    loop {
        let read = pipe.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        let Some(chunk) = buffer.get(..read) else {
            return Err(std::io::Error::other(
                "git stash list reader returned an invalid byte count",
            ));
        };
        for byte in chunk {
            line.push(*byte);
            if line.len() > MAX_STASH_LIST_LINE_BYTES {
                return Err(std::io::Error::other("git stash list line exceeded its limit"));
            }
            if *byte == b'\n' {
                accumulator.process_line(&line)?;
                line.clear();
            }
        }
    }
    if !line.is_empty() {
        accumulator.process_line(&line)?;
    }
    Ok(accumulator.finish())
}

fn read_stash_stream(
    mut pipe: impl Read,
    kind: StashReaderKind,
) -> std::io::Result<StashReaderResult> {
    match kind {
        StashReaderKind::Digest => {
            let mut hasher = Sha256::new();
            let mut buffer = [0_u8; STASH_PIPE_BUFFER_BYTES];
            loop {
                let read = pipe.read(&mut buffer)?;
                if read == 0 {
                    return Ok(StashReaderResult::Digest(hasher.finalize().to_vec()));
                }
                let Some(chunk) = buffer.get(..read) else {
                    return Err(std::io::Error::other(
                        "git status reader returned an invalid byte count",
                    ));
                };
                hasher.update(chunk);
            }
        }
        StashReaderKind::Bounded => {
            let mut retained = Vec::new();
            let mut buffer = [0_u8; STASH_PIPE_BUFFER_BYTES];
            loop {
                let read = pipe.read(&mut buffer)?;
                if read == 0 {
                    return Ok(StashReaderResult::Bounded(retained));
                }
                let Some(chunk) = buffer.get(..read) else {
                    return Err(std::io::Error::other(
                        "git status reader returned an invalid byte count",
                    ));
                };
                let remaining = MAX_STASH_OUTPUT_BYTES.saturating_sub(retained.len());
                let taken = read.min(remaining);
                let Some(prefix) = chunk.get(..taken) else {
                    return Err(std::io::Error::other(
                        "git status reader returned an invalid byte count",
                    ));
                };
                retained.extend_from_slice(prefix);
                if taken < read {
                    return Err(std::io::Error::other("git status stderr exceeded its limit"));
                }
            }
        }
        StashReaderKind::List(expected) => {
            read_stash_list(&mut pipe, &expected).map(StashReaderResult::List)
        }
    }
}

pub(crate) fn spawn_stash_reader(
    pipe: impl Read + Send + 'static,
    kind: StashReaderKind,
) -> std::io::Result<StashPipeReader> {
    let (sender, receiver) = mpsc::sync_channel(1);
    let name = match &kind {
        StashReaderKind::Digest => "streaming-git-status-reader",
        StashReaderKind::Bounded => "bounded-git-status-stderr-reader",
        StashReaderKind::List(_) => "streaming-git-stash-list-reader",
    };
    let handle = thread::Builder::new().name(name.to_owned()).spawn(move || {
        let result = read_stash_stream(pipe, kind);
        let _ = sender.send(result);
    })?;
    Ok(StashPipeReader { receiver, handle })
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
                    "git status timed out",
                ));
            }
            None => thread::sleep(STASH_POLL_INTERVAL),
        }
    }
}

pub(crate) fn receive_stash_reader(
    reader: &StashPipeReader,
    started: Instant,
) -> std::io::Result<StashReaderResult> {
    let remaining = STASH_COMMAND_TIMEOUT
        .checked_sub(started.elapsed())
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::TimedOut, "git status timed out"))?;
    match reader.receiver.recv_timeout(remaining) {
        Ok(result) => result,
        Err(RecvTimeoutError::Timeout) => {
            Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "git status timed out"))
        }
        Err(RecvTimeoutError::Disconnected) => {
            Err(std::io::Error::other("git status reader disconnected"))
        }
    }
}

pub(crate) fn join_stash_readers(readers: Vec<StashPipeReader>) -> std::io::Result<()> {
    let mut first_error = None;
    for reader in readers {
        if reader.handle.join().is_err() && first_error.is_none() {
            first_error = Some(std::io::Error::other("git status reader panicked"));
        }
    }
    first_error.map_or(Ok(()), Err)
}

pub(crate) fn cleanup_stash_child(
    child: &mut Child,
    readers: Vec<StashPipeReader>,
) -> std::io::Result<()> {
    let termination = terminate_bounded_git_child(child);
    let readers = join_stash_readers(readers);
    match (termination, readers) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
        (Err(termination), Err(readers)) => Err(std::io::Error::other(format!(
            "git status cleanup failed ({termination}); reader cleanup failed ({readers})"
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
pub(crate) struct StashWorktreeIdentity {
    git_dir_digest: String,
    worktree_root_digest: String,
}

impl StashWorktreeIdentity {
    pub(crate) fn try_new(git_dir: String, worktree_root: String) -> Result<Self, String> {
        for (label, value) in [("Git directory", &git_dir), ("worktree root", &worktree_root)] {
            let path = Path::new(value);
            if value.is_empty()
                || value.len() > MAX_STASH_PATH_BYTES
                || !path.is_absolute()
                || path.components().any(|component| {
                    matches!(
                        component,
                        std::path::Component::CurDir | std::path::Component::ParentDir
                    )
                })
            {
                return Err(format!("guarded stash {label} identity is not a safe absolute path"));
            }
        }
        Ok(Self {
            git_dir_digest: format!("{:x}", Sha256::digest(git_dir.as_bytes())),
            worktree_root_digest: format!("{:x}", Sha256::digest(worktree_root.as_bytes())),
        })
    }

    fn from_digests(git_dir_digest: String, worktree_root_digest: String) -> Result<Self, String> {
        for (label, value) in
            [("Git directory", &git_dir_digest), ("worktree root", &worktree_root_digest)]
        {
            if value.len() != PATH_DIGEST_HEX_BYTES
                || !value.bytes().all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
            {
                return Err(format!("guarded stash {label} identity has an invalid path digest"));
            }
        }
        Ok(Self { git_dir_digest, worktree_root_digest })
    }

    pub(crate) fn git_dir(&self) -> &str {
        &self.git_dir_digest
    }

    pub(crate) fn worktree_root(&self) -> &str {
        &self.worktree_root_digest
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StashRecord {
    pub(crate) outcome: GitStashPushOutcome,
    pub(crate) worktree: StashWorktreeIdentity,
}

impl StashRecord {
    pub(crate) fn new(outcome: GitStashPushOutcome, worktree: StashWorktreeIdentity) -> Self {
        Self { outcome, worktree }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct StoredStashRecord {
    worktree_git_dir: String,
    worktree_root: String,
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
    let worktree =
        StashWorktreeIdentity::from_digests(stored.worktree_git_dir, stored.worktree_root)?;
    let outcome = match stored.outcome {
        StoredStashOutcome::NothingToStash => Ok(GitStashPushOutcome::NothingToStash),
        StoredStashOutcome::Created { commit } => {
            CommitHash::try_new(commit).map(GitStashPushOutcome::Created).map_err(|error| {
                format!("guarded stash record has an invalid commit identity: {error}")
            })
        }
    }?;
    Ok(StashRecord { outcome, worktree })
}

fn encode(record: &StashRecord) -> Result<Vec<u8>, String> {
    let outcome = match &record.outcome {
        GitStashPushOutcome::NothingToStash => StoredStashOutcome::NothingToStash,
        GitStashPushOutcome::Created(commit) => {
            StoredStashOutcome::Created { commit: commit.as_ref().to_owned() }
        }
    };
    let stored = StoredStashRecord {
        worktree_git_dir: record.worktree.git_dir().to_owned(),
        worktree_root: record.worktree.worktree_root().to_owned(),
        outcome,
    };
    let encoded = serde_json::to_vec(&stored)
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

/// Atomically persist one guarded stash pairing outcome in a Git state directory.
pub(crate) fn write(state_dir: &Path, record: &StashRecord) -> Result<(), String> {
    let path = record_path(state_dir);
    match fs::symlink_metadata(&path) {
        Ok(metadata) => reject_non_regular(&path, &metadata)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("cannot inspect guarded stash record: {error}")),
    }
    let encoded = encode(record)?;
    atomic_write_file(&path, &encoded)
        .map_err(|error| format!("cannot atomically write guarded stash record: {error}"))
}

fn restore_expected_record(path: &Path, expected: &StashRecord) -> Result<(), String> {
    match read_record_bytes(path)? {
        Some(content) if decode(&content)? == *expected => Ok(()),
        Some(_) => Err("guarded stash pairing record changed while clearing".to_owned()),
        None => {
            let encoded = encode(expected)?;
            atomic_write_file(path, &encoded)
                .map_err(|error| format!("cannot restore guarded stash pairing record: {error}"))
        }
    }
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
        .map_err(|error| format!("cannot clear guarded stash pairing record: {error}"))?;
    let sync_result = File::open(state_dir)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| format!("cannot persist guarded stash record clear: {error}"));
    match sync_result {
        Ok(()) => Ok(()),
        Err(error) => {
            let restoration = restore_expected_record(&path, expected);
            Err(format!("{error}; record restoration: {restoration:?}"))
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::{
        STASH_RECORD_FILE, StashRecord, StashWorktreeIdentity, clear, read, read_stash_list, write,
    };
    use domain::CommitHash;
    use std::fs;
    use std::io::Cursor;
    use usecase::git_stash::GitStashPushOutcome;

    fn created() -> GitStashPushOutcome {
        GitStashPushOutcome::Created(
            CommitHash::try_new("0123456789abcdef0123456789abcdef01234567").unwrap(),
        )
    }

    fn record() -> StashRecord {
        StashRecord::new(
            created(),
            StashWorktreeIdentity::try_new(
                "/guarded-stash/git".to_owned(),
                "/guarded-stash/main".to_owned(),
            )
            .expect("fixture worktree identity must be valid"),
        )
    }

    #[test]
    fn test_stash_list_reader_summarizes_large_stack_with_bounded_memory() {
        let expected = CommitHash::try_new(format!("{:040x}", 400))
            .expect("fixture stash identity must be valid");
        let mut output = Vec::new();
        for index in 1..=400 {
            output.extend_from_slice(format!("{:040x}\n", index).as_bytes());
        }
        assert!(
            output.len() > super::MAX_STASH_OUTPUT_BYTES,
            "fixture must exceed the retained diagnostic output limit"
        );
        let mut input = Cursor::new(output);
        let summary =
            read_stash_list(&mut input, &expected).expect("large stash list must be summarized");
        assert_eq!(summary.count, 400);
        assert_eq!(summary.expected_index, Some(399));
        assert_eq!(summary.expected_matches, 1);
        assert_eq!(
            summary.first_identity,
            Some(
                CommitHash::try_new(format!("{:040x}", 1)).expect("fixture identity must be valid")
            )
        );
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
        assert_eq!(entries.len(), 1, "atomic record write must leave no temporary file");
        assert_eq!(
            entries.first().map(|entry| entry.file_name().to_string_lossy().into_owned()),
            Some(STASH_RECORD_FILE.to_owned())
        );
        clear(repository.path(), &expected).expect("record must be cleared");
        assert_eq!(read(repository.path()).expect("record absence must be readable"), None);
    }

    #[test]
    fn test_stash_record_rejects_malformed_content_without_silently_clearing_it() {
        let repository = tempfile::tempdir().expect("repository fixture must exist");
        let path = repository.path().join(STASH_RECORD_FILE);
        fs::write(&path, b"not-json").expect("malformed record must be written");
        assert!(read(repository.path()).is_err());
        assert!(path.exists());
    }
}
