//! Orthogonal grounds classification for ADR decisions.

/// Orthogonal classification of the grounds for a single ADR decision.
///
/// This enum is used by `AdrSignalEvaluator` (T002) to express the signal
/// produced after inspecting an entry's
/// [`crate::adr_decision::AdrDecisionCommon`] fields. The classification is
/// independent of lifecycle state: every lifecycle typestate embeds
/// `AdrDecisionCommon`, so grounds can be evaluated uniformly.
///
/// Signal mapping:
///
/// | `DecisionGrounds`   | Signal | Meaning                                              |
/// |---------------------|--------|------------------------------------------------------|
/// | `UserDecisionRef`   | 🔵     | User explicitly approved the decision.               |
/// | `ReviewFindingRef`  | 🟡     | Decision emerged from the review process.            |
/// | `Grandfathered`     | _(skip)_ | Legacy decision exempt from signal check (D4).   |
/// | `NoGrounds`         | 🔴     | No trace; orchestrator independent decision.         |
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecisionGrounds {
    /// The decision has a `user_decision_ref` — user explicitly approved it (🔵).
    UserDecisionRef,
    /// The decision has a `review_finding_ref` (and no `user_decision_ref`) — it
    /// emerged from the review process (🟡).
    ReviewFindingRef,
    /// The decision has `grandfathered: true` — it is exempt from `verify-adr-signals`
    /// CI checks per the D4 gradual back-fill policy.
    Grandfathered,
    /// Neither `user_decision_ref` nor `review_finding_ref` is set and
    /// `grandfathered` is `false` — an untraced orchestrator decision (🔴).
    NoGrounds,
}

#[cfg(test)]
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
}
