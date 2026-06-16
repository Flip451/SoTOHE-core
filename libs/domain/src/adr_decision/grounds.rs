//! Validated grounding reference newtype and orthogonal grounds classification
//! for ADR decisions.

use crate::ValidationError;

/// Validated newtype for a grounding-reference string carried by an ADR decision
/// (`user_decision_ref` or `review_finding_ref` in the ADR YAML front-matter).
///
/// Construction goes through [`DecisionGroundRef::try_new`], which rejects empty
/// strings and whitespace-only strings as
/// [`ValidationError::EmptyDecisionGroundRef`]. A non-empty, non-whitespace
/// invariant is therefore guaranteed by the type — empty placeholders cannot
/// silently satisfy `is_some()` on the embedding `Option<DecisionGroundRef>` and
/// thus cannot falsely drive the ADR-decision grounds signal.
///
/// A single shared type is used for both `user_decision_ref` and
/// `review_finding_ref` because their validation rule is identical — this
/// mirrors how [`crate::AdrAnchor`] is shared across the `AdrRef` and
/// `ConventionRef` reference structs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionGroundRef(String);

impl DecisionGroundRef {
    /// Validate and wrap `value` as a [`DecisionGroundRef`].
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::EmptyDecisionGroundRef`] when `value` is empty
    /// or contains only whitespace.
    pub fn try_new(value: impl Into<String>) -> Result<Self, ValidationError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(ValidationError::EmptyDecisionGroundRef);
        }
        Ok(Self(value))
    }

    /// Borrow the inner string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Orthogonal classification of the grounds for a single ADR decision.
///
/// Produced by [`crate::adr_decision::evaluate_adr_decision`] after inspecting
/// the embedded [`crate::adr_decision::AdrDecisionCommon`] fields. The
/// classification is independent of lifecycle state: every lifecycle typestate
/// embeds `AdrDecisionCommon`, so grounds can be evaluated uniformly.
///
/// Signal mapping (review-priority rule, see
/// `knowledge/adr/2026-06-16-0042-adr-signal-review-grounding-precedence.md`
/// §D1 — supersedes the original 2026-04-27-1234 §D1 user-priority table):
///
/// | `DecisionGrounds`   | Signal | Meaning                                                                                       |
/// |---------------------|--------|-----------------------------------------------------------------------------------------------|
/// | `Grandfathered`     | _(skip)_ | Legacy decision exempt from signal check (D4 backfill policy).                              |
/// | `ReviewFindingRef`  | 🟡     | A `review_finding_ref` is present (regardless of whether a `user_decision_ref` also is).      |
/// | `UserDecisionRef`   | 🔵     | A `user_decision_ref` is present **and** no `review_finding_ref` remains to escalate.         |
/// | `NoGrounds`         | 🔴     | No trace: neither ref present, `grandfathered` is `false`.                                    |
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecisionGrounds {
    /// The decision has a `user_decision_ref` and no `review_finding_ref` — the
    /// user explicitly approved it without an unresolved review-derived ground
    /// remaining (🔵).
    UserDecisionRef,
    /// The decision has a `review_finding_ref` — a review-derived grounding is
    /// present, so the signal stays at 🟡 regardless of whether a
    /// `user_decision_ref` is also present (the unresolved review-derived
    /// grounding takes precedence so the decision is not silently promoted to
    /// 🔵).
    ReviewFindingRef,
    /// The decision has `grandfathered: true` — it is exempt from `verify-adr-signals`
    /// CI checks per the D4 gradual back-fill policy.
    Grandfathered,
    /// Neither `user_decision_ref` nor `review_finding_ref` is set and
    /// `grandfathered` is `false` — an untraced orchestrator decision (🔴).
    NoGrounds,
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_decision_grounds_user_decision_ref_variant_constructs() {
        let g = DecisionGrounds::UserDecisionRef;
        assert_eq!(g, DecisionGrounds::UserDecisionRef);
    }

    #[test]
    fn test_decision_grounds_review_finding_ref_variant_constructs() {
        let g = DecisionGrounds::ReviewFindingRef;
        assert_eq!(g, DecisionGrounds::ReviewFindingRef);
    }

    #[test]
    fn test_decision_grounds_grandfathered_variant_constructs() {
        let g = DecisionGrounds::Grandfathered;
        assert_eq!(g, DecisionGrounds::Grandfathered);
    }

    #[test]
    fn test_decision_grounds_no_grounds_variant_constructs() {
        let g = DecisionGrounds::NoGrounds;
        assert_eq!(g, DecisionGrounds::NoGrounds);
    }

    #[test]
    fn test_decision_ground_ref_accepts_non_empty_value() {
        let r = DecisionGroundRef::try_new("chat:2026-04-25").unwrap();
        assert_eq!(r.as_str(), "chat:2026-04-25");
    }

    #[test]
    fn test_decision_ground_ref_rejects_empty_string() {
        let err = DecisionGroundRef::try_new("").unwrap_err();
        assert!(matches!(err, ValidationError::EmptyDecisionGroundRef));
    }

    #[test]
    fn test_decision_ground_ref_rejects_whitespace_only_string() {
        let err = DecisionGroundRef::try_new("   \t  ").unwrap_err();
        assert!(matches!(err, ValidationError::EmptyDecisionGroundRef));
    }

    #[test]
    fn test_decision_ground_ref_preserves_internal_whitespace() {
        let r = DecisionGroundRef::try_new("review finding RF-12 (PR #142)").unwrap();
        assert_eq!(r.as_str(), "review finding RF-12 (PR #142)");
    }
}
