//! Pure domain free function classifying an ADR decision into a signal grounds.

use super::common::AdrDecisionCommon;
use super::entry::AdrDecisionEntry;
use super::grounds::DecisionGrounds;

/// Classify a single [`AdrDecisionEntry`] into a [`DecisionGrounds`] signal.
///
/// Pattern-matches on the lifecycle variant of `entry` and inspects the
/// embedded [`AdrDecisionCommon`] grounds fields to produce the orthogonal
/// classification. All five lifecycle variants carry `AdrDecisionCommon`, so
/// classification is uniform.
///
/// Priority (highest first), per the review-priority rule of
/// `knowledge/adr/2026-06-16-0042-adr-signal-review-grounding-precedence.md`
/// §D1 (supersedes 2026-04-27-1234 §D1):
///
/// 1. `grandfathered: true` → [`DecisionGrounds::Grandfathered`] (D4 exemption,
///    skipped by `verify-adr-signals`).
/// 2. `review_finding_ref: Some(_)` → [`DecisionGrounds::ReviewFindingRef`] (🟡)
///    — checked **before** `user_decision_ref` so any decision still carrying
///    an unresolved review-derived grounding stays at 🟡 regardless of whether
///    a `user_decision_ref` is also present.
/// 3. `user_decision_ref: Some(_)` and no `review_finding_ref` →
///    [`DecisionGrounds::UserDecisionRef`] (🔵).
/// 4. otherwise → [`DecisionGrounds::NoGrounds`] (🔴).
///
/// Infallible: every reachable [`AdrDecisionEntry`] is well-formed because the
/// infrastructure codec validates at the deserialization boundary (T003).
#[must_use]
pub fn evaluate_adr_decision(entry: AdrDecisionEntry) -> DecisionGrounds {
    let common = match &entry {
        AdrDecisionEntry::ProposedDecision(d) => &d.common,
        AdrDecisionEntry::AcceptedDecision(d) => &d.common,
        AdrDecisionEntry::ImplementedDecision(d) => &d.common,
        AdrDecisionEntry::SupersededDecision(d) => &d.common,
        AdrDecisionEntry::DeprecatedDecision(d) => &d.common,
    };
    classify_grounds(common)
}

fn classify_grounds(common: &AdrDecisionCommon) -> DecisionGrounds {
    if common.grandfathered() {
        return DecisionGrounds::Grandfathered;
    }
    if common.review_finding_ref().is_some() {
        return DecisionGrounds::ReviewFindingRef;
    }
    if common.user_decision_ref().is_some() {
        return DecisionGrounds::UserDecisionRef;
    }
    DecisionGrounds::NoGrounds
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::adr_decision::{
        AcceptedDecision, AdrDecisionCommon, DecisionGroundRef, DeprecatedDecision,
        ImplementedDecision, ProposedDecision, SupersededDecision,
    };

    fn ref_from(value: Option<&str>) -> Option<DecisionGroundRef> {
        value.map(|s| DecisionGroundRef::try_new(s).unwrap())
    }

    fn common_with(
        id: &str,
        user_ref: Option<&str>,
        review_ref: Option<&str>,
        grandfathered: bool,
    ) -> AdrDecisionCommon {
        AdrDecisionCommon::new(id, ref_from(user_ref), ref_from(review_ref), None, grandfathered)
            .unwrap()
    }

    #[test]
    fn test_evaluate_adr_decision_proposed_with_grandfathered_returns_grandfathered() {
        let entry = AdrDecisionEntry::ProposedDecision(ProposedDecision::new(common_with(
            "D1", None, None, true,
        )));
        assert_eq!(evaluate_adr_decision(entry), DecisionGrounds::Grandfathered);
    }

    #[test]
    fn test_evaluate_adr_decision_accepted_with_user_ref_returns_user_decision_ref() {
        let entry = AdrDecisionEntry::AcceptedDecision(AcceptedDecision::new(common_with(
            "D2",
            Some("chat:2026-04-25"),
            None,
            false,
        )));
        assert_eq!(evaluate_adr_decision(entry), DecisionGrounds::UserDecisionRef);
    }

    #[test]
    fn test_evaluate_adr_decision_implemented_with_review_ref_returns_review_finding_ref() {
        let entry = AdrDecisionEntry::ImplementedDecision(
            ImplementedDecision::new(
                common_with("D3", None, Some("RF-12"), false),
                "abc123".to_string(),
            )
            .unwrap(),
        );
        assert_eq!(evaluate_adr_decision(entry), DecisionGrounds::ReviewFindingRef);
    }

    #[test]
    fn test_evaluate_adr_decision_superseded_with_no_grounds_returns_no_grounds() {
        let entry = AdrDecisionEntry::SupersededDecision(
            SupersededDecision::new(
                common_with("D4", None, None, false),
                "knowledge/adr/foo.md#D7".to_string(),
            )
            .unwrap(),
        );
        assert_eq!(evaluate_adr_decision(entry), DecisionGrounds::NoGrounds);
    }

    #[test]
    fn test_evaluate_adr_decision_deprecated_with_user_ref_returns_user_decision_ref() {
        let entry = AdrDecisionEntry::DeprecatedDecision(DeprecatedDecision::new(common_with(
            "D5",
            Some("chat:2026-04-26"),
            None,
            false,
        )));
        assert_eq!(evaluate_adr_decision(entry), DecisionGrounds::UserDecisionRef);
    }

    #[test]
    fn test_evaluate_adr_decision_grandfathered_takes_priority_over_user_ref() {
        let entry = AdrDecisionEntry::ProposedDecision(ProposedDecision::new(common_with(
            "D6",
            Some("chat"),
            Some("RF"),
            true,
        )));
        assert_eq!(evaluate_adr_decision(entry), DecisionGrounds::Grandfathered);
    }

    #[test]
    fn test_evaluate_adr_decision_review_ref_takes_priority_over_user_ref() {
        // Review-priority rule (ADR 2026-06-16-0042 §D1, supersedes 2026-04-27-1234 §D1):
        // when both refs are present, the decision still carries an unresolved
        // review-derived grounding and must stay 🟡 rather than be silently
        // promoted to 🔵 by the user_decision_ref.
        let entry = AdrDecisionEntry::AcceptedDecision(AcceptedDecision::new(common_with(
            "D7",
            Some("chat"),
            Some("RF"),
            false,
        )));
        assert_eq!(evaluate_adr_decision(entry), DecisionGrounds::ReviewFindingRef);
    }

    // Full 5 typestate × 4 grounds matrix
    // Proposed: remaining 3 grounds (grandfathered tested above)
    #[test]
    fn test_evaluate_adr_decision_proposed_with_user_ref_returns_user_decision_ref() {
        let entry = AdrDecisionEntry::ProposedDecision(ProposedDecision::new(common_with(
            "D8",
            Some("chat:2026-04-25"),
            None,
            false,
        )));
        assert_eq!(evaluate_adr_decision(entry), DecisionGrounds::UserDecisionRef);
    }

    #[test]
    fn test_evaluate_adr_decision_proposed_with_review_ref_returns_review_finding_ref() {
        let entry = AdrDecisionEntry::ProposedDecision(ProposedDecision::new(common_with(
            "D9",
            None,
            Some("RF-01"),
            false,
        )));
        assert_eq!(evaluate_adr_decision(entry), DecisionGrounds::ReviewFindingRef);
    }

    #[test]
    fn test_evaluate_adr_decision_proposed_with_no_grounds_returns_no_grounds() {
        let entry = AdrDecisionEntry::ProposedDecision(ProposedDecision::new(common_with(
            "D10", None, None, false,
        )));
        assert_eq!(evaluate_adr_decision(entry), DecisionGrounds::NoGrounds);
    }

    // Accepted: remaining 3 grounds (user_ref tested above)
    #[test]
    fn test_evaluate_adr_decision_accepted_with_grandfathered_returns_grandfathered() {
        let entry = AdrDecisionEntry::AcceptedDecision(AcceptedDecision::new(common_with(
            "D11", None, None, true,
        )));
        assert_eq!(evaluate_adr_decision(entry), DecisionGrounds::Grandfathered);
    }

    #[test]
    fn test_evaluate_adr_decision_accepted_with_review_ref_returns_review_finding_ref() {
        let entry = AdrDecisionEntry::AcceptedDecision(AcceptedDecision::new(common_with(
            "D12",
            None,
            Some("RF-02"),
            false,
        )));
        assert_eq!(evaluate_adr_decision(entry), DecisionGrounds::ReviewFindingRef);
    }

    #[test]
    fn test_evaluate_adr_decision_accepted_with_no_grounds_returns_no_grounds() {
        let entry = AdrDecisionEntry::AcceptedDecision(AcceptedDecision::new(common_with(
            "D13", None, None, false,
        )));
        assert_eq!(evaluate_adr_decision(entry), DecisionGrounds::NoGrounds);
    }

    // Implemented: remaining 3 grounds (review_ref tested above)
    #[test]
    fn test_evaluate_adr_decision_implemented_with_grandfathered_returns_grandfathered() {
        let entry = AdrDecisionEntry::ImplementedDecision(
            ImplementedDecision::new(common_with("D14", None, None, true), "abc1234".to_string())
                .unwrap(),
        );
        assert_eq!(evaluate_adr_decision(entry), DecisionGrounds::Grandfathered);
    }

    #[test]
    fn test_evaluate_adr_decision_implemented_with_user_ref_returns_user_decision_ref() {
        let entry = AdrDecisionEntry::ImplementedDecision(
            ImplementedDecision::new(
                common_with("D15", Some("chat:2026-04-25"), None, false),
                "abc1234".to_string(),
            )
            .unwrap(),
        );
        assert_eq!(evaluate_adr_decision(entry), DecisionGrounds::UserDecisionRef);
    }

    #[test]
    fn test_evaluate_adr_decision_implemented_with_no_grounds_returns_no_grounds() {
        let entry = AdrDecisionEntry::ImplementedDecision(
            ImplementedDecision::new(common_with("D16", None, None, false), "abc1234".to_string())
                .unwrap(),
        );
        assert_eq!(evaluate_adr_decision(entry), DecisionGrounds::NoGrounds);
    }

    // Superseded: remaining 3 grounds (no_grounds tested above)
    #[test]
    fn test_evaluate_adr_decision_superseded_with_grandfathered_returns_grandfathered() {
        let entry = AdrDecisionEntry::SupersededDecision(
            SupersededDecision::new(
                common_with("D17", None, None, true),
                "knowledge/adr/foo.md#D7".to_string(),
            )
            .unwrap(),
        );
        assert_eq!(evaluate_adr_decision(entry), DecisionGrounds::Grandfathered);
    }

    #[test]
    fn test_evaluate_adr_decision_superseded_with_user_ref_returns_user_decision_ref() {
        let entry = AdrDecisionEntry::SupersededDecision(
            SupersededDecision::new(
                common_with("D18", Some("chat:2026-04-25"), None, false),
                "knowledge/adr/foo.md#D7".to_string(),
            )
            .unwrap(),
        );
        assert_eq!(evaluate_adr_decision(entry), DecisionGrounds::UserDecisionRef);
    }

    #[test]
    fn test_evaluate_adr_decision_superseded_with_review_ref_returns_review_finding_ref() {
        let entry = AdrDecisionEntry::SupersededDecision(
            SupersededDecision::new(
                common_with("D19", None, Some("RF-03"), false),
                "knowledge/adr/foo.md#D7".to_string(),
            )
            .unwrap(),
        );
        assert_eq!(evaluate_adr_decision(entry), DecisionGrounds::ReviewFindingRef);
    }

    // Deprecated: remaining 3 grounds (user_ref tested above)
    #[test]
    fn test_evaluate_adr_decision_deprecated_with_grandfathered_returns_grandfathered() {
        let entry = AdrDecisionEntry::DeprecatedDecision(DeprecatedDecision::new(common_with(
            "D20", None, None, true,
        )));
        assert_eq!(evaluate_adr_decision(entry), DecisionGrounds::Grandfathered);
    }

    #[test]
    fn test_evaluate_adr_decision_deprecated_with_review_ref_returns_review_finding_ref() {
        let entry = AdrDecisionEntry::DeprecatedDecision(DeprecatedDecision::new(common_with(
            "D21",
            None,
            Some("RF-04"),
            false,
        )));
        assert_eq!(evaluate_adr_decision(entry), DecisionGrounds::ReviewFindingRef);
    }

    #[test]
    fn test_evaluate_adr_decision_deprecated_with_no_grounds_returns_no_grounds() {
        let entry = AdrDecisionEntry::DeprecatedDecision(DeprecatedDecision::new(common_with(
            "D22", None, None, false,
        )));
        assert_eq!(evaluate_adr_decision(entry), DecisionGrounds::NoGrounds);
    }
}
