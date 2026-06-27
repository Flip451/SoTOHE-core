//! High-level driver services for the `ref_verify` command family.
//!
//! These traits are the primary ports consumed by the `RefVerifyDriver` in the
//! `cli_driver` crate.  They accept raw CLI input (track_id, items_dir) and return
//! an opaque result that the driver renders into a `CommandOutcome`.
//!
//! Infrastructure adapters implement these traits by performing scope resolution,
//! branch detection, config loading, and delegating to the appropriate interactor.

use std::path::Path;

use domain::ContentHash;
use domain::tddd::LayerId;
use domain::tddd::semantic_verify::{SemanticVerdict, VerifyOriginRef};

use super::RefVerifyCacheScope;

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

// ── Results filter types ──────────────────────────────────────────────────────

/// Selects which SoT Chain to include in the `ref-verify results` output.
///
/// Maps to the `--chain {1|2|all}` CLI argument. Unlike [`super::RefVerifyScope`],
/// this filter separates chain selection from layer selection: `Chain2` without an
/// embedded layer means "all Chain-2 lanes".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefVerifyChainFilter {
    /// Include only Chain-1 (spec↔ADR) pairs.
    Chain1,
    /// Include only Chain-2 (catalogue↔spec) pairs.
    Chain2,
    /// Include both Chain-1 and Chain-2 pairs.
    All,
}

/// Selects which architecture layer to include in Chain-2 results.
///
/// Maps to the `--layer <name>|all` CLI argument. `Specific(LayerId)` narrows
/// Chain-2 output to a single layer; `All` includes all layers. This filter has
/// no effect when [`RefVerifyChainFilter`] is `Chain1`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefVerifyLayerFilter {
    /// Narrow Chain-2 output to this single layer.
    Specific(LayerId),
    /// Include all layers.
    All,
}

/// Controls which verdict classes appear in the record block of `ref-verify results`.
///
/// `FailPending` is the ADR-defined default when CLI `--filter` is omitted; explicit
/// CLI values map to `Pass`/`Fail`/`Pending`/`All` for `--filter {pass|fail|pending|all}`.
/// The lane-summary header block is always shown for all verdicts regardless of this filter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefVerifyVerdictFilter {
    /// Include fail and pending records (default when `--filter` is omitted).
    FailPending,
    /// Include only pass records.
    Pass,
    /// Include only fail records.
    Fail,
    /// Include only pending records.
    Pending,
    /// Include all records regardless of verdict.
    All,
}

// ── Results output DTOs ───────────────────────────────────────────────────────

/// Output DTO carrying the pass/fail/pending counts for a single Chain×Layer lane.
///
/// `label` is a human-readable display string (e.g. `Chain1 (spec↔ADR)`,
/// `Chain2:domain`); free text with no constraint. Transferred from the
/// infrastructure adapter to the cli_driver for header-block formatting. No
/// serde derives.
#[derive(Debug, Clone)]
pub struct RefVerifyLaneSummary {
    /// Human-readable display label for this lane.
    pub label: String,
    /// Number of pairs with a Pass verdict in this lane.
    pub pass_count: usize,
    /// Number of pairs with a Fail verdict in this lane.
    pub fail_count: usize,
    /// Number of pairs with no Pass/Fail cache entry in this lane.
    pub pending_count: usize,
}

/// Output DTO for a single (claim, evidence) pair in the results record block.
///
/// `chain_layer` is the display label for the record's Chain×Layer lane (e.g.
/// `Chain1` or `Chain2:domain`). `reason` is the display reason: fail records
/// use the cached [`SemanticVerdict::Fail`] reason, pending records use the
/// unresolved-pair reason, and pass records may use an empty reason. For
/// pass/fail pairs, origin fields come from the verify-cache entry. For pending
/// pairs, origin fields are re-derived from the current pair source without
/// invoking the LLM verifier. Transferred from the infrastructure adapter to
/// the cli_driver for record-block formatting.
#[derive(Debug, Clone)]
pub struct RefVerifyPairRecord {
    /// Routing scope for this pair (SpecAdr or CatalogueSpec).
    pub chain_scope: RefVerifyCacheScope,
    /// Display label for this record's Chain×Layer lane.
    pub chain_layer: String,
    /// SHA-256 hash of the claim element.
    pub claim_hash: ContentHash,
    /// SHA-256 hash of the evidence element.
    pub evidence_hash: ContentHash,
    /// Semantic verdict for this pair.
    pub verdict: SemanticVerdict,
    /// Human-readable reason string (fail reason, unresolved message, or empty).
    pub reason: String,
    /// Origin reference identifying the artifact and location of the claim.
    pub claim_origin: VerifyOriginRef,
    /// Origin reference identifying the artifact and location of the evidence.
    pub evidence_origin: VerifyOriginRef,
}

/// Aggregate output DTO for the `ref-verify results` service call.
///
/// `lane_summaries` carries chain×layer summary rows (filtered by chain and
/// layer options) for the header block. `pair_records` carries individual pair
/// records (fully filtered by chain, layer, and verdict) for the record block.
/// `total_pass`/`total_fail`/`total_pending` summarize the same chain/layer-
/// filtered lane set as `lane_summaries`; they are unaffected by the verdict
/// filter.
#[derive(Debug)]
pub struct RefVerifyResultsOutput {
    /// Per-lane header summary rows (chain/layer filtered, all verdicts).
    pub lane_summaries: Vec<RefVerifyLaneSummary>,
    /// Individual pair records (chain/layer/verdict filtered).
    pub pair_records: Vec<RefVerifyPairRecord>,
    /// Total pass count across the chain/layer-filtered lane set.
    pub total_pass: usize,
    /// Total fail count across the chain/layer-filtered lane set.
    pub total_fail: usize,
    /// Total pending count across the chain/layer-filtered lane set.
    pub total_pending: usize,
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
/// The concrete implementation (`FsRefVerifyAggregateAdapter` in
/// `infrastructure`) wires all sub-services internally, keeping the driver
/// free of multi-service injection (D3/D4 cli_driver policy).
///
/// Extended by D1/D2/D3 to add the `results` method. The `results` method has
/// a temporary default `Unavailable` implementation in T003 so the trait
/// extension is compile-safe before `FsRefVerifyAggregateAdapter` overrides it
/// in T004. `RefVerifyDriver` holds one `Arc<dyn RefVerifyAggregateService>`
/// and dispatches each `RefVerifyInput` variant to the corresponding method.
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

    /// Read the verify-cache and re-derive pending pairs, then return structured
    /// results data filtered by the given chain, layer, and verdict filters.
    ///
    /// This method has a compile-safe default body returning
    /// [`RefVerifyDriverError::Unavailable`] so existing implementors continue
    /// to compile until T004 overrides it with the concrete
    /// `FsRefVerifyAggregateAdapter` implementation. No LLM verifier subprocess
    /// is started by this method.
    ///
    /// # Errors
    ///
    /// Returns [`RefVerifyDriverError::Unavailable`] by default (until T004).
    /// The concrete implementation may return [`RefVerifyDriverError::Wiring`]
    /// or [`RefVerifyDriverError::Usecase`] on infrastructure failure.
    fn results(
        &self,
        track_id: &str,
        items_dir: &Path,
        chain: RefVerifyChainFilter,
        layer: RefVerifyLayerFilter,
        verdict: RefVerifyVerdictFilter,
    ) -> Result<RefVerifyResultsOutput, RefVerifyDriverError> {
        let _ = (track_id, items_dir, chain, layer, verdict);
        Err(RefVerifyDriverError::Unavailable("ref-verify results not implemented".to_owned()))
    }
}
