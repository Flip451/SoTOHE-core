//! `sotp telemetry` subcommand composition — per-context composition root.
//!
//! Provides:
//! - `TelemetryCompositionRoot::telemetry_driver`: builds a wired
//!   `TelemetryDriver` backed by `usecase::telemetry::TelemetryAggregateInteractor`.
//!
//! This module is wiring-only (CN-03): it constructs the infrastructure
//! adapters (`FsGitWorkflowAdapter`, `FsTelemetryReportAdapter`,
//! `FsArchivedTelemetryFactoryAdapter`) and injects them into the usecase
//! `TelemetryAggregateInteractor`, which owns report and emission ports. The
//! primary driver keeps command dispatch and timing at the adapter boundary;
//! no completion orchestration lives here.

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

impl TelemetryCompositionRoot {
    /// Build a wired [`cli_driver::telemetry::TelemetryDriver`] for the telemetry
    /// family.
    ///
    /// Wiring-only (CN-03): constructs the infrastructure adapters and injects
    /// them into `usecase::telemetry::TelemetryAggregateInteractor`.
    pub fn telemetry_driver(&self) -> cli_driver::telemetry::TelemetryDriver {
        use infrastructure::{
            FsArchivedTelemetryFactoryAdapter, FsGitWorkflowAdapter, FsTelemetryEmitDynamicAdapter,
            FsTelemetryReportAdapter,
        };
        use usecase::git_workflow::GitPrimitivePort;
        use usecase::telemetry::{
            ArchivedTelemetryFactoryPort, TelemetryAggregateInteractor, TelemetryArchiveInteractor,
            TelemetryEmitDynamicPort, TelemetryEmitInteractor, TelemetryReportInteractor,
            TelemetryReportPort,
        };

        let git: Arc<dyn GitPrimitivePort> = Arc::new(FsGitWorkflowAdapter::new());
        let report_port: Arc<dyn TelemetryReportPort> = Arc::new(FsTelemetryReportAdapter::new());
        let archived_factory: Arc<dyn ArchivedTelemetryFactoryPort> =
            Arc::new(FsArchivedTelemetryFactoryAdapter::new());
        let active_emit: Arc<dyn TelemetryEmitDynamicPort> =
            Arc::new(FsTelemetryEmitDynamicAdapter::new());
        let emit = Arc::new(TelemetryEmitInteractor::new(Arc::clone(&active_emit)));
        let archived = Arc::new(TelemetryArchiveInteractor::new(
            Arc::clone(&git),
            Arc::clone(&archived_factory),
        ));
        let service = Arc::new(TelemetryAggregateInteractor::new(
            TelemetryReportInteractor::new(report_port),
            emit,
            archived,
        )) as Arc<dyn usecase::TelemetryAggregateService>;
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
