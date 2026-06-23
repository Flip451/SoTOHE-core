//! Semantic reference verification application service (SoT Chain integrity gate).
//!
//! This module implements the usecase layer for the `bin/sotp ref-verify`
//! command (D9 / IN-06 / IN-08 / IN-10 / IN-11 / IN-12 / AC-03 / AC-07 /
//! AC-08 / AC-09 / AC-10 of ADR `2026-05-27-1601-sot-chain-semantic-review-gate.md`).
//!
//! ## Design constraints
//!
//! - **CN-01**: Does not share any types with the code-review infrastructure
//!   (`Verdict`, `ReviewTarget`, `review.json`). This module owns all
//!   `RefVerify*` types independently.
//! - **CN-05**: Parallelism limit is enforced through the validated
//!   [`RefVerifyParallelism`] value object.
//! - **CN-06**: Invalid configuration values (zero percent, zero parallelism)
//!   are rejected at construction time.
//! - **No filesystem I/O**: the interactor does not read configuration files or
//!   access the filesystem directly — all I/O is delegated to injected
//!   secondary ports.

use std::fmt;

use domain::TrackId;
use domain::tddd::LayerId;
use domain::tddd::semantic_verify::{ModelTier, SemanticVerdict, SemanticVerifyEntry};

// ── RefVerifyScope ────────────────────────────────────────────────────────────

/// Selects which SoT Chain links to verify.
///
/// - `Chain1`: spec → ADR (Chain-1 only).
/// - `Chain2 { layer }`: catalogue → spec for one layer (Chain-2 only).
/// - `All`: both chains, all layers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefVerifyScope {
    /// Chain-1: spec.json → ADR.
    Chain1,
    /// Chain-2: catalogue → spec for the given layer.
    Chain2 {
        /// Target layer identifier.
        layer: LayerId,
    },
    /// Both Chain-1 and Chain-2 (all layers).
    All,
}

// ── RefVerifyCacheScope ───────────────────────────────────────────────────────

/// Routes verified pairs to the concrete cache artifact required by D8.
///
/// `SpecAdr` maps to `spec-adr-verify-cache.json`; `CatalogueSpec { layer }`
/// maps to `<layer>-catalogue-spec-verify-cache.json`.
///
/// [`RefVerifyScope::All`] is decomposed into these cache scopes before
/// cache load/save so infrastructure can route each pair to the correct
/// artifact.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RefVerifyCacheScope {
    /// Chain-1 cache: `spec-adr-verify-cache.json`.
    SpecAdr,
    /// Chain-2 cache: `<layer>-catalogue-spec-verify-cache.json`.
    CatalogueSpec {
        /// Architecture layer this cache entry belongs to.
        layer: LayerId,
    },
}

// ── RefVerifyCommand ──────────────────────────────────────────────────────────

/// Command input for [`RefVerifyApplicationService::execute`].
///
/// Identifies which track and which Chain scope (Chain1, Chain2, or All) to
/// verify. The `current_branch` field is the caller's current git branch string
/// (e.g. `"track/my-feature-2026-06-01"`); the interactor checks it against
/// the track's expected branch `"track/<track_id>"` and returns
/// [`RefVerifyError::TrackNotActive`] when they do not match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefVerifyCommand {
    /// Track identifier.
    pub track_id: TrackId,
    /// Which Chain scope to verify.
    pub scope: RefVerifyScope,
    /// Current git branch provided by the caller.
    ///
    /// Must equal `"track/<track_id>"` or the interactor returns
    /// [`RefVerifyError::TrackNotActive`].
    pub current_branch: String,
}

// ── RefVerifyPair ─────────────────────────────────────────────────────────────

/// Usecase-owned representation of one semantic reference pair.
///
/// Carries the claim/evidence text for the verifier, externally determined
/// hashes for cache lookup, the cache artifact scope used to route results,
/// and a `known_bad` flag for degradation probing.
///
/// `known_bad` pairs are monitor probes: they are evaluated every run and are
/// **not** stored in or loaded from the verify-cache (AC-09 / CN-06).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefVerifyPair {
    /// The semantic claim to be verified (e.g. a spec element text).
    pub claim: String,
    /// The evidence backing the claim (e.g. an ADR decision text).
    pub evidence: String,
    /// SHA-256 hash of the claim element, computed by infrastructure.
    pub claim_hash: domain::ContentHash,
    /// SHA-256 hash of the evidence element, computed by infrastructure.
    pub evidence_hash: domain::ContentHash,
    /// Routing scope: determines which cache artifact this pair is stored in.
    pub cache_scope: RefVerifyCacheScope,
    /// When `true`, this pair is a known-bad monitor probe. It is evaluated
    /// every run (bypassing cache) but its verdict is not persisted.
    pub known_bad: bool,
}

// ── RefVerifyPercent ──────────────────────────────────────────────────────────

/// Validated nonzero percentage value object for ref-verify health-check
/// configuration.
///
/// `try_new` accepts only values in `1..=100` so known-bad injection and
/// detection thresholds cannot be disabled by zero-valued settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefVerifyPercent(u8);

impl RefVerifyPercent {
    /// Construct a [`RefVerifyPercent`] from a raw `u8` value.
    ///
    /// # Errors
    ///
    /// Returns [`RefVerifyError::InvalidConfig`] when `value` is 0 or > 100.
    pub fn try_new(value: u8) -> Result<Self, RefVerifyError> {
        if value == 0 || value > 100 {
            return Err(RefVerifyError::InvalidConfig {
                message: format!("percent value must be in 1..=100, got {value}"),
            });
        }
        Ok(Self(value))
    }

    /// Return the inner percentage value.
    pub fn as_u8(self) -> u8 {
        self.0
    }
}

// ── RefVerifyParallelism ──────────────────────────────────────────────────────

/// Validated nonzero parallelism limit for ref-verify batching.
///
/// `try_new` rejects 0 so the interactor cannot be configured with a
/// nonsensical worker limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefVerifyParallelism(usize);

impl RefVerifyParallelism {
    /// Construct a [`RefVerifyParallelism`] from a raw `usize` value.
    ///
    /// # Errors
    ///
    /// Returns [`RefVerifyError::InvalidConfig`] when `value` is 0.
    pub fn try_new(value: usize) -> Result<Self, RefVerifyError> {
        if value == 0 {
            return Err(RefVerifyError::InvalidConfig {
                message: "max_parallelism must be nonzero".to_owned(),
            });
        }
        Ok(Self(value))
    }

    /// Return the inner parallelism value.
    pub fn as_usize(self) -> usize {
        self.0
    }
}

// ── RefVerifyConfig ───────────────────────────────────────────────────────────

/// Usecase configuration injected by the composition layer.
///
/// Keeps known-bad injection rate, detection threshold, and parallelism
/// limits out of filesystem/config I/O inside the interactor while making
/// invalid settings unrepresentable through [`RefVerifyPercent`] and
/// [`RefVerifyParallelism`].
///
/// `try_new` validates raw DTO values at the usecase boundary. The
/// `Default` implementation supplies 10% known-bad injection, 90% detection
/// threshold, and a nonzero `max_parallelism` default.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefVerifyConfig {
    /// Rate at which known-bad monitor probes are injected into each batch.
    pub known_bad_injection_rate_percent: RefVerifyPercent,
    /// Minimum fraction of known-bad probes that must be detected as Fail
    /// for the calibration to be considered healthy.
    pub known_bad_detection_threshold_percent: RefVerifyPercent,
    /// Maximum number of pairs verified in parallel.
    pub max_parallelism: RefVerifyParallelism,
}

impl RefVerifyConfig {
    /// Validate and construct a [`RefVerifyConfig`] from raw DTO values.
    ///
    /// # Errors
    ///
    /// Returns [`RefVerifyError::InvalidConfig`] when any raw value is
    /// invalid (0 percent, > 100 percent, zero parallelism).
    pub fn try_new(
        known_bad_injection_rate_percent: u8,
        known_bad_detection_threshold_percent: u8,
        max_parallelism: usize,
    ) -> Result<Self, RefVerifyError> {
        let injection = RefVerifyPercent::try_new(known_bad_injection_rate_percent)?;
        let threshold = RefVerifyPercent::try_new(known_bad_detection_threshold_percent)?;
        let parallelism = RefVerifyParallelism::try_new(max_parallelism)?;
        Ok(Self {
            known_bad_injection_rate_percent: injection,
            known_bad_detection_threshold_percent: threshold,
            max_parallelism: parallelism,
        })
    }
}

impl Default for RefVerifyConfig {
    fn default() -> Self {
        Self {
            // Safety: 10 and 90 are valid percent values; 4 is nonzero.
            known_bad_injection_rate_percent: RefVerifyPercent(10),
            known_bad_detection_threshold_percent: RefVerifyPercent(90),
            max_parallelism: RefVerifyParallelism(4),
        }
    }
}

// ── RefVerifyError ────────────────────────────────────────────────────────────

/// Failure modes of [`RefVerifyApplicationService::execute`].
///
/// - `InvalidConfig`: returned by `RefVerifyPercent` / `RefVerifyParallelism`
///   / `RefVerifyConfig` constructors for impossible settings.
/// - `TrackNotActive`: guards the active-track contract.
/// - `VerifierPort`: wraps adapter failures from [`RefVerifierPort`].
/// - `CachePersistence`: wraps artifact write failures from
///   [`RefVerifyCachePort`].
/// - `SemanticFailuresConfirmed`: reports final-tier production Fail entries
///   back to the writer/fix loop (D5 / AC-04 / OS-04).
/// - `HumanEscalationRequired`: reserved for final-tier Pending / unresolved
///   cases or confirmed known-bad verifier degradation.
#[derive(Debug)]
pub enum RefVerifyError {
    /// One or more configuration values are outside their valid range.
    InvalidConfig {
        /// Human-readable description of the invalid value.
        message: String,
    },
    /// The current git branch is not an active track branch.
    TrackNotActive {
        /// The branch that was rejected.
        branch: String,
    },
    /// The verifier adapter returned an error.
    VerifierPort {
        /// Human-readable description of the adapter failure.
        message: String,
    },
    /// The cache persistence adapter returned an error.
    CachePersistence {
        /// Human-readable description of the write failure.
        message: String,
    },
    /// Final-tier evaluation confirmed production Fail verdicts; the
    /// writer/fix loop must resolve the failures before re-committing.
    SemanticFailuresConfirmed {
        /// Number of production pairs with a confirmed Fail verdict.
        pair_count: usize,
    },
    /// Human review is required because final-tier evaluation left
    /// Pending verdicts or the known-bad detection rate fell below the
    /// configured threshold after final-tier re-evaluation.
    HumanEscalationRequired {
        /// Number of unresolved pairs (Pending or known-bad degradation).
        pair_count: usize,
    },
}

impl fmt::Display for RefVerifyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RefVerifyError::InvalidConfig { message } => {
                write!(f, "invalid ref-verify config: {message}")
            }
            RefVerifyError::TrackNotActive { branch } => {
                write!(f, "branch '{branch}' is not an active track branch")
            }
            RefVerifyError::VerifierPort { message } => {
                write!(f, "semantic verifier adapter error: {message}")
            }
            RefVerifyError::CachePersistence { message } => {
                write!(f, "verify-cache persistence error: {message}")
            }
            RefVerifyError::SemanticFailuresConfirmed { pair_count } => {
                write!(
                    f,
                    "semantic review confirmed {pair_count} production failure(s); \
                     resolve before committing"
                )
            }
            RefVerifyError::HumanEscalationRequired { pair_count } => {
                write!(
                    f,
                    "human review required for {pair_count} unresolved pair(s) \
                     or known-bad detection failure"
                )
            }
        }
    }
}

impl std::error::Error for RefVerifyError {}

// ── Secondary ports ───────────────────────────────────────────────────────────

/// Secondary port for infrastructure-backed enumeration of Chain1/Chain2
/// reference pairs.
///
/// Infrastructure implements this port by reading spec, catalogue, and ADR
/// artifacts, computing claim/evidence hashes, assigning the
/// [`RefVerifyCacheScope`] for each pair, and injecting configured known-bad
/// monitor probes. This keeps filesystem I/O outside the usecase layer.
pub trait RefVerifyPairSourcePort: Send + Sync {
    /// Load the full set of reference pairs for the given command and config.
    ///
    /// # Errors
    ///
    /// Returns [`RefVerifyError`] on read failure.
    fn load_pairs(
        &self,
        cmd: &RefVerifyCommand,
        config: &RefVerifyConfig,
    ) -> Result<Vec<RefVerifyPair>, RefVerifyError>;
}

/// Secondary port for loading and saving semantic verify-cache entries for one
/// concrete cache artifact.
///
/// The interactor decomposes [`RefVerifyScope::All`] into
/// [`RefVerifyCacheScope`] groups before calling this port, so infrastructure
/// can route `SpecAdr` to `spec-adr-verify-cache.json` and `CatalogueSpec {
/// layer }` to `<layer>-catalogue-spec-verify-cache.json`.
///
/// Known-bad monitor probes are **not** loaded from or saved to these caches.
pub trait RefVerifyCachePort: Send + Sync {
    /// Load existing verify-cache entries for the given scope.
    ///
    /// # Errors
    ///
    /// Returns [`RefVerifyError`] on read failure.
    fn load_entries(
        &self,
        cmd: &RefVerifyCommand,
        cache_scope: &RefVerifyCacheScope,
    ) -> Result<Vec<SemanticVerifyEntry>, RefVerifyError>;

    /// Persist updated verify-cache entries for the given scope.
    ///
    /// # Errors
    ///
    /// Returns [`RefVerifyError`] on write failure.
    fn save_entries(
        &self,
        cmd: &RefVerifyCommand,
        cache_scope: &RefVerifyCacheScope,
        entries: Vec<SemanticVerifyEntry>,
    ) -> Result<(), RefVerifyError>;
}

/// Secondary port for semantic review of a single `(claim, evidence)` pair.
///
/// The adapter returns only a [`SemanticVerdict`]; the usecase layer owns
/// externally determined `claim_hash` / `evidence_hash` values and wraps the
/// verdict into a [`SemanticVerifyEntry`] for cache persistence.
///
/// [`ModelTier`] selects fast vs. final model per D5 three-tier escalation.
/// `cache_scope` selects which Chain's prompt template / capability the
/// adapter uses (D11): [`RefVerifyCacheScope::SpecAdr`] routes to
/// `ref-verifier-chain1`, [`RefVerifyCacheScope::CatalogueSpec`] routes to
/// `ref-verifier-chain2`.
///
/// Implemented by the chain-routing capability adapter in infrastructure
/// (D7 / D11 / CN-01).
pub trait RefVerifierPort: Send + Sync {
    /// Semantically review a `(claim, evidence)` pair at the given model tier
    /// using the prompt template / capability appropriate for `cache_scope`.
    ///
    /// # Errors
    ///
    /// Returns [`RefVerifyError::VerifierPort`] on adapter failure.
    fn verify_pair(
        &self,
        claim: String,
        evidence: String,
        cache_scope: &RefVerifyCacheScope,
        tier: ModelTier,
    ) -> Result<SemanticVerdict, RefVerifyError>;
}

// ── Primary port ──────────────────────────────────────────────────────────────

/// Primary port for the `bin/sotp ref-verify` use case (D9).
///
/// Runs three-tier escalation over the selected Chain scope and persists
/// updated cache artifacts.
pub trait RefVerifyApplicationService: Send + Sync {
    /// Execute the semantic reference verification use case.
    ///
    /// # Errors
    ///
    /// Returns [`RefVerifyError`] on any pipeline failure. See variant docs
    /// for the distinction between `SemanticFailuresConfirmed` (writer/fix
    /// loop re-entry) and `HumanEscalationRequired` (manual intervention).
    fn execute(&self, cmd: &RefVerifyCommand) -> Result<(), RefVerifyError>;
}

mod interactor;
pub use interactor::VerifySemanticRefsInteractor;

pub mod check_approved;
pub use check_approved::{
    CheckApprovedOutcome, RefVerifyCheckApprovedInteractor, RefVerifyCheckApprovedService,
};

pub mod driver_service;
pub use driver_service::{
    RefVerifyAggregateService, RefVerifyCheckApprovedDriverService, RefVerifyCheckApprovedOutcome,
    RefVerifyDriverError, RefVerifyRunOutcome, RefVerifyRunService,
};
