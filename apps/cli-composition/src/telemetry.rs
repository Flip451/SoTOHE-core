//! `sotp telemetry` subcommand composition — per-context composition root.
//!
//! Provides:
//! - `TelemetryCompositionRoot::telemetry_driver`: builds a wired
//!   `TelemetryDriver` backed by `TelemetryAggregateServiceImpl`.
//!
//! `TelemetryAggregateServiceImpl` implements both methods of
//! `TelemetryAggregateService`:
//!   - `report`: aggregates telemetry data via `FsTelemetryReportAdapter` and
//!     returns a `TelemetryReportOutput` DTO. Presentation formatting is
//!     delegated to `cli_driver::telemetry::TelemetryDriver` (OS-04 / AC-06).
//!   - `emit_archived`: wires `FsArchivedTrackTelemetryAdapter` →
//!     `ArchivedTrackTelemetryInteractor` and emits a single archived-track
//!     telemetry event (D8 / AC-10).
//!
//! No telemetry writer is constructed or invoked here for `report`; it is a
//! pure data-aggregation command that reads and returns structured data
//! without emitting any `TelemetryEvent` itself.

use std::path::PathBuf;
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Per-context composition root
// ---------------------------------------------------------------------------

/// Composition root for the `telemetry` command family.
///
/// Unit struct: no adapter dependencies are injected at construction time.
pub struct TelemetryCompositionRoot;

impl TelemetryCompositionRoot {
    /// Create a new `TelemetryCompositionRoot`.
    pub fn new() -> Self {
        Self
    }
}

impl Default for TelemetryCompositionRoot {
    fn default() -> Self {
        Self::new()
    }
}

/// Input DTO for `sotp telemetry report`.
#[derive(Debug, Clone)]
pub struct TelemetryReportInput {
    /// Track ID whose telemetry log should be aggregated.
    pub track_id: String,
    /// Path to the track items directory (e.g. `track/items`).
    pub items_dir: PathBuf,
}

/// Concrete implementation of `TelemetryAggregateService` that wires
/// the infrastructure adapters for both the `report` and `emit_archived`
/// service methods.
///
/// `report` returns the aggregated `TelemetryReportOutput` DTO directly;
/// presentation formatting is performed by `cli_driver::telemetry::TelemetryDriver`.
/// `emit_archived` wires `FsArchivedTrackTelemetryAdapter` →
/// `ArchivedTrackTelemetryInteractor` inline (no composition-root helper shim).
struct TelemetryAggregateServiceImpl;

impl usecase::TelemetryAggregateService for TelemetryAggregateServiceImpl {
    fn report(
        &self,
        track_id: &str,
        items_dir: &std::path::Path,
    ) -> Result<
        usecase::telemetry::TelemetryReportOutput,
        usecase::telemetry::TelemetryAggregateServiceError,
    > {
        use infrastructure::FsTelemetryReportAdapter;
        use usecase::telemetry::{TelemetryAggregateServiceError, TelemetryReportPort as _};
        let adapter = FsTelemetryReportAdapter::new();
        adapter.aggregate(track_id, items_dir).map_err(|e| {
            TelemetryAggregateServiceError::ReportUnavailable(format!("telemetry report: {e}"))
        })
    }

    fn emit_archived(
        &self,
        items_dir: &std::path::Path,
        track_id: &str,
        subcommand: String,
        exit_code: i32,
        duration_ms: u64,
    ) -> Result<(), usecase::telemetry::TelemetryAggregateServiceError> {
        use infrastructure::FsArchivedTrackTelemetryAdapter;
        use infrastructure::git_cli::GitRepository as _;
        use usecase::telemetry::{
            ArchivedTrackTelemetryCommand, ArchivedTrackTelemetryInteractor,
            ArchivedTrackTelemetryService as _, TelemetryAggregateServiceError,
        };

        // Derive the project root, then discover the repo to get an absolute root path.
        let project_root = crate::track::resolve_project_root(items_dir)
            .map_err(|e| TelemetryAggregateServiceError::EmitUnavailable(e.to_string()))?;
        let repo =
            infrastructure::git_cli::SystemGitRepo::discover_from(&project_root).map_err(|e| {
                TelemetryAggregateServiceError::EmitUnavailable(format!(
                    "failed to discover git repository: {e}"
                ))
            })?;
        let repo_root = repo.root().to_path_buf();

        // Telemetry directory for the archived track.
        let telemetry_dir = repo_root.join("track").join("archive").join(track_id).join("logs");

        let adapter = FsArchivedTrackTelemetryAdapter::new(telemetry_dir);
        let interactor = ArchivedTrackTelemetryInteractor::new(Arc::new(adapter));

        interactor
            .emit(ArchivedTrackTelemetryCommand {
                subcommand,
                track_id: track_id.to_owned(),
                exit_code,
                duration_ms,
            })
            .map_err(|e: usecase::telemetry::ArchivedTrackTelemetryError| {
                TelemetryAggregateServiceError::EmitUnavailable(e.to_string())
            })
    }
}

impl TelemetryCompositionRoot {
    /// Build a wired [`cli_driver::telemetry::TelemetryDriver`] for the telemetry family.
    pub fn telemetry_driver(&self) -> cli_driver::telemetry::TelemetryDriver {
        let service =
            Arc::new(TelemetryAggregateServiceImpl) as Arc<dyn usecase::TelemetryAggregateService>;
        cli_driver::telemetry::TelemetryDriver::new(service)
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::io::Write;

    use super::*;

    fn write_jsonl_fixture(items_dir: &std::path::Path, track_id: &str, lines: &[&str]) {
        let logs_dir = items_dir.join(track_id).join("logs");
        std::fs::create_dir_all(&logs_dir).unwrap();
        let mut file = std::fs::File::create(logs_dir.join("telemetry.jsonl")).unwrap();
        for line in lines {
            file.write_all(line.as_bytes()).unwrap();
            file.write_all(b"\n").unwrap();
        }
    }

    fn setup_repo_with_items(track_id: &str) -> tempfile::TempDir {
        let tmp = tempfile::TempDir::new().unwrap();
        crate::test_support::seed_repo(tmp.path(), &format!("track/{track_id}"));
        std::fs::create_dir_all(tmp.path().join("track").join("items").join(track_id)).unwrap();
        tmp
    }

    const SUBCOMMAND_LINE: &str = r#"{"event_type":"TrackSubcommand","schema_version":1,"track_id":"t","command":"track spec-design","exit_code":0,"duration_ms":1200,"timestamp":"2026-06-10T00:00:00Z"}"#;
    const NON_ZERO_EXIT_LINE: &str = r#"{"event_type":"NonZeroExit","schema_version":1,"track_id":"t","command":"track spec-design","exit_code":1,"error_chain":"gate failed","timestamp":"2026-06-10T01:00:00Z"}"#;
    const HOOK_BLOCK_LINE: &str = r#"{"event_type":"HookBlock","schema_version":1,"track_id":"t","hook_name":"block-direct-git-ops","timestamp":"2026-06-10T02:00:00Z"}"#;

    // ── telemetry driver: happy path ──────────────────────────────────────────

    #[test]
    fn test_telemetry_driver_report_happy_path_exits_zero_with_output() {
        let _guard = crate::test_support::process_env_lock().lock().unwrap();
        let tmp = setup_repo_with_items("t");
        let items_dir = tmp.path().join("track").join("items");
        write_jsonl_fixture(
            &items_dir,
            "t",
            &[SUBCOMMAND_LINE, NON_ZERO_EXIT_LINE, HOOK_BLOCK_LINE],
        );

        let outcome = crate::test_support::run_in_dir(tmp.path(), || {
            TelemetryCompositionRoot::new().telemetry_driver().handle(
                cli_driver::telemetry::TelemetryInput::Report(
                    cli_driver::telemetry::TelemetryReportInput {
                        track_id: "t".to_owned(),
                        items_dir: std::path::PathBuf::from("track/items"),
                    },
                ),
            )
        });
        assert_eq!(outcome.exit_code, 0);

        let text = outcome.stdout.unwrap();
        assert!(text.contains("track spec-design"), "phase name must appear in output");
        assert!(text.contains("1200"), "phase duration must appear in output");
        assert!(text.contains("gate failed"), "error chain must appear in output");
        assert!(text.contains("block-direct-git-ops"), "hook name must appear in output");
        assert!(text.contains("Skipped lines: 0"), "skip count must always appear");
    }

    // ── telemetry driver: skipped lines ──────────────────────────────────────

    #[test]
    fn test_telemetry_driver_report_shows_skipped_line_count_when_nonzero() {
        let _guard = crate::test_support::process_env_lock().lock().unwrap();
        let tmp = setup_repo_with_items("t");
        let items_dir = tmp.path().join("track").join("items");
        write_jsonl_fixture(&items_dir, "t", &[SUBCOMMAND_LINE, "not valid json", HOOK_BLOCK_LINE]);

        let outcome = crate::test_support::run_in_dir(tmp.path(), || {
            TelemetryCompositionRoot::new().telemetry_driver().handle(
                cli_driver::telemetry::TelemetryInput::Report(
                    cli_driver::telemetry::TelemetryReportInput {
                        track_id: "t".to_owned(),
                        items_dir: std::path::PathBuf::from("track/items"),
                    },
                ),
            )
        });

        assert_eq!(outcome.exit_code, 0, "skipped lines must not fail the command (AC-09)");
        let text = outcome.stdout.unwrap();
        assert!(text.contains("Skipped lines: 1"), "skipped count must be shown; got: {text}");
    }

    // ── telemetry driver: empty log ───────────────────────────────────────────

    #[test]
    fn test_telemetry_driver_report_missing_log_exits_zero_with_empty_report() {
        let _guard = crate::test_support::process_env_lock().lock().unwrap();
        let tmp = setup_repo_with_items("t");

        let outcome = crate::test_support::run_in_dir(tmp.path(), || {
            TelemetryCompositionRoot::new().telemetry_driver().handle(
                cli_driver::telemetry::TelemetryInput::Report(
                    cli_driver::telemetry::TelemetryReportInput {
                        track_id: "t".to_owned(),
                        items_dir: std::path::PathBuf::from("track/items"),
                    },
                ),
            )
        });

        assert_eq!(outcome.exit_code, 0);
        let text = outcome.stdout.unwrap();
        assert!(text.contains("(no phase data recorded)"), "empty report must note absence");
        assert!(text.contains("Skipped lines: 0"), "empty report must still show skip count");
    }

    // ── telemetry driver: track not found ────────────────────────────────────

    #[test]
    fn test_telemetry_driver_report_track_not_found_returns_failure_outcome() {
        let _guard = crate::test_support::process_env_lock().lock().unwrap();
        let tmp = tempfile::TempDir::new().unwrap();
        crate::test_support::seed_repo(tmp.path(), "track/main-init");
        std::fs::create_dir_all(tmp.path().join("track").join("items")).unwrap();

        let outcome = crate::test_support::run_in_dir(tmp.path(), || {
            TelemetryCompositionRoot::new().telemetry_driver().handle(
                cli_driver::telemetry::TelemetryInput::Report(
                    cli_driver::telemetry::TelemetryReportInput {
                        track_id: "does-not-exist".to_owned(),
                        items_dir: std::path::PathBuf::from("track/items"),
                    },
                ),
            )
        });

        assert_ne!(outcome.exit_code, 0, "missing track must produce a non-zero exit");
        let msg = outcome.stderr.unwrap_or_default();
        assert!(
            msg.contains("does-not-exist") || msg.contains("track not found"),
            "error must mention track id; got: {msg}"
        );
    }
}
