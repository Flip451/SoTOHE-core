//! Drift findings and per-edge resolution records for the obligation gate.
//!
//! [`TestObligationDrift`] pairs a [`TestObligationDriftKind`] with the identity
//! it concerns (an obligation id for `missing`, an edge id for the others) plus a
//! human-readable detail. [`EdgeResolutionOutcome`] is how a single obligation
//! edge resolved, and [`EdgeVerdictRecord`] ties an edge to its outcome and any
//! drift the `check` gate detected (IN-13 / CN-04 / AC-05).

use crate::tddd::catalogue_v2::roles::ConstructionError;
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
    obligation_id: Option<TestObligationId>,
    #[allow(dead_code)]
    edge_id: TestObligationEdgeId,
    #[allow(dead_code)]
    claim_source: Option<DiagnosticMessage>,
    #[allow(dead_code)]
    evidence_source: Option<DiagnosticMessage>,
    #[allow(dead_code)]
    outcome: EdgeResolutionOutcome,
    #[allow(dead_code)]
    verdict_reason: Option<DiagnosticMessage>,
    #[allow(dead_code)]
    drift: Option<TestObligationDrift>,
}

impl EdgeVerdictRecord {
    /// Builds an [`EdgeVerdictRecord`].
    #[must_use]
    pub fn new(
        obligation_id: Option<TestObligationId>,
        edge_id: TestObligationEdgeId,
        claim_source: Option<DiagnosticMessage>,
        evidence_source: Option<DiagnosticMessage>,
        outcome: EdgeResolutionOutcome,
        verdict_reason: Option<DiagnosticMessage>,
        drift: Option<TestObligationDrift>,
    ) -> Self {
        Self {
            obligation_id,
            edge_id,
            claim_source,
            evidence_source,
            outcome,
            verdict_reason,
            drift,
        }
    }
}

/// Validated non-empty set of [`TestObligationDrift`] findings.
///
/// Carries the detected drifts in
/// [`ObligationCheckError::DriftsDetected`](crate::tddd::test_obligation::errors::ObligationCheckError::DriftsDetected)
/// so the drift error can never be raised with an empty finding set. Mirrors the
/// [`NonEmptyEdgeIds`](crate::tddd::test_obligation::ids::NonEmptyEdgeIds) /
/// [`NonEmptyTestLocations`](crate::tddd::test_obligation::binding::NonEmptyTestLocations)
/// pattern: [`try_new`](Self::try_new) rejects empty vectors via
/// [`ConstructionError::EmptyCollection`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NonEmptyDrifts((TestObligationDrift, Vec<TestObligationDrift>));

impl NonEmptyDrifts {
    /// Builds a [`NonEmptyDrifts`] from a required first element and the
    /// remaining (possibly empty) elements; the invariant holds by construction.
    #[must_use]
    pub fn new(first: TestObligationDrift, rest: Vec<TestObligationDrift>) -> Self {
        let mut values = Vec::with_capacity(rest.len() + 1);
        values.push(first.clone());
        values.extend(rest);
        Self((first, values))
    }

    /// Validates and wraps `values`, rejecting an empty vector.
    ///
    /// # Errors
    ///
    /// Returns [`ConstructionError::EmptyCollection`] when `values` is empty.
    pub fn try_new(values: Vec<TestObligationDrift>) -> Result<Self, ConstructionError> {
        let Some(first) = values.first().cloned() else {
            return Err(ConstructionError::EmptyCollection);
        };
        Ok(Self((first, values)))
    }

    /// Returns the drifts as a slice, guaranteed non-empty.
    #[must_use]
    pub fn as_slice(&self) -> &[TestObligationDrift] {
        self.0.1.as_slice()
    }

    /// Returns the first drift, always present by invariant.
    #[must_use]
    pub fn first(&self) -> &TestObligationDrift {
        &self.0.0
    }

    /// Structural predicate backing the `non_empty` invariant declaration.
    #[must_use]
    pub fn is_non_empty(&self) -> bool {
        true
    }
}

/// Validated non-empty set of [`EdgeVerdictRecord`] entries.
///
/// Carries the per-edge failure / escalation records in
/// [`ObligationEvaluateError::SemanticFailuresConfirmed`](crate::tddd::test_obligation::errors::ObligationEvaluateError::SemanticFailuresConfirmed)
/// / [`HumanEscalationRequired`](crate::tddd::test_obligation::errors::ObligationEvaluateError::HumanEscalationRequired)
/// so those errors can never be raised with an empty record set. Mirrors the
/// [`NonEmptyDrifts`] / [`NonEmptyEdgeIds`](crate::tddd::test_obligation::ids::NonEmptyEdgeIds)
/// pattern: [`try_new`](Self::try_new) rejects empty vectors via
/// [`ConstructionError::EmptyCollection`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NonEmptyEdgeVerdictRecords((EdgeVerdictRecord, Vec<EdgeVerdictRecord>));

impl NonEmptyEdgeVerdictRecords {
    /// Builds a [`NonEmptyEdgeVerdictRecords`] from a required first element and
    /// the remaining (possibly empty) elements; the invariant holds by construction.
    #[must_use]
    pub fn new(first: EdgeVerdictRecord, rest: Vec<EdgeVerdictRecord>) -> Self {
        let mut values = Vec::with_capacity(rest.len() + 1);
        values.push(first.clone());
        values.extend(rest);
        Self((first, values))
    }

    /// Validates and wraps `values`, rejecting an empty vector.
    ///
    /// # Errors
    ///
    /// Returns [`ConstructionError::EmptyCollection`] when `values` is empty.
    pub fn try_new(values: Vec<EdgeVerdictRecord>) -> Result<Self, ConstructionError> {
        let Some(first) = values.first().cloned() else {
            return Err(ConstructionError::EmptyCollection);
        };
        Ok(Self((first, values)))
    }

    /// Returns the records as a slice, guaranteed non-empty.
    #[must_use]
    pub fn as_slice(&self) -> &[EdgeVerdictRecord] {
        self.0.1.as_slice()
    }

    /// Returns the first record, always present by invariant.
    #[must_use]
    pub fn first(&self) -> &EdgeVerdictRecord {
        &self.0.0
    }

    /// Structural predicate backing the `non_empty` invariant declaration.
    #[must_use]
    pub fn is_non_empty(&self) -> bool {
        true
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

    fn drift() -> TestObligationDrift {
        TestObligationDrift::missing_obligation(obligation_id(), detail("no binding"))
    }

    fn verdict_record() -> EdgeVerdictRecord {
        EdgeVerdictRecord::new(
            Some(obligation_id()),
            edge_id(),
            None,
            None,
            EdgeResolutionOutcome::MissingBinding,
            None,
            Some(drift()),
        )
    }

    #[test]
    fn test_non_empty_drifts_new_exposes_entries() {
        let first = drift();
        let drifts = NonEmptyDrifts::new(first.clone(), vec![first.clone()]);

        assert!(drifts.is_non_empty());
        assert_eq!(drifts.as_slice().len(), 2);
        assert_eq!(drifts.first(), &first);
    }

    #[test]
    fn test_non_empty_drifts_try_new_accepts_non_empty() {
        let drifts = NonEmptyDrifts::try_new(vec![drift()]).unwrap();

        assert!(drifts.is_non_empty());
        assert_eq!(drifts.as_slice().len(), 1);
    }

    #[test]
    fn test_non_empty_drifts_try_new_rejects_empty() {
        assert_eq!(NonEmptyDrifts::try_new(vec![]), Err(ConstructionError::EmptyCollection));
    }

    #[test]
    fn test_non_empty_edge_verdict_records_new_exposes_entries() {
        let first = verdict_record();
        let records = NonEmptyEdgeVerdictRecords::new(first.clone(), vec![first.clone()]);

        assert!(records.is_non_empty());
        assert_eq!(records.as_slice().len(), 2);
        assert_eq!(records.first(), &first);
    }

    #[test]
    fn test_non_empty_edge_verdict_records_try_new_accepts_non_empty() {
        let records = NonEmptyEdgeVerdictRecords::try_new(vec![verdict_record()]).unwrap();

        assert!(records.is_non_empty());
        assert_eq!(records.as_slice().len(), 1);
    }

    #[test]
    fn test_non_empty_edge_verdict_records_try_new_rejects_empty() {
        assert_eq!(
            NonEmptyEdgeVerdictRecords::try_new(vec![]),
            Err(ConstructionError::EmptyCollection)
        );
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
        let fulfilled = EdgeVerdictRecord::new(
            Some(obligation_id()),
            edge_id(),
            Some(detail("fulfillment binding")),
            Some(detail("domain::user::tests::test_fulfilled")),
            EdgeResolutionOutcome::Fulfilled,
            None,
            None,
        );
        let missing = EdgeVerdictRecord::new(
            None,
            edge_id(),
            None,
            None,
            EdgeResolutionOutcome::MissingBinding,
            Some(detail("binding is missing")),
            Some(TestObligationDrift::missing_obligation(obligation_id(), detail("no binding"))),
        );
        assert_ne!(fulfilled, missing);
    }
}
