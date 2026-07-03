//! `dry` command family — primary adapter driver.
//!
//! `DryDriver` holds four injected services and exposes
//! `handle(input) -> CommandOutcome`: the legacy
//! [`usecase::dry_driver::DryDriverService`] now backs only `dry fix-local`
//! (OS-08), while `dry write` / `dry results` / `dry check-approved` are
//! backed by the IN-14 driver services
//! ([`usecase::dry_write_driver::DryWriteDriverService`],
//! [`usecase::dry_results_driver::DryResultsDriverService`],
//! [`usecase::dry_check_approved_driver::DryCheckApprovedDriverService`]).
//!
//! `dry check-approved`'s wall-clock timing measurement and `GateEval`
//! telemetry emission live in the bin layer
//! (`apps/cli/src/commands/dry.rs::execute_dry_check_approved`), mirroring
//! `execute_verify_with_telemetry`. This driver stays a pure invoke+render
//! controller: it calls the service and renders the outcome, with no
//! side-effecting I/O of its own.

use std::path::PathBuf;
use std::sync::Arc;

use usecase::dry_check_approved_driver::DryCheckApprovedDriverService;
use usecase::dry_driver::{
    DryCheckApprovedDriverInput, DryCheckApprovedOutcome, DryDriverOutcome, DryDriverService,
    DryFixLocalDriverInput, DryResultsDriverInput, DryWriteDriverInput, DryWriteOutcome,
};
use usecase::dry_results_driver::{
    DryResultsDriverService, DryResultsOutcome, DryResultsVerdictSummary,
};
use usecase::dry_write_driver::DryWriteDriverService;

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
/// Holds four injected services; exposes `handle(input) -> CommandOutcome`.
pub struct DryDriver {
    /// Backs only `dry fix-local` (OS-08).
    service: Arc<dyn DryDriverService>,
    write_service: Arc<dyn DryWriteDriverService>,
    results_service: Arc<dyn DryResultsDriverService>,
    check_approved_service: Arc<dyn DryCheckApprovedDriverService>,
}

impl DryDriver {
    /// Create a new `DryDriver` with the four given services.
    pub fn new(
        service: Arc<dyn DryDriverService>,
        write_service: Arc<dyn DryWriteDriverService>,
        results_service: Arc<dyn DryResultsDriverService>,
        check_approved_service: Arc<dyn DryCheckApprovedDriverService>,
    ) -> Self {
        Self { service, write_service, results_service, check_approved_service }
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
        let outcome = self.write_service.dry_write(DryWriteDriverInput {
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
            self.results_service.dry_results(DryResultsDriverInput { track_id, filter, items_dir });
        render_dry_results_outcome(outcome)
    }

    fn dry_check_approved(
        &self,
        track_id: String,
        base_commit: Option<String>,
        items_dir: PathBuf,
    ) -> CommandOutcome {
        let outcome = self.check_approved_service.dry_check_approved(DryCheckApprovedDriverInput {
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

/// Render a `DryResultsOutcome` (usecase boundary type) into a `CommandOutcome`.
///
/// Reproduces byte-for-byte the text previously produced by cli_composition's
/// `DryCompositionRoot::dry_results`'s line-by-line construction (CN-07/AC-19).
fn render_dry_results_outcome(outcome: DryResultsOutcome) -> CommandOutcome {
    match outcome {
        DryResultsOutcome::Success { records } => {
            let mut lines: Vec<String> = Vec::new();
            lines.push(format!("dry results: {} record(s)", records.len()));
            for record in &records {
                lines.push(format!(
                    "  pair: [{} ({})] <-> [{} ({})]",
                    record.low_path,
                    record.low_content_hash,
                    record.high_path,
                    record.high_content_hash,
                ));
                lines.push(format!("  changed_path (display-only): {}", record.changed_path));
                let verdict_str = match &record.verdict {
                    DryResultsVerdictSummary::NotAViolation => "not-a-violation".to_owned(),
                    DryResultsVerdictSummary::Accepted => "accepted".to_owned(),
                    DryResultsVerdictSummary::Violation { refactor_proposal } => {
                        format!("violation | proposal: {refactor_proposal}")
                    }
                };
                lines.push(format!("  verdict: {verdict_str}"));
                lines.push(format!(
                    "  score: {} | threshold: {} | base: {}",
                    record.similarity_score, record.threshold, record.base_commit,
                ));
                lines.push(format!("  rationale: {}", record.rationale));
                lines.push(format!("  recorded_at: {}", record.recorded_at));
            }

            CommandOutcome::success(Some(lines.join("\n")))
        }
        DryResultsOutcome::Failure { message } => CommandOutcome::failure(Some(message)),
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
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use usecase::dry_results_driver::DryResultsRecordSummary;

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

    // ── dry results render shape (relocated from apps/cli-composition/src/dry.rs) ──

    fn make_record(verdict: DryResultsVerdictSummary) -> DryResultsRecordSummary {
        DryResultsRecordSummary {
            low_path: "src/a.rs".to_owned(),
            low_content_hash: "a".repeat(64),
            high_path: "src/b.rs".to_owned(),
            high_content_hash: "b".repeat(64),
            changed_path: "src/a.rs".to_owned(),
            verdict,
            similarity_score: 0.9,
            threshold: 0.85,
            base_commit: "c".repeat(40),
            rationale: "test rationale".to_owned(),
            recorded_at: "2026-06-30T00:00:00Z".to_owned(),
        }
    }

    #[test]
    fn test_dry_results_outcome_empty_records_reports_zero_count() {
        let outcome = render_dry_results_outcome(DryResultsOutcome::Success { records: vec![] });
        assert_eq!(outcome.exit_code, 0);
        assert_eq!(outcome.stdout.as_deref(), Some("dry results: 0 record(s)"));
    }

    #[test]
    fn test_dry_results_outcome_renders_pair_and_verdict_lines() {
        let outcome = render_dry_results_outcome(DryResultsOutcome::Success {
            records: vec![make_record(DryResultsVerdictSummary::NotAViolation)],
        });
        let stdout = outcome.stdout.unwrap();
        let lines: Vec<&str> = stdout.split('\n').collect();

        assert_eq!(lines[0], "dry results: 1 record(s)");
        assert_eq!(
            lines[1],
            format!("  pair: [src/a.rs ({})] <-> [src/b.rs ({})]", "a".repeat(64), "b".repeat(64))
        );
        assert_eq!(lines[2], "  changed_path (display-only): src/a.rs");
        assert_eq!(lines[3], "  verdict: not-a-violation");
        assert_eq!(lines[4], format!("  score: 0.9 | threshold: 0.85 | base: {}", "c".repeat(40)));
        assert_eq!(lines[5], "  rationale: test rationale");
        assert_eq!(lines[6], "  recorded_at: 2026-06-30T00:00:00Z");
    }

    #[test]
    fn test_dry_results_outcome_accepted_verdict_renders_accepted() {
        let outcome = render_dry_results_outcome(DryResultsOutcome::Success {
            records: vec![make_record(DryResultsVerdictSummary::Accepted)],
        });
        let stdout = outcome.stdout.unwrap();
        assert!(stdout.contains("  verdict: accepted"));
    }

    #[test]
    fn test_dry_results_outcome_violation_verdict_renders_proposal() {
        let outcome = render_dry_results_outcome(DryResultsOutcome::Success {
            records: vec![make_record(DryResultsVerdictSummary::Violation {
                refactor_proposal: "Extract shared helper.".to_owned(),
            })],
        });
        let stdout = outcome.stdout.unwrap();
        assert!(stdout.contains("  verdict: violation | proposal: Extract shared helper."));
    }

    #[test]
    fn test_dry_results_outcome_failure_maps_to_failure_command_outcome() {
        let outcome =
            render_dry_results_outcome(DryResultsOutcome::Failure { message: "boom".to_owned() });
        assert_eq!(outcome.exit_code, 1);
        assert_eq!(outcome.stderr.as_deref(), Some("boom"));
        assert_eq!(outcome.stdout, None);
    }

    #[test]
    fn test_dry_results_outcome_exit_0_regardless_of_record_count() {
        // INFORMATIONAL: dry results always exits 0 on a successful read,
        // whether or not any records matched the filter.
        for records in [vec![], vec![make_record(DryResultsVerdictSummary::Accepted)]] {
            let outcome = render_dry_results_outcome(DryResultsOutcome::Success { records });
            assert_eq!(outcome.exit_code, 0);
        }
    }
}
