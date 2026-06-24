//! Check-approved service and helpers.

use std::path::Path;

use domain::TrackId;
use domain::review_v2::ReviewExistsPort;

use usecase::review_v2::{ReviewApprovalDecision, ReviewApprovalOutput, ReviewCheckApprovedError};

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
pub(crate) fn check_approved_str(
    track_id_str: &str,
    items_dir: &Path,
) -> Result<ReviewApprovalOutput, ReviewCheckApprovedError> {
    use domain::review_v2::ReviewApprovalVerdict;

    let track_id = TrackId::try_new(track_id_str)
        .map_err(|e| ReviewCheckApprovedError::InvalidTrackId(e.to_string()))?;

    let comp = build_review_v2(&track_id, items_dir)
        .map_err(|e| ReviewCheckApprovedError::ReviewStoreError(e.to_string()))?;

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
