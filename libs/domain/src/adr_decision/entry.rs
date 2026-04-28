//! Heterogeneous-collection boundary enum for ADR decision lifecycle states.

use super::state::{
    AcceptedDecision, DeprecatedDecision, ImplementedDecision, ProposedDecision, SupersededDecision,
};

/// Enum wrapper providing a heterogeneous-collection boundary for
/// `Vec<AdrDecisionEntry>` in `AdrFrontMatter::decisions` (T002).
///
/// Each variant holds one of the five lifecycle typestate structs, allowing a
/// `Vec` to hold decisions at any lifecycle state while each typestate remains a
/// distinct, compile-time-typed value.
///
/// [`crate::adr_decision::evaluate_adr_decision`] pattern-matches on this
/// enum to extract the embedded [`crate::adr_decision::AdrDecisionCommon`]
/// grounds fields from whichever typestate variant is present.
///
/// No serde derives — serde lives in the infrastructure DTO layer (CN-05).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdrDecisionEntry {
    /// A decision in the proposed (drafted, awaiting review) state.
    ProposedDecision(ProposedDecision),
    /// A decision in the accepted (review complete, ready for implementation) state.
    AcceptedDecision(AcceptedDecision),
    /// A decision in the implemented (actualized) state.
    ImplementedDecision(ImplementedDecision),
    /// A decision that has been superseded by a later decision.
    SupersededDecision(SupersededDecision),
    /// A decision that has been retired without replacement.
    DeprecatedDecision(DeprecatedDecision),
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::adr_decision::common::AdrDecisionCommon;

    fn common(id: &str) -> AdrDecisionCommon {
        AdrDecisionCommon::new(id, None, None, None, false).unwrap()
    }

    #[test]
    fn test_adr_decision_entry_proposed_variant_holds_proposed_decision() {
        let entry = AdrDecisionEntry::ProposedDecision(ProposedDecision::new(common("D1")));
        assert!(matches!(entry, AdrDecisionEntry::ProposedDecision(_)));
    }

    #[test]
    fn test_adr_decision_entry_accepted_variant_holds_accepted_decision() {
        let entry = AdrDecisionEntry::AcceptedDecision(AcceptedDecision::new(common("D2")));
        assert!(matches!(entry, AdrDecisionEntry::AcceptedDecision(_)));
    }

    #[test]
    fn test_adr_decision_entry_implemented_variant_holds_implemented_decision() {
        let entry = AdrDecisionEntry::ImplementedDecision(
            ImplementedDecision::new(common("D3"), "abc1234".to_string()).unwrap(),
        );
        if let AdrDecisionEntry::ImplementedDecision(ref d) = entry {
            assert_eq!(d.implemented_in(), "abc1234");
        } else {
            panic!("expected ImplementedDecision variant");
        }
    }

    #[test]
    fn test_adr_decision_entry_superseded_variant_holds_superseded_decision() {
        let entry = AdrDecisionEntry::SupersededDecision(
            SupersededDecision::new(common("D4"), "2026-05-01-other.md#D1".to_string()).unwrap(),
        );
        if let AdrDecisionEntry::SupersededDecision(ref d) = entry {
            assert_eq!(d.superseded_by(), "2026-05-01-other.md#D1");
        } else {
            panic!("expected SupersededDecision variant");
        }
    }

    #[test]
    fn test_adr_decision_entry_deprecated_variant_holds_deprecated_decision() {
        let entry = AdrDecisionEntry::DeprecatedDecision(DeprecatedDecision::new(common("D5")));
        assert!(matches!(entry, AdrDecisionEntry::DeprecatedDecision(_)));
    }
}
