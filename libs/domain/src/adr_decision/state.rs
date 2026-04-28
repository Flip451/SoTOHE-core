//! Lifecycle typestate structs for ADR decisions.
//!
//! Each struct is an independent Rust type (not a single generic parameterised by
//! phantom type) per ADR D2 / impl-plan T001 requirements. Illegal transitions are
//! ruled out at compile time: `ImplementedDecision` carries `implemented_in` only
//! in this state, `SupersededDecision` carries `superseded_by` only in this state,
//! and terminal states (`SupersededDecision`, `DeprecatedDecision`) expose no
//! transition methods at all.
//!
//! No serde derives — deserialization lives in the infrastructure adapter (CN-05).

use super::common::{AdrDecisionCommon, AdrDecisionCommonError};

// ── ProposedDecision ──────────────────────────────────────────────────────────

/// A decision that is newly drafted and awaiting review.
///
/// Carries only [`AdrDecisionCommon`] (id, grounds trace fields, grandfathered
/// flag). No implementation or supersession reference exists yet — the proposal is
/// open.
///
/// Outgoing transitions:
/// - [`ProposedDecision::accept`] → [`AcceptedDecision`]
/// - [`ProposedDecision::deprecate`] → [`DeprecatedDecision`]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProposedDecision {
    /// Shared grounds and identity fields.
    pub common: AdrDecisionCommon,
}

impl ProposedDecision {
    /// Create a new [`ProposedDecision`] wrapping the given `common` payload.
    pub fn new(common: AdrDecisionCommon) -> Self {
        Self { common }
    }

    /// Advance to [`AcceptedDecision`] once review is complete.
    pub fn accept(self) -> AcceptedDecision {
        AcceptedDecision { common: self.common }
    }

    /// Retire this decision before acceptance (withdrawn / abandoned).
    pub fn deprecate(self) -> DeprecatedDecision {
        DeprecatedDecision { common: self.common }
    }
}

// ── AcceptedDecision ──────────────────────────────────────────────────────────

/// A decision that has completed review and is ready for implementation.
///
/// Carries only [`AdrDecisionCommon`]. Acceptance ratifies the decision but
/// implementation has not started yet.
///
/// Outgoing transitions:
/// - [`AcceptedDecision::implement`] → [`ImplementedDecision`]
/// - [`AcceptedDecision::supersede`] → [`SupersededDecision`]
/// - [`AcceptedDecision::deprecate`] → [`DeprecatedDecision`]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptedDecision {
    /// Shared grounds and identity fields.
    pub common: AdrDecisionCommon,
}

impl AcceptedDecision {
    /// Create a new [`AcceptedDecision`] wrapping the given `common` payload.
    pub fn new(common: AdrDecisionCommon) -> Self {
        Self { common }
    }

    /// Advance to [`ImplementedDecision`] once the decision has been actualized.
    ///
    /// `implemented_in` is the commit hash or reference where the decision was
    /// applied (e.g. `"abc1234"` or `"track/my-feature@0c0f24c"`). Must not be empty.
    ///
    /// # Errors
    ///
    /// Returns [`AdrDecisionCommonError::EmptyImplementedIn`] when `implemented_in` is empty.
    pub fn implement(
        self,
        implemented_in: String,
    ) -> Result<ImplementedDecision, AdrDecisionCommonError> {
        if implemented_in.is_empty() {
            return Err(AdrDecisionCommonError::EmptyImplementedIn);
        }
        Ok(ImplementedDecision { common: self.common, implemented_in })
    }

    /// Advance to [`SupersededDecision`] when a later decision replaces this one
    /// before implementation.
    ///
    /// `superseded_by` is the ADR anchor reference identifying the superseding
    /// decision (e.g. `"2026-04-28-0001-some-adr.md#D2"`). Must not be empty.
    ///
    /// # Errors
    ///
    /// Returns [`AdrDecisionCommonError::EmptySupersededBy`] when `superseded_by` is empty.
    pub fn supersede(
        self,
        superseded_by: String,
    ) -> Result<SupersededDecision, AdrDecisionCommonError> {
        if superseded_by.is_empty() {
            return Err(AdrDecisionCommonError::EmptySupersededBy);
        }
        Ok(SupersededDecision { common: self.common, superseded_by })
    }

    /// Retire this decision after acceptance (withdrawn after ratification).
    pub fn deprecate(self) -> DeprecatedDecision {
        DeprecatedDecision { common: self.common }
    }
}

// ── ImplementedDecision ───────────────────────────────────────────────────────

/// A decision that has been fully implemented.
///
/// Carries [`AdrDecisionCommon`] plus `implemented_in: String` — the commit hash
/// or reference where the decision was actualized. This field is structurally
/// present **only** in this state; it cannot appear in `ProposedDecision` or
/// `AcceptedDecision`, eliminating any need for `Option<implemented_in>` elsewhere.
///
/// Outgoing transitions:
/// - [`ImplementedDecision::supersede`] → [`SupersededDecision`]
/// - [`ImplementedDecision::deprecate`] → [`DeprecatedDecision`]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImplementedDecision {
    /// Shared grounds and identity fields.
    pub common: AdrDecisionCommon,
    /// Commit hash or reference where this decision was actualized.
    /// Private to preserve the invariant that this field is always non-empty;
    /// access via [`ImplementedDecision::implemented_in`].
    implemented_in: String,
}

impl ImplementedDecision {
    /// Create a new [`ImplementedDecision`].
    ///
    /// `implemented_in` must identify the commit or reference that actualized the
    /// decision (e.g. `"abc1234"`). Must not be empty.
    ///
    /// # Errors
    ///
    /// Returns [`AdrDecisionCommonError::EmptyImplementedIn`] when `implemented_in` is empty.
    pub fn new(
        common: AdrDecisionCommon,
        implemented_in: String,
    ) -> Result<Self, AdrDecisionCommonError> {
        if implemented_in.is_empty() {
            return Err(AdrDecisionCommonError::EmptyImplementedIn);
        }
        Ok(Self { common, implemented_in })
    }

    /// Advance to [`SupersededDecision`] when the implementation is later replaced.
    ///
    /// `superseded_by` is the ADR anchor reference identifying the superseding
    /// decision. Must not be empty.
    ///
    /// # Errors
    ///
    /// Returns [`AdrDecisionCommonError::EmptySupersededBy`] when `superseded_by` is empty.
    pub fn supersede(
        self,
        superseded_by: String,
    ) -> Result<SupersededDecision, AdrDecisionCommonError> {
        if superseded_by.is_empty() {
            return Err(AdrDecisionCommonError::EmptySupersededBy);
        }
        Ok(SupersededDecision { common: self.common, superseded_by })
    }

    /// The commit hash or reference where this decision was actualized.
    pub fn implemented_in(&self) -> &str {
        &self.implemented_in
    }

    /// Retire the implementation (implementation retired without replacement).
    pub fn deprecate(self) -> DeprecatedDecision {
        DeprecatedDecision { common: self.common }
    }
}

// ── SupersededDecision ────────────────────────────────────────────────────────

/// A decision that has been replaced by a later decision in a subsequent ADR.
///
/// Carries [`AdrDecisionCommon`] plus `superseded_by: String` — the ADR anchor
/// reference (e.g. `"2026-04-28-0001-some-adr.md#D2"`) identifying the superseding
/// decision. This field is structurally present **only** in this state.
///
/// **Terminal state**: no outgoing transitions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupersededDecision {
    /// Shared grounds and identity fields.
    pub common: AdrDecisionCommon,
    /// ADR anchor reference of the superseding decision.
    /// Private to preserve the invariant that this field is always non-empty;
    /// access via [`SupersededDecision::superseded_by`].
    superseded_by: String,
}

impl SupersededDecision {
    /// Create a new [`SupersededDecision`].
    ///
    /// `superseded_by` must be a non-empty ADR anchor reference (e.g.
    /// `"2026-04-28-0001-some-adr.md#D2"`).
    ///
    /// # Errors
    ///
    /// Returns [`AdrDecisionCommonError::EmptySupersededBy`] when `superseded_by` is empty.
    pub fn new(
        common: AdrDecisionCommon,
        superseded_by: String,
    ) -> Result<Self, AdrDecisionCommonError> {
        if superseded_by.is_empty() {
            return Err(AdrDecisionCommonError::EmptySupersededBy);
        }
        Ok(Self { common, superseded_by })
    }

    /// The ADR anchor reference of the superseding decision.
    pub fn superseded_by(&self) -> &str {
        &self.superseded_by
    }
}

// ── DeprecatedDecision ────────────────────────────────────────────────────────

/// A decision that has been retired without replacement.
///
/// Carries only [`AdrDecisionCommon`]. Deprecation signals voluntary retirement
/// rather than replacement; the structural distinction from [`SupersededDecision`]
/// makes it impossible to confuse the two end states.
///
/// **Terminal state**: no outgoing transitions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeprecatedDecision {
    /// Shared grounds and identity fields.
    pub common: AdrDecisionCommon,
}

impl DeprecatedDecision {
    /// Create a new [`DeprecatedDecision`] wrapping the given `common` payload.
    pub fn new(common: AdrDecisionCommon) -> Self {
        Self { common }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::adr_decision::common::AdrDecisionCommon;

    fn common(id: &str) -> AdrDecisionCommon {
        AdrDecisionCommon::new(id, None, None, None, false).unwrap()
    }

    fn common_with_user_ref(id: &str, uref: &str) -> AdrDecisionCommon {
        AdrDecisionCommon::new(id, Some(uref.to_string()), None, None, false).unwrap()
    }

    // ── Constructor happy paths ───────────────────────────────────────────────

    #[test]
    fn test_proposed_decision_with_valid_common_succeeds() {
        let decision = ProposedDecision::new(common("D1"));
        assert_eq!(decision.common.id(), "D1");
    }

    #[test]
    fn test_accepted_decision_with_valid_common_succeeds() {
        let decision = AcceptedDecision::new(common("D2"));
        assert_eq!(decision.common.id(), "D2");
    }

    #[test]
    fn test_implemented_decision_with_valid_common_and_ref_succeeds() {
        let decision = ImplementedDecision::new(common("D3"), "abc1234".to_string()).unwrap();
        assert_eq!(decision.common.id(), "D3");
        assert_eq!(decision.implemented_in(), "abc1234");
    }

    #[test]
    fn test_implemented_decision_with_empty_ref_returns_error() {
        let result = ImplementedDecision::new(common("D3"), String::new());
        assert!(matches!(result, Err(AdrDecisionCommonError::EmptyImplementedIn)));
    }

    #[test]
    fn test_superseded_decision_with_valid_common_and_ref_succeeds() {
        let decision =
            SupersededDecision::new(common("D4"), "2026-05-01-0001-new-adr.md#D1".to_string())
                .unwrap();
        assert_eq!(decision.common.id(), "D4");
        assert_eq!(decision.superseded_by(), "2026-05-01-0001-new-adr.md#D1");
    }

    #[test]
    fn test_superseded_decision_with_empty_ref_returns_error() {
        let result = SupersededDecision::new(common("D4"), String::new());
        assert!(matches!(result, Err(AdrDecisionCommonError::EmptySupersededBy)));
    }

    #[test]
    fn test_deprecated_decision_with_valid_common_succeeds() {
        let decision = DeprecatedDecision::new(common("D5"));
        assert_eq!(decision.common.id(), "D5");
    }

    // ── Transition methods ────────────────────────────────────────────────────

    #[test]
    fn test_proposed_accept_transitions_to_accepted_decision() {
        let proposed = ProposedDecision::new(common_with_user_ref("D1", "chat:2026-04-25"));
        let accepted = proposed.accept();
        assert_eq!(accepted.common.id(), "D1");
        assert_eq!(accepted.common.user_decision_ref(), Some("chat:2026-04-25"));
    }

    #[test]
    fn test_proposed_deprecate_transitions_to_deprecated_decision() {
        let proposed = ProposedDecision::new(common("D1"));
        let deprecated = proposed.deprecate();
        assert_eq!(deprecated.common.id(), "D1");
    }

    #[test]
    fn test_accepted_implement_transitions_to_implemented_decision_with_ref() {
        let accepted = AcceptedDecision::new(common("D2"));
        let implemented = accepted.implement("abc1234".to_string()).unwrap();
        assert_eq!(implemented.common.id(), "D2");
        assert_eq!(implemented.implemented_in(), "abc1234");
    }

    #[test]
    fn test_accepted_implement_with_empty_ref_returns_error() {
        let accepted = AcceptedDecision::new(common("D2"));
        let result = accepted.implement(String::new());
        assert!(matches!(result, Err(AdrDecisionCommonError::EmptyImplementedIn)));
    }

    #[test]
    fn test_accepted_supersede_transitions_to_superseded_decision_with_ref() {
        let accepted = AcceptedDecision::new(common("D2"));
        let superseded = accepted.supersede("2026-05-01-other.md#D3".to_string()).unwrap();
        assert_eq!(superseded.common.id(), "D2");
        assert_eq!(superseded.superseded_by(), "2026-05-01-other.md#D3");
    }

    #[test]
    fn test_accepted_supersede_with_empty_ref_returns_error() {
        let accepted = AcceptedDecision::new(common("D2"));
        let result = accepted.supersede(String::new());
        assert!(matches!(result, Err(AdrDecisionCommonError::EmptySupersededBy)));
    }

    #[test]
    fn test_accepted_deprecate_transitions_to_deprecated_decision() {
        let accepted = AcceptedDecision::new(common("D2"));
        let deprecated = accepted.deprecate();
        assert_eq!(deprecated.common.id(), "D2");
    }

    #[test]
    fn test_implemented_supersede_transitions_to_superseded_decision_with_ref() {
        let implemented = ImplementedDecision::new(common("D3"), "deadbeef".to_string()).unwrap();
        let superseded = implemented.supersede("2026-05-02-other.md#D1".to_string()).unwrap();
        assert_eq!(superseded.common.id(), "D3");
        assert_eq!(superseded.superseded_by(), "2026-05-02-other.md#D1");
    }

    #[test]
    fn test_implemented_supersede_with_empty_ref_returns_error() {
        let implemented = ImplementedDecision::new(common("D3"), "deadbeef".to_string()).unwrap();
        let result = implemented.supersede(String::new());
        assert!(matches!(result, Err(AdrDecisionCommonError::EmptySupersededBy)));
    }

    #[test]
    fn test_implemented_deprecate_transitions_to_deprecated_decision() {
        let implemented = ImplementedDecision::new(common("D3"), "deadbeef".to_string()).unwrap();
        let deprecated = implemented.deprecate();
        assert_eq!(deprecated.common.id(), "D3");
    }
}
