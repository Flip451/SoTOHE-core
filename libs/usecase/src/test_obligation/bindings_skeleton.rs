//! `bin/sotp test-obligation bindings-skeleton` — authoring helper.
//!
//! This read-only helper turns the generated `obligations.json` into a
//! schema-conformant `test-bindings.json` draft for implementers: exact
//! obligation ids with TODO placeholder test locations. It deliberately does
//! not persist `test-bindings.json` and does not evaluate verdicts; the
//! binding artifact remains implementer-authored, and the fail-closed codec
//! rejects the draft until every TODO placeholder is replaced.

use std::sync::Arc;

use domain::TrackId;
use domain::tddd::test_obligation::errors::ArtifactCodecError;
use domain::tddd::test_obligation::ports::ObligationsArtifactPort;

/// Command input for [`TestBindingsSkeletonApplicationService`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestBindingsSkeletonCommand {
    track_id: TrackId,
}

impl TestBindingsSkeletonCommand {
    /// Builds a skeleton command for one track.
    #[must_use]
    pub fn new(track_id: TrackId) -> Self {
        Self { track_id }
    }

    /// Returns the target track id.
    #[must_use]
    pub fn track_id(&self) -> &TrackId {
        &self.track_id
    }
}

/// One fulfillment draft record: the obligation id an implementer binds
/// tests to. Briefs and target entries stay in `obligations.json`; the
/// skeleton carries only what the final artifact schema needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestBindingsSkeletonRecord {
    entry_key: String,
    obligation_kind: String,
    item_identifier: String,
}

impl TestBindingsSkeletonRecord {
    /// Builds a fulfillment draft record.
    #[must_use]
    pub fn new(entry_key: String, obligation_kind: String, item_identifier: String) -> Self {
        Self { entry_key, obligation_kind, item_identifier }
    }

    /// Returns the obligation entry key.
    #[must_use]
    pub fn entry_key(&self) -> &str {
        &self.entry_key
    }

    /// Returns the obligation kind's canonical spelling.
    #[must_use]
    pub fn obligation_kind(&self) -> &str {
        &self.obligation_kind
    }

    /// Returns the obligation item identifier.
    #[must_use]
    pub fn item_identifier(&self) -> &str {
        &self.item_identifier
    }
}

/// Structured skeleton output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestBindingsSkeletonOutcome {
    track_id: TrackId,
    records: Vec<TestBindingsSkeletonRecord>,
}

impl TestBindingsSkeletonOutcome {
    /// Builds a skeleton outcome.
    #[must_use]
    pub fn new(track_id: TrackId, records: Vec<TestBindingsSkeletonRecord>) -> Self {
        Self { track_id, records }
    }

    /// Returns the track id.
    #[must_use]
    pub fn track_id(&self) -> &TrackId {
        &self.track_id
    }

    /// Returns the fulfillment draft records.
    #[must_use]
    pub fn records(&self) -> &[TestBindingsSkeletonRecord] {
        &self.records
    }
}

/// Failure of `test-obligation bindings-skeleton`.
#[derive(Debug)]
pub enum TestBindingsSkeletonError {
    /// `obligations.json` has not been materialized yet.
    ObligationsAbsent,
    /// Loading `obligations.json` failed.
    ArtifactLoad(ArtifactCodecError),
}

/// Primary port for producing a bindings skeleton.
pub trait TestBindingsSkeletonApplicationService {
    /// Builds a draft skeleton from `obligations.json`.
    ///
    /// # Errors
    ///
    /// Returns [`TestBindingsSkeletonError`] when `obligations.json` is absent
    /// or malformed.
    fn execute(
        &self,
        cmd: &TestBindingsSkeletonCommand,
    ) -> Result<TestBindingsSkeletonOutcome, TestBindingsSkeletonError>;
}

/// Interactor implementing [`TestBindingsSkeletonApplicationService`].
pub struct TestBindingsSkeletonInteractor {
    obligations_port: Arc<dyn ObligationsArtifactPort + Send + Sync>,
}

impl TestBindingsSkeletonInteractor {
    /// Builds the interactor over the obligations artifact port.
    #[must_use]
    pub fn new(obligations_port: Arc<dyn ObligationsArtifactPort + Send + Sync>) -> Self {
        Self { obligations_port }
    }
}

impl TestBindingsSkeletonApplicationService for TestBindingsSkeletonInteractor {
    fn execute(
        &self,
        cmd: &TestBindingsSkeletonCommand,
    ) -> Result<TestBindingsSkeletonOutcome, TestBindingsSkeletonError> {
        let Some(obligations) = self
            .obligations_port
            .load(cmd.track_id())
            .map_err(TestBindingsSkeletonError::ArtifactLoad)?
        else {
            return Err(TestBindingsSkeletonError::ObligationsAbsent);
        };

        // D1: an obligation that owns no anchors is not a fulfillment target.
        // Emitting a draft fulfillment would make `check` report a structural
        // orphan as soon as the implementer persisted the skeleton.
        let records = obligations
            .obligations()
            .iter()
            .filter(|obligation| !obligation.spec_refs().is_empty())
            .map(|obligation| {
                TestBindingsSkeletonRecord::new(
                    obligation.id().entry_key().as_str().to_owned(),
                    obligation.id().obligation_kind().as_kebab().to_owned(),
                    obligation.id().item_identifier().as_str().to_owned(),
                )
            })
            .collect();

        Ok(TestBindingsSkeletonOutcome::new(obligations.track_id().clone(), records))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use domain::ContentHash;
    use domain::tddd::catalogue_v2::roles::DataRole;
    use domain::tddd::semantic_verify::{
        CatalogueEntryKey, CatalogueEntryRef, CatalogueSectionKey,
    };
    use domain::tddd::test_obligation::hashes::DeclarationHash;
    use domain::tddd::test_obligation::ids::{
        DiagnosticMessage, TestObligationAnchorId, TestObligationBrief, TestObligationId,
        TestObligationItemIdentifier,
    };
    use domain::tddd::test_obligation::obligations::{ObligationsDocument, TestObligation};
    use domain::tddd::test_obligation::vocab::{TargetEntryRoleKind, TestObligationKind};

    struct StubObligations(Option<ObligationsDocument>);

    impl ObligationsArtifactPort for StubObligations {
        fn load(
            &self,
            _track_id: &TrackId,
        ) -> Result<Option<ObligationsDocument>, ArtifactCodecError> {
            Ok(self.0.clone())
        }

        fn save(&self, _doc: &ObligationsDocument) -> Result<(), DiagnosticMessage> {
            Ok(())
        }
    }

    fn track() -> TrackId {
        TrackId::try_new("my-track").unwrap()
    }

    fn obligation() -> TestObligation {
        obligation_with_spec_refs(vec![
            TestObligationAnchorId::try_new("spec.json".to_owned(), "IN-05".to_owned()).unwrap(),
        ])
    }

    fn obligation_with_spec_refs(spec_refs: Vec<TestObligationAnchorId>) -> TestObligation {
        let id = TestObligationId::new(
            CatalogueEntryKey::try_new("domain::User".to_owned()).unwrap(),
            TestObligationKind::Boundary,
            TestObligationItemIdentifier::try_new("invariant:non_empty".to_owned()).unwrap(),
        );
        let target = CatalogueEntryRef::new(
            "track/items/my-track/domain-types.json".to_owned(),
            CatalogueSectionKey::Types,
            CatalogueEntryKey::try_new("domain::User".to_owned()).unwrap(),
        );
        TestObligation::new(
            id,
            target,
            TargetEntryRoleKind::DataRole(DataRole::value_object()),
            TestObligationBrief::try_new("cover empty-name rejection".to_owned()).unwrap(),
            DeclarationHash::new(ContentHash::from_bytes([1; 32])),
            spec_refs,
        )
    }

    #[test]
    fn test_skeleton_maps_obligations_to_draft_records() {
        let service = TestBindingsSkeletonInteractor::new(Arc::new(StubObligations(Some(
            ObligationsDocument::new(track(), vec![obligation()]),
        ))));

        let outcome = service.execute(&TestBindingsSkeletonCommand::new(track())).unwrap();

        assert_eq!(outcome.track_id().as_ref(), "my-track");
        assert_eq!(outcome.records().len(), 1);
        let record = &outcome.records()[0];
        assert_eq!(record.entry_key(), "domain::User");
        assert_eq!(record.obligation_kind(), "boundary");
        assert_eq!(record.item_identifier(), "invariant:non_empty");
    }

    #[test]
    fn test_skeleton_omits_obligations_that_own_no_anchors() {
        let service = TestBindingsSkeletonInteractor::new(Arc::new(StubObligations(Some(
            ObligationsDocument::new(track(), vec![obligation_with_spec_refs(Vec::new())]),
        ))));

        let outcome = service.execute(&TestBindingsSkeletonCommand::new(track())).unwrap();

        assert!(outcome.records().is_empty());
    }

    #[test]
    fn test_skeleton_fails_when_obligations_absent() {
        let service = TestBindingsSkeletonInteractor::new(Arc::new(StubObligations(None)));

        let err = service.execute(&TestBindingsSkeletonCommand::new(track())).unwrap_err();

        assert!(matches!(err, TestBindingsSkeletonError::ObligationsAbsent));
    }

    struct FailingObligations;

    impl ObligationsArtifactPort for FailingObligations {
        fn load(
            &self,
            _track_id: &TrackId,
        ) -> Result<Option<ObligationsDocument>, ArtifactCodecError> {
            Err(ArtifactCodecError::Io(
                DiagnosticMessage::try_new("unreadable obligations artifact".to_owned()).unwrap(),
            ))
        }

        fn save(&self, _doc: &ObligationsDocument) -> Result<(), DiagnosticMessage> {
            Ok(())
        }
    }

    #[test]
    fn test_skeleton_maps_artifact_load_error_to_error_variant() {
        let service = TestBindingsSkeletonInteractor::new(Arc::new(FailingObligations));

        let err = service.execute(&TestBindingsSkeletonCommand::new(track())).unwrap_err();

        assert!(matches!(err, TestBindingsSkeletonError::ArtifactLoad(ArtifactCodecError::Io(_))));
    }
}
