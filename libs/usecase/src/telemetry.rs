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

use std::path::{Path, PathBuf};
use std::sync::Arc;

use thiserror::Error;

use crate::git_workflow::GitPrimitivePort;

pub mod command_trace;
pub mod report;
pub mod review_yield;

pub use report::{
    TelemetryErrorEntry, TelemetryHookBlockEntry, TelemetryPhaseDuration, TelemetryReportError,
    TelemetryReportOutput,
};
pub use review_yield::{
    ReviewDetectionRateBasisPoints, ReviewExecutionCount, ReviewFindingCount, ReviewYieldMetric,
    ReviewYieldValue, ReviewYieldValueError,
};

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
    /// Emit an active-track command completion into the existing telemetry
    /// writer path. `source_track_id` is captured before command dispatch so
    /// a command that changes branches cannot retarget the record.
    /// This port is intentionally synchronous because filesystem adapters may
    /// perform blocking I/O; failures are returned through the typed error.
    ///
    fn emit_active(
        &self,
        items_dir: &Path,
        source_track_id: Option<&str>,
        subcommand: String,
        exit_code: i32,
        duration_ms: u64,
        error_chain: Option<String>,
    ) -> Result<(), TelemetryEmitDynamicPortError>;
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

/// Report-facing primary port for the `telemetry` command family.
pub trait TelemetryReportService: Send + Sync {
    /// Aggregate telemetry data for `track_id` and return the structured DTO.
    /// This service is intentionally synchronous because report adapters may
    /// perform blocking filesystem reads.
    fn report(
        &self,
        track_id: &str,
        items_dir: &Path,
    ) -> Result<TelemetryReportOutput, TelemetryAggregateServiceError>;
}

/// Active-track emission primary port for the `telemetry` command family.
pub trait TelemetryEmitService: Send + Sync {
    /// Emit a completion through the existing active-track telemetry writer.
    /// Implementations preserve the fire-and-forget diagnostic policy.
    /// The boundary remains synchronous because the adapter may perform
    /// blocking filesystem I/O.
    fn emit_completed(
        &self,
        items_dir: &Path,
        source_track_id: Option<String>,
        subcommand: String,
        exit_code: i32,
        duration_ms: u64,
        error_chain: Option<String>,
    ) -> Result<(), TelemetryAggregateServiceError>;
}

/// Archived-track port for the existing archive workflow.
pub trait TelemetryArchivedService: Send + Sync {
    /// Emit a telemetry event for a subcommand dispatched against an archived track.
    /// This port is synchronous because its adapter may perform
    /// blocking filesystem I/O.
    fn emit_archived(
        &self,
        items_dir: &Path,
        track_id: &str,
        subcommand: String,
        exit_code: i32,
        duration_ms: u64,
    ) -> Result<(), TelemetryAggregateServiceError>;
}

/// Single driver-facing service surface composed from the report, active
/// completion, and the existing archived-track port. Keeping this
/// marker as the only injected trait preserves the cli_driver composition
/// boundary while allowing each responsibility to carry its own SoT anchors.
pub trait TelemetryAggregateService:
    TelemetryReportService + TelemetryEmitService + TelemetryArchivedService + Send + Sync
{
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

// ── ArchivedTelemetryFactoryPort ────────────────────────────────────────────────

/// Factory secondary port constructing a path-parameterized
/// [`ArchivedTrackTelemetryPort`] for
/// [`TelemetryAggregateInteractor::emit_archived`]. Keeping the construction
/// behind a port lets the usecase interactor own the archive orchestration while
/// the concrete `std::fs`/`chrono` adapter stays in infrastructure.
///
/// Implemented by
/// `infrastructure::telemetry::archived_track::FsArchivedTelemetryFactoryAdapter`.
/// IN-10 / CN-03 / AC-09.
pub trait ArchivedTelemetryFactoryPort: Send + Sync {
    /// Construct an [`ArchivedTrackTelemetryPort`] that writes under `telemetry_dir`.
    fn build(&self, telemetry_dir: &Path) -> Arc<dyn ArchivedTrackTelemetryPort>;
}

// ── TelemetryReportInteractor / TelemetryEmitInteractor ─────────────────────────

/// UseCase interactor for the telemetry report path. Holds only the report
/// port — the report aggregation changes independently of the archived-emit
/// orchestration. IN-10 / CN-03 / AC-09.
pub struct TelemetryReportInteractor {
    report_port: Arc<dyn TelemetryReportPort>,
}

impl TelemetryReportInteractor {
    /// Inject the telemetry-report port.
    #[must_use]
    pub fn new(report_port: Arc<dyn TelemetryReportPort>) -> Self {
        Self { report_port }
    }

    /// Aggregate the telemetry report for a track.
    ///
    /// # Errors
    /// Returns [`TelemetryAggregateServiceError::ReportUnavailable`] when the
    /// report port fails.
    pub fn report(
        &self,
        track_id: &domain::TrackId,
        items_dir: &Path,
    ) -> Result<TelemetryReportOutput, TelemetryAggregateServiceError> {
        self.report_port.aggregate(track_id.as_ref(), items_dir).map_err(|e| {
            TelemetryAggregateServiceError::ReportUnavailable(format!("telemetry report: {e}"))
        })
    }
}

/// UseCase interactor for active-track telemetry emission.
pub struct TelemetryEmitInteractor {
    active_emit: Arc<dyn TelemetryEmitDynamicPort>,
}

impl TelemetryEmitInteractor {
    /// Inject the active-track emission port.
    #[must_use]
    pub fn new(active_emit: Arc<dyn TelemetryEmitDynamicPort>) -> Self {
        Self { active_emit }
    }

    /// Emit an active-track command completion. The infrastructure adapter
    /// owns branch-bound resolution, the existing telemetry writer, and the
    /// fail-open write policy.
    pub fn emit_completed(
        &self,
        items_dir: &Path,
        source_track_id: Option<String>,
        subcommand: String,
        exit_code: i32,
        duration_ms: u64,
        error_chain: Option<String>,
    ) -> Result<(), TelemetryAggregateServiceError> {
        self.active_emit
            .emit_active(
                items_dir,
                source_track_id.as_deref(),
                subcommand,
                exit_code,
                duration_ms,
                error_chain,
            )
            .map_err(|e| TelemetryAggregateServiceError::EmitUnavailable(e.to_string()))
    }
}

/// UseCase interactor for archived-track telemetry emission.
pub struct TelemetryArchiveInteractor {
    git: Arc<dyn GitPrimitivePort>,
    archived_factory: Arc<dyn ArchivedTelemetryFactoryPort>,
}

impl TelemetryArchiveInteractor {
    /// Inject the repo-root and archived-event ports.
    #[must_use]
    pub fn new(
        git: Arc<dyn GitPrimitivePort>,
        archived_factory: Arc<dyn ArchivedTelemetryFactoryPort>,
    ) -> Self {
        Self { git, archived_factory }
    }

    /// Emit an archived-track telemetry event.
    pub fn emit_archived(
        &self,
        items_dir: &Path,
        track_id: &domain::TrackId,
        subcommand: String,
        exit_code: i32,
        duration_ms: u64,
    ) -> Result<(), TelemetryAggregateServiceError> {
        // Derive the project root, then resolve the absolute repo root through
        // the git-primitive port (relocated from the composition root; behavior
        // preserved bit-for-bit). CN-03.
        let project_root = project_root_from_items_dir(items_dir).ok_or_else(|| {
            TelemetryAggregateServiceError::EmitUnavailable(format!(
                "--items-dir must point to '<project-root>/track/items'; got {}",
                items_dir.display()
            ))
        })?;
        let repo_root = self.git.resolve_repo_root(Some(&project_root)).map_err(|e| {
            TelemetryAggregateServiceError::EmitUnavailable(format!(
                "failed to discover git repository: {e}"
            ))
        })?;

        // The existing archived-track workflow owns its archive-local sink;
        // preserve that path for callers of the archived service. The common
        // command-completion path uses `emit_completed` and writes to the
        // active `track/items/<track-id>/logs/telemetry.jsonl` sink instead.
        let telemetry_dir =
            repo_root.join("track").join("archive").join(track_id.as_ref()).join("logs");

        let port = self.archived_factory.build(&telemetry_dir);
        let interactor = ArchivedTrackTelemetryInteractor::new(port);
        interactor
            .emit(ArchivedTrackTelemetryCommand {
                subcommand,
                track_id: track_id.as_ref().to_owned(),
                exit_code,
                duration_ms,
            })
            .map_err(|e| TelemetryAggregateServiceError::EmitUnavailable(e.to_string()))
    }
}

impl TelemetryEmitService for TelemetryEmitInteractor {
    fn emit_completed(
        &self,
        items_dir: &Path,
        source_track_id: Option<String>,
        subcommand: String,
        exit_code: i32,
        duration_ms: u64,
        error_chain: Option<String>,
    ) -> Result<(), TelemetryAggregateServiceError> {
        self.emit_completed(
            items_dir,
            source_track_id,
            subcommand,
            exit_code,
            duration_ms,
            error_chain,
        )
    }
}

impl TelemetryArchivedService for TelemetryArchiveInteractor {
    fn emit_archived(
        &self,
        items_dir: &Path,
        track_id: &str,
        subcommand: String,
        exit_code: i32,
        duration_ms: u64,
    ) -> Result<(), TelemetryAggregateServiceError> {
        let track_id = domain::TrackId::try_new(track_id.to_owned()).map_err(|e| {
            TelemetryAggregateServiceError::EmitUnavailable(format!("invalid track ID: {e}"))
        })?;
        self.emit_archived(items_dir, &track_id, subcommand, exit_code, duration_ms)
    }
}

// ── TelemetryAggregateInteractor ────────────────────────────────────────────────

/// Thin facade implementing [`TelemetryAggregateService`] over the focused
/// report, active-emission, and archive interactors.
/// IN-10 / CN-03 / AC-09.
pub struct TelemetryAggregateInteractor {
    report: TelemetryReportInteractor,
    emit: Arc<TelemetryEmitInteractor>,
    archived: Arc<TelemetryArchiveInteractor>,
}

impl TelemetryAggregateInteractor {
    /// Compose the report and focused emitters behind the service facade.
    #[must_use]
    pub fn new(
        report: TelemetryReportInteractor,
        emit: Arc<TelemetryEmitInteractor>,
        archived: Arc<TelemetryArchiveInteractor>,
    ) -> Self {
        Self { report, emit, archived }
    }
}

/// Recover the project root from an `--items-dir` argument by stripping the
/// trailing `track/items` segments, mirroring the composition-layer contract.
/// Returns `None` when `items_dir` does not end in `track/items`.
fn project_root_from_items_dir(items_dir: &Path) -> Option<PathBuf> {
    let items_name = items_dir.file_name().and_then(|n| n.to_str());
    let track_dir = items_dir.parent();
    let track_name = track_dir.and_then(Path::file_name).and_then(|n| n.to_str());
    let project_root = track_dir.and_then(Path::parent);
    match (items_name, track_name, project_root) {
        (Some("items"), Some("track"), Some(root)) => {
            if root.as_os_str().is_empty() {
                Some(PathBuf::from("."))
            } else {
                Some(root.to_path_buf())
            }
        }
        _ => None,
    }
}

impl TelemetryReportService for TelemetryAggregateInteractor {
    // The facade owns track-id validation: the baseline service trait keeps the
    // raw `&str` driver-facing surface, and the sub-interactors only accept the
    // validated `domain::TrackId` (IN-10 / CN-03 / AC-09).
    fn report(
        &self,
        track_id: &str,
        items_dir: &Path,
    ) -> Result<TelemetryReportOutput, TelemetryAggregateServiceError> {
        let track_id = domain::TrackId::try_new(track_id.to_owned()).map_err(|e| {
            TelemetryAggregateServiceError::ReportUnavailable(format!("invalid track ID: {e}"))
        })?;
        self.report.report(&track_id, items_dir)
    }
}

impl TelemetryEmitService for TelemetryAggregateInteractor {
    fn emit_completed(
        &self,
        items_dir: &Path,
        source_track_id: Option<String>,
        subcommand: String,
        exit_code: i32,
        duration_ms: u64,
        error_chain: Option<String>,
    ) -> Result<(), TelemetryAggregateServiceError> {
        self.emit.emit_completed(
            items_dir,
            source_track_id,
            subcommand,
            exit_code,
            duration_ms,
            error_chain,
        )
    }
}

impl TelemetryArchivedService for TelemetryAggregateInteractor {
    fn emit_archived(
        &self,
        items_dir: &Path,
        track_id: &str,
        subcommand: String,
        exit_code: i32,
        duration_ms: u64,
    ) -> Result<(), TelemetryAggregateServiceError> {
        let track_id = domain::TrackId::try_new(track_id.to_owned()).map_err(|e| {
            TelemetryAggregateServiceError::EmitUnavailable(format!("invalid track ID: {e}"))
        })?;
        self.archived.emit_archived(items_dir, &track_id, subcommand, exit_code, duration_ms)
    }
}

impl TelemetryAggregateService for TelemetryAggregateInteractor {}

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

// ── TelemetryAggregateInteractor tests ──────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]
mod aggregate_interactor_tests {
    use std::num::NonZeroU64;
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};

    use crate::git_workflow::{
        ExplicitTrackBranch, GitPrimitivePort, GitWorkflowError, TrackBranchClaim,
    };

    use super::{
        ArchivedTelemetryFactoryPort, ArchivedTrackTelemetryError, ArchivedTrackTelemetryPort,
        TelemetryAggregateServiceError, TelemetryArchivedService, TelemetryEmitDynamicPort,
        TelemetryEmitDynamicPortError, TelemetryEmitService, TelemetryErrorEntry,
        TelemetryHookBlockEntry, TelemetryPhaseDuration, TelemetryReportError,
        TelemetryReportOutput, TelemetryReportPort, TelemetryReportService,
        command_trace::TelemetrySkippedLineCount,
    };
    use crate::capability_exec::{ModelName, ProviderName, ReasoningEffort};
    use crate::telemetry::command_trace::{
        CommandDurationMillis, CommandExecutionCount, CommandExecutionMetric, SotpCommandIdentity,
    };
    use crate::telemetry::review_yield::{
        ReviewDetectionRateBasisPoints, ReviewExecutionCount, ReviewFindingCount,
        ReviewYieldMetric, ReviewYieldValue, ReviewYieldValueError,
    };

    /// Minimal [`GitPrimitivePort`] stub: only `resolve_repo_root` is meaningful;
    /// every other primitive returns a benign default. `repo_root == None` makes
    /// `resolve_repo_root` fail (to exercise the emit error path).
    #[derive(Default)]
    struct StubGit {
        repo_root: Option<PathBuf>,
    }

    impl GitPrimitivePort for StubGit {
        fn current_branch(&self, _pr: Option<&Path>) -> Result<Option<String>, GitWorkflowError> {
            Ok(Some("track/feature-2026-07-04".to_owned()))
        }
        fn sync_current_branch(&self, _pr: Option<&Path>) -> Result<(), GitWorkflowError> {
            Ok(())
        }
        fn switch_branch(&self, _pr: Option<&Path>, _b: &str) -> Result<(), GitWorkflowError> {
            Ok(())
        }
        fn create_branch(
            &self,
            _pr: Option<&Path>,
            _n: &str,
            _b: &str,
        ) -> Result<(), GitWorkflowError> {
            Ok(())
        }
        fn branch_exists(&self, _pr: Option<&Path>, _b: &str) -> Result<bool, GitWorkflowError> {
            Ok(false)
        }
        fn move_path(
            &self,
            _pr: Option<&Path>,
            _s: &Path,
            _d: &Path,
        ) -> Result<(), GitWorkflowError> {
            Ok(())
        }
        fn fetch_branch(&self, _pr: Option<&Path>, _b: &str) -> Result<(), GitWorkflowError> {
            Ok(())
        }
        fn show_file_at_ref(
            &self,
            _pr: Option<&Path>,
            _r: &str,
            _p: &Path,
        ) -> Result<String, GitWorkflowError> {
            Ok(String::new())
        }
        fn resolve_commit(
            &self,
            _pr: Option<&Path>,
            _rev: &str,
        ) -> Result<Option<domain::CommitHash>, GitWorkflowError> {
            Ok(None)
        }
        fn resolve_repo_root(&self, _pr: Option<&Path>) -> Result<PathBuf, GitWorkflowError> {
            self.repo_root.clone().ok_or_else(|| {
                GitWorkflowError::Unavailable(crate::git_workflow::DiagnosticText::new(
                    "no repo root",
                ))
            })
        }
        fn stage_all(&self, _pr: Option<&Path>) -> Result<(), GitWorkflowError> {
            Ok(())
        }
        fn stage_from_file(
            &self,
            _pr: Option<&Path>,
            _p: &Path,
            _c: bool,
        ) -> Result<(), GitWorkflowError> {
            Ok(())
        }
        fn commit_from_message_file(
            &self,
            _pr: Option<&Path>,
            _p: &Path,
            _c: bool,
        ) -> Result<(), GitWorkflowError> {
            Ok(())
        }
        fn note_from_file(
            &self,
            _pr: Option<&Path>,
            _p: &Path,
            _c: bool,
        ) -> Result<(), GitWorkflowError> {
            Ok(())
        }
        fn unstage(&self, _pr: Option<&Path>, _paths: &[PathBuf]) -> Result<(), GitWorkflowError> {
            Ok(())
        }
        fn read_explicit_track_branch(
            &self,
            _pr: Option<&Path>,
            _td: &Path,
        ) -> Result<ExplicitTrackBranch, GitWorkflowError> {
            Ok(ExplicitTrackBranch {
                display_path: String::new(),
                expected_branch: None,
                status: None,
            })
        }
        fn collect_track_branch_claims(
            &self,
            _pr: Option<&Path>,
        ) -> Result<Vec<TrackBranchClaim>, GitWorkflowError> {
            Ok(Vec::new())
        }
    }

    /// Report port stub: returns a canned output or a canned error.
    struct StubReport {
        result: Mutex<Option<Result<TelemetryReportOutput, TelemetryReportError>>>,
    }

    impl TelemetryReportPort for StubReport {
        fn aggregate(
            &self,
            _track_id: &str,
            _items_dir: &Path,
        ) -> Result<TelemetryReportOutput, TelemetryReportError> {
            self.result.lock().unwrap().take().expect("aggregate called more than once")
        }
    }

    /// Records the `emit` calls it receives.
    #[derive(Default)]
    struct RecordingArchivedPort {
        calls: Mutex<Vec<String>>,
    }

    impl ArchivedTrackTelemetryPort for RecordingArchivedPort {
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

    #[derive(Default)]
    struct RecordingActivePort {
        calls: Mutex<Vec<ActiveEmission>>,
        attempts: Mutex<usize>,
        fail: bool,
    }

    #[derive(Debug, Eq, PartialEq)]
    struct ActiveEmission {
        items_dir: PathBuf,
        source_track_id: Option<String>,
        subcommand: String,
        exit_code: i32,
        duration_ms: u64,
        error_chain: Option<String>,
    }

    impl TelemetryEmitDynamicPort for RecordingActivePort {
        fn emit_active(
            &self,
            items_dir: &Path,
            source_track_id: Option<&str>,
            subcommand: String,
            exit_code: i32,
            duration_ms: u64,
            error_chain: Option<String>,
        ) -> Result<(), TelemetryEmitDynamicPortError> {
            *self.attempts.lock().unwrap() += 1;
            if self.fail {
                return Err(TelemetryEmitDynamicPortError::EmitUnavailable(
                    "active telemetry unavailable".to_owned(),
                ));
            }
            self.calls.lock().unwrap().push(ActiveEmission {
                items_dir: items_dir.to_owned(),
                source_track_id: source_track_id.map(str::to_owned),
                subcommand,
                exit_code,
                duration_ms,
                error_chain,
            });
            Ok(())
        }
    }

    /// Factory stub: records the `telemetry_dir` it was asked to build and hands
    /// back a shared [`RecordingArchivedPort`].
    struct StubFactory {
        built_dir: Mutex<Option<PathBuf>>,
        port: Arc<RecordingArchivedPort>,
    }

    impl ArchivedTelemetryFactoryPort for StubFactory {
        fn build(&self, telemetry_dir: &Path) -> Arc<dyn ArchivedTrackTelemetryPort> {
            *self.built_dir.lock().unwrap() = Some(telemetry_dir.to_path_buf());
            Arc::clone(&self.port) as Arc<dyn ArchivedTrackTelemetryPort>
        }
    }

    fn empty_report() -> TelemetryReportOutput {
        TelemetryReportOutput {
            phase_durations: Vec::<TelemetryPhaseDuration>::new(),
            errors: Vec::<TelemetryErrorEntry>::new(),
            hook_blocks: Vec::<TelemetryHookBlockEntry>::new(),
            skipped_lines: TelemetrySkippedLineCount::from(0),
            command_metrics: Vec::new(),
            review_yield_metrics: Vec::new(),
        }
    }

    fn build_aggregate(
        git: Arc<dyn super::GitPrimitivePort>,
        report: Arc<dyn super::TelemetryReportPort>,
        factory: Arc<dyn super::ArchivedTelemetryFactoryPort>,
        active: Arc<dyn super::TelemetryEmitDynamicPort>,
    ) -> super::TelemetryAggregateInteractor {
        let emit = Arc::new(super::TelemetryEmitInteractor::new(active));
        let archived = Arc::new(super::TelemetryArchiveInteractor::new(Arc::clone(&git), factory));
        super::TelemetryAggregateInteractor::new(
            super::TelemetryReportInteractor::new(report),
            emit,
            archived,
        )
    }

    fn facade(
        git: Arc<dyn super::GitPrimitivePort>,
        report: Arc<dyn super::TelemetryReportPort>,
        factory: Arc<dyn super::ArchivedTelemetryFactoryPort>,
    ) -> super::TelemetryAggregateInteractor {
        build_aggregate(git, report, factory, Arc::new(RecordingActivePort::default()))
    }

    fn facade_with_active(
        git: Arc<dyn super::GitPrimitivePort>,
        report: Arc<dyn super::TelemetryReportPort>,
        factory: Arc<dyn super::ArchivedTelemetryFactoryPort>,
        active: Arc<dyn super::TelemetryEmitDynamicPort>,
    ) -> super::TelemetryAggregateInteractor {
        build_aggregate(git, report, factory, active)
    }

    #[test]
    fn report_delegates_to_report_port() {
        let git = Arc::new(StubGit { repo_root: Some(PathBuf::from("/repo")) });
        let report = Arc::new(StubReport { result: Mutex::new(Some(Ok(empty_report()))) });
        let factory = Arc::new(StubFactory {
            built_dir: Mutex::new(None),
            port: Arc::new(RecordingArchivedPort::default()),
        });
        let interactor = facade(git, report, factory);

        let out = interactor.report("t", Path::new("track/items")).unwrap();
        assert_eq!(*out.skipped_lines.as_ref(), 0);
    }

    #[test]
    fn test_telemetry_aggregate_interactor_report_typed_metrics_preserves_values()
    -> Result<(), Box<dyn std::error::Error>> {
        let metric = CommandExecutionMetric::new(
            SotpCommandIdentity::try_new("telemetry".to_owned())?,
            CommandExecutionCount::from(4),
            CommandExecutionCount::from(1),
            CommandDurationMillis::from(240),
        )?;
        let output = TelemetryReportOutput {
            phase_durations: Vec::new(),
            errors: Vec::new(),
            hook_blocks: Vec::new(),
            skipped_lines: TelemetrySkippedLineCount::from(2),
            command_metrics: vec![metric],
            review_yield_metrics: Vec::new(),
        };
        let git = Arc::new(StubGit { repo_root: Some(PathBuf::from("/repo")) });
        let report = Arc::new(StubReport { result: Mutex::new(Some(Ok(output))) });
        let factory = Arc::new(StubFactory {
            built_dir: Mutex::new(None),
            port: Arc::new(RecordingArchivedPort::default()),
        });
        let interactor = facade(git, report, factory);

        let output = interactor.report("t", Path::new("track/items"))?;

        assert_eq!(*output.skipped_lines.as_ref(), 2);
        assert_eq!(output.command_metrics.len(), 1);
        assert_eq!(
            output.command_metrics.first().map(|metric| metric.command().as_str()),
            Some("telemetry")
        );
        assert_eq!(
            output.command_metrics.first().map(|metric| *metric.executions().as_ref()),
            Some(4)
        );
        assert_eq!(
            output.command_metrics.first().map(|metric| *metric.total_duration().as_ref()),
            Some(240)
        );
        assert_eq!(
            output.command_metrics.first().map(|metric| metric.failure_rate().value()),
            Some(2_500)
        );
        Ok(())
    }

    #[test]
    fn test_review_yield_value_objects_preserve_valid_counts_and_rate()
    -> Result<(), Box<dyn std::error::Error>> {
        let findings = ReviewFindingCount::new(2);
        let executions = ReviewExecutionCount::new(
            NonZeroU64::new(3).expect("the test execution count must be non-zero"),
        );
        let detection_rate = ReviewDetectionRateBasisPoints::try_new(6_666)?;

        assert_eq!(findings.value(), 2);
        assert_eq!(executions.to_string(), "3");
        assert_eq!(detection_rate.to_string(), "6666");
        Ok(())
    }

    #[test]
    fn test_review_yield_value_covers_each_recorded_axis() -> Result<(), Box<dyn std::error::Error>>
    {
        let values = [
            ReviewYieldValue::Scope(domain::review_v2::ScopeName::parse("architecture")?),
            ReviewYieldValue::RoundType(domain::review_v2::RoundType::Final),
            ReviewYieldValue::Provider(ProviderName::try_new("codex".to_owned())?),
            ReviewYieldValue::Model(ModelName::try_new("gpt-5".to_owned())?),
            ReviewYieldValue::ReasoningEffort(ReasoningEffort::High),
        ];

        assert_eq!(values.len(), 5);
        Ok(())
    }

    #[test]
    fn test_review_yield_detection_rate_rejects_out_of_range_value() {
        let result = ReviewDetectionRateBasisPoints::try_new(10_001);

        assert!(matches!(result, Err(ReviewYieldValueError::DetectionRateOutOfRange)));
    }

    #[test]
    fn test_telemetry_aggregate_interactor_report_preserves_review_yield_metrics()
    -> Result<(), Box<dyn std::error::Error>> {
        let metric = ReviewYieldMetric {
            value: ReviewYieldValue::Provider(ProviderName::try_new("codex".to_owned())?),
            execution_count: ReviewExecutionCount::new(
                NonZeroU64::new(3).expect("the test execution count must be non-zero"),
            ),
            detection_rate: ReviewDetectionRateBasisPoints::try_new(3_333)?,
        };
        let output = TelemetryReportOutput {
            phase_durations: Vec::new(),
            errors: Vec::new(),
            hook_blocks: Vec::new(),
            skipped_lines: TelemetrySkippedLineCount::from(0),
            command_metrics: Vec::new(),
            review_yield_metrics: vec![metric.clone()],
        };
        let report = Arc::new(StubReport { result: Mutex::new(Some(Ok(output))) });
        let factory = Arc::new(StubFactory {
            built_dir: Mutex::new(None),
            port: Arc::new(RecordingArchivedPort::default()),
        });
        let interactor =
            facade(Arc::new(StubGit { repo_root: Some(PathBuf::from("/repo")) }), report, factory);

        let output = interactor.report("t", Path::new("track/items"))?;

        assert_eq!(output.review_yield_metrics, vec![metric]);
        Ok(())
    }

    #[test]
    fn report_maps_port_error_to_report_unavailable() {
        let git = Arc::new(StubGit { repo_root: Some(PathBuf::from("/repo")) });
        let report = Arc::new(StubReport {
            result: Mutex::new(Some(Err(TelemetryReportError::TrackNotFound("t".to_owned())))),
        });
        let factory = Arc::new(StubFactory {
            built_dir: Mutex::new(None),
            port: Arc::new(RecordingArchivedPort::default()),
        });
        let interactor = facade(git, report, factory);

        let err = interactor.report("t", Path::new("track/items")).unwrap_err();
        assert!(matches!(err, TelemetryAggregateServiceError::ReportUnavailable(_)));
        assert!(err.to_string().contains("telemetry report:"));
    }

    #[test]
    fn test_telemetry_aggregate_interactor_emit_completed_records_identity_duration_exit_and_error()
    {
        let git = Arc::new(StubGit { repo_root: Some(PathBuf::from("/repo")) });
        let report = Arc::new(StubReport { result: Mutex::new(None) });
        let factory = Arc::new(StubFactory {
            built_dir: Mutex::new(None),
            port: Arc::new(RecordingArchivedPort::default()),
        });
        let active = Arc::new(RecordingActivePort::default());
        let interactor = facade_with_active(
            git,
            report,
            factory,
            Arc::clone(&active) as Arc<dyn TelemetryEmitDynamicPort>,
        );

        interactor
            .emit_completed(
                Path::new("/repo/track/items"),
                Some("track-id".to_owned()),
                "sotp dry".to_owned(),
                17,
                240,
                Some("command failed".to_owned()),
            )
            .unwrap();

        assert_eq!(
            active.calls.lock().unwrap().as_slice(),
            [ActiveEmission {
                items_dir: PathBuf::from("/repo/track/items"),
                source_track_id: Some("track-id".to_owned()),
                subcommand: "sotp dry".to_owned(),
                exit_code: 17,
                duration_ms: 240,
                error_chain: Some("command failed".to_owned()),
            }]
        );
    }

    #[test]
    fn test_telemetry_aggregate_interactor_emit_completed_forwards_completion() {
        let active = Arc::new(RecordingActivePort::default());
        let interactor = facade_with_active(
            Arc::new(StubGit { repo_root: Some(PathBuf::from("/repo")) }),
            Arc::new(StubReport { result: Mutex::new(None) }),
            Arc::new(StubFactory {
                built_dir: Mutex::new(None),
                port: Arc::new(RecordingArchivedPort::default()),
            }),
            Arc::clone(&active) as Arc<dyn TelemetryEmitDynamicPort>,
        );

        interactor
            .emit_completed(
                Path::new("/repo/track/items"),
                None,
                "sotp dry".to_owned(),
                0,
                240,
                None,
            )
            .unwrap();

        assert_eq!(
            active.calls.lock().unwrap().as_slice(),
            [ActiveEmission {
                items_dir: PathBuf::from("/repo/track/items"),
                source_track_id: None,
                subcommand: "sotp dry".to_owned(),
                exit_code: 0,
                duration_ms: 240,
                error_chain: None,
            }]
        );
    }

    #[test]
    fn test_telemetry_aggregate_service_reports_and_emits_completed_track_event() {
        let metric = CommandExecutionMetric::new(
            SotpCommandIdentity::try_new("sotp dry".to_owned()).unwrap(),
            CommandExecutionCount::from(2),
            CommandExecutionCount::from(1),
            CommandDurationMillis::from(240),
        )
        .unwrap();
        let report = Arc::new(StubReport {
            result: Mutex::new(Some(Ok(TelemetryReportOutput {
                phase_durations: Vec::new(),
                errors: Vec::new(),
                hook_blocks: Vec::new(),
                skipped_lines: TelemetrySkippedLineCount::from(0),
                command_metrics: vec![metric],
                review_yield_metrics: Vec::new(),
            }))),
        });
        let archived = Arc::new(RecordingArchivedPort::default());
        let factory =
            Arc::new(StubFactory { built_dir: Mutex::new(None), port: Arc::clone(&archived) });
        let active = Arc::new(RecordingActivePort::default());
        let interactor = facade_with_active(
            Arc::new(StubGit { repo_root: Some(PathBuf::from("/repo")) }),
            report,
            factory,
            Arc::clone(&active) as Arc<dyn TelemetryEmitDynamicPort>,
        );

        let output =
            interactor.report("feature-2026-07-04", Path::new("/repo/track/items")).unwrap();
        assert_eq!(output.command_metrics.len(), 1);
        assert_eq!(output.command_metrics[0].command().as_str(), "sotp dry");
        interactor
            .emit_archived(
                Path::new("/repo/track/items"),
                "feature-2026-07-04",
                "sotp dry".to_owned(),
                0,
                240,
            )
            .unwrap();
        interactor
            .emit_completed(
                Path::new("/repo/track/items"),
                Some("track-id".to_owned()),
                "sotp dry".to_owned(),
                17,
                240,
                Some("command failed".to_owned()),
            )
            .unwrap();

        assert_eq!(archived.calls.lock().unwrap().len(), 1);
        assert_eq!(
            active.calls.lock().unwrap().as_slice(),
            [ActiveEmission {
                items_dir: PathBuf::from("/repo/track/items"),
                source_track_id: Some("track-id".to_owned()),
                subcommand: "sotp dry".to_owned(),
                exit_code: 17,
                duration_ms: 240,
                error_chain: Some("command failed".to_owned()),
            }]
        );
    }

    #[test]
    fn test_telemetry_aggregate_interactor_emit_completed_reports_active_append_failure() {
        let active = Arc::new(RecordingActivePort {
            calls: Mutex::new(Vec::new()),
            attempts: Mutex::new(0),
            fail: true,
        });
        let interactor = facade_with_active(
            Arc::new(StubGit { repo_root: Some(PathBuf::from("/repo")) }),
            Arc::new(StubReport { result: Mutex::new(None) }),
            Arc::new(StubFactory {
                built_dir: Mutex::new(None),
                port: Arc::new(RecordingArchivedPort::default()),
            }),
            Arc::clone(&active) as Arc<dyn TelemetryEmitDynamicPort>,
        );

        let error = interactor
            .emit_completed(
                Path::new("/repo/track/items"),
                Some("track-id".to_owned()),
                "sotp dry".to_owned(),
                17,
                240,
                None,
            )
            .unwrap_err();
        assert!(matches!(error, TelemetryAggregateServiceError::EmitUnavailable(_)));
        assert!(active.calls.lock().unwrap().is_empty());
        assert_eq!(*active.attempts.lock().unwrap(), 1);
    }

    #[test]
    fn emit_archived_derives_telemetry_dir_and_drives_port() {
        let git = Arc::new(StubGit { repo_root: Some(PathBuf::from("/repo")) });
        let report = Arc::new(StubReport { result: Mutex::new(None) });
        let archived = Arc::new(RecordingArchivedPort::default());
        let factory =
            Arc::new(StubFactory { built_dir: Mutex::new(None), port: Arc::clone(&archived) });
        let factory_port: Arc<dyn ArchivedTelemetryFactoryPort> = factory.clone();
        let interactor = facade(git, report, factory_port);

        interactor
            .emit_archived(
                Path::new("/work/track/items"),
                "feature-2026-07-04",
                "track init".to_owned(),
                0,
                42,
            )
            .unwrap();

        assert_eq!(
            factory.built_dir.lock().unwrap().as_deref(),
            Some(Path::new("/repo/track/archive/feature-2026-07-04/logs"))
        );
        let calls = archived.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls.first().map(String::as_str), Some("feature-2026-07-04|track init|0|42"));
    }

    #[test]
    fn test_telemetry_archived_service_reuses_telemetry_jsonl_without_rotation() {
        let git = Arc::new(StubGit { repo_root: Some(PathBuf::from("/repo")) });
        let report = Arc::new(StubReport { result: Mutex::new(None) });
        let archived = Arc::new(RecordingArchivedPort::default());
        let factory =
            Arc::new(StubFactory { built_dir: Mutex::new(None), port: Arc::clone(&archived) });
        let interactor = facade(git, report, factory.clone());

        interactor
            .emit_archived(
                Path::new("/work/track/items"),
                "feature-2026-07-04",
                "track archive".to_owned(),
                0,
                42,
            )
            .unwrap();

        let telemetry_dir = factory.built_dir.lock().unwrap().clone().unwrap();
        assert!(telemetry_dir.ends_with(Path::new("logs")));
        assert!(!telemetry_dir.to_string_lossy().contains("command-trace"));
        assert!(!telemetry_dir.join("telemetry.1.jsonl").exists());
        assert_eq!(archived.calls.lock().unwrap().len(), 1);
    }

    #[test]
    fn emit_archived_maps_repo_root_failure_to_emit_unavailable() {
        let git = Arc::new(StubGit { repo_root: None });
        let report = Arc::new(StubReport { result: Mutex::new(None) });
        let factory = Arc::new(StubFactory {
            built_dir: Mutex::new(None),
            port: Arc::new(RecordingArchivedPort::default()),
        });
        let interactor = facade(git, report, factory);

        let err = interactor
            .emit_archived(Path::new("/work/track/items"), "t", "track init".to_owned(), 0, 0)
            .unwrap_err();
        assert!(matches!(err, TelemetryAggregateServiceError::EmitUnavailable(_)));
    }

    #[test]
    fn emit_archived_rejects_invalid_track_id_before_building_port() {
        let git = Arc::new(StubGit { repo_root: Some(PathBuf::from("/repo")) });
        let report = Arc::new(StubReport { result: Mutex::new(None) });
        let factory = Arc::new(StubFactory {
            built_dir: Mutex::new(None),
            port: Arc::new(RecordingArchivedPort::default()),
        });
        let interactor = facade(git, report, factory.clone());

        let err = interactor
            .emit_archived(Path::new("/work/track/items"), "../evil", "track init".to_owned(), 0, 0)
            .unwrap_err();

        assert!(matches!(err, TelemetryAggregateServiceError::EmitUnavailable(_)));
        assert!(err.to_string().contains("invalid track ID"));
        assert_eq!(factory.built_dir.lock().unwrap().as_deref(), None);
    }

    #[test]
    fn emit_archived_maps_emit_failure_to_emit_unavailable() {
        struct FailingArchivedPort;

        impl ArchivedTrackTelemetryPort for FailingArchivedPort {
            fn emit(
                &self,
                _track_id: String,
                _subcommand: String,
                _exit_code: i32,
                _duration_ms: u64,
            ) -> Result<(), ArchivedTrackTelemetryError> {
                Err(ArchivedTrackTelemetryError::EmitUnavailable("test emit failure".to_owned()))
            }
        }

        struct FailingFactory;

        impl ArchivedTelemetryFactoryPort for FailingFactory {
            fn build(&self, _telemetry_dir: &Path) -> Arc<dyn ArchivedTrackTelemetryPort> {
                Arc::new(FailingArchivedPort)
            }
        }

        let git = Arc::new(StubGit { repo_root: Some(PathBuf::from("/repo")) });
        let report = Arc::new(StubReport { result: Mutex::new(None) });
        let interactor =
            facade(git, report, Arc::new(FailingFactory) as Arc<dyn ArchivedTelemetryFactoryPort>);

        let err = interactor
            .emit_archived(
                Path::new("/work/track/items"),
                "feature-2026-07-04",
                "track init".to_owned(),
                0,
                0,
            )
            .unwrap_err();

        assert!(matches!(err, TelemetryAggregateServiceError::EmitUnavailable(_)));
        assert!(err.to_string().contains("test emit failure"));
    }

    #[test]
    fn emit_archived_rejects_malformed_items_dir() {
        let git = Arc::new(StubGit { repo_root: Some(PathBuf::from("/repo")) });
        let report = Arc::new(StubReport { result: Mutex::new(None) });
        let factory = Arc::new(StubFactory {
            built_dir: Mutex::new(None),
            port: Arc::new(RecordingArchivedPort::default()),
        });
        let interactor = facade(git, report, factory);

        let err = interactor
            .emit_archived(Path::new("/work/not-items"), "t", "track init".to_owned(), 0, 0)
            .unwrap_err();
        assert!(matches!(err, TelemetryAggregateServiceError::EmitUnavailable(_)));
        assert!(err.to_string().contains("--items-dir must point to"));
    }
}
