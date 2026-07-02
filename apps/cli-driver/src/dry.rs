//! `dry` command family — primary adapter driver.
//!
//! `DryDriver` holds an injected [`usecase::dry_driver::DryDriverService`] and
//! exposes `handle(input) -> CommandOutcome`.

use std::path::PathBuf;
use std::sync::Arc;

use usecase::dry_driver::{
    DryCheckApprovedDriverInput, DryCheckApprovedOutcome, DryDriverOutcome, DryDriverService,
    DryFixLocalDriverInput, DryResultsDriverInput, DryWriteDriverInput, DryWriteOutcome,
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
        render_dry_write_outcome(outcome)
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
        render_dry_check_approved_outcome(outcome)
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

/// Render a `DryWriteOutcome` (usecase boundary type) into a `CommandOutcome`.
///
/// Reproduces byte-for-byte the text previously produced by cli_composition's
/// `dry_write_outcome` helper (IN-13/AC-18, CN-07/AC-19).
fn render_dry_write_outcome(outcome: DryWriteOutcome) -> CommandOutcome {
    match outcome {
        DryWriteOutcome::Success {
            pairs_checked,
            records_appended,
            diff_fragments_processed,
            findings,
        } => {
            let mut output_lines: Vec<String> = Vec::new();
            output_lines.push(format!(
                "dry write: {pairs_checked} pair(s) checked; {records_appended} record(s) appended; \
                 {} violation(s) found; {diff_fragments_processed} diff fragment(s) processed",
                findings.len()
            ));
            for finding in &findings {
                output_lines.push(format!(
                    "  changed: {} (hash: {})",
                    finding.changed_path, finding.changed_content_hash,
                ));
                output_lines.push(format!(
                    "  candidate: {} (hash: {})",
                    finding.candidate_path, finding.candidate_content_hash,
                ));
                output_lines.push(format!("  proposal: {}", finding.refactor_proposal));
            }

            CommandOutcome::success(Some(output_lines.join("\n")))
        }
        DryWriteOutcome::Failure { message } => CommandOutcome::failure(Some(message)),
    }
}

/// Render a `DryCheckApprovedOutcome` (usecase boundary type) into a `CommandOutcome`.
///
/// Reproduces byte-for-byte the text previously produced by cli_composition's
/// `dry_check_approved_outcome` helper (IN-13/AC-18, CN-07/AC-19).
fn render_dry_check_approved_outcome(outcome: DryCheckApprovedOutcome) -> CommandOutcome {
    match outcome {
        DryCheckApprovedOutcome::Approved => CommandOutcome {
            stdout: Some("dry check-approved: APPROVED — all pairs verified".to_owned()),
            stderr: None,
            exit_code: 0,
        },
        DryCheckApprovedOutcome::Blocked { unresolved_pair_count } => CommandOutcome {
            stdout: None,
            stderr: Some(format!(
                "dry check-approved: BLOCKED — {unresolved_pair_count} unresolved pair(s); \
                 run `sotp dry write` to record verdicts"
            )),
            exit_code: 1,
        },
        DryCheckApprovedOutcome::Failure { message } => CommandOutcome::failure(Some(message)),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_dry_write_outcome_reports_checked_and_appended_counts() {
        let outcome = render_dry_write_outcome(DryWriteOutcome::Success {
            pairs_checked: 3,
            records_appended: 2,
            diff_fragments_processed: 1,
            findings: vec![],
        });
        let stdout = outcome.stdout.as_deref().unwrap_or("");

        assert_eq!(outcome.exit_code, 0);
        assert!(stdout.contains("3 pair(s) checked"), "stdout must include pairs checked");
        assert!(stdout.contains("2 record(s) appended"), "stdout must include records appended");
        assert!(stdout.contains("0 violation(s) found"), "stdout must include finding count");
        assert!(
            stdout.contains("1 diff fragment(s) processed"),
            "stdout must include processed diff fragments"
        );
    }

    // ── Approved/Blocked exit-code semantics ─────────────────────────────────

    #[test]
    fn test_approved_verdict_maps_to_exit_0() {
        let outcome = render_dry_check_approved_outcome(DryCheckApprovedOutcome::Approved);
        assert_eq!(outcome.exit_code, 0, "Approved must produce exit code 0");
        assert!(outcome.stdout.is_some(), "Approved must report on stdout");
        assert_eq!(outcome.stderr, None);
    }

    #[test]
    fn test_blocked_verdict_maps_to_exit_1() {
        let outcome = render_dry_check_approved_outcome(DryCheckApprovedOutcome::Blocked {
            unresolved_pair_count: 2,
        });
        assert_eq!(outcome.exit_code, 1, "Blocked must produce exit code 1");
        assert_eq!(outcome.stdout, None);
        assert!(
            outcome.stderr.as_deref().is_some_and(|msg| msg.contains("2 unresolved pair(s)")),
            "Blocked stderr must include unresolved count"
        );
    }
}
