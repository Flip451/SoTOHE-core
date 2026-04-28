//! Shared value object embedded in every lifecycle typestate.

use thiserror::Error;

/// Validation errors for [`AdrDecisionCommon`] fields and lifecycle typestate fields.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AdrDecisionCommonError {
    /// The decision `id` field must not be empty.
    #[error("decision id must not be empty")]
    EmptyId,
    /// The `implemented_in` field of [`crate::adr_decision::ImplementedDecision`] must not be
    /// empty. It must identify the commit or reference where the decision was actualized.
    #[error("implemented_in must not be empty")]
    EmptyImplementedIn,
    /// The `superseded_by` field of [`crate::adr_decision::SupersededDecision`] must not be
    /// empty. It must identify the ADR anchor reference of the superseding decision.
    #[error("superseded_by must not be empty")]
    EmptySupersededBy,
}

/// Common grounds and identity fields shared by every ADR decision lifecycle state.
///
/// Each of the five typestate structs (`ProposedDecision`, `AcceptedDecision`,
/// `ImplementedDecision`, `SupersededDecision`, `DeprecatedDecision`) embeds an
/// `AdrDecisionCommon` as its shared payload, so grounds trace fields are accessed
/// uniformly regardless of lifecycle state.
///
/// No serde derives — deserialization lives in the infrastructure adapter per CN-05.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdrDecisionCommon {
    /// Short identifier for this decision (e.g. `"D1"`, `"D2"`). Must not be empty.
    id: String,
    /// Reference to the user's explicit approval (e.g. a chat segment timestamp or
    /// approval marker). `Some` → blue signal.
    user_decision_ref: Option<String>,
    /// Reference to the review-process finding that produced this decision.
    /// `Some` → yellow signal (if `user_decision_ref` is `None`).
    review_finding_ref: Option<String>,
    /// Candidate selection note (e.g. `"chose option A over B and C"`).
    candidate_selection: Option<String>,
    /// When `true`, the `verify-adr-signals` check skips this decision (D4 exemption).
    grandfathered: bool,
}

impl AdrDecisionCommon {
    /// Create a validated [`AdrDecisionCommon`].
    ///
    /// # Errors
    ///
    /// Returns [`AdrDecisionCommonError::EmptyId`] when `id` is empty.
    pub fn new(
        id: impl Into<String>,
        user_decision_ref: Option<String>,
        review_finding_ref: Option<String>,
        candidate_selection: Option<String>,
        grandfathered: bool,
    ) -> Result<Self, AdrDecisionCommonError> {
        let id = id.into();
        if id.is_empty() {
            return Err(AdrDecisionCommonError::EmptyId);
        }
        Ok(Self { id, user_decision_ref, review_finding_ref, candidate_selection, grandfathered })
    }

    /// The decision identifier (e.g. `"D1"`).
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Reference to the user's explicit approval of this decision, if any.
    pub fn user_decision_ref(&self) -> Option<&str> {
        self.user_decision_ref.as_deref()
    }

    /// Reference to the review-process finding that produced this decision, if any.
    pub fn review_finding_ref(&self) -> Option<&str> {
        self.review_finding_ref.as_deref()
    }

    /// Candidate selection note, if any.
    pub fn candidate_selection(&self) -> Option<&str> {
        self.candidate_selection.as_deref()
    }

    /// Whether this decision is exempt from `verify-adr-signals` CI checks.
    pub fn grandfathered(&self) -> bool {
        self.grandfathered
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_adr_decision_common_with_valid_id_succeeds() {
        let common = AdrDecisionCommon::new("D1", None, None, None, false).unwrap();
        assert_eq!(common.id(), "D1");
        assert_eq!(common.user_decision_ref(), None);
        assert_eq!(common.review_finding_ref(), None);
        assert_eq!(common.candidate_selection(), None);
        assert!(!common.grandfathered());
    }

    #[test]
    fn test_adr_decision_common_with_empty_id_returns_error() {
        let result = AdrDecisionCommon::new("", None, None, None, false);
        assert!(matches!(result, Err(AdrDecisionCommonError::EmptyId)));
    }

    #[test]
    fn test_adr_decision_common_with_all_fields_populated_succeeds() {
        let common = AdrDecisionCommon::new(
            "D2",
            Some("chat_segment:2026-04-25T03:50:00Z".to_string()),
            Some("review_finding:RF-42".to_string()),
            Some("chose option A".to_string()),
            true,
        )
        .unwrap();
        assert_eq!(common.id(), "D2");
        assert_eq!(common.user_decision_ref(), Some("chat_segment:2026-04-25T03:50:00Z"));
        assert_eq!(common.review_finding_ref(), Some("review_finding:RF-42"));
        assert_eq!(common.candidate_selection(), Some("chose option A"));
        assert!(common.grandfathered());
    }
}
