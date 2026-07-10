//! Unit tests for [`super::TestObligationResultsInteractor`] (T019).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use std::sync::Arc;

use domain::tddd::LayerId;
use domain::tddd::catalogue_v2::roles::DataRole;
use domain::tddd::semantic_verify::{CatalogueEntryKey, CatalogueEntryRef, CatalogueSectionKey};
use domain::tddd::test_obligation::binding::{
    NonEmptyTestLocations, TestBindingRecord, TestBindingsDocument, TestLocation,
};
use domain::tddd::test_obligation::drift::{EdgeResolutionOutcome, EdgeVerdictRecord};
use domain::tddd::test_obligation::errors::{
    ArtifactCodecError, ObligationResultsError, VerifyCacheError,
};
use domain::tddd::test_obligation::hashes::{AnchorTextHash, BoundTestsSetHash, DeclarationHash};
use domain::tddd::test_obligation::ids::{
    DiagnosticMessage, TestFunctionName, TestModulePath, TestObligationAnchorId,
    TestObligationBrief, TestObligationEdgeId, TestObligationId, TestObligationItemIdentifier,
    WaivedReason,
};
use domain::tddd::test_obligation::obligations::{ObligationsDocument, TestObligation};
use domain::tddd::test_obligation::ports::{
    ObligationFulfillmentCachePort, ObligationsArtifactPort, TestBindingsArtifactPort,
    WaiverCachePort,
};
use domain::tddd::test_obligation::verdict::{
    ObligationFulfillmentCacheDocument, ObligationFulfillmentCacheEntry,
    ObligationFulfillmentCacheKey, ObligationFulfillmentVerdict, WaiverCacheDocument,
    WaiverCacheEntry, WaiverCacheKey, WaiverVerdict,
};
use domain::tddd::test_obligation::vocab::{
    FulfillmentFailCategory, TargetEntryRoleKind, TestObligationKind,
};
use domain::{ContentHash, EvidenceCitation, TrackId};

use super::{
    TestObligationChainLabel, TestObligationResultsApplicationService,
    TestObligationResultsCommand, TestObligationResultsInteractor,
};

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
        Arc::new(StubFulfillmentCache(fulfillment)),
        Arc::new(StubWaiverCache(waiver)),
    )
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
    let output = interactor(Some(bindings), Some(cache), None)
        .execute(&TestObligationResultsCommand::new(track()))
        .unwrap();

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
    let output = interactor(Some(bindings), Some(cache), None)
        .execute(&TestObligationResultsCommand::new(track()))
        .unwrap();
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

    let output = interactor(Some(bindings), Some(cache), None)
        .execute(&TestObligationResultsCommand::new(track()))
        .unwrap();

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
    .execute(&TestObligationResultsCommand::new(track()))
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
        .execute(&TestObligationResultsCommand::new(track()))
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
    .execute(&TestObligationResultsCommand::new(track()))
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
    .execute(&TestObligationResultsCommand::new(track()))
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
    let output =
        interactor(None, None, None).execute(&TestObligationResultsCommand::new(track())).unwrap();
    assert!(output.lane_summaries().is_empty());
    assert!(output.records().is_empty());
    assert!(output.uncited_findings().is_empty());
}

#[test]
fn test_obligations_io_error_maps_to_io_error() {
    let interactor = TestObligationResultsInteractor::new(
        Arc::new(FailingObligations { error: artifact_io_error }),
        Arc::new(StubBindings(None)),
        Arc::new(StubFulfillmentCache(None)),
        Arc::new(StubWaiverCache(None)),
    );

    let result = interactor.execute(&TestObligationResultsCommand::new(track()));

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
        Arc::new(StubFulfillmentCache(None)),
        Arc::new(StubWaiverCache(None)),
    );

    let result = interactor.execute(&TestObligationResultsCommand::new(track()));

    match result {
        Err(ObligationResultsError::MalformedArtifact(message)) => {
            assert_eq!(message.as_str(), "bad json");
        }
        other => panic!("unexpected result: {other:?}"),
    }
}
