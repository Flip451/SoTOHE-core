//! Archived-track telemetry use case.
//!
//! Defines the command DTO, error type, secondary port, application service
//! trait, and interactor for recording telemetry when a subcommand is dispatched
//! against an archived track. The infrastructure adapter
//! (`FsArchivedTrackTelemetryAdapter`) lives in `libs/infrastructure` and is
//! injected at composition time.

use std::sync::Arc;

use thiserror::Error;

// ── Error ─────────────────────────────────────────────────────────────────────

/// Error type for [`ArchivedTrackTelemetryPort`].
///
/// Both variants carry `String` payloads so that the usecase layer remains free
/// of `std::io` and `serde_json` dependencies (hexagonal purity). The adapter
/// converts concrete error types to strings at the infrastructure boundary:
///
/// - `io::Error` → `Io(e.to_string())`
/// - `serde_json::Error` → `Serialize(e.to_string())`
#[derive(Debug, Error)]
pub enum ArchivedTrackTelemetryError {
    /// A filesystem I/O failure occurred while opening, creating, or writing the
    /// telemetry file or its parent directory. The payload is the underlying
    /// `io::Error` converted to `String` at the adapter boundary.
    #[error("archived-track telemetry I/O error: {0}")]
    Io(String),

    /// JSON serialization of the telemetry event failed. The payload is the
    /// `serde_json::Error` message converted to `String` at the adapter boundary.
    #[error("archived-track telemetry serialize error: {0}")]
    Serialize(String),
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
/// Both error variants carry `String` payloads.  The infrastructure adapter
/// converts concrete error types at the boundary:
/// - `io::Error` → `Io(e.to_string())`
/// - `serde_json::Error` → `Serialize(e.to_string())`
pub trait ArchivedTrackTelemetryPort: Send + Sync {
    /// Emit a single telemetry event for `subcommand`.
    ///
    /// # Errors
    ///
    /// Returns [`ArchivedTrackTelemetryError::Io`] on filesystem failure.
    /// Returns [`ArchivedTrackTelemetryError::Serialize`] on JSON serialization
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
                Err(ArchivedTrackTelemetryError::Serialize("test failure".to_string()))
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
            matches!(result, Err(ArchivedTrackTelemetryError::Serialize(_))),
            "error variant must be Serialize"
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
