//! Shared support for the `test-obligation check` interactor.

// `ObligationCheckError` carries unboxed non-empty payloads by catalogue
// contract; see the parent check module for the full rationale.
#![allow(clippy::result_large_err)]

use domain::tddd::catalogue_v2::CatalogueDocument;
use domain::tddd::catalogue_v2::roles::ItemAction;
use domain::tddd::semantic_verify::{CatalogueEntryKey, SpecSectionKind};
use domain::tddd::test_obligation::binding::{
    TestBindingRecord, TestBindingsDocument, TestLocation,
};
use domain::tddd::test_obligation::drift::TestObligationDrift;
use domain::tddd::test_obligation::errors::ObligationCheckError;
use domain::tddd::test_obligation::ids::{
    TestObligationAnchorId, TestObligationEdgeId, TestObligationId, TestObligationItemIdentifier,
    WaivedReason,
};
use domain::tddd::test_obligation::obligations::ObligationsDocument;
use domain::tddd::test_obligation::scope::UncitedSpecElementFinding;
use domain::tddd::test_obligation::vocab::TestObligationKind;
use domain::{SpecDocument, SpecElementId, SpecRef, SpecRequirement};

use super::status_lanes::{StatusLaneFinding, StatusLaneFindingKind, StatusLaneTarget};
use super::{LoadedCatalogueDocument, cited_anchor_ids, diag};

/// Mutable accumulators for a single `check` run.
#[derive(Default)]
pub(super) struct GateState {
    pub(super) structural_drifts: Vec<TestObligationDrift>,
    pub(super) status_drifts: Vec<(TestObligationDrift, StatusLaneFinding)>,
    pub(super) unresolved: Vec<(TestObligationEdgeId, StatusLaneFinding)>,
    pub(super) verdict_absent: Vec<(TestObligationEdgeId, StatusLaneFinding)>,
    pub(super) resolved: Vec<TestObligationEdgeId>,
}

impl GateState {
    /// Records an existence or freshness drift that status-lane interpretation
    /// may defer only for a todo-attributed entry.
    pub(super) fn status_drift(
        &mut self,
        drift: TestObligationDrift,
        target: StatusLaneTarget,
        kind: StatusLaneFindingKind,
    ) {
        self.status_drifts.push((drift, StatusLaneFinding::new(target, kind)));
    }

    /// Records an orphaned binding, which remains structural in every lane.
    pub(super) fn structural_drift(&mut self, drift: TestObligationDrift) {
        self.structural_drifts.push(drift);
    }

    /// Records an edge with no binding resolution.
    pub(super) fn unresolved(&mut self, edge: TestObligationEdgeId, target: StatusLaneTarget) {
        self.unresolved
            .push((edge.clone(), StatusLaneFinding::new(target, StatusLaneFindingKind::Missing)));
    }

    /// Records an edge with no usable current verdict, including fingerprint
    /// mismatch or absence.
    pub(super) fn verdict_absent(&mut self, edge: TestObligationEdgeId, target: StatusLaneTarget) {
        self.verdict_absent.push((
            edge.clone(),
            StatusLaneFinding::new(target, StatusLaneFindingKind::VerdictAbsent),
        ));
    }

    /// Returns all status-attributable unresolved findings for display.
    pub(super) fn status_findings(&self) -> Vec<StatusLaneFinding> {
        self.status_drifts
            .iter()
            .map(|(_, finding)| finding.clone())
            .chain(self.unresolved.iter().map(|(_, finding)| finding.clone()))
            .chain(self.verdict_absent.iter().map(|(_, finding)| finding.clone()))
            .collect()
    }
}

/// A parsed spec element (id + section + anchor text).
pub(super) struct SpecElement {
    id: String,
    section: SpecSectionKind,
    text: String,
}

/// Returns the bound tests for the obligation with `id`, if a `Fulfillment`
/// binding exists.
pub(super) fn fulfillment_tests<'a>(
    bindings: &'a TestBindingsDocument,
    id: &TestObligationId,
) -> Option<&'a [TestLocation]> {
    bindings.records().iter().find_map(|record| match record {
        TestBindingRecord::Fulfillment { obligation_id, tests } if obligation_id == id => {
            Some(tests.as_slice())
        }
        _ => None,
    })
}

/// Returns the bound tests for `edge`, if a `VoluntaryBinding` exists.
pub(super) fn voluntary_tests<'a>(
    bindings: &'a TestBindingsDocument,
    edge: &TestObligationEdgeId,
) -> Option<&'a [TestLocation]> {
    bindings.records().iter().find_map(|record| match record {
        TestBindingRecord::VoluntaryBinding { edge_id, tests } if edge_id == edge => {
            Some(tests.as_slice())
        }
        _ => None,
    })
}

/// Returns the waived reason for `edge`, if a `Waiver` binding exists.
pub(super) fn waived_reason(
    bindings: &TestBindingsDocument,
    edge: &TestObligationEdgeId,
) -> Option<WaivedReason> {
    bindings.records().iter().find_map(|record| match record {
        TestBindingRecord::Waiver { edge_id, reason } if edge_id == edge => Some(reason.clone()),
        _ => None,
    })
}

/// Returns every active catalogue ref edge that participates in check totality.
pub(super) fn active_cited_edges_from_catalogues(
    catalogues: &[LoadedCatalogueDocument],
) -> Result<Vec<TestObligationEdgeId>, ObligationCheckError> {
    let mut edges = Vec::new();
    for catalogue in catalogues {
        let document = catalogue.document();
        for (name, entry) in document.types() {
            if active_ref_action(entry.action()) {
                push_edges_for_spec_refs(&mut edges, name.as_str(), entry.spec_refs())?;
            }
        }
        for (name, entry) in document.traits() {
            if active_ref_action(entry.action()) {
                push_edges_for_spec_refs(&mut edges, name.as_str(), entry.spec_refs())?;
            }
        }
        for (path, entry) in document.functions() {
            if active_ref_action(entry.action()) {
                push_edges_for_spec_refs(&mut edges, &path.to_string(), entry.spec_refs())?;
            }
        }
    }
    Ok(edges)
}

/// `Add` / `Modify` entries own active ref edges; `Reference` / `Delete` do not.
fn active_ref_action(action: ItemAction) -> bool {
    matches!(action, ItemAction::Add | ItemAction::Modify)
}

/// Appends active ref edges for one catalogue entry, preserving first-seen order.
fn push_edges_for_spec_refs(
    edges: &mut Vec<TestObligationEdgeId>,
    entry_key: &str,
    refs: &[SpecRef],
) -> Result<(), ObligationCheckError> {
    let entry_key = CatalogueEntryKey::try_new(entry_key.to_owned())
        .map_err(|_| ObligationCheckError::InvalidCatalogueState(diag("empty catalogue key")))?;
    for spec_ref in refs {
        let anchor = TestObligationAnchorId::try_new(
            spec_ref.file.display().to_string(),
            spec_ref.anchor.as_ref().to_owned(),
        )
        .map_err(|_| ObligationCheckError::InvalidCatalogueState(diag("empty spec ref")))?;
        let edge = TestObligationEdgeId::new(entry_key.clone(), anchor);
        if !edges.iter().any(|candidate| candidate == &edge) {
            edges.push(edge);
        }
    }
    Ok(())
}

/// Returns whether `edge` is still produced by the current obligations.
pub(super) fn edge_is_derived(
    obligations: &ObligationsDocument,
    edge: &TestObligationEdgeId,
) -> bool {
    obligations.obligations().iter().any(|obligation| {
        obligation.id().entry_key() == edge.entry_key()
            && obligation.spec_refs().iter().any(|anchor| anchor == edge.anchor_id())
    })
}

/// Returns whether `edge` still belongs to the current edge universe.
pub(super) fn edge_is_known(
    obligations: &ObligationsDocument,
    cited_edges: &[TestObligationEdgeId],
    edge: &TestObligationEdgeId,
) -> bool {
    edge_is_derived(obligations, edge) || cited_edges.iter().any(|candidate| candidate == edge)
}

/// Synthesizes the cache obligation id used by `evaluate` for voluntary edges.
pub(super) fn synthetic_voluntary_obligation_id(edge: &TestObligationEdgeId) -> TestObligationId {
    TestObligationId::new(
        edge.entry_key().clone(),
        TestObligationKind::Logic,
        synthetic_voluntary_item_identifier(edge.anchor_id()),
    )
}

/// Builds the stable voluntary-binding item identifier from an anchor id.
fn synthetic_voluntary_item_identifier(
    anchor: &TestObligationAnchorId,
) -> TestObligationItemIdentifier {
    let mut text = format!("voluntary:{}", anchor.element_id());
    loop {
        match TestObligationItemIdentifier::try_new(text) {
            Ok(item) => return item,
            // Unreachable: the prefix guarantees non-empty; reset defensively.
            Err(_) => text = "voluntary:edge".to_owned(),
        }
    }
}

/// Builds a synthetic edge id for an orphaned `Fulfillment` binding's drift.
pub(super) fn synthetic_edge(obligation_id: &TestObligationId) -> TestObligationEdgeId {
    let mut element = format!("orphaned:{}", obligation_id.item_identifier().as_str());
    let anchor = loop {
        match TestObligationAnchorId::try_new("binding".to_owned(), element) {
            Ok(anchor) => break anchor,
            // Unreachable: the prefix guarantees non-empty; reset defensively.
            Err(_) => element = "orphaned:edge".to_owned(),
        }
    };
    TestObligationEdgeId::new(obligation_id.entry_key().clone(), anchor)
}

/// Returns the anchor text for `anchor`, or an empty string when unknown.
pub(super) fn anchor_text(
    spec_texts: &[(String, String)],
    anchor: &TestObligationAnchorId,
) -> String {
    spec_texts
        .iter()
        .find(|(id, _)| id == anchor.element_id())
        .map(|(_, text)| text.clone())
        .unwrap_or_default()
}

/// Projects the parsed elements into `(element_id, text)` pairs.
pub(super) fn anchor_texts(elements: &[SpecElement]) -> Vec<(String, String)> {
    elements.iter().map(|e| (e.id.clone(), e.text.clone())).collect()
}

/// Computes the uncited `AC` / `CN` findings from catalogues + spec elements.
pub(super) fn compute_uncited_from(
    catalogues: &[LoadedCatalogueDocument],
    elements: &[SpecElement],
) -> Vec<UncitedSpecElementFinding> {
    let documents: Vec<CatalogueDocument> =
        catalogues.iter().map(|catalogue| catalogue.document().clone()).collect();
    let cited = cited_anchor_ids(&documents);
    let mut findings = Vec::new();
    for element in elements {
        let is_ac_or_cn = matches!(
            element.section,
            SpecSectionKind::AcceptanceCriteria | SpecSectionKind::Constraint
        );
        if is_ac_or_cn && !cited.iter().any(|c| c == &element.id) {
            if let Ok(id) = SpecElementId::try_new(element.id.clone()) {
                findings.push(UncitedSpecElementFinding::new(id, element.section.clone()));
            }
        }
    }
    findings
}

/// Projects parsed spec document sections into the local uncited-finding view.
pub(super) fn spec_elements_from_document(spec: &SpecDocument) -> Vec<SpecElement> {
    let mut elements = Vec::new();
    push_requirements(&mut elements, spec.goal(), SpecSectionKind::Goal);
    push_requirements(&mut elements, spec.scope().in_scope(), SpecSectionKind::InScope);
    push_requirements(&mut elements, spec.scope().out_of_scope(), SpecSectionKind::OutOfScope);
    push_requirements(&mut elements, spec.constraints(), SpecSectionKind::Constraint);
    push_requirements(
        &mut elements,
        spec.acceptance_criteria(),
        SpecSectionKind::AcceptanceCriteria,
    );
    elements
}

/// Appends one parsed spec section to the local uncited-finding view.
fn push_requirements(
    elements: &mut Vec<SpecElement>,
    requirements: &[SpecRequirement],
    section: SpecSectionKind,
) {
    for requirement in requirements {
        elements.push(SpecElement {
            id: requirement.id().as_ref().to_owned(),
            section: section.clone(),
            text: requirement.text().to_owned(),
        });
    }
}
