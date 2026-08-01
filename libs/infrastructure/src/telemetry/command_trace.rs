//! Typed configuration and local JSONL persistence for command traces.

use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write as _};
use std::path::{Path, PathBuf};

use fs4::fs_std::FileExt as _;
use serde::Serialize;
use thiserror::Error;
use usecase::telemetry::command_trace::{
    CommandExecutionResult, CommandTraceRecord, CommandTraceWriteError, CommandTraceWriterPort,
};

use crate::track::symlink_guard::reject_symlinks_up_to_root;

/// A positive byte limit for an active command-trace file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandTraceFileSizeLimitBytes(u64);

impl CommandTraceFileSizeLimitBytes {
    /// Constructs a positive file-size limit.
    pub fn try_new(value: u64) -> Result<Self, CommandTracePolicyError> {
        if value == 0 {
            return Err(CommandTracePolicyError::ZeroFileSizeLimit);
        }

        Ok(Self(value))
    }

    /// Returns the configured number of bytes.
    pub fn value(&self) -> u64 {
        self.0
    }
}

/// A positive count of rotated command-trace files to retain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandTraceRetainedFileCount(u16);

impl CommandTraceRetainedFileCount {
    /// Constructs a positive retained-file count.
    pub fn try_new(value: u16) -> Result<Self, CommandTracePolicyError> {
        if value == 0 {
            return Err(CommandTracePolicyError::ZeroRetainedFileCount);
        }

        Ok(Self(value))
    }

    /// Returns the configured number of files to retain.
    pub fn value(&self) -> u16 {
        self.0
    }
}

/// Failures constructing typed command-trace rotation-policy values.
#[derive(Debug, Error)]
pub enum CommandTracePolicyError {
    /// The active file-size limit was zero.
    #[error("command trace file-size limit must be greater than zero")]
    ZeroFileSizeLimit,

    /// The rotated-file retention count was zero.
    #[error("command trace retained-file count must be greater than zero")]
    ZeroRetainedFileCount,
}

/// Typed configuration for local command-trace file rotation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandTraceRotationPolicy {
    /// Maximum byte size of the active file before rotation.
    pub max_file_size: CommandTraceFileSizeLimitBytes,
    /// Number of rotated files to retain.
    pub retained_files: CommandTraceRetainedFileCount,
}

impl CommandTraceRotationPolicy {
    /// Combines validated rotation bounds into one policy.
    pub fn new(
        max_file_size: CommandTraceFileSizeLimitBytes,
        retained_files: CommandTraceRetainedFileCount,
    ) -> Self {
        Self { max_file_size, retained_files }
    }
}

/// Synchronous local-filesystem implementation of [`CommandTraceWriterPort`].
///
/// Each successful call appends one newline-delimited JSON record. Rotated files use
/// `.{n}` suffixes, where `.1` is the newest and larger numbers are older.
#[derive(Debug)]
pub struct FsCommandTraceAdapter {
    output_path: PathBuf,
    rotation: CommandTraceRotationPolicy,
}

impl FsCommandTraceAdapter {
    /// Creates an adapter using the supplied output path and validated rotation policy.
    #[must_use]
    pub fn new(output_path: PathBuf, rotation: CommandTraceRotationPolicy) -> Self {
        Self { output_path, rotation }
    }

    fn write_record(&self, record: CommandTraceRecord) -> Result<(), CommandTraceWriteError> {
        let line = serialize_record(record)?;
        let line_size =
            u64::try_from(line.len()).map_err(|_| CommandTraceWriteError::Unavailable)?;

        self.reject_symlinks()?;
        self.ensure_parent_directory()?;
        let _lock = self.acquire_write_lock()?;
        // Check again after creating missing parents and acquiring the writer lock.
        self.reject_symlinks()?;
        let active_size = self.active_file_size()?;
        let prospective_size =
            active_size.checked_add(line_size).ok_or(CommandTraceWriteError::Unavailable)?;
        if active_size > 0 && prospective_size > self.rotation.max_file_size.value() {
            self.rotate_active_file()?;
        } else {
            self.prune_excess_rotated_files()?;
        }

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.output_path)
            .map_err(|_| CommandTraceWriteError::Unavailable)?;
        file.write_all(&line).map_err(|_| CommandTraceWriteError::Unavailable)
    }

    fn reject_symlinks(&self) -> Result<(), CommandTraceWriteError> {
        reject_symlinks_up_to_root(&self.output_path)
            .map_err(|_| CommandTraceWriteError::Unavailable)
    }

    fn active_file_size(&self) -> Result<u64, CommandTraceWriteError> {
        match fs::metadata(&self.output_path) {
            Ok(metadata) if metadata.is_file() => Ok(metadata.len()),
            Ok(_) => Err(CommandTraceWriteError::Unavailable),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(0),
            Err(_) => Err(CommandTraceWriteError::Unavailable),
        }
    }

    fn ensure_parent_directory(&self) -> Result<(), CommandTraceWriteError> {
        match self.output_path.parent().filter(|path| !path.as_os_str().is_empty()) {
            Some(parent) => {
                fs::create_dir_all(parent).map_err(|_| CommandTraceWriteError::Unavailable)
            }
            None => Ok(()),
        }
    }

    /// Acquires the per-output exclusive lock for the full rotation-and-append transaction.
    fn acquire_write_lock(&self) -> Result<fs::File, CommandTraceWriteError> {
        let lock_path = self.lock_path();
        reject_symlinks_up_to_root(&lock_path).map_err(|_| CommandTraceWriteError::Unavailable)?;

        let mut options = OpenOptions::new();
        options.create(true).write(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;

            options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
        }
        let file = options.open(lock_path).map_err(|_| CommandTraceWriteError::Unavailable)?;
        file.lock_exclusive().map_err(|_| CommandTraceWriteError::Unavailable)?;
        Ok(file)
    }

    fn rotate_active_file(&self) -> Result<(), CommandTraceWriteError> {
        let retained_files = self.rotation.retained_files.value();
        self.prune_excess_rotated_files()?;
        self.remove_file_if_present(&self.rotated_path(retained_files))?;

        for index in (1..retained_files).rev() {
            let source = self.rotated_path(index);
            let destination = self.rotated_path(index + 1);
            self.rename_if_present(&source, &destination)?;
        }

        fs::rename(&self.output_path, self.rotated_path(1))
            .map_err(|_| CommandTraceWriteError::Unavailable)
    }

    /// Deletes every stale generation beyond the configured retention bound, oldest first.
    fn prune_excess_rotated_files(&self) -> Result<(), CommandTraceWriteError> {
        let retained_files = self.rotation.retained_files.value();
        if let Some(first_excess) = retained_files.checked_add(1) {
            let mut highest_excess = None;
            for index in first_excess..=u16::MAX {
                let path = self.rotated_path(index);
                match fs::symlink_metadata(path) {
                    Ok(_) => highest_excess = Some(index),
                    // Generations can have gaps when a prior rotation was interrupted.
                    // Keep checking through the bounded suffix space so stale files after
                    // a gap cannot evade the retention limit.
                    Err(error) if error.kind() == ErrorKind::NotFound => continue,
                    Err(_) => return Err(CommandTraceWriteError::Unavailable),
                }
            }

            if let Some(highest_excess) = highest_excess {
                for index in (first_excess..=highest_excess).rev() {
                    self.remove_file_if_present(&self.rotated_path(index))?;
                }
            }
        }

        Ok(())
    }

    fn remove_file_if_present(&self, path: &Path) -> Result<(), CommandTraceWriteError> {
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
            Err(_) => Err(CommandTraceWriteError::Unavailable),
        }
    }

    fn rename_if_present(
        &self,
        source: &Path,
        destination: &Path,
    ) -> Result<(), CommandTraceWriteError> {
        match fs::rename(source, destination) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
            Err(_) => Err(CommandTraceWriteError::Unavailable),
        }
    }

    fn rotated_path(&self, index: u16) -> PathBuf {
        rotated_path(&self.output_path, index)
    }

    fn lock_path(&self) -> PathBuf {
        suffixed_path(&self.output_path, ".lock")
    }
}

impl CommandTraceWriterPort for FsCommandTraceAdapter {
    fn record(&self, record: CommandTraceRecord) -> Result<(), CommandTraceWriteError> {
        self.write_record(record)
    }
}

fn serialize_record(record: CommandTraceRecord) -> Result<Vec<u8>, CommandTraceWriteError> {
    let mut line = serde_json::to_vec(&JsonCommandTraceRecord::from(record))
        .map_err(|_| CommandTraceWriteError::Unavailable)?;
    line.push(b'\n');
    Ok(line)
}

fn rotated_path(output_path: &Path, index: u16) -> PathBuf {
    suffixed_path(output_path, &format!(".{index}"))
}

fn suffixed_path(output_path: &Path, suffix: &str) -> PathBuf {
    let mut path = output_path.as_os_str().to_os_string();
    path.push(suffix);
    PathBuf::from(path)
}

#[derive(Serialize)]
struct JsonCommandTraceRecord {
    command: String,
    duration_ms: u64,
    result: JsonCommandExitResult,
}

impl From<CommandTraceRecord> for JsonCommandTraceRecord {
    fn from(record: CommandTraceRecord) -> Self {
        let result = match record.result {
            CommandExecutionResult::Success => JsonCommandExitResult::Success,
            CommandExecutionResult::Failure(exit_code) => {
                JsonCommandExitResult::Failure { exit_code: exit_code.value() }
            }
        };

        Self {
            command: record.command.as_str().to_owned(),
            duration_ms: *record.duration.as_ref(),
            result,
        }
    }
}

#[derive(Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum JsonCommandExitResult {
    Success,
    Failure { exit_code: i32 },
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use std::fs;
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    use tempfile::TempDir;
    use usecase::telemetry::command_trace::{
        CommandDurationMillis, CommandExecutionResult, CommandExitCode, CommandTraceRecord,
        CommandTraceWriteError, CommandTraceWriterPort, SotpCommandIdentity,
    };

    use super::*;

    fn valid_file_size_limit() -> Result<CommandTraceFileSizeLimitBytes, CommandTracePolicyError> {
        CommandTraceFileSizeLimitBytes::try_new(4_096)
    }

    fn valid_retained_file_count() -> Result<CommandTraceRetainedFileCount, CommandTracePolicyError>
    {
        CommandTraceRetainedFileCount::try_new(3)
    }

    fn record(command: &str, result: CommandExecutionResult) -> CommandTraceRecord {
        CommandTraceRecord {
            command: SotpCommandIdentity::try_new(command.to_owned()).expect("valid command"),
            duration: CommandDurationMillis::from(127),
            result,
        }
    }

    fn adapter(
        output_path: PathBuf,
        max_file_size: u64,
        retained_files: u16,
    ) -> FsCommandTraceAdapter {
        let max_file_size = CommandTraceFileSizeLimitBytes::try_new(max_file_size)
            .expect("positive file size limit");
        let retained_files = CommandTraceRetainedFileCount::try_new(retained_files)
            .expect("positive retained-file count");
        FsCommandTraceAdapter::new(
            output_path,
            CommandTraceRotationPolicy::new(max_file_size, retained_files),
        )
    }

    #[test]
    fn test_command_trace_file_size_limit_bytes_zero_returns_zero_file_size_limit() {
        assert!(matches!(
            CommandTraceFileSizeLimitBytes::try_new(0),
            Err(CommandTracePolicyError::ZeroFileSizeLimit)
        ));
    }

    #[test]
    fn test_command_trace_file_size_limit_bytes_positive_value_round_trips()
    -> Result<(), CommandTracePolicyError> {
        assert_eq!(valid_file_size_limit()?.value(), 4_096);
        Ok(())
    }

    #[test]
    fn test_command_trace_retained_file_count_zero_returns_zero_retained_file_count() {
        assert!(matches!(
            CommandTraceRetainedFileCount::try_new(0),
            Err(CommandTracePolicyError::ZeroRetainedFileCount)
        ));
    }

    #[test]
    fn test_command_trace_retained_file_count_positive_value_round_trips()
    -> Result<(), CommandTracePolicyError> {
        assert_eq!(valid_retained_file_count()?.value(), 3);
        Ok(())
    }

    #[test]
    fn test_command_trace_rotation_policy_valid_values_constructs_policy()
    -> Result<(), CommandTracePolicyError> {
        let max_file_size = valid_file_size_limit()?;
        let retained_files = valid_retained_file_count()?;

        let policy = CommandTraceRotationPolicy::new(max_file_size, retained_files);

        assert_eq!(policy.max_file_size, max_file_size);
        assert_eq!(policy.retained_files, retained_files);
        Ok(())
    }

    #[test]
    fn test_fs_command_trace_adapter_record_success_appends_valid_jsonl_records() {
        let directory = TempDir::new().expect("temporary directory");
        let output_path = directory.path().join("traces").join("commands.jsonl");
        let adapter = adapter(output_path.clone(), 4_096, 3);

        adapter
            .record(record("track plan", CommandExecutionResult::Success))
            .expect("success record writes");
        adapter
            .record(record(
                "track review",
                CommandExecutionResult::Failure(
                    CommandExitCode::try_new(17).expect("non-zero exit code"),
                ),
            ))
            .expect("failure record writes");

        let content = fs::read_to_string(output_path).expect("JSONL file reads");
        assert!(content.ends_with('\n'));
        let records: Vec<serde_json::Value> = content
            .lines()
            .map(serde_json::from_str)
            .collect::<Result<_, _>>()
            .expect("each JSONL line is valid JSON");

        assert_eq!(records.len(), 2);
        let first = records.first().expect("first record");
        let second = records.get(1).expect("second record");
        let first_result = first.get("result").expect("first result");
        let second_result = second.get("result").expect("second result");
        assert_eq!(first.get("command").and_then(serde_json::Value::as_str), Some("track plan"));
        assert_eq!(first.get("duration_ms").and_then(serde_json::Value::as_u64), Some(127));
        assert_eq!(first_result.get("status").and_then(serde_json::Value::as_str), Some("success"));
        assert_eq!(second.get("command").and_then(serde_json::Value::as_str), Some("track review"));
        assert_eq!(second.get("duration_ms").and_then(serde_json::Value::as_u64), Some(127));
        assert_eq!(
            second_result.get("status").and_then(serde_json::Value::as_str),
            Some("failure")
        );
        assert_eq!(second_result.get("exit_code").and_then(serde_json::Value::as_i64), Some(17));
    }

    #[test]
    fn test_fs_command_trace_adapter_record_file_parent_returns_unavailable_error() {
        let directory = TempDir::new().expect("temporary directory");
        let blocked_parent = directory.path().join("not-a-directory");
        fs::write(&blocked_parent, "file").expect("blocked parent file writes");
        let adapter = adapter(blocked_parent.join("commands.jsonl"), 4_096, 3);

        let result = adapter.record(record("track plan", CommandExecutionResult::Success));

        assert!(matches!(result, Err(CommandTraceWriteError::Unavailable)));
    }

    #[test]
    fn test_fs_command_trace_adapter_record_directory_leaf_returns_unavailable_error() {
        let directory = TempDir::new().expect("temporary directory");
        let output_path = directory.path().join("commands.jsonl");
        fs::create_dir(&output_path).expect("directory leaf creates");
        let adapter = adapter(output_path.clone(), 1, 3);

        let result = adapter.record(record("track plan", CommandExecutionResult::Success));

        assert!(matches!(result, Err(CommandTraceWriteError::Unavailable)));
        assert!(output_path.is_dir());
        assert!(!rotated_path(&output_path, 1).exists());
    }

    #[cfg(unix)]
    #[test]
    fn test_fs_command_trace_adapter_record_symlinked_leaf_returns_unavailable_error() {
        let directory = TempDir::new().expect("temporary directory");
        let target = directory.path().join("outside.jsonl");
        fs::write(&target, "outside\n").expect("target file writes");
        let output_path = directory.path().join("commands.jsonl");
        std::os::unix::fs::symlink(&target, &output_path).expect("leaf symlink creates");
        let adapter = adapter(output_path, 4_096, 3);

        let result = adapter.record(record("track plan", CommandExecutionResult::Success));

        assert!(matches!(result, Err(CommandTraceWriteError::Unavailable)));
        assert_eq!(fs::read_to_string(target).expect("target file reads"), "outside\n");
    }

    #[cfg(unix)]
    #[test]
    fn test_fs_command_trace_adapter_record_symlinked_parent_returns_unavailable_error() {
        let directory = TempDir::new().expect("temporary directory");
        let outside = directory.path().join("outside");
        fs::create_dir(&outside).expect("outside directory creates");
        let symlinked_parent = directory.path().join("traces");
        std::os::unix::fs::symlink(&outside, &symlinked_parent).expect("parent symlink creates");
        let adapter = adapter(symlinked_parent.join("missing").join("commands.jsonl"), 4_096, 3);

        let result = adapter.record(record("track plan", CommandExecutionResult::Success));

        assert!(matches!(result, Err(CommandTraceWriteError::Unavailable)));
        assert!(!outside.join("missing").exists());
    }

    #[test]
    fn test_fs_command_trace_adapter_record_over_limit_rotates_before_append() {
        let directory = TempDir::new().expect("temporary directory");
        let output_path = directory.path().join("commands.jsonl");
        let first = record("command-01", CommandExecutionResult::Success);
        let second = record("command-02", CommandExecutionResult::Success);
        let max_file_size =
            u64::try_from(serialize_record(first.clone()).expect("serializes").len())
                .expect("line length fits u64");
        let adapter = adapter(output_path.clone(), max_file_size, 2);

        adapter.record(first).expect("first record writes");
        adapter.record(second).expect("second record writes");

        assert!(fs::metadata(&output_path).expect("active file metadata").len() <= max_file_size);
        let active = fs::read_to_string(&output_path).expect("active file reads");
        let rotated =
            fs::read_to_string(rotated_path(&output_path, 1)).expect("rotated file reads");
        assert!(active.contains("command-02"));
        assert!(rotated.contains("command-01"));
    }

    #[test]
    fn test_fs_command_trace_adapter_record_excess_rotations_delete_oldest_file_first() {
        let directory = TempDir::new().expect("temporary directory");
        let output_path = directory.path().join("commands.jsonl");
        let first = record("command-01", CommandExecutionResult::Success);
        let max_file_size = u64::try_from(serialize_record(first).expect("serializes").len())
            .expect("line length fits u64");
        let adapter = adapter(output_path.clone(), max_file_size, 2);

        for command in ["command-01", "command-02", "command-03", "command-04"] {
            adapter
                .record(record(command, CommandExecutionResult::Success))
                .expect("record writes");
        }

        assert!(
            fs::read_to_string(&output_path).expect("active file reads").contains("command-04")
        );
        assert!(
            fs::read_to_string(rotated_path(&output_path, 1))
                .expect("newest rotated file reads")
                .contains("command-03")
        );
        assert!(
            fs::read_to_string(rotated_path(&output_path, 2))
                .expect("oldest retained file reads")
                .contains("command-02")
        );
        assert!(!rotated_path(&output_path, 3).exists());
    }

    #[test]
    fn test_fs_command_trace_adapter_record_reduced_retention_prunes_stale_oldest_generation() {
        let directory = TempDir::new().expect("temporary directory");
        let output_path = directory.path().join("commands.jsonl");
        fs::write(&output_path, "previous-active\n").expect("active file writes");
        fs::write(rotated_path(&output_path, 1), "newest\n").expect("newest generation writes");
        fs::write(rotated_path(&output_path, 2), "middle\n").expect("middle generation writes");
        fs::write(rotated_path(&output_path, 3), "oldest\n").expect("oldest generation writes");
        let adapter = adapter(output_path.clone(), 1, 2);

        adapter
            .record(record("command-04", CommandExecutionResult::Success))
            .expect("record writes after rotation");

        assert!(
            fs::read_to_string(rotated_path(&output_path, 1))
                .expect("newest rotated file reads")
                .contains("previous-active")
        );
        assert!(
            fs::read_to_string(rotated_path(&output_path, 2))
                .expect("oldest retained file reads")
                .contains("newest")
        );
        assert!(!rotated_path(&output_path, 3).exists());
    }

    #[test]
    fn test_fs_command_trace_adapter_record_prunes_gapped_stale_generation() {
        let directory = TempDir::new().expect("temporary directory");
        let output_path = directory.path().join("commands.jsonl");
        fs::write(&output_path, "previous-active\n").expect("active file writes");
        fs::write(rotated_path(&output_path, 1), "newest\n").expect("newest generation writes");
        fs::write(rotated_path(&output_path, 2), "oldest\n").expect("oldest generation writes");
        fs::write(rotated_path(&output_path, 4), "stale\n").expect("gapped generation writes");
        let adapter = adapter(output_path.clone(), 1, 2);

        adapter
            .record(record("command-04", CommandExecutionResult::Success))
            .expect("record writes after rotation");

        assert!(
            fs::read_to_string(rotated_path(&output_path, 1))
                .expect("newest rotated file reads")
                .contains("previous-active")
        );
        assert!(
            fs::read_to_string(rotated_path(&output_path, 2))
                .expect("oldest retained file reads")
                .contains("newest")
        );
        assert!(!rotated_path(&output_path, 3).exists());
        assert!(!rotated_path(&output_path, 4).exists());
    }

    #[test]
    fn test_fs_command_trace_adapter_record_waits_for_existing_writer_lock() {
        let directory = TempDir::new().expect("temporary directory");
        let output_path = directory.path().join("commands.jsonl");
        let trace_adapter = adapter(output_path.clone(), 4_096, 3);
        let lock = trace_adapter.acquire_write_lock().expect("writer lock acquires");
        let (started_tx, started_rx) = mpsc::channel();
        let (result_tx, result_rx) = mpsc::channel();
        let waiting_adapter = adapter(output_path.clone(), 4_096, 3);

        let writer = thread::spawn(move || {
            started_tx.send(()).expect("writer start signal sends");
            result_tx
                .send(waiting_adapter.record(record("track plan", CommandExecutionResult::Success)))
                .expect("writer result sends");
        });

        started_rx.recv().expect("writer start signal receives");
        assert!(result_rx.recv_timeout(Duration::from_millis(100)).is_err());
        drop(lock);
        result_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("writer unblocks after lock releases")
            .expect("writer record succeeds");
        writer.join().expect("writer thread joins");
        assert!(output_path.exists());
    }
}
