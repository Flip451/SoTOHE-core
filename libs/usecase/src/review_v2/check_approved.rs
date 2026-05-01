//! Review check-approved application service (usecase layer).
//!
//! Wraps `domain::review_v2::ReviewApprovalVerdict` behind
//! `ReviewApprovalOutput` so the CLI never imports domain review types
//! directly (CN-01 / D1).

use std::path::PathBuf;
use std::sync::Arc;

use thiserror::Error;

// ── ReviewApprovalDecision ────────────────────────────────────────────────────

/// Usecase-layer enum mirroring `domain::review_v2::ReviewApprovalVerdict`.
///
/// Used by [`ReviewApprovalOutput`]. The CLI consumes this enum instead of
/// `domain::ReviewApprovalVerdict` directly, satisfying CN-01.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewApprovalDecision {
    Approved,
    ApprovedWithBypass,
    Blocked,
}

// ── ReviewApprovalOutput ──────────────────────────────────────────────────────

/// DTO returned by the review check-approved operation.
///
/// Carries `decision` (`Approved`/`ApprovedWithBypass`/`Blocked`), optional
/// bypass scope count, and the list of required scopes when blocked. Replaces
/// direct CLI→`domain::review_v2::ReviewApprovalVerdict` usage so the CLI
/// never imports domain review types directly.
#[derive(Debug)]
pub struct ReviewApprovalOutput {
    pub decision: ReviewApprovalDecision,
    pub bypass_scope_count: Option<usize>,
    pub blocked_scopes: Vec<String>,
}

// ── ReviewCheckApprovedError ──────────────────────────────────────────────────

/// Error type for [`ReviewCheckApprovedService`].
///
/// Wraps invalid track ID, review store failures, and approval evaluation
/// failures without leaking domain error types across the usecase boundary.
#[derive(Debug, Error)]
pub enum ReviewCheckApprovedError {
    #[error("invalid track ID: {0}")]
    InvalidTrackId(String),
    #[error("review store error: {0}")]
    ReviewStoreError(String),
    #[error("evaluation failed: {0}")]
    EvaluationFailed(String),
}

// ── ReviewCheckApprovedService ────────────────────────────────────────────────

/// Application service trait for the review check-approved use case.
///
/// Driven by the CLI. Takes string `track_id` and `items_dir` to avoid
/// importing `domain::TrackId` in the CLI. Returns [`ReviewApprovalOutput`]
/// which is a usecase-owned DTO.
pub trait ReviewCheckApprovedService: Send + Sync {
    /// Checks whether the review is approved for the given track.
    ///
    /// # Errors
    ///
    /// Returns [`ReviewCheckApprovedError`] on track ID validation, store, or
    /// evaluation failures.
    fn check_approved(
        &self,
        track_id: String,
        items_dir: PathBuf,
    ) -> Result<ReviewApprovalOutput, ReviewCheckApprovedError>;
}

// ── ReviewCheckApprovedInteractor ─────────────────────────────────────────────

/// Function type for building the composition root for check-approved operations.
///
/// Receives `(track_id: &str, items_dir: &Path)` and returns
/// `Result<ReviewApprovalOutput, ReviewCheckApprovedError>`. Using `&str`
/// keeps `domain::TrackId` out of the public API: the closure wraps the domain
/// conversion internally. The CLI composition root injects the domain+infra
/// wiring without importing domain ID types.
pub(crate) type CheckApprovedBuildFn = Arc<
    dyn Fn(&str, &std::path::Path) -> Result<ReviewApprovalOutput, ReviewCheckApprovedError>
        + Send
        + Sync,
>;

/// Concrete struct implementing [`ReviewCheckApprovedService`].
///
/// Constructs domain types internally and converts results to
/// [`ReviewApprovalOutput`] before returning to the CLI.
///
/// The `build_fn` field is a function pointer that builds the composition root
/// from `(track_id, items_dir)` and returns the review cycle + store needed to
/// call `evaluate_approval`. This design avoids importing the full composition
/// root into the usecase crate (which would violate the hexagonal boundary).
pub struct ReviewCheckApprovedInteractor {
    build_fn: CheckApprovedBuildFn,
}

impl ReviewCheckApprovedInteractor {
    /// Creates a new interactor with the given composition-root builder.
    ///
    /// The builder receives `(&TrackId, &Path)` and returns
    /// `Result<ReviewApprovalOutput, ReviewCheckApprovedError>`. This lets the
    /// CLI composition root inject the domain+infra wiring without importing it
    /// from the usecase crate.
    #[must_use]
    pub fn new<F>(build_fn: F) -> Self
    where
        F: Fn(&str, &std::path::Path) -> Result<ReviewApprovalOutput, ReviewCheckApprovedError>
            + Send
            + Sync
            + 'static,
    {
        Self { build_fn: Arc::new(build_fn) }
    }
}

impl ReviewCheckApprovedService for ReviewCheckApprovedInteractor {
    fn check_approved(
        &self,
        track_id: String,
        items_dir: PathBuf,
    ) -> Result<ReviewApprovalOutput, ReviewCheckApprovedError> {
        // Validate the track ID format at the usecase boundary before forwarding.
        domain::TrackId::try_new(&track_id)
            .map_err(|e| ReviewCheckApprovedError::InvalidTrackId(e.to_string()))?;
        // Pass the validated string to the closure; domain conversion happens inside the closure.
        (self.build_fn)(&track_id, &items_dir)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    /// Converts `domain::review_v2::ReviewApprovalVerdict` to
    /// [`ReviewApprovalOutput`] for test assertions.
    ///
    /// This conversion is intentionally kept test-local: the usecase public API
    /// does not expose a `verdict_to_output` helper that takes a domain type,
    /// so the CLI composition root is expected to perform the mapping inline
    /// inside its `CheckApprovedBuildFn` closure.
    fn verdict_to_output(
        verdict: domain::review_v2::ReviewApprovalVerdict,
    ) -> ReviewApprovalOutput {
        match verdict {
            domain::review_v2::ReviewApprovalVerdict::Approved => ReviewApprovalOutput {
                decision: ReviewApprovalDecision::Approved,
                bypass_scope_count: None,
                blocked_scopes: Vec::new(),
            },
            domain::review_v2::ReviewApprovalVerdict::ApprovedWithBypass { not_started_count } => {
                ReviewApprovalOutput {
                    decision: ReviewApprovalDecision::ApprovedWithBypass,
                    bypass_scope_count: Some(not_started_count),
                    blocked_scopes: Vec::new(),
                }
            }
            domain::review_v2::ReviewApprovalVerdict::Blocked { required_scopes } => {
                ReviewApprovalOutput {
                    decision: ReviewApprovalDecision::Blocked,
                    bypass_scope_count: None,
                    blocked_scopes: required_scopes.iter().map(|s| s.to_string()).collect(),
                }
            }
        }
    }

    #[test]
    fn test_verdict_to_output_approved_maps_correctly() {
        let out = verdict_to_output(domain::review_v2::ReviewApprovalVerdict::Approved);
        assert_eq!(out.decision, ReviewApprovalDecision::Approved);
        assert!(out.bypass_scope_count.is_none());
        assert!(out.blocked_scopes.is_empty());
    }

    #[test]
    fn test_verdict_to_output_approved_with_bypass_maps_correctly() {
        let out = verdict_to_output(domain::review_v2::ReviewApprovalVerdict::ApprovedWithBypass {
            not_started_count: 2,
        });
        assert_eq!(out.decision, ReviewApprovalDecision::ApprovedWithBypass);
        assert_eq!(out.bypass_scope_count, Some(2));
        assert!(out.blocked_scopes.is_empty());
    }

    #[test]
    fn test_verdict_to_output_blocked_maps_correctly() {
        use domain::review_v2::{MainScopeName, ScopeName};
        let scope = ScopeName::Main(MainScopeName::new("domain").unwrap());
        let out = verdict_to_output(domain::review_v2::ReviewApprovalVerdict::Blocked {
            required_scopes: vec![scope],
        });
        assert_eq!(out.decision, ReviewApprovalDecision::Blocked);
        assert!(out.bypass_scope_count.is_none());
        assert_eq!(out.blocked_scopes, vec!["domain".to_owned()]);
    }

    #[test]
    fn test_review_check_approved_error_invalid_track_id_variants_exist() {
        let err = ReviewCheckApprovedError::InvalidTrackId("bad".to_owned());
        assert!(matches!(err, ReviewCheckApprovedError::InvalidTrackId(_)));
        let err2 = ReviewCheckApprovedError::ReviewStoreError("io".to_owned());
        assert!(matches!(err2, ReviewCheckApprovedError::ReviewStoreError(_)));
        let err3 = ReviewCheckApprovedError::EvaluationFailed("cycle".to_owned());
        assert!(matches!(err3, ReviewCheckApprovedError::EvaluationFailed(_)));
    }

    #[test]
    fn check_approved_valid_track_id_delegates_to_closure() {
        // A valid track ID must pass validation and be forwarded to the closure.
        let interactor = ReviewCheckApprovedInteractor::new(|_track_id, _items_dir| {
            Ok(ReviewApprovalOutput {
                decision: ReviewApprovalDecision::Approved,
                bypass_scope_count: None,
                blocked_scopes: Vec::new(),
            })
        });
        let result = interactor
            .check_approved("my-track-2026".to_owned(), std::path::PathBuf::new())
            .unwrap();
        assert_eq!(result.decision, ReviewApprovalDecision::Approved);
    }

    #[test]
    fn check_approved_invalid_track_id_returns_error_before_closure() {
        // An empty track ID must be rejected by the usecase boundary before the
        // closure is called. Verify the closure is never invoked via AtomicBool.
        use std::sync::Arc;
        use std::sync::atomic::{AtomicBool, Ordering};
        let called = Arc::new(AtomicBool::new(false));
        let called_inner = Arc::clone(&called);
        let interactor = ReviewCheckApprovedInteractor::new(move |_track_id, _items_dir| {
            called_inner.store(true, Ordering::SeqCst);
            Ok(ReviewApprovalOutput {
                decision: ReviewApprovalDecision::Approved,
                bypass_scope_count: None,
                blocked_scopes: Vec::new(),
            })
        });
        let err = interactor.check_approved(String::new(), std::path::PathBuf::new()).unwrap_err();
        assert!(
            matches!(err, ReviewCheckApprovedError::InvalidTrackId(_)),
            "expected InvalidTrackId, got: {err}"
        );
        assert!(!called.load(Ordering::SeqCst), "closure must not be called for invalid track IDs");
    }

    #[test]
    fn check_approved_closure_error_is_propagated() {
        // Errors returned by the closure must be propagated unchanged.
        let interactor = ReviewCheckApprovedInteractor::new(|_track_id, _items_dir| {
            Err(ReviewCheckApprovedError::ReviewStoreError("io error".to_owned()))
        });
        let err = interactor
            .check_approved("my-track-2026".to_owned(), std::path::PathBuf::new())
            .unwrap_err();
        assert!(
            matches!(err, ReviewCheckApprovedError::ReviewStoreError(_)),
            "expected ReviewStoreError, got: {err}"
        );
    }
}
