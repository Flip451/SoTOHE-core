//! Derived-obligation value objects for the obligations artifact.
//!
//! [`TestObligation`] is a single obligation the derivation engine produced for a
//! catalogue entry: its stable [`TestObligationId`], the target entry it concerns
//! ([`CatalogueEntryRef`] + [`TargetEntryRoleKind`]), the implementer-facing
//! [`TestObligationBrief`], the [`DeclarationHash`] the obligation was derived
//! from, and the spec / ADR anchors it cites. [`ObligationsDocument`] is the
//! track-scoped collection persisted as the obligations artifact (IN-05 / IN-07 /
//! CN-11 / AC-03).

use crate::TrackId;
use crate::tddd::catalogue_v2::roles::ItemAction;
use crate::tddd::catalogue_v2::{MethodDeclaration, TraitEntry};
use crate::tddd::semantic_verify::CatalogueEntryRef;
use crate::tddd::test_obligation::errors::TestBindingConsistencyError;
use crate::tddd::test_obligation::hashes::DeclarationHash;
use crate::tddd::test_obligation::ids::{
    DiagnosticMessage, TestObligationAnchorId, TestObligationBrief, TestObligationEdgeId,
    TestObligationId, unavailable_diagnostic_message,
};
use crate::tddd::test_obligation::vocab::TargetEntryRoleKind;

/// Validates Add/Modify non-empty `spec_refs` and the parent-action combination
/// on a trait. Entry-level refs are not an inventory: method refs need not be a
/// subset of them and need not cover them.
///
/// # Errors
///
/// Returns a [`DiagnosticMessage`] when an Add/Modify method has empty
/// `spec_refs`, or a method on a Reference/Delete parent declares any.
pub fn validate_method_anchor_coverage(entry: &TraitEntry) -> Result<(), DiagnosticMessage> {
    validate_add_modify_methods_have_spec_refs(entry.methods())?;
    validate_parent_forbids_method_spec_refs(entry.action(), entry.methods())?;
    Ok(())
}

/// Validates that every Add/Modify method declares non-empty `spec_refs`.
///
/// # Errors
///
/// Returns a [`DiagnosticMessage`] when an Add or Modify method has empty
/// `spec_refs`.
pub fn validate_add_modify_methods_have_spec_refs(
    methods: &[MethodDeclaration],
) -> Result<(), DiagnosticMessage> {
    for method in methods {
        if matches!(method.action(), ItemAction::Add | ItemAction::Modify)
            && method.spec_refs().is_empty()
        {
            return Err(coverage_diag(&format!(
                "add/modify method '{}' must declare non-empty spec_refs",
                method.name().as_str(),
            )));
        }
    }
    Ok(())
}

/// Rejects non-empty method `spec_refs` when the parent is Reference or Delete.
///
/// Combined with [`validate_add_modify_methods_have_spec_refs`], this makes
/// Add/Modify methods undeclarable on those parents.
///
/// # Errors
///
/// Returns a [`DiagnosticMessage`] when a method on a Reference/Delete parent
/// declares any `spec_refs`.
pub fn validate_parent_forbids_method_spec_refs(
    parent: ItemAction,
    methods: &[MethodDeclaration],
) -> Result<(), DiagnosticMessage> {
    if !matches!(parent, ItemAction::Reference | ItemAction::Delete) {
        return Ok(());
    }
    for method in methods {
        if !method.spec_refs().is_empty() {
            return Err(coverage_diag(&format!(
                "method '{}' on a reference/delete parent must not declare spec_refs",
                method.name().as_str(),
            )));
        }
    }
    Ok(())
}

fn coverage_diag(detail: &str) -> DiagnosticMessage {
    DiagnosticMessage::try_new(detail.to_owned())
        .unwrap_or_else(|_| unavailable_diagnostic_message())
}

/// A single derived test obligation for one catalogue entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestObligation {
    id: TestObligationId,
    target_entry: CatalogueEntryRef,
    target_role: TargetEntryRoleKind,
    brief: TestObligationBrief,
    declaration_hash: DeclarationHash,
    spec_refs: Vec<TestObligationAnchorId>,
}

impl TestObligation {
    /// Builds a [`TestObligation`] from its derived components.
    #[must_use]
    pub fn new(
        id: TestObligationId,
        target_entry: CatalogueEntryRef,
        target_role: TargetEntryRoleKind,
        brief: TestObligationBrief,
        declaration_hash: DeclarationHash,
        spec_refs: Vec<TestObligationAnchorId>,
    ) -> Self {
        Self { id, target_entry, target_role, brief, declaration_hash, spec_refs }
    }

    /// Returns the obligation's stable identity.
    #[must_use]
    pub fn id(&self) -> &TestObligationId {
        &self.id
    }

    /// Returns the catalogue entry this obligation targets.
    #[must_use]
    pub fn target_entry(&self) -> &CatalogueEntryRef {
        &self.target_entry
    }

    /// Returns the resolved role of the target entry.
    #[must_use]
    pub fn target_role(&self) -> &TargetEntryRoleKind {
        &self.target_role
    }

    /// Returns the implementer-facing brief.
    #[must_use]
    pub fn brief(&self) -> &TestObligationBrief {
        &self.brief
    }

    /// Returns the declaration hash the obligation was derived from.
    #[must_use]
    pub fn declaration_hash(&self) -> &DeclarationHash {
        &self.declaration_hash
    }

    /// Returns the spec / ADR anchors this obligation cites.
    #[must_use]
    pub fn spec_refs(&self) -> &[TestObligationAnchorId] {
        &self.spec_refs
    }

    /// Returns whether this obligation owns `edge_id`: the catalogue entry
    /// keys match and the edge's anchor is one of the cited spec references.
    #[must_use]
    pub fn owns_edge(&self, edge_id: &TestObligationEdgeId) -> bool {
        self.id.entry_key() == edge_id.entry_key()
            && self.spec_refs.iter().any(|spec_ref| {
                spec_ref.file_path() == edge_id.anchor_id().file_path()
                    && spec_ref.element_id() == edge_id.anchor_id().element_id()
            })
    }
}

/// Ownership resolution for one edge, keeping the no-owner, unique-owner, and
/// multiple-owner cases distinct.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EdgeOwnership<'a> {
    /// No obligation owns the edge.
    None,
    /// Exactly one obligation owns the edge.
    Unique(&'a TestObligation),
    /// Several obligations own the edge; each (edge × obligation) pair is
    /// resolved independently by the gates.
    Multiple(Vec<&'a TestObligation>),
}

/// Track-scoped collection of derived obligations (the obligations artifact).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObligationsDocument {
    track_id: TrackId,
    obligations: Vec<TestObligation>,
}

impl ObligationsDocument {
    /// Builds an [`ObligationsDocument`] for `track_id`.
    #[must_use]
    pub fn new(track_id: TrackId, obligations: Vec<TestObligation>) -> Self {
        Self { track_id, obligations }
    }

    /// Returns the track this document was derived for.
    #[must_use]
    pub fn track_id(&self) -> &TrackId {
        &self.track_id
    }

    /// Returns the derived obligations.
    #[must_use]
    pub fn obligations(&self) -> &[TestObligation] {
        &self.obligations
    }

    /// Resolves the ownership of `edge_id`, distinguishing the no-owner,
    /// unique-owner, and multiple-owner cases so consumers can never conflate
    /// "no owner" with "several owners requiring per-pair handling".
    #[must_use]
    pub fn edge_ownership(&self, edge_id: &TestObligationEdgeId) -> EdgeOwnership<'_> {
        let owners = self.owners_of_edge(edge_id);
        match owners.as_slice() {
            [] => EdgeOwnership::None,
            [owner] => EdgeOwnership::Unique(owner),
            _ => EdgeOwnership::Multiple(owners),
        }
    }

    /// Returns EVERY obligation owning `edge_id`, in document order.
    ///
    /// Several obligations (e.g. one per trait method) can share an entry key
    /// and cite the same anchor; each (edge × obligation) pair is resolved
    /// independently by the gates.
    #[must_use]
    pub fn owners_of_edge(&self, edge_id: &TestObligationEdgeId) -> Vec<&TestObligation> {
        self.obligations.iter().filter(|obligation| obligation.owns_edge(edge_id)).collect()
    }

    /// Rejects a voluntary binding recorded for an edge that owns a derived obligation.
    ///
    /// # Errors
    ///
    /// Returns [`TestBindingConsistencyError::VoluntaryBindingOwnsDerivedObligation`]
    /// when `edge_id` is owned by one or more derived obligations.
    pub fn validate_voluntary_binding(
        &self,
        edge_id: &TestObligationEdgeId,
    ) -> Result<(), TestBindingConsistencyError> {
        if self.owners_of_edge(edge_id).is_empty() {
            return Ok(());
        }
        Err(TestBindingConsistencyError::VoluntaryBindingOwnsDerivedObligation {
            edge_id: edge_id.clone(),
        })
    }

    /// Diagnoses whether this persisted obligation document is stale against the
    /// current derivation, comparing the complete obligation multiset (IN-08).
    ///
    /// Exact obligations match irrespective of ordering and with duplicate
    /// occurrences counted. Remaining obligations with the same stable ID are
    /// changes; the rest are additions or removals required to make this
    /// document match `expected`.
    #[must_use]
    pub fn staleness_against(&self, expected: &ObligationsDocument) -> Option<DiagnosticMessage> {
        let mut unmatched_expected = expected.obligations.iter().collect::<Vec<_>>();
        let mut unmatched_current = Vec::new();
        let mut differing_obligations = Vec::new();

        for current in &self.obligations {
            if let Some(index) = unmatched_expected.iter().position(|expected| *expected == current)
            {
                unmatched_expected.swap_remove(index);
            } else {
                unmatched_current.push(current);
            }
        }

        let mut changed = 0;
        let mut added = 0;
        for expected in unmatched_expected {
            if let Some(index) =
                unmatched_current.iter().position(|current| current.id() == expected.id())
            {
                let current = unmatched_current.swap_remove(index);
                push_stale_detail(
                    &mut differing_obligations,
                    format!(
                        "{} ({})",
                        obligation_id_label(current),
                        changed_field_kinds(current, expected).join(", ")
                    ),
                );
                changed += 1;
            } else {
                push_stale_detail(
                    &mut differing_obligations,
                    format!("{} (missing_from_artifact)", obligation_id_label(expected)),
                );
                added += 1;
            }
        }

        let removed = unmatched_current.len();
        for current in unmatched_current {
            push_stale_detail(
                &mut differing_obligations,
                format!("{} (unexpected_in_artifact)", obligation_id_label(current)),
            );
        }
        if added == 0 && removed == 0 && changed == 0 {
            return None;
        }

        Some(stale_obligations_detail(added, removed, changed, &differing_obligations))
    }
}

/// Number of obligation differences included in the stale-artifact diagnostic.
const MAX_STALE_OBLIGATION_DETAILS: usize = 5;

/// Retains a bounded sample of stale-obligation differences for diagnostics.
fn push_stale_detail(details: &mut Vec<String>, detail: String) {
    if details.len() < MAX_STALE_OBLIGATION_DETAILS {
        details.push(detail);
    }
}

/// Formats the stable identifier of an obligation for a stale-artifact diagnostic.
fn obligation_id_label(obligation: &TestObligation) -> String {
    format!(
        "{}:{:?}:{}",
        obligation.id().entry_key().as_str(),
        obligation.id().obligation_kind(),
        obligation.id().item_identifier().as_str()
    )
}

/// Reports which non-identity fields differ between obligations with the same id.
fn changed_field_kinds(current: &TestObligation, expected: &TestObligation) -> Vec<&'static str> {
    let mut fields = Vec::new();
    if current.target_entry().file_path != expected.target_entry().file_path {
        fields.push("target_entry.file_path");
    }
    if current.target_entry().section_key != expected.target_entry().section_key {
        fields.push("target_entry.section_key");
    }
    if current.target_entry().entry_key != expected.target_entry().entry_key {
        fields.push("target_entry.entry_key");
    }
    if current.target_role() != expected.target_role() {
        fields.push("target_role");
    }
    if current.brief() != expected.brief() {
        fields.push("brief");
    }
    if current.declaration_hash() != expected.declaration_hash() {
        fields.push("declaration_hash");
    }
    if current.spec_refs() != expected.spec_refs() {
        fields.push("spec_refs");
    }
    fields
}

/// Builds the non-empty stale-obligations diagnostic for the IN-08 gate.
#[must_use]
fn stale_obligations_detail(
    added: usize,
    removed: usize,
    changed: usize,
    differing_obligations: &[String],
) -> DiagnosticMessage {
    let detail_count = added + removed + changed;
    let detail_suffix = if differing_obligations.is_empty() {
        String::new()
    } else {
        format!(
            "; differing_obligations=[{}] (showing {} of {detail_count})",
            differing_obligations.join("; "),
            differing_obligations.len(),
        )
    };
    let mut detail = format!(
        "stale obligations artifact: added={added}, removed={removed}, changed={changed}{detail_suffix}; rerun test-obligation derive"
    );
    loop {
        match DiagnosticMessage::try_new(detail) {
            Ok(detail) => return detail,
            // The generated text is non-empty. Retain the defensive fallback
            // so diagnostics remain panic-free if that invariant changes.
            Err(_) => detail = "stale obligations artifact".to_owned(),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::ContentHash;
    use crate::plan_ref::SpecRef;
    use crate::tddd::catalogue_v2::roles::DataRole;
    use crate::tddd::semantic_verify::{CatalogueEntryKey, CatalogueSectionKey};
    use crate::tddd::test_obligation::ids::TestObligationItemIdentifier;
    use crate::tddd::test_obligation::vocab::TestObligationKind;

    fn sample_obligation() -> TestObligation {
        let id = TestObligationId::new(
            CatalogueEntryKey::try_new("domain::User".to_owned()).unwrap(),
            TestObligationKind::Boundary,
            TestObligationItemIdentifier::try_new("invariant:non_empty".to_owned()).unwrap(),
        );
        let target_entry = CatalogueEntryRef::new(
            "track/items/x/domain-types.json".to_owned(),
            CatalogueSectionKey::Types,
            CatalogueEntryKey::try_new("domain::User".to_owned()).unwrap(),
        );
        TestObligation::new(
            id,
            target_entry,
            TargetEntryRoleKind::DataRole(DataRole::value_object()),
            TestObligationBrief::try_new("cover empty-name rejection".to_owned()).unwrap(),
            DeclarationHash::new(ContentHash::from_bytes([3u8; 32])),
            vec![
                TestObligationAnchorId::try_new("spec.json".to_owned(), "IN-05".to_owned())
                    .unwrap(),
            ],
        )
    }

    fn edge(entry_key: &str, anchor: &str) -> TestObligationEdgeId {
        TestObligationEdgeId::new(
            CatalogueEntryKey::try_new(entry_key.to_owned()).unwrap(),
            TestObligationAnchorId::try_new("spec.json".to_owned(), anchor.to_owned()).unwrap(),
        )
    }

    fn obligation_with_id(item_identifier: &str) -> TestObligation {
        let mut obligation = sample_obligation();
        obligation.id = TestObligationId::new(
            CatalogueEntryKey::try_new("domain::User".to_owned()).unwrap(),
            TestObligationKind::Boundary,
            TestObligationItemIdentifier::try_new(item_identifier.to_owned()).unwrap(),
        );
        obligation
    }

    #[test]
    fn test_obligation_exposes_all_components() {
        let obligation = sample_obligation();
        assert_eq!(obligation.id().entry_key().as_str(), "domain::User");
        assert_eq!(obligation.target_entry().entry_key.as_str(), "domain::User");
        assert_eq!(
            obligation.target_role(),
            &TargetEntryRoleKind::DataRole(DataRole::value_object())
        );
        assert_eq!(obligation.brief().as_str(), "cover empty-name rejection");
        assert_eq!(obligation.declaration_hash().as_hash(), &ContentHash::from_bytes([3u8; 32]));
        assert_eq!(obligation.spec_refs().len(), 1);
        assert_eq!(
            obligation.spec_refs().first().map(TestObligationAnchorId::element_id),
            Some("IN-05")
        );
    }

    #[test]
    fn test_obligations_document_round_trips() {
        let track_id = TrackId::try_new("my-track").unwrap();
        let doc = ObligationsDocument::new(track_id.clone(), vec![sample_obligation()]);
        assert_eq!(doc.track_id(), &track_id);
        assert_eq!(doc.obligations().len(), 1);
    }

    #[test]
    fn test_owns_edge_requires_matching_entry_key_and_cited_anchor() {
        let obligation = sample_obligation();

        assert!(obligation.owns_edge(&edge("domain::User", "IN-05")));
        assert!(
            !obligation.owns_edge(&edge("domain::Other", "IN-05")),
            "a different entry key must not own the edge"
        );
        assert!(
            !obligation.owns_edge(&edge("domain::User", "IN-06")),
            "an anchor absent from spec_refs must not own the edge"
        );
    }

    #[test]
    fn test_owners_of_edge_returns_every_owner_in_document_order() {
        let first = obligation_with_id("invariant:first");
        let second = obligation_with_id("invariant:second");
        let doc = ObligationsDocument::new(
            TrackId::try_new("my-track").unwrap(),
            vec![first.clone(), second.clone()],
        );

        let owners = doc.owners_of_edge(&edge("domain::User", "IN-05"));
        assert_eq!(
            owners.iter().map(|o| o.id().clone()).collect::<Vec<_>>(),
            vec![first.id().clone(), second.id().clone()],
            "every obligation sharing the entry key and anchor owns the edge"
        );
        assert!(doc.owners_of_edge(&edge("domain::User", "IN-06")).is_empty());
    }

    #[test]
    fn test_validate_voluntary_binding_with_derived_owner_returns_consistency_error() {
        let document = ObligationsDocument::new(
            TrackId::try_new("my-track").unwrap(),
            vec![sample_obligation()],
        );
        let edge_id = edge("domain::User", "IN-05");

        assert!(matches!(
            document.validate_voluntary_binding(&edge_id),
            Err(TestBindingConsistencyError::VoluntaryBindingOwnsDerivedObligation { edge_id: actual })
                if actual == edge_id
        ));
    }

    #[test]
    fn test_validate_voluntary_binding_without_derived_owner_succeeds() {
        let document = ObligationsDocument::new(
            TrackId::try_new("my-track").unwrap(),
            vec![sample_obligation()],
        );

        assert!(document.validate_voluntary_binding(&edge("domain::User", "IN-06")).is_ok());
    }

    #[test]
    fn test_empty_obligations_document_is_valid() {
        let doc = ObligationsDocument::new(TrackId::try_new("empty-track").unwrap(), vec![]);
        assert!(doc.obligations().is_empty());
    }

    #[test]
    fn test_edge_ownership_unique_owner_returns_unique() {
        let owner = sample_obligation();
        let owner_id = owner.id().clone();
        let document = ObligationsDocument::new(TrackId::try_new("my-track").unwrap(), vec![owner]);

        match document.edge_ownership(&edge("domain::User", "IN-05")) {
            EdgeOwnership::Unique(resolved) => assert_eq!(resolved.id(), &owner_id),
            other => panic!("a single owner must resolve as Unique, got {other:?}"),
        }
    }

    #[test]
    fn test_edge_ownership_unmatched_entry_returns_none() {
        let document = ObligationsDocument::new(
            TrackId::try_new("my-track").unwrap(),
            vec![sample_obligation()],
        );

        assert_eq!(document.edge_ownership(&edge("domain::Account", "IN-05")), EdgeOwnership::None);
    }

    #[test]
    fn test_edge_ownership_multiple_owners_returns_multiple() {
        let first = obligation_with_id("invariant:first");
        let second = obligation_with_id("invariant:second");
        let document = ObligationsDocument::new(
            TrackId::try_new("my-track").unwrap(),
            vec![first.clone(), second.clone()],
        );

        match document.edge_ownership(&edge("domain::User", "IN-05")) {
            EdgeOwnership::Multiple(owners) => assert_eq!(
                owners.iter().map(|o| o.id().clone()).collect::<Vec<_>>(),
                vec![first.id().clone(), second.id().clone()],
                "multiple owners are reported distinctly, never conflated with no-owner"
            ),
            other => panic!("shared ownership must resolve as Multiple, got {other:?}"),
        }
    }

    #[test]
    fn test_edge_ownership_anchor_not_cited_returns_none() {
        let document = ObligationsDocument::new(
            TrackId::try_new("my-track").unwrap(),
            vec![sample_obligation()],
        );

        assert_eq!(document.edge_ownership(&edge("domain::User", "IN-06")), EdgeOwnership::None);
    }

    #[test]
    fn test_obligations_document_staleness_against_identical_returns_none() {
        let current = ObligationsDocument::new(
            TrackId::try_new("my-track").unwrap(),
            vec![sample_obligation()],
        );
        let expected = current.clone();

        assert_eq!(current.staleness_against(&expected), None);
    }

    #[test]
    fn test_obligations_document_staleness_against_added_returns_added_detail() {
        let current = ObligationsDocument::new(TrackId::try_new("my-track").unwrap(), vec![]);
        let expected = ObligationsDocument::new(
            TrackId::try_new("my-track").unwrap(),
            vec![sample_obligation()],
        );

        let detail = current.staleness_against(&expected).unwrap();

        assert!(detail.as_str().contains("added=1"));
        assert!(detail.as_str().contains("removed=0"));
        assert!(detail.as_str().contains("changed=0"));
    }

    #[test]
    fn test_obligations_document_staleness_against_removed_returns_removed_detail() {
        let current = ObligationsDocument::new(
            TrackId::try_new("my-track").unwrap(),
            vec![sample_obligation()],
        );
        let expected = ObligationsDocument::new(TrackId::try_new("my-track").unwrap(), vec![]);

        let detail = current.staleness_against(&expected).unwrap();

        assert!(detail.as_str().contains("added=0"));
        assert!(detail.as_str().contains("removed=1"));
        assert!(detail.as_str().contains("changed=0"));
    }

    #[test]
    fn test_obligations_document_staleness_against_changed_target_entry_reports_changed() {
        let current = ObligationsDocument::new(
            TrackId::try_new("my-track").unwrap(),
            vec![sample_obligation()],
        );
        let mut changed = sample_obligation();
        changed.target_entry = CatalogueEntryRef::new(
            "track/items/x/usecase-types.json".to_owned(),
            CatalogueSectionKey::Types,
            CatalogueEntryKey::try_new("domain::User".to_owned()).unwrap(),
        );
        let expected =
            ObligationsDocument::new(TrackId::try_new("my-track").unwrap(), vec![changed]);

        let detail = current.staleness_against(&expected).unwrap();

        assert!(detail.as_str().contains("changed=1"));
        assert!(
            detail
                .as_str()
                .contains("domain::User:Boundary:invariant:non_empty (target_entry.file_path)")
        );
    }

    #[test]
    fn test_obligations_document_staleness_against_changed_target_role_reports_changed() {
        let current = ObligationsDocument::new(
            TrackId::try_new("my-track").unwrap(),
            vec![sample_obligation()],
        );
        let mut changed = sample_obligation();
        changed.target_role = TargetEntryRoleKind::DataRole(DataRole::entity().unwrap());
        let expected =
            ObligationsDocument::new(TrackId::try_new("my-track").unwrap(), vec![changed]);

        let detail = current.staleness_against(&expected).unwrap();

        assert!(detail.as_str().contains("changed=1"));
        assert!(detail.as_str().contains("target_role"));
    }

    #[test]
    fn test_obligations_document_staleness_against_changed_brief_reports_changed() {
        let current = ObligationsDocument::new(
            TrackId::try_new("my-track").unwrap(),
            vec![sample_obligation()],
        );
        let mut changed = sample_obligation();
        changed.brief =
            TestObligationBrief::try_new("cover name normalization".to_owned()).unwrap();
        let expected =
            ObligationsDocument::new(TrackId::try_new("my-track").unwrap(), vec![changed]);

        let detail = current.staleness_against(&expected).unwrap();

        assert!(detail.as_str().contains("changed=1"));
        assert!(detail.as_str().contains("brief"));
    }

    #[test]
    fn test_obligations_document_staleness_against_changed_declaration_hash_reports_changed() {
        let current = ObligationsDocument::new(
            TrackId::try_new("my-track").unwrap(),
            vec![sample_obligation()],
        );
        let mut changed = sample_obligation();
        changed.declaration_hash = DeclarationHash::new(ContentHash::from_bytes([4u8; 32]));
        let expected =
            ObligationsDocument::new(TrackId::try_new("my-track").unwrap(), vec![changed]);

        let detail = current.staleness_against(&expected).unwrap();

        assert!(detail.as_str().contains("changed=1"));
        assert!(detail.as_str().contains("declaration_hash"));
    }

    #[test]
    fn test_obligations_document_staleness_against_changed_spec_refs_reports_changed() {
        let current = ObligationsDocument::new(
            TrackId::try_new("my-track").unwrap(),
            vec![sample_obligation()],
        );
        let mut changed = sample_obligation();
        changed.spec_refs = vec![
            TestObligationAnchorId::try_new("spec.json".to_owned(), "IN-06".to_owned()).unwrap(),
        ];
        let expected =
            ObligationsDocument::new(TrackId::try_new("my-track").unwrap(), vec![changed]);

        let detail = current.staleness_against(&expected).unwrap();

        assert!(detail.as_str().contains("changed=1"));
    }

    #[test]
    fn test_obligations_document_staleness_against_duplicate_reports_removed_detail() {
        let duplicate = sample_obligation();
        let current = ObligationsDocument::new(
            TrackId::try_new("my-track").unwrap(),
            vec![duplicate.clone(), duplicate],
        );
        let expected = ObligationsDocument::new(
            TrackId::try_new("my-track").unwrap(),
            vec![sample_obligation()],
        );

        let detail = current.staleness_against(&expected).unwrap();

        assert!(detail.as_str().contains("added=0"));
        assert!(detail.as_str().contains("removed=1"));
        assert!(detail.as_str().contains("changed=0"));
    }

    #[test]
    fn test_obligations_document_staleness_detail_is_bounded_with_total_count() {
        let current = ObligationsDocument::new(TrackId::try_new("my-track").unwrap(), vec![]);
        let expected = ObligationsDocument::new(
            TrackId::try_new("my-track").unwrap(),
            (0..6).map(|index| obligation_with_id(&format!("invariant:example-{index}"))).collect(),
        );

        let detail = current.staleness_against(&expected).unwrap();

        assert!(detail.as_str().contains("added=6"));
        assert!(detail.as_str().contains("showing 5 of 6"));
        assert!(detail.as_str().contains("missing_from_artifact"));
    }

    fn method(name: &str, action: ItemAction, spec_refs: Vec<SpecRef>) -> MethodDeclaration {
        MethodDeclaration::new(
            crate::tddd::catalogue_v2::MethodName::new(name).unwrap(),
            Some(crate::tddd::catalogue_v2::roles::SelfReceiver::SharedRef),
            vec![],
            crate::tddd::catalogue_v2::TypeRef::new("()").unwrap(),
            false,
            false,
            vec![],
            vec![],
            spec_refs,
            action,
            None,
        )
    }

    fn spec_ref(anchor: &str) -> SpecRef {
        SpecRef::new(
            std::path::PathBuf::from("track/items/x/spec.json"),
            crate::SpecElementId::try_new(anchor).unwrap(),
        )
    }

    #[test]
    fn test_add_modify_methods_without_spec_refs_are_rejected() {
        assert!(
            validate_add_modify_methods_have_spec_refs(&[method("load", ItemAction::Add, vec![],)])
                .is_err()
        );
        assert!(
            validate_add_modify_methods_have_spec_refs(&[method(
                "load",
                ItemAction::Modify,
                vec![],
            )])
            .is_err()
        );
    }

    #[test]
    fn test_add_method_with_spec_refs_on_reference_parent_is_rejected() {
        let entry = crate::tddd::catalogue_v2::TraitEntry::new(
            ItemAction::Reference,
            crate::tddd::catalogue_v2::roles::ContractRole::SecondaryPort,
            vec![method("load", ItemAction::Add, vec![spec_ref("IN-99")])],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            crate::tddd::catalogue_v2::ModulePath::root(),
            None,
            vec![spec_ref("IN-01")],
            vec![],
        );
        assert!(validate_method_anchor_coverage(&entry).is_err());
    }

    #[test]
    fn test_reference_method_need_not_cover_entry_anchors() {
        let entry = crate::tddd::catalogue_v2::TraitEntry::new(
            ItemAction::Add,
            crate::tddd::catalogue_v2::roles::ContractRole::SecondaryPort,
            vec![method("load", ItemAction::Reference, vec![spec_ref("IN-01")])],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            crate::tddd::catalogue_v2::ModulePath::root(),
            None,
            vec![spec_ref("IN-01")],
            vec![],
        );
        assert!(validate_method_anchor_coverage(&entry).is_ok());
    }

    #[test]
    fn test_method_refs_outside_entry_set_are_accepted() {
        let entry = crate::tddd::catalogue_v2::TraitEntry::new(
            ItemAction::Add,
            crate::tddd::catalogue_v2::roles::ContractRole::SecondaryPort,
            vec![method("load", ItemAction::Add, vec![spec_ref("IN-02")])],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            crate::tddd::catalogue_v2::ModulePath::root(),
            None,
            vec![spec_ref("IN-01")],
            vec![],
        );
        assert!(validate_method_anchor_coverage(&entry).is_ok());
    }

    #[test]
    fn test_method_refs_that_do_not_cover_entry_set_are_accepted() {
        let entry = crate::tddd::catalogue_v2::TraitEntry::new(
            ItemAction::Add,
            crate::tddd::catalogue_v2::roles::ContractRole::SecondaryPort,
            vec![
                method("load", ItemAction::Add, vec![spec_ref("IN-01")]),
                method("save", ItemAction::Add, vec![spec_ref("IN-01")]),
            ],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            crate::tddd::catalogue_v2::ModulePath::root(),
            None,
            vec![spec_ref("IN-01"), spec_ref("AC-01")],
            vec![],
        );
        assert!(validate_method_anchor_coverage(&entry).is_ok());
    }

    #[test]
    fn test_reference_method_without_spec_refs_is_accepted() {
        assert!(
            validate_add_modify_methods_have_spec_refs(&[method(
                "load",
                ItemAction::Reference,
                vec![],
            )])
            .is_ok()
        );
    }

    #[test]
    fn test_add_method_with_spec_refs_is_accepted() {
        assert!(
            validate_add_modify_methods_have_spec_refs(&[method(
                "load",
                ItemAction::Add,
                vec![spec_ref("IN-14")],
            )])
            .is_ok()
        );
    }

    #[test]
    fn test_delete_method_without_spec_refs_is_accepted() {
        assert!(
            validate_add_modify_methods_have_spec_refs(&[method(
                "load",
                ItemAction::Delete,
                vec![],
            )])
            .is_ok()
        );
    }

    #[test]
    fn test_empty_add_method_is_rejected_even_when_sibling_has_spec_refs() {
        assert!(
            validate_add_modify_methods_have_spec_refs(&[
                method("save", ItemAction::Add, vec![spec_ref("IN-14")]),
                method("load", ItemAction::Add, vec![]),
            ])
            .is_err()
        );
    }

    #[test]
    fn test_reference_parent_rejects_method_spec_refs() {
        assert!(
            validate_parent_forbids_method_spec_refs(
                ItemAction::Reference,
                &[method("load", ItemAction::Reference, vec![spec_ref("IN-01")])],
            )
            .is_err()
        );
    }

    #[test]
    fn test_delete_parent_rejects_method_spec_refs() {
        assert!(
            validate_parent_forbids_method_spec_refs(
                ItemAction::Delete,
                &[method("load", ItemAction::Delete, vec![spec_ref("IN-01")])],
            )
            .is_err()
        );
    }

    #[test]
    fn test_reference_parent_accepts_empty_method_spec_refs() {
        assert!(
            validate_parent_forbids_method_spec_refs(
                ItemAction::Reference,
                &[method("load", ItemAction::Reference, vec![])],
            )
            .is_ok()
        );
    }

    #[test]
    fn test_add_parent_still_allows_method_spec_refs() {
        assert!(
            validate_parent_forbids_method_spec_refs(
                ItemAction::Add,
                &[method("load", ItemAction::Add, vec![spec_ref("IN-14")])],
            )
            .is_ok()
        );
    }
}
