//! Report-side telemetry DTOs and secondary port.

use super::command_trace::{CommandExecutionMetric, TelemetrySkippedLineCount};
use super::review_yield::ReviewYieldMetric;

/// Report record for a single telemetry phase duration.
#[derive(Debug, Clone)]
pub struct TelemetryPhaseDuration {
    /// Phase name (command label).
    pub phase_name: String,
    /// Total milliseconds.
    pub total_ms: u64,
    /// Number of events.
    pub event_count: usize,
}

/// A single error entry from telemetry.
#[derive(Debug, Clone)]
pub struct TelemetryErrorEntry {
    /// ISO-8601 timestamp.
    pub timestamp: String,
    /// Command label.
    pub command: String,
    /// Exit code.
    pub exit_code: i32,
    /// Error chain text.
    pub error_chain: String,
}

/// A single hook block entry from telemetry.
#[derive(Debug, Clone)]
pub struct TelemetryHookBlockEntry {
    /// ISO-8601 timestamp.
    pub timestamp: String,
    /// Hook name.
    pub hook_name: String,
}

/// Aggregated telemetry output for a track.
#[derive(Debug, Clone)]
pub struct TelemetryReportOutput {
    /// Phase duration summaries sorted by phase name.
    pub phase_durations: Vec<TelemetryPhaseDuration>,
    /// Error entries.
    pub errors: Vec<TelemetryErrorEntry>,
    /// Hook block entries.
    pub hook_blocks: Vec<TelemetryHookBlockEntry>,
    /// Count of skipped (unparseable) lines.
    pub skipped_lines: TelemetrySkippedLineCount,
    /// Per-command execution metrics.
    pub command_metrics: Vec<CommandExecutionMetric>,
    /// Structured-review yield metrics grouped by a recorded dimension.
    pub review_yield_metrics: Vec<ReviewYieldMetric>,
}

/// Error type for [`super::TelemetryReportPort`].
#[derive(Debug, thiserror::Error)]
pub enum TelemetryReportError {
    /// The specified track directory does not exist.
    #[error("track not found: {0}")]
    TrackNotFound(String),
    /// The telemetry report could not be loaded.
    #[error("telemetry report unavailable: {0}")]
    ReportUnavailable(String),
}
