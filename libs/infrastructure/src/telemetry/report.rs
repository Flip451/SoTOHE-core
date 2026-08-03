//! `TelemetryReport` — secondary adapter that reads and aggregates
//! `track/items/<id>/logs/telemetry.jsonl` into a `TelemetryReportSnapshot`.
//!
//! Fail-open line skipping: broken JSON lines and lines with an unknown
//! `schema_version` are counted in `skipped_lines` but never cause an error
//! (CN-04).

use std::collections::HashMap;
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
    /// Number of input lines that could not be retained (broken JSON, unknown
    /// `schema_version`, oversized records, or projection-cardinality caps).
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

/// Maximum number of distinct report rows retained per projection. Aggregation
/// continues streaming after the cap, counting omitted rows instead of
/// rejecting an otherwise valid append-only log.
const MAX_TELEMETRY_RETAINED_ENTRIES: usize = 8_192;

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
        self.aggregate_once(track_id)
    }

    /// Performs one bounded snapshot aggregation of the canonical JSONL file.
    fn aggregate_once(
        &self,
        track_id: &TrackId,
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

        (|| {
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
            for log_path in log_paths {
                let file =
                    open_read_no_follow(&log_path).map_err(|e| TelemetryReportError::Io {
                        path: log_path.display().to_string(),
                        message: e.to_string(),
                    })?;
                let mut remaining_snapshot_bytes = file
                    .metadata()
                    .map_err(|e| TelemetryReportError::Io {
                        path: log_path.display().to_string(),
                        message: e.to_string(),
                    })?
                    .len();
                let mut reader = io::BufReader::new(file);
                let mut line = Vec::new();

                loop {
                    line.clear();
                    match read_bounded_line(&mut reader, &mut line, &mut remaining_snapshot_bytes)
                        .map_err(|e| TelemetryReportError::Io {
                        path: log_path.display().to_string(),
                        message: e.to_string(),
                    })? {
                        BoundedLineRead::EndOfFile => break,
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
                            TelemetryEvent::TrackSubcommand {
                                command,
                                duration_ms,
                                exit_code,
                                ..
                            } => {
                                // The pre-instrumentation writer persisted command labels
                                // without the binary name (for example, `track transition`),
                                // while the common CLI boundary now records `sotp track
                                // transition`.  Use the established label as the aggregate key
                                // so a track's history remains one logical metric.
                                let metric_command =
                                    command.strip_prefix("sotp ").unwrap_or(&command);
                                let Ok(identity) =
                                    SotpCommandIdentity::try_new(metric_command.to_owned())
                                else {
                                    skipped_lines = skipped_lines.saturating_add(1);
                                    continue;
                                };
                                let mut projection_truncated = false;

                                if phase_map.contains_key(&command)
                                    || phase_map.len() < MAX_TELEMETRY_RETAINED_ENTRIES
                                {
                                    let entry = phase_map.entry(command.clone()).or_insert((0, 0));
                                    entry.0 = entry.0.saturating_add(duration_ms);
                                    entry.1 = entry.1.saturating_add(1);
                                } else {
                                    projection_truncated = true;
                                }

                                // Command metrics are projected from the same
                                // established TrackSubcommand event that is
                                // already stored in telemetry.jsonl. No
                                // separate command-trace record format is
                                // accepted or written.
                                if command_map.contains_key(&identity)
                                    || command_map.len() < MAX_TELEMETRY_RETAINED_ENTRIES
                                {
                                    let metric = command_map.entry(identity).or_insert((0, 0, 0));
                                    metric.0 = metric.0.saturating_add(1);
                                    if exit_code != 0 {
                                        metric.1 = metric.1.saturating_add(1);
                                    }
                                    metric.2 = metric.2.saturating_add(duration_ms);
                                } else {
                                    projection_truncated = true;
                                }
                                if projection_truncated {
                                    skipped_lines = skipped_lines.saturating_add(1);
                                }
                            }
                            TelemetryEvent::NonZeroExit {
                                timestamp,
                                command,
                                exit_code,
                                error_chain,
                                ..
                            } => {
                                if errors.len() < MAX_TELEMETRY_RETAINED_ENTRIES {
                                    errors.push(TelemetryErrorEntry {
                                        timestamp,
                                        command,
                                        exit_code,
                                        error_chain,
                                    });
                                } else {
                                    skipped_lines = skipped_lines.saturating_add(1);
                                }
                            }
                            TelemetryEvent::HookBlock { timestamp, hook_name, .. } => {
                                if hook_blocks.len() < MAX_TELEMETRY_RETAINED_ENTRIES {
                                    hook_blocks
                                        .push(TelemetryHookBlockEntry { timestamp, hook_name });
                                } else {
                                    skipped_lines = skipped_lines.saturating_add(1);
                                }
                            }
                            _ => {}
                        },
                        Err(_) => skipped_lines = skipped_lines.saturating_add(1),
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
        })()
    }

    fn guard_path(&self, path: &Path) -> Result<bool, TelemetryReportError> {
        reject_symlinks_below(path, &self.items_dir).map_err(|e| TelemetryReportError::Io {
            path: path.display().to_string(),
            message: e.to_string(),
        })
    }

    fn telemetry_log_paths(&self, logs_dir: &Path) -> Result<Vec<PathBuf>, TelemetryReportError> {
        let active = logs_dir.join("telemetry.jsonl");
        self.guard_path(&active)?;
        match std::fs::symlink_metadata(&active) {
            Ok(metadata) if metadata.is_file() => Ok(vec![active]),
            Ok(_) => Err(TelemetryReportError::Io {
                path: active.display().to_string(),
                message: "not a file".to_owned(),
            }),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(error) => Err(TelemetryReportError::Io {
                path: active.display().to_string(),
                message: error.to_string(),
            }),
        }
    }
}

fn read_bounded_line<R: BufRead>(
    reader: &mut R,
    line: &mut Vec<u8>,
    remaining_snapshot_bytes: &mut u64,
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
        if *remaining_snapshot_bytes == 0 {
            return Ok(if saw_bytes {
                if oversized { BoundedLineRead::Oversized } else { BoundedLineRead::Line }
            } else {
                BoundedLineRead::EndOfFile
            });
        }
        let permitted =
            buffer.len().min((*remaining_snapshot_bytes).try_into().unwrap_or(usize::MAX));
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
        *remaining_snapshot_bytes = remaining_snapshot_bytes.saturating_sub(consumed as u64);
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
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use super::*;
    use std::io::{Cursor, Write};
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

    fn make_track_dir(dir: &std::path::Path, track_id: &str) {
        std::fs::create_dir_all(dir.join(track_id)).unwrap();
    }

    fn track_id(value: &str) -> TrackId {
        TrackId::try_new(value.to_owned()).unwrap()
    }

    const SUBCOMMAND_LINE: &str = r#"{"event_type":"TrackSubcommand","schema_version":1,"track_id":"t","command":"track spec-design","exit_code":0,"duration_ms":1200,"timestamp":"2026-06-10T00:00:00Z"}"#;
    const NON_ZERO_EXIT_LINE: &str = r#"{"event_type":"NonZeroExit","schema_version":1,"track_id":"t","command":"track spec-design","exit_code":1,"error_chain":"gate failed","timestamp":"2026-06-10T01:00:00Z"}"#;
    const HOOK_BLOCK_LINE: &str = r#"{"event_type":"HookBlock","schema_version":1,"track_id":"t","hook_name":"block-direct-git-ops","timestamp":"2026-06-10T02:00:00Z"}"#;
    const COMMAND_SUCCESS_LINE: &str = r#"{"event_type":"TrackSubcommand","schema_version":1,"track_id":"t","command":"track plan","exit_code":0,"duration_ms":120,"timestamp":"2026-06-10T00:00:00Z"}"#;
    const COMMAND_FAILURE_LINE: &str = r#"{"event_type":"TrackSubcommand","schema_version":1,"track_id":"t","command":"track plan","exit_code":17,"duration_ms":80,"timestamp":"2026-06-10T00:00:00Z"}"#;

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
        let new_success_line = r#"{"event_type":"TrackSubcommand","schema_version":1,"track_id":"t","command":"sotp track plan","exit_code":0,"duration_ms":40,"timestamp":"2026-06-10T00:00:00Z"}"#;
        let tmp = TempDir::new().unwrap();
        write_jsonl(
            tmp.path(),
            "t",
            &[COMMAND_SUCCESS_LINE, COMMAND_SUCCESS_LINE, COMMAND_FAILURE_LINE, new_success_line],
        );

        let report = TelemetryReport::new(tmp.path().to_path_buf());
        let output = report.aggregate(&track_id("t")).unwrap();

        assert_eq!(output.command_metrics.len(), 1);
        let metric = output.command_metrics.first().unwrap();
        assert_eq!(metric.command().as_str(), "track plan");
        assert_eq!(*metric.executions().as_ref(), 4);
        assert_eq!(*metric.failures().as_ref(), 1);
        assert_eq!(*metric.total_duration().as_ref(), 360);
        assert_eq!(metric.failure_rate().value(), 2_500);
    }

    #[test]
    fn test_aggregate_ignores_noncanonical_rotated_file() {
        let tmp = TempDir::new().unwrap();
        write_jsonl(tmp.path(), "t", &[COMMAND_SUCCESS_LINE]);
        std::fs::write(tmp.path().join("t/logs/telemetry.jsonl.1"), COMMAND_FAILURE_LINE).unwrap();

        let output =
            TelemetryReport::new(tmp.path().to_path_buf()).aggregate(&track_id("t")).unwrap();

        let metric = output.command_metrics.first().unwrap();
        assert_eq!(*metric.executions().as_ref(), 1);
        assert_eq!(*metric.failures().as_ref(), 0);
    }

    #[test]
    fn test_aggregate_streams_append_only_log_beyond_one_megabyte() {
        let tmp = TempDir::new().unwrap();
        let lines = vec![COMMAND_SUCCESS_LINE; 10_000];
        write_jsonl(tmp.path(), "t", &lines);

        let output =
            TelemetryReport::new(tmp.path().to_path_buf()).aggregate(&track_id("t")).unwrap();

        let metric = output.command_metrics.first().unwrap();
        assert_eq!(*metric.executions().as_ref(), 10_000);
        assert_eq!(*metric.failures().as_ref(), 0);
    }

    #[test]
    fn test_aggregate_caps_unique_command_projection_once_per_omitted_record() {
        let tmp = TempDir::new().unwrap();
        let lines = (0..=MAX_TELEMETRY_RETAINED_ENTRIES)
            .map(|index| {
                format!(
                    "{{\"event_type\":\"TrackSubcommand\",\"schema_version\":1,\"track_id\":\"t\",\"command\":\"track command-{index}\",\"exit_code\":0,\"duration_ms\":1,\"timestamp\":\"2026-06-10T00:00:00Z\"}}"
                )
            })
            .collect::<Vec<_>>();
        write_jsonl(tmp.path(), "t", &lines);

        let output =
            TelemetryReport::new(tmp.path().to_path_buf()).aggregate(&track_id("t")).unwrap();

        assert_eq!(output.phase_durations.len(), MAX_TELEMETRY_RETAINED_ENTRIES);
        assert_eq!(output.command_metrics.len(), MAX_TELEMETRY_RETAINED_ENTRIES);
        assert_eq!(*output.skipped_lines.as_ref(), 1);
    }

    #[test]
    fn test_aggregate_caps_error_and_hook_projections() {
        let tmp = TempDir::new().unwrap();
        let error_lines = (0..=MAX_TELEMETRY_RETAINED_ENTRIES)
            .map(|index| {
                format!(
                    "{{\"event_type\":\"NonZeroExit\",\"schema_version\":1,\"track_id\":\"t\",\"command\":\"track error-{index}\",\"exit_code\":1,\"error_chain\":\"failed\",\"timestamp\":\"2026-06-10T00:00:00Z\"}}"
                )
            })
            .chain((0..=MAX_TELEMETRY_RETAINED_ENTRIES).map(|index| {
                format!(
                    "{{\"event_type\":\"HookBlock\",\"schema_version\":1,\"track_id\":\"t\",\"hook_name\":\"hook-{index}\",\"timestamp\":\"2026-06-10T00:00:00Z\"}}"
                )
            }))
            .collect::<Vec<_>>();
        write_jsonl(tmp.path(), "t", &error_lines);

        let output =
            TelemetryReport::new(tmp.path().to_path_buf()).aggregate(&track_id("t")).unwrap();

        assert_eq!(output.errors.len(), MAX_TELEMETRY_RETAINED_ENTRIES);
        assert_eq!(output.hook_blocks.len(), MAX_TELEMETRY_RETAINED_ENTRIES);
        assert_eq!(*output.skipped_lines.as_ref(), 2);
    }

    #[test]
    fn test_read_bounded_line_honors_captured_snapshot_length() {
        let first = b"{\"event_type\":\"TrackSubcommand\"}\n";
        let second = b"{\"event_type\":\"TrackSubcommand\",\"extra\":\"appended\"}\n";
        let mut reader =
            io::BufReader::new(Cursor::new([first.as_slice(), second.as_slice()].concat()));
        let mut remaining = first.len() as u64;
        let mut line = Vec::new();

        assert!(matches!(
            read_bounded_line(&mut reader, &mut line, &mut remaining),
            Ok(BoundedLineRead::Line)
        ));
        assert_eq!(line, first);
        line.clear();
        assert!(matches!(
            read_bounded_line(&mut reader, &mut line, &mut remaining),
            Ok(BoundedLineRead::EndOfFile)
        ));
    }

    #[test]
    fn test_aggregate_malformed_command_record_is_skipped_and_counted() {
        let malformed_command = r#"{"event_type":"TrackSubcommand","schema_version":1,"track_id":"t","command":"","exit_code":0,"duration_ms":80,"timestamp":"2026-06-10T00:00:00Z"}"#;
        let tmp = TempDir::new().unwrap();
        write_jsonl(tmp.path(), "t", &[COMMAND_SUCCESS_LINE, malformed_command]);

        let report = TelemetryReport::new(tmp.path().to_path_buf());
        let output = report.aggregate(&track_id("t")).unwrap();

        assert_eq!(*output.skipped_lines.as_ref(), 1);
        assert_eq!(output.command_metrics.len(), 1);
        assert_eq!(output.phase_durations.len(), 1);
        assert_eq!(output.phase_durations[0].total_ms, 120);
    }

    #[test]
    fn test_aggregate_large_command_exit_code_is_accepted_and_counted() {
        let out_of_range_failure = r#"{"event_type":"TrackSubcommand","schema_version":1,"track_id":"t","command":"track plan","exit_code":256,"duration_ms":80,"timestamp":"2026-06-10T00:00:00Z"}"#;
        let tmp = TempDir::new().unwrap();
        write_jsonl(tmp.path(), "t", &[COMMAND_SUCCESS_LINE, out_of_range_failure]);

        let report = TelemetryReport::new(tmp.path().to_path_buf());
        let output = report.aggregate(&track_id("t")).unwrap();

        assert_eq!(*output.skipped_lines.as_ref(), 0);
        assert_eq!(output.command_metrics.len(), 1);
        let metric = output.command_metrics.first().unwrap();
        assert_eq!(*metric.executions().as_ref(), 2);
        assert_eq!(*metric.failures().as_ref(), 1);
    }

    #[test]
    fn test_aggregate_unknown_schema_command_record_is_skipped_and_counted() {
        let unknown_schema_command = r#"{"event_type":"TrackSubcommand","schema_version":999,"track_id":"t","command":"track plan","exit_code":0,"duration_ms":80,"timestamp":"2026-06-10T00:00:00Z"}"#;
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
        let null_schema_command = r#"{"event_type":"TrackSubcommand","schema_version":null,"track_id":"t","command":"track plan","exit_code":0,"duration_ms":80,"timestamp":"2026-06-10T00:00:00Z"}"#;
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
        let future_command = r#"{"event_type":"TrackSubcommand","schema_version":1,"track_id":"t","command":"track plan","exit_code":0,"duration_ms":80,"timestamp":"2026-06-10T00:00:00Z","new_field":"future value"}"#;
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
        let future_result = r#"{"event_type":"TrackSubcommand","schema_version":1,"track_id":"t","command":"track plan","exit_code":0,"duration_ms":80,"timestamp":"2026-06-10T00:00:00Z","future_result":{"new_field":"future value"}}"#;
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
    fn test_aggregate_ignores_stray_lock_file() {
        let tmp = TempDir::new().unwrap();
        write_jsonl(tmp.path(), "t", &[COMMAND_SUCCESS_LINE]);
        let lock_path = tmp.path().join("t").join("logs").join("telemetry.jsonl.lock");
        std::fs::create_dir(&lock_path).unwrap();

        let output =
            TelemetryReport::new(tmp.path().to_path_buf()).aggregate(&track_id("t")).unwrap();

        assert_eq!(output.command_metrics.len(), 1);
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
