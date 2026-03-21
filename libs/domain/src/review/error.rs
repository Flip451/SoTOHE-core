//! Review error types.

use thiserror::Error;

use super::types::ReviewStatus;

/// Errors from review state operations.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ReviewError {
    #[error("final round requires review status fast_passed, but current status is {0}")]
    FinalRequiresFastPassed(ReviewStatus),

    #[error("code hash mismatch: review recorded against {expected}, but current code is {actual}")]
    StaleCodeHash { expected: String, actual: String },

    #[error("review status is {0}, not approved")]
    NotApproved(ReviewStatus),

    #[error("invalid concern: {0}")]
    InvalidConcern(String),

    #[error("review escalation is active for concerns: {concerns:?}")]
    EscalationActive { concerns: Vec<String> },

    #[error("review escalation is not active")]
    EscalationNotActive,

    #[error("resolution evidence is required: {0}")]
    ResolutionEvidenceMissing(&'static str),

    #[error("resolution concerns do not match blocked concerns")]
    ResolutionConcernMismatch { expected: Vec<String>, actual: Vec<String> },
}
