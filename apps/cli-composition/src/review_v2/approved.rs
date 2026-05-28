//! Check-approved service and helpers.

use std::path::Path;

use domain::TrackId;
use domain::review_v2::ReviewExistsPort;

use usecase::review_v2::{
    ReviewApprovalDecision, ReviewApprovalOutput, ReviewCheckApprovedError,
    ReviewCheckApprovedInteractor, ReviewCheckApprovedService,
};

use super::shared::build_review_v2;

/// Runs the full check-approved operation from string inputs and returns a
/// `ReviewApprovalOutput` DTO (usecase-owned, no domain types exposed).
///
/// Encapsulates `TrackId`, `ReviewExistsPort`, and `ReviewApprovalVerdict`
/// conversions so that the CLI layer never imports domain review types directly
/// (CN-01 / AC-03).
///
/// # Errors
/// Returns `ReviewCheckApprovedError` on track ID validation, store, or
/// evaluation failures.
pub fn check_approved_str(
    track_id_str: &str,
    items_dir: &Path,
) -> Result<ReviewApprovalOutput, ReviewCheckApprovedError> {
    use domain::review_v2::ReviewApprovalVerdict;

    let track_id = TrackId::try_new(track_id_str)
        .map_err(|e| ReviewCheckApprovedError::InvalidTrackId(e.to_string()))?;

    let comp = build_review_v2(&track_id, items_dir)
        .map_err(ReviewCheckApprovedError::ReviewStoreError)?;

    let review_json_exists = comp
        .review_store
        .review_json_exists()
        .map_err(|e| ReviewCheckApprovedError::ReviewStoreError(format!("{e}")))?;

    let verdict = comp
        .cycle
        .evaluate_approval(&comp.review_store, review_json_exists)
        .map_err(|e| ReviewCheckApprovedError::EvaluationFailed(e.to_string()))?;

    Ok(match verdict {
        ReviewApprovalVerdict::Approved => ReviewApprovalOutput {
            decision: ReviewApprovalDecision::Approved,
            bypass_scope_count: None,
            blocked_scopes: Vec::new(),
        },
        ReviewApprovalVerdict::ApprovedWithBypass { not_started_count } => ReviewApprovalOutput {
            decision: ReviewApprovalDecision::ApprovedWithBypass,
            bypass_scope_count: Some(not_started_count),
            blocked_scopes: Vec::new(),
        },
        ReviewApprovalVerdict::Blocked { required_scopes } => ReviewApprovalOutput {
            decision: ReviewApprovalDecision::Blocked,
            bypass_scope_count: None,
            blocked_scopes: required_scopes.iter().map(|s| s.to_string()).collect(),
        },
    })
}

/// Extracts the finding count from a verdict JSON string produced by
/// [`render_review_payload`].
///
/// Parses the JSON `"findings"` array and returns its length. Returns `0` when
/// the JSON cannot be parsed (fail-safe: the caller still has the raw JSON in
/// `summary`).
pub(super) fn count_findings_in_verdict_json(verdict_json: &str) -> usize {
    serde_json::from_str::<serde_json::Value>(verdict_json)
        .ok()
        .and_then(|v| v.get("findings").and_then(|f| f.as_array()).map(Vec::len))
        .unwrap_or(0)
}

/// Constructs an `Arc<dyn ReviewCheckApprovedService>` that the CLI can call
/// without importing infrastructure or domain types.
///
/// Returns a `ReviewCheckApprovedInteractor` whose closure delegates to
/// [`check_approved_str`] to perform the full domain + I/O operation.
///
/// # Purpose
///
/// This factory gives the CLI a usecase-service-trait handle rather than a
/// concrete `ReviewV2Composition` struct, satisfying the CN-01 / AC-03 wiring
/// requirement: the CLI composition root wires through
/// `Arc<dyn ReviewCheckApprovedService>` rather than touching concrete
/// infrastructure adapters directly.
#[must_use]
pub fn build_check_approved_service() -> std::sync::Arc<dyn ReviewCheckApprovedService> {
    use std::sync::Arc;
    Arc::new(ReviewCheckApprovedInteractor::new(|track_id, items_dir| {
        check_approved_str(track_id, items_dir)
    }))
}
