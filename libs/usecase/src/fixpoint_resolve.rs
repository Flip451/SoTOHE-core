//! Mechanized fixpoint resolution for the DFP→RFP→ref-verify→commit loop.
//!
//! The [`FixpointResolveInteractor`] implements [`FixpointResolveService`] by querying
//! three gate ports in priority order (dry → review → ref-verify) and returning the
//! single [`domain::track_phase::FixpointStep`] the orchestrator must execute next.
//!
//! Design decisions: D2 / IN-02 / AC-03 / AC-04 / CN-02.

use std::collections::BTreeSet;
use std::sync::Arc;

use domain::TrackId;
use domain::dry_check::{DryCheckApprovalVerdict, FragmentRef};
use domain::track_phase::{FixpointStep, ReviewScopeSet};
use thiserror::Error;

use crate::dry_check::DryCheckApprovalService;

// ── FixpointCurrentBranch ─────────────────────────────────────────────────────

/// Validated opaque current git branch label for fixpoint resolution.
///
/// `try_new` rejects empty or whitespace-only strings with
/// [`FixpointResolveError::InvalidCurrentBranch`]; branch naming remains an
/// infrastructure/orchestrator concern so the usecase only validates non-emptiness.
///
/// # Errors
///
/// [`try_new`](FixpointCurrentBranch::try_new) returns
/// [`FixpointResolveError::InvalidCurrentBranch`] when `value` is empty or
/// consists solely of whitespace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixpointCurrentBranch(String);

impl FixpointCurrentBranch {
    /// Construct a [`FixpointCurrentBranch`] from a non-empty, non-whitespace string.
    ///
    /// # Errors
    ///
    /// Returns [`FixpointResolveError::InvalidCurrentBranch`] when `value` is empty
    /// or whitespace-only.
    pub fn try_new(value: String) -> Result<Self, FixpointResolveError> {
        if value.trim().is_empty() {
            return Err(FixpointResolveError::InvalidCurrentBranch {
                message: "current branch must be non-empty and non-whitespace".to_owned(),
            });
        }
        Ok(Self(value))
    }

    /// Returns the branch label as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

// ── FixpointResolveCommand ────────────────────────────────────────────────────

/// Command input for [`FixpointResolveService::resolve`].
///
/// Identifies the active track, the caller's current git branch, and the current
/// diff [`FragmentRef`] set needed by the dry gate public API.
///
/// `current_branch` is a validated opaque [`FixpointCurrentBranch`] value so empty
/// branch labels cannot enter the interactor.  The CLI composition layer (T015) is
/// responsible for asserting that `current_branch` matches the track's recorded
/// branch and returning [`FixpointResolveError::TrackNotActive`] before calling
/// [`FixpointResolveService::resolve`]; the interactor itself composes only the
/// three gate ports (dry / review / ref-verify) per CN-02.
///
/// `current_fragment_refs` is computed by the composition layer from the current diff
/// and passed through so the interactor does not acquire diff fragments itself (CN-02).
#[derive(Debug, Clone)]
pub struct FixpointResolveCommand {
    /// The track whose fixpoint is being resolved.
    pub track_id: domain::TrackId,
    /// Validated current git branch label.
    ///
    /// Validated by [`FixpointCurrentBranch::try_new`] to be non-empty and
    /// non-whitespace.  Branch-against-track consistency is enforced by the CLI
    /// composition layer before [`FixpointResolveService::resolve`] is called.
    pub current_branch: FixpointCurrentBranch,
    /// Current diff [`FragmentRef`] set computed by the composition layer.
    pub current_fragment_refs: BTreeSet<FragmentRef>,
}

// ── FixpointResolveError ──────────────────────────────────────────────────────

/// Error variants for [`FixpointResolveService::resolve`].
///
/// `GateQueryFailed` carries an opaque gate name string (e.g. `"dry"`, `"review"`,
/// `"ref_verify"`) and a message; both are labels used only for human-readable
/// diagnostics.
///
/// `TrackNotActive` is intended for the CLI composition layer (T015): the composition
/// must assert that [`FixpointResolveCommand::current_branch`] matches the track's
/// recorded branch before calling `resolve`, and return this variant when it does not.
/// The interactor itself does not perform this check because branch-against-track
/// consistency is an orchestration/infrastructure concern, not a gate-composition
/// concern (CN-02).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum FixpointResolveError {
    /// The supplied track ID failed validation.
    #[error("invalid track ID: {message}")]
    InvalidTrackId {
        /// Human-readable validation failure detail.
        message: String,
    },
    /// The supplied current branch label is empty or whitespace-only.
    #[error("invalid current branch: {message}")]
    InvalidCurrentBranch {
        /// Human-readable validation failure detail.
        message: String,
    },
    /// The track's branch does not match the active branch.
    ///
    /// Returned by the CLI composition layer (T015) when it detects that the
    /// caller's `current_branch` does not match the track's recorded branch.
    /// Not produced by [`FixpointResolveInteractor::resolve`] itself.
    #[error("track is not active on branch '{branch}'")]
    TrackNotActive {
        /// The branch on which the track was expected to be active.
        branch: String,
    },
    /// One of the gate queries failed.
    #[error("gate '{gate}' query failed: {message}")]
    GateQueryFailed {
        /// Opaque gate name label (e.g. `"dry"`, `"review"`, `"ref_verify"`).
        gate: String,
        /// Human-readable error detail.
        message: String,
    },
}

// ── ReviewGateStatus ──────────────────────────────────────────────────────────

/// Review gate status consumed by the fixpoint resolver (D2).
///
/// - [`Approved`](ReviewGateStatus::Approved): no review scope is stale or has
///   findings remaining.
/// - [`NeedsReview`](ReviewGateStatus::NeedsReview): carries the non-empty
///   deterministic set of opaque review scope labels that must run through RFP.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewGateStatus {
    /// All review scopes are approved; no RFP is needed.
    Approved,
    /// One or more review scopes require RFP.
    NeedsReview {
        /// Non-empty set of scope labels that must run RFP.
        scopes: ReviewScopeSet,
    },
}

// ── RefVerifyGateStatus ───────────────────────────────────────────────────────

/// Ref-verify gate status consumed by the fixpoint resolver (D2).
///
/// - [`Approved`](RefVerifyGateStatus::Approved): ref-verify passed.
/// - [`Blocked`](RefVerifyGateStatus::Blocked): ref-verify must run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefVerifyGateStatus {
    /// Ref-verify gate is clear.
    Approved,
    /// Ref-verify must run before committing.
    Blocked,
}

// ── ReviewGateStatePort ───────────────────────────────────────────────────────

/// Secondary port used by [`FixpointResolveInteractor`] to query the review gate
/// through its public read API.
///
/// Returns an enum status rather than exposing `review.json` internals (CN-02).
///
/// # Errors
///
/// Implementations return [`FixpointResolveError::GateQueryFailed`] on any
/// underlying I/O or parse failure.
pub trait ReviewGateStatePort: Send + Sync {
    /// Query the current review gate status for `track_id`.
    ///
    /// # Errors
    ///
    /// Returns [`FixpointResolveError::GateQueryFailed`] on query failure.
    fn review_status(&self, track_id: &TrackId) -> Result<ReviewGateStatus, FixpointResolveError>;
}

// ── RefVerifyGateStatePort ────────────────────────────────────────────────────

/// Secondary port used by [`FixpointResolveInteractor`] to query the ref-verify
/// gate through its public read API.
///
/// Returns only gate status and never exposes ref-verify cache internals (CN-02).
///
/// # Errors
///
/// Implementations return [`FixpointResolveError::GateQueryFailed`] on any
/// underlying I/O or parse failure.
pub trait RefVerifyGateStatePort: Send + Sync {
    /// Query the current ref-verify gate status for `track_id`.
    ///
    /// # Errors
    ///
    /// Returns [`FixpointResolveError::GateQueryFailed`] on query failure.
    fn ref_verify_status(
        &self,
        track_id: &TrackId,
    ) -> Result<RefVerifyGateStatus, FixpointResolveError>;
}

// ── FixpointResolveService ────────────────────────────────────────────────────

/// Application service for mechanized fixpoint resolution (D2).
///
/// Queries the three gate states (dry / review / ref-verify) via their public APIs
/// and returns the next required phase as a [`domain::track_phase::FixpointStep`].
///
/// CN-02: no direct access to internal gate file formats.
///
/// # Errors
///
/// Returns [`FixpointResolveError`] on validation failures or gate query errors.
pub trait FixpointResolveService: Send + Sync {
    /// Determine the next convergence step for the given command.
    ///
    /// # Errors
    ///
    /// Returns [`FixpointResolveError`] if gate queries fail or command input is
    /// invalid.
    fn resolve(&self, cmd: &FixpointResolveCommand) -> Result<FixpointStep, FixpointResolveError>;
}

// ── FixpointResolveInteractor ─────────────────────────────────────────────────

/// Implements [`FixpointResolveService`].
///
/// Holds injected port dependencies (dry approval port, review state port,
/// ref-verify result port) as private `Arc<dyn Port>` fields.
/// Queries each gate's public API only — no direct access to `dry-check.json`,
/// `review.json`, or ref-verify cache internals (CN-02).
///
/// The branch-against-track consistency check (`TrackNotActive`) is the
/// responsibility of the CLI composition layer (T015) and is performed before
/// this interactor is called.
pub struct FixpointResolveInteractor {
    dry_approval: Arc<dyn DryCheckApprovalService + Send + Sync>,
    review_state: Arc<dyn ReviewGateStatePort>,
    ref_verify_results: Arc<dyn RefVerifyGateStatePort>,
}

impl FixpointResolveInteractor {
    /// Create a new [`FixpointResolveInteractor`].
    ///
    /// # Parameters
    ///
    /// - `dry_approval`: pure-read dry gate via [`DryCheckApprovalService::check_approved`].
    /// - `review_state`: review gate status via [`ReviewGateStatePort::review_status`].
    /// - `ref_verify_results`: ref-verify gate status via
    ///   [`RefVerifyGateStatePort::ref_verify_status`].
    #[must_use]
    pub fn new(
        dry_approval: Arc<dyn DryCheckApprovalService + Send + Sync>,
        review_state: Arc<dyn ReviewGateStatePort>,
        ref_verify_results: Arc<dyn RefVerifyGateStatePort>,
    ) -> Self {
        Self { dry_approval, review_state, ref_verify_results }
    }
}

impl FixpointResolveService for FixpointResolveInteractor {
    /// Evaluate all gate states in priority order and return the next required step.
    ///
    /// Algorithm (CN-02 — public API composition only):
    ///
    /// 1. Query dry gate via `dry_approval.check_approved(track_id, current_fragment_refs)`.
    /// 2. If [`DryCheckApprovalVerdict::Blocked`] → return [`FixpointStep::RunDfp`].
    /// 3. If `Approved` → query review gate via `review_state.review_status(track_id)`.
    /// 4. If [`ReviewGateStatus::NeedsReview { scopes }`] → return
    ///    [`FixpointStep::RunRfp { scopes }`].
    /// 5. If `Approved` → query ref-verify via `ref_verify_results.ref_verify_status(track_id)`.
    /// 6. If [`RefVerifyGateStatus::Blocked`] → return [`FixpointStep::RunRefVerify`].
    /// 7. If `Approved` → return [`FixpointStep::Commit`].
    ///
    /// Note: `cmd.current_branch` is validated by [`FixpointCurrentBranch::try_new`]
    /// before this method is called; branch-against-track consistency is enforced by
    /// the CLI composition layer (T015) via [`FixpointResolveError::TrackNotActive`].
    ///
    /// # Errors
    ///
    /// Returns [`FixpointResolveError::GateQueryFailed`] when any gate port returns
    /// an error, carrying the gate name (`"dry"`, `"review"`, or `"ref_verify"`) and
    /// the error detail.
    fn resolve(&self, cmd: &FixpointResolveCommand) -> Result<FixpointStep, FixpointResolveError> {
        // ── Step 1–2: Dry gate. ───────────────────────────────────────────────
        let dry_verdict = self
            .dry_approval
            .check_approved(&cmd.track_id, &cmd.current_fragment_refs)
            .map_err(|e| FixpointResolveError::GateQueryFailed {
                gate: "dry".to_owned(),
                message: e.to_string(),
            })?;

        if matches!(dry_verdict, DryCheckApprovalVerdict::Blocked { .. }) {
            return Ok(FixpointStep::RunDfp);
        }

        // ── Step 3–4: Review gate. ────────────────────────────────────────────
        let review_status = self.review_state.review_status(&cmd.track_id).map_err(|e| {
            FixpointResolveError::GateQueryFailed {
                gate: "review".to_owned(),
                message: e.to_string(),
            }
        })?;

        if let ReviewGateStatus::NeedsReview { scopes } = review_status {
            return Ok(FixpointStep::RunRfp { scopes });
        }

        // ── Step 5–6: Ref-verify gate. ────────────────────────────────────────
        let ref_verify_status =
            self.ref_verify_results.ref_verify_status(&cmd.track_id).map_err(|e| {
                FixpointResolveError::GateQueryFailed {
                    gate: "ref_verify".to_owned(),
                    message: e.to_string(),
                }
            })?;

        if matches!(ref_verify_status, RefVerifyGateStatus::Blocked) {
            return Ok(FixpointStep::RunRefVerify);
        }

        // ── Step 7: All gates green. ──────────────────────────────────────────
        Ok(FixpointStep::Commit)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use std::collections::BTreeSet;

    use domain::TrackId;
    use domain::dry_check::DryCheckApprovalVerdict;
    use domain::track_phase::ReviewScopeSet;

    use crate::dry_check::DryCheckCycleError;

    use super::*;

    // ── Test doubles ──────────────────────────────────────────────────────────

    /// Stub dry approval that returns a fixed verdict or a simulated error.
    ///
    /// `fail` controls whether the port returns an error; when `false` the
    /// `verdict` field is returned. Using an enum flag avoids the need for
    /// `DryCheckCycleError: Clone`.
    struct StubDryApproval {
        verdict: DryCheckApprovalVerdict,
        /// When `true`, the port returns a `CoveragePort` error.
        fail: bool,
    }

    impl DryCheckApprovalService for StubDryApproval {
        fn check_approved(
            &self,
            _track_id: &TrackId,
            _current_fragment_refs: &BTreeSet<domain::dry_check::FragmentRef>,
        ) -> Result<DryCheckApprovalVerdict, DryCheckCycleError> {
            if self.fail {
                return Err(DryCheckCycleError::CoveragePort("simulated error".to_owned()));
            }
            Ok(self.verdict.clone())
        }
    }

    struct StubReviewGate {
        status: Result<ReviewGateStatus, FixpointResolveError>,
    }

    impl ReviewGateStatePort for StubReviewGate {
        fn review_status(
            &self,
            _track_id: &TrackId,
        ) -> Result<ReviewGateStatus, FixpointResolveError> {
            self.status.clone()
        }
    }

    struct StubRefVerifyGate {
        status: Result<RefVerifyGateStatus, FixpointResolveError>,
    }

    impl RefVerifyGateStatePort for StubRefVerifyGate {
        fn ref_verify_status(
            &self,
            _track_id: &TrackId,
        ) -> Result<RefVerifyGateStatus, FixpointResolveError> {
            self.status.clone()
        }
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn make_track_id() -> TrackId {
        TrackId::try_new("test-track-2026").unwrap()
    }

    fn make_cmd() -> FixpointResolveCommand {
        FixpointResolveCommand {
            track_id: make_track_id(),
            current_branch: FixpointCurrentBranch::try_new("track/test".to_owned()).unwrap(),
            current_fragment_refs: BTreeSet::new(),
        }
    }

    fn make_interactor(
        dry_verdict: DryCheckApprovalVerdict,
        review: Result<ReviewGateStatus, FixpointResolveError>,
        ref_verify: Result<RefVerifyGateStatus, FixpointResolveError>,
    ) -> FixpointResolveInteractor {
        FixpointResolveInteractor::new(
            Arc::new(StubDryApproval { verdict: dry_verdict, fail: false }),
            Arc::new(StubReviewGate { status: review }),
            Arc::new(StubRefVerifyGate { status: ref_verify }),
        )
    }

    fn make_interactor_with_dry_error(
        review: Result<ReviewGateStatus, FixpointResolveError>,
        ref_verify: Result<RefVerifyGateStatus, FixpointResolveError>,
    ) -> FixpointResolveInteractor {
        FixpointResolveInteractor::new(
            Arc::new(StubDryApproval { verdict: DryCheckApprovalVerdict::Approved, fail: true }),
            Arc::new(StubReviewGate { status: review }),
            Arc::new(StubRefVerifyGate { status: ref_verify }),
        )
    }

    // ── FixpointCurrentBranch ─────────────────────────────────────────────────

    #[test]
    fn fixpoint_current_branch_try_new_with_empty_string_returns_invalid_current_branch() {
        let result = FixpointCurrentBranch::try_new(String::new());
        assert!(matches!(result, Err(FixpointResolveError::InvalidCurrentBranch { .. })));
    }

    #[test]
    fn fixpoint_current_branch_try_new_with_whitespace_only_returns_invalid_current_branch() {
        let result = FixpointCurrentBranch::try_new("   ".to_owned());
        assert!(matches!(result, Err(FixpointResolveError::InvalidCurrentBranch { .. })));
    }

    #[test]
    fn fixpoint_current_branch_try_new_with_valid_label_succeeds_and_as_str_round_trips() {
        let result = FixpointCurrentBranch::try_new("track/foo".to_owned());
        assert!(result.is_ok());
        assert_eq!(result.unwrap().as_str(), "track/foo");
    }

    // ── FixpointResolveInteractor — gate priority sequence ────────────────────

    #[test]
    fn interactor_resolve_dry_blocked_returns_run_dfp() {
        let interactor = make_interactor(
            DryCheckApprovalVerdict::Blocked { unresolved_pair_count: 2 },
            Ok(ReviewGateStatus::Approved),
            Ok(RefVerifyGateStatus::Approved),
        );
        let step = interactor.resolve(&make_cmd()).unwrap();
        assert_eq!(step, FixpointStep::RunDfp);
    }

    #[test]
    fn interactor_resolve_dry_approved_review_needs_review_returns_run_rfp_with_scopes() {
        let mut scope_set = BTreeSet::new();
        scope_set.insert("plan-artifacts".to_owned());
        scope_set.insert("code".to_owned());
        let scopes = ReviewScopeSet::try_new(scope_set).unwrap();

        let interactor = make_interactor(
            DryCheckApprovalVerdict::Approved,
            Ok(ReviewGateStatus::NeedsReview { scopes: scopes.clone() }),
            Ok(RefVerifyGateStatus::Approved),
        );
        let step = interactor.resolve(&make_cmd()).unwrap();
        assert_eq!(step, FixpointStep::RunRfp { scopes });
    }

    #[test]
    fn interactor_resolve_dry_approved_review_approved_ref_verify_blocked_returns_run_ref_verify() {
        let interactor = make_interactor(
            DryCheckApprovalVerdict::Approved,
            Ok(ReviewGateStatus::Approved),
            Ok(RefVerifyGateStatus::Blocked),
        );
        let step = interactor.resolve(&make_cmd()).unwrap();
        assert_eq!(step, FixpointStep::RunRefVerify);
    }

    #[test]
    fn interactor_resolve_all_gates_approved_returns_commit() {
        let interactor = make_interactor(
            DryCheckApprovalVerdict::Approved,
            Ok(ReviewGateStatus::Approved),
            Ok(RefVerifyGateStatus::Approved),
        );
        let step = interactor.resolve(&make_cmd()).unwrap();
        assert_eq!(step, FixpointStep::Commit);
    }

    // ── FixpointResolveInteractor — gate error propagation ────────────────────

    #[test]
    fn interactor_resolve_dry_gate_error_returns_gate_query_failed_for_dry() {
        let interactor = make_interactor_with_dry_error(
            Ok(ReviewGateStatus::Approved),
            Ok(RefVerifyGateStatus::Approved),
        );
        let err = interactor.resolve(&make_cmd()).unwrap_err();
        assert!(
            matches!(&err, FixpointResolveError::GateQueryFailed { gate, .. } if gate == "dry"),
            "expected GateQueryFailed for 'dry' but got {err:?}"
        );
    }

    #[test]
    fn interactor_resolve_review_gate_error_returns_gate_query_failed_for_review() {
        let interactor = make_interactor(
            DryCheckApprovalVerdict::Approved,
            Err(FixpointResolveError::GateQueryFailed {
                gate: "review".to_owned(),
                message: "simulated error".to_owned(),
            }),
            Ok(RefVerifyGateStatus::Approved),
        );
        let err = interactor.resolve(&make_cmd()).unwrap_err();
        assert!(
            matches!(&err, FixpointResolveError::GateQueryFailed { gate, .. } if gate == "review"),
            "expected GateQueryFailed for 'review' but got {err:?}"
        );
    }

    #[test]
    fn interactor_resolve_ref_verify_gate_error_returns_gate_query_failed_for_ref_verify() {
        let interactor = make_interactor(
            DryCheckApprovalVerdict::Approved,
            Ok(ReviewGateStatus::Approved),
            Err(FixpointResolveError::GateQueryFailed {
                gate: "ref_verify".to_owned(),
                message: "simulated error".to_owned(),
            }),
        );
        let err = interactor.resolve(&make_cmd()).unwrap_err();
        assert!(
            matches!(&err, FixpointResolveError::GateQueryFailed { gate, .. } if gate == "ref_verify"),
            "expected GateQueryFailed for 'ref_verify' but got {err:?}"
        );
    }
}
