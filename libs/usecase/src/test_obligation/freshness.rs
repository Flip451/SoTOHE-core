//! Canonical semantic material shared by evaluate, check, and results.
//!
//! The three verifier lanes must hash the same structured specification element
//! and the same entry-local responsibility.  Keeping both the projection from
//! `SpecDocument` and the material format here prevents a consumer from
//! silently dropping the specification section (IN-01 / IN-04 / AC-03).

use domain::tddd::semantic_verify::{SpecElementRef, SpecSectionKind};
use domain::tddd::test_obligation::hashes::{ObligationResponsibilityHash, SpecElementHash};
use domain::tddd::test_obligation::ids::{
    TestObligationAnchorId, TestObligationBrief, TestObligationEdgeId, TestObligationId,
};
use domain::{SpecDocument, SpecElementId};

use super::sha256_content_hash;

/// Returns the stable wire label for one top-level specification section.
fn section_label(section: &SpecSectionKind) -> &'static str {
    match section {
        SpecSectionKind::Goal => "goal",
        SpecSectionKind::InScope => "in_scope",
        SpecSectionKind::OutOfScope => "out_of_scope",
        SpecSectionKind::Constraint => "constraint",
        SpecSectionKind::AcceptanceCriteria => "acceptance_criteria",
    }
}

/// Builds the canonical material for a structured specification element.
///
/// Section membership is deliberately part of the material.  Thus moving an
/// element with unchanged identifier and text to another top-level section
/// invalidates both pass and fail verdict keys in every lane.
pub(super) fn spec_element_material(spec_element: &SpecElementRef) -> String {
    format!(
        "section={}\nelement_id={}\ntext_label={}",
        section_label(&spec_element.section),
        spec_element.element_id.as_ref(),
        spec_element.text_label,
    )
}

/// Builds the material for an entry-local obligation responsibility.
pub(super) fn responsibility_material(
    obligation_id: &TestObligationId,
    obligation_brief: &TestObligationBrief,
) -> String {
    format!(
        "entry_key={}\nobligation_kind={}\nitem_identifier={}\nobligation_brief={}",
        obligation_id.entry_key().as_str(),
        obligation_id.obligation_kind().as_kebab(),
        obligation_id.item_identifier().as_str(),
        obligation_brief.as_str(),
    )
}

/// Hashes the structured specification element selected by an obligation edge.
///
/// A missing anchor is represented explicitly rather than guessed from the
/// identifier or text.  Normal evaluation cannot produce a cache key for such
/// an edge, while check/results will report the resulting mismatch as stale.
pub(super) fn spec_element_hash(
    spec_elements: &[SpecElementRef],
    edge: &TestObligationEdgeId,
) -> SpecElementHash {
    let material = spec_element_for_anchor(spec_elements, edge.anchor_id())
        .map(spec_element_material)
        .unwrap_or_else(|| {
            format!("section=missing\nelement_id={}\ntext_label=", edge.anchor_id().element_id())
        });
    SpecElementHash::new(sha256_content_hash(material.as_bytes()))
}

/// Hashes the canonical entry-local responsibility material.
pub(super) fn responsibility_hash(
    obligation_id: &TestObligationId,
    obligation_brief: &TestObligationBrief,
) -> ObligationResponsibilityHash {
    ObligationResponsibilityHash::new(sha256_content_hash(
        responsibility_material(obligation_id, obligation_brief).as_bytes(),
    ))
}

/// Finds the structured specification element selected by an edge.
pub(super) fn spec_element_for_anchor<'a>(
    spec_elements: &'a [SpecElementRef],
    anchor: &TestObligationAnchorId,
) -> Option<&'a SpecElementRef> {
    spec_elements.iter().find(|element| element.element_id.as_ref() == anchor.element_id())
}

/// Projects every supported top-level spec section without losing membership.
pub(super) fn spec_elements_from_document(spec: &SpecDocument) -> Vec<SpecElementRef> {
    let mut elements = Vec::new();
    append_requirements(&mut elements, spec.goal(), SpecSectionKind::Goal);
    append_requirements(&mut elements, spec.scope().in_scope(), SpecSectionKind::InScope);
    append_requirements(&mut elements, spec.scope().out_of_scope(), SpecSectionKind::OutOfScope);
    append_requirements(&mut elements, spec.constraints(), SpecSectionKind::Constraint);
    append_requirements(
        &mut elements,
        spec.acceptance_criteria(),
        SpecSectionKind::AcceptanceCriteria,
    );
    elements
}

/// Resolves an element by id through the same structured projection used by
/// check/results, so evaluate cannot select a different section interpretation.
pub(super) fn resolve_spec_element(
    spec: &SpecDocument,
    element_id: &str,
) -> Option<SpecElementRef> {
    let element_id = SpecElementId::try_new(element_id.to_owned()).ok()?;
    spec_elements_from_document(spec).into_iter().find(|element| element.element_id == element_id)
}

fn append_requirements(
    elements: &mut Vec<SpecElementRef>,
    requirements: &[domain::SpecRequirement],
    section: SpecSectionKind,
) {
    for requirement in requirements {
        elements.push(SpecElementRef::new(
            section.clone(),
            requirement.id().clone(),
            requirement.text().to_owned(),
        ));
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

    use super::*;
    use domain::tddd::test_obligation::ids::TestObligationItemIdentifier;
    use domain::tddd::test_obligation::vocab::TestObligationKind;

    fn element(section: SpecSectionKind, text: &str) -> SpecElementRef {
        SpecElementRef::new(
            section,
            SpecElementId::try_new("IN-01".to_owned()).unwrap(),
            text.to_owned(),
        )
    }

    fn obligation() -> TestObligationId {
        TestObligationId::new(
            domain::tddd::semantic_verify::CatalogueEntryKey::try_new("Entry".to_owned()).unwrap(),
            TestObligationKind::Contract,
            TestObligationItemIdentifier::try_new("responsibility".to_owned()).unwrap(),
        )
    }

    #[test]
    fn section_membership_changes_canonical_hash_for_equal_id_and_text() {
        let in_scope = element(SpecSectionKind::InScope, "same text");
        let out_of_scope = element(SpecSectionKind::OutOfScope, "same text");

        assert_ne!(spec_element_material(&in_scope), spec_element_material(&out_of_scope));
        assert_ne!(
            spec_element_hash(
                std::slice::from_ref(&in_scope),
                &TestObligationEdgeId::new(
                    obligation().entry_key().clone(),
                    TestObligationAnchorId::try_new("spec.json".to_owned(), "IN-01".to_owned())
                        .unwrap(),
                ),
            ),
            spec_element_hash(
                &[out_of_scope],
                &TestObligationEdgeId::new(
                    obligation().entry_key().clone(),
                    TestObligationAnchorId::try_new("spec.json".to_owned(), "IN-01".to_owned())
                        .unwrap(),
                ),
            ),
        );
    }

    #[test]
    fn responsibility_material_changes_when_brief_changes() {
        let id = obligation();
        let first = TestObligationBrief::try_new("first brief".to_owned()).unwrap();
        let second = TestObligationBrief::try_new("second brief".to_owned()).unwrap();

        assert_ne!(responsibility_material(&id, &first), responsibility_material(&id, &second));
        assert_ne!(responsibility_hash(&id, &first), responsibility_hash(&id, &second));
    }
}
