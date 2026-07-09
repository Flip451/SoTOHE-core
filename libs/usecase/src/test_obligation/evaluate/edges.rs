//! Edge / obligation lookup and spec-anchor resolution helpers for `evaluate`.

use domain::SpecDocument;
use domain::tddd::test_obligation::ids::{
    TestObligationAnchorId, TestObligationEdgeId, TestObligationId, TestObligationItemIdentifier,
};
use domain::tddd::test_obligation::obligations::{ObligationsDocument, TestObligation};
use domain::tddd::test_obligation::vocab::TestObligationKind;

/// Finds the obligation with the given id in the obligations document.
pub(super) fn find_obligation<'a>(
    obligations: &'a ObligationsDocument,
    id: &TestObligationId,
) -> Option<&'a TestObligation> {
    obligations.obligations().iter().find(|o| o.id() == id)
}

/// Finds a derived obligation that owns `edge_id`.
pub(super) fn find_obligation_for_edge<'a>(
    obligations: &'a ObligationsDocument,
    edge_id: &TestObligationEdgeId,
) -> Option<&'a TestObligation> {
    obligations.obligations().iter().find(|obligation| {
        obligation.id().entry_key() == edge_id.entry_key()
            && obligation.spec_refs().iter().any(|anchor| anchor == edge_id.anchor_id())
    })
}

/// Resolves the anchor text of the spec element `element_id`, searching every
/// section of the spec document.
pub(super) fn resolve_anchor_text(spec: &SpecDocument, element_id: &str) -> Option<String> {
    let sections = [
        spec.goal(),
        spec.scope().in_scope(),
        spec.scope().out_of_scope(),
        spec.constraints(),
        spec.acceptance_criteria(),
    ];
    for section in sections {
        if let Some(requirement) = section.iter().find(|r| r.id().as_ref() == element_id) {
            return Some(requirement.text().to_owned());
        }
    }
    None
}

/// Synthesises an obligation id for a voluntary binding's cache entry (AC-12).
pub(super) fn synthetic_obligation_id(edge_id: &TestObligationEdgeId) -> TestObligationId {
    let item = build_item_identifier(edge_id.anchor_id());
    TestObligationId::new(edge_id.entry_key().clone(), TestObligationKind::Logic, item)
}

/// Builds a voluntary-binding item identifier from an anchor id.
fn build_item_identifier(anchor: &TestObligationAnchorId) -> TestObligationItemIdentifier {
    let mut text = format!("voluntary:{}", anchor.element_id());
    loop {
        match TestObligationItemIdentifier::try_new(text) {
            Ok(item) => return item,
            // Unreachable: the prefix guarantees non-empty input; reset defensively.
            Err(_) => text = "voluntary:edge".to_owned(),
        }
    }
}
