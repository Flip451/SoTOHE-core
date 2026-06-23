//! `ref_verify` command family — primary adapter driver.
//!
//! `RefVerifyDriver` holds a single injected `RefVerifyAggregateService` and
//! exposes `handle(input) -> CommandOutcome`. One injected interactor — no
//! per-service fields (D3/D4 cli_driver policy).

use std::path::PathBuf;
use std::sync::Arc;

use usecase::ref_verify::{
    RefVerifyAggregateService, RefVerifyCheckApprovedOutcome, RefVerifyDriverError,
    RefVerifyRunOutcome,
};

use crate::render::CommandOutcome;

// ---------------------------------------------------------------------------
// Input types
// ---------------------------------------------------------------------------

/// Input DTO for `sotp ref-verify run`.
#[derive(Debug, Clone)]
pub struct RefVerifyRunInput {
    /// Track ID whose semantic references should be verified.
    pub track_id: String,
    /// Path to the track items directory (e.g. `track/items`).
    pub items_dir: PathBuf,
}

/// Input DTO for `sotp ref-verify check-approved`.
#[derive(Debug, Clone)]
pub struct RefVerifyCheckApprovedInput {
    /// Track ID whose semantic references should be checked.
    pub track_id: String,
    /// Path to the track items directory (e.g. `track/items`).
    pub items_dir: PathBuf,
}

/// Typed input for the `ref_verify` command family.
pub enum RefVerifyInput {
    /// Run semantic reference verification.
    Run(RefVerifyRunInput),
    /// Check whether all production reference pairs have verified Pass cache entries.
    CheckApproved(RefVerifyCheckApprovedInput),
}

// ---------------------------------------------------------------------------
// Driver
// ---------------------------------------------------------------------------

/// Primary adapter driver for the `ref_verify` command family.
///
/// Holds a single injected `RefVerifyAggregateService`; exposes
/// `handle(input) -> CommandOutcome`. One injected interactor — no per-service
/// fields (D3/D4 cli_driver policy).
pub struct RefVerifyDriver {
    service: Arc<dyn RefVerifyAggregateService>,
}

impl RefVerifyDriver {
    /// Create a new `RefVerifyDriver` with a single injected aggregate service.
    pub fn new(service: Arc<dyn RefVerifyAggregateService>) -> Self {
        Self { service }
    }

    /// Handle a ref_verify command.
    pub fn handle(&self, input: RefVerifyInput) -> CommandOutcome {
        match input {
            RefVerifyInput::Run(input) => self.ref_verify_run(input),
            RefVerifyInput::CheckApproved(input) => self.ref_verify_check_approved(input),
        }
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    fn ref_verify_run(&self, input: RefVerifyRunInput) -> CommandOutcome {
        match self.service.run(&input.track_id, &input.items_dir) {
            Ok(RefVerifyRunOutcome::Passed) => CommandOutcome::success(Some(
                "[OK] Semantic reference verification passed — all pairs verified.".to_owned(),
            )),
            Ok(RefVerifyRunOutcome::SemanticFailuresConfirmed { pair_count }) => CommandOutcome {
                stdout: None,
                stderr: Some(format!(
                    "[BLOCKED] Semantic review confirmed {pair_count} production failure(s). \
                     Resolve the failures before committing."
                )),
                exit_code: 1,
            },
            Ok(RefVerifyRunOutcome::HumanEscalationRequired { pair_count }) => CommandOutcome {
                stdout: None,
                stderr: Some(format!(
                    "[ESCALATE] Human review required for {pair_count} unresolved pair(s) \
                     or known-bad detection failure."
                )),
                exit_code: 1,
            },
            Err(RefVerifyDriverError::Wiring(msg)) => {
                CommandOutcome::failure(Some(format!("ref-verify run failed (wiring): {msg}")))
            }
            Err(e) => CommandOutcome::failure(Some(format!("ref-verify run failed: {e}"))),
        }
    }

    fn ref_verify_check_approved(&self, input: RefVerifyCheckApprovedInput) -> CommandOutcome {
        match self.service.check_approved(&input.track_id, &input.items_dir) {
            Ok(RefVerifyCheckApprovedOutcome::NoPairs) => CommandOutcome::success(Some(
                "[OK] No production reference pairs found — check-approved gate passes.".to_owned(),
            )),
            Ok(RefVerifyCheckApprovedOutcome::AllApproved) => CommandOutcome::success(Some(
                "[OK] All production reference pairs have verified Pass cache entries.".to_owned(),
            )),
            Ok(RefVerifyCheckApprovedOutcome::NotApproved { missing_or_non_pass }) => {
                CommandOutcome {
                    stdout: None,
                    stderr: Some(format!(
                        "[BLOCKED] ref-verify check-approved failed: {} pair(s) without Pass cache:\n{}",
                        missing_or_non_pass.len(),
                        missing_or_non_pass.join("\n")
                    )),
                    exit_code: 1,
                }
            }
            Err(RefVerifyDriverError::Wiring(msg)) => CommandOutcome::failure(Some(format!(
                "ref-verify check-approved failed (wiring): {msg}"
            ))),
            Err(e) => {
                CommandOutcome::failure(Some(format!("ref-verify check-approved failed: {e}")))
            }
        }
    }
}

/// Format a missing-or-non-pass pair entry as a bracketed status string.
///
/// Used when iterating over production_pairs to build the missing_or_non_pass vec.
/// Format: `"pair ({claim_hex}, {evidence_hex}) {reason}"`
pub fn format_pair_status(claim_hex: &str, evidence_hex: &str, reason: &str) -> String {
    format!("pair ({claim_hex}, {evidence_hex}) {reason}")
}
