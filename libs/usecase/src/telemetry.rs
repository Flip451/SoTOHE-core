//! Archived-track telemetry use case.
//!
//! Defines the command DTO, error type, secondary port, application service
//! trait, and interactor for recording telemetry when a subcommand is dispatched
//! against an archived track. The infrastructure adapter
//! (`FsArchivedTrackTelemetryAdapter`) lives in `libs/infrastructure` and is
//! injected at composition time.
//!
//! Also defines `TelemetryReportPort` — the secondary port for reading and
//! aggregating telemetry JSONL files, used by `cli_driver::TelemetryDriver`.

use std::path::Path;
use std::sync::Arc;

use thiserror::Error;

// ── TelemetryReportPort ───────────────────────────────────────────────────────

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
    pub skipped_lines: usize,
}

/// Error type for [`TelemetryReportPort`].
#[derive(Debug, thiserror::Error)]
pub enum TelemetryReportError {
    /// The specified track directory does not exist.
    #[error("track not found: {0}")]
    TrackNotFound(String),
    /// The telemetry report could not be loaded.
    #[error("telemetry report unavailable: {0}")]
    ReportUnavailable(String),
}

/// Error type for [`TelemetryEmitDynamicPort`].
#[derive(Debug, Error)]
pub enum TelemetryEmitDynamicPortError {
    /// Resolution or I/O failure when emitting the telemetry event.
    #[error("emit unavailable: {0}")]
    EmitUnavailable(String),
}

/// Secondary port for emitting archived-track telemetry with dynamic path resolution.
///
/// Unlike [`ArchivedTrackTelemetryPort`], this port accepts the full context at
/// call time (including `items_dir` and `track_id`) so the driver does not need
/// to know the repo root at construction time.
pub trait TelemetryEmitDynamicPort: Send + Sync {
    /// Emit a telemetry event for an archived-track subcommand.
    ///
    /// # Errors
    /// Returns [`TelemetryEmitDynamicPortError::EmitUnavailable`] on resolution
    /// or I/O failure.
    fn emit_archived(
        &self,
        items_dir: &Path,
        track_id: &str,
        subcommand: String,
        exit_code: i32,
        duration_ms: u64,
    ) -> Result<(), TelemetryEmitDynamicPortError>;
}

/// Secondary port for aggregating telemetry JSONL data for a track.
///
/// Abstracts the infrastructure `TelemetryReport` behind a pure usecase boundary
/// so that `cli_driver` never imports `infrastructure` directly.
///
/// `items_dir` is passed per-call so the same port implementation can serve
/// multiple track items directories without requiring re-construction.
pub trait TelemetryReportPort: Send + Sync {
    /// Aggregate telemetry for `track_id` using `items_dir`.
    ///
    /// # Errors
    /// Returns [`TelemetryReportError::TrackNotFound`] when the track directory
    /// does not exist. Returns [`TelemetryReportError::ReportUnavailable`] when
    /// the report cannot be loaded.
    fn aggregate(
        &self,
        track_id: &str,
        items_dir: &Path,
    ) -> Result<TelemetryReportOutput, TelemetryReportError>;
}

// ── TelemetryAggregateService ─────────────────────────────────────────────────

/// Error type for [`TelemetryAggregateService`].
#[derive(Debug, Error)]
pub enum TelemetryAggregateServiceError {
    /// The report could not be produced (track not found or I/O failure).
    #[error("report unavailable: {0}")]
    ReportUnavailable(String),
    /// The archived-track telemetry event could not be emitted (resolution or
    /// I/O failure).
    #[error("emit unavailable: {0}")]
    EmitUnavailable(String),
}

/// Aggregate primary port for the `telemetry` command family.
///
/// `TelemetryDriver` holds exactly one `Arc<dyn TelemetryAggregateService>` and
/// delegates each `TelemetryInput` variant to the corresponding method.
/// The concrete implementation in `cli_composition` wires both sub-services
/// internally, keeping the driver free of multi-service injection (D3/D4
/// cli_driver policy).
pub trait TelemetryAggregateService: Send + Sync {
    /// Aggregate telemetry data for `track_id` and return the structured DTO.
    ///
    /// Presentation formatting (the final report string) is the responsibility
    /// of the driver layer (`cli_driver::telemetry`); this method returns the
    /// raw `TelemetryReportOutput` so the driver can render it.
    ///
    /// # Errors
    /// Returns [`TelemetryAggregateServiceError::ReportUnavailable`] on
    /// track-not-found or I/O failure.
    fn report(
        &self,
        track_id: &str,
        items_dir: &Path,
    ) -> Result<TelemetryReportOutput, TelemetryAggregateServiceError>;

    /// Emit a telemetry event for a subcommand dispatched against an archived track.
    ///
    /// # Errors
    /// Returns [`TelemetryAggregateServiceError::EmitUnavailable`] on resolution
    /// or I/O failure.
    fn emit_archived(
        &self,
        items_dir: &Path,
        track_id: &str,
        subcommand: String,
        exit_code: i32,
        duration_ms: u64,
    ) -> Result<(), TelemetryAggregateServiceError>;
}

// ── Error ─────────────────────────────────────────────────────────────────────

/// Error type for [`ArchivedTrackTelemetryPort`].
///
/// The single `EmitUnavailable` variant collapses both filesystem I/O failures
/// and JSON serialization failures into a single usecase-level concept so the
/// public API does not leak storage/serialization categories. The adapter
/// converts concrete error types to strings at the infrastructure boundary:
///
/// - `io::Error` → `EmitUnavailable(e.to_string())`
/// - `serde_json::Error` → `EmitUnavailable(e.to_string())`
#[derive(Debug, Error)]
pub enum ArchivedTrackTelemetryError {
    /// The archived telemetry event could not be emitted. The payload is a
    /// human-readable description of the underlying failure (filesystem write
    /// failure, JSON serialization failure, etc.) converted at the adapter
    /// boundary.
    #[error("archived-track telemetry emit unavailable: {0}")]
    EmitUnavailable(String),
}

// ── Command ───────────────────────────────────────────────────────────────────

/// CQRS command for the archived-track telemetry use case.
///
/// `subcommand` is an opaque CLI subcommand label recorded as free text; it is
/// not a domain value object.
pub struct ArchivedTrackTelemetryCommand {
    /// The CLI subcommand label to record in the telemetry event.
    pub subcommand: String,
    /// The archived track identifier (e.g. `"my-feature-2026-01-01"`); recorded
    /// in the canonical `TelemetryEvent::TrackSubcommand.track_id` field so the
    /// archived JSONL line is parseable by `TelemetryReport::aggregate`.
    pub track_id: String,
    /// Process exit code (`0` = success).
    pub exit_code: i32,
    /// Wall-clock duration of the archive operation in milliseconds.
    pub duration_ms: u64,
}

// ── Secondary port ────────────────────────────────────────────────────────────

/// Secondary port for emitting a telemetry event when an archived-track
/// subcommand is dispatched.
///
/// Abstracts the direct `std::fs` / `serde_json` / `chrono` I/O that previously
/// lived in `apps/cli/src/main.rs:247-294`. The infrastructure adapter owns
/// timestamp capture and receives the telemetry directory at construction time.
///
/// # Error mapping
///
/// The single `EmitUnavailable` variant carries a `String` payload. The
/// infrastructure adapter converts concrete error types at the boundary:
/// - `io::Error` → `EmitUnavailable(e.to_string())`
/// - `serde_json::Error` → `EmitUnavailable(e.to_string())`
pub trait ArchivedTrackTelemetryPort: Send + Sync {
    /// Emit a single telemetry event for `subcommand`.
    ///
    /// # Errors
    ///
    /// Returns [`ArchivedTrackTelemetryError::EmitUnavailable`] on filesystem failure.
    /// Returns [`ArchivedTrackTelemetryError::EmitUnavailable`] on JSON serialization
    /// failure.
    fn emit(
        &self,
        track_id: String,
        subcommand: String,
        exit_code: i32,
        duration_ms: u64,
    ) -> Result<(), ArchivedTrackTelemetryError>;
}

// ── Application service trait ─────────────────────────────────────────────────

/// Application service (primary port) for archived-track telemetry emission.
///
/// `cli_driver` invokes this service; the interactor delegates persistence
/// through [`ArchivedTrackTelemetryPort`] so the secondary adapter stays behind
/// the usecase boundary.
pub trait ArchivedTrackTelemetryService: Send + Sync {
    /// Emit a telemetry event from a command DTO.
    ///
    /// # Errors
    ///
    /// Propagates any error returned by the injected
    /// [`ArchivedTrackTelemetryPort`].
    fn emit(&self, cmd: ArchivedTrackTelemetryCommand) -> Result<(), ArchivedTrackTelemetryError>;
}

// ── Interactor ────────────────────────────────────────────────────────────────

/// Interactor implementing [`ArchivedTrackTelemetryService`].
///
/// Holds the injected [`ArchivedTrackTelemetryPort`] as a private field and
/// delegates telemetry persistence through that secondary port, keeping
/// `cli_driver` from invoking infrastructure adapters directly.
pub struct ArchivedTrackTelemetryInteractor {
    port: Arc<dyn ArchivedTrackTelemetryPort>,
}

impl ArchivedTrackTelemetryInteractor {
    /// Constructs a new interactor with the given port.
    #[must_use]
    pub fn new(port: Arc<dyn ArchivedTrackTelemetryPort>) -> Self {
        Self { port }
    }
}

impl ArchivedTrackTelemetryService for ArchivedTrackTelemetryInteractor {
    fn emit(&self, cmd: ArchivedTrackTelemetryCommand) -> Result<(), ArchivedTrackTelemetryError> {
        self.port.emit(cmd.track_id, cmd.subcommand, cmd.exit_code, cmd.duration_ms)
    }
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::{
        ArchivedTrackTelemetryCommand, ArchivedTrackTelemetryError,
        ArchivedTrackTelemetryInteractor, ArchivedTrackTelemetryPort,
        ArchivedTrackTelemetryService,
    };

    // ── Mock port ─────────────────────────────────────────────────────────────

    #[derive(Default)]
    struct MockPort {
        calls: Mutex<Vec<String>>,
    }

    impl ArchivedTrackTelemetryPort for MockPort {
        fn emit(
            &self,
            track_id: String,
            subcommand: String,
            exit_code: i32,
            duration_ms: u64,
        ) -> Result<(), ArchivedTrackTelemetryError> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("{track_id}|{subcommand}|{exit_code}|{duration_ms}"));
            Ok(())
        }
    }

    // ── Tests ─────────────────────────────────────────────────────────────────

    #[test]
    fn interactor_delegates_subcommand_to_port_verbatim() {
        let mock = Arc::new(MockPort::default());
        let interactor = ArchivedTrackTelemetryInteractor::new(
            Arc::clone(&mock) as Arc<dyn ArchivedTrackTelemetryPort>
        );

        let cmd = ArchivedTrackTelemetryCommand {
            subcommand: "track spec-design".to_string(),
            track_id: "t1".to_string(),
            exit_code: 0,
            duration_ms: 42,
        };
        interactor.emit(cmd).unwrap();

        let calls = mock.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0], "t1|track spec-design|0|42");
    }

    #[test]
    fn interactor_propagates_port_error() {
        struct FailingPort;
        impl ArchivedTrackTelemetryPort for FailingPort {
            fn emit(
                &self,
                _track_id: String,
                _subcommand: String,
                _exit_code: i32,
                _duration_ms: u64,
            ) -> Result<(), ArchivedTrackTelemetryError> {
                Err(ArchivedTrackTelemetryError::EmitUnavailable("test failure".to_string()))
            }
        }

        let interactor = ArchivedTrackTelemetryInteractor::new(Arc::new(FailingPort));
        let cmd = ArchivedTrackTelemetryCommand {
            subcommand: "track impl".to_string(),
            track_id: "t1".to_string(),
            exit_code: 1,
            duration_ms: 0,
        };
        let result = interactor.emit(cmd);

        assert!(result.is_err(), "interactor must propagate port error");
        assert!(
            matches!(result, Err(ArchivedTrackTelemetryError::EmitUnavailable(_))),
            "error variant must be EmitUnavailable"
        );
    }

    #[test]
    fn multiple_emits_each_recorded_by_port() {
        let mock = Arc::new(MockPort::default());
        let interactor = ArchivedTrackTelemetryInteractor::new(
            Arc::clone(&mock) as Arc<dyn ArchivedTrackTelemetryPort>
        );

        for label in &["track init", "track review", "track commit"] {
            let cmd = ArchivedTrackTelemetryCommand {
                subcommand: (*label).to_string(),
                track_id: "t1".to_string(),
                exit_code: 0,
                duration_ms: 1,
            };
            interactor.emit(cmd).unwrap();
        }

        let calls = mock.calls.lock().unwrap();
        assert_eq!(calls.len(), 3);
        assert_eq!(calls[0], "t1|track init|0|1");
        assert_eq!(calls[1], "t1|track review|0|1");
        assert_eq!(calls[2], "t1|track commit|0|1");
    }
}
