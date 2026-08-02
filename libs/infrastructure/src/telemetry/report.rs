//! `TelemetryReport` — secondary adapter that reads and aggregates
//! `track/items/<id>/logs/telemetry.jsonl` into a `TelemetryReportSnapshot`.
//!
//! Fail-open line skipping: broken JSON lines and lines with an unknown
//! `schema_version` are counted in `skipped_lines` but never cause an error
//! (CN-04).

use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};

use domain::TrackId;
use serde::Deserialize;
use thiserror::Error;
use usecase::telemetry::command_trace::{
    CommandDurationMillis, CommandExecutionCount, CommandExecutionMetric, SotpCommandIdentity,
    TelemetrySkippedLineCount,
};

use crate::telemetry::TelemetryEvent;
use crate::telemetry::report_command_trace::decode;
use crate::track::symlink_guard::reject_symlinks_below;

// ---------------------------------------------------------------------------
// Output DTOs
// ---------------------------------------------------------------------------

/// Infrastructure read model for an aggregated telemetry report.
///
/// Carries phase-by-phase duration summary, error list, hook block list, and
/// the count of lines skipped due to parse failures or unknown `schema_version`
/// (fail-open per CN-04).
#[derive(Debug, Clone)]
pub struct TelemetryReportSnapshot {
    /// Per-phase aggregated duration, derived from `TrackSubcommand` events.
    pub phase_durations: Vec<PhaseDurationSummary>,
    /// Non-zero exit events projected from `NonZeroExit` JSONL events.
    pub errors: Vec<TelemetryErrorEntry>,
    /// Hook block events projected from `HookBlock` JSONL events.
    pub hook_blocks: Vec<TelemetryHookBlockEntry>,
    /// Number of lines skipped (broken JSON or unknown `schema_version`).
    pub skipped_lines: TelemetrySkippedLineCount,
    /// Per-command execution metrics parsed from persisted command records.
    pub command_metrics: Vec<CommandExecutionMetric>,
}

/// Per-phase aggregated duration in the telemetry report.
///
/// `phase_name` is a free-form label derived from the `command` field of
/// `TrackSubcommand` events (e.g. `"track spec-design"`, `"track type-design"`).
/// Raw `String` is justified: phase names are open-ended identifiers with no
/// domain-level finite set or validation constraint at the report aggregation
/// boundary.
#[derive(Debug, Clone)]
pub struct PhaseDurationSummary {
    /// Free-form phase label taken from `TrackSubcommand.command`.
    pub phase_name: String,
    /// Sum of `duration_ms` values across all events for this phase.
    pub total_ms: u64,
    /// Number of `TrackSubcommand` events aggregated into this entry.
    pub event_count: u32,
}

/// Single non-zero exit event entry in the telemetry report's error list.
///
/// Projected from `NonZeroExit` events in the JSONL log.
#[derive(Debug, Clone)]
pub struct TelemetryErrorEntry {
    /// ISO-8601 timestamp of the event.
    pub timestamp: String,
    /// Subcommand name from `NonZeroExit.command`.
    pub command: String,
    /// Non-zero exit code.
    pub exit_code: i32,
    /// Human-readable error chain (may be truncated; see `TelemetryWriter`).
    pub error_chain: String,
}

/// Single hook block event entry in the telemetry report's hook block list.
///
/// Projected from `HookBlock` events in the JSONL log.
#[derive(Debug, Clone)]
pub struct TelemetryHookBlockEntry {
    /// ISO-8601 timestamp of the event.
    pub timestamp: String,
    /// Hook identifier that triggered the block.
    pub hook_name: String,
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Failure modes of `TelemetryReport::aggregate`.
///
/// `Io` covers file-system read failures. `TrackNotFound` is returned when the
/// requested track directory does not exist. Parse failures on individual lines
/// are **not** errors; they are absorbed by the fail-open skipping logic
/// (CN-04) and counted in `TelemetryReportSnapshot.skipped_lines`.
#[derive(Debug, Error)]
pub enum TelemetryReportError {
    /// A filesystem I/O error occurred while reading the JSONL file.
    #[error("telemetry I/O error reading {path}: {message}")]
    Io {
        /// Filesystem path that caused the error.
        path: String,
        /// Underlying I/O error message.
        message: String,
    },

    /// The requested track directory does not exist under `items_dir`.
    #[error("track not found: {track_id}")]
    TrackNotFound {
        /// The track identifier that was not found.
        track_id: String,
    },
}

// ---------------------------------------------------------------------------
// Known schema versions
// ---------------------------------------------------------------------------

/// The set of `schema_version` values this reader understands.
const KNOWN_SCHEMA_VERSIONS: &[u32] = &[1];

/// Largest JSONL record accepted by the report reader; bounds corrupted or attacker input.
const MAX_TELEMETRY_RECORD_BYTES: usize = 64 * 1024;

/// The maximum amount of telemetry input accepted by one aggregation. Together
/// with the per-record limit, this bounds all report collections because every
/// retained output entry originates from at least one accepted input record.
const MAX_TELEMETRY_INPUT_BYTES: usize = 1024 * 1024;

/// One active file plus positive `u16` rotation suffixes bounds directory traversal and memory.
const MAX_TELEMETRY_LOG_DIRECTORY_ENTRIES: usize = u16::MAX as usize + 1;

/// The version envelope is decoded before a full event payload.  This lets the
/// reader reject future versions without accidentally accepting their fields
/// through the current typed DTO.
#[derive(Debug, Deserialize)]
struct TelemetrySchemaVersionEnvelope {
    schema_version: u32,
}

enum BoundedLineRead {
    EndOfFile,
    Line,
    Oversized,
    InputBudgetExhausted,
}

/// Result of attempting to join the writer's lock protocol without mutating a
/// read-only telemetry directory.
enum ReadLock {
    Present(File),
    Absent,
}

fn is_known_schema_version(v: u32) -> bool {
    KNOWN_SCHEMA_VERSIONS.contains(&v)
}

// ---------------------------------------------------------------------------
// TelemetryReport
// ---------------------------------------------------------------------------

/// Reads and aggregates `telemetry.jsonl` for a given track-id to produce a
/// `TelemetryReportSnapshot`.
///
/// Implements fail-open line skipping: broken JSON lines and lines with an
/// unknown `schema_version` are counted but not failed on per CN-04.
/// Private fields: `items_dir` path.
#[derive(Debug)]
pub struct TelemetryReport {
    items_dir: PathBuf,
}

impl TelemetryReport {
    /// Create a new `TelemetryReport` that reads from the given `items_dir`
    /// (e.g. `track/items`).
    pub fn new(items_dir: PathBuf) -> Self {
        Self { items_dir }
    }

    /// Aggregate telemetry events for `track_id` from its JSONL log.
    ///
    /// Returns `TelemetryReportError::TrackNotFound` if the track directory
    /// does not exist. Returns an empty `TelemetryReportSnapshot` (with
    /// `skipped_lines=0`) if the log file does not exist — this is the normal
    /// state before any subcommands have been run for the track.
    ///
    /// # Errors
    /// Returns `TelemetryReportError::TrackNotFound` when the track directory
    /// is absent. Returns `TelemetryReportError::Io` on filesystem failures or
    /// when a complete report would exceed the bounded input budget.
    pub fn aggregate(
        &self,
        track_id: &TrackId,
    ) -> Result<TelemetryReportSnapshot, TelemetryReportError> {
        self.aggregate_with_lock_retry(track_id, true)
    }

    /// Performs one aggregation, retrying once when a writer creates its lock
    /// while the legacy lockless compatibility path is reading.
    fn aggregate_with_lock_retry(
        &self,
        track_id: &TrackId,
        retry_lockless_read: bool,
    ) -> Result<TelemetryReportSnapshot, TelemetryReportError> {
        let track_id_text = track_id.as_ref();
        let track_dir = self.items_dir.join(track_id_text);

        if !self.guard_path(&track_dir)? {
            return Err(TelemetryReportError::TrackNotFound { track_id: track_id_text.to_owned() });
        }

        match std::fs::symlink_metadata(&track_dir) {
            Ok(metadata) if metadata.is_dir() => {}
            Ok(_) => {
                return Err(TelemetryReportError::Io {
                    path: track_dir.display().to_string(),
                    message: "not a directory".to_owned(),
                });
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                return Err(TelemetryReportError::TrackNotFound {
                    track_id: track_id_text.to_owned(),
                });
            }
            Err(e) => {
                return Err(TelemetryReportError::Io {
                    path: track_dir.display().to_string(),
                    message: e.to_string(),
                });
            }
        }

        let logs_dir = track_dir.join("logs");
        // Missing log file is a normal state (no events written yet) — return
        // empty output (CN-04 / fail-open).
        let empty_output = || TelemetryReportSnapshot {
            phase_durations: Vec::new(),
            errors: Vec::new(),
            hook_blocks: Vec::new(),
            skipped_lines: TelemetrySkippedLineCount::from(0),
            command_metrics: Vec::new(),
        };

        if !self.guard_path(&logs_dir)? {
            return Ok(empty_output());
        }
        match std::fs::symlink_metadata(&logs_dir) {
            Ok(metadata) if metadata.is_dir() => {}
            Ok(_) => {
                return Err(TelemetryReportError::Io {
                    path: logs_dir.display().to_string(),
                    message: "not a directory".to_owned(),
                });
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(empty_output()),
            Err(e) => {
                return Err(TelemetryReportError::Io {
                    path: logs_dir.display().to_string(),
                    message: e.to_string(),
                });
            }
        }

        let lock = self.acquire_read_lock(&logs_dir)?;
        let lock_was_absent = matches!(&lock, ReadLock::Absent);
        let _lock = match lock {
            ReadLock::Present(file) => Some(file),
            ReadLock::Absent => None,
        };
        let aggregation_result = (|| {
            let log_paths = self.telemetry_log_paths(&logs_dir)?;
            if log_paths.is_empty() {
                return Ok(empty_output());
            }

            // Accumulators.
            let mut phase_map: HashMap<String, (u64, u32)> = HashMap::new(); // name -> (total_ms, count)
            let mut errors: Vec<TelemetryErrorEntry> = Vec::new();
            let mut hook_blocks: Vec<TelemetryHookBlockEntry> = Vec::new();
            let mut command_map: HashMap<SotpCommandIdentity, (u64, u64, u64)> = HashMap::new();
            let mut skipped_lines: u64 = 0;
            let mut remaining_input_bytes = MAX_TELEMETRY_INPUT_BYTES;

            for log_path in log_paths {
                let file =
                    open_read_no_follow(&log_path).map_err(|e| TelemetryReportError::Io {
                        path: log_path.display().to_string(),
                        message: e.to_string(),
                    })?;
                let mut reader = io::BufReader::new(file);
                let mut line = Vec::new();

                loop {
                    line.clear();
                    match read_bounded_line(&mut reader, &mut line, &mut remaining_input_bytes)
                        .map_err(|e| TelemetryReportError::Io {
                            path: log_path.display().to_string(),
                            message: e.to_string(),
                        })? {
                        BoundedLineRead::EndOfFile => break,
                        BoundedLineRead::InputBudgetExhausted => {
                            return Err(TelemetryReportError::Io {
                                path: log_path.display().to_string(),
                                message: format!(
                                    "telemetry report input exceeds the {MAX_TELEMETRY_INPUT_BYTES}-byte limit"
                                ),
                            });
                        }
                        BoundedLineRead::Oversized => {
                            skipped_lines = skipped_lines.saturating_add(1);
                            continue;
                        }
                        BoundedLineRead::Line => {}
                    }

                    if line.iter().all(|b| b.is_ascii_whitespace()) {
                        skipped_lines = skipped_lines.saturating_add(1);
                        continue;
                    }

                    if matches!(
                        serde_json::from_slice::<TelemetrySchemaVersionEnvelope>(&line),
                        Ok(envelope) if !is_known_schema_version(envelope.schema_version)
                    ) {
                        skipped_lines = skipped_lines.saturating_add(1);
                        continue;
                    }

                    match serde_json::from_slice::<TelemetryEvent>(&line) {
                        Ok(event) => match event {
                            TelemetryEvent::TrackSubcommand { command, duration_ms, .. } => {
                                let entry = phase_map.entry(command).or_insert((0, 0));
                                entry.0 = entry.0.saturating_add(duration_ms);
                                entry.1 = entry.1.saturating_add(1);
                            }
                            TelemetryEvent::NonZeroExit {
                                timestamp,
                                command,
                                exit_code,
                                error_chain,
                                ..
                            } => {
                                errors.push(TelemetryErrorEntry {
                                    timestamp,
                                    command,
                                    exit_code,
                                    error_chain,
                                });
                            }
                            TelemetryEvent::HookBlock { timestamp, hook_name, .. } => {
                                hook_blocks.push(TelemetryHookBlockEntry { timestamp, hook_name });
                            }
                            _ => {}
                        },
                        Err(_) => match decode(&line, KNOWN_SCHEMA_VERSIONS) {
                            Some(record) => {
                                let entry = command_map.entry(record.command).or_insert((0, 0, 0));
                                entry.0 = entry.0.saturating_add(1);
                                entry.1 = entry.1.saturating_add(u64::from(record.failed));
                                entry.2 = entry.2.saturating_add(record.duration_ms);
                            }
                            None => skipped_lines = skipped_lines.saturating_add(1),
                        },
                    }
                }
            }

            // Convert phase_map to sorted Vec<PhaseDurationSummary>.
            let mut phase_durations: Vec<PhaseDurationSummary> = phase_map
                .into_iter()
                .map(|(phase_name, (total_ms, event_count))| PhaseDurationSummary {
                    phase_name,
                    total_ms,
                    event_count,
                })
                .collect();
            phase_durations.sort_by(|a, b| a.phase_name.cmp(&b.phase_name));

            let mut command_metrics = Vec::with_capacity(command_map.len());
            for (command, (executions, failures, total_duration)) in command_map {
                match CommandExecutionMetric::new(
                    command,
                    CommandExecutionCount::from(executions),
                    CommandExecutionCount::from(failures),
                    CommandDurationMillis::from(total_duration),
                ) {
                    Ok(metric) => command_metrics.push(metric),
                    Err(_) => skipped_lines = skipped_lines.saturating_add(executions),
                }
            }
            command_metrics
                .sort_by(|left, right| left.command().as_str().cmp(right.command().as_str()));

            let snapshot = TelemetryReportSnapshot {
                phase_durations,
                errors,
                hook_blocks,
                skipped_lines: TelemetrySkippedLineCount::from(skipped_lines),
                command_metrics,
            };

            Ok(snapshot)
        })();

        // A writer creates and retains this lock before rotating files. Legacy
        // read-only archives legitimately lack it, but if it appeared during a
        // lockless read, discard both successful and failed mixed-generation
        // attempts and retry under the writer's shared-lock protocol.
        if lock_was_absent && self.lock_file_exists(&logs_dir)? {
            if retry_lockless_read {
                return self.aggregate_with_lock_retry(track_id, false);
            }
            return Err(TelemetryReportError::Io {
                path: logs_dir.join("telemetry.jsonl.lock").display().to_string(),
                message: "telemetry lock changed during lockless report aggregation".to_owned(),
            });
        }

        aggregation_result
    }

    fn guard_path(&self, path: &Path) -> Result<bool, TelemetryReportError> {
        reject_symlinks_below(path, &self.items_dir).map_err(|e| TelemetryReportError::Io {
            path: path.display().to_string(),
            message: e.to_string(),
        })
    }

    fn acquire_read_lock(&self, logs_dir: &Path) -> Result<ReadLock, TelemetryReportError> {
        let lock_path = logs_dir.join("telemetry.jsonl.lock");
        self.guard_path(&lock_path)?;

        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;

            options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK);
        }
        let file = match options.open(&lock_path) {
            Ok(file) => file,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(ReadLock::Absent),
            Err(e) => {
                return Err(TelemetryReportError::Io {
                    path: lock_path.display().to_string(),
                    message: e.to_string(),
                });
            }
        };
        let metadata = file.metadata().map_err(|e| TelemetryReportError::Io {
            path: lock_path.display().to_string(),
            message: e.to_string(),
        })?;
        if !metadata.is_file() {
            return Err(TelemetryReportError::Io {
                path: lock_path.display().to_string(),
                message: "not a regular file".to_owned(),
            });
        }
        fs4::fs_std::FileExt::lock_shared(&file).map_err(|e| TelemetryReportError::Io {
            path: lock_path.display().to_string(),
            message: e.to_string(),
        })?;
        Ok(ReadLock::Present(file))
    }

    fn lock_file_exists(&self, logs_dir: &Path) -> Result<bool, TelemetryReportError> {
        let lock_path = logs_dir.join("telemetry.jsonl.lock");
        self.guard_path(&lock_path)?;
        match std::fs::symlink_metadata(&lock_path) {
            Ok(_) => Ok(true),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(TelemetryReportError::Io {
                path: lock_path.display().to_string(),
                message: error.to_string(),
            }),
        }
    }

    fn telemetry_log_paths(&self, logs_dir: &Path) -> Result<Vec<PathBuf>, TelemetryReportError> {
        let entries = std::fs::read_dir(logs_dir).map_err(|e| TelemetryReportError::Io {
            path: logs_dir.display().to_string(),
            message: e.to_string(),
        })?;
        let mut active = None;
        let mut rotated = Vec::new();

        for (entry_count, entry) in entries.enumerate() {
            if entry_count >= MAX_TELEMETRY_LOG_DIRECTORY_ENTRIES {
                return Err(TelemetryReportError::Io {
                    path: logs_dir.display().to_string(),
                    message: "too many telemetry log directory entries".to_owned(),
                });
            }
            let entry = entry.map_err(|e| TelemetryReportError::Io {
                path: logs_dir.display().to_string(),
                message: e.to_string(),
            })?;
            let path = entry.path();
            let file_name = entry.file_name();
            if file_name == "telemetry.jsonl" {
                active = Some(path);
            } else if let Some(generation) = file_name
                .to_str()
                .and_then(|name| name.strip_prefix("telemetry.jsonl."))
                .and_then(|suffix| suffix.parse::<u16>().ok())
                .filter(|generation| *generation > 0)
            {
                rotated.push((generation, path));
            }
        }

        rotated.sort_by(|(left, _), (right, _)| right.cmp(left));
        let mut paths: Vec<PathBuf> = rotated.into_iter().map(|(_, path)| path).collect();
        if let Some(active) = active {
            paths.push(active);
        }
        for path in &paths {
            self.guard_path(path)?;
            match std::fs::symlink_metadata(path) {
                Ok(metadata) if metadata.is_file() => {}
                Ok(_) => {
                    return Err(TelemetryReportError::Io {
                        path: path.display().to_string(),
                        message: "not a file".to_owned(),
                    });
                }
                Err(e) => {
                    return Err(TelemetryReportError::Io {
                        path: path.display().to_string(),
                        message: e.to_string(),
                    });
                }
            }
        }
        Ok(paths)
    }
}

fn read_bounded_line<R: BufRead>(
    reader: &mut R,
    line: &mut Vec<u8>,
    remaining_input_bytes: &mut usize,
) -> io::Result<BoundedLineRead> {
    let mut saw_bytes = false;
    let mut oversized = false;

    loop {
        let buffer = reader.fill_buf()?;
        if buffer.is_empty() {
            return Ok(if saw_bytes {
                if oversized { BoundedLineRead::Oversized } else { BoundedLineRead::Line }
            } else {
                BoundedLineRead::EndOfFile
            });
        }
        if *remaining_input_bytes == 0 {
            return Ok(BoundedLineRead::InputBudgetExhausted);
        }

        let permitted = buffer.len().min(*remaining_input_bytes);
        let available = buffer.get(..permitted).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "invalid telemetry reader buffer range")
        })?;
        let (consumed, has_newline) = match available.iter().position(|byte| *byte == b'\n') {
            Some(position) => (position.saturating_add(1), true),
            None => (available.len(), false),
        };
        saw_bytes = true;
        if !oversized && line.len().saturating_add(consumed) <= MAX_TELEMETRY_RECORD_BYTES {
            let bytes = buffer.get(..consumed).ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "invalid telemetry reader buffer range")
            })?;
            line.extend_from_slice(bytes);
        } else {
            oversized = true;
        }
        reader.consume(consumed);
        *remaining_input_bytes = remaining_input_bytes.saturating_sub(consumed);
        if has_newline {
            return Ok(if oversized { BoundedLineRead::Oversized } else { BoundedLineRead::Line });
        }
    }
}

fn open_read_no_follow(path: &Path) -> io::Result<std::fs::File> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    options.open(path)
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn write_jsonl<L>(dir: &std::path::Path, track_id: &str, lines: &[L])
    where
        L: AsRef<[u8]>,
    {
        let logs_dir = dir.join(track_id).join("logs");
        std::fs::create_dir_all(&logs_dir).unwrap();
        let mut file = std::fs::File::create(logs_dir.join("telemetry.jsonl")).unwrap();
        for line in lines {
            file.write_all(line.as_ref()).unwrap();
            file.write_all(b"\n").unwrap();
        }
    }

    fn write_rotated_jsonl<L>(dir: &std::path::Path, track_id: &str, generation: u64, lines: &[L])
    where
        L: AsRef<[u8]>,
    {
        let logs_dir = dir.join(track_id).join("logs");
        std::fs::create_dir_all(&logs_dir).unwrap();
        let mut file =
            std::fs::File::create(logs_dir.join(format!("telemetry.jsonl.{generation}"))).unwrap();
        for line in lines {
            file.write_all(line.as_ref()).unwrap();
            file.write_all(b"\n").unwrap();
        }
    }

    fn make_track_dir(dir: &std::path::Path, track_id: &str) {
        std::fs::create_dir_all(dir.join(track_id)).unwrap();
    }

    fn track_id(value: &str) -> TrackId {
        TrackId::try_new(value.to_owned()).unwrap()
    }

    const SUBCOMMAND_LINE: &str = r#"{"event_type":"TrackSubcommand","schema_version":1,"track_id":"t","command":"track spec-design","exit_code":0,"duration_ms":1200,"timestamp":"2026-06-10T00:00:00Z"}"#;
    const NON_ZERO_EXIT_LINE: &str = r#"{"event_type":"NonZeroExit","schema_version":1,"track_id":"t","command":"track spec-design","exit_code":1,"error_chain":"gate failed","timestamp":"2026-06-10T01:00:00Z"}"#;
    const HOOK_BLOCK_LINE: &str = r#"{"event_type":"HookBlock","schema_version":1,"track_id":"t","hook_name":"block-direct-git-ops","timestamp":"2026-06-10T02:00:00Z"}"#;
    const COMMAND_SUCCESS_LINE: &str =
        r#"{"command":"track plan","duration_ms":120,"result":{"status":"success"}}"#;
    const COMMAND_FAILURE_LINE: &str =
        r#"{"command":"track plan","duration_ms":80,"result":{"status":"failure","exit_code":17}}"#;

    /// Happy path: aggregate collects phase durations, errors, and hook blocks.
    #[test]
    fn test_aggregate_happy_path() {
        let tmp = TempDir::new().unwrap();
        write_jsonl(tmp.path(), "t", &[SUBCOMMAND_LINE, NON_ZERO_EXIT_LINE, HOOK_BLOCK_LINE]);

        let report = TelemetryReport::new(tmp.path().to_path_buf());
        let output = report.aggregate(&track_id("t")).unwrap();

        assert_eq!(*output.skipped_lines.as_ref(), 0);
        assert_eq!(output.phase_durations.len(), 1);
        let pd = output.phase_durations.first().unwrap();
        assert_eq!(pd.phase_name, "track spec-design");
        assert_eq!(pd.total_ms, 1200);
        assert_eq!(pd.event_count, 1);

        assert_eq!(output.errors.len(), 1);
        let err_entry = output.errors.first().unwrap();
        assert_eq!(err_entry.command, "track spec-design");
        assert_eq!(err_entry.exit_code, 1);
        assert_eq!(err_entry.error_chain, "gate failed");

        assert_eq!(output.hook_blocks.len(), 1);
        let hb = output.hook_blocks.first().unwrap();
        assert_eq!(hb.hook_name, "block-direct-git-ops");
    }

    /// Multiple TrackSubcommand events for the same command are accumulated.
    #[test]
    fn test_aggregate_accumulates_phase_durations() {
        let line2 = r#"{"event_type":"TrackSubcommand","schema_version":1,"track_id":"t","command":"track spec-design","exit_code":0,"duration_ms":800,"timestamp":"2026-06-10T00:01:00Z"}"#;
        let tmp = TempDir::new().unwrap();
        write_jsonl(tmp.path(), "t", &[SUBCOMMAND_LINE, line2]);

        let report = TelemetryReport::new(tmp.path().to_path_buf());
        let output = report.aggregate(&track_id("t")).unwrap();

        assert_eq!(output.phase_durations.len(), 1);
        let pd = output.phase_durations.first().unwrap();
        assert_eq!(pd.total_ms, 2000); // 1200 + 800
        assert_eq!(pd.event_count, 2);
    }

    #[test]
    fn test_aggregate_persisted_command_records_produces_typed_metrics() {
        let tmp = TempDir::new().unwrap();
        write_jsonl(
            tmp.path(),
            "t",
            &[COMMAND_SUCCESS_LINE, COMMAND_SUCCESS_LINE, COMMAND_FAILURE_LINE],
        );

        let report = TelemetryReport::new(tmp.path().to_path_buf());
        let output = report.aggregate(&track_id("t")).unwrap();

        assert_eq!(output.command_metrics.len(), 1);
        let metric = output.command_metrics.first().unwrap();
        assert_eq!(metric.command().as_str(), "track plan");
        assert_eq!(*metric.executions().as_ref(), 3);
        assert_eq!(*metric.failures().as_ref(), 1);
        assert_eq!(*metric.total_duration().as_ref(), 320);
        assert_eq!(metric.failure_rate().value(), 3_333);
    }

    #[test]
    fn test_aggregate_malformed_command_record_is_skipped_and_counted() {
        let malformed_command = r#"{"command":"","duration_ms":80,"result":{"status":"success"}}"#;
        let tmp = TempDir::new().unwrap();
        write_jsonl(tmp.path(), "t", &[COMMAND_SUCCESS_LINE, malformed_command]);

        let report = TelemetryReport::new(tmp.path().to_path_buf());
        let output = report.aggregate(&track_id("t")).unwrap();

        assert_eq!(*output.skipped_lines.as_ref(), 1);
        assert_eq!(output.command_metrics.len(), 1);
    }

    #[test]
    fn test_aggregate_out_of_range_command_exit_code_is_skipped_and_counted() {
        let out_of_range_failure = r#"{"command":"track plan","duration_ms":80,"result":{"status":"failure","exit_code":256}}"#;
        let tmp = TempDir::new().unwrap();
        write_jsonl(tmp.path(), "t", &[COMMAND_SUCCESS_LINE, out_of_range_failure]);

        let report = TelemetryReport::new(tmp.path().to_path_buf());
        let output = report.aggregate(&track_id("t")).unwrap();

        assert_eq!(*output.skipped_lines.as_ref(), 1);
        assert_eq!(output.command_metrics.len(), 1);
    }

    #[test]
    fn test_aggregate_unknown_schema_command_record_is_skipped_and_counted() {
        let unknown_schema_command = r#"{"schema_version":999,"command":"track plan","duration_ms":80,"result":{"status":"success"}}"#;
        let tmp = TempDir::new().unwrap();
        write_jsonl(tmp.path(), "t", &[COMMAND_SUCCESS_LINE, unknown_schema_command]);

        let report = TelemetryReport::new(tmp.path().to_path_buf());
        let output = report.aggregate(&track_id("t")).unwrap();

        assert_eq!(*output.skipped_lines.as_ref(), 1);
        assert_eq!(output.command_metrics.len(), 1);
        let metric = output.command_metrics.first().unwrap();
        assert_eq!(*metric.executions().as_ref(), 1);
    }

    #[test]
    fn test_aggregate_null_command_schema_version_is_skipped_and_counted() {
        let null_schema_command = r#"{"schema_version":null,"command":"track plan","duration_ms":80,"result":{"status":"success"}}"#;
        let tmp = TempDir::new().unwrap();
        write_jsonl(tmp.path(), "t", &[COMMAND_SUCCESS_LINE, null_schema_command]);

        let report = TelemetryReport::new(tmp.path().to_path_buf());
        let output = report.aggregate(&track_id("t")).unwrap();

        assert_eq!(*output.skipped_lines.as_ref(), 1);
        assert_eq!(output.command_metrics.len(), 1);
        let metric = output.command_metrics.first().unwrap();
        assert_eq!(*metric.executions().as_ref(), 1);
    }

    #[test]
    fn test_aggregate_future_command_record_fields_are_skipped_and_counted() {
        let future_command = r#"{"schema_version":1,"command":"track plan","duration_ms":80,"result":{"status":"success"},"new_field":"future value"}"#;
        let tmp = TempDir::new().unwrap();
        write_jsonl(tmp.path(), "t", &[COMMAND_SUCCESS_LINE, future_command]);

        let report = TelemetryReport::new(tmp.path().to_path_buf());
        let output = report.aggregate(&track_id("t")).unwrap();

        assert_eq!(*output.skipped_lines.as_ref(), 1);
        assert_eq!(output.command_metrics.len(), 1);
        let metric = output.command_metrics.first().unwrap();
        assert_eq!(*metric.executions().as_ref(), 1);
    }

    #[test]
    fn test_aggregate_future_command_result_fields_are_skipped_and_counted() {
        let future_result = r#"{"schema_version":1,"command":"track plan","duration_ms":80,"result":{"status":"success","new_field":"future value"}}"#;
        let tmp = TempDir::new().unwrap();
        write_jsonl(tmp.path(), "t", &[COMMAND_SUCCESS_LINE, future_result]);

        let report = TelemetryReport::new(tmp.path().to_path_buf());
        let output = report.aggregate(&track_id("t")).unwrap();

        assert_eq!(*output.skipped_lines.as_ref(), 1);
        assert_eq!(output.command_metrics.len(), 1);
        let metric = output.command_metrics.first().unwrap();
        assert_eq!(*metric.executions().as_ref(), 1);
    }

    /// Corrupted JSON lines are skipped and counted in skipped_lines.
    #[test]
    fn test_aggregate_corrupted_line_is_skipped_and_counted() {
        let tmp = TempDir::new().unwrap();
        write_jsonl(
            tmp.path(),
            "t",
            &[SUBCOMMAND_LINE, "not valid json at all", "{broken", NON_ZERO_EXIT_LINE],
        );

        let report = TelemetryReport::new(tmp.path().to_path_buf());
        let output = report.aggregate(&track_id("t")).unwrap();

        assert_eq!(*output.skipped_lines.as_ref(), 2, "two broken lines must be counted");
        assert_eq!(output.phase_durations.len(), 1);
        assert_eq!(output.errors.len(), 1);
    }

    /// Non-UTF-8 corrupted lines are skipped and counted, not returned as I/O errors.
    #[test]
    fn test_aggregate_non_utf8_corrupted_line_is_skipped_and_counted() {
        let tmp = TempDir::new().unwrap();
        write_jsonl(
            tmp.path(),
            "t",
            &[
                SUBCOMMAND_LINE.as_bytes(),
                &b"{\"event_type\":\"TrackSubcommand\",\xff}"[..],
                NON_ZERO_EXIT_LINE.as_bytes(),
            ],
        );

        let report = TelemetryReport::new(tmp.path().to_path_buf());
        let output = report.aggregate(&track_id("t")).unwrap();

        assert_eq!(*output.skipped_lines.as_ref(), 1, "non-UTF-8 broken JSON must be counted");
        assert_eq!(output.phase_durations.len(), 1);
        assert_eq!(output.errors.len(), 1);
    }

    /// Lines with an unknown schema_version are skipped (CN-08 / AC-09).
    #[test]
    fn test_aggregate_unknown_schema_version_is_skipped() {
        // A structurally valid TrackSubcommand but with schema_version = 999.
        let future_line = r#"{"event_type":"TrackSubcommand","schema_version":999,"track_id":"t","command":"track spec-design","exit_code":0,"duration_ms":500,"timestamp":"2026-06-10T00:00:00Z"}"#;
        let tmp = TempDir::new().unwrap();
        write_jsonl(tmp.path(), "t", &[future_line, SUBCOMMAND_LINE]);

        let report = TelemetryReport::new(tmp.path().to_path_buf());
        let output = report.aggregate(&track_id("t")).unwrap();

        assert_eq!(*output.skipped_lines.as_ref(), 1, "future schema_version line must be skipped");
        // The valid line still contributes.
        assert_eq!(output.phase_durations.len(), 1);
        assert_eq!(output.phase_durations.first().unwrap().total_ms, 1200);
    }

    #[test]
    fn test_aggregate_event_with_unknown_field_is_skipped_and_counted() {
        let future_event = r#"{"event_type":"TrackSubcommand","schema_version":1,"track_id":"t","command":"track spec-design","exit_code":0,"duration_ms":500,"timestamp":"2026-06-10T00:00:00Z","future_field":"not accepted"}"#;
        let tmp = TempDir::new().unwrap();
        write_jsonl(tmp.path(), "t", &[future_event, SUBCOMMAND_LINE]);

        let output =
            TelemetryReport::new(tmp.path().to_path_buf()).aggregate(&track_id("t")).unwrap();

        assert_eq!(*output.skipped_lines.as_ref(), 1);
        assert_eq!(output.phase_durations.len(), 1);
        assert_eq!(output.phase_durations.first().unwrap().total_ms, 1200);
    }

    #[test]
    fn test_aggregate_includes_retained_rotated_generations() {
        let tmp = TempDir::new().unwrap();
        write_rotated_jsonl(tmp.path(), "t", 2, &[COMMAND_SUCCESS_LINE]);
        write_rotated_jsonl(tmp.path(), "t", 1, &[COMMAND_FAILURE_LINE]);
        write_jsonl(tmp.path(), "t", &[COMMAND_SUCCESS_LINE]);

        let output =
            TelemetryReport::new(tmp.path().to_path_buf()).aggregate(&track_id("t")).unwrap();

        let metric = output.command_metrics.first().unwrap();
        assert_eq!(*metric.executions().as_ref(), 3);
        assert_eq!(*metric.failures().as_ref(), 1);
        assert_eq!(*metric.total_duration().as_ref(), 320);
    }

    #[test]
    fn test_aggregate_ignores_rotated_generation_outside_writer_domain() {
        let tmp = TempDir::new().unwrap();
        write_rotated_jsonl(tmp.path(), "t", u64::from(u16::MAX) + 1, &[COMMAND_FAILURE_LINE]);
        write_jsonl(tmp.path(), "t", &[COMMAND_SUCCESS_LINE]);

        let output =
            TelemetryReport::new(tmp.path().to_path_buf()).aggregate(&track_id("t")).unwrap();

        let metric = output.command_metrics.first().unwrap();
        assert_eq!(*metric.executions().as_ref(), 1);
        assert_eq!(*metric.failures().as_ref(), 0);
    }

    #[test]
    fn test_aggregate_total_input_budget_returns_typed_failure() {
        let tmp = TempDir::new().unwrap();
        let logs_dir = tmp.path().join("t").join("logs");
        std::fs::create_dir_all(&logs_dir).unwrap();
        let record = format!("{COMMAND_SUCCESS_LINE}\n");
        let record_count = MAX_TELEMETRY_INPUT_BYTES / record.len() + 1;
        let mut input = Vec::with_capacity(record.len() * record_count);
        for _ in 0..record_count {
            input.extend_from_slice(record.as_bytes());
        }
        std::fs::write(logs_dir.join("telemetry.jsonl"), input).unwrap();

        let result = TelemetryReport::new(tmp.path().to_path_buf()).aggregate(&track_id("t"));

        assert!(matches!(
            result,
            Err(TelemetryReportError::Io { message, .. })
                if message.contains("input exceeds the 1048576-byte limit")
        ));
    }

    #[cfg(unix)]
    #[test]
    fn test_aggregate_without_lock_file_reads_read_only_logs_directory() {
        use std::os::unix::fs::PermissionsExt as _;

        let tmp = TempDir::new().unwrap();
        write_jsonl(tmp.path(), "t", &[COMMAND_SUCCESS_LINE]);
        let logs_dir = tmp.path().join("t").join("logs");
        std::fs::set_permissions(&logs_dir, std::fs::Permissions::from_mode(0o555)).unwrap();

        let output =
            TelemetryReport::new(tmp.path().to_path_buf()).aggregate(&track_id("t")).unwrap();

        assert_eq!(output.command_metrics.len(), 1);
        assert!(!logs_dir.join("telemetry.jsonl.lock").exists());
        std::fs::set_permissions(&logs_dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[test]
    fn test_aggregate_non_regular_lock_returns_io_error() {
        let tmp = TempDir::new().unwrap();
        write_jsonl(tmp.path(), "t", &[COMMAND_SUCCESS_LINE]);
        let lock_path = tmp.path().join("t").join("logs").join("telemetry.jsonl.lock");
        std::fs::create_dir(&lock_path).unwrap();

        let result = TelemetryReport::new(tmp.path().to_path_buf()).aggregate(&track_id("t"));

        assert!(matches!(
            result,
            Err(TelemetryReportError::Io { path, message })
                if path.ends_with("telemetry.jsonl.lock") && message == "not a regular file"
        ));
    }

    #[test]
    fn test_aggregate_oversized_record_is_skipped_and_counted() {
        let tmp = TempDir::new().unwrap();
        let logs_dir = tmp.path().join("t").join("logs");
        std::fs::create_dir_all(&logs_dir).unwrap();
        let mut record = vec![b'x'; MAX_TELEMETRY_RECORD_BYTES + 1];
        record.push(b'\n');
        std::fs::write(logs_dir.join("telemetry.jsonl"), record).unwrap();

        let output =
            TelemetryReport::new(tmp.path().to_path_buf()).aggregate(&track_id("t")).unwrap();

        assert_eq!(*output.skipped_lines.as_ref(), 1);
        assert!(output.command_metrics.is_empty());
        assert!(output.phase_durations.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn test_aggregate_symlinked_rotated_generation_returns_io_error() {
        let tmp = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();
        let logs_dir = tmp.path().join("t").join("logs");
        std::fs::create_dir_all(&logs_dir).unwrap();
        std::fs::write(outside.path().join("telemetry.jsonl.1"), COMMAND_SUCCESS_LINE).unwrap();
        std::os::unix::fs::symlink(
            outside.path().join("telemetry.jsonl.1"),
            logs_dir.join("telemetry.jsonl.1"),
        )
        .unwrap();

        let result = TelemetryReport::new(tmp.path().to_path_buf()).aggregate(&track_id("t"));

        assert!(matches!(result, Err(TelemetryReportError::Io { .. })));
    }

    /// Missing telemetry.jsonl for an existing track returns empty output.
    #[test]
    fn test_aggregate_missing_log_file_returns_empty_output() {
        let tmp = TempDir::new().unwrap();
        make_track_dir(tmp.path(), "t");

        let report = TelemetryReport::new(tmp.path().to_path_buf());
        let output = report.aggregate(&track_id("t")).unwrap();

        assert_eq!(*output.skipped_lines.as_ref(), 0);
        assert!(output.phase_durations.is_empty());
        assert!(output.errors.is_empty());
        assert!(output.hook_blocks.is_empty());
    }

    /// Non-existent track_id returns TrackNotFound error.
    #[test]
    fn test_aggregate_nonexistent_track_returns_track_not_found() {
        let tmp = TempDir::new().unwrap();
        let report = TelemetryReport::new(tmp.path().to_path_buf());
        let result = report.aggregate(&track_id("does-not-exist"));

        assert!(
            matches!(result, Err(TelemetryReportError::TrackNotFound { ref track_id }) if track_id == "does-not-exist"),
            "expected TrackNotFound; got: {result:?}"
        );
    }

    /// A non-directory at the track path is reported as an I/O error.
    #[test]
    fn test_aggregate_track_path_file_returns_io_error() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("t"), "not a directory").unwrap();

        let report = TelemetryReport::new(tmp.path().to_path_buf());
        let result = report.aggregate(&track_id("t"));

        assert!(
            matches!(result, Err(TelemetryReportError::Io { ref path, ref message }) if path.ends_with("/t") && message == "not a directory"),
            "expected Io for non-directory track path; got: {result:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_aggregate_symlinked_track_dir_returns_io_error() {
        let tmp = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();
        write_jsonl(outside.path(), "ignored", &[SUBCOMMAND_LINE]);
        std::os::unix::fs::symlink(outside.path().join("ignored"), tmp.path().join("t")).unwrap();

        let report = TelemetryReport::new(tmp.path().to_path_buf());
        let result = report.aggregate(&track_id("t"));

        assert!(
            matches!(result, Err(TelemetryReportError::Io { .. })),
            "symlinked track dir must be rejected before reading telemetry: {result:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_aggregate_symlinked_logs_dir_returns_io_error() {
        let tmp = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();
        make_track_dir(tmp.path(), "t");
        std::fs::create_dir_all(outside.path().join("logs")).unwrap();
        std::os::unix::fs::symlink(outside.path(), tmp.path().join("t").join("logs")).unwrap();

        let report = TelemetryReport::new(tmp.path().to_path_buf());
        let result = report.aggregate(&track_id("t"));

        assert!(
            matches!(result, Err(TelemetryReportError::Io { .. })),
            "symlinked logs dir must be rejected before reading telemetry: {result:?}"
        );
    }

    /// A malformed logs path is reported as I/O instead of empty output.
    #[test]
    fn test_aggregate_logs_path_file_returns_io_error() {
        let tmp = TempDir::new().unwrap();
        make_track_dir(tmp.path(), "t");
        std::fs::write(tmp.path().join("t").join("logs"), "not a directory").unwrap();

        let report = TelemetryReport::new(tmp.path().to_path_buf());
        let result = report.aggregate(&track_id("t"));

        assert!(
            matches!(result, Err(TelemetryReportError::Io { ref path, .. }) if path.ends_with("/t/logs")),
            "expected Io for malformed logs path; got: {result:?}"
        );
    }

    /// Empty lines in the JSONL file are skipped and counted as malformed JSON.
    #[test]
    fn test_aggregate_empty_lines_are_skipped_and_counted() {
        let tmp = TempDir::new().unwrap();
        write_jsonl(tmp.path(), "t", &[SUBCOMMAND_LINE, "", "  ", NON_ZERO_EXIT_LINE]);

        let report = TelemetryReport::new(tmp.path().to_path_buf());
        let output = report.aggregate(&track_id("t")).unwrap();

        assert_eq!(*output.skipped_lines.as_ref(), 2, "blank lines must count as skipped");
    }

    /// TelemetryReportError::TrackNotFound implements Display.
    #[test]
    fn test_report_error_track_not_found_display() {
        let err = TelemetryReportError::TrackNotFound { track_id: "my-track".to_owned() };
        let s = format!("{err}");
        assert!(s.contains("my-track"), "Display must mention track_id; got: {s}");
    }

    /// TelemetryReportError::Io implements Display.
    #[test]
    fn test_report_error_io_display() {
        let err = TelemetryReportError::Io {
            path: "/tmp/x.jsonl".to_owned(),
            message: "permission denied".to_owned(),
        };
        let s = format!("{err}");
        assert!(s.contains("/tmp/x.jsonl"), "Display must mention path; got: {s}");
        assert!(s.contains("permission denied"), "Display must mention message; got: {s}");
    }
}
