//! `dry` command family — primary adapter driver.
//!
//! `DryDriver` holds an injected [`usecase::dry_driver::DryDriverService`] and
//! exposes `handle(input) -> CommandOutcome`.

use std::path::PathBuf;
use std::sync::Arc;

use usecase::dry_driver::{
    DryCheckApprovedDriverInput, DryDriverOutcome, DryDriverService, DryFixLocalDriverInput,
    DryResultsDriverInput, DryWriteDriverInput,
};

use crate::render::CommandOutcome;

// ---------------------------------------------------------------------------
// Input type
// ---------------------------------------------------------------------------

/// Typed input for the `dry` command family.
pub enum DryInput {
    /// Run the DRY-check write cycle: detect near-duplicate violations in the
    /// current diff scope and record results to dry-check.json.
    Write {
        /// Track ID used to locate the per-track dry-check.json and .commit_hash.
        track_id: String,
        /// Optional explicit base commit (overrides .commit_hash store lookup).
        base_commit: Option<String>,
        /// Path to the LanceDB semantic index database.
        db_path: PathBuf,
        /// Cosine similarity threshold (0.0–1.0) for the dry-check gate.
        threshold: Option<f32>,
        /// Root of the workspace to scan for Rust sources (corpus extraction).
        workspace_root: PathBuf,
        /// Path to the track items directory.
        items_dir: PathBuf,
        /// Codex model name for the DryCheckAgentPort.
        model: Option<String>,
        /// Capability name forwarded to CodexDryChecker.
        capability_name: String,
    },
    /// Show the historical dry-check results (informational, always exits 0).
    Results {
        /// Track ID used to locate the per-track dry-check.json.
        track_id: String,
        /// Verdict filter: `"all"` / `"not-a-violation"` / `"accepted"` / `"violation"`.
        filter: String,
        /// Path to the track items directory.
        items_dir: PathBuf,
    },
    /// Gate: exit 0 when all above-threshold pairs are verified; non-zero otherwise.
    CheckApproved {
        /// Track ID used to locate the per-track dry-check.json and .commit_hash.
        track_id: String,
        /// Optional explicit base commit (overrides .commit_hash store lookup).
        base_commit: Option<String>,
        /// Path to the track items directory.
        items_dir: PathBuf,
    },
    /// Run the dry-fix-lead fixer with provider auto-resolved from agent-profiles.json.
    FixLocal {
        /// Track ID. Required (no auto-resolve from branch for write operations).
        track_id: String,
        /// Path to the briefing file passed to the dry-fix-lead fixer. Required.
        briefing_file: PathBuf,
        /// Model for the fixer subprocess.
        model: Option<String>,
    },
}

// ---------------------------------------------------------------------------
// Driver
// ---------------------------------------------------------------------------

/// Primary adapter driver for the `dry` command family.
///
/// Holds an injected [`DryDriverService`]; exposes `handle(input) -> CommandOutcome`.
pub struct DryDriver {
    service: Arc<dyn DryDriverService>,
}

impl DryDriver {
    /// Create a new `DryDriver` with the given service.
    pub fn new(service: Arc<dyn DryDriverService>) -> Self {
        Self { service }
    }

    /// Handle a dry command.
    pub fn handle(&self, input: DryInput) -> CommandOutcome {
        match input {
            DryInput::Write {
                track_id,
                base_commit,
                db_path,
                threshold,
                workspace_root,
                items_dir,
                model,
                capability_name,
            } => self.dry_write(
                track_id,
                base_commit,
                db_path,
                threshold,
                workspace_root,
                items_dir,
                model,
                capability_name,
            ),
            DryInput::Results { track_id, filter, items_dir } => {
                self.dry_results(track_id, filter, items_dir)
            }
            DryInput::CheckApproved { track_id, base_commit, items_dir } => {
                self.dry_check_approved(track_id, base_commit, items_dir)
            }
            DryInput::FixLocal { track_id, briefing_file, model } => {
                self.dry_fix_local(track_id, briefing_file, model)
            }
        }
    }

    // -----------------------------------------------------------------------
    // Private helpers — translate input fields → service calls
    // -----------------------------------------------------------------------

    #[allow(clippy::too_many_arguments)]
    fn dry_write(
        &self,
        track_id: String,
        base_commit: Option<String>,
        db_path: PathBuf,
        threshold: Option<f32>,
        workspace_root: PathBuf,
        items_dir: PathBuf,
        model: Option<String>,
        capability_name: String,
    ) -> CommandOutcome {
        let outcome = self.service.dry_write(DryWriteDriverInput {
            track_id,
            base_commit,
            db_path,
            threshold,
            workspace_root,
            items_dir,
            model,
            capability_name,
        });
        into_command_outcome(outcome)
    }

    fn dry_results(&self, track_id: String, filter: String, items_dir: PathBuf) -> CommandOutcome {
        let outcome =
            self.service.dry_results(DryResultsDriverInput { track_id, filter, items_dir });
        into_command_outcome(outcome)
    }

    fn dry_check_approved(
        &self,
        track_id: String,
        base_commit: Option<String>,
        items_dir: PathBuf,
    ) -> CommandOutcome {
        let outcome = self.service.dry_check_approved(DryCheckApprovedDriverInput {
            track_id,
            base_commit,
            items_dir,
        });
        into_command_outcome(outcome)
    }

    fn dry_fix_local(
        &self,
        track_id: String,
        briefing_file: PathBuf,
        model: Option<String>,
    ) -> CommandOutcome {
        let outcome =
            self.service.dry_fix_local(DryFixLocalDriverInput { track_id, briefing_file, model });
        into_command_outcome(outcome)
    }
}

// ---------------------------------------------------------------------------
// Conversion helper
// ---------------------------------------------------------------------------

/// Convert a `DryDriverOutcome` (usecase boundary type) into a `CommandOutcome`.
fn into_command_outcome(outcome: DryDriverOutcome) -> CommandOutcome {
    CommandOutcome { stdout: outcome.stdout, stderr: outcome.stderr, exit_code: outcome.exit_code }
}
