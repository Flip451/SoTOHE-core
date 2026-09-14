//! Unit tests for [`super::TestObligationResultsInteractor`] (T019).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::catalogue_document_loader::AttestedCatalogueDocumentLoaderPort;
use crate::test_obligation::ports::ObligationFulfillmentCachePort;
use domain::SpecDocumentLoaderPort;
use domain::tddd::LayerId;
use domain::tddd::catalogue_v2::catalogue_impl_signals_ports::CatalogueDocumentLoaderError;
use domain::tddd::catalogue_v2::roles::{DataRole, ItemAction};
use domain::tddd::catalogue_v2::{
    AttestedCatalogueDocument, CatalogueDocument, CrateName, ModulePath, StructKind, StructShape,
    TypeEntry, TypeKindV2,
};
use domain::tddd::semantic_verify::{
    CatalogueEntryKey, CatalogueEntryRef, CatalogueSectionKey, SpecElementRef, SpecSectionKind,
};
use domain::tddd::test_obligation::binding::{
    NonEmptyTestLocations, TestBindingRecord, TestBindingsDocument, TestLocation,
};
use domain::tddd::test_obligation::drift::{EdgeResolutionOutcome, EdgeVerdictRecord};
use domain::tddd::test_obligation::errors::TestSourceScanError;
use domain::tddd::test_obligation::errors::{
    ArtifactCodecError, ObligationResultsError, VerifyCacheError,
};
use domain::tddd::test_obligation::hashes::VerifierPromptFingerprint;
use domain::tddd::test_obligation::hashes::{
    BoundTestsSetHash, DeclarationHash, ObligationResponsibilityHash, SpecElementHash,
    WaivedReasonHash,
};
use domain::tddd::test_obligation::ids::{
    DiagnosticMessage, TestFunctionName, TestModulePath, TestObligationAnchorId,
    TestObligationBrief, TestObligationEdgeId, TestObligationId, TestObligationItemIdentifier,
    WaivedReason,
};
use domain::tddd::test_obligation::obligations::{ObligationsDocument, TestObligation};
use domain::tddd::test_obligation::ports::{
    ObligationsArtifactPort, TestBindingsArtifactPort, TestSourceScannerPort, WaiverCachePort,
};
use domain::tddd::test_obligation::verdict::{
    ObligationFulfillmentCacheDocument, ObligationFulfillmentCacheEntry,
    ObligationFulfillmentCacheEntryState, ObligationFulfillmentCacheKey,
    ObligationFulfillmentVerdict, WaiverCacheDocument, WaiverCacheEntry, WaiverCacheKey,
    WaiverVerdict,
};
use domain::tddd::test_obligation::vocab::{
    FulfillmentFailCategory, TargetEntryRoleKind, TestObligationKind,
};
use domain::{
    ContentHash, EvidenceCitation, SpecDocument, SpecDocumentLoadError, SpecElementId, SpecRef,
    SpecRequirement, SpecScope, TaskId, TaskStatusKind, TrackId,
};

use super::{
    TestObligationChainLabel, TestObligationResultsApplicationService,
    TestObligationResultsCommand, TestObligationResultsInteractor, TestObligationResultsOutput,
    TestObligationStatusLaneSummary,
};
use crate::pre_review_gate::{
    ImplPlanReadError, ImplPlanReaderPort, TaskContractReadError, TaskContractReaderPort,
};
use crate::test_obligation::{
    declaration_with_obligation_context, declaration_with_obligation_item,
    obligation_declaration_text, sha256_content_hash,
};
use domain::task_contract::{ContractedEntryRef, TaskContractDocument};

// ---------------------------------------------------------------------------
// Test doubles
// ---------------------------------------------------------------------------

struct StubObligations(Option<ObligationsDocument>);
impl ObligationsArtifactPort for StubObligations {
    fn load(&self, _track_id: &TrackId) -> Result<Option<ObligationsDocument>, ArtifactCodecError> {
        Ok(self.0.clone())
    }
    fn save(&self, _doc: &ObligationsDocument) -> Result<(), DiagnosticMessage> {
        Ok(())
    }
}

struct StubBindings(Option<TestBindingsDocument>);
impl TestBindingsArtifactPort for StubBindings {
    fn load(
        &self,
        _track_id: &TrackId,
    ) -> Result<Option<TestBindingsDocument>, ArtifactCodecError> {
        Ok(self.0.clone())
    }
    fn save(&self, _doc: &TestBindingsDocument) -> Result<(), DiagnosticMessage> {
        Ok(())
    }
}

struct FailingObligations {
    error: fn() -> ArtifactCodecError,
}

impl ObligationsArtifactPort for FailingObligations {
    fn load(&self, _track_id: &TrackId) -> Result<Option<ObligationsDocument>, ArtifactCodecError> {
        Err((self.error)())
    }

    fn save(&self, _doc: &ObligationsDocument) -> Result<(), DiagnosticMessage> {
        Ok(())
    }
}

struct FailingBindings {
    error: fn() -> ArtifactCodecError,
}

impl TestBindingsArtifactPort for FailingBindings {
    fn load(
        &self,
        _track_id: &TrackId,
    ) -> Result<Option<TestBindingsDocument>, ArtifactCodecError> {
        Err((self.error)())
    }

    fn save(&self, _doc: &TestBindingsDocument) -> Result<(), DiagnosticMessage> {
        Ok(())
    }
}

struct StubFulfillmentCache(Option<ObligationFulfillmentCacheDocument>);

struct FailingFulfillmentCache {
    error: fn() -> VerifyCacheError,
}

impl ObligationFulfillmentCachePort for FailingFulfillmentCache {
    fn load(
        &self,
        _track_id: &TrackId,
    ) -> Result<Option<ObligationFulfillmentCacheDocument>, VerifyCacheError> {
        Err((self.error)())
    }

    fn save(&self, _doc: &ObligationFulfillmentCacheDocument) -> Result<(), DiagnosticMessage> {
        Ok(())
    }
}

fn cache_entry(
    edge_id: TestObligationEdgeId,
    obligation_id: TestObligationId,
    key: ObligationFulfillmentCacheKey,
    verdict: ObligationFulfillmentVerdict,
    verifier_fingerprint: Option<VerifierPromptFingerprint>,
) -> ObligationFulfillmentCacheEntry {
    let location = TestLocation::new(
        LayerId::try_new("usecase".to_owned()).unwrap(),
        TestModulePath::try_new("fixture".to_owned()).unwrap(),
        TestFunctionName::try_new("entry".to_owned()).unwrap(),
    );
    let state = match verifier_fingerprint {
        Some(verifier_fingerprint) => ObligationFulfillmentCacheEntryState::Identified {
            verifier_fingerprint,
            bound_tests: Some(NonEmptyTestLocations::new(location, Vec::new())),
        },
        None => ObligationFulfillmentCacheEntryState::Legacy,
    };
    ObligationFulfillmentCacheEntry::new(edge_id, obligation_id, key, verdict, state)
}

impl ObligationFulfillmentCachePort for StubFulfillmentCache {
    fn load(
        &self,
        _track_id: &TrackId,
    ) -> Result<Option<ObligationFulfillmentCacheDocument>, VerifyCacheError> {
        Ok(self.0.clone())
    }
    fn save(&self, _doc: &ObligationFulfillmentCacheDocument) -> Result<(), DiagnosticMessage> {
        Ok(())
    }
}

struct StubWaiverCache(Option<WaiverCacheDocument>);
impl WaiverCachePort for StubWaiverCache {
    fn load(&self, _track_id: &TrackId) -> Result<Option<WaiverCacheDocument>, VerifyCacheError> {
        Ok(self.0.clone())
    }
    fn save(&self, _doc: &WaiverCacheDocument) -> Result<(), DiagnosticMessage> {
        Ok(())
    }
}

struct UnusedScanner;
impl TestSourceScannerPort for UnusedScanner {
    fn scan_test_body(
        &self,
        _location: &TestLocation,
    ) -> Result<Option<String>, TestSourceScanError> {
        Ok(None)
    }

    fn hash_test_body(
        &self,
        _source: &str,
    ) -> domain::tddd::test_obligation::hashes::TestBodySpanHash {
        domain::tddd::test_obligation::hashes::TestBodySpanHash::new(hash(0))
    }
}

struct UnusedSpecReader;
impl SpecDocumentLoaderPort for UnusedSpecReader {
    fn load(&self, path: &Path) -> Result<SpecDocument, SpecDocumentLoadError> {
        Err(SpecDocumentLoadError::NotFound { path: path.to_path_buf() })
    }
}

struct UnusedCatalogueReader;
impl AttestedCatalogueDocumentLoaderPort for UnusedCatalogueReader {
    fn load(&self, path: &Path) -> Result<AttestedCatalogueDocument, CatalogueDocumentLoaderError> {
        Err(CatalogueDocumentLoaderError::NotFound { path: path.to_path_buf() })
    }
}

struct UnusedTaskContractReader;
impl TaskContractReaderPort for UnusedTaskContractReader {
    fn read(
        &self,
        _track_id: &TrackId,
    ) -> Result<domain::task_contract::TaskContractDocument, TaskContractReadError> {
        Err(TaskContractReadError::NotFound)
    }
}

struct UnusedImplPlanReader;
impl ImplPlanReaderPort for UnusedImplPlanReader {
    fn read_task_statuses(
        &self,
        _track_id: &TrackId,
    ) -> Result<HashMap<TaskId, TaskStatusKind>, ImplPlanReadError> {
        Err(ImplPlanReadError::ReadFailed { message: domain::FreeText::new("unused") })
    }
}

struct StatusScanner;
impl TestSourceScannerPort for StatusScanner {
    fn scan_test_body(
        &self,
        _location: &TestLocation,
    ) -> Result<Option<String>, TestSourceScanError> {
        Ok(Some("assert status lane".to_owned()))
    }

    fn hash_test_body(
        &self,
        _source: &str,
    ) -> domain::tddd::test_obligation::hashes::TestBodySpanHash {
        domain::tddd::test_obligation::hashes::TestBodySpanHash::new(hash(0))
    }
}

struct StatusSpecReader(SpecDocument);
impl SpecDocumentLoaderPort for StatusSpecReader {
    fn load(&self, _path: &Path) -> Result<SpecDocument, SpecDocumentLoadError> {
        Ok(self.0.clone())
    }
}

struct StatusCatalogueReader(CatalogueDocument);
impl AttestedCatalogueDocumentLoaderPort for StatusCatalogueReader {
    fn load(
        &self,
        _path: &Path,
    ) -> Result<AttestedCatalogueDocument, CatalogueDocumentLoaderError> {
        Ok(AttestedCatalogueDocument::attest(b"T014 test catalogue", |_| {
            Ok::<_, std::convert::Infallible>(self.0.clone())
        })
        .unwrap())
    }
}

struct StatusTaskContractReader(TaskContractDocument);
impl TaskContractReaderPort for StatusTaskContractReader {
    fn read(&self, _track_id: &TrackId) -> Result<TaskContractDocument, TaskContractReadError> {
        Ok(self.0.clone())
    }
}

struct StatusImplPlanReader(HashMap<TaskId, TaskStatusKind>);
impl ImplPlanReaderPort for StatusImplPlanReader {
    fn read_task_statuses(
        &self,
        _track_id: &TrackId,
    ) -> Result<HashMap<TaskId, TaskStatusKind>, ImplPlanReadError> {
        Ok(self.0.clone())
    }
}

// ---------------------------------------------------------------------------
// Fixture builders
// ---------------------------------------------------------------------------

fn track() -> TrackId {
    TrackId::try_new("my-track").unwrap()
}

fn hash(byte: u8) -> ContentHash {
    ContentHash::from_bytes([byte; 32])
}

fn entry_key(name: &str) -> CatalogueEntryKey {
    CatalogueEntryKey::try_new(name.to_owned()).unwrap()
}

fn anchor(id: &str) -> TestObligationAnchorId {
    TestObligationAnchorId::try_new("spec.json".to_owned(), id.to_owned()).unwrap()
}

fn edge(name: &str, anchor_id: &str) -> TestObligationEdgeId {
    TestObligationEdgeId::new(entry_key(name), anchor(anchor_id))
}

fn obligation_id(name: &str, item: &str) -> TestObligationId {
    TestObligationId::new(
        entry_key(name),
        TestObligationKind::Boundary,
        TestObligationItemIdentifier::try_new(item.to_owned()).unwrap(),
    )
}

fn fulfillment_key() -> ObligationFulfillmentCacheKey {
    ObligationFulfillmentCacheKey::new(
        BoundTestsSetHash::new(hash(1)),
        DeclarationHash::new(hash(2)),
        SpecElementHash::new(hash(3)),
        ObligationResponsibilityHash::new(hash(4)),
    )
}

fn citation() -> EvidenceCitation {
    EvidenceCitation::try_new("asserts the rejection".to_owned()).unwrap()
}

fn reason(text: &str) -> DiagnosticMessage {
    DiagnosticMessage::try_new(text.to_owned()).unwrap()
}

fn artifact_io_error() -> ArtifactCodecError {
    ArtifactCodecError::Io(reason("read failed"))
}

fn artifact_malformed_error() -> ArtifactCodecError {
    ArtifactCodecError::MalformedJson(reason("bad json"))
}

fn location(layer: &str) -> TestLocation {
    TestLocation::new(
        LayerId::try_new(layer).unwrap(),
        TestModulePath::try_new(format!("{layer}::tests")).unwrap(),
        TestFunctionName::try_new("test_case".to_owned()).unwrap(),
    )
}

fn fulfillment_binding(obligation_id: TestObligationId) -> TestBindingRecord {
    TestBindingRecord::Fulfillment {
        obligation_id,
        tests: NonEmptyTestLocations::try_new(vec![location("infrastructure")]).unwrap(),
    }
}

fn obligation(name: &str, item: &str) -> TestObligation {
    obligation_with_spec_refs(name, item, vec![anchor("IN-06")])
}

fn obligation_with_spec_refs(
    name: &str,
    item: &str,
    spec_refs: Vec<TestObligationAnchorId>,
) -> TestObligation {
    TestObligation::new(
        obligation_id(name, item),
        CatalogueEntryRef::new(
            "domain-types.json".to_owned(),
            CatalogueSectionKey::Types,
            entry_key(name),
        ),
        TargetEntryRoleKind::DataRole(DataRole::value_object()),
        TestObligationBrief::try_new("cover results provenance".to_owned()).unwrap(),
        DeclarationHash::new(hash(2)),
        spec_refs,
    )
}

fn waiver_failure_cache(edge_id: TestObligationEdgeId) -> WaiverCacheDocument {
    WaiverCacheDocument::new(
        track(),
        vec![WaiverCacheEntry::new(
            edge_id,
            None,
            WaiverCacheKey::new(
                domain::tddd::test_obligation::hashes::WaivedReasonHash::new(hash(5)),
                DeclarationHash::new(hash(2)),
                SpecElementHash::new(hash(3)),
                ObligationResponsibilityHash::new(hash(4)),
            ),
            WaiverVerdict::Fail { reason: reason("does not hold") },
            None,
        )],
    )
}

fn interactor(
    bindings: Option<TestBindingsDocument>,
    fulfillment: Option<ObligationFulfillmentCacheDocument>,
    waiver: Option<WaiverCacheDocument>,
) -> TestObligationResultsInteractor {
    interactor_with_obligations(None, bindings, fulfillment, waiver)
}

fn interactor_with_obligations(
    obligations: Option<ObligationsDocument>,
    bindings: Option<TestBindingsDocument>,
    fulfillment: Option<ObligationFulfillmentCacheDocument>,
    waiver: Option<WaiverCacheDocument>,
) -> TestObligationResultsInteractor {
    TestObligationResultsInteractor::new(
        Arc::new(StubObligations(obligations)),
        Arc::new(StubBindings(bindings)),
        Arc::new(UnusedScanner),
        Arc::new(StubFulfillmentCache(fulfillment)),
        Arc::new(StubWaiverCache(waiver)),
        VerifierPromptFingerprint::new(hash(9)),
        VerifierPromptFingerprint::new(hash(10)),
        Arc::new(UnusedSpecReader),
        Arc::new(UnusedCatalogueReader),
        Arc::new(UnusedTaskContractReader),
        Arc::new(UnusedImplPlanReader),
    )
}

fn command() -> TestObligationResultsCommand {
    TestObligationResultsCommand::new(track(), Vec::new())
}

fn status_catalogue() -> CatalogueDocument {
    let mut catalogue = CatalogueDocument::new(
        5,
        CrateName::new("domain").unwrap(),
        LayerId::try_new("domain").unwrap(),
    );
    catalogue.insert_type(
        CatalogueEntryKey::try_new("Money".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::Struct(StructKind::new(StructShape::Unit, None)),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Some(ModulePath::root()),
            None,
            vec![SpecRef::new(
                PathBuf::from("spec.json"),
                SpecElementId::try_new("IN-05").unwrap(),
            )],
            Vec::new(),
        ),
    );
    catalogue
}

fn status_spec() -> SpecDocument {
    SpecDocument::new(
        "Status lane results".to_owned(),
        "1.0".to_owned(),
        Vec::new(),
        SpecScope::new(
            vec![
                SpecRequirement::new(
                    SpecElementId::try_new("IN-05").unwrap(),
                    "aggregate unresolved results by status".to_owned(),
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                )
                .unwrap(),
            ],
            Vec::new(),
        ),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        None,
    )
    .unwrap()
}

fn status_obligation() -> TestObligation {
    TestObligation::new(
        obligation_id("Money", "status-lane"),
        CatalogueEntryRef::new(
            "domain-types.json".to_owned(),
            CatalogueSectionKey::Types,
            entry_key("Money"),
        ),
        TargetEntryRoleKind::DataRole(DataRole::value_object()),
        TestObligationBrief::try_new("cover status lane aggregation".to_owned()).unwrap(),
        DeclarationHash::new(hash(2)),
        vec![anchor("IN-05")],
    )
}

fn status_interactor(
    bindings: TestBindingsDocument,
    fulfillment: Option<ObligationFulfillmentCacheDocument>,
    status: TaskStatusKind,
) -> TestObligationResultsInteractor {
    status_interactor_with_spec(bindings, fulfillment, status, status_spec())
}

fn status_interactor_with_spec(
    bindings: TestBindingsDocument,
    fulfillment: Option<ObligationFulfillmentCacheDocument>,
    status: TaskStatusKind,
    spec: SpecDocument,
) -> TestObligationResultsInteractor {
    status_interactor_with_caches(bindings, fulfillment, None, status, spec)
}

fn status_interactor_with_caches(
    bindings: TestBindingsDocument,
    fulfillment: Option<ObligationFulfillmentCacheDocument>,
    waiver: Option<WaiverCacheDocument>,
    status: TaskStatusKind,
    spec: SpecDocument,
) -> TestObligationResultsInteractor {
    let task_id = TaskId::try_new("T001".to_owned()).unwrap();
    let mut entries = BTreeMap::new();
    entries.insert(
        task_id.clone(),
        vec![ContractedEntryRef::new(LayerId::try_new("domain").unwrap(), entry_key("Money"))],
    );
    let mut statuses = HashMap::new();
    statuses.insert(task_id, status);
    TestObligationResultsInteractor::new(
        Arc::new(StubObligations(Some(ObligationsDocument::new(
            track(),
            vec![status_obligation()],
        )))),
        Arc::new(StubBindings(Some(bindings))),
        Arc::new(StatusScanner),
        Arc::new(StubFulfillmentCache(fulfillment)),
        Arc::new(StubWaiverCache(waiver)),
        VerifierPromptFingerprint::new(hash(9)),
        VerifierPromptFingerprint::new(hash(10)),
        Arc::new(StatusSpecReader(spec)),
        Arc::new(StatusCatalogueReader(status_catalogue())),
        Arc::new(StatusTaskContractReader(TaskContractDocument::new(track(), entries).unwrap())),
        Arc::new(StatusImplPlanReader(statuses)),
    )
}

fn status_command() -> TestObligationResultsCommand {
    TestObligationResultsCommand::new(track(), vec![PathBuf::from("domain-types.json")])
}

fn status_spec_moved_to_out_of_scope() -> SpecDocument {
    SpecDocument::new(
        "Status lane results".to_owned(),
        "1.0".to_owned(),
        Vec::new(),
        SpecScope::new(
            Vec::new(),
            vec![
                SpecRequirement::new(
                    SpecElementId::try_new("IN-05").unwrap(),
                    "aggregate unresolved results by status".to_owned(),
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                )
                .unwrap(),
            ],
        ),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        None,
    )
    .unwrap()
}

fn status_key_for_section(section: SpecSectionKind) -> ObligationFulfillmentCacheKey {
    let obligation = status_obligation();
    let declaration = declaration_with_obligation_context(
        &obligation_declaration_text(&[status_catalogue()], &obligation).unwrap(),
        obligation.id(),
        obligation.brief(),
    );
    let element = SpecElementRef::new(
        section,
        SpecElementId::try_new("IN-05").unwrap(),
        "aggregate unresolved results by status".to_owned(),
    );
    ObligationFulfillmentCacheKey::new(
        BoundTestsSetHash::new(sha256_content_hash(b"assert status lane\n")),
        DeclarationHash::new(sha256_content_hash(declaration.as_bytes())),
        crate::test_obligation::freshness::spec_element_hash(&[element], &edge("Money", "IN-05")),
        crate::test_obligation::freshness::responsibility_hash(obligation.id(), obligation.brief()),
    )
}

fn status_fresh_key() -> ObligationFulfillmentCacheKey {
    status_key_for_section(SpecSectionKind::InScope)
}

fn status_key_with_declaration_hash(
    declaration_hash: DeclarationHash,
) -> ObligationFulfillmentCacheKey {
    let current = status_fresh_key();
    ObligationFulfillmentCacheKey::new(
        current.bound_tests_set_hash().clone(),
        declaration_hash,
        current.spec_element_hash().clone(),
        current.responsibility_hash().clone(),
    )
}

fn status_key_with_responsibility_hash(
    responsibility_hash: ObligationResponsibilityHash,
) -> ObligationFulfillmentCacheKey {
    let current = status_fresh_key();
    ObligationFulfillmentCacheKey::new(
        current.bound_tests_set_hash().clone(),
        current.declaration_hash().clone(),
        current.spec_element_hash().clone(),
        responsibility_hash,
    )
}

fn status_moved_key() -> ObligationFulfillmentCacheKey {
    status_key_for_section(SpecSectionKind::OutOfScope)
}

fn status_waiver_reason() -> WaivedReason {
    WaivedReason::try_new("valid status-lane waiver".to_owned()).unwrap()
}

fn status_waiver_key_for_section(section: SpecSectionKind) -> WaiverCacheKey {
    let obligation = status_obligation();
    let declaration = declaration_with_obligation_item(
        &obligation_declaration_text(&[status_catalogue()], &obligation).unwrap(),
        obligation.id().item_identifier().as_str(),
    );
    let element = SpecElementRef::new(
        section,
        SpecElementId::try_new("IN-05").unwrap(),
        "aggregate unresolved results by status".to_owned(),
    );
    WaiverCacheKey::new(
        WaivedReasonHash::new(sha256_content_hash(status_waiver_reason().as_str().as_bytes())),
        DeclarationHash::new(sha256_content_hash(declaration.as_bytes())),
        crate::test_obligation::freshness::spec_element_hash(&[element], &edge("Money", "IN-05")),
        crate::test_obligation::freshness::responsibility_hash(obligation.id(), obligation.brief()),
    )
}

fn status_waiver_cache(verdict: WaiverVerdict, key: WaiverCacheKey) -> WaiverCacheDocument {
    WaiverCacheDocument::new(
        track(),
        vec![WaiverCacheEntry::new(
            edge("Money", "IN-05"),
            Some(status_obligation().id().clone()),
            key,
            verdict,
            Some(VerifierPromptFingerprint::new(hash(10))),
        )],
    )
}

fn assert_stale_and_refreshed_fulfillment_results(
    stale_key: ObligationFulfillmentCacheKey,
    stale_fingerprint: VerifierPromptFingerprint,
) {
    let obligation = status_obligation();
    let binding =
        TestBindingsDocument::new(track(), vec![fulfillment_binding(obligation.id().clone())]);
    let current_key = status_fresh_key();
    let current_fingerprint = VerifierPromptFingerprint::new(hash(9));

    for verdict in [
        ObligationFulfillmentVerdict::Fulfilled { citation: citation() },
        ObligationFulfillmentVerdict::Fail {
            category: FulfillmentFailCategory::Contradiction,
            reason: reason("cached failure"),
        },
    ] {
        let refreshed_verdict_is_absent =
            matches!(&verdict, ObligationFulfillmentVerdict::Fail { .. });
        let stale = status_interactor_with_spec(
            binding.clone(),
            Some(ObligationFulfillmentCacheDocument::new(
                track(),
                vec![cache_entry(
                    edge("Money", "IN-05"),
                    obligation.id().clone(),
                    stale_key.clone(),
                    verdict.clone(),
                    Some(stale_fingerprint.clone()),
                )],
            )),
            TaskStatusKind::Done,
            status_spec(),
        )
        .execute(&status_command())
        .unwrap();

        let stale_lane = stale
            .lane_summaries()
            .iter()
            .find(|summary| summary.chain_name() == &TestObligationChainLabel::Fulfillment)
            .unwrap();
        assert_eq!(stale_lane.pass_count(), 0);
        assert_eq!(stale_lane.fail_count(), 0);
        assert_eq!(stale_lane.pending_count(), 1);
        assert_eq!(stale.records().len(), 1);
        let stale_status = stale
            .status_lane_summaries()
            .unwrap()
            .iter()
            .find(|summary| summary.task_status() == TaskStatusKind::Done)
            .unwrap();
        assert_eq!(stale_status.stale_count() + stale_status.verdict_absent_count(), 1);

        let refreshed = status_interactor_with_spec(
            binding.clone(),
            Some(ObligationFulfillmentCacheDocument::new(
                track(),
                vec![cache_entry(
                    edge("Money", "IN-05"),
                    obligation.id().clone(),
                    current_key.clone(),
                    verdict.clone(),
                    Some(current_fingerprint.clone()),
                )],
            )),
            TaskStatusKind::Done,
            status_spec(),
        )
        .execute(&status_command())
        .unwrap();

        let refreshed_lane = refreshed
            .lane_summaries()
            .iter()
            .find(|summary| summary.chain_name() == &TestObligationChainLabel::Fulfillment)
            .unwrap();
        match verdict {
            ObligationFulfillmentVerdict::Fulfilled { .. } => {
                assert_eq!(refreshed_lane.pass_count(), 1);
                assert_eq!(refreshed_lane.fail_count(), 0);
                assert_eq!(refreshed.records().len(), 0);
            }
            ObligationFulfillmentVerdict::Fail { .. } => {
                assert_eq!(refreshed_lane.pass_count(), 0);
                assert_eq!(refreshed_lane.fail_count(), 1);
                assert_eq!(refreshed.records().len(), 1);
            }
            ObligationFulfillmentVerdict::Pending => panic!("fixture has no pending verdict"),
        }
        let refreshed_status = refreshed
            .status_lane_summaries()
            .unwrap()
            .iter()
            .find(|summary| summary.task_status() == TaskStatusKind::Done)
            .unwrap();
        assert_eq!(refreshed_status.stale_count(), 0);
        assert_eq!(
            refreshed_status.verdict_absent_count(),
            usize::from(refreshed_verdict_is_absent)
        );
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn test_fulfillment_lane_counts_and_records() {
    // IN-10 / AC-09: pass / fail / pending are counted; fail + pending yield records.
    let fulfilled_id = obligation_id("Money", "invariant:a");
    let failed_id = obligation_id("Money", "invariant:b");
    let pending_id = obligation_id("Money", "invariant:c");
    let bindings = TestBindingsDocument::new(
        track(),
        vec![
            fulfillment_binding(fulfilled_id.clone()),
            fulfillment_binding(failed_id.clone()),
            fulfillment_binding(pending_id.clone()),
        ],
    );
    let entries = vec![
        cache_entry(
            edge("Money", "IN-05"),
            fulfilled_id.clone(),
            fulfillment_key(),
            ObligationFulfillmentVerdict::Fulfilled { citation: citation() },
            None,
        ),
        cache_entry(
            edge("Money", "IN-06"),
            failed_id.clone(),
            fulfillment_key(),
            ObligationFulfillmentVerdict::Fail {
                category: FulfillmentFailCategory::Contradiction,
                reason: reason("asserts the opposite"),
            },
            None,
        ),
        cache_entry(
            edge("Money", "IN-07"),
            pending_id.clone(),
            fulfillment_key(),
            ObligationFulfillmentVerdict::Pending,
            None,
        ),
    ];
    let cache = ObligationFulfillmentCacheDocument::new(track(), entries);
    let output = interactor(Some(bindings), Some(cache), None).execute(&command()).unwrap();

    let fulfillment_lanes: Vec<_> = output
        .lane_summaries()
        .iter()
        .filter(|l| *l.chain_name() == TestObligationChainLabel::Fulfillment)
        .collect();
    assert_eq!(fulfillment_lanes.len(), 1);
    assert_eq!(fulfillment_lanes[0].pass_count(), 0);
    assert_eq!(fulfillment_lanes[0].fail_count(), 0);
    assert_eq!(fulfillment_lanes[0].pending_count(), 3);
    // Without freshness context every legacy row is fail-closed as pending.
    assert_eq!(output.records().len(), 3);
    assert!(output.records().contains(&EdgeVerdictRecord::new(
        Some(fulfilled_id),
        edge("Money", "IN-05"),
        Some(reason("fulfillment binding")),
        Some(reason("infrastructure::infrastructure::tests::test_case")),
        EdgeResolutionOutcome::Fulfillment(ObligationFulfillmentVerdict::Pending),
        None,
    )));
    assert!(output.records().contains(&EdgeVerdictRecord::new(
        Some(failed_id),
        edge("Money", "IN-06"),
        Some(reason("fulfillment binding")),
        Some(reason("infrastructure::infrastructure::tests::test_case")),
        EdgeResolutionOutcome::Fulfillment(ObligationFulfillmentVerdict::Pending),
        None,
    )));
    assert!(output.records().contains(&EdgeVerdictRecord::new(
        Some(pending_id),
        edge("Money", "IN-07"),
        Some(reason("fulfillment binding")),
        Some(reason("infrastructure::infrastructure::tests::test_case")),
        EdgeResolutionOutcome::Fulfillment(ObligationFulfillmentVerdict::Pending),
        None,
    )));
}

#[test]
fn test_layer_resolved_from_binding_test_location() {
    // The lane layer is resolved from the obligation's binding tests.
    let obligation = obligation_id("Money", "invariant:a");
    let binding = TestBindingRecord::Fulfillment {
        obligation_id: obligation.clone(),
        tests: NonEmptyTestLocations::try_new(vec![location("infrastructure")]).unwrap(),
    };
    let bindings = TestBindingsDocument::new(track(), vec![binding]);
    let cache = ObligationFulfillmentCacheDocument::new(
        track(),
        vec![cache_entry(
            edge("Money", "IN-05"),
            obligation,
            fulfillment_key(),
            ObligationFulfillmentVerdict::Fulfilled { citation: citation() },
            None,
        )],
    );
    let output = interactor(Some(bindings), Some(cache), None).execute(&command()).unwrap();
    assert_eq!(output.lane_summaries()[0].layer().as_ref(), "infrastructure");
}

#[test]
fn test_layer_resolved_from_migrated_voluntary_binding() {
    let obligation = obligation_id("Money", "invariant:a");
    let bound_edge = edge("Money", "IN-05");
    let binding = TestBindingRecord::VoluntaryBinding {
        edge_id: bound_edge.clone(),
        tests: NonEmptyTestLocations::try_new(vec![location("infrastructure")]).unwrap(),
    };
    let bindings = TestBindingsDocument::new(track(), vec![binding]);
    let cache = ObligationFulfillmentCacheDocument::new(
        track(),
        vec![cache_entry(
            bound_edge,
            obligation,
            fulfillment_key(),
            ObligationFulfillmentVerdict::Fulfilled { citation: citation() },
            None,
        )],
    );

    let output = interactor(Some(bindings), Some(cache), None).execute(&command()).unwrap();

    assert_eq!(output.lane_summaries()[0].layer().as_ref(), "infrastructure");
}

#[test]
fn test_waiver_lane_counts() {
    let waived_edge = edge("Money", "IN-06");
    let resolved_obligation = obligation("Money", "invariant:b");
    let resolved_obligation_id = resolved_obligation.id().clone();
    let waiver_reason = "the fallback cannot emit a pass verdict";
    let bindings = TestBindingsDocument::new(
        track(),
        vec![TestBindingRecord::Waiver {
            edge_id: waived_edge.clone(),
            reason: WaivedReason::try_new(waiver_reason.to_owned()).unwrap(),
        }],
    );
    let entries = vec![
        WaiverCacheEntry::new(
            edge("Money", "IN-05"),
            None,
            WaiverCacheKey::new(
                domain::tddd::test_obligation::hashes::WaivedReasonHash::new(hash(4)),
                DeclarationHash::new(hash(2)),
                SpecElementHash::new(hash(3)),
                ObligationResponsibilityHash::new(hash(4)),
            ),
            WaiverVerdict::Waived { citation: citation() },
            None,
        ),
        WaiverCacheEntry::new(
            waived_edge.clone(),
            None,
            WaiverCacheKey::new(
                domain::tddd::test_obligation::hashes::WaivedReasonHash::new(hash(5)),
                DeclarationHash::new(hash(2)),
                SpecElementHash::new(hash(3)),
                ObligationResponsibilityHash::new(hash(4)),
            ),
            WaiverVerdict::Fail { reason: reason("does not hold") },
            None,
        ),
    ];
    let cache = WaiverCacheDocument::new(track(), entries);
    let output = interactor_with_obligations(
        Some(ObligationsDocument::new(track(), vec![resolved_obligation])),
        Some(bindings),
        None,
        Some(cache),
    )
    .execute(&command())
    .unwrap();

    let waiver_lanes: Vec<_> = output
        .lane_summaries()
        .iter()
        .filter(|l| *l.chain_name() == TestObligationChainLabel::Waiver)
        .collect();
    assert_eq!(waiver_lanes.len(), 1);
    assert_eq!(waiver_lanes[0].pass_count(), 0);
    assert_eq!(waiver_lanes[0].fail_count(), 0);
    assert_eq!(waiver_lanes[0].pending_count(), 2);
    assert_eq!(output.records().len(), 2);
    assert!(output.records().contains(&EdgeVerdictRecord::new(
        Some(resolved_obligation_id),
        waived_edge,
        Some(reason("waiver")),
        Some(reason(waiver_reason)),
        EdgeResolutionOutcome::Waiver(WaiverVerdict::Pending),
        None,
    )));
}

#[test]
fn test_waiver_record_without_exact_binding_has_no_provenance() {
    let waived_edge = edge("Money", "IN-06");
    let resolved_obligation = obligation("Money", "invariant:a");
    let expected_record = EdgeVerdictRecord::new(
        Some(resolved_obligation.id().clone()),
        waived_edge.clone(),
        None,
        None,
        EdgeResolutionOutcome::Waiver(WaiverVerdict::Pending),
        None,
    );

    for bindings in [
        None,
        Some(TestBindingsDocument::new(
            track(),
            vec![TestBindingRecord::Waiver {
                edge_id: edge("Money", "IN-05"),
                reason: WaivedReason::try_new("unrelated waiver".to_owned()).unwrap(),
            }],
        )),
    ] {
        let output = interactor_with_obligations(
            Some(ObligationsDocument::new(track(), vec![resolved_obligation.clone()])),
            bindings,
            None,
            Some(waiver_failure_cache(waived_edge.clone())),
        )
        .execute(&command())
        .unwrap();

        assert_eq!(output.records(), std::slice::from_ref(&expected_record));
    }
}

#[test]
fn test_waiver_record_resolves_unique_owner_by_anchor() {
    let waived_edge = edge("Money", "IN-07");
    let unrelated_obligation = obligation("Money", "invariant:a");
    let owner = obligation_with_spec_refs("Money", "invariant:b", vec![anchor("IN-07")]);
    let owner_id = owner.id().clone();
    let bindings = TestBindingsDocument::new(
        track(),
        vec![TestBindingRecord::Waiver {
            edge_id: waived_edge.clone(),
            reason: WaivedReason::try_new("valid waiver".to_owned()).unwrap(),
        }],
    );

    let output = interactor_with_obligations(
        Some(ObligationsDocument::new(track(), vec![unrelated_obligation, owner])),
        Some(bindings),
        None,
        Some(waiver_failure_cache(waived_edge.clone())),
    )
    .execute(&command())
    .unwrap();

    assert_eq!(
        output.records(),
        &[EdgeVerdictRecord::new(
            Some(owner_id),
            waived_edge,
            Some(reason("waiver")),
            Some(reason("valid waiver")),
            EdgeResolutionOutcome::Waiver(WaiverVerdict::Pending),
            None,
        )]
    );
}

#[test]
fn test_waiver_record_with_ambiguous_anchor_owner_leaves_obligation_unresolved() {
    let waived_edge = edge("Money", "IN-07");
    let first_owner = obligation_with_spec_refs("Money", "invariant:a", vec![anchor("IN-07")]);
    let second_owner = obligation_with_spec_refs("Money", "invariant:b", vec![anchor("IN-07")]);
    let bindings = TestBindingsDocument::new(
        track(),
        vec![TestBindingRecord::Waiver {
            edge_id: waived_edge.clone(),
            reason: WaivedReason::try_new("valid waiver".to_owned()).unwrap(),
        }],
    );

    let output = interactor_with_obligations(
        Some(ObligationsDocument::new(track(), vec![first_owner, second_owner])),
        Some(bindings),
        None,
        Some(waiver_failure_cache(waived_edge.clone())),
    )
    .execute(&command())
    .unwrap();

    assert_eq!(
        output.records(),
        &[EdgeVerdictRecord::new(
            None,
            waived_edge,
            Some(reason("waiver")),
            Some(reason("valid waiver")),
            EdgeResolutionOutcome::Waiver(WaiverVerdict::Pending),
            None,
        )]
    );
}

#[test]
fn test_absent_caches_yield_empty_ok_output() {
    // CN-09: informational — never errors on absent caches, always Ok.
    let output = interactor(None, None, None).execute(&command()).unwrap();
    assert!(output.lane_summaries().is_empty());
    assert!(output.records().is_empty());
    assert!(output.uncited_findings().is_empty());
    assert!(matches!(
        output.status_lane_summaries(),
        Ok(summaries) if summaries.is_empty()
    ));
}

#[test]
fn test_status_lane_summary_keeps_all_lanes_and_unresolved_breakdowns() {
    let output = TestObligationResultsOutput::new(
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Ok(vec![
            TestObligationStatusLaneSummary::new(TaskStatusKind::Todo, 1, 2, 3),
            TestObligationStatusLaneSummary::new(TaskStatusKind::InProgress, 4, 5, 6),
            TestObligationStatusLaneSummary::new(TaskStatusKind::Done, 7, 8, 9),
            TestObligationStatusLaneSummary::new(TaskStatusKind::Skipped, 10, 11, 12),
        ]),
    );

    let summaries = output.status_lane_summaries().unwrap();
    assert_eq!(summaries.len(), 4);
    assert_eq!(summaries[0].task_status(), TaskStatusKind::Todo);
    assert_eq!(summaries[1].task_status(), TaskStatusKind::InProgress);
    assert_eq!(summaries[2].task_status(), TaskStatusKind::Done);
    assert_eq!(summaries[3].task_status(), TaskStatusKind::Skipped);
    assert_eq!(summaries[0].missing_count(), 1);
    assert_eq!(summaries[1].stale_count(), 5);
    assert_eq!(summaries[2].verdict_absent_count(), 9);
    assert_eq!(summaries[3].missing_count(), 10);
    assert_eq!(summaries[3].stale_count(), 11);
    assert_eq!(summaries[3].verdict_absent_count(), 12);
}

#[test]
fn test_results_interactor_aggregates_status_lanes_without_gate_failure() {
    for status in [TaskStatusKind::Todo, TaskStatusKind::InProgress, TaskStatusKind::Done] {
        let output =
            status_interactor(TestBindingsDocument::new(track(), Vec::new()), None, status)
                .execute(&status_command())
                .unwrap();
        let summary = output
            .status_lane_summaries()
            .unwrap()
            .iter()
            .find(|summary| summary.task_status() == status)
            .unwrap();
        assert_eq!(summary.missing_count(), 1);
        assert_eq!(summary.stale_count(), 0);
        assert_eq!(summary.verdict_absent_count(), 0);
    }

    let obligation = status_obligation();
    let bindings =
        TestBindingsDocument::new(track(), vec![fulfillment_binding(obligation.id().clone())]);
    let verdict_absent = status_interactor(bindings.clone(), None, TaskStatusKind::Skipped)
        .execute(&status_command())
        .unwrap();
    let skipped = verdict_absent
        .status_lane_summaries()
        .unwrap()
        .iter()
        .find(|summary| summary.task_status() == TaskStatusKind::Skipped)
        .unwrap();
    assert_eq!(skipped.verdict_absent_count(), 1);

    let stale_cache = ObligationFulfillmentCacheDocument::new(
        track(),
        vec![cache_entry(
            edge("Money", "IN-05"),
            obligation.id().clone(),
            fulfillment_key(),
            ObligationFulfillmentVerdict::Fulfilled { citation: citation() },
            Some(VerifierPromptFingerprint::new(hash(9))),
        )],
    );
    let stale = status_interactor(bindings, Some(stale_cache), TaskStatusKind::Done)
        .execute(&status_command())
        .unwrap();
    let done = stale
        .status_lane_summaries()
        .unwrap()
        .iter()
        .find(|summary| summary.task_status() == TaskStatusKind::Done)
        .unwrap();
    assert_eq!(done.stale_count(), 1);
}

#[test]
fn test_test_obligation_results_interactor_reports_stale_and_post_reevaluation_freshness() {
    let obligation = status_obligation();
    let binding =
        TestBindingsDocument::new(track(), vec![fulfillment_binding(obligation.id().clone())]);
    for verdict in [
        ObligationFulfillmentVerdict::Fulfilled { citation: citation() },
        ObligationFulfillmentVerdict::Fail {
            category: FulfillmentFailCategory::Contradiction,
            reason: reason("cached failure"),
        },
    ] {
        let cache = ObligationFulfillmentCacheDocument::new(
            track(),
            vec![cache_entry(
                edge("Money", "IN-05"),
                obligation.id().clone(),
                status_fresh_key(),
                verdict.clone(),
                Some(VerifierPromptFingerprint::new(hash(9))),
            )],
        );
        let fresh_interactor: TestObligationResultsInteractor = status_interactor_with_spec(
            binding.clone(),
            Some(cache.clone()),
            TaskStatusKind::Done,
            status_spec(),
        );
        let fresh = fresh_interactor.execute(&status_command()).unwrap();
        let fresh_summary = fresh
            .status_lane_summaries()
            .unwrap()
            .iter()
            .find(|summary| summary.task_status() == TaskStatusKind::Done)
            .unwrap();
        assert_eq!(fresh_summary.stale_count(), 0);

        let moved_interactor: TestObligationResultsInteractor = status_interactor_with_spec(
            binding.clone(),
            Some(cache),
            TaskStatusKind::Done,
            status_spec_moved_to_out_of_scope(),
        );
        let moved = moved_interactor.execute(&status_command()).unwrap();
        let moved_summary = moved
            .status_lane_summaries()
            .unwrap()
            .iter()
            .find(|summary| summary.task_status() == TaskStatusKind::Done)
            .unwrap();
        assert_eq!(moved_summary.stale_count(), 1);

        let post_reevaluation_cache = ObligationFulfillmentCacheDocument::new(
            track(),
            vec![cache_entry(
                edge("Money", "IN-05"),
                obligation.id().clone(),
                status_moved_key(),
                verdict.clone(),
                Some(VerifierPromptFingerprint::new(hash(9))),
            )],
        );
        let post_reevaluation_interactor: TestObligationResultsInteractor =
            status_interactor_with_spec(
                binding.clone(),
                Some(post_reevaluation_cache),
                TaskStatusKind::Done,
                status_spec_moved_to_out_of_scope(),
            );
        let post_reevaluation = post_reevaluation_interactor.execute(&status_command()).unwrap();
        let post_summary = post_reevaluation
            .status_lane_summaries()
            .unwrap()
            .iter()
            .find(|summary| summary.task_status() == TaskStatusKind::Done)
            .unwrap();
        assert_eq!(post_summary.stale_count(), 0);
        let fulfillment_lane = post_reevaluation
            .lane_summaries()
            .iter()
            .find(|summary| summary.chain_name() == &TestObligationChainLabel::Fulfillment)
            .unwrap();
        match &verdict {
            ObligationFulfillmentVerdict::Fulfilled { .. } => {
                assert_eq!(fulfillment_lane.pass_count(), 1);
            }
            ObligationFulfillmentVerdict::Fail { .. } => {
                assert_eq!(fulfillment_lane.fail_count(), 1);
            }
            ObligationFulfillmentVerdict::Pending => {
                assert_eq!(fulfillment_lane.pending_count(), 1);
            }
        }
    }
}

#[test]
fn test_results_projects_stale_pass_and_fail_to_pending_in_both_verifier_lanes() {
    let obligation = status_obligation();
    let fulfillment_binding =
        TestBindingsDocument::new(track(), vec![fulfillment_binding(obligation.id().clone())]);

    for verdict in [
        ObligationFulfillmentVerdict::Fulfilled { citation: citation() },
        ObligationFulfillmentVerdict::Fail {
            category: FulfillmentFailCategory::Contradiction,
            reason: reason("cached failure"),
        },
    ] {
        let output = status_interactor_with_spec(
            fulfillment_binding.clone(),
            Some(ObligationFulfillmentCacheDocument::new(
                track(),
                vec![cache_entry(
                    edge("Money", "IN-05"),
                    obligation.id().clone(),
                    status_fresh_key(),
                    verdict,
                    Some(VerifierPromptFingerprint::new(hash(9))),
                )],
            )),
            TaskStatusKind::Done,
            status_spec_moved_to_out_of_scope(),
        )
        .execute(&status_command())
        .unwrap();

        let fulfillment_lane = output
            .lane_summaries()
            .iter()
            .find(|summary| summary.chain_name() == &TestObligationChainLabel::Fulfillment)
            .unwrap();
        assert_eq!(fulfillment_lane.pass_count(), 0);
        assert_eq!(fulfillment_lane.fail_count(), 0);
        assert_eq!(fulfillment_lane.pending_count(), 1);
        assert_eq!(output.records().len(), 1);
        let done = output
            .status_lane_summaries()
            .unwrap()
            .iter()
            .find(|summary| summary.task_status() == TaskStatusKind::Done)
            .unwrap();
        assert_eq!(done.stale_count(), 1);
        assert_eq!(done.verdict_absent_count(), 0);
    }

    let waiver_binding = TestBindingsDocument::new(
        track(),
        vec![TestBindingRecord::Waiver {
            edge_id: edge("Money", "IN-05"),
            reason: status_waiver_reason(),
        }],
    );
    for verdict in [
        WaiverVerdict::Waived { citation: citation() },
        WaiverVerdict::Fail { reason: reason("cached waiver failure") },
    ] {
        let output = status_interactor_with_caches(
            waiver_binding.clone(),
            None,
            Some(status_waiver_cache(
                verdict,
                status_waiver_key_for_section(SpecSectionKind::InScope),
            )),
            TaskStatusKind::Done,
            status_spec_moved_to_out_of_scope(),
        )
        .execute(&status_command())
        .unwrap();

        let waiver_lane = output
            .lane_summaries()
            .iter()
            .find(|summary| summary.chain_name() == &TestObligationChainLabel::Waiver)
            .unwrap();
        assert_eq!(waiver_lane.pass_count(), 0);
        assert_eq!(waiver_lane.fail_count(), 0);
        assert_eq!(waiver_lane.pending_count(), 1);
        assert_eq!(output.records().len(), 1);
        let done = output
            .status_lane_summaries()
            .unwrap()
            .iter()
            .find(|summary| summary.task_status() == TaskStatusKind::Done)
            .unwrap();
        assert_eq!(done.stale_count(), 1);
        assert_eq!(done.verdict_absent_count(), 0);
    }
}

#[test]
fn test_results_rechecks_stale_waiver_pass_and_fail_after_refresh() {
    // AC-03 / AC-04: a stale waiver row is shown as pending, while the normal
    // replacement write restores the new pass/fail result without deleting it.
    let binding = TestBindingsDocument::new(
        track(),
        vec![TestBindingRecord::Waiver {
            edge_id: edge("Money", "IN-05"),
            reason: status_waiver_reason(),
        }],
    );

    for (stale_verdict, refreshed_verdict) in [
        (
            WaiverVerdict::Waived { citation: citation() },
            WaiverVerdict::Fail { reason: reason("refreshed waiver failure") },
        ),
        (
            WaiverVerdict::Fail { reason: reason("stale waiver failure") },
            WaiverVerdict::Waived { citation: citation() },
        ),
    ] {
        let stale = status_interactor_with_caches(
            binding.clone(),
            None,
            Some(status_waiver_cache(
                stale_verdict,
                status_waiver_key_for_section(SpecSectionKind::InScope),
            )),
            TaskStatusKind::Done,
            status_spec_moved_to_out_of_scope(),
        )
        .execute(&status_command())
        .unwrap();
        let stale_lane = stale
            .lane_summaries()
            .iter()
            .find(|summary| summary.chain_name() == &TestObligationChainLabel::Waiver)
            .unwrap();
        assert_eq!(stale_lane.pass_count(), 0);
        assert_eq!(stale_lane.fail_count(), 0);
        assert_eq!(stale_lane.pending_count(), 1);

        let refreshed = status_interactor_with_caches(
            binding.clone(),
            None,
            Some(status_waiver_cache(
                refreshed_verdict.clone(),
                status_waiver_key_for_section(SpecSectionKind::OutOfScope),
            )),
            TaskStatusKind::Done,
            status_spec_moved_to_out_of_scope(),
        )
        .execute(&status_command())
        .unwrap();
        let refreshed_lane = refreshed
            .lane_summaries()
            .iter()
            .find(|summary| summary.chain_name() == &TestObligationChainLabel::Waiver)
            .unwrap();
        match &refreshed_verdict {
            WaiverVerdict::Waived { .. } => {
                assert_eq!(refreshed_lane.pass_count(), 1);
                assert_eq!(refreshed_lane.fail_count(), 0);
                assert_eq!(refreshed.records().len(), 0);
            }
            WaiverVerdict::Fail { .. } => {
                assert_eq!(refreshed_lane.pass_count(), 0);
                assert_eq!(refreshed_lane.fail_count(), 1);
                assert_eq!(refreshed.records().len(), 1);
            }
            WaiverVerdict::Pending => panic!("matrix has no pending verdict"),
        }
        let status = refreshed
            .status_lane_summaries()
            .unwrap()
            .iter()
            .find(|summary| summary.task_status() == TaskStatusKind::Done)
            .unwrap();
        assert_eq!(status.stale_count(), 0);
        assert_eq!(
            status.verdict_absent_count(),
            usize::from(matches!(refreshed_verdict, WaiverVerdict::Fail { .. }))
        );
    }
}

#[test]
fn test_results_stales_pass_and_fail_when_target_responsibility_changes() {
    // AC-03: a changed target-responsibility brief invalidates both cached
    // outcomes while all other cache-key components remain current.
    let obligation = status_obligation();
    let previous_brief =
        TestObligationBrief::try_new("previous target responsibility".to_owned()).unwrap();
    let stale_responsibility =
        crate::test_obligation::freshness::responsibility_hash(obligation.id(), &previous_brief);
    let current = status_fresh_key();
    assert_ne!(stale_responsibility, *current.responsibility_hash());
    let stale_key = status_key_with_responsibility_hash(stale_responsibility);
    assert_eq!(stale_key.bound_tests_set_hash(), current.bound_tests_set_hash());
    assert_eq!(stale_key.declaration_hash(), current.declaration_hash());
    assert_eq!(stale_key.spec_element_hash(), current.spec_element_hash());

    assert_stale_and_refreshed_fulfillment_results(
        stale_key,
        VerifierPromptFingerprint::new(hash(9)),
    );
}

#[test]
fn test_results_stales_pass_and_fail_when_judgment_request_structure_changes() {
    // AC-03: a changed normalized request/declaration input invalidates both
    // cached outcomes while the evidence, specification, and responsibility
    // inputs remain current.
    let current = status_fresh_key();
    let stale_declaration = DeclarationHash::new(sha256_content_hash(
        b"previous normalized judgment request structure",
    ));
    assert_ne!(stale_declaration, *current.declaration_hash());
    let stale_key = status_key_with_declaration_hash(stale_declaration);
    assert_eq!(stale_key.bound_tests_set_hash(), current.bound_tests_set_hash());
    assert_eq!(stale_key.spec_element_hash(), current.spec_element_hash());
    assert_eq!(stale_key.responsibility_hash(), current.responsibility_hash());

    assert_stale_and_refreshed_fulfillment_results(
        stale_key,
        VerifierPromptFingerprint::new(hash(9)),
    );
}

#[test]
fn test_results_stales_pass_and_fail_when_verifier_input_format_changes() {
    // AC-03 / CN-01: a prior verifier input-format or semantic fingerprint
    // cannot make either cached outcome look current; reevaluation restores it.
    let current_fingerprint = VerifierPromptFingerprint::new(hash(9));
    let stale_fingerprint = VerifierPromptFingerprint::new(hash(8));
    assert_ne!(stale_fingerprint, current_fingerprint);

    assert_stale_and_refreshed_fulfillment_results(status_fresh_key(), stale_fingerprint);
}

#[test]
fn test_results_interactor_with_unresolved_or_status_read_error_returns_ok() {
    let unresolved = status_interactor(
        TestBindingsDocument::new(track(), Vec::new()),
        None,
        TaskStatusKind::Skipped,
    )
    .execute(&status_command());
    assert!(unresolved.is_ok());
    let unresolved_output = unresolved.unwrap();
    let skipped = unresolved_output
        .status_lane_summaries()
        .unwrap()
        .iter()
        .find(|summary| summary.task_status() == TaskStatusKind::Skipped)
        .unwrap();
    assert_eq!(skipped.missing_count(), 1);

    let obligation = status_obligation();
    let bindings =
        TestBindingsDocument::new(track(), vec![fulfillment_binding(obligation.id().clone())]);
    let failing_cache = ObligationFulfillmentCacheDocument::new(
        track(),
        vec![cache_entry(
            edge("Money", "IN-05"),
            obligation.id().clone(),
            fulfillment_key(),
            ObligationFulfillmentVerdict::Fail {
                category: FulfillmentFailCategory::CentralUnverified,
                reason: reason("status lane is independent"),
            },
            Some(VerifierPromptFingerprint::new(hash(9))),
        )],
    );

    let missing_task_contract = TestObligationResultsInteractor::new(
        Arc::new(StubObligations(Some(ObligationsDocument::new(
            track(),
            vec![obligation.clone()],
        )))),
        Arc::new(StubBindings(Some(bindings.clone()))),
        Arc::new(StatusScanner),
        Arc::new(StubFulfillmentCache(Some(failing_cache.clone()))),
        Arc::new(StubWaiverCache(None)),
        VerifierPromptFingerprint::new(hash(9)),
        VerifierPromptFingerprint::new(hash(10)),
        Arc::new(StatusSpecReader(status_spec())),
        Arc::new(StatusCatalogueReader(status_catalogue())),
        Arc::new(UnusedTaskContractReader),
        Arc::new(UnusedImplPlanReader),
    )
    .execute(&status_command())
    .unwrap();
    assert_eq!(missing_task_contract.lane_summaries().len(), 1);
    assert_eq!(missing_task_contract.records().len(), 1);
    assert!(matches!(
        missing_task_contract.status_lane_summaries(),
        Err(message) if message.as_str().contains("task attribution failed")
    ));

    let task_id = TaskId::try_new("T001".to_owned()).unwrap();
    let mut entries = BTreeMap::new();
    entries.insert(
        task_id,
        vec![ContractedEntryRef::new(LayerId::try_new("domain").unwrap(), entry_key("Money"))],
    );
    let missing_impl_plan = TestObligationResultsInteractor::new(
        Arc::new(StubObligations(Some(ObligationsDocument::new(
            track(),
            vec![obligation.clone()],
        )))),
        Arc::new(StubBindings(Some(bindings.clone()))),
        Arc::new(StatusScanner),
        Arc::new(StubFulfillmentCache(Some(failing_cache.clone()))),
        Arc::new(StubWaiverCache(None)),
        VerifierPromptFingerprint::new(hash(9)),
        VerifierPromptFingerprint::new(hash(10)),
        Arc::new(StatusSpecReader(status_spec())),
        Arc::new(StatusCatalogueReader(status_catalogue())),
        Arc::new(StatusTaskContractReader(TaskContractDocument::new(track(), entries).unwrap())),
        Arc::new(UnusedImplPlanReader),
    )
    .execute(&status_command())
    .unwrap();
    assert_eq!(missing_impl_plan.lane_summaries().len(), 1);
    assert_eq!(missing_impl_plan.records().len(), 1);
    assert!(matches!(
        missing_impl_plan.status_lane_summaries(),
        Err(message) if message.as_str().contains("task attribution failed")
    ));

    let missing_catalogue = TestObligationResultsInteractor::new(
        Arc::new(StubObligations(Some(ObligationsDocument::new(track(), vec![obligation])))),
        Arc::new(StubBindings(Some(bindings))),
        Arc::new(StatusScanner),
        Arc::new(StubFulfillmentCache(Some(failing_cache))),
        Arc::new(StubWaiverCache(None)),
        VerifierPromptFingerprint::new(hash(9)),
        VerifierPromptFingerprint::new(hash(10)),
        Arc::new(StatusSpecReader(status_spec())),
        Arc::new(UnusedCatalogueReader),
        Arc::new(UnusedTaskContractReader),
        Arc::new(UnusedImplPlanReader),
    )
    .execute(&status_command())
    .unwrap();
    assert_eq!(missing_catalogue.lane_summaries().len(), 1);
    assert_eq!(missing_catalogue.records().len(), 1);
    assert!(matches!(
        missing_catalogue.status_lane_summaries(),
        Err(message) if message.as_str().contains("catalogue read failed")
    ));
}

#[test]
fn test_results_projects_rows_pending_when_status_projection_fails() {
    let obligation = status_obligation();
    let binding =
        TestBindingsDocument::new(track(), vec![fulfillment_binding(obligation.id().clone())]);

    for verdict in [
        ObligationFulfillmentVerdict::Fulfilled { citation: citation() },
        ObligationFulfillmentVerdict::Fail {
            category: FulfillmentFailCategory::CentralUnverified,
            reason: reason("cached failure"),
        },
    ] {
        let interactor = TestObligationResultsInteractor::new(
            Arc::new(StubObligations(Some(ObligationsDocument::new(
                track(),
                vec![obligation.clone()],
            )))),
            Arc::new(StubBindings(Some(binding.clone()))),
            Arc::new(StatusScanner),
            Arc::new(StubFulfillmentCache(Some(ObligationFulfillmentCacheDocument::new(
                track(),
                vec![cache_entry(
                    edge("Money", "IN-05"),
                    obligation.id().clone(),
                    status_fresh_key(),
                    verdict,
                    Some(VerifierPromptFingerprint::new(hash(9))),
                )],
            )))),
            Arc::new(StubWaiverCache(None)),
            VerifierPromptFingerprint::new(hash(9)),
            VerifierPromptFingerprint::new(hash(10)),
            Arc::new(StatusSpecReader(status_spec())),
            Arc::new(UnusedCatalogueReader),
            Arc::new(UnusedTaskContractReader),
            Arc::new(UnusedImplPlanReader),
        );

        let output = interactor.execute(&status_command()).unwrap();
        let lane = output
            .lane_summaries()
            .iter()
            .find(|summary| summary.chain_name() == &TestObligationChainLabel::Fulfillment)
            .unwrap();
        assert_eq!(lane.pass_count(), 0);
        assert_eq!(lane.fail_count(), 0);
        assert_eq!(lane.pending_count(), 1);
        assert_eq!(output.records().len(), 1);
        assert!(matches!(
            output.status_lane_summaries(),
            Err(message) if message.as_str().contains("catalogue read failed")
        ));
    }

    let waiver_binding = TestBindingsDocument::new(
        track(),
        vec![TestBindingRecord::Waiver {
            edge_id: edge("Money", "IN-05"),
            reason: status_waiver_reason(),
        }],
    );
    for verdict in [
        WaiverVerdict::Waived { citation: citation() },
        WaiverVerdict::Fail { reason: reason("cached waiver failure") },
    ] {
        let interactor = TestObligationResultsInteractor::new(
            Arc::new(StubObligations(Some(ObligationsDocument::new(
                track(),
                vec![obligation.clone()],
            )))),
            Arc::new(StubBindings(Some(waiver_binding.clone()))),
            Arc::new(StatusScanner),
            Arc::new(StubFulfillmentCache(None)),
            Arc::new(StubWaiverCache(Some(status_waiver_cache(
                verdict,
                status_waiver_key_for_section(SpecSectionKind::InScope),
            )))),
            VerifierPromptFingerprint::new(hash(9)),
            VerifierPromptFingerprint::new(hash(10)),
            Arc::new(StatusSpecReader(status_spec())),
            Arc::new(UnusedCatalogueReader),
            Arc::new(UnusedTaskContractReader),
            Arc::new(UnusedImplPlanReader),
        );

        let output = interactor.execute(&status_command()).unwrap();
        let lane = output
            .lane_summaries()
            .iter()
            .find(|summary| summary.chain_name() == &TestObligationChainLabel::Waiver)
            .unwrap();
        assert_eq!(lane.pass_count(), 0);
        assert_eq!(lane.fail_count(), 0);
        assert_eq!(lane.pending_count(), 1);
        assert_eq!(output.records().len(), 1);
        assert!(matches!(
            output.status_lane_summaries(),
            Err(message) if message.as_str().contains("catalogue read failed")
        ));
    }
}

#[test]
fn test_results_interactor_with_absent_verifier_fingerprint_counts_verdict_absent() {
    let obligation = status_obligation();
    let bindings =
        TestBindingsDocument::new(track(), vec![fulfillment_binding(obligation.id().clone())]);
    let cache = ObligationFulfillmentCacheDocument::new(
        track(),
        vec![cache_entry(
            edge("Money", "IN-05"),
            obligation.id().clone(),
            fulfillment_key(),
            ObligationFulfillmentVerdict::Fulfilled { citation: citation() },
            None,
        )],
    );

    let output = status_interactor(bindings, Some(cache), TaskStatusKind::Done)
        .execute(&status_command())
        .unwrap();
    let fulfillment_lane = output
        .lane_summaries()
        .iter()
        .find(|summary| summary.chain_name() == &TestObligationChainLabel::Fulfillment)
        .unwrap();
    assert_eq!(fulfillment_lane.pass_count(), 0);
    assert_eq!(fulfillment_lane.fail_count(), 0);
    assert_eq!(fulfillment_lane.pending_count(), 1);
    assert_eq!(output.records().len(), 1);
    let done = output
        .status_lane_summaries()
        .unwrap()
        .iter()
        .find(|summary| summary.task_status() == TaskStatusKind::Done)
        .unwrap();
    assert_eq!(done.missing_count(), 0);
    assert_eq!(done.stale_count(), 0);
    assert_eq!(done.verdict_absent_count(), 1);
}

#[test]
fn test_results_projects_duplicate_current_rows_to_pending_in_both_lanes() {
    let obligation = status_obligation();
    let binding =
        TestBindingsDocument::new(track(), vec![fulfillment_binding(obligation.id().clone())]);
    let fingerprint = VerifierPromptFingerprint::new(hash(9));
    let key = status_fresh_key();
    let fulfillment_cache = ObligationFulfillmentCacheDocument::new(
        track(),
        vec![
            cache_entry(
                edge("Money", "IN-05"),
                obligation.id().clone(),
                key.clone(),
                ObligationFulfillmentVerdict::Fulfilled { citation: citation() },
                Some(fingerprint.clone()),
            ),
            cache_entry(
                edge("Money", "IN-05"),
                obligation.id().clone(),
                key,
                ObligationFulfillmentVerdict::Fail {
                    category: FulfillmentFailCategory::Contradiction,
                    reason: reason("conflicting current row"),
                },
                Some(fingerprint.clone()),
            ),
        ],
    );
    let fulfillment_output =
        status_interactor(binding, Some(fulfillment_cache), TaskStatusKind::Done)
            .execute(&status_command())
            .unwrap();
    let fulfillment_lane = fulfillment_output
        .lane_summaries()
        .iter()
        .find(|summary| summary.chain_name() == &TestObligationChainLabel::Fulfillment)
        .unwrap();
    assert_eq!(fulfillment_lane.pass_count(), 0);
    assert_eq!(fulfillment_lane.fail_count(), 0);
    assert_eq!(fulfillment_lane.pending_count(), 2);
    assert_eq!(fulfillment_output.records().len(), 2);
    let fulfillment_status = fulfillment_output
        .status_lane_summaries()
        .unwrap()
        .iter()
        .find(|summary| summary.task_status() == TaskStatusKind::Done)
        .unwrap();
    assert_eq!(fulfillment_status.verdict_absent_count(), 1);

    let waiver_binding = TestBindingsDocument::new(
        track(),
        vec![TestBindingRecord::Waiver {
            edge_id: edge("Money", "IN-05"),
            reason: status_waiver_reason(),
        }],
    );
    let waiver_key = status_waiver_key_for_section(SpecSectionKind::InScope);
    let waiver_cache = WaiverCacheDocument::new(
        track(),
        vec![
            WaiverCacheEntry::new(
                edge("Money", "IN-05"),
                Some(obligation.id().clone()),
                waiver_key.clone(),
                WaiverVerdict::Waived { citation: citation() },
                Some(VerifierPromptFingerprint::new(hash(10))),
            ),
            WaiverCacheEntry::new(
                edge("Money", "IN-05"),
                Some(obligation.id().clone()),
                waiver_key,
                WaiverVerdict::Fail { reason: reason("conflicting current row") },
                Some(VerifierPromptFingerprint::new(hash(10))),
            ),
        ],
    );
    let waiver_output = status_interactor_with_caches(
        waiver_binding,
        None,
        Some(waiver_cache),
        TaskStatusKind::Done,
        status_spec(),
    )
    .execute(&status_command())
    .unwrap();
    let waiver_lane = waiver_output
        .lane_summaries()
        .iter()
        .find(|summary| summary.chain_name() == &TestObligationChainLabel::Waiver)
        .unwrap();
    assert_eq!(waiver_lane.pass_count(), 0);
    assert_eq!(waiver_lane.fail_count(), 0);
    assert_eq!(waiver_lane.pending_count(), 2);
    assert_eq!(waiver_output.records().len(), 2);
    let waiver_status = waiver_output
        .status_lane_summaries()
        .unwrap()
        .iter()
        .find(|summary| summary.task_status() == TaskStatusKind::Done)
        .unwrap();
    assert_eq!(waiver_status.verdict_absent_count(), 1);
}

#[test]
fn test_results_new_aggregates_todo_in_progress_and_done_lanes() {
    for status in [TaskStatusKind::Todo, TaskStatusKind::InProgress, TaskStatusKind::Done] {
        let task_id = TaskId::try_new("T001".to_owned()).unwrap();
        let mut entries = BTreeMap::new();
        entries.insert(
            task_id.clone(),
            vec![ContractedEntryRef::new(LayerId::try_new("domain").unwrap(), entry_key("Money"))],
        );
        let mut statuses = HashMap::new();
        statuses.insert(task_id, status);

        let interactor = TestObligationResultsInteractor::new(
            Arc::new(StubObligations(Some(ObligationsDocument::new(
                track(),
                vec![status_obligation()],
            )))),
            Arc::new(StubBindings(Some(TestBindingsDocument::new(track(), Vec::new())))),
            Arc::new(StatusScanner),
            Arc::new(StubFulfillmentCache(None)),
            Arc::new(StubWaiverCache(None)),
            VerifierPromptFingerprint::new(hash(9)),
            VerifierPromptFingerprint::new(hash(10)),
            Arc::new(StatusSpecReader(status_spec())),
            Arc::new(StatusCatalogueReader(status_catalogue())),
            Arc::new(StatusTaskContractReader(
                TaskContractDocument::new(track(), entries).unwrap(),
            )),
            Arc::new(StatusImplPlanReader(statuses)),
        );

        let output = interactor.execute(&status_command()).unwrap();
        let summary = output
            .status_lane_summaries()
            .unwrap()
            .iter()
            .find(|summary| summary.task_status() == status)
            .unwrap();
        assert_eq!(summary.missing_count(), 1);
        assert_eq!(summary.stale_count(), 0);
        assert_eq!(summary.verdict_absent_count(), 0);
    }
}

#[test]
fn test_results_new_keeps_unresolved_skipped_lane_informational() {
    let obligation = status_obligation();
    let task_id = TaskId::try_new("T001".to_owned()).unwrap();
    let mut entries = BTreeMap::new();
    entries.insert(
        task_id.clone(),
        vec![ContractedEntryRef::new(LayerId::try_new("domain").unwrap(), entry_key("Money"))],
    );
    let mut statuses = HashMap::new();
    statuses.insert(task_id, TaskStatusKind::Skipped);

    let interactor = TestObligationResultsInteractor::new(
        Arc::new(StubObligations(Some(ObligationsDocument::new(
            track(),
            vec![obligation.clone()],
        )))),
        Arc::new(StubBindings(Some(TestBindingsDocument::new(
            track(),
            vec![fulfillment_binding(obligation.id().clone())],
        )))),
        Arc::new(StatusScanner),
        Arc::new(StubFulfillmentCache(None)),
        Arc::new(StubWaiverCache(None)),
        VerifierPromptFingerprint::new(hash(9)),
        VerifierPromptFingerprint::new(hash(10)),
        Arc::new(StatusSpecReader(status_spec())),
        Arc::new(StatusCatalogueReader(status_catalogue())),
        Arc::new(StatusTaskContractReader(TaskContractDocument::new(track(), entries).unwrap())),
        Arc::new(StatusImplPlanReader(statuses)),
    );

    let output = interactor.execute(&status_command()).unwrap();
    let skipped = output
        .status_lane_summaries()
        .unwrap()
        .iter()
        .find(|summary| summary.task_status() == TaskStatusKind::Skipped)
        .unwrap();
    assert_eq!(skipped.missing_count(), 0);
    assert_eq!(skipped.stale_count(), 0);
    assert_eq!(skipped.verdict_absent_count(), 1);
}

#[test]
fn test_results_does_not_report_unbound_anchorless_obligation_as_missing() {
    let obligation = obligation_with_spec_refs("Money", "status-lane", Vec::new());
    let mut catalogue = CatalogueDocument::new(
        5,
        CrateName::new("domain").unwrap(),
        LayerId::try_new("domain").unwrap(),
    );
    catalogue.insert_type(
        CatalogueEntryKey::try_new("Money".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::Struct(StructKind::new(StructShape::Unit, None)),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Some(ModulePath::root()),
            None,
            Vec::new(),
            Vec::new(),
        ),
    );
    let task_id = TaskId::try_new("T001".to_owned()).unwrap();
    let mut entries = BTreeMap::new();
    entries.insert(
        task_id.clone(),
        vec![ContractedEntryRef::new(LayerId::try_new("domain").unwrap(), entry_key("Money"))],
    );
    let mut statuses = HashMap::new();
    statuses.insert(task_id, TaskStatusKind::Done);

    let output = TestObligationResultsInteractor::new(
        Arc::new(StubObligations(Some(ObligationsDocument::new(track(), vec![obligation])))),
        Arc::new(StubBindings(Some(TestBindingsDocument::new(track(), Vec::new())))),
        Arc::new(StatusScanner),
        Arc::new(StubFulfillmentCache(None)),
        Arc::new(StubWaiverCache(None)),
        VerifierPromptFingerprint::new(hash(9)),
        VerifierPromptFingerprint::new(hash(10)),
        Arc::new(StatusSpecReader(status_spec())),
        Arc::new(StatusCatalogueReader(catalogue)),
        Arc::new(StatusTaskContractReader(TaskContractDocument::new(track(), entries).unwrap())),
        Arc::new(StatusImplPlanReader(statuses)),
    )
    .execute(&status_command())
    .unwrap();

    let summaries = output.status_lane_summaries().unwrap();
    assert!(summaries.iter().all(|summary| summary.missing_count() == 0));
}

#[test]
fn test_results_command_with_unreadable_catalogue_marks_status_lanes_unavailable() {
    let command =
        TestObligationResultsCommand::new(track(), vec![PathBuf::from("domain-types.json")]);
    let output = interactor_with_obligations(
        Some(ObligationsDocument::new(track(), Vec::new())),
        Some(TestBindingsDocument::new(track(), Vec::new())),
        None,
        None,
    )
    .execute(&command)
    .unwrap();

    assert!(output.lane_summaries().is_empty());
    assert!(output.records().is_empty());
    assert!(matches!(
        output.status_lane_summaries(),
        Err(message)
            if message.as_str().contains("catalogue read failed")
    ));
}

#[test]
fn test_obligations_io_error_maps_to_io_error() {
    let interactor = TestObligationResultsInteractor::new(
        Arc::new(FailingObligations { error: artifact_io_error }),
        Arc::new(StubBindings(None)),
        Arc::new(UnusedScanner),
        Arc::new(StubFulfillmentCache(None)),
        Arc::new(StubWaiverCache(None)),
        VerifierPromptFingerprint::new(hash(9)),
        VerifierPromptFingerprint::new(hash(10)),
        Arc::new(UnusedSpecReader),
        Arc::new(UnusedCatalogueReader),
        Arc::new(UnusedTaskContractReader),
        Arc::new(UnusedImplPlanReader),
    );

    let result = interactor.execute(&command());

    match result {
        Err(ObligationResultsError::IoError(message)) => {
            assert_eq!(message.as_str(), "read failed");
        }
        other => panic!("unexpected result: {other:?}"),
    }
}

#[test]
fn test_bindings_malformed_error_maps_to_malformed_artifact() {
    let interactor = TestObligationResultsInteractor::new(
        Arc::new(StubObligations(None)),
        Arc::new(FailingBindings { error: artifact_malformed_error }),
        Arc::new(UnusedScanner),
        Arc::new(StubFulfillmentCache(None)),
        Arc::new(StubWaiverCache(None)),
        VerifierPromptFingerprint::new(hash(9)),
        VerifierPromptFingerprint::new(hash(10)),
        Arc::new(UnusedSpecReader),
        Arc::new(UnusedCatalogueReader),
        Arc::new(UnusedTaskContractReader),
        Arc::new(UnusedImplPlanReader),
    );

    let result = interactor.execute(&command());

    match result {
        Err(ObligationResultsError::MalformedArtifact(message)) => {
            assert_eq!(message.as_str(), "bad json");
        }
        other => panic!("unexpected result: {other:?}"),
    }
}

#[test]
fn test_fulfillment_cache_source_scan_io_error_maps_to_io_error() {
    let interactor = TestObligationResultsInteractor::new(
        Arc::new(StubObligations(None)),
        Arc::new(StubBindings(None)),
        Arc::new(UnusedScanner),
        Arc::new(FailingFulfillmentCache { error: || VerifyCacheError::Io(reason("scan failed")) }),
        Arc::new(StubWaiverCache(None)),
        VerifierPromptFingerprint::new(hash(9)),
        VerifierPromptFingerprint::new(hash(10)),
        Arc::new(UnusedSpecReader),
        Arc::new(UnusedCatalogueReader),
        Arc::new(UnusedTaskContractReader),
        Arc::new(UnusedImplPlanReader),
    );

    match interactor.execute(&command()) {
        Err(ObligationResultsError::IoError(message)) => {
            assert_eq!(message.as_str(), "scan failed")
        }
        other => panic!("unexpected result: {other:?}"),
    }
}

#[test]
fn test_fulfillment_cache_source_scan_parse_error_maps_to_malformed_artifact() {
    let interactor = TestObligationResultsInteractor::new(
        Arc::new(StubObligations(None)),
        Arc::new(StubBindings(None)),
        Arc::new(UnusedScanner),
        Arc::new(FailingFulfillmentCache {
            error: || VerifyCacheError::MalformedJson(reason("bad syntax")),
        }),
        Arc::new(StubWaiverCache(None)),
        VerifierPromptFingerprint::new(hash(9)),
        VerifierPromptFingerprint::new(hash(10)),
        Arc::new(UnusedSpecReader),
        Arc::new(UnusedCatalogueReader),
        Arc::new(UnusedTaskContractReader),
        Arc::new(UnusedImplPlanReader),
    );

    match interactor.execute(&command()) {
        Err(ObligationResultsError::MalformedArtifact(message)) => {
            assert_eq!(message.as_str(), "bad syntax");
        }
        other => panic!("unexpected result: {other:?}"),
    }
}
