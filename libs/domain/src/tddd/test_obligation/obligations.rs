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
use crate::tddd::semantic_verify::CatalogueEntryRef;
use crate::tddd::test_obligation::hashes::DeclarationHash;
use crate::tddd::test_obligation::ids::{
    TestObligationAnchorId, TestObligationBrief, TestObligationId,
};
use crate::tddd::test_obligation::vocab::TargetEntryRoleKind;

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
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::ContentHash;
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
    fn test_empty_obligations_document_is_valid() {
        let doc = ObligationsDocument::new(TrackId::try_new("empty-track").unwrap(), vec![]);
        assert!(doc.obligations().is_empty());
    }
}
