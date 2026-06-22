//! `sotp telemetry` subcommand composition.
//!
//! Provides:
//! - `CliApp::telemetry_report`: constructs `TelemetryReport`, calls
//!   `aggregate`, and formats the result as a human-readable text report.
//! - `CliApp::telemetry_emit_archived_track_subcommand`: wires
//!   `FsArchivedTrackTelemetryAdapter` + `ArchivedTrackTelemetryInteractor`
//!   and emits a single archived-track telemetry event (D8 / AC-10).
//!
//! `telemetry_report` is a pure display command (OS-04 / AC-06): it reads and
//! formats telemetry data without emitting any `TelemetryEvent` itself.  No
//! telemetry writer is constructed or invoked here.
//!
//! Aggregation logic lives in `infrastructure::telemetry::TelemetryReport`
//! (CN-07); this module is a thin formatting + error-mapping layer.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::{CliApp, CommandOutcome, error::CompositionError};

/// Input DTO for `sotp telemetry report`.
#[derive(Debug, Clone)]
pub struct TelemetryReportInput {
    /// Track ID whose telemetry log should be aggregated.
    pub track_id: String,
    /// Path to the track items directory (e.g. `track/items`).
    pub items_dir: PathBuf,
}

impl CliApp {
    /// Aggregate and format telemetry for `input.track_id`.
    ///
    /// Reads `<items_dir>/<track_id>/logs/telemetry.jsonl`, aggregates phase
    /// durations, error entries, and hook block entries, and returns a
    /// human-readable report as `CommandOutcome.stdout`.
    ///
    /// Exits 0 when aggregation succeeds (including when corrupted lines were
    /// skipped — AC-08 / AC-09).  Exits 1 when the track directory does not
    /// exist (`TelemetryReportError::TrackNotFound`) or an I/O error occurs
    /// (`TelemetryReportError::Io`).
    ///
    /// # Errors
    /// Returns `Err(String)` for structural errors that prevent even a partial
    /// report (e.g. track directory absent or unreadable file).
    pub fn telemetry_report(
        &self,
        input: TelemetryReportInput,
    ) -> Result<CommandOutcome, CompositionError> {
        use infrastructure::telemetry::TelemetryReport;

        let report = TelemetryReport::new(input.items_dir);
        let output = report
            .aggregate(&input.track_id)
            .map_err(|e| CompositionError::Infrastructure(format!("telemetry report: {e}")))?;

        let text = format_report(&input.track_id, &output);
        Ok(CommandOutcome::success(Some(text)))
    }

    /// Emit a telemetry event for a subcommand dispatched against an archived track.
    ///
    /// Constructs the telemetry directory path as
    /// `<repo_root>/track/archive/<track_id>/logs`, then wires
    /// `FsArchivedTrackTelemetryAdapter` → `ArchivedTrackTelemetryInteractor`
    /// and calls `interactor.emit(ArchivedTrackTelemetryCommand { subcommand })`.
    ///
    /// This is the D8 composition shim (AC-10): the bin path delegates here so
    /// that `apps/cli/src/main.rs` contains no direct `std::fs` / `serde_json` /
    /// `chrono::Utc::now()` I/O for the archived-track telemetry path.
    ///
    /// `items_dir` is the track items directory (e.g. `"track/items"`) used to
    /// derive the project root. `track_id` identifies the archived track.
    /// `subcommand` is the opaque CLI subcommand label (e.g. `"track archive"`).
    ///
    /// # Errors
    ///
    /// Returns `Err(String)` when:
    /// - `items_dir` does not have the expected `<root>/track/items` structure.
    /// - Git repository discovery from the project root fails.
    /// - The adapter reports an I/O or serialization error.
    pub fn telemetry_emit_archived_track_subcommand(
        &self,
        items_dir: &Path,
        track_id: &str,
        subcommand: String,
    ) -> Result<(), String> {
        use infrastructure::FsArchivedTrackTelemetryAdapter;
        use infrastructure::git_cli::GitRepository as _;
        use usecase::telemetry::{
            ArchivedTrackTelemetryCommand, ArchivedTrackTelemetryInteractor,
            ArchivedTrackTelemetryService as _,
        };

        // Derive the project root, then discover the repo to get an absolute root path.
        let project_root = crate::track::resolve_project_root(items_dir)?;
        let repo = infrastructure::git_cli::SystemGitRepo::discover_from(&project_root)
            .map_err(|e| format!("failed to discover git repository: {e}"))?;
        let repo_root = repo.root().to_path_buf();

        // Telemetry directory for the archived track.
        let telemetry_dir = repo_root.join("track").join("archive").join(track_id).join("logs");

        let adapter = FsArchivedTrackTelemetryAdapter::new(telemetry_dir);
        let interactor = ArchivedTrackTelemetryInteractor::new(Arc::new(adapter));

        interactor
            .emit(ArchivedTrackTelemetryCommand { subcommand })
            .map_err(|e: usecase::telemetry::ArchivedTrackTelemetryError| e.to_string())
    }
}

// ---------------------------------------------------------------------------
// Formatting helpers
// ---------------------------------------------------------------------------

/// Format a `TelemetryReportOutput` as a human-readable text report.
fn format_report(
    track_id: &str,
    output: &infrastructure::telemetry::TelemetryReportOutput,
) -> String {
    let mut lines: Vec<String> = Vec::new();

    lines.push(format!("Telemetry report for track: {track_id}"));
    lines.push(String::new());

    // ── Phase durations ───────────────────────────────────────────────────────
    lines.push("Phase durations:".to_owned());
    if output.phase_durations.is_empty() {
        lines.push("  (no phase data recorded)".to_owned());
    } else {
        for pd in &output.phase_durations {
            lines.push(format!(
                "  {:<40} {:>8} ms  ({} event(s))",
                pd.phase_name, pd.total_ms, pd.event_count
            ));
        }
    }
    lines.push(String::new());

    // ── Errors ────────────────────────────────────────────────────────────────
    lines.push(format!("Errors ({}):", output.errors.len()));
    if output.errors.is_empty() {
        lines.push("  (none)".to_owned());
    } else {
        for err in &output.errors {
            lines.push(format!(
                "  [{}] {} (exit {}): {}",
                err.timestamp, err.command, err.exit_code, err.error_chain
            ));
        }
    }
    lines.push(String::new());

    // ── Hook blocks ───────────────────────────────────────────────────────────
    lines.push(format!("Hook blocks ({}):", output.hook_blocks.len()));
    if output.hook_blocks.is_empty() {
        lines.push("  (none)".to_owned());
    } else {
        for hb in &output.hook_blocks {
            lines.push(format!("  [{}] {}", hb.timestamp, hb.hook_name));
        }
    }
    lines.push(String::new());

    // ── Skipped lines ─────────────────────────────────────────────────────────
    lines.push(format!("Skipped lines: {}", output.skipped_lines));
    if output.skipped_lines > 0 {
        lines.push("  (parse failure or unknown schema_version)".to_owned());
    }
    lines.push(String::new());

    lines.join("\n")
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::io::Write;

    use super::*;
    use crate::CliApp;

    fn write_jsonl_fixture(tmp: &tempfile::TempDir, track_id: &str, lines: &[&str]) {
        let logs_dir = tmp.path().join(track_id).join("logs");
        std::fs::create_dir_all(&logs_dir).unwrap();
        let mut file = std::fs::File::create(logs_dir.join("telemetry.jsonl")).unwrap();
        for line in lines {
            file.write_all(line.as_bytes()).unwrap();
            file.write_all(b"\n").unwrap();
        }
    }

    const SUBCOMMAND_LINE: &str = r#"{"event_type":"TrackSubcommand","schema_version":1,"track_id":"t","command":"track spec-design","exit_code":0,"duration_ms":1200,"timestamp":"2026-06-10T00:00:00Z"}"#;
    const NON_ZERO_EXIT_LINE: &str = r#"{"event_type":"NonZeroExit","schema_version":1,"track_id":"t","command":"track spec-design","exit_code":1,"error_chain":"gate failed","timestamp":"2026-06-10T01:00:00Z"}"#;
    const HOOK_BLOCK_LINE: &str = r#"{"event_type":"HookBlock","schema_version":1,"track_id":"t","hook_name":"block-direct-git-ops","timestamp":"2026-06-10T02:00:00Z"}"#;

    // ── telemetry_report: happy path ──────────────────────────────────────────

    #[test]
    fn test_telemetry_report_happy_path_exits_zero_with_output() {
        let tmp = tempfile::TempDir::new().unwrap();
        write_jsonl_fixture(&tmp, "t", &[SUBCOMMAND_LINE, NON_ZERO_EXIT_LINE, HOOK_BLOCK_LINE]);

        let result = CliApp::new().telemetry_report(TelemetryReportInput {
            track_id: "t".to_owned(),
            items_dir: tmp.path().to_path_buf(),
        });
        let outcome = result.unwrap();
        assert_eq!(outcome.exit_code, 0);

        let text = outcome.stdout.unwrap();
        assert!(text.contains("track spec-design"), "phase name must appear in output");
        assert!(text.contains("1200"), "phase duration must appear in output");
        assert!(text.contains("gate failed"), "error chain must appear in output");
        assert!(text.contains("block-direct-git-ops"), "hook name must appear in output");
        assert!(text.contains("Skipped lines: 0"), "skip count must always appear");
    }

    // ── telemetry_report: skipped lines ──────────────────────────────────────

    #[test]
    fn test_telemetry_report_shows_skipped_line_count_when_nonzero() {
        let tmp = tempfile::TempDir::new().unwrap();
        write_jsonl_fixture(&tmp, "t", &[SUBCOMMAND_LINE, "not valid json", HOOK_BLOCK_LINE]);

        let outcome = CliApp::new()
            .telemetry_report(TelemetryReportInput {
                track_id: "t".to_owned(),
                items_dir: tmp.path().to_path_buf(),
            })
            .unwrap();

        assert_eq!(outcome.exit_code, 0, "skipped lines must not fail the command (AC-09)");
        let text = outcome.stdout.unwrap();
        assert!(text.contains("Skipped lines: 1"), "skipped count must be shown; got: {text}");
    }

    // ── telemetry_report: empty log ───────────────────────────────────────────

    #[test]
    fn test_telemetry_report_missing_log_exits_zero_with_empty_report() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("t")).unwrap();

        let outcome = CliApp::new()
            .telemetry_report(TelemetryReportInput {
                track_id: "t".to_owned(),
                items_dir: tmp.path().to_path_buf(),
            })
            .unwrap();

        assert_eq!(outcome.exit_code, 0);
        let text = outcome.stdout.unwrap();
        assert!(text.contains("(no phase data recorded)"), "empty report must note absence");
        assert!(text.contains("Skipped lines: 0"), "empty report must still show skip count");
    }

    // ── telemetry_report: track not found ────────────────────────────────────

    #[test]
    fn test_telemetry_report_track_not_found_returns_err() {
        let tmp = tempfile::TempDir::new().unwrap();

        let result = CliApp::new().telemetry_report(TelemetryReportInput {
            track_id: "does-not-exist".to_owned(),
            items_dir: tmp.path().to_path_buf(),
        });

        assert!(result.is_err(), "missing track must return Err");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("does-not-exist") || msg.contains("track not found"),
            "error must mention track id; got: {msg}"
        );
    }
}
