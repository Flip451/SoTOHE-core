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

pub mod command_trace;

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

    lines.push("Command metrics:".to_owned());
    if output.command_metrics.is_empty() {
        lines.push("  (no command data recorded)".to_owned());
    } else {
        for metric in &output.command_metrics {
            let executions = *metric.executions().as_ref();
            let failures = *metric.failures().as_ref();
            let total_duration = *metric.total_duration().as_ref();
            let failure_rate = metric.failure_rate().value();
            lines.push(format!(
                "  {:<40} {:>8} execution(s) {:>8} ms  ({} failure(s), {}.{:02}% failure rate)",
                metric.command().as_str(),
                executions,
                total_duration,
                failures,
                failure_rate / 100,
                failure_rate % 100,
            ));
        }
    }
    lines.push(String::new());

    let skipped_lines = output.skipped_lines.as_ref();
    lines.push(format!("Skipped lines: {skipped_lines}"));
    if *skipped_lines > 0 {
        lines.push("  (parse failure or unknown schema_version)".to_owned());
    }
    lines.push(String::new());

    lines.join("\n")
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use std::path::Path;

    use usecase::telemetry::{
        TelemetryAggregateServiceError,
        command_trace::{
            CommandDurationMillis, CommandExecutionCount, CommandExecutionMetric,
            SotpCommandIdentity, TelemetrySkippedLineCount,
        },
    };

    use super::*;

    struct MetricsService {
        report: TelemetryReportOutput,
    }

    impl TelemetryAggregateService for MetricsService {
        fn report(
            &self,
            _track_id: &str,
            _items_dir: &Path,
        ) -> Result<TelemetryReportOutput, TelemetryAggregateServiceError> {
            Ok(self.report.clone())
        }

        fn emit_archived(
            &self,
            _items_dir: &Path,
            _track_id: &str,
            _subcommand: String,
            _exit_code: i32,
            _duration_ms: u64,
        ) -> Result<(), TelemetryAggregateServiceError> {
            Ok(())
        }
    }

    struct FailingReportService;

    impl TelemetryAggregateService for FailingReportService {
        fn report(
            &self,
            _track_id: &str,
            _items_dir: &Path,
        ) -> Result<TelemetryReportOutput, TelemetryAggregateServiceError> {
            Err(TelemetryAggregateServiceError::ReportUnavailable(
                "report fixture unavailable".to_owned(),
            ))
        }

        fn emit_archived(
            &self,
            _items_dir: &Path,
            _track_id: &str,
            _subcommand: String,
            _exit_code: i32,
            _duration_ms: u64,
        ) -> Result<(), TelemetryAggregateServiceError> {
            Ok(())
        }
    }

    #[test]
    fn test_telemetry_report_command_metrics_render_frequency_duration_and_failure_rate()
    -> Result<(), Box<dyn std::error::Error>> {
        let metric = CommandExecutionMetric::new(
            SotpCommandIdentity::try_new("track status".to_owned())?,
            CommandExecutionCount::from(3),
            CommandExecutionCount::from(1),
            CommandDurationMillis::from(540),
        )?;
        let service = Arc::new(MetricsService {
            report: TelemetryReportOutput {
                phase_durations: Vec::new(),
                errors: Vec::new(),
                hook_blocks: Vec::new(),
                skipped_lines: TelemetrySkippedLineCount::from(0),
                command_metrics: vec![metric],
            },
        });
        let driver = TelemetryDriver::new(service);

        let outcome = driver.handle(TelemetryInput::Report(TelemetryReportInput {
            track_id: "test-track".to_owned(),
            items_dir: PathBuf::from("track/items"),
        }));
        let report = outcome.stdout.expect("successful report has stdout");

        assert_eq!(outcome.exit_code, 0);
        assert!(report.contains("Command metrics:"));
        assert!(report.contains("track status"));
        assert!(report.contains("3 execution(s)"));
        assert!(report.contains("540 ms"));
        assert!(report.contains("1 failure(s), 33.33% failure rate"));
        Ok(())
    }

    #[test]
    fn test_telemetry_report_empty_command_metrics_renders_empty_data_message() {
        let service = Arc::new(MetricsService {
            report: TelemetryReportOutput {
                phase_durations: Vec::new(),
                errors: Vec::new(),
                hook_blocks: Vec::new(),
                skipped_lines: TelemetrySkippedLineCount::from(0),
                command_metrics: Vec::new(),
            },
        });
        let driver = TelemetryDriver::new(service);

        let outcome = driver.handle(TelemetryInput::Report(TelemetryReportInput {
            track_id: "test-track".to_owned(),
            items_dir: PathBuf::from("track/items"),
        }));

        assert_eq!(outcome.exit_code, 0);
        assert_eq!(outcome.stderr, None);
        assert!(
            outcome
                .stdout
                .as_deref()
                .is_some_and(|report| report.contains("  (no command data recorded)"))
        );
    }

    #[test]
    fn test_telemetry_report_service_failure_preserves_failure_outcome() {
        let driver = TelemetryDriver::new(Arc::new(FailingReportService));

        let outcome = driver.handle(TelemetryInput::Report(TelemetryReportInput {
            track_id: "test-track".to_owned(),
            items_dir: PathBuf::from("track/items"),
        }));

        assert_eq!(outcome.exit_code, 1);
        assert_eq!(outcome.stdout, None);
        assert_eq!(
            outcome.stderr.as_deref(),
            Some("report unavailable: report fixture unavailable")
        );
    }
}
