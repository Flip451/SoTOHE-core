// STAGED FOR T021 — not yet compiled; Cargo.toml + workspace member added atomically in T021 per CN-06.
//
//! `ref_verify` command family — primary adapter driver.
//!
//! `RefVerifyDriver` holds injected use-case interactors and exposes
//! `handle(input) -> CommandOutcome`.  The render helpers here mirror
//! `apps/cli-composition/src/ref_verify.rs` (lines 135-158 and 264-269);
//! T021 removes the `cli_composition` duplicate when the live path is flipped.

// TODO(T021): add use-case + infrastructure imports once Cargo.toml is materialized.
// use std::path::PathBuf;
// use std::sync::Arc;
// use domain::{ContentHash, TrackId};
// use domain::tddd::semantic_verify::SemanticVerdict;
// use usecase::ref_verify::{
//     RefVerifyApplicationService as _, RefVerifyCachePort as _, RefVerifyCacheScope,
//     RefVerifyCommand, RefVerifyConfig, RefVerifyError, RefVerifyPairSourcePort as _,
//     RefVerifyScope, VerifySemanticRefsInteractor,
// };
// use infrastructure::ref_verify::{
//     AgentRefVerifierAdapter, RefVerifyCacheAdapter, RefVerifyPairSourceAdapter,
//     RefVerifyScopeResolver, make_ref_verifier_process_runner,
// };
// use infrastructure::agent_profiles::{AGENT_PROFILES_PATH, AgentProfiles};

use std::path::PathBuf;

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
/// Holds injected use-case interactors; exposes `handle(input) -> CommandOutcome`.
pub struct RefVerifyDriver {
    // TODO(T021): inject use-case interactors here.
    // pair_source: Arc<dyn usecase::ref_verify::RefVerifyPairSourcePort>,
    // cache: Arc<dyn usecase::ref_verify::RefVerifyCachePort>,
    // verifier: Arc<dyn usecase::ref_verify::AgentRefVerifierPort>,
}

impl RefVerifyDriver {
    /// Create a new `RefVerifyDriver`.
    ///
    /// TODO(T021): accept injected interactors as parameters once the crate
    /// dependency graph is materialized.
    pub fn new() -> Self {
        Self {}
    }

    /// Handle a ref_verify command.
    ///
    /// TODO(T021): wire real use-case invocation once Cargo.toml is materialized.
    pub fn handle(&self, input: RefVerifyInput) -> CommandOutcome {
        match input {
            RefVerifyInput::Run(input) => self.ref_verify_run(input),
            RefVerifyInput::CheckApproved(input) => self.ref_verify_check_approved(input),
        }
    }

    // -----------------------------------------------------------------------
    // Render helpers (logic duplicated from cli_composition/src/ref_verify.rs
    // lines 135-158 and 264-269; T021 removes the cli_composition copy).
    // -----------------------------------------------------------------------

    fn ref_verify_run(&self, _input: RefVerifyRunInput) -> CommandOutcome {
        // TODO(T021): invoke VerifySemanticRefsInteractor::execute here.
        // On Ok(()): CommandOutcome::success(Some("[OK] Semantic reference verification passed…"))
        // On Err(SemanticFailuresConfirmed): exit_code 1 + [BLOCKED] message.
        // On Err(HumanEscalationRequired): exit_code 1 + [ESCALATE] message.
        // Mirrors cli_composition/src/ref_verify.rs RefVerifyCompositionRoot::ref_verify_run
        // (lines 113-190).
        CommandOutcome::success(None)
    }

    fn ref_verify_check_approved(&self, _input: RefVerifyCheckApprovedInput) -> CommandOutcome {
        // TODO(T021): invoke RefVerifyCacheAdapter + RefVerifyPairSourceAdapter here.
        // On zero production pairs: CommandOutcome::success("[OK] No production reference…")
        // On all Pass cache entries: CommandOutcome::success("[OK] All production…")
        // On missing/non-pass entries: exit_code 1 + [BLOCKED] message.
        // Mirrors cli_composition/src/ref_verify.rs RefVerifyCompositionRoot::ref_verify_check_approved
        // (lines 193-309), including bracketed status formatting.
        CommandOutcome::success(None)
    }
}

impl Default for RefVerifyDriver {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Bracketed status formatting helpers (duplicated from cli_composition/src/ref_verify.rs
// lines 264-269; T021 removes the cli_composition copy).
// ---------------------------------------------------------------------------

/// Format a missing-or-non-pass pair entry as a bracketed status string.
///
/// Mirrors the inline formatting in
/// `cli_composition::ref_verify::RefVerifyCompositionRoot::ref_verify_check_approved`
/// (lines 264-269).
fn format_pair_status(claim_hex: &str, evidence_hex: &str, reason: &str) -> String {
    // TODO(T021): used when iterating over production_pairs to build
    // the missing_or_non_pass vec. Format: "pair ({claim_hex}, {evidence_hex}) {reason}"
    format!("pair ({claim_hex}, {evidence_hex}) {reason}")
}
