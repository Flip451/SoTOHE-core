#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use std::collections::BTreeSet;

use domain::dry_check::{DryCheckApprovalVerdict, DryCheckConfigFingerprint, FragmentRef};
use domain::track_phase::ReviewScopeSet;

use super::*;
use crate::d4_orchestration::D4OrchestrationError;
use crate::dry_check::{
    DryCheckApprovalService, DryCheckConfig, DryCheckCycleError, DryCheckParallelism,
    DryCheckPercent,
};
use crate::fixpoint_resolve::{FixpointDryGateOutput, RefVerifyGateStatus, ReviewGateStatus};

// ── Test doubles ──────────────────────────────────────────────────────────

struct StubWorkspaceContext {
    result: Result<FixpointWorkspaceContext, FixpointWorkspaceContextError>,
}

impl FixpointWorkspaceContextPort for StubWorkspaceContext {
    fn resolve_context(
        &self,
        _items_dir: &Path,
        _track_id: &TrackId,
    ) -> Result<FixpointWorkspaceContext, FixpointWorkspaceContextError> {
        self.result.clone()
    }
}

struct StubDryConfigLoader {
    result: Result<(DryCheckConfig, DryCheckConfigFingerprint), DryCheckConfigLoaderError>,
}

impl DryCheckConfigLoaderPort for StubDryConfigLoader {
    fn load(
        &self,
        _repo_root: &Path,
    ) -> Result<(DryCheckConfig, DryCheckConfigFingerprint), DryCheckConfigLoaderError> {
        self.result.clone()
    }
}

struct StubDryApprovalForDriver {
    verdict: DryCheckApprovalVerdict,
}

impl DryCheckApprovalService for StubDryApprovalForDriver {
    fn check_approved(
        &self,
        _track_id: &TrackId,
        _current_fragment_refs: &BTreeSet<FragmentRef>,
    ) -> Result<DryCheckApprovalVerdict, DryCheckCycleError> {
        Ok(self.verdict.clone())
    }
}

struct StubFixpointDryGateService {
    fail: Option<D4OrchestrationError>,
    verdict: DryCheckApprovalVerdict,
}

impl FixpointDryGateService for StubFixpointDryGateService {
    fn resolve_dry_gate(
        &self,
        _cmd: FixpointDryGateCommand,
    ) -> Result<FixpointDryGateOutput, D4OrchestrationError> {
        if let Some(ref e) = self.fail {
            return Err(e.clone());
        }
        Ok(FixpointDryGateOutput {
            current_fragment_refs: BTreeSet::new(),
            dry_approval: Arc::new(StubDryApprovalForDriver { verdict: self.verdict.clone() }),
            approval_workspace_root: PathBuf::new(),
        })
    }
}

struct StubDryGateFactory {
    fail: Option<D4OrchestrationError>,
    verdict: DryCheckApprovalVerdict,
}

impl FixpointDryGateFactoryPort for StubDryGateFactory {
    fn build(&self, _base_branch: &str) -> Arc<dyn FixpointDryGateService> {
        Arc::new(StubFixpointDryGateService {
            fail: self.fail.clone(),
            verdict: self.verdict.clone(),
        })
    }
}

struct StubReviewGateForDriver {
    status: Result<ReviewGateStatus, FixpointResolveError>,
}

impl ReviewGateStatePort for StubReviewGateForDriver {
    fn review_status(&self, _track_id: &TrackId) -> Result<ReviewGateStatus, FixpointResolveError> {
        self.status.clone()
    }
}

struct StubRefVerifyGateForDriver {
    status: Result<RefVerifyGateStatus, FixpointResolveError>,
}

impl RefVerifyGateStatePort for StubRefVerifyGateForDriver {
    fn ref_verify_status(
        &self,
        _track_id: &TrackId,
    ) -> Result<RefVerifyGateStatus, FixpointResolveError> {
        self.status.clone()
    }
}

struct StubGateStateFactory {
    review_status: ReviewGateStatus,
    ref_verify_status: RefVerifyGateStatus,
}

impl FixpointGateStateFactoryPort for StubGateStateFactory {
    fn build_review_gate(
        &self,
        _items_dir: &Path,
        _base_branch: &str,
    ) -> Arc<dyn ReviewGateStatePort> {
        Arc::new(StubReviewGateForDriver { status: Ok(self.review_status.clone()) })
    }

    fn build_ref_verify_gate(&self, _items_dir: &Path) -> Arc<dyn RefVerifyGateStatePort> {
        Arc::new(StubRefVerifyGateForDriver { status: Ok(self.ref_verify_status.clone()) })
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────

fn test_dry_check_config(enabled: bool) -> DryCheckConfig {
    DryCheckConfig::new(
        DryCheckPercent::try_new(10).unwrap(),
        DryCheckPercent::try_new(90).unwrap(),
        DryCheckParallelism::try_new(4).unwrap(),
        enabled,
    )
}

fn make_config_fingerprint() -> DryCheckConfigFingerprint {
    DryCheckConfigFingerprint::new("c".repeat(64)).unwrap()
}

fn make_context() -> FixpointWorkspaceContext {
    FixpointWorkspaceContext {
        repo_root: PathBuf::from("/repo"),
        canonical_items_dir: PathBuf::from("/repo/track/items"),
        canonical_root: PathBuf::from("/repo"),
        base_branch: "main".to_owned(),
    }
}

fn make_input() -> FixpointResolveDriverInput {
    FixpointResolveDriverInput {
        track_id: "test-track-2026".to_owned(),
        current_branch: "track/test-track-2026".to_owned(),
        items_dir: PathBuf::from("track/items"),
    }
}

fn make_driver_interactor(
    dry_verdict: DryCheckApprovalVerdict,
    review_status: ReviewGateStatus,
    ref_verify_status: RefVerifyGateStatus,
) -> FixpointResolveDriverInteractor {
    FixpointResolveDriverInteractor::new(
        Arc::new(StubWorkspaceContext { result: Ok(make_context()) }),
        Arc::new(StubDryConfigLoader {
            result: Ok((test_dry_check_config(true), make_config_fingerprint())),
        }),
        Arc::new(StubDryGateFactory { fail: None, verdict: dry_verdict }),
        Arc::new(StubGateStateFactory { review_status, ref_verify_status }),
    )
}

/// Stub ports that all fail loudly with a distinctive message if called —
/// used to prove that a short-circuit guard fires before any port is
/// invoked (the resulting `Failure.message` would contain "must not be
/// called" instead of the expected guard diagnostic if it did not).
fn make_short_circuit_interactor() -> FixpointResolveDriverInteractor {
    FixpointResolveDriverInteractor::new(
        Arc::new(StubWorkspaceContext {
            result: Err(FixpointWorkspaceContextError::Unavailable(
                "must not be called".to_owned(),
            )),
        }),
        Arc::new(StubDryConfigLoader {
            result: Err(DryCheckConfigLoaderError::Unavailable("must not be called".to_owned())),
        }),
        Arc::new(StubDryGateFactory {
            fail: Some(D4OrchestrationError::DryGate("must not be called".to_owned())),
            verdict: DryCheckApprovalVerdict::Approved,
        }),
        Arc::new(StubGateStateFactory {
            review_status: ReviewGateStatus::Approved,
            ref_verify_status: RefVerifyGateStatus::Approved,
        }),
    )
}

// ── FixpointStep → Outcome mapping ──────────────────────────────────────────

#[test]
fn fixpoint_resolve_dry_blocked_returns_run_dfp() {
    let interactor = make_driver_interactor(
        DryCheckApprovalVerdict::Blocked { unresolved_pair_count: 2 },
        ReviewGateStatus::Approved,
        RefVerifyGateStatus::Approved,
    );
    let outcome = interactor.fixpoint_resolve(make_input());
    assert!(
        matches!(outcome, FixpointResolveDriverOutcome::RunDfp),
        "expected RunDfp, got {outcome:?}"
    );
}

#[test]
fn fixpoint_resolve_review_needs_review_returns_run_rfp_with_sorted_scopes() {
    let mut set = BTreeSet::new();
    set.insert("impl-plan".to_owned());
    set.insert("code".to_owned());
    let scopes = ReviewScopeSet::try_new(set).unwrap();
    let interactor = make_driver_interactor(
        DryCheckApprovalVerdict::Approved,
        ReviewGateStatus::NeedsReview { scopes },
        RefVerifyGateStatus::Approved,
    );
    let outcome = interactor.fixpoint_resolve(make_input());
    match outcome {
        FixpointResolveDriverOutcome::RunRfp { scopes } => {
            assert_eq!(scopes, vec!["code".to_owned(), "impl-plan".to_owned()]);
        }
        other => panic!("expected RunRfp, got {other:?}"),
    }
}

#[test]
fn fixpoint_resolve_ref_verify_blocked_returns_run_ref_verify() {
    let interactor = make_driver_interactor(
        DryCheckApprovalVerdict::Approved,
        ReviewGateStatus::Approved,
        RefVerifyGateStatus::Blocked,
    );
    let outcome = interactor.fixpoint_resolve(make_input());
    assert!(
        matches!(outcome, FixpointResolveDriverOutcome::RunRefVerify),
        "expected RunRefVerify, got {outcome:?}"
    );
}

#[test]
fn fixpoint_resolve_all_gates_green_returns_commit() {
    let interactor = make_driver_interactor(
        DryCheckApprovalVerdict::Approved,
        ReviewGateStatus::Approved,
        RefVerifyGateStatus::Approved,
    );
    let outcome = interactor.fixpoint_resolve(make_input());
    assert!(
        matches!(outcome, FixpointResolveDriverOutcome::Commit),
        "expected Commit, got {outcome:?}"
    );
}

// ── Port failure propagation ────────────────────────────────────────────────

#[test]
fn fixpoint_resolve_workspace_context_failure_returns_failure_outcome() {
    let interactor = FixpointResolveDriverInteractor::new(
        Arc::new(StubWorkspaceContext {
            result: Err(FixpointWorkspaceContextError::Unavailable("boom".to_owned())),
        }),
        Arc::new(StubDryConfigLoader {
            result: Ok((test_dry_check_config(true), make_config_fingerprint())),
        }),
        Arc::new(StubDryGateFactory { fail: None, verdict: DryCheckApprovalVerdict::Approved }),
        Arc::new(StubGateStateFactory {
            review_status: ReviewGateStatus::Approved,
            ref_verify_status: RefVerifyGateStatus::Approved,
        }),
    );
    let outcome = interactor.fixpoint_resolve(make_input());
    match outcome {
        FixpointResolveDriverOutcome::Failure { message } => {
            assert!(message.contains("boom"), "got: {message}");
        }
        other => panic!("expected Failure, got {other:?}"),
    }
}

#[test]
fn fixpoint_resolve_dry_config_loader_failure_returns_failure_outcome() {
    let interactor = FixpointResolveDriverInteractor::new(
        Arc::new(StubWorkspaceContext { result: Ok(make_context()) }),
        Arc::new(StubDryConfigLoader {
            result: Err(DryCheckConfigLoaderError::Unavailable("bad config".to_owned())),
        }),
        Arc::new(StubDryGateFactory { fail: None, verdict: DryCheckApprovalVerdict::Approved }),
        Arc::new(StubGateStateFactory {
            review_status: ReviewGateStatus::Approved,
            ref_verify_status: RefVerifyGateStatus::Approved,
        }),
    );
    let outcome = interactor.fixpoint_resolve(make_input());
    match outcome {
        FixpointResolveDriverOutcome::Failure { message } => {
            assert!(message.contains("bad config"), "got: {message}");
        }
        other => panic!("expected Failure, got {other:?}"),
    }
}

#[test]
fn fixpoint_resolve_dry_gate_failure_returns_failure_outcome() {
    let interactor = FixpointResolveDriverInteractor::new(
        Arc::new(StubWorkspaceContext { result: Ok(make_context()) }),
        Arc::new(StubDryConfigLoader {
            result: Ok((test_dry_check_config(true), make_config_fingerprint())),
        }),
        Arc::new(StubDryGateFactory {
            fail: Some(D4OrchestrationError::DryGate("dry gate exploded".to_owned())),
            verdict: DryCheckApprovalVerdict::Approved,
        }),
        Arc::new(StubGateStateFactory {
            review_status: ReviewGateStatus::Approved,
            ref_verify_status: RefVerifyGateStatus::Approved,
        }),
    );
    let outcome = interactor.fixpoint_resolve(make_input());
    match outcome {
        FixpointResolveDriverOutcome::Failure { message } => {
            assert!(message.contains("dry gate exploded"), "got: {message}");
        }
        other => panic!("expected Failure, got {other:?}"),
    }
}

// ── Relocated from apps/cli-composition/src/track/fixpoint_resolve.rs ─────
// (the three short-circuit tests that fire before any port is called)

#[test]
fn fixpoint_resolve_wrong_branch_returns_track_not_active_failure() {
    let interactor = make_short_circuit_interactor();
    let input = FixpointResolveDriverInput {
        track_id: "my-track-2026".to_owned(),
        current_branch: "main".to_owned(),
        items_dir: PathBuf::from("track/items"),
    };
    let outcome = interactor.fixpoint_resolve(input);
    match outcome {
        FixpointResolveDriverOutcome::Failure { message } => {
            assert!(
                message.contains("not active") || message.contains("track/my-track-2026"),
                "error must mention the expected branch, got: {message}"
            );
        }
        other => panic!("expected Failure, got {other:?}"),
    }
}

#[test]
fn fixpoint_resolve_empty_current_branch_returns_invalid_current_branch_failure() {
    let interactor = make_short_circuit_interactor();
    let input = FixpointResolveDriverInput {
        track_id: "my-track-2026".to_owned(),
        current_branch: String::new(),
        items_dir: PathBuf::from("track/items"),
    };
    let outcome = interactor.fixpoint_resolve(input);
    match outcome {
        FixpointResolveDriverOutcome::Failure { message } => {
            assert!(
                message.contains("invalid current branch"),
                "error must mention the invalid current branch, got: {message}"
            );
        }
        other => panic!("expected Failure, got {other:?}"),
    }
}

#[test]
fn fixpoint_resolve_invalid_track_id_returns_failure() {
    let interactor = make_short_circuit_interactor();
    let input = FixpointResolveDriverInput {
        track_id: String::new(),
        current_branch: "track/x".to_owned(),
        items_dir: PathBuf::from("track/items"),
    };
    let outcome = interactor.fixpoint_resolve(input);
    match outcome {
        FixpointResolveDriverOutcome::Failure { message } => {
            assert!(
                message.contains("track id"),
                "error must mention the invalid track id, got: {message}"
            );
        }
        other => panic!("expected Failure, got {other:?}"),
    }
}
