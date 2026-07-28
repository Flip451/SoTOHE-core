//! Verifier-invocation adjacency: escalation driver execution and verdict tally.

use domain::tddd::test_obligation::drift::{EdgeResolutionOutcome, EdgeVerdictRecord};
use domain::tddd::test_obligation::errors::{ObligationEvaluateError, SemanticVerifierError};
use domain::tddd::test_obligation::ids::{
    TestObligationAnchorId, TestObligationEdgeId, TestObligationId,
};
use domain::tddd::test_obligation::obligations::TestObligation;
use domain::tddd::test_obligation::verdict::{ObligationFulfillmentVerdict, WaiverVerdict};

use super::Tally;

/// Maps a semantic-verifier port error onto the evaluate error vocabulary.
pub(super) fn map_verifier_error(error: SemanticVerifierError) -> ObligationEvaluateError {
    ObligationEvaluateError::VerifierPort(error)
}

/// Increments the tally for a fulfillment verdict.
pub(super) fn record_fulfillment(
    edge_id: &TestObligationEdgeId,
    verdict: &ObligationFulfillmentVerdict,
    tally: &mut Tally,
) {
    match verdict {
        ObligationFulfillmentVerdict::Fulfilled { .. } => tally.pass += 1,
        ObligationFulfillmentVerdict::Fail { .. } => {
            tally.fail += 1;
            tally.failure_records.push(EdgeVerdictRecord::new(
                None,
                edge_id.clone(),
                None,
                None,
                EdgeResolutionOutcome::Fulfillment(verdict.clone()),
                None,
            ));
        }
        ObligationFulfillmentVerdict::Pending => {
            tally.pending += 1;
            tally.pending_records.push(EdgeVerdictRecord::new(
                None,
                edge_id.clone(),
                None,
                None,
                EdgeResolutionOutcome::Fulfillment(verdict.clone()),
                None,
            ));
        }
    }
}

/// Records a bound fulfillment whose derived obligation cannot be resolved.
pub(super) fn record_pending_obligation_id(obligation_id: &TestObligationId, tally: &mut Tally) {
    let mut element = format!(
        "{}:{}",
        obligation_id.obligation_kind().as_kebab(),
        obligation_id.item_identifier().as_str()
    );
    let anchor = loop {
        match TestObligationAnchorId::try_new("unresolved-obligation".to_owned(), element) {
            Ok(anchor) => break anchor,
            // Unreachable: the components are non-empty; reset defensively.
            Err(_) => element = "unresolved".to_owned(),
        }
    };
    record_pending_fulfillment_edge(
        TestObligationEdgeId::new(obligation_id.entry_key().clone(), anchor),
        tally,
    );
}

/// Records every known obligation edge as pending; falls back to the obligation id.
pub(super) fn record_pending_obligation_edges(obligation: &TestObligation, tally: &mut Tally) {
    let mut recorded_edge = false;
    for anchor in obligation.spec_refs() {
        record_pending_fulfillment_edge(
            TestObligationEdgeId::new(obligation.id().entry_key().clone(), anchor.clone()),
            tally,
        );
        recorded_edge = true;
    }
    if !recorded_edge {
        record_pending_obligation_id(obligation.id(), tally);
    }
}

/// Records a bound fulfillment edge whose verifier inputs could not be resolved.
pub(super) fn record_pending_fulfillment_edge(edge_id: TestObligationEdgeId, tally: &mut Tally) {
    record_pending_with_outcome(
        edge_id,
        EdgeResolutionOutcome::Fulfillment(ObligationFulfillmentVerdict::Pending),
        tally,
    );
}

/// Records a bound waiver edge whose verifier inputs could not be resolved.
pub(super) fn record_pending_waiver_edge(edge_id: TestObligationEdgeId, tally: &mut Tally) {
    record_pending_with_outcome(
        edge_id,
        EdgeResolutionOutcome::Waiver(WaiverVerdict::Pending),
        tally,
    );
}

fn record_pending_with_outcome(
    edge_id: TestObligationEdgeId,
    outcome: EdgeResolutionOutcome,
    tally: &mut Tally,
) {
    tally.pending += 1;
    tally.pending_records.push(EdgeVerdictRecord::new(None, edge_id, None, None, outcome, None));
}

/// Increments the tally for a waiver verdict.
pub(super) fn record_waiver(
    edge_id: &TestObligationEdgeId,
    verdict: &WaiverVerdict,
    tally: &mut Tally,
) {
    match verdict {
        WaiverVerdict::Waived { .. } => tally.pass += 1,
        WaiverVerdict::Fail { .. } => {
            tally.fail += 1;
            tally.failure_records.push(EdgeVerdictRecord::new(
                None,
                edge_id.clone(),
                None,
                None,
                EdgeResolutionOutcome::Waiver(verdict.clone()),
                None,
            ));
        }
        WaiverVerdict::Pending => {
            tally.pending += 1;
            tally.pending_records.push(EdgeVerdictRecord::new(
                None,
                edge_id.clone(),
                None,
                None,
                EdgeResolutionOutcome::Waiver(verdict.clone()),
                None,
            ));
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use domain::tddd::semantic_verify::CatalogueEntryKey;
    use domain::tddd::test_obligation::drift::{EdgeResolutionOutcome, EdgeVerdictRecord};
    use domain::tddd::test_obligation::ids::{
        DiagnosticMessage, TestObligationAnchorId, TestObligationEdgeId, TestObligationId,
        TestObligationItemIdentifier,
    };
    use domain::tddd::test_obligation::verdict::{ObligationFulfillmentVerdict, WaiverVerdict};
    use domain::tddd::test_obligation::vocab::{FulfillmentFailCategory, TestObligationKind};

    use super::{record_fulfillment, record_pending_obligation_id, record_waiver};
    use crate::test_obligation::evaluate::Tally;

    fn edge_id() -> TestObligationEdgeId {
        TestObligationEdgeId::new(
            CatalogueEntryKey::try_new("domain::User".to_owned()).unwrap(),
            TestObligationAnchorId::try_new("spec.json".to_owned(), "CN-04".to_owned()).unwrap(),
        )
    }

    fn reason(text: &str) -> DiagnosticMessage {
        DiagnosticMessage::try_new(text.to_owned()).unwrap()
    }

    #[test]
    fn test_failure_records_keep_reason_only_in_nested_verdict() {
        let edge_id = edge_id();
        let fulfillment = ObligationFulfillmentVerdict::Fail {
            category: FulfillmentFailCategory::Contradiction,
            reason: reason("fulfillment fails"),
        };
        let waiver = WaiverVerdict::Fail { reason: reason("waiver fails") };
        let mut tally = Tally::default();

        record_fulfillment(&edge_id, &fulfillment, &mut tally);
        record_waiver(&edge_id, &waiver, &mut tally);

        assert_eq!(tally.fail, 2);
        assert_eq!(
            tally.failure_records,
            vec![
                EdgeVerdictRecord::new(
                    None,
                    edge_id.clone(),
                    None,
                    None,
                    EdgeResolutionOutcome::Fulfillment(fulfillment),
                    None,
                ),
                EdgeVerdictRecord::new(
                    None,
                    edge_id,
                    None,
                    None,
                    EdgeResolutionOutcome::Waiver(waiver),
                    None,
                ),
            ]
        );
    }

    #[test]
    fn test_unresolved_bound_obligation_records_pending_fulfillment_not_missing_binding() {
        let obligation_id = TestObligationId::new(
            CatalogueEntryKey::try_new("domain::User".to_owned()).unwrap(),
            TestObligationKind::Boundary,
            TestObligationItemIdentifier::try_new("invariant:active".to_owned()).unwrap(),
        );
        let expected_edge = TestObligationEdgeId::new(
            obligation_id.entry_key().clone(),
            TestObligationAnchorId::try_new(
                "unresolved-obligation".to_owned(),
                "boundary:invariant:active".to_owned(),
            )
            .unwrap(),
        );
        let mut tally = Tally::default();

        record_pending_obligation_id(&obligation_id, &mut tally);

        assert_eq!(tally.pending, 1);
        assert_eq!(
            tally.pending_records,
            vec![EdgeVerdictRecord::new(
                None,
                expected_edge,
                None,
                None,
                EdgeResolutionOutcome::Fulfillment(ObligationFulfillmentVerdict::Pending),
                None,
            )]
        );
    }
}
