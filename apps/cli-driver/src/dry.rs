// STAGED FOR T021 — not yet compiled; Cargo.toml + workspace member added atomically in T021 per CN-06.
//
//! `dry` command family — primary adapter driver.
//!
//! `DryDriver` holds injected use-case interactors and exposes
//! `handle(input) -> CommandOutcome`.  The render helper here mirrors the
//! per-record formatter in
//! `apps/cli-composition/src/dry.rs` (lines 437-492 `dry_results` per-record
//! formatter + result assembly);
//! T021 removes the `cli_composition` duplicate when the live path is flipped.

// TODO(T021): add use-case + infrastructure imports once Cargo.toml is materialized.
// use std::path::PathBuf;
// use std::sync::Arc;
// use domain::dry_check::{DryCheckReader as _, DryCheckVerdict};
// use infrastructure::dry_check::FsDryCheckStore;
// use infrastructure::git_cli::{GitRepository, SystemGitRepo};
// use usecase::dry_check::{DryCheckApprovalInteractor, DryCheckApprovalService as _,
//     DryCheckInteractor, DryCheckResultsInteractor, DryCheckResultsService as _,
//     DryCheckService as _, fragment_ref_of};

use std::path::PathBuf;

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
/// Holds injected use-case interactors; exposes `handle(input) -> CommandOutcome`.
pub struct DryDriver {
    // TODO(T021): inject use-case interactors here once the crate dependency
    // graph is materialized.
}

impl DryDriver {
    /// Create a new `DryDriver`.
    ///
    /// TODO(T021): accept injected interactors as parameters once the crate
    /// dependency graph is materialized.
    pub fn new() -> Self {
        Self {}
    }

    /// Handle a dry command.
    ///
    /// TODO(T021): wire real use-case invocation once Cargo.toml is materialized.
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
    // Render helpers (logic duplicated from cli_composition/src/dry.rs
    // lines 437-492; T021 removes the cli_composition copies).
    // -----------------------------------------------------------------------

    #[allow(clippy::too_many_arguments)]
    fn dry_write(
        &self,
        _track_id: String,
        _base_commit: Option<String>,
        _db_path: PathBuf,
        _threshold: Option<f32>,
        _workspace_root: PathBuf,
        _items_dir: PathBuf,
        _model: Option<String>,
        _capability_name: String,
    ) -> CommandOutcome {
        // TODO(T021): build DryWriteInput, invoke DryCompositionRoot::dry_write,
        // propagate CompositionError to exit-code 1.
        // Mirrors cli_composition/src/dry.rs DryCompositionRoot::dry_write.
        CommandOutcome::success(None)
    }

    fn dry_results(
        &self,
        _track_id: String,
        _filter: String,
        _items_dir: PathBuf,
    ) -> CommandOutcome {
        // TODO(T021): discover git root, resolve items_dir, parse verdict filter,
        // open FsDryCheckStore, invoke DryCheckResultsInteractor::get_results,
        // then format each record via format_dry_results_str.
        // Mirrors cli_composition/src/dry.rs DryCompositionRoot::dry_results (lines 450-512).
        CommandOutcome::success(None)
    }

    fn dry_check_approved(
        &self,
        _track_id: String,
        _base_commit: Option<String>,
        _items_dir: PathBuf,
    ) -> CommandOutcome {
        // TODO(T021): resolve diff base, open stores, invoke DryCheckApprovalInteractor,
        // return CommandOutcome reflecting DryCheckApprovedOutcome (exit 0 / 1).
        // Mirrors cli_composition/src/dry.rs DryCompositionRoot::dry_check_approved.
        CommandOutcome::success(None)
    }

    fn dry_fix_local(
        &self,
        _track_id: String,
        _briefing_file: PathBuf,
        _model: Option<String>,
    ) -> CommandOutcome {
        // TODO(T021): resolve dry-fix-lead capability from agent-profiles.json,
        // build RunDryFixLocalInput, invoke fixer, emit terminal status line.
        // Mirrors cli_composition/src/dry.rs DryCompositionRoot::dry_run_fix_local.
        CommandOutcome::success(None)
    }
}

impl Default for DryDriver {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Render helpers (duplicated from cli_composition/src/dry.rs lines 437-492;
// T021 removes the cli_composition copies and moves these to cli_driver::render).
// ---------------------------------------------------------------------------

/// Format the `sotp dry results` output as a string, given the record list.
///
/// Mirrors the per-record formatter in
/// `cli_composition::dry::DryCompositionRoot::dry_results`
/// (dry.rs lines 481-511 — the per-record loop and summary header).
///
/// TODO(T021): wire real domain types (`domain::dry_check::DryCheckRecord`,
/// `domain::dry_check::DryCheckVerdict`) once the dependency graph is materialized.
/// Currently returns a placeholder so the staged file is self-consistent.
#[allow(dead_code)]
fn format_dry_results_str(
    _records_len: usize,
    // TODO(T021): replace with real `&[domain::dry_check::DryCheckRecord]` slice.
    _records: &[()],
) -> String {
    // TODO(T021): paste the per-record rendering loop from
    // cli_composition/src/dry.rs lines 481-511 once `domain::dry_check` types
    // are available as dependencies. The implementation:
    //
    //   1. lines.push(format!("dry results: {} record(s)", records.len()))
    //   2. For each record:
    //      - push "  pair: [{low.path} ({low.hash})] <-> [{high.path} ({high.hash})]"
    //      - push "  changed_path (display-only): {record.changed_path()}"
    //      - match record.verdict():
    //          NotAViolation => "not-a-violation"
    //          Accepted      => "accepted"
    //          Violation { refactor_proposal } => "violation | proposal: {proposal}"
    //      - push "  verdict: {verdict_str}"
    //      - push "  score: {score} | threshold: {threshold} | base: {base_commit}"
    //      - push "  rationale: {record.rationale()}"
    //      - push "  recorded_at: {record.recorded_at()}"
    //   3. lines.join("\n")
    String::new()
}
