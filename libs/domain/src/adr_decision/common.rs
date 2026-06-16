//! Shared value object embedded in every lifecycle typestate.

use thiserror::Error;

use super::grounds::DecisionGroundRef;

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
/// The `user_decision_ref` and `review_finding_ref` fields use the
/// [`DecisionGroundRef`] validated newtype rather than raw `String` so the
/// non-empty-and-non-whitespace invariant is enforced by the type — empty
/// placeholders cannot satisfy `is_some()` and silently drive the signal.
///
/// No serde derives — deserialization lives in the infrastructure adapter per CN-05.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdrDecisionCommon {
    /// Short identifier for this decision (e.g. `"D1"`, `"D2"`). Must not be empty.
    id: String,
    /// Reference to the user's explicit approval (e.g. a chat segment timestamp or
    /// approval marker). `Some` → blue signal (when no `review_finding_ref` remains).
    user_decision_ref: Option<DecisionGroundRef>,
    /// Reference to the review-process finding that produced this decision.
    /// `Some` → yellow signal (regardless of whether `user_decision_ref` is also set,
    /// per the review-priority rule of
    /// `knowledge/adr/2026-06-16-0042-adr-signal-review-grounding-precedence.md` §D1).
    review_finding_ref: Option<DecisionGroundRef>,
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
        user_decision_ref: Option<DecisionGroundRef>,
        review_finding_ref: Option<DecisionGroundRef>,
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
    pub fn user_decision_ref(&self) -> Option<&DecisionGroundRef> {
        self.user_decision_ref.as_ref()
    }

    /// Reference to the review-process finding that produced this decision, if any.
    pub fn review_finding_ref(&self) -> Option<&DecisionGroundRef> {
        self.review_finding_ref.as_ref()
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
            Some(DecisionGroundRef::try_new("chat_segment:2026-04-25T03:50:00Z").unwrap()),
            Some(DecisionGroundRef::try_new("review_finding:RF-42").unwrap()),
            Some("chose option A".to_string()),
            true,
        )
        .unwrap();
        assert_eq!(common.id(), "D2");
        assert_eq!(
            common.user_decision_ref().map(DecisionGroundRef::as_str),
            Some("chat_segment:2026-04-25T03:50:00Z")
        );
        assert_eq!(
            common.review_finding_ref().map(DecisionGroundRef::as_str),
            Some("review_finding:RF-42")
        );
        assert_eq!(common.candidate_selection(), Some("chose option A"));
        assert!(common.grandfathered());
    }
}
