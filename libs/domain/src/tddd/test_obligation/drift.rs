//! Drift findings and per-edge resolution records for the obligation gate.
//!
//! [`TestObligationDrift`] pairs a [`TestObligationDriftKind`] with the identity
//! it concerns (an obligation id for `missing`, an edge id for the others) plus a
//! human-readable detail. [`EdgeResolutionOutcome`] is how a single obligation
//! edge resolved, and [`EdgeVerdictRecord`] ties an edge to its outcome and any
//! drift the `check` gate detected (IN-13 / CN-04 / AC-05).

use crate::tddd::test_obligation::ids::{
    DiagnosticMessage, TestObligationEdgeId, TestObligationId,
};
use crate::tddd::test_obligation::vocab::{FulfillmentFailCategory, TestObligationDriftKind};

/// A single detected drift against a test obligation or its binding edge.
///
/// The constructors mirror the six [`TestObligationDriftKind`] cases: the
/// existence family (`missing` is keyed by obligation id, `orphaned` by edge id)
/// and the freshness family (all keyed by edge id).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestObligationDrift {
    // Read by the results interactor (T019) when it renders the drift report; no
    // read accessors are declared in this batch's type contract.
    #[allow(dead_code)]
    kind: TestObligationDriftKind,
    #[allow(dead_code)]
    obligation_id: Option<TestObligationId>,
    #[allow(dead_code)]
    edge_id: Option<TestObligationEdgeId>,
    #[allow(dead_code)]
    detail: DiagnosticMessage,
}

impl TestObligationDrift {
    /// An obligation with no binding (or whose bound test no longer exists).
    #[must_use]
    pub fn missing_obligation(obligation_id: TestObligationId, detail: DiagnosticMessage) -> Self {
        Self {
            kind: TestObligationDriftKind::Missing,
            obligation_id: Some(obligation_id),
            edge_id: None,
            detail,
        }
    }

    /// A binding edge with no derived obligation.
    #[must_use]
    pub fn orphaned_edge(edge_id: TestObligationEdgeId, detail: DiagnosticMessage) -> Self {
        Self {
            kind: TestObligationDriftKind::Orphaned,
            obligation_id: None,
            edge_id: Some(edge_id),
            detail,
        }
    }

    /// An edge whose anchor text hash changed, staling its verdict.
    #[must_use]
    pub fn spec_changed_edge(edge_id: TestObligationEdgeId, detail: DiagnosticMessage) -> Self {
        Self {
            kind: TestObligationDriftKind::SpecChanged,
            obligation_id: None,
            edge_id: Some(edge_id),
            detail,
        }
    }

    /// An edge whose entry declaration hash changed, staling its verdict.
    #[must_use]
    pub fn decl_changed_edge(edge_id: TestObligationEdgeId, detail: DiagnosticMessage) -> Self {
        Self {
            kind: TestObligationDriftKind::DeclChanged,
            obligation_id: None,
            edge_id: Some(edge_id),
            detail,
        }
    }

    /// An edge whose bound test body hash changed, staling its verdict.
    #[must_use]
    pub fn test_changed_edge(edge_id: TestObligationEdgeId, detail: DiagnosticMessage) -> Self {
        Self {
            kind: TestObligationDriftKind::TestChanged,
            obligation_id: None,
            edge_id: Some(edge_id),
            detail,
        }
    }

    /// An edge whose waived reason hash changed, staling its verdict.
    #[must_use]
    pub fn reason_changed_edge(edge_id: TestObligationEdgeId, detail: DiagnosticMessage) -> Self {
        Self {
            kind: TestObligationDriftKind::ReasonChanged,
            obligation_id: None,
            edge_id: Some(edge_id),
            detail,
        }
    }
}

/// How a single obligation edge resolved during the `check` gate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EdgeResolutionOutcome {
    /// A fresh fulfilled verdict resolves the edge.
    Fulfilled,
    /// A fresh waived verdict resolves the edge.
    Waived,
    /// The edge resolved to a failing fulfillment verdict of this category.
    Fail(FulfillmentFailCategory),
    /// The edge has no fresh verdict; treated as fail at the gate.
    Pending,
    /// The edge has no binding at all.
    MissingBinding,
}

/// An edge paired with how it resolved and any drift detected against it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdgeVerdictRecord {
    // Read by the results interactor (T019) when it renders per-edge records; no
    // read accessors are declared in this batch's type contract.
    #[allow(dead_code)]
    edge_id: TestObligationEdgeId,
    #[allow(dead_code)]
    outcome: EdgeResolutionOutcome,
    #[allow(dead_code)]
    drift: Option<TestObligationDrift>,
}

impl EdgeVerdictRecord {
    /// Builds an [`EdgeVerdictRecord`].
    #[must_use]
    pub fn new(
        edge_id: TestObligationEdgeId,
        outcome: EdgeResolutionOutcome,
        drift: Option<TestObligationDrift>,
    ) -> Self {
        Self { edge_id, outcome, drift }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::tddd::semantic_verify::CatalogueEntryKey;
    use crate::tddd::test_obligation::ids::{TestObligationAnchorId, TestObligationItemIdentifier};
    use crate::tddd::test_obligation::vocab::TestObligationKind;

    fn detail(text: &str) -> DiagnosticMessage {
        DiagnosticMessage::try_new(text.to_owned()).unwrap()
    }

    fn edge_id() -> TestObligationEdgeId {
        TestObligationEdgeId::new(
            CatalogueEntryKey::try_new("domain::User".to_owned()).unwrap(),
            TestObligationAnchorId::try_new("spec.json".to_owned(), "IN-13".to_owned()).unwrap(),
        )
    }

    fn obligation_id() -> TestObligationId {
        TestObligationId::new(
            CatalogueEntryKey::try_new("domain::User".to_owned()).unwrap(),
            TestObligationKind::Boundary,
            TestObligationItemIdentifier::try_new("invariant:non_empty".to_owned()).unwrap(),
        )
    }

    #[test]
    fn test_all_drift_constructors_produce_distinct_findings() {
        let drifts = [
            TestObligationDrift::missing_obligation(obligation_id(), detail("no binding")),
            TestObligationDrift::orphaned_edge(edge_id(), detail("no obligation")),
            TestObligationDrift::spec_changed_edge(edge_id(), detail("anchor changed")),
            TestObligationDrift::decl_changed_edge(edge_id(), detail("decl changed")),
            TestObligationDrift::test_changed_edge(edge_id(), detail("test changed")),
            TestObligationDrift::reason_changed_edge(edge_id(), detail("reason changed")),
        ];
        for (i, a) in drifts.iter().enumerate() {
            for b in drifts.iter().skip(i + 1) {
                assert_ne!(a, b);
            }
        }
    }

    #[test]
    fn test_identical_drift_findings_are_equal() {
        let a = TestObligationDrift::orphaned_edge(edge_id(), detail("no obligation"));
        let b = TestObligationDrift::orphaned_edge(edge_id(), detail("no obligation"));
        assert_eq!(a, b);
    }

    #[test]
    fn test_edge_resolution_outcome_variants_are_distinct() {
        assert_ne!(EdgeResolutionOutcome::Fulfilled, EdgeResolutionOutcome::Waived);
        assert_ne!(
            EdgeResolutionOutcome::Fail(FulfillmentFailCategory::Contradiction),
            EdgeResolutionOutcome::Fail(FulfillmentFailCategory::Substitution)
        );
        assert_ne!(EdgeResolutionOutcome::Pending, EdgeResolutionOutcome::MissingBinding);
    }

    #[test]
    fn test_edge_verdict_record_holds_outcome_and_optional_drift() {
        let fulfilled = EdgeVerdictRecord::new(edge_id(), EdgeResolutionOutcome::Fulfilled, None);
        let missing = EdgeVerdictRecord::new(
            edge_id(),
            EdgeResolutionOutcome::MissingBinding,
            Some(TestObligationDrift::missing_obligation(obligation_id(), detail("no binding"))),
        );
        assert_ne!(fulfilled, missing);
    }
}
