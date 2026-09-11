//! Edge / obligation lookup and spec-anchor resolution helpers for `evaluate`.

use domain::SpecDocument;
use domain::SpecElementId;
use domain::tddd::semantic_verify::{SpecElementRef, SpecSectionKind};
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

/// Resolves the structured specification element `element_id`, preserving its
/// actual section membership and verbatim text.
pub(super) fn resolve_spec_element(
    spec: &SpecDocument,
    element_id: &str,
) -> Option<SpecElementRef> {
    let element_id = SpecElementId::try_new(element_id.to_owned()).ok()?;
    let sections = [
        (SpecSectionKind::Goal, spec.goal()),
        (SpecSectionKind::InScope, spec.scope().in_scope()),
        (SpecSectionKind::OutOfScope, spec.scope().out_of_scope()),
        (SpecSectionKind::Constraint, spec.constraints()),
        (SpecSectionKind::AcceptanceCriteria, spec.acceptance_criteria()),
    ];
    for (section_kind, requirements) in sections {
        if let Some(requirement) = requirements.iter().find(|r| r.id() == &element_id) {
            return Some(SpecElementRef::new(
                section_kind,
                element_id,
                requirement.text().to_owned(),
            ));
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
