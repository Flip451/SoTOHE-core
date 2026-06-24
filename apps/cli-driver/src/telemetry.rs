//! `telemetry` command family — primary adapter driver.
//!
//! `TelemetryDriver` holds a single injected `TelemetryAggregateService` and
//! exposes `handle(input) -> CommandOutcome`. One injected interactor — no
//! per-service fields (D3/D4 cli_driver policy).

use std::path::PathBuf;
use std::sync::Arc;

use usecase::TelemetryAggregateService;
use usecase::telemetry::TelemetryReportOutput;

use crate::render::CommandOutcome;

// ---------------------------------------------------------------------------
// Input types
// ---------------------------------------------------------------------------

/// Input DTO for `sotp telemetry report`.
#[derive(Debug, Clone)]
pub struct TelemetryReportInput {
    /// Track ID whose telemetry log should be aggregated.
    pub track_id: String,
    /// Path to the track items directory (e.g. `track/items`).
    pub items_dir: PathBuf,
}

/// Typed input for the `telemetry` command family.
pub enum TelemetryInput {
    /// Aggregate and format telemetry for a track.
    Report(TelemetryReportInput),
    /// Emit a telemetry event for a subcommand dispatched against an archived track.
    EmitArchivedTrackSubcommand {
        /// Path to the track items directory (used to derive project root).
        items_dir: PathBuf,
        /// Track ID identifying the archived track.
        track_id: String,
        /// Opaque CLI subcommand label (e.g. `"track archive"`).
        subcommand: String,
        /// Process exit code.
        exit_code: i32,
        /// Wall-clock duration in milliseconds.
        duration_ms: u64,
    },
}

// ---------------------------------------------------------------------------
// Driver
// ---------------------------------------------------------------------------

/// Primary adapter driver for the `telemetry` command family.
///
/// Holds a single injected `TelemetryAggregateService`; exposes
/// `handle(input) -> CommandOutcome`. One injected interactor — no per-service
/// fields (D3/D4 cli_driver policy).
pub struct TelemetryDriver {
    service: Arc<dyn TelemetryAggregateService>,
}

impl TelemetryDriver {
    /// Create a new `TelemetryDriver` with a single injected aggregate service.
    pub fn new(service: Arc<dyn TelemetryAggregateService>) -> Self {
        Self { service }
    }

    /// Handle a telemetry command.
    pub fn handle(&self, input: TelemetryInput) -> CommandOutcome {
        match input {
            TelemetryInput::Report(input) => self.telemetry_report(input),
            TelemetryInput::EmitArchivedTrackSubcommand {
                items_dir,
                track_id,
                subcommand,
                exit_code,
                duration_ms,
            } => self.telemetry_emit_archived(
                items_dir,
                track_id,
                subcommand,
                exit_code,
                duration_ms,
            ),
        }
    }

    fn telemetry_report(&self, input: TelemetryReportInput) -> CommandOutcome {
        match self.service.report(&input.track_id, &input.items_dir) {
            Ok(output) => {
                let text = format_report(&input.track_id, &output);
                CommandOutcome::success(Some(text))
            }
            Err(e) => CommandOutcome::failure(Some(e.to_string())),
        }
    }

    fn telemetry_emit_archived(
        &self,
        items_dir: PathBuf,
        track_id: String,
        subcommand: String,
        exit_code: i32,
        duration_ms: u64,
    ) -> CommandOutcome {
        match self.service.emit_archived(&items_dir, &track_id, subcommand, exit_code, duration_ms)
        {
            Ok(()) => CommandOutcome::success(None),
            Err(e) => CommandOutcome::failure(Some(format!("archived-track telemetry: {e}"))),
        }
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

/// Format a [`TelemetryReportOutput`] DTO into a human-readable report string.
///
/// Pure function — no side effects, no I/O. Returns a `String`; the caller
/// (`TelemetryDriver::telemetry_report`) is responsible for outputting it.
fn format_report(track_id: &str, output: &TelemetryReportOutput) -> String {
    let mut lines: Vec<String> = Vec::new();

    lines.push(format!("Telemetry report for track: {track_id}"));
    lines.push(String::new());

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

    lines.push(format!("Hook blocks ({}):", output.hook_blocks.len()));
    if output.hook_blocks.is_empty() {
        lines.push("  (none)".to_owned());
    } else {
        for hb in &output.hook_blocks {
            lines.push(format!("  [{}] {}", hb.timestamp, hb.hook_name));
        }
    }
    lines.push(String::new());

    lines.push(format!("Skipped lines: {}", output.skipped_lines));
    if output.skipped_lines > 0 {
        lines.push("  (parse failure or unknown schema_version)".to_owned());
    }
    lines.push(String::new());

    lines.join("\n")
}
