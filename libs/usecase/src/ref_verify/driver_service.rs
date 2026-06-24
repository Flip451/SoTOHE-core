//! High-level driver services for the `ref_verify` command family.
//!
//! These traits are the primary ports consumed by the `RefVerifyDriver` in the
//! `cli_driver` crate.  They accept raw CLI input (track_id, items_dir) and return
//! an opaque result that the driver renders into a `CommandOutcome`.
//!
//! Infrastructure adapters implement these traits by performing scope resolution,
//! branch detection, config loading, and delegating to the appropriate interactor.

use std::path::Path;

// ── Error / outcome types ─────────────────────────────────────────────────────

/// Failure modes for [`RefVerifyRunService`] and [`RefVerifyCheckApprovedDriverService`].
#[derive(Debug)]
pub enum RefVerifyDriverError {
    /// The requested operation could not be prepared or executed.
    Unavailable(String),
    /// Wiring failure (invalid track ID, project root resolution).
    Wiring(String),
    /// Use-case-level failure propagated from the interactor.
    Usecase(String),
}

impl std::fmt::Display for RefVerifyDriverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unavailable(msg) => write!(f, "ref-verify unavailable: {msg}"),
            Self::Wiring(msg) => write!(f, "wiring error: {msg}"),
            Self::Usecase(msg) => write!(f, "use-case error: {msg}"),
        }
    }
}

impl std::error::Error for RefVerifyDriverError {}

/// Outcome of the `ref-verify run` operation, ready for driver rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefVerifyRunOutcome {
    /// All pairs verified successfully.
    Passed,
    /// Production pairs confirmed as failed.
    SemanticFailuresConfirmed {
        /// Number of production pairs with a confirmed Fail verdict.
        pair_count: usize,
    },
    /// Human review required.
    HumanEscalationRequired {
        /// Number of unresolved pairs.
        pair_count: usize,
    },
}

/// Outcome of the `ref-verify check-approved` operation, ready for driver rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefVerifyCheckApprovedOutcome {
    /// No production reference pairs found — gate passes vacuously.
    NoPairs,
    /// All production reference pairs have verified Pass cache entries.
    AllApproved,
    /// One or more production reference pairs lack a Pass cache entry.
    NotApproved {
        /// Human-readable descriptions of each missing/non-pass pair.
        missing_or_non_pass: Vec<String>,
    },
}

// ── Primary ports ─────────────────────────────────────────────────────────────

/// Primary port for the `ref-verify run` subcommand.
///
/// Takes raw CLI input and performs scope resolution, branch detection,
/// config loading, and semantic verification through injected secondary ports.
pub trait RefVerifyRunService: Send + Sync {
    /// Execute the semantic reference verification pipeline.
    ///
    /// # Errors
    ///
    /// Returns [`RefVerifyDriverError`] when the request cannot be prepared or executed.
    /// Use-case-level outcomes (SemanticFailuresConfirmed, HumanEscalationRequired)
    /// are returned as `Ok(RefVerifyRunOutcome::*)`.
    fn run(
        &self,
        track_id: &str,
        items_dir: &Path,
    ) -> Result<RefVerifyRunOutcome, RefVerifyDriverError>;
}

/// Primary port for the `ref-verify check-approved` subcommand.
///
/// Takes raw CLI input and verifies all production pairs have Pass cache entries.
pub trait RefVerifyCheckApprovedDriverService: Send + Sync {
    /// Execute the check-approved gate.
    ///
    /// # Errors
    ///
    /// Returns [`RefVerifyDriverError`] when the request cannot be prepared or executed.
    /// Gate outcomes are returned as `Ok(RefVerifyCheckApprovedOutcome::*)`.
    fn check_approved(
        &self,
        track_id: &str,
        items_dir: &Path,
    ) -> Result<RefVerifyCheckApprovedOutcome, RefVerifyDriverError>;
}

// ── Aggregate port ────────────────────────────────────────────────────────────

/// Aggregate primary port for the `ref_verify` command family.
///
/// `RefVerifyDriver` holds exactly one `Arc<dyn RefVerifyAggregateService>` and
/// delegates each `RefVerifyInput` variant to the corresponding method.
/// The concrete implementation (`RefVerifyAggregateServiceImpl` in
/// `cli_composition`) wires both sub-services internally, keeping the driver
/// free of multi-service injection (D3/D4 cli_driver policy).
pub trait RefVerifyAggregateService: Send + Sync {
    /// Execute the semantic reference verification pipeline.
    ///
    /// # Errors
    ///
    /// Returns [`RefVerifyDriverError`] when the request cannot be prepared or executed.
    fn run(
        &self,
        track_id: &str,
        items_dir: &Path,
    ) -> Result<RefVerifyRunOutcome, RefVerifyDriverError>;

    /// Execute the check-approved gate.
    ///
    /// # Errors
    ///
    /// Returns [`RefVerifyDriverError`] when the request cannot be prepared or executed.
    fn check_approved(
        &self,
        track_id: &str,
        items_dir: &Path,
    ) -> Result<RefVerifyCheckApprovedOutcome, RefVerifyDriverError>;
}
