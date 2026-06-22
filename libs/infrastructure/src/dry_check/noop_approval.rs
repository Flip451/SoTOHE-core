//! No-op adapter for [`usecase::dry_check::DryCheckApprovalService`].
//!
//! Relocated from `cli_composition::track::fixpoint_resolve` per ADR 1328 D7.
//!
//! [`NoOpDryApprovalService`] unconditionally returns `Approved` and is used when
//! `dry_config.enabled` is `false`: the `FixpointResolveInteractor` bypasses the dry
//! gate in that case, but the field is `Arc<dyn DryCheckApprovalService>` and must be
//! constructed regardless. This no-op ensures construction succeeds without any I/O.

use std::collections::BTreeSet;

use domain::TrackId;
use domain::dry_check::{DryCheckApprovalVerdict, FragmentRef};
use usecase::dry_check::{DryCheckApprovalService, DryCheckCycleError};

/// A trivial no-op [`DryCheckApprovalService`] that always returns `Approved`.
pub struct NoOpDryApprovalService;

impl DryCheckApprovalService for NoOpDryApprovalService {
    fn check_approved(
        &self,
        _track_id: &TrackId,
        _current_fragment_refs: &BTreeSet<FragmentRef>,
    ) -> Result<DryCheckApprovalVerdict, DryCheckCycleError> {
        Ok(DryCheckApprovalVerdict::Approved)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::collections::BTreeSet;

    use domain::TrackId;
    use domain::dry_check::DryCheckApprovalVerdict;
    use usecase::dry_check::DryCheckApprovalService as _;

    use super::NoOpDryApprovalService;

    #[test]
    fn test_noop_dry_approval_service_check_approved_returns_approved() {
        let service = NoOpDryApprovalService;
        let track_id = TrackId::try_new("my-track-2026").unwrap();
        let refs = BTreeSet::new();
        let result = service.check_approved(&track_id, &refs);
        assert!(result.is_ok(), "NoOpDryApprovalService::check_approved must return Ok");
        assert_eq!(
            result.unwrap(),
            DryCheckApprovalVerdict::Approved,
            "NoOpDryApprovalService::check_approved must return Approved"
        );
    }
}
