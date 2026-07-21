//! Unit tests for [`super::TestObligationResultsInteractor`] (T019).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use domain::SpecDocumentLoaderPort;
use domain::tddd::LayerId;
use domain::tddd::catalogue_v2::catalogue_impl_signals_ports::{
    CatalogueDocumentLoaderError, CatalogueDocumentLoaderPort,
};
use domain::tddd::catalogue_v2::roles::{DataRole, ItemAction};
use domain::tddd::catalogue_v2::{
    CatalogueDocument, CrateName, ModulePath, StructKind, StructShape, TypeEntry, TypeKindV2,
    TypeName,
};
use domain::tddd::semantic_verify::{CatalogueEntryKey, CatalogueEntryRef, CatalogueSectionKey};
use domain::tddd::test_obligation::binding::{
    NonEmptyTestLocations, TestBindingRecord, TestBindingsDocument, TestLocation,
};
use domain::tddd::test_obligation::drift::{EdgeResolutionOutcome, EdgeVerdictRecord};
use domain::tddd::test_obligation::errors::TestSourceScanError;
use domain::tddd::test_obligation::errors::{
    ArtifactCodecError, ObligationResultsError, VerifyCacheError,
};
use domain::tddd::test_obligation::hashes::VerifierPromptFingerprint;
use domain::tddd::test_obligation::hashes::{AnchorTextHash, BoundTestsSetHash, DeclarationHash};
use domain::tddd::test_obligation::ids::{
    DiagnosticMessage, TestFunctionName, TestModulePath, TestObligationAnchorId,
    TestObligationBrief, TestObligationEdgeId, TestObligationId, TestObligationItemIdentifier,
    WaivedReason,
};
use domain::tddd::test_obligation::obligations::{ObligationsDocument, TestObligation};
use domain::tddd::test_obligation::ports::{
    ObligationFulfillmentCachePort, ObligationsArtifactPort, TestBindingsArtifactPort,
    TestSourceScannerPort, WaiverCachePort,
};
use domain::tddd::test_obligation::verdict::{
    ObligationFulfillmentCacheDocument, ObligationFulfillmentCacheEntry,
    ObligationFulfillmentCacheKey, ObligationFulfillmentVerdict, WaiverCacheDocument,
    WaiverCacheEntry, WaiverCacheKey, WaiverVerdict,
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
use crate::pre_review_gate::{ImplPlanReaderPort, PreReviewGateError, TaskContractReaderPort};
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
impl CatalogueDocumentLoaderPort for UnusedCatalogueReader {
    fn load(
        &self,
        path: &Path,
    ) -> Result<domain::tddd::catalogue_v2::CatalogueDocument, CatalogueDocumentLoaderError> {
        Err(CatalogueDocumentLoaderError::NotFound { path: path.to_path_buf() })
    }
}

struct UnusedTaskContractReader;
impl TaskContractReaderPort for UnusedTaskContractReader {
    fn read(
        &self,
        _track_id: &TrackId,
    ) -> Result<domain::task_contract::TaskContractDocument, PreReviewGateError> {
        Err(PreReviewGateError::TaskContractNotFound)
    }
}

struct UnusedImplPlanReader;
impl ImplPlanReaderPort for UnusedImplPlanReader {
    fn read_task_statuses(
        &self,
        _track_id: &TrackId,
    ) -> Result<HashMap<TaskId, TaskStatusKind>, PreReviewGateError> {
        Err(PreReviewGateError::ImplPlanReadFailed { message: "unused".to_owned() })
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
impl CatalogueDocumentLoaderPort for StatusCatalogueReader {
    fn load(&self, _path: &Path) -> Result<CatalogueDocument, CatalogueDocumentLoaderError> {
        Ok(self.0.clone())
    }
}

struct StatusTaskContractReader(TaskContractDocument);
impl TaskContractReaderPort for StatusTaskContractReader {
    fn read(&self, _track_id: &TrackId) -> Result<TaskContractDocument, PreReviewGateError> {
        Ok(self.0.clone())
    }
}

struct StatusImplPlanReader(HashMap<TaskId, TaskStatusKind>);
impl ImplPlanReaderPort for StatusImplPlanReader {
    fn read_task_statuses(
        &self,
        _track_id: &TrackId,
    ) -> Result<HashMap<TaskId, TaskStatusKind>, PreReviewGateError> {
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
        AnchorTextHash::new(hash(3)),
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
                AnchorTextHash::new(hash(3)),
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
        TypeName::new("Money").unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::Struct(StructKind::new(StructShape::Unit, None)),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            ModulePath::root(),
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
        Arc::new(StubWaiverCache(None)),
        VerifierPromptFingerprint::new(hash(9)),
        VerifierPromptFingerprint::new(hash(10)),
        Arc::new(StatusSpecReader(status_spec())),
        Arc::new(StatusCatalogueReader(status_catalogue())),
        Arc::new(StatusTaskContractReader(TaskContractDocument::new(track(), entries).unwrap())),
        Arc::new(StatusImplPlanReader(statuses)),
    )
}

fn status_command() -> TestObligationResultsCommand {
    TestObligationResultsCommand::new(track(), vec![PathBuf::from("domain-types.json")])
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
        ObligationFulfillmentCacheEntry::new(
            edge("Money", "IN-05"),
            fulfilled_id,
            fulfillment_key(),
            ObligationFulfillmentVerdict::Fulfilled { citation: citation() },
            None,
        ),
        ObligationFulfillmentCacheEntry::new(
            edge("Money", "IN-06"),
            failed_id.clone(),
            fulfillment_key(),
            ObligationFulfillmentVerdict::Fail {
                category: FulfillmentFailCategory::Contradiction,
                reason: reason("asserts the opposite"),
            },
            None,
        ),
        ObligationFulfillmentCacheEntry::new(
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
    assert_eq!(fulfillment_lanes[0].pass_count(), 1);
    assert_eq!(fulfillment_lanes[0].fail_count(), 1);
    assert_eq!(fulfillment_lanes[0].pending_count(), 1);
    // Records are emitted for the fail + pending edges only.
    assert_eq!(output.records().len(), 2);
    assert!(output.records().contains(&EdgeVerdictRecord::new(
        Some(failed_id),
        edge("Money", "IN-06"),
        Some(reason("fulfillment binding")),
        Some(reason("infrastructure::infrastructure::tests::test_case")),
        EdgeResolutionOutcome::Fail(FulfillmentFailCategory::Contradiction),
        Some(reason("asserts the opposite")),
        None,
    )));
    assert!(output.records().contains(&EdgeVerdictRecord::new(
        Some(pending_id),
        edge("Money", "IN-07"),
        Some(reason("fulfillment binding")),
        Some(reason("infrastructure::infrastructure::tests::test_case")),
        EdgeResolutionOutcome::Pending,
        None,
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
        vec![ObligationFulfillmentCacheEntry::new(
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
        vec![ObligationFulfillmentCacheEntry::new(
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
                AnchorTextHash::new(hash(3)),
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
                AnchorTextHash::new(hash(3)),
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
    assert_eq!(waiver_lanes[0].pass_count(), 1);
    assert_eq!(waiver_lanes[0].fail_count(), 1);
    assert!(output.records().contains(&EdgeVerdictRecord::new(
        Some(resolved_obligation_id),
        waived_edge,
        Some(reason("waiver")),
        Some(reason(waiver_reason)),
        EdgeResolutionOutcome::Fail(FulfillmentFailCategory::CentralUnverified),
        Some(reason("does not hold")),
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
        EdgeResolutionOutcome::Fail(FulfillmentFailCategory::CentralUnverified),
        Some(reason("does not hold")),
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
            EdgeResolutionOutcome::Fail(FulfillmentFailCategory::CentralUnverified),
            Some(reason("does not hold")),
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
            EdgeResolutionOutcome::Fail(FulfillmentFailCategory::CentralUnverified),
            Some(reason("does not hold")),
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
        vec![ObligationFulfillmentCacheEntry::new(
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
        vec![ObligationFulfillmentCacheEntry::new(
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
fn test_results_interactor_with_absent_verifier_fingerprint_counts_verdict_absent() {
    let obligation = status_obligation();
    let bindings =
        TestBindingsDocument::new(track(), vec![fulfillment_binding(obligation.id().clone())]);
    let cache = ObligationFulfillmentCacheDocument::new(
        track(),
        vec![ObligationFulfillmentCacheEntry::new(
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
