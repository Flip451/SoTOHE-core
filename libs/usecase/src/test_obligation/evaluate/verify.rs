//! Verifier-invocation adjacency: escalation driver execution and verdict tally.

use domain::tddd::test_obligation::drift::{EdgeResolutionOutcome, EdgeVerdictRecord};
use domain::tddd::test_obligation::errors::{ObligationEvaluateError, SemanticVerifierError};
use domain::tddd::test_obligation::ids::{
    TestObligationAnchorId, TestObligationEdgeId, TestObligationId,
};
use domain::tddd::test_obligation::obligations::TestObligation;
use domain::tddd::test_obligation::verdict::{ObligationFulfillmentVerdict, WaiverVerdict};
use domain::tddd::test_obligation::vocab::FulfillmentFailCategory;

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
        ObligationFulfillmentVerdict::Fail { category, .. } => {
            tally.fail += 1;
            tally.failure_records.push(EdgeVerdictRecord::new(
                edge_id.clone(),
                EdgeResolutionOutcome::Fail(category.clone()),
                None,
            ));
        }
        ObligationFulfillmentVerdict::Pending => {
            tally.pending += 1;
            tally.pending_records.push(EdgeVerdictRecord::new(
                edge_id.clone(),
                EdgeResolutionOutcome::Pending,
                None,
            ));
        }
    }
}

/// Records an unresolved derived obligation using a deterministic synthetic edge.
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
    record_pending_edge(
        TestObligationEdgeId::new(obligation_id.entry_key().clone(), anchor),
        tally,
    );
}

/// Records every known obligation edge as pending; falls back to the obligation id.
pub(super) fn record_pending_obligation_edges(obligation: &TestObligation, tally: &mut Tally) {
    let mut recorded_edge = false;
    for anchor in obligation.spec_refs() {
        record_pending_edge(
            TestObligationEdgeId::new(obligation.id().entry_key().clone(), anchor.clone()),
            tally,
        );
        recorded_edge = true;
    }
    if !recorded_edge {
        record_pending_obligation_id(obligation.id(), tally);
    }
}

/// Records an unresolved edge as pending so `execute` fails closed after cache save.
pub(super) fn record_pending_edge(edge_id: TestObligationEdgeId, tally: &mut Tally) {
    tally.pending += 1;
    tally.pending_records.push(EdgeVerdictRecord::new(
        edge_id,
        EdgeResolutionOutcome::Pending,
        None,
    ));
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
                edge_id.clone(),
                EdgeResolutionOutcome::Fail(FulfillmentFailCategory::CentralUnverified),
                None,
            ));
        }
        WaiverVerdict::Pending => {
            tally.pending += 1;
            tally.pending_records.push(EdgeVerdictRecord::new(
                edge_id.clone(),
                EdgeResolutionOutcome::Pending,
                None,
            ));
        }
    }
}
