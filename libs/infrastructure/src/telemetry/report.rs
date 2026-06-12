//! `TelemetryReport` — secondary adapter that reads and aggregates
//! `track/items/<id>/logs/telemetry.jsonl` into a `TelemetryReportOutput`.
//!
//! Fail-open line skipping: broken JSON lines and lines with an unknown
//! `schema_version` are counted in `skipped_lines` but never cause an error
//! (CN-08 / AC-08 / IN-07).

use std::collections::HashMap;
use std::io::{self, BufRead};
use std::path::PathBuf;

use domain::TrackId;
use thiserror::Error;

use crate::telemetry::TelemetryEvent;

// ---------------------------------------------------------------------------
// Output DTOs
// ---------------------------------------------------------------------------

/// Aggregated output of `bin/sotp telemetry report <track-id>`.
///
/// Carries phase-by-phase duration summary, error list, hook block list, and
/// the count of lines skipped due to parse failures or unknown `schema_version`
/// (fail-open per CN-08 / AC-08).
#[derive(Debug, Clone)]
pub struct TelemetryReportOutput {
    /// Per-phase aggregated duration, derived from `TrackSubcommand` events.
    pub phase_durations: Vec<PhaseDurationSummary>,
    /// Non-zero exit events projected from `NonZeroExit` JSONL events.
    pub errors: Vec<TelemetryErrorEntry>,
    /// Hook block events projected from `HookBlock` JSONL events.
    pub hook_blocks: Vec<TelemetryHookBlockEntry>,
    /// Number of lines skipped (broken JSON or unknown `schema_version`).
    pub skipped_lines: u32,
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
/// (CN-08) and counted in `TelemetryReportOutput.skipped_lines`.
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

fn is_known_schema_version(v: u32) -> bool {
    KNOWN_SCHEMA_VERSIONS.contains(&v)
}

/// Extract `schema_version` from a `TelemetryEvent` variant (all carry one).
fn schema_version_of(event: &TelemetryEvent) -> u32 {
    match event {
        TelemetryEvent::TrackSubcommand { schema_version, .. } => *schema_version,
        TelemetryEvent::GateEval { schema_version, .. } => *schema_version,
        TelemetryEvent::ReviewRound { schema_version, .. } => *schema_version,
        TelemetryEvent::ExternalSubprocess { schema_version, .. } => *schema_version,
        TelemetryEvent::HookBlock { schema_version, .. } => *schema_version,
        TelemetryEvent::AdvisoryHookFired { schema_version, .. } => *schema_version,
        TelemetryEvent::NonZeroExit { schema_version, .. } => *schema_version,
    }
}

// ---------------------------------------------------------------------------
// TelemetryReport
// ---------------------------------------------------------------------------

/// Reads and aggregates `telemetry.jsonl` for a given track-id to produce a
/// `TelemetryReportOutput`.
///
/// Implements fail-open line skipping: broken JSON lines and lines with an
/// unknown `schema_version` are counted but not failed on per CN-08
/// (IN-07, AC-08). Private fields: `items_dir` path.
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
    /// does not exist. Returns an empty `TelemetryReportOutput` (with
    /// `skipped_lines=0`) if the log file does not exist — this is the normal
    /// state before any subcommands have been run for the track (CN-08).
    ///
    /// # Errors
    /// Returns `TelemetryReportError::TrackNotFound` when the track directory
    /// is absent. Returns `TelemetryReportError::Io` on read failures after
    /// the file has been opened.
    pub fn aggregate(&self, track_id: &str) -> Result<TelemetryReportOutput, TelemetryReportError> {
        let valid_track_id = TrackId::try_new(track_id.to_owned())
            .map_err(|_| TelemetryReportError::TrackNotFound { track_id: track_id.to_owned() })?;
        let track_id = valid_track_id.as_ref();
        let track_dir = self.items_dir.join(track_id);

        match std::fs::metadata(&track_dir) {
            Ok(metadata) if metadata.is_dir() => {}
            Ok(_) => {
                return Err(TelemetryReportError::Io {
                    path: track_dir.display().to_string(),
                    message: "not a directory".to_owned(),
                });
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                return Err(TelemetryReportError::TrackNotFound { track_id: track_id.to_owned() });
            }
            Err(e) => {
                return Err(TelemetryReportError::Io {
                    path: track_dir.display().to_string(),
                    message: e.to_string(),
                });
            }
        }

        let log_path = track_dir.join("logs").join("telemetry.jsonl");

        // Missing log file is a normal state (no events written yet) — return
        // empty output (CN-08 / fail-open).
        let file = match std::fs::File::open(&log_path) {
            Ok(file) => file,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                return Ok(TelemetryReportOutput {
                    phase_durations: Vec::new(),
                    errors: Vec::new(),
                    hook_blocks: Vec::new(),
                    skipped_lines: 0,
                });
            }
            Err(e) => {
                return Err(TelemetryReportError::Io {
                    path: log_path.display().to_string(),
                    message: e.to_string(),
                });
            }
        };

        let mut reader = io::BufReader::new(file);

        // Accumulators.
        let mut phase_map: HashMap<String, (u64, u32)> = HashMap::new(); // name -> (total_ms, count)
        let mut errors: Vec<TelemetryErrorEntry> = Vec::new();
        let mut hook_blocks: Vec<TelemetryHookBlockEntry> = Vec::new();
        let mut skipped_lines: u32 = 0;

        let mut line = Vec::new();
        loop {
            line.clear();
            let bytes_read =
                reader.read_until(b'\n', &mut line).map_err(|e| TelemetryReportError::Io {
                    path: log_path.display().to_string(),
                    message: e.to_string(),
                })?;
            if bytes_read == 0 {
                break;
            }

            if line.iter().all(|b| b.is_ascii_whitespace()) {
                skipped_lines = skipped_lines.saturating_add(1);
                continue;
            }

            // Attempt typed deserialize; skip on any failure (CN-08).
            let event: TelemetryEvent = match serde_json::from_slice(&line) {
                Ok(e) => e,
                Err(_) => {
                    skipped_lines = skipped_lines.saturating_add(1);
                    continue;
                }
            };

            // Mandatory schema_version check after successful deserialize (CN-08 / AC-09).
            if !is_known_schema_version(schema_version_of(&event)) {
                skipped_lines = skipped_lines.saturating_add(1);
                continue;
            }

            // Aggregate by event kind.
            match event {
                TelemetryEvent::TrackSubcommand { command, duration_ms, .. } => {
                    let entry = phase_map.entry(command).or_insert((0, 0));
                    entry.0 = entry.0.saturating_add(duration_ms);
                    entry.1 = entry.1.saturating_add(1);
                }
                TelemetryEvent::NonZeroExit {
                    timestamp, command, exit_code, error_chain, ..
                } => {
                    errors.push(TelemetryErrorEntry { timestamp, command, exit_code, error_chain });
                }
                TelemetryEvent::HookBlock { timestamp, hook_name, .. } => {
                    hook_blocks.push(TelemetryHookBlockEntry { timestamp, hook_name });
                }
                // Other event types are not aggregated in T008 scope.
                _ => {}
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

        Ok(TelemetryReportOutput { phase_durations, errors, hook_blocks, skipped_lines })
    }
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

    fn make_track_dir(dir: &std::path::Path, track_id: &str) {
        std::fs::create_dir_all(dir.join(track_id)).unwrap();
    }

    const SUBCOMMAND_LINE: &str = r#"{"event_type":"TrackSubcommand","schema_version":1,"track_id":"t","command":"track spec-design","exit_code":0,"duration_ms":1200,"timestamp":"2026-06-10T00:00:00Z"}"#;
    const NON_ZERO_EXIT_LINE: &str = r#"{"event_type":"NonZeroExit","schema_version":1,"track_id":"t","command":"track spec-design","exit_code":1,"error_chain":"gate failed","timestamp":"2026-06-10T01:00:00Z"}"#;
    const HOOK_BLOCK_LINE: &str = r#"{"event_type":"HookBlock","schema_version":1,"track_id":"t","hook_name":"block-direct-git-ops","timestamp":"2026-06-10T02:00:00Z"}"#;

    /// Happy path: aggregate collects phase durations, errors, and hook blocks.
    #[test]
    fn test_aggregate_happy_path() {
        let tmp = TempDir::new().unwrap();
        write_jsonl(tmp.path(), "t", &[SUBCOMMAND_LINE, NON_ZERO_EXIT_LINE, HOOK_BLOCK_LINE]);

        let report = TelemetryReport::new(tmp.path().to_path_buf());
        let output = report.aggregate("t").unwrap();

        assert_eq!(output.skipped_lines, 0);
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
        let output = report.aggregate("t").unwrap();

        assert_eq!(output.phase_durations.len(), 1);
        let pd = output.phase_durations.first().unwrap();
        assert_eq!(pd.total_ms, 2000); // 1200 + 800
        assert_eq!(pd.event_count, 2);
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
        let output = report.aggregate("t").unwrap();

        assert_eq!(output.skipped_lines, 2, "two broken lines must be counted");
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
        let output = report.aggregate("t").unwrap();

        assert_eq!(output.skipped_lines, 1, "non-UTF-8 broken JSON must be counted");
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
        let output = report.aggregate("t").unwrap();

        assert_eq!(output.skipped_lines, 1, "future schema_version line must be skipped");
        // The valid line still contributes.
        assert_eq!(output.phase_durations.len(), 1);
        assert_eq!(output.phase_durations.first().unwrap().total_ms, 1200);
    }

    /// Missing telemetry.jsonl for an existing track returns empty output.
    #[test]
    fn test_aggregate_missing_log_file_returns_empty_output() {
        let tmp = TempDir::new().unwrap();
        make_track_dir(tmp.path(), "t");

        let report = TelemetryReport::new(tmp.path().to_path_buf());
        let output = report.aggregate("t").unwrap();

        assert_eq!(output.skipped_lines, 0);
        assert!(output.phase_durations.is_empty());
        assert!(output.errors.is_empty());
        assert!(output.hook_blocks.is_empty());
    }

    /// Non-existent track_id returns TrackNotFound error.
    #[test]
    fn test_aggregate_nonexistent_track_returns_track_not_found() {
        let tmp = TempDir::new().unwrap();
        let report = TelemetryReport::new(tmp.path().to_path_buf());
        let result = report.aggregate("does-not-exist");

        assert!(
            matches!(result, Err(TelemetryReportError::TrackNotFound { ref track_id }) if track_id == "does-not-exist"),
            "expected TrackNotFound; got: {result:?}"
        );
    }

    /// Path-like track ids are rejected before any filesystem join.
    #[test]
    fn test_aggregate_invalid_track_id_returns_track_not_found() {
        let tmp = TempDir::new().unwrap();
        let report = TelemetryReport::new(tmp.path().to_path_buf());
        let result = report.aggregate("../outside");

        assert!(
            matches!(result, Err(TelemetryReportError::TrackNotFound { ref track_id }) if track_id == "../outside"),
            "expected TrackNotFound for invalid track id; got: {result:?}"
        );
    }

    /// A non-directory at the track path is reported as an I/O error.
    #[test]
    fn test_aggregate_track_path_file_returns_io_error() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("t"), "not a directory").unwrap();

        let report = TelemetryReport::new(tmp.path().to_path_buf());
        let result = report.aggregate("t");

        assert!(
            matches!(result, Err(TelemetryReportError::Io { ref path, ref message }) if path.ends_with("/t") && message == "not a directory"),
            "expected Io for non-directory track path; got: {result:?}"
        );
    }

    /// A malformed logs path is reported as I/O instead of empty output.
    #[test]
    fn test_aggregate_logs_path_file_returns_io_error() {
        let tmp = TempDir::new().unwrap();
        make_track_dir(tmp.path(), "t");
        std::fs::write(tmp.path().join("t").join("logs"), "not a directory").unwrap();

        let report = TelemetryReport::new(tmp.path().to_path_buf());
        let result = report.aggregate("t");

        assert!(
            matches!(result, Err(TelemetryReportError::Io { ref path, .. }) if path.ends_with("/t/logs/telemetry.jsonl")),
            "expected Io for malformed logs path; got: {result:?}"
        );
    }

    /// Empty lines in the JSONL file are skipped and counted as malformed JSON.
    #[test]
    fn test_aggregate_empty_lines_are_skipped_and_counted() {
        let tmp = TempDir::new().unwrap();
        write_jsonl(tmp.path(), "t", &[SUBCOMMAND_LINE, "", "  ", NON_ZERO_EXIT_LINE]);

        let report = TelemetryReport::new(tmp.path().to_path_buf());
        let output = report.aggregate("t").unwrap();

        assert_eq!(output.skipped_lines, 2, "blank lines must count as skipped");
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
