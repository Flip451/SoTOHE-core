//! CLI subcommands for `sotp telemetry`: workflow telemetry tools.
//!
//! Provides:
//! - `report <track-id>`: aggregate and display telemetry for a track.
//!
//! This is a pure display command (OS-04): it reads and prints telemetry data
//! without emitting any `TelemetryEvent` itself.  No file IO is performed on
//! the telemetry log beyond reading it (AC-06).
//!
//! All composition (adapter construction, aggregation, output formatting)
//! lives in `cli_composition`; this module is a thin arg-parsing + dispatch
//! layer (CN-01 / CN-02 / CN-07).

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Subcommand};
use cli_composition::TelemetryCompositionRoot;
use cli_driver::telemetry::{TelemetryInput, TelemetryReportInput};

use crate::commands::driver_outcome_to_exit;

// ── sotp telemetry ────────────────────────────────────────────────────────────

/// Subcommands for `sotp telemetry`.
#[derive(Debug, Subcommand)]
pub enum TelemetryCommand {
    /// Aggregate and display telemetry for a track: phase durations, errors,
    /// hook blocks, and skipped-line count.
    ///
    /// Reads `track/items/<track-id>/logs/telemetry.jsonl` (or the directory
    /// specified by `--items-dir`) and prints a human-readable summary to
    /// stdout.  Exits 0 even when corrupted lines were skipped (AC-09).
    /// Exits 1 when the track directory does not exist or an I/O error occurs.
    Report(ReportArgs),
}

// ── sotp telemetry report ─────────────────────────────────────────────────────

/// Arguments for `sotp telemetry report`.
#[derive(Debug, Args)]
pub struct ReportArgs {
    /// Track ID whose telemetry log should be aggregated and displayed.
    pub track_id: String,

    /// Path to the track items directory.
    #[arg(long, default_value = "track/items")]
    pub items_dir: PathBuf,
}

// ── Dispatch ──────────────────────────────────────────────────────────────────

/// Execute `sotp telemetry <subcommand>`.
pub fn execute(cmd: TelemetryCommand) -> ExitCode {
    match cmd {
        TelemetryCommand::Report(args) => driver_outcome_to_exit(
            TelemetryCompositionRoot::new().telemetry_driver().handle(TelemetryInput::Report(
                TelemetryReportInput { track_id: args.track_id, items_dir: args.items_dir },
            )),
        ),
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use clap::Parser;

    use super::*;

    /// Thin clap wrapper for parsing `sotp telemetry <subcmd>` in tests.
    #[derive(Parser)]
    struct TestCli {
        #[command(subcommand)]
        cmd: TelemetryCommand,
    }

    fn parse_telemetry(args: &[&str]) -> TelemetryCommand {
        TestCli::parse_from(args).cmd
    }

    // ── sotp telemetry report: arg parsing ────────────────────────────────────

    #[test]
    fn test_telemetry_report_parses_positional_track_id() {
        let cmd = parse_telemetry(&["telemetry", "report", "my-track-2026-06-11"]);
        match cmd {
            TelemetryCommand::Report(args) => {
                assert_eq!(args.track_id, "my-track-2026-06-11");
                assert_eq!(args.items_dir, PathBuf::from("track/items"));
            }
        }
    }

    #[test]
    fn test_telemetry_report_parses_custom_items_dir() {
        let cmd = parse_telemetry(&[
            "telemetry",
            "report",
            "my-track",
            "--items-dir",
            "custom/track/items",
        ]);
        match cmd {
            TelemetryCommand::Report(args) => {
                assert_eq!(args.track_id, "my-track");
                assert_eq!(args.items_dir, PathBuf::from("custom/track/items"));
            }
        }
    }

    #[test]
    fn test_telemetry_report_requires_track_id() {
        let result = TestCli::try_parse_from(["telemetry", "report"]);
        assert!(result.is_err(), "track_id is required and must be rejected when absent");
    }

    #[test]
    fn test_telemetry_unknown_subcommand_is_rejected() {
        let result = TestCli::try_parse_from(["telemetry", "unknown-subcmd"]);
        assert!(result.is_err(), "unrecognized telemetry subcommand must be rejected by clap");
    }
}
