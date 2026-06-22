// STAGED FOR T021 — not yet compiled; Cargo.toml + workspace member added atomically in T021 per CN-06.
//
//! `telemetry` command family — primary adapter driver.
//!
//! `TelemetryDriver` holds injected use-case interactors and exposes
//! `handle(input) -> CommandOutcome`.  The formatting helpers here mirror
//! `apps/cli-composition/src/telemetry.rs`;
//! T021 removes the `cli_composition` duplicate when the live path is flipped.

// TODO(T021): add use-case + infrastructure imports once Cargo.toml is materialized.
// use std::path::{Path, PathBuf};
// use std::sync::Arc;
// use infrastructure::FsArchivedTrackTelemetryAdapter;
// use infrastructure::git_cli::GitRepository as _;
// use infrastructure::telemetry::{TelemetryReport, TelemetryReportOutput, PhaseDuration, ...};
// use usecase::telemetry::{
//     ArchivedTrackTelemetryCommand, ArchivedTrackTelemetryInteractor,
//     ArchivedTrackTelemetryService as _,
// };

use std::path::{Path, PathBuf};

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
    },
}

// ---------------------------------------------------------------------------
// Driver
// ---------------------------------------------------------------------------

/// Primary adapter driver for the `telemetry` command family.
///
/// Holds injected use-case interactors; exposes `handle(input) -> CommandOutcome`.
pub struct TelemetryDriver {
    // TODO(T021): inject use-case interactors here (currently this family has
    // no injectable adapter dependencies — infrastructure functions are called
    // inline, same as cli_composition::TelemetryCompositionRoot).
}

impl TelemetryDriver {
    /// Create a new `TelemetryDriver`.
    ///
    /// TODO(T021): accept injected interactors as parameters once the crate
    /// dependency graph is materialized.
    pub fn new() -> Self {
        Self {}
    }

    /// Handle a telemetry command.
    ///
    /// TODO(T021): wire real use-case invocation once Cargo.toml is materialized.
    pub fn handle(&self, input: TelemetryInput) -> CommandOutcome {
        match input {
            TelemetryInput::Report(input) => self.telemetry_report(input),
            TelemetryInput::EmitArchivedTrackSubcommand { items_dir, track_id, subcommand } => {
                self.telemetry_emit_archived_track_subcommand(&items_dir, &track_id, subcommand)
            }
        }
    }

    // -----------------------------------------------------------------------
    // Render helpers (logic duplicated from cli_composition/src/telemetry.rs;
    // T021 removes the cli_composition copy).
    // -----------------------------------------------------------------------

    fn telemetry_report(&self, _input: TelemetryReportInput) -> CommandOutcome {
        // TODO(T021): invoke infrastructure::telemetry::TelemetryReport::aggregate and
        // format_report(&input.track_id, &output) here.
        // Mirrors cli_composition/src/telemetry.rs TelemetryCompositionRoot::telemetry_report.
        CommandOutcome::success(None)
    }

    fn telemetry_emit_archived_track_subcommand(
        &self,
        _items_dir: &Path,
        _track_id: &str,
        _subcommand: String,
    ) -> CommandOutcome {
        // TODO(T021): derive project root, discover git repo, build
        // FsArchivedTrackTelemetryAdapter + ArchivedTrackTelemetryInteractor
        // and call interactor.emit(ArchivedTrackTelemetryCommand { subcommand }) here.
        // Mirrors cli_composition/src/telemetry.rs TelemetryCompositionRoot::telemetry_emit_archived_track_subcommand.
        CommandOutcome::success(None)
    }
}

impl Default for TelemetryDriver {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Formatting helpers (duplicated from cli_composition/src/telemetry.rs;
// T021 removes the cli_composition copy).
// ---------------------------------------------------------------------------

/// Format a `TelemetryReportOutput` as a human-readable text report.
///
/// Mirrors `cli_composition::telemetry::format_report`.
#[allow(dead_code)]
fn format_report(track_id: &str, phase_section: &str) -> String {
    // TODO(T021): accept `&infrastructure::telemetry::TelemetryReportOutput` and
    // implement the full section-by-section format:
    //   "Telemetry report for track: {track_id}"
    //   "Phase durations:" / "  (no phase data recorded)"
    //   "Errors ({n}):" / "  (none)" / "  [{timestamp}] {command} (exit {exit_code}): {error_chain}"
    //   "Hook blocks ({n}):" / "  (none)" / "  [{timestamp}] {hook_name}"
    //   "Skipped lines: {n}"
    // Kept as a named stub with placeholder signature until T021.
    let _ = phase_section;
    format!("Telemetry report for track: {track_id}\n")
}
