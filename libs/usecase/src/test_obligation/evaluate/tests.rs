//! Unit tests for [`super::EvaluateTestObligationsInteractor`] (T018).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::{Pin, pin};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

use domain::tddd::LayerId;
use domain::tddd::catalogue_v2::catalogue_impl_signals_ports::{
    CatalogueDocumentLoaderError, CatalogueDocumentLoaderPort,
};
use domain::tddd::catalogue_v2::roles::{ContractRole, DataRole, ItemAction};
use domain::tddd::catalogue_v2::{
    CatalogueDocument, CrateName, MethodDeclaration, MethodName, ModulePath, SelfReceiver,
    StructKind, StructShape, TraitEntry, TraitImplDeclV2, TraitName, TypeEntry, TypeKindV2,
    TypeName, TypeRef,
};
use domain::tddd::semantic_verify::{
    CatalogueEntryKey, CatalogueEntryRef, CatalogueSectionKey, ModelTier,
};
use domain::tddd::test_obligation::binding::{
    NonEmptyTestLocations, TestBindingRecord, TestBindingsDocument, TestLocation,
};
use domain::tddd::test_obligation::drift::{EdgeResolutionOutcome, EdgeVerdictRecord};
use domain::tddd::test_obligation::errors::{
    ArtifactCodecError, ObligationEvaluateError, SemanticVerifierError, TestSourceScanError,
    VerifyCacheError,
};
use domain::tddd::test_obligation::hashes::{
    AnchorTextHash, BoundTestsSetHash, DeclarationHash, TestBodySpanHash,
    VerifierPromptFingerprint, WaivedReasonHash,
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

use crate::semantic_verdict_core::driver::{
    SemanticEscalationDriverPort, SemanticEscalationFuture,
};
use crate::test_obligation::hasher::ContentHasherPort;
use crate::test_obligation::ports::ObligationFulfillmentCachePort;
use domain::tddd::test_obligation::pair::{ObligationFulfillmentPair, WaiverPair};
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
    ContentHash, EvidenceCitation, SpecDocument, SpecDocumentLoadError, SpecElementId,
    SpecRequirement, SpecScope, TrackId,
};

use domain::SpecDocumentLoaderPort;

use super::plan::PlannedAction;
use super::{
    EvaluateTestObligationsApplicationService, EvaluateTestObligationsCommand,
    EvaluateTestObligationsInteractor, TestObligationEvaluateConfig,
};
use crate::test_obligation::LoadedCatalogueDocument;

fn run<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    let mut cx = Context::from_waker(Waker::noop());
    loop {
        match future.as_mut().poll(&mut cx) {
            Poll::Ready(output) => return output,
            Poll::Pending => continue,
        }
    }
}

// ---------------------------------------------------------------------------
// Test doubles
// ---------------------------------------------------------------------------

struct StubObligations(Option<ObligationsDocument>);
impl ObligationsArtifactPort for StubObligations {
    fn load(&self, _t: &TrackId) -> Result<Option<ObligationsDocument>, ArtifactCodecError> {
        Ok(self.0.clone())
    }
    fn save(&self, _d: &ObligationsDocument) -> Result<(), DiagnosticMessage> {
        Ok(())
    }
}

struct StubBindings(Option<TestBindingsDocument>);
impl TestBindingsArtifactPort for StubBindings {
    fn load(&self, _t: &TrackId) -> Result<Option<TestBindingsDocument>, ArtifactCodecError> {
        Ok(self.0.clone())
    }
    fn save(&self, _d: &TestBindingsDocument) -> Result<(), DiagnosticMessage> {
        Ok(())
    }
}

struct StubScanner;
impl TestSourceScannerPort for StubScanner {
    fn scan_test_body(&self, _l: &TestLocation) -> Result<Option<String>, TestSourceScanError> {
        Ok(Some("assert!(money.is_positive());".to_owned()))
    }
    fn hash_test_body(&self, _s: &str) -> TestBodySpanHash {
        TestBodySpanHash::new(ContentHash::from_bytes([0u8; 32]))
    }
}

struct ScriptedFulfillment {
    fast: ObligationFulfillmentVerdict,
    last: ObligationFulfillmentVerdict,
    verifier_error: Mutex<Option<DiagnosticMessage>>,
    // AC-08: optional calibration overrides. Per-category slots take
    // precedence; the global slot forces one verdict for every shape. With no
    // override, the stub returns a `Fail` whose category matches the probe
    // marker embedded in `known_bad_calibration_probe_<category>_<index>`.
    calibration: Mutex<Option<ObligationFulfillmentVerdict>>,
    calibration_contradiction: Mutex<Option<ObligationFulfillmentVerdict>>,
    calibration_substitution: Mutex<Option<ObligationFulfillmentVerdict>>,
    calibration_central_unverified: Mutex<Option<ObligationFulfillmentVerdict>>,
    calibration_calls: Mutex<usize>,
    /// Records the `tests_source` seen for every calibration probe so
    /// tests can assert per-category shape distribution (AC-08).
    calibration_probe_sources: Mutex<Vec<String>>,
    tiers: Mutex<Vec<ModelTier>>,
    declarations: Mutex<Vec<String>>,
}
impl
    SemanticEscalationDriverPort<
        ObligationFulfillmentPair,
        ObligationFulfillmentCacheKey,
        ObligationFulfillmentVerdict,
        SemanticVerifierError,
    > for ScriptedFulfillment
{
    fn evaluate_with_escalation<'a>(
        &'a self,
        pair: &'a ObligationFulfillmentPair,
        _key: &'a ObligationFulfillmentCacheKey,
        initial_tier: ModelTier,
    ) -> SemanticEscalationFuture<'a, ObligationFulfillmentVerdict, SemanticVerifierError> {
        Box::pin(async move {
            let source = pair.tests_source().as_str();
            if source.contains("known_bad_calibration_probe") {
                *self.calibration_calls.lock().unwrap() += 1;
                self.calibration_probe_sources.lock().unwrap().push(source.to_owned());
                if source.contains("_contradiction_")
                    && let Some(v) = self.calibration_contradiction.lock().unwrap().clone()
                {
                    return Ok(v);
                }
                if source.contains("_substitution_")
                    && let Some(v) = self.calibration_substitution.lock().unwrap().clone()
                {
                    return Ok(v);
                }
                if source.contains("_central_unverified_")
                    && let Some(v) = self.calibration_central_unverified.lock().unwrap().clone()
                {
                    return Ok(v);
                }
                if let Some(v) = self.calibration.lock().unwrap().clone() {
                    return Ok(v);
                }
                if source.contains("_contradiction_") {
                    return Ok(fulfillment_fail_for_category(
                        FulfillmentFailCategory::Contradiction,
                    ));
                }
                if source.contains("_substitution_") {
                    return Ok(fulfillment_fail_for_category(
                        FulfillmentFailCategory::Substitution,
                    ));
                }
                return Ok(fulfillment_fail_for_category(
                    FulfillmentFailCategory::CentralUnverified,
                ));
            }
            self.tiers.lock().unwrap().push(initial_tier);
            self.declarations.lock().unwrap().push(pair.entry_declaration().as_str().to_owned());
            if let Some(message) = self.verifier_error.lock().unwrap().clone() {
                return Err(SemanticVerifierError::VerifierPort(message));
            }
            if matches!(self.fast, ObligationFulfillmentVerdict::Fulfilled { .. }) {
                return Ok(self.fast.clone());
            }
            self.tiers.lock().unwrap().push(ModelTier::Final);
            Ok(self.last.clone())
        })
    }
}

struct ScriptedWaiver {
    verdict: WaiverVerdict,
    calls: Mutex<usize>,
    tiers: Mutex<Vec<ModelTier>>,
    declarations: Mutex<Vec<String>>,
}
impl SemanticEscalationDriverPort<WaiverPair, WaiverCacheKey, WaiverVerdict, SemanticVerifierError>
    for ScriptedWaiver
{
    fn evaluate_with_escalation<'a>(
        &'a self,
        pair: &'a WaiverPair,
        _key: &'a WaiverCacheKey,
        initial_tier: ModelTier,
    ) -> SemanticEscalationFuture<'a, WaiverVerdict, SemanticVerifierError> {
        Box::pin(async move {
            *self.calls.lock().unwrap() += 1;
            self.tiers.lock().unwrap().push(initial_tier);
            self.declarations.lock().unwrap().push(pair.entry_declaration().as_str().to_owned());
            if matches!(self.verdict, WaiverVerdict::Waived { .. }) {
                return Ok(self.verdict.clone());
            }
            *self.calls.lock().unwrap() += 1;
            self.tiers.lock().unwrap().push(ModelTier::Final);
            Ok(self.verdict.clone())
        })
    }
}

/// Shared peak-concurrency probe for the evaluator fan-out tests.
#[derive(Clone)]
struct DispatchTracker {
    in_flight: Arc<AtomicUsize>,
    peak: Arc<AtomicUsize>,
}

impl DispatchTracker {
    fn new() -> Self {
        Self { in_flight: Arc::new(AtomicUsize::new(0)), peak: Arc::new(AtomicUsize::new(0)) }
    }

    fn record_start(&self) {
        let current = self.in_flight.fetch_add(1, Ordering::SeqCst) + 1;
        let mut peak = self.peak.load(Ordering::SeqCst);
        while current > peak {
            match self.peak.compare_exchange(peak, current, Ordering::SeqCst, Ordering::SeqCst) {
                Ok(_) => break,
                Err(actual) => peak = actual,
            }
        }
    }

    fn record_finish(&self) {
        self.in_flight.fetch_sub(1, Ordering::SeqCst);
    }

    fn peak(&self) -> usize {
        self.peak.load(Ordering::SeqCst)
    }
}

/// A verifier result that first pends so the multiplexer has observable
/// in-flight work to bound.
struct TrackedVerdictFuture<T> {
    verdict: Option<T>,
    tracker: DispatchTracker,
    registered: bool,
    pended: bool,
    completed: bool,
}

impl<T> TrackedVerdictFuture<T> {
    fn new(verdict: T, tracker: DispatchTracker) -> Self {
        Self { verdict: Some(verdict), tracker, registered: false, pended: false, completed: false }
    }
}

impl<T: Unpin> Future for TrackedVerdictFuture<T> {
    type Output = Result<T, SemanticVerifierError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        if !this.registered {
            this.tracker.record_start();
            this.registered = true;
        }
        if !this.pended {
            this.pended = true;
            cx.waker().wake_by_ref();
            return Poll::Pending;
        }
        this.completed = true;
        this.tracker.record_finish();
        Poll::Ready(Ok(this.verdict.take().expect("tracked verdict is returned once")))
    }
}

impl<T> Drop for TrackedVerdictFuture<T> {
    fn drop(&mut self) {
        if self.registered && !self.completed {
            self.tracker.record_finish();
        }
    }
}

struct BoundedFulfillmentDriver {
    tracker: DispatchTracker,
    /// When set, non-calibration (real) pairs are tracked here instead of
    /// `tracker`, so a test can assert that no real adjudication was
    /// dispatched while the AC-08 calibration probes still flow.
    real_tracker: Option<DispatchTracker>,
}

impl
    SemanticEscalationDriverPort<
        ObligationFulfillmentPair,
        ObligationFulfillmentCacheKey,
        ObligationFulfillmentVerdict,
        SemanticVerifierError,
    > for BoundedFulfillmentDriver
{
    fn evaluate_with_escalation<'a>(
        &'a self,
        pair: &'a ObligationFulfillmentPair,
        _key: &'a ObligationFulfillmentCacheKey,
        _initial_tier: ModelTier,
    ) -> SemanticEscalationFuture<'a, ObligationFulfillmentVerdict, SemanticVerifierError> {
        let source = pair.tests_source().as_str();
        let verdict = if source.contains("_contradiction_") {
            fulfillment_fail_for_category(FulfillmentFailCategory::Contradiction)
        } else if source.contains("_substitution_") {
            fulfillment_fail_for_category(FulfillmentFailCategory::Substitution)
        } else if source.contains("_central_unverified_") {
            fulfillment_fail_for_category(FulfillmentFailCategory::CentralUnverified)
        } else {
            fulfilled()
        };
        let tracker = if source.contains("calibration_probe") {
            self.tracker.clone()
        } else {
            self.real_tracker.clone().unwrap_or_else(|| self.tracker.clone())
        };
        Box::pin(TrackedVerdictFuture::new(verdict, tracker))
    }
}

struct BoundedWaiverDriver {
    tracker: DispatchTracker,
}

impl SemanticEscalationDriverPort<WaiverPair, WaiverCacheKey, WaiverVerdict, SemanticVerifierError>
    for BoundedWaiverDriver
{
    fn evaluate_with_escalation<'a>(
        &'a self,
        _pair: &'a WaiverPair,
        _key: &'a WaiverCacheKey,
        _initial_tier: ModelTier,
    ) -> SemanticEscalationFuture<'a, WaiverVerdict, SemanticVerifierError> {
        let verdict = WaiverVerdict::Waived {
            citation: EvidenceCitation::try_new("bounded waiver coverage".to_owned()).unwrap(),
        };
        Box::pin(TrackedVerdictFuture::new(verdict, self.tracker.clone()))
    }
}

#[derive(Default)]
struct CapFulfillmentCache {
    loaded: Mutex<Option<ObligationFulfillmentCacheDocument>>,
    saved: Mutex<Option<ObligationFulfillmentCacheDocument>>,
    save_error: Mutex<Option<DiagnosticMessage>>,
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

impl ObligationFulfillmentCachePort for CapFulfillmentCache {
    fn load(
        &self,
        _t: &TrackId,
    ) -> Result<Option<ObligationFulfillmentCacheDocument>, VerifyCacheError> {
        Ok(self.loaded.lock().unwrap().clone())
    }
    fn save(&self, d: &ObligationFulfillmentCacheDocument) -> Result<(), DiagnosticMessage> {
        if let Some(error) = self.save_error.lock().unwrap().clone() {
            return Err(error);
        }
        *self.saved.lock().unwrap() = Some(d.clone());
        Ok(())
    }
}

#[derive(Default)]
struct CapWaiverCache {
    loaded: Mutex<Option<WaiverCacheDocument>>,
    saved: Mutex<Option<WaiverCacheDocument>>,
    save_error: Mutex<Option<DiagnosticMessage>>,
}
impl WaiverCachePort for CapWaiverCache {
    fn load(&self, _t: &TrackId) -> Result<Option<WaiverCacheDocument>, VerifyCacheError> {
        Ok(self.loaded.lock().unwrap().clone())
    }
    fn save(&self, d: &WaiverCacheDocument) -> Result<(), DiagnosticMessage> {
        if let Some(error) = self.save_error.lock().unwrap().clone() {
            return Err(error);
        }
        *self.saved.lock().unwrap() = Some(d.clone());
        Ok(())
    }
}

struct StubSpec(SpecDocument);
impl SpecDocumentLoaderPort for StubSpec {
    fn load(&self, _p: &Path) -> Result<SpecDocument, SpecDocumentLoadError> {
        Ok(self.0.clone())
    }
}

struct StubCatalogue(CatalogueDocument);
impl CatalogueDocumentLoaderPort for StubCatalogue {
    fn load(&self, _p: &Path) -> Result<CatalogueDocument, CatalogueDocumentLoaderError> {
        Ok(self.0.clone())
    }
}

struct SumHasher;
impl ContentHasherPort for SumHasher {
    fn sha256(&self, bytes: &[u8]) -> ContentHash {
        let sum = bytes.iter().fold(0u8, |a, b| a.wrapping_add(*b));
        ContentHash::from_bytes([sum; 32])
    }
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

fn track() -> TrackId {
    TrackId::try_new("my-track").unwrap()
}

fn fulfillment_verifier_fingerprint() -> VerifierPromptFingerprint {
    VerifierPromptFingerprint::new(ContentHash::from_bytes([8u8; 32]))
}

fn waiver_verifier_fingerprint() -> VerifierPromptFingerprint {
    VerifierPromptFingerprint::new(ContentHash::from_bytes([9u8; 32]))
}

fn config() -> TestObligationEvaluateConfig {
    TestObligationEvaluateConfig::try_new(10, 90, 4).unwrap()
}

/// Config variant with a caller-supplied injection rate; used by AC-08
/// tests that need `probe_count >= 3`.
fn config_with_rate(rate: u8) -> TestObligationEvaluateConfig {
    TestObligationEvaluateConfig::try_new(rate, 90, 4).unwrap()
}

fn anchor() -> TestObligationAnchorId {
    TestObligationAnchorId::try_new("spec.json".to_owned(), "IN-05".to_owned()).unwrap()
}

fn anchor_text() -> &'static str {
    "Money must always be positive."
}

fn edge() -> TestObligationEdgeId {
    TestObligationEdgeId::new(CatalogueEntryKey::try_new("Money".to_owned()).unwrap(), anchor())
}

fn obligation() -> TestObligation {
    let entry_key = CatalogueEntryKey::try_new("Money".to_owned()).unwrap();
    TestObligation::new(
        TestObligationId::new(
            entry_key.clone(),
            TestObligationKind::Boundary,
            TestObligationItemIdentifier::try_new("invariant:positive".to_owned()).unwrap(),
        ),
        CatalogueEntryRef::new(
            "domain-types.json".to_owned(),
            CatalogueSectionKey::Types,
            entry_key,
        ),
        TargetEntryRoleKind::DataRole(DataRole::value_object()),
        TestObligationBrief::try_new("cover positivity".to_owned()).unwrap(),
        DeclarationHash::new(ContentHash::from_bytes([2u8; 32])),
        vec![anchor()],
    )
}

fn trait_impl_obligation() -> TestObligation {
    let entry_key = CatalogueEntryKey::try_new("Money".to_owned()).unwrap();
    TestObligation::new(
        TestObligationId::new(
            entry_key.clone(),
            TestObligationKind::ContractConformance,
            TestObligationItemIdentifier::try_new("trait_impl:MyPort".to_owned()).unwrap(),
        ),
        CatalogueEntryRef::new(
            "domain-types.json".to_owned(),
            CatalogueSectionKey::Traits,
            entry_key,
        ),
        TargetEntryRoleKind::TraitImpl(ContractRole::SecondaryPort),
        TestObligationBrief::try_new("cover impl".to_owned()).unwrap(),
        DeclarationHash::new(ContentHash::from_bytes([4u8; 32])),
        vec![anchor()],
    )
}

fn obligations_doc() -> ObligationsDocument {
    ObligationsDocument::new(track(), vec![obligation()])
}

fn location() -> TestLocation {
    TestLocation::new(
        LayerId::try_new("domain").unwrap(),
        TestModulePath::try_new("domain::money::tests".to_owned()).unwrap(),
        TestFunctionName::try_new("test_positive".to_owned()).unwrap(),
    )
}

fn fulfillment_bindings() -> TestBindingsDocument {
    TestBindingsDocument::new(
        track(),
        vec![TestBindingRecord::Fulfillment {
            obligation_id: obligation().id().clone(),
            tests: NonEmptyTestLocations::try_new(vec![location()]).unwrap(),
        }],
    )
}

fn voluntary_bindings() -> TestBindingsDocument {
    TestBindingsDocument::new(
        track(),
        vec![TestBindingRecord::VoluntaryBinding {
            edge_id: edge(),
            tests: NonEmptyTestLocations::try_new(vec![location()]).unwrap(),
        }],
    )
}

fn trait_impl_bindings() -> TestBindingsDocument {
    TestBindingsDocument::new(
        track(),
        vec![TestBindingRecord::Fulfillment {
            obligation_id: trait_impl_obligation().id().clone(),
            tests: NonEmptyTestLocations::try_new(vec![location()]).unwrap(),
        }],
    )
}

fn waiver_reason() -> WaivedReason {
    WaivedReason::try_new("covered by the integration suite".to_owned()).unwrap()
}

fn waiver_bindings() -> TestBindingsDocument {
    TestBindingsDocument::new(
        track(),
        vec![TestBindingRecord::Waiver { edge_id: edge(), reason: waiver_reason() }],
    )
}

/// Three waiver records sharing the same edge — the calibration probe
/// count only depends on the record count, so this yields
/// `production_pair_count = 3` (one AC-08 category per probe when paired
/// with `config_with_rate(100)`).
fn triple_waiver_bindings() -> TestBindingsDocument {
    TestBindingsDocument::new(
        track(),
        vec![
            TestBindingRecord::Waiver { edge_id: edge(), reason: waiver_reason() },
            TestBindingRecord::Waiver { edge_id: edge(), reason: waiver_reason() },
            TestBindingRecord::Waiver { edge_id: edge(), reason: waiver_reason() },
        ],
    )
}

/// Repeated fulfillment and waiver records provide enough pending verifier
/// futures to expose the evaluator's configured fan-out ceiling.
fn repeated_lane_bindings(repetitions: usize) -> TestBindingsDocument {
    // The waiver lane uses a DISTINCT anchor (IN-06): a waiver on the same
    // edge as the fulfillment obligation would suppress the fulfillment
    // adjudication (waiver-precedence rule), collapsing the fulfillment
    // fan-out this fixture exists to exercise.
    let waiver_edge = TestObligationEdgeId::new(
        CatalogueEntryKey::try_new("Money".to_owned()).unwrap(),
        TestObligationAnchorId::try_new("spec.json".to_owned(), "IN-06".to_owned()).unwrap(),
    );
    let mut records = Vec::with_capacity(repetitions.saturating_mul(2));
    for _ in 0..repetitions {
        records.push(TestBindingRecord::Fulfillment {
            obligation_id: obligation().id().clone(),
            tests: NonEmptyTestLocations::try_new(vec![location()]).unwrap(),
        });
        records.push(TestBindingRecord::Waiver {
            edge_id: waiver_edge.clone(),
            reason: waiver_reason(),
        });
    }
    TestBindingsDocument::new(track(), records)
}

fn spec_doc() -> SpecDocument {
    let in_scope = vec![
        SpecRequirement::new(
            SpecElementId::try_new("IN-05").unwrap(),
            anchor_text(),
            vec![],
            vec![],
            vec![],
        )
        .unwrap(),
        SpecRequirement::new(
            SpecElementId::try_new("IN-06").unwrap(),
            "Money conversions must be lossless.",
            vec![],
            vec![],
            vec![],
        )
        .unwrap(),
    ];
    SpecDocument::new(
        "Test spec",
        "1.0",
        vec![],
        SpecScope::new(in_scope, vec![]),
        vec![],
        vec![],
        vec![],
        vec![],
        None,
    )
    .unwrap()
}

fn spec_doc_with_in_scope_and_acceptance_criteria() -> SpecDocument {
    let requirement = |id: &str| {
        SpecRequirement::new(
            SpecElementId::try_new(id).unwrap(),
            format!("{id} requirement text."),
            vec![],
            vec![],
            vec![],
        )
        .unwrap()
    };
    SpecDocument::new(
        "Test spec",
        "1.0",
        vec![],
        SpecScope::new(vec![requirement("IN-05")], vec![]),
        vec![],
        vec![requirement("AC-05")],
        vec![],
        vec![],
        None,
    )
    .unwrap()
}

fn empty_spec_doc() -> SpecDocument {
    SpecDocument::new(
        "Test spec",
        "1.0",
        vec![],
        SpecScope::new(vec![], vec![]),
        vec![],
        vec![],
        vec![],
        vec![],
        None,
    )
    .unwrap()
}

fn money_catalogue() -> CatalogueDocument {
    let mut doc = CatalogueDocument::new(
        5,
        CrateName::new("domain").unwrap(),
        LayerId::try_new("domain").unwrap(),
    );
    doc.insert_type(
        TypeName::new("Money").unwrap(),
        TypeEntry::new(
            domain::tddd::catalogue_v2::roles::ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::Struct(StructKind::new(StructShape::Unit, None)),
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        ),
    );
    doc.insert_trait(
        TraitName::new("MyPort").unwrap(),
        TraitEntry::new(
            ItemAction::Add,
            ContractRole::SecondaryPort,
            vec![
                MethodDeclaration::new(
                    MethodName::new("load").unwrap(),
                    Some(SelfReceiver::SharedRef),
                    vec![],
                    TypeRef::new("Result<CacheDocument, VerifyCacheError>").unwrap(),
                    false,
                    false,
                    vec![],
                    vec![],
                    vec![],
                    None,
                ),
                MethodDeclaration::new(
                    MethodName::new("save").unwrap(),
                    Some(SelfReceiver::SharedRef),
                    vec![],
                    TypeRef::new("Result<(), DiagnosticMessage>").unwrap(),
                    false,
                    false,
                    vec![],
                    vec![],
                    vec![],
                    None,
                ),
            ],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        ),
    );
    doc.push_trait_impl(TraitImplDeclV2::new(
        TypeRef::new("MyPort").unwrap(),
        TypeRef::new("Money").unwrap(),
    ));
    doc
}

fn empty_catalogue() -> CatalogueDocument {
    CatalogueDocument::new(
        5,
        CrateName::new("domain").unwrap(),
        LayerId::try_new("domain").unwrap(),
    )
}

fn voluntary_edge(entry_key: &str, element_id: &str) -> TestObligationEdgeId {
    TestObligationEdgeId::new(
        CatalogueEntryKey::try_new(entry_key.to_owned()).unwrap(),
        TestObligationAnchorId::try_new(
            "track/items/agent-dispatch-cost-reduction-2026-07-13/spec.json".to_owned(),
            element_id.to_owned(),
        )
        .unwrap(),
    )
}

fn type_entry(kind: TypeKindV2, role: DataRole) -> TypeEntry {
    TypeEntry::new(
        domain::tddd::catalogue_v2::roles::ItemAction::Add,
        role,
        kind,
        vec![],
        vec![],
        vec![],
        ModulePath::root(),
        None,
        vec![],
        vec![],
    )
}

fn catalogue_with_type_entries(
    crate_name: &str,
    layer: &str,
    entries: Vec<(&str, TypeEntry)>,
) -> CatalogueDocument {
    let mut catalogue = CatalogueDocument::new(
        5,
        CrateName::new(crate_name).unwrap(),
        LayerId::try_new(layer).unwrap(),
    );
    for (name, entry) in entries {
        catalogue.insert_type(TypeName::new(name).unwrap(), entry);
    }
    catalogue
}

struct Harness {
    fulfillment_driver: Arc<ScriptedFulfillment>,
    waiver_driver: Arc<ScriptedWaiver>,
    fulfillment_cache: Arc<CapFulfillmentCache>,
    waiver_cache: Arc<CapWaiverCache>,
    interactor: EvaluateTestObligationsInteractor,
}

fn harness(
    obligations: Option<ObligationsDocument>,
    bindings: Option<TestBindingsDocument>,
    fast: ObligationFulfillmentVerdict,
    last: ObligationFulfillmentVerdict,
    waiver: WaiverVerdict,
) -> Harness {
    harness_with_scanner(obligations, bindings, fast, last, waiver, Arc::new(StubScanner))
}

fn harness_with_scanner(
    obligations: Option<ObligationsDocument>,
    bindings: Option<TestBindingsDocument>,
    fast: ObligationFulfillmentVerdict,
    last: ObligationFulfillmentVerdict,
    waiver: WaiverVerdict,
    scanner: Arc<dyn TestSourceScannerPort + Send + Sync>,
) -> Harness {
    harness_with_scanner_and_caches(obligations, bindings, fast, last, waiver, scanner, None, None)
}

fn harness_with_existing_caches(
    obligations: Option<ObligationsDocument>,
    bindings: Option<TestBindingsDocument>,
    fast: ObligationFulfillmentVerdict,
    last: ObligationFulfillmentVerdict,
    waiver: WaiverVerdict,
    existing_fulfillment: Option<ObligationFulfillmentCacheDocument>,
    existing_waiver: Option<WaiverCacheDocument>,
) -> Harness {
    harness_with_scanner_and_caches(
        obligations,
        bindings,
        fast,
        last,
        waiver,
        Arc::new(StubScanner),
        existing_fulfillment,
        existing_waiver,
    )
}

#[allow(clippy::too_many_arguments)]
fn harness_with_scanner_and_caches(
    obligations: Option<ObligationsDocument>,
    bindings: Option<TestBindingsDocument>,
    fast: ObligationFulfillmentVerdict,
    last: ObligationFulfillmentVerdict,
    waiver: WaiverVerdict,
    scanner: Arc<dyn TestSourceScannerPort + Send + Sync>,
    existing_fulfillment: Option<ObligationFulfillmentCacheDocument>,
    existing_waiver: Option<WaiverCacheDocument>,
) -> Harness {
    harness_with_read_models(
        obligations,
        bindings,
        fast,
        last,
        waiver,
        Arc::clone(&scanner),
        existing_fulfillment,
        existing_waiver,
        spec_doc(),
        money_catalogue(),
    )
}

#[allow(clippy::too_many_arguments)]
fn harness_with_read_models(
    obligations: Option<ObligationsDocument>,
    bindings: Option<TestBindingsDocument>,
    fast: ObligationFulfillmentVerdict,
    last: ObligationFulfillmentVerdict,
    waiver: WaiverVerdict,
    scanner: Arc<dyn TestSourceScannerPort + Send + Sync>,
    existing_fulfillment: Option<ObligationFulfillmentCacheDocument>,
    existing_waiver: Option<WaiverCacheDocument>,
    spec: SpecDocument,
    catalogue: CatalogueDocument,
) -> Harness {
    harness_with_read_models_and_config(
        obligations,
        bindings,
        fast,
        last,
        waiver,
        Arc::clone(&scanner),
        existing_fulfillment,
        existing_waiver,
        spec,
        catalogue,
        config(),
    )
}

#[allow(clippy::too_many_arguments)]
fn harness_with_read_models_and_config(
    obligations: Option<ObligationsDocument>,
    bindings: Option<TestBindingsDocument>,
    fast: ObligationFulfillmentVerdict,
    last: ObligationFulfillmentVerdict,
    waiver: WaiverVerdict,
    scanner: Arc<dyn TestSourceScannerPort + Send + Sync>,
    existing_fulfillment: Option<ObligationFulfillmentCacheDocument>,
    existing_waiver: Option<WaiverCacheDocument>,
    spec: SpecDocument,
    catalogue: CatalogueDocument,
    cfg: TestObligationEvaluateConfig,
) -> Harness {
    harness_with_read_models_and_config_impl(
        obligations,
        bindings,
        fast,
        last,
        waiver,
        scanner,
        existing_fulfillment,
        existing_waiver,
        spec,
        catalogue,
        cfg,
    )
}

#[allow(clippy::too_many_arguments)]
fn harness_with_read_models_and_config_impl(
    obligations: Option<ObligationsDocument>,
    bindings: Option<TestBindingsDocument>,
    fast: ObligationFulfillmentVerdict,
    last: ObligationFulfillmentVerdict,
    waiver: WaiverVerdict,
    scanner: Arc<dyn TestSourceScannerPort + Send + Sync>,
    existing_fulfillment: Option<ObligationFulfillmentCacheDocument>,
    existing_waiver: Option<WaiverCacheDocument>,
    spec: SpecDocument,
    catalogue: CatalogueDocument,
    cfg: TestObligationEvaluateConfig,
) -> Harness {
    let fulfillment_driver = Arc::new(ScriptedFulfillment {
        fast,
        last,
        verifier_error: Mutex::new(None),
        calibration: Mutex::new(None),
        calibration_contradiction: Mutex::new(None),
        calibration_substitution: Mutex::new(None),
        calibration_central_unverified: Mutex::new(None),
        calibration_calls: Mutex::new(0),
        calibration_probe_sources: Mutex::new(Vec::new()),
        tiers: Mutex::new(Vec::new()),
        declarations: Mutex::new(Vec::new()),
    });
    let waiver_driver = Arc::new(ScriptedWaiver {
        verdict: waiver,
        calls: Mutex::new(0),
        tiers: Mutex::new(Vec::new()),
        declarations: Mutex::new(Vec::new()),
    });
    let fulfillment_cache = Arc::new(CapFulfillmentCache {
        loaded: Mutex::new(existing_fulfillment),
        saved: Mutex::new(None),
        save_error: Mutex::new(None),
    });
    let waiver_cache = Arc::new(CapWaiverCache {
        loaded: Mutex::new(existing_waiver),
        saved: Mutex::new(None),
        save_error: Mutex::new(None),
    });
    let interactor = EvaluateTestObligationsInteractor::new(
        Arc::new(StubObligations(obligations)),
        Arc::new(StubBindings(bindings)),
        Arc::clone(&scanner),
        Arc::clone(&fulfillment_driver)
            as Arc<
                dyn SemanticEscalationDriverPort<
                        ObligationFulfillmentPair,
                        ObligationFulfillmentCacheKey,
                        ObligationFulfillmentVerdict,
                        SemanticVerifierError,
                    >,
            >,
        Arc::clone(&waiver_driver)
            as Arc<
                dyn SemanticEscalationDriverPort<
                        WaiverPair,
                        WaiverCacheKey,
                        WaiverVerdict,
                        SemanticVerifierError,
                    >,
            >,
        Arc::clone(&fulfillment_cache) as Arc<dyn ObligationFulfillmentCachePort + Send + Sync>,
        Arc::clone(&waiver_cache) as Arc<dyn WaiverCachePort + Send + Sync>,
        fulfillment_verifier_fingerprint(),
        waiver_verifier_fingerprint(),
        cfg,
        Arc::new(StubSpec(spec)),
        Arc::new(StubCatalogue(catalogue)),
        Arc::new(SumHasher),
    );
    Harness { fulfillment_driver, waiver_driver, fulfillment_cache, waiver_cache, interactor }
}

fn interactor_with_bounded_drivers(
    bindings: TestBindingsDocument,
    config: TestObligationEvaluateConfig,
    fulfillment_driver: Arc<BoundedFulfillmentDriver>,
    waiver_driver: Arc<BoundedWaiverDriver>,
) -> EvaluateTestObligationsInteractor {
    EvaluateTestObligationsInteractor::new(
        Arc::new(StubObligations(Some(obligations_doc()))),
        Arc::new(StubBindings(Some(bindings))),
        Arc::new(StubScanner),
        fulfillment_driver
            as Arc<
                dyn SemanticEscalationDriverPort<
                        ObligationFulfillmentPair,
                        ObligationFulfillmentCacheKey,
                        ObligationFulfillmentVerdict,
                        SemanticVerifierError,
                    > + Send
                    + Sync,
            >,
        waiver_driver
            as Arc<
                dyn SemanticEscalationDriverPort<
                        WaiverPair,
                        WaiverCacheKey,
                        WaiverVerdict,
                        SemanticVerifierError,
                    > + Send
                    + Sync,
            >,
        Arc::new(CapFulfillmentCache::default()),
        Arc::new(CapWaiverCache::default()),
        fulfillment_verifier_fingerprint(),
        waiver_verifier_fingerprint(),
        config,
        Arc::new(StubSpec(spec_doc())),
        Arc::new(StubCatalogue(money_catalogue())),
        Arc::new(SumHasher),
    )
}

fn command() -> EvaluateTestObligationsCommand {
    EvaluateTestObligationsCommand::new(
        track(),
        "track/my-track".to_owned(),
        vec![PathBuf::from("domain-types.json")],
        PathBuf::from("track/items/my-track/spec.json"),
    )
}

fn fulfilled() -> ObligationFulfillmentVerdict {
    ObligationFulfillmentVerdict::Fulfilled {
        citation: EvidenceCitation::try_new("asserts positivity".to_owned()).unwrap(),
    }
}

fn fulfillment_fail() -> ObligationFulfillmentVerdict {
    fulfillment_fail_for_category(FulfillmentFailCategory::CentralUnverified)
}

fn fulfillment_fail_for_category(
    category: FulfillmentFailCategory,
) -> ObligationFulfillmentVerdict {
    let category_name = category.as_kebab();
    ObligationFulfillmentVerdict::Fail {
        category,
        reason: DiagnosticMessage::try_new(format!("{category_name} calibration failure")).unwrap(),
    }
}

fn sum_hash(bytes: &[u8]) -> ContentHash {
    SumHasher.sha256(bytes)
}

fn cached_fulfillment_doc(
    verdict: ObligationFulfillmentVerdict,
) -> ObligationFulfillmentCacheDocument {
    cached_fulfillment_doc_with_fingerprint(verdict, Some(fulfillment_verifier_fingerprint()))
}

fn cached_fulfillment_doc_with_fingerprint(
    verdict: ObligationFulfillmentVerdict,
    verifier_fingerprint: Option<VerifierPromptFingerprint>,
) -> ObligationFulfillmentCacheDocument {
    let obligation = obligation();
    let declaration =
        crate::test_obligation::obligation_declaration_text(&[money_catalogue()], &obligation)
            .unwrap();
    let key = ObligationFulfillmentCacheKey::new(
        BoundTestsSetHash::new(sum_hash("assert!(money.is_positive());\n".as_bytes())),
        DeclarationHash::new(sum_hash(declaration.as_bytes())),
        AnchorTextHash::new(sum_hash(anchor_text().as_bytes())),
    );
    ObligationFulfillmentCacheDocument::new(
        track(),
        vec![cache_entry(edge(), obligation.id().clone(), key, verdict, verifier_fingerprint)],
    )
}

fn cached_waiver_doc(verdict: WaiverVerdict) -> WaiverCacheDocument {
    cached_waiver_doc_with_fingerprint(verdict, Some(waiver_verifier_fingerprint()))
}

fn cached_waiver_doc_with_fingerprint(
    verdict: WaiverVerdict,
    verifier_fingerprint: Option<VerifierPromptFingerprint>,
) -> WaiverCacheDocument {
    let declaration =
        crate::test_obligation::find_declaration_text(&[money_catalogue()], "Money").unwrap();
    let key = WaiverCacheKey::new(
        WaivedReasonHash::new(sum_hash(waiver_reason().as_str().as_bytes())),
        DeclarationHash::new(sum_hash(declaration.as_bytes())),
        AnchorTextHash::new(sum_hash(anchor_text().as_bytes())),
    );
    WaiverCacheDocument::new(
        track(),
        vec![WaiverCacheEntry::new(
            edge(),
            Some(obligation().id().clone()),
            key,
            verdict,
            verifier_fingerprint,
        )],
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn test_config_validation_rejects_out_of_range() {
    // AC-15 / IN-01.
    assert!(TestObligationEvaluateConfig::try_new(101, 90, 1).is_err());
    assert!(TestObligationEvaluateConfig::try_new(10, 0, 1).is_err());
    assert!(TestObligationEvaluateConfig::try_new(10, 90, 0).is_err());
    assert!(TestObligationEvaluateConfig::try_new(0, 100, 1).is_ok());
}

#[test]
fn test_config_exposes_validated_calibration_bounds() {
    let config = TestObligationEvaluateConfig::try_new(10, 90, 4).unwrap();

    assert_eq!(config.injection_rate(), 10);
    assert_eq!(config.detection_threshold().get(), 90);
    assert_eq!(config.parallelism(), 4);
}

fn assert_evaluator_fan_out_bound(config: TestObligationEvaluateConfig, expected_bound: usize) {
    // 16 records in each lane make both production fan-outs exceed the
    // default bound. The 32 total records also produce four calibration
    // probes, so that fan-out exceeds a three-worker bound as well.
    let fulfillment_tracker = DispatchTracker::new();
    let waiver_tracker = DispatchTracker::new();
    let interactor = interactor_with_bounded_drivers(
        repeated_lane_bindings(16),
        config,
        Arc::new(BoundedFulfillmentDriver {
            tracker: fulfillment_tracker.clone(),
            real_tracker: None,
        }),
        Arc::new(BoundedWaiverDriver { tracker: waiver_tracker.clone() }),
    );

    let outcome = run(interactor.execute(&command())).unwrap();

    assert_eq!(outcome.pass_count(), 32);
    assert_eq!(fulfillment_tracker.peak(), expected_bound);
    assert_eq!(waiver_tracker.peak(), expected_bound);
}

#[test]
fn test_config_parallelism_one_serializes_all_evaluation_fan_outs() {
    assert_evaluator_fan_out_bound(TestObligationEvaluateConfig::try_new(10, 90, 1).unwrap(), 1);
}

#[test]
fn test_default_config_parallelism_bounds_all_evaluation_fan_outs() {
    let config = TestObligationEvaluateConfig::default();
    assert_eq!(config.parallelism(), 4);

    assert_evaluator_fan_out_bound(config.clone(), config.parallelism());
}

/// A waiver record on an edge suppresses the fulfillment record's
/// adjudication for that edge (the check gate resolves waived edges through
/// their waiver, so judging the bound tests against the waived claim would
/// double-adjudicate and fail on behavior the waiver already covers).
#[test]
fn test_waiver_edge_suppresses_fulfillment_adjudication_for_that_edge() {
    let probe_tracker = DispatchTracker::new();
    let real_fulfillment_tracker = DispatchTracker::new();
    let waiver_tracker = DispatchTracker::new();
    let bindings = TestBindingsDocument::new(
        track(),
        vec![
            TestBindingRecord::Fulfillment {
                obligation_id: obligation().id().clone(),
                tests: NonEmptyTestLocations::try_new(vec![location()]).unwrap(),
            },
            TestBindingRecord::Waiver { edge_id: edge(), reason: waiver_reason() },
        ],
    );
    let interactor = interactor_with_bounded_drivers(
        bindings,
        TestObligationEvaluateConfig::default(),
        Arc::new(BoundedFulfillmentDriver {
            tracker: probe_tracker.clone(),
            real_tracker: Some(real_fulfillment_tracker.clone()),
        }),
        Arc::new(BoundedWaiverDriver { tracker: waiver_tracker.clone() }),
    );

    let outcome = run(interactor.execute(&command())).unwrap();

    // Only the waiver lane adjudicates: the obligation's sole edge is waived.
    // The AC-08 calibration probes still flow (probe_tracker), but no real
    // fulfillment pair may reach the driver.
    assert_eq!(outcome.pass_count(), 1);
    assert_eq!(real_fulfillment_tracker.peak(), 0, "waived edge must not be re-adjudicated");
}

#[test]
fn test_new_accepts_and_wires_declared_dependencies() {
    let fulfillment_driver = Arc::new(ScriptedFulfillment {
        fast: fulfilled(),
        last: fulfillment_fail(),
        verifier_error: Mutex::new(None),
        calibration: Mutex::new(None),
        calibration_contradiction: Mutex::new(None),
        calibration_substitution: Mutex::new(None),
        calibration_central_unverified: Mutex::new(None),
        calibration_calls: Mutex::new(0),
        calibration_probe_sources: Mutex::new(Vec::new()),
        tiers: Mutex::new(Vec::new()),
        declarations: Mutex::new(Vec::new()),
    });
    let waiver_driver = Arc::new(ScriptedWaiver {
        verdict: WaiverVerdict::Pending,
        calls: Mutex::new(0),
        tiers: Mutex::new(Vec::new()),
        declarations: Mutex::new(Vec::new()),
    });
    let fulfillment_cache = Arc::new(CapFulfillmentCache::default());
    let waiver_cache = Arc::new(CapWaiverCache::default());

    let interactor = EvaluateTestObligationsInteractor::new(
        Arc::new(StubObligations(Some(obligations_doc()))),
        Arc::new(StubBindings(Some(fulfillment_bindings()))),
        Arc::new(StubScanner),
        Arc::clone(&fulfillment_driver)
            as Arc<
                dyn SemanticEscalationDriverPort<
                        ObligationFulfillmentPair,
                        ObligationFulfillmentCacheKey,
                        ObligationFulfillmentVerdict,
                        SemanticVerifierError,
                    >,
            >,
        Arc::clone(&waiver_driver)
            as Arc<
                dyn SemanticEscalationDriverPort<
                        WaiverPair,
                        WaiverCacheKey,
                        WaiverVerdict,
                        SemanticVerifierError,
                    >,
            >,
        Arc::clone(&fulfillment_cache) as Arc<dyn ObligationFulfillmentCachePort + Send + Sync>,
        Arc::clone(&waiver_cache) as Arc<dyn WaiverCachePort + Send + Sync>,
        fulfillment_verifier_fingerprint(),
        waiver_verifier_fingerprint(),
        config(),
        Arc::new(StubSpec(spec_doc())),
        Arc::new(StubCatalogue(money_catalogue())),
        Arc::new(SumHasher),
    );

    let outcome = run(interactor.execute(&command())).unwrap();

    assert_eq!(outcome.pass_count(), 1);
    assert_eq!(fulfillment_driver.tiers.lock().unwrap().as_slice(), &[ModelTier::Fast]);
    assert_eq!(fulfillment_cache.saved.lock().unwrap().as_ref().unwrap().entries().len(), 1);
}

/// A voluntary binding may not target an edge already owned by derived obligations.
#[test]
fn test_voluntary_binding_with_derived_owners_returns_consistency_error() {
    let fulfillment_driver = Arc::new(ScriptedFulfillment {
        fast: fulfilled(),
        last: fulfillment_fail(),
        verifier_error: Mutex::new(None),
        calibration: Mutex::new(None),
        calibration_contradiction: Mutex::new(None),
        calibration_substitution: Mutex::new(None),
        calibration_central_unverified: Mutex::new(None),
        calibration_calls: Mutex::new(0),
        calibration_probe_sources: Mutex::new(Vec::new()),
        tiers: Mutex::new(Vec::new()),
        declarations: Mutex::new(Vec::new()),
    });
    let waiver_driver = Arc::new(ScriptedWaiver {
        verdict: WaiverVerdict::Pending,
        calls: Mutex::new(0),
        tiers: Mutex::new(Vec::new()),
        declarations: Mutex::new(Vec::new()),
    });
    let fulfillment_cache = Arc::new(CapFulfillmentCache::default());
    // Both obligations own the Money × IN-05 edge (same entry key, same anchor).
    let obligations =
        ObligationsDocument::new(track(), vec![obligation(), trait_impl_obligation()]);
    let bindings = TestBindingsDocument::new(
        track(),
        vec![TestBindingRecord::VoluntaryBinding {
            edge_id: edge(),
            tests: NonEmptyTestLocations::try_new(vec![location()]).unwrap(),
        }],
    );

    let interactor = EvaluateTestObligationsInteractor::new(
        Arc::new(StubObligations(Some(obligations))),
        Arc::new(StubBindings(Some(bindings))),
        Arc::new(StubScanner),
        Arc::clone(&fulfillment_driver)
            as Arc<
                dyn SemanticEscalationDriverPort<
                        ObligationFulfillmentPair,
                        ObligationFulfillmentCacheKey,
                        ObligationFulfillmentVerdict,
                        SemanticVerifierError,
                    >,
            >,
        waiver_driver
            as Arc<
                dyn SemanticEscalationDriverPort<
                        WaiverPair,
                        WaiverCacheKey,
                        WaiverVerdict,
                        SemanticVerifierError,
                    >,
            >,
        Arc::clone(&fulfillment_cache) as Arc<dyn ObligationFulfillmentCachePort + Send + Sync>,
        Arc::new(CapWaiverCache::default()) as Arc<dyn WaiverCachePort + Send + Sync>,
        fulfillment_verifier_fingerprint(),
        waiver_verifier_fingerprint(),
        config(),
        Arc::new(StubSpec(spec_doc())),
        Arc::new(StubCatalogue(money_catalogue())),
        Arc::new(SumHasher),
    );

    let result = run(interactor.execute(&command()));

    assert!(matches!(
        result,
        Err(ObligationEvaluateError::BindingConsistency(
            domain::tddd::test_obligation::errors::TestBindingConsistencyError::VoluntaryBindingOwnsDerivedObligation { edge_id }
        )) if edge_id == edge()
    ));
    assert!(fulfillment_cache.saved.lock().unwrap().is_none());
}

/// A waiver on an edge owned by SEVERAL obligations must be adjudicated once
/// per owner. Each declaration can make a different claim about the same edge,
/// so taking the first owner would leave the other claim unverified.
#[test]
fn test_waiver_adjudicates_every_obligation_owning_the_edge() {
    let h = harness(
        Some(ObligationsDocument::new(track(), vec![obligation(), trait_impl_obligation()])),
        Some(waiver_bindings()),
        fulfilled(),
        fulfillment_fail(),
        WaiverVerdict::Waived {
            citation: EvidenceCitation::try_new("multi-owner waiver".to_owned()).unwrap(),
        },
    );

    let outcome = run(h.interactor.execute(&command())).unwrap();

    assert_eq!(outcome.pass_count(), 2, "each owning obligation must be adjudicated");
    assert_eq!(*h.waiver_driver.calls.lock().unwrap(), 2);
    let cache = h.waiver_cache.saved.lock().unwrap();
    let entries = cache.as_ref().unwrap().entries();
    assert_eq!(entries.len(), 2);
    assert!(entries.iter().any(|entry| entry.obligation_id() == Some(obligation().id())));
    assert!(
        entries.iter().any(|entry| entry.obligation_id() == Some(trait_impl_obligation().id()))
    );
    let declarations = h.waiver_driver.declarations.lock().unwrap();
    assert!(declarations.iter().any(|declaration| declaration.contains("kind")));
    assert!(declarations.iter().any(|declaration| declaration.contains("trait_ref")));
}

/// Ownership validation runs before waiver precedence, so an invalid voluntary
/// binding cannot be hidden by a waiver for the same derived edge.
#[test]
fn test_waiver_does_not_hide_invalid_voluntary_binding() {
    let probe_tracker = DispatchTracker::new();
    let real_fulfillment_tracker = DispatchTracker::new();
    let waiver_tracker = DispatchTracker::new();
    let bindings = TestBindingsDocument::new(
        track(),
        vec![
            TestBindingRecord::VoluntaryBinding {
                edge_id: edge(),
                tests: NonEmptyTestLocations::try_new(vec![location()]).unwrap(),
            },
            TestBindingRecord::Waiver { edge_id: edge(), reason: waiver_reason() },
        ],
    );
    let interactor = interactor_with_bounded_drivers(
        bindings,
        TestObligationEvaluateConfig::default(),
        Arc::new(BoundedFulfillmentDriver {
            tracker: probe_tracker.clone(),
            real_tracker: Some(real_fulfillment_tracker.clone()),
        }),
        Arc::new(BoundedWaiverDriver { tracker: waiver_tracker.clone() }),
    );

    let result = run(interactor.execute(&command()));

    assert!(matches!(
        result,
        Err(ObligationEvaluateError::BindingConsistency(
            domain::tddd::test_obligation::errors::TestBindingConsistencyError::VoluntaryBindingOwnsDerivedObligation { edge_id }
        )) if edge_id == edge()
    ));
    assert_eq!(
        real_fulfillment_tracker.peak(),
        0,
        "ownership validation must run before fulfillment dispatch"
    );
}

/// Valid voluntary bindings always target catalogue-only edges and consume one
/// calibration pair; waivers continue to scale with derived ownership.
#[test]
fn test_production_pair_count_counts_valid_voluntary_binding_once() {
    let obligations =
        ObligationsDocument::new(track(), vec![obligation(), trait_impl_obligation()]);
    let bindings = TestBindingsDocument::new(
        track(),
        vec![TestBindingRecord::VoluntaryBinding {
            edge_id: edge(),
            tests: NonEmptyTestLocations::try_new(vec![location()]).unwrap(),
        }],
    );
    assert_eq!(super::production_pair_count(&obligations, &bindings), 1);

    let waiver = TestBindingsDocument::new(
        track(),
        vec![TestBindingRecord::Waiver { edge_id: edge(), reason: waiver_reason() }],
    );
    assert_eq!(super::production_pair_count(&obligations, &waiver), 2);

    let unowned_edge = TestObligationEdgeId::new(
        CatalogueEntryKey::try_new("Money".to_owned()).unwrap(),
        TestObligationAnchorId::try_new("spec.json".to_owned(), "IN-06".to_owned()).unwrap(),
    );
    let unowned = TestBindingsDocument::new(
        track(),
        vec![TestBindingRecord::VoluntaryBinding {
            edge_id: unowned_edge.clone(),
            tests: NonEmptyTestLocations::try_new(vec![location()]).unwrap(),
        }],
    );
    assert_eq!(
        super::production_pair_count(&obligations, &unowned),
        1,
        "catalogue-only voluntary edges keep the minimum budget of one"
    );

    let unowned_waiver = TestBindingsDocument::new(
        track(),
        vec![TestBindingRecord::Waiver { edge_id: unowned_edge, reason: waiver_reason() }],
    );
    assert_eq!(
        super::production_pair_count(&obligations, &unowned_waiver),
        1,
        "catalogue-only waiver edges keep the minimum budget of one"
    );
}

#[test]
fn test_fulfilled_on_fast_counts_pass_without_escalation() {
    // AC-06: a fast pass is authoritative; no escalation to final.
    let h = harness(
        Some(obligations_doc()),
        Some(fulfillment_bindings()),
        fulfilled(),
        fulfillment_fail(),
        WaiverVerdict::Pending,
    );
    let outcome = run(h.interactor.execute(&command())).unwrap();
    assert_eq!(outcome.pass_count(), 1);
    assert_eq!(outcome.fail_count(), 0);
    assert_eq!(outcome.known_bad_detection_rate().value(), 100);
    assert_eq!(*h.fulfillment_driver.calibration_calls.lock().unwrap(), 3);
    assert_eq!(h.fulfillment_driver.tiers.lock().unwrap().as_slice(), &[ModelTier::Fast]);
    // The verdict is frozen in the fulfillment cache.
    assert_eq!(h.fulfillment_cache.saved.lock().unwrap().clone().unwrap().entries().len(), 1);
}

#[test]
fn test_stand_in_verifier_port_error_fails_closed_without_pass_verdict() {
    let h = harness(
        Some(obligations_doc()),
        Some(fulfillment_bindings()),
        fulfilled(),
        fulfilled(),
        WaiverVerdict::Pending,
    );
    *h.fulfillment_driver.verifier_error.lock().unwrap() =
        Some(DiagnosticMessage::try_new("provider unavailable".to_owned()).unwrap());

    let result = run(h.interactor.execute(&command()));

    assert!(matches!(
        result,
        Err(ObligationEvaluateError::VerifierPort(SemanticVerifierError::VerifierPort(message)))
            if message.as_str() == "provider unavailable"
    ));
    assert!(h.fulfillment_cache.saved.lock().unwrap().is_none());
}

#[test]
fn test_absent_fulfillment_cache_is_rebuilt_with_resolved_bound_tests() {
    let h = harness(
        Some(obligations_doc()),
        Some(fulfillment_bindings()),
        fulfilled(),
        fulfilled(),
        WaiverVerdict::Pending,
    );

    let outcome = run(h.interactor.execute(&command())).unwrap();

    assert_eq!(outcome.pass_count(), 1);
    assert_eq!(h.fulfillment_driver.tiers.lock().unwrap().as_slice(), &[ModelTier::Fast]);
    let saved = h.fulfillment_cache.saved.lock().unwrap().clone().unwrap();
    assert_eq!(saved.entries().len(), 1);
    assert!(saved.entries()[0].bound_tests().is_some_and(|tests| !tests.as_slice().is_empty()));
    assert_eq!(
        saved.entries()[0].key().bound_tests_set_hash(),
        &BoundTestsSetHash::new(sum_hash("assert!(money.is_positive());\n".as_bytes()))
    );
}

#[test]
fn test_evaluation_persists_the_exact_resolved_bound_test_locations() {
    let h = harness(
        Some(obligations_doc()),
        Some(fulfillment_bindings()),
        fulfilled(),
        fulfilled(),
        WaiverVerdict::Pending,
    );

    let outcome = run(h.interactor.execute(&command())).unwrap();

    assert_eq!(outcome.pass_count(), 1);
    let saved = h.fulfillment_cache.saved.lock().unwrap().clone().unwrap();
    assert_eq!(saved.entries().len(), 1);
    assert_eq!(saved.entries()[0].bound_tests().unwrap().as_slice(), &[location()]);
}

#[test]
fn test_known_bad_probe_undetected_category_fails_closed_without_cache_save() {
    // AC-08: with the default 10% injection rate against a single production
    // pair, calibration issues all three category probes. If the verifier
    // fails to detect any of them, the per-category gate fires before the
    // aggregate threshold check and prevents the caches from being saved.
    let h = harness(
        Some(obligations_doc()),
        Some(fulfillment_bindings()),
        fulfilled(),
        fulfilled(),
        WaiverVerdict::Pending,
    );
    *h.fulfillment_driver.calibration.lock().unwrap() = Some(fulfilled());

    let result = run(h.interactor.execute(&command()));

    assert!(
        matches!(
            &result,
            Err(ObligationEvaluateError::VerifierPort(SemanticVerifierError::VerifierPort(message)))
                if message.as_str().contains("known-bad calibration detected 0 probes for categories")
                    && message.as_str().contains("contradiction")
                    && message.as_str().contains("substitution")
                    && message.as_str().contains("central_unverified")
        ),
        "unexpected result: {result:?}"
    );
    assert_eq!(*h.fulfillment_driver.calibration_calls.lock().unwrap(), 3);
    assert!(h.fulfillment_cache.saved.lock().unwrap().is_none());
    assert!(h.waiver_cache.saved.lock().unwrap().is_none());
}

#[test]
fn test_calibration_probes_exercise_all_three_ac08_categories() {
    // AC-08: a one-production-pair run at the default 10% injection rate
    // still issues at least one deterministic probe for each fulfillment-fail
    // category before the verifier can be declared healthy.
    let h = harness(
        Some(obligations_doc()),
        Some(fulfillment_bindings()),
        fulfilled(),
        fulfilled(),
        WaiverVerdict::Pending,
    );

    let _ = run(h.interactor.execute(&command()));

    let probes = h.fulfillment_driver.calibration_probe_sources.lock().unwrap().clone();
    assert_eq!(probes.len(), 3, "unexpected probe count: {probes:?}");
    assert!(
        probes.iter().any(|s| s.contains("known_bad_calibration_probe_contradiction_")),
        "contradiction probe missing: {probes:?}"
    );
    assert!(
        probes.iter().any(|s| s.contains("known_bad_calibration_probe_substitution_")),
        "substitution probe missing: {probes:?}"
    );
    assert!(
        probes.iter().any(|s| s.contains("known_bad_calibration_probe_central_unverified_")),
        "central-unverified probe missing: {probes:?}"
    );
}

#[test]
fn test_calibration_fails_when_one_category_detects_zero_probes() {
    // AC-08: if the verifier flags contradiction and central-unverified
    // probes but fails to catch the substitution shape, aggregate detection
    // may sit at 2/3 (66%) — below the 90 threshold — but the per-category
    // gate fires first with a message that names the missed category so the
    // operator sees which shape the prompt regressed on.
    let h = harness_with_read_models_and_config(
        Some(obligations_doc()),
        Some(triple_waiver_bindings()),
        fulfilled(),
        fulfilled(),
        WaiverVerdict::Waived { citation: EvidenceCitation::try_new("stub".to_owned()).unwrap() },
        Arc::new(StubScanner),
        None,
        None,
        spec_doc(),
        money_catalogue(),
        config_with_rate(100),
    );
    // Substitution probes will be reported as fulfilled (not detected);
    // the other two categories keep the default `fulfillment_fail`.
    *h.fulfillment_driver.calibration_substitution.lock().unwrap() = Some(fulfilled());

    let result = run(h.interactor.execute(&command()));

    assert!(
        matches!(
            &result,
            Err(ObligationEvaluateError::VerifierPort(SemanticVerifierError::VerifierPort(message)))
                if message.as_str().contains("known-bad calibration detected 0 probes for categories")
                    && message.as_str().contains("substitution")
                    && !message.as_str().contains("contradiction")
                    && !message.as_str().contains("central_unverified")
        ),
        "unexpected result: {result:?}"
    );
    // All three probes were issued before the gate rejected the run.
    assert_eq!(*h.fulfillment_driver.calibration_calls.lock().unwrap(), 3);
    // Calibration failure short-circuits before any production evaluation
    // or cache save (matches the pre-existing single-category failure path).
    assert!(h.fulfillment_cache.saved.lock().unwrap().is_none());
    assert!(h.waiver_cache.saved.lock().unwrap().is_none());
}

#[test]
fn test_calibration_fails_when_verifier_reports_wrong_category_for_shape() {
    // AC-08 requires each issued shape to be detected as its corresponding
    // fail category. A verifier that returns `Fail(Contradiction)` for every
    // known-bad probe is still blind to substitution and central-unverified
    // failures, even though the aggregate fail count is 3/3.
    let h = harness_with_read_models_and_config(
        Some(obligations_doc()),
        Some(triple_waiver_bindings()),
        fulfilled(),
        fulfilled(),
        WaiverVerdict::Waived { citation: EvidenceCitation::try_new("stub".to_owned()).unwrap() },
        Arc::new(StubScanner),
        None,
        None,
        spec_doc(),
        money_catalogue(),
        config_with_rate(100),
    );
    let contradiction = fulfillment_fail_for_category(FulfillmentFailCategory::Contradiction);
    *h.fulfillment_driver.calibration_substitution.lock().unwrap() = Some(contradiction.clone());
    *h.fulfillment_driver.calibration_central_unverified.lock().unwrap() = Some(contradiction);

    let result = run(h.interactor.execute(&command()));

    assert!(
        matches!(
            &result,
            Err(ObligationEvaluateError::VerifierPort(SemanticVerifierError::VerifierPort(message)))
                if message.as_str().contains("known-bad calibration detected 0 probes for categories")
                    && message.as_str().contains("substitution")
                    && message.as_str().contains("central_unverified")
                    && !message.as_str().contains("contradiction")
        ),
        "unexpected result: {result:?}"
    );
    assert_eq!(*h.fulfillment_driver.calibration_calls.lock().unwrap(), 3);
    assert!(h.fulfillment_cache.saved.lock().unwrap().is_none());
    assert!(h.waiver_cache.saved.lock().unwrap().is_none());
}

#[test]
fn test_matching_fulfillment_cache_reuses_frozen_verdict() {
    let h = harness_with_existing_caches(
        Some(obligations_doc()),
        Some(fulfillment_bindings()),
        fulfillment_fail(),
        fulfillment_fail(),
        WaiverVerdict::Pending,
        Some(cached_fulfillment_doc(fulfilled())),
        None,
    );

    let outcome = run(h.interactor.execute(&command())).unwrap();

    assert_eq!(outcome.pass_count(), 1);
    assert_eq!(outcome.fail_count(), 0);
    assert!(h.fulfillment_driver.tiers.lock().unwrap().is_empty());
    let saved = h.fulfillment_cache.saved.lock().unwrap().clone().unwrap();
    assert!(matches!(saved.entries()[0].verdict(), ObligationFulfillmentVerdict::Fulfilled { .. }));
}

#[test]
fn test_evaluation_reuses_cache_when_bound_test_diagnostics_differ() {
    let current = cached_fulfillment_doc(fulfilled()).entries()[0].clone();
    let diagnostic_only_location = TestLocation::new(
        LayerId::try_new("usecase".to_owned()).unwrap(),
        TestModulePath::try_new("fixture".to_owned()).unwrap(),
        TestFunctionName::try_new("diagnostic_only_location".to_owned()).unwrap(),
    );
    let cached_with_different_diagnostics = ObligationFulfillmentCacheEntry::new(
        edge(),
        obligation().id().clone(),
        current.key().clone(),
        current.verdict().clone(),
        ObligationFulfillmentCacheEntryState::Identified {
            verifier_fingerprint: current.verifier_fingerprint().cloned().unwrap(),
            bound_tests: Some(NonEmptyTestLocations::new(
                diagnostic_only_location.clone(),
                Vec::new(),
            )),
        },
    );
    let h = harness_with_existing_caches(
        Some(obligations_doc()),
        Some(fulfillment_bindings()),
        fulfillment_fail(),
        fulfillment_fail(),
        WaiverVerdict::Pending,
        Some(ObligationFulfillmentCacheDocument::new(
            track(),
            vec![cached_with_different_diagnostics],
        )),
        None,
    );

    let outcome = run(h.interactor.execute(&command())).unwrap();

    assert_eq!(outcome.pass_count(), 1);
    assert!(h.fulfillment_driver.tiers.lock().unwrap().is_empty());
}

#[test]
fn test_duplicate_current_fulfillment_entries_reverify_and_replace_cache() {
    let current = cached_fulfillment_doc(fulfilled()).entries()[0].clone();
    let duplicate = cache_entry(
        edge(),
        obligation().id().clone(),
        current.key().clone(),
        ObligationFulfillmentVerdict::Pending,
        Some(fulfillment_verifier_fingerprint()),
    );
    let h = harness_with_existing_caches(
        Some(obligations_doc()),
        Some(fulfillment_bindings()),
        fulfilled(),
        fulfilled(),
        WaiverVerdict::Pending,
        Some(ObligationFulfillmentCacheDocument::new(track(), vec![current, duplicate])),
        None,
    );

    let outcome = run(h.interactor.execute(&command())).unwrap();

    assert_eq!(outcome.pass_count(), 1);
    assert_eq!(h.fulfillment_driver.tiers.lock().unwrap().as_slice(), &[ModelTier::Fast]);
    let saved = h.fulfillment_cache.saved.lock().unwrap().clone().unwrap();
    assert_eq!(saved.entries().len(), 1);
    assert!(matches!(saved.entries()[0].verdict(), ObligationFulfillmentVerdict::Fulfilled { .. }));
}

#[test]
fn test_evaluate_reuses_current_entry_after_all_cache_identity_mismatches() {
    let current = cached_fulfillment_doc(fulfilled()).entries()[0].clone();
    let historical_entries = vec![
        cache_entry(
            edge(),
            obligation().id().clone(),
            ObligationFulfillmentCacheKey::new(
                BoundTestsSetHash::new(sum_hash(b"historical bound-test source")),
                current.key().declaration_hash().clone(),
                current.key().anchor_text_hash().clone(),
            ),
            fulfillment_fail(),
            Some(fulfillment_verifier_fingerprint()),
        ),
        cache_entry(
            edge(),
            obligation().id().clone(),
            ObligationFulfillmentCacheKey::new(
                current.key().bound_tests_set_hash().clone(),
                DeclarationHash::new(sum_hash(b"historical entry declaration")),
                current.key().anchor_text_hash().clone(),
            ),
            fulfillment_fail(),
            Some(fulfillment_verifier_fingerprint()),
        ),
        cache_entry(
            edge(),
            obligation().id().clone(),
            ObligationFulfillmentCacheKey::new(
                current.key().bound_tests_set_hash().clone(),
                current.key().declaration_hash().clone(),
                AnchorTextHash::new(sum_hash(b"historical anchor text")),
            ),
            fulfillment_fail(),
            Some(fulfillment_verifier_fingerprint()),
        ),
        cache_entry(
            edge(),
            obligation().id().clone(),
            current.key().clone(),
            fulfillment_fail(),
            Some(VerifierPromptFingerprint::new(sum_hash(b"historical verifier prompt"))),
        ),
    ];
    let [bound_tests_mismatch, declaration_mismatch, anchor_mismatch, fingerprint_mismatch] =
        historical_entries.as_slice()
    else {
        panic!("fixture must include one row for every cache identity mismatch");
    };
    assert_ne!(
        bound_tests_mismatch.key().bound_tests_set_hash(),
        current.key().bound_tests_set_hash()
    );
    assert_ne!(declaration_mismatch.key().declaration_hash(), current.key().declaration_hash());
    assert_ne!(anchor_mismatch.key().anchor_text_hash(), current.key().anchor_text_hash());
    assert_ne!(fingerprint_mismatch.verifier_fingerprint(), current.verifier_fingerprint());
    let mut entries = historical_entries;
    entries.push(current.clone());
    let h = harness_with_existing_caches(
        Some(obligations_doc()),
        Some(fulfillment_bindings()),
        fulfillment_fail(),
        fulfillment_fail(),
        WaiverVerdict::Pending,
        Some(ObligationFulfillmentCacheDocument::new(track(), entries)),
        None,
    );

    let outcome = run(h.interactor.execute(&command())).unwrap();

    assert_eq!(outcome.pass_count(), 1);
    assert!(h.fulfillment_driver.tiers.lock().unwrap().is_empty());
    let saved = h.fulfillment_cache.saved.lock().unwrap().clone().unwrap();
    assert_eq!(saved.entries().len(), 1);
    assert_eq!(saved.entries()[0].key(), current.key());
    assert_eq!(saved.entries()[0].verifier_fingerprint(), current.verifier_fingerprint());
    assert!(saved.entries()[0].bound_tests().is_some_and(|tests| tests.is_non_empty()));
}

#[test]
fn test_mismatched_fulfillment_fingerprint_reverifies_and_overwrites_entry() {
    let h = harness_with_existing_caches(
        Some(obligations_doc()),
        Some(fulfillment_bindings()),
        fulfilled(),
        fulfilled(),
        WaiverVerdict::Pending,
        Some(cached_fulfillment_doc_with_fingerprint(
            fulfillment_fail(),
            Some(VerifierPromptFingerprint::new(ContentHash::from_bytes([7u8; 32]))),
        )),
        None,
    );

    let outcome = run(h.interactor.execute(&command())).unwrap();

    assert_eq!(outcome.pass_count(), 1);
    assert_eq!(h.fulfillment_driver.tiers.lock().unwrap().as_slice(), &[ModelTier::Fast]);
    let saved = h.fulfillment_cache.saved.lock().unwrap().clone().unwrap();
    assert!(matches!(saved.entries()[0].verdict(), ObligationFulfillmentVerdict::Fulfilled { .. }));
    assert_eq!(
        saved.entries()[0].verifier_fingerprint(),
        Some(&fulfillment_verifier_fingerprint())
    );
}

#[test]
fn test_absent_fulfillment_fingerprint_reverifies_and_overwrites_entry() {
    let h = harness_with_existing_caches(
        Some(obligations_doc()),
        Some(fulfillment_bindings()),
        fulfilled(),
        fulfilled(),
        WaiverVerdict::Pending,
        Some(cached_fulfillment_doc_with_fingerprint(fulfillment_fail(), None)),
        None,
    );

    let outcome = run(h.interactor.execute(&command())).unwrap();

    assert_eq!(outcome.pass_count(), 1);
    assert_eq!(h.fulfillment_driver.tiers.lock().unwrap().as_slice(), &[ModelTier::Fast]);
    let saved = h.fulfillment_cache.saved.lock().unwrap().clone().unwrap();
    assert_eq!(
        saved.entries()[0].verifier_fingerprint(),
        Some(&fulfillment_verifier_fingerprint())
    );
}

#[test]
fn test_voluntary_binding_for_derived_edge_returns_consistency_error() {
    let h = harness(
        Some(obligations_doc()),
        Some(voluntary_bindings()),
        fulfilled(),
        fulfillment_fail(),
        WaiverVerdict::Pending,
    );

    let result = run(h.interactor.execute(&command()));

    assert!(matches!(
        result,
        Err(ObligationEvaluateError::BindingConsistency(
            domain::tddd::test_obligation::errors::TestBindingConsistencyError::VoluntaryBindingOwnsDerivedObligation { edge_id }
        )) if edge_id == edge()
    ));
    assert!(h.fulfillment_cache.saved.lock().unwrap().is_none());
}

#[test]
fn test_ownerless_voluntary_binding_is_evaluated() {
    let bindings = TestBindingsDocument::new(
        track(),
        vec![TestBindingRecord::VoluntaryBinding {
            edge_id: edge(),
            tests: NonEmptyTestLocations::try_new(vec![location()]).unwrap(),
        }],
    );
    let h = harness(
        Some(ObligationsDocument::new(track(), vec![])),
        Some(bindings),
        fulfilled(),
        fulfillment_fail(),
        WaiverVerdict::Pending,
    );

    let outcome = run(h.interactor.execute(&command())).unwrap();

    assert_eq!(outcome.pass_count(), 1);
    assert_eq!(h.fulfillment_driver.tiers.lock().unwrap().as_slice(), &[ModelTier::Fast]);
    assert_eq!(h.fulfillment_cache.saved.lock().unwrap().as_ref().unwrap().entries().len(), 1);
}

#[test]
fn test_plan_voluntary_bindings_for_struct_and_enum_catalogue_entries_uses_llm_lane() {
    let h = harness(
        Some(ObligationsDocument::new(track(), vec![])),
        None,
        fulfilled(),
        fulfillment_fail(),
        WaiverVerdict::Pending,
    );
    let plain_struct = TypeKindV2::Struct(StructKind::new(
        StructShape::Plain { fields: vec![], has_stripped_fields: true },
        None,
    ));
    let domain_catalogue = catalogue_with_type_entries(
        "domain",
        "domain",
        vec![
            (
                "TypeSignalsCurrentInputs",
                type_entry(plain_struct.clone(), DataRole::value_object()),
            ),
            ("TypeSignalsFreshness", type_entry(plain_struct, DataRole::value_object())),
            (
                "TypeSignalsReuseDecision",
                type_entry(TypeKindV2::Enum { variants: vec![] }, DataRole::value_object()),
            ),
        ],
    );
    let infrastructure_catalogue = catalogue_with_type_entries(
        "infrastructure",
        "infrastructure",
        vec![(
            "LoadCatalogueSpecSignalsForViewError",
            type_entry(TypeKindV2::Enum { variants: vec![] }, DataRole::ErrorType),
        )],
    );
    let records = vec![
        TestBindingRecord::VoluntaryBinding {
            edge_id: voluntary_edge("TypeSignalsCurrentInputs", "AC-05"),
            tests: NonEmptyTestLocations::try_new(vec![location()]).unwrap(),
        },
        TestBindingRecord::VoluntaryBinding {
            edge_id: voluntary_edge("TypeSignalsFreshness", "IN-05"),
            tests: NonEmptyTestLocations::try_new(vec![location()]).unwrap(),
        },
        TestBindingRecord::VoluntaryBinding {
            edge_id: voluntary_edge("TypeSignalsReuseDecision", "AC-05"),
            tests: NonEmptyTestLocations::try_new(vec![location()]).unwrap(),
        },
        TestBindingRecord::VoluntaryBinding {
            edge_id: voluntary_edge("LoadCatalogueSpecSignalsForViewError", "IN-05"),
            tests: NonEmptyTestLocations::try_new(vec![location()]).unwrap(),
        },
        TestBindingRecord::VoluntaryBinding {
            edge_id: voluntary_edge("LoadCatalogueSpecSignalsForViewError", "AC-05"),
            tests: NonEmptyTestLocations::try_new(vec![location()]).unwrap(),
        },
    ];
    let catalogues = vec![
        LoadedCatalogueDocument::new(
            Path::new("track/items/agent-dispatch-cost-reduction-2026-07-13/domain-types.json"),
            domain_catalogue,
        ),
        LoadedCatalogueDocument::new(
            Path::new(
                "track/items/agent-dispatch-cost-reduction-2026-07-13/infrastructure-types.json",
            ),
            infrastructure_catalogue,
        ),
    ];

    let bindings = TestBindingsDocument::new(track(), records);
    let plan = h
        .interactor
        .plan_binding_records(
            &bindings,
            &ObligationsDocument::new(track(), vec![]),
            &catalogues,
            &spec_doc_with_in_scope_and_acceptance_criteria(),
            None,
            None,
        )
        .unwrap();

    assert_eq!(plan.len(), bindings.records().len());
    assert!(plan.iter().all(|action| matches!(action, PlannedAction::Fulfillment(_))));
}

#[test]
fn test_trait_impl_obligation_verifies_against_impl_declaration() {
    let h = harness(
        Some(ObligationsDocument::new(track(), vec![trait_impl_obligation()])),
        Some(trait_impl_bindings()),
        fulfilled(),
        fulfillment_fail(),
        WaiverVerdict::Pending,
    );

    let outcome = run(h.interactor.execute(&command())).unwrap();

    assert_eq!(outcome.pass_count(), 1);
    let declarations = h.fulfillment_driver.declarations.lock().unwrap();
    assert_eq!(declarations.len(), 1);
    assert!(declarations[0].contains("trait_ref"));
    assert!(declarations[0].contains("MyPort"));
    assert!(declarations[0].contains("for_type"));
    assert!(declarations[0].contains("Money"));
    assert!(declarations[0].contains("trait_declaration"));
    assert!(declarations[0].contains("load"));
    assert!(declarations[0].contains("save"));
}

#[test]
fn test_fast_fail_escalates_to_final_then_passes() {
    // AC-06: fast → final escalation fires per pair; a final pass wins.
    let h = harness(
        Some(obligations_doc()),
        Some(fulfillment_bindings()),
        fulfillment_fail(),
        fulfilled(),
        WaiverVerdict::Pending,
    );
    let outcome = run(h.interactor.execute(&command())).unwrap();
    assert_eq!(outcome.pass_count(), 1);
    assert_eq!(
        h.fulfillment_driver.tiers.lock().unwrap().as_slice(),
        &[ModelTier::Fast, ModelTier::Final]
    );
}

#[test]
fn test_final_fail_returns_semantic_failures_after_cache_save() {
    let h = harness(
        Some(obligations_doc()),
        Some(fulfillment_bindings()),
        fulfillment_fail(),
        fulfillment_fail(),
        WaiverVerdict::Pending,
    );
    let result = run(h.interactor.execute(&command()));
    let expected_record = EdgeVerdictRecord::new(
        None,
        edge(),
        None,
        None,
        EdgeResolutionOutcome::Fulfillment(fulfillment_fail()),
        None,
    );
    assert!(matches!(
        result,
        Err(ObligationEvaluateError::SemanticFailuresConfirmed { records })
            if records.as_slice() == [expected_record]
    ));
    let saved = h.fulfillment_cache.saved.lock().unwrap().clone().unwrap();
    assert!(matches!(saved.entries()[0].verdict(), ObligationFulfillmentVerdict::Fail { .. }));
}

#[test]
fn test_pending_verdict_requires_human_escalation_after_cache_save() {
    let h = harness(
        Some(obligations_doc()),
        Some(fulfillment_bindings()),
        ObligationFulfillmentVerdict::Pending,
        ObligationFulfillmentVerdict::Pending,
        WaiverVerdict::Pending,
    );

    let result = run(h.interactor.execute(&command()));

    assert!(matches!(
        result,
        Err(ObligationEvaluateError::HumanEscalationRequired { records })
            if records.as_slice()
                == [EdgeVerdictRecord::new(
                    None,
                    edge(),
                    None,
                    None,
                    EdgeResolutionOutcome::Fulfillment(ObligationFulfillmentVerdict::Pending),
                    None,
                )]
    ));
    assert_eq!(
        h.fulfillment_driver.tiers.lock().unwrap().as_slice(),
        &[ModelTier::Fast, ModelTier::Final]
    );
    let saved = h.fulfillment_cache.saved.lock().unwrap().clone().unwrap();
    assert!(matches!(saved.entries()[0].verdict(), ObligationFulfillmentVerdict::Pending));
}

#[test]
fn test_missing_obligation_binding_requires_human_escalation_after_cache_save() {
    let h = harness(
        Some(ObligationsDocument::new(track(), vec![])),
        Some(fulfillment_bindings()),
        fulfilled(),
        fulfilled(),
        WaiverVerdict::Pending,
    );

    let result = run(h.interactor.execute(&command()));

    assert!(matches!(
        result,
        Err(ObligationEvaluateError::HumanEscalationRequired { records })
            if records.as_slice().len() == 1
    ));
    assert!(h.fulfillment_driver.tiers.lock().unwrap().is_empty());
    assert!(h.fulfillment_cache.saved.lock().unwrap().clone().unwrap().entries().is_empty());
}

#[test]
fn test_missing_declaration_requires_human_escalation_after_cache_save() {
    let h = harness_with_read_models(
        Some(obligations_doc()),
        Some(fulfillment_bindings()),
        fulfilled(),
        fulfilled(),
        WaiverVerdict::Pending,
        Arc::new(StubScanner),
        None,
        None,
        spec_doc(),
        empty_catalogue(),
    );

    let result = run(h.interactor.execute(&command()));

    assert!(matches!(
        result,
        Err(ObligationEvaluateError::HumanEscalationRequired { records })
            if records.as_slice()
                == [EdgeVerdictRecord::new(
                    None,
                    edge(),
                    None,
                    None,
                    EdgeResolutionOutcome::Fulfillment(ObligationFulfillmentVerdict::Pending),
                    None,
                )]
    ));
    assert!(h.fulfillment_driver.tiers.lock().unwrap().is_empty());
    assert!(h.fulfillment_cache.saved.lock().unwrap().clone().unwrap().entries().is_empty());
}

#[test]
fn test_missing_spec_anchor_requires_human_escalation_after_cache_save() {
    let h = harness_with_read_models(
        Some(obligations_doc()),
        Some(fulfillment_bindings()),
        fulfilled(),
        fulfilled(),
        WaiverVerdict::Pending,
        Arc::new(StubScanner),
        None,
        None,
        empty_spec_doc(),
        money_catalogue(),
    );

    let result = run(h.interactor.execute(&command()));

    assert!(matches!(
        result,
        Err(ObligationEvaluateError::HumanEscalationRequired { records })
            if records.as_slice().len() == 1
    ));
    assert!(h.fulfillment_driver.tiers.lock().unwrap().is_empty());
    assert!(h.fulfillment_cache.saved.lock().unwrap().clone().unwrap().entries().is_empty());
}

#[test]
fn test_waiver_binding_uses_waiver_lane() {
    let h = harness(
        Some(obligations_doc()),
        Some(waiver_bindings()),
        fulfilled(),
        fulfilled(),
        WaiverVerdict::Waived {
            citation: EvidenceCitation::try_new("integration suite line 42".to_owned()).unwrap(),
        },
    );
    let outcome = run(h.interactor.execute(&command())).unwrap();
    assert_eq!(outcome.pass_count(), 1);
    assert_eq!(*h.waiver_driver.calls.lock().unwrap(), 1);
    assert_eq!(h.waiver_cache.saved.lock().unwrap().clone().unwrap().entries().len(), 1);
}

#[test]
fn test_trait_impl_waiver_verifies_against_impl_declaration() {
    let h = harness(
        Some(ObligationsDocument::new(track(), vec![trait_impl_obligation()])),
        Some(waiver_bindings()),
        fulfilled(),
        fulfilled(),
        WaiverVerdict::Waived {
            citation: EvidenceCitation::try_new("trait impl waiver".to_owned()).unwrap(),
        },
    );

    let outcome = run(h.interactor.execute(&command())).unwrap();

    assert_eq!(outcome.pass_count(), 1);
    assert_eq!(*h.waiver_driver.calls.lock().unwrap(), 1);
    let declarations = h.waiver_driver.declarations.lock().unwrap();
    assert_eq!(declarations.len(), 1);
    assert!(declarations[0].contains("trait_ref"));
    assert!(declarations[0].contains("MyPort"));
    assert!(declarations[0].contains("for_type"));
    assert!(declarations[0].contains("Money"));
    assert!(declarations[0].contains("trait_declaration"));
    assert!(declarations[0].contains("load"));
    assert!(declarations[0].contains("save"));
}

#[test]
fn test_matching_waiver_cache_reuses_frozen_verdict() {
    let h = harness_with_existing_caches(
        Some(obligations_doc()),
        Some(waiver_bindings()),
        fulfillment_fail(),
        fulfillment_fail(),
        WaiverVerdict::Pending,
        None,
        Some(cached_waiver_doc(WaiverVerdict::Waived {
            citation: EvidenceCitation::try_new("cached waiver citation".to_owned()).unwrap(),
        })),
    );

    let outcome = run(h.interactor.execute(&command())).unwrap();

    assert_eq!(outcome.pass_count(), 1);
    assert_eq!(outcome.pending_count(), 0);
    assert_eq!(*h.waiver_driver.calls.lock().unwrap(), 0);
    let saved = h.waiver_cache.saved.lock().unwrap().clone().unwrap();
    assert!(matches!(saved.entries()[0].verdict(), WaiverVerdict::Waived { .. }));
}

#[test]
fn test_mismatched_waiver_fingerprint_reverifies_and_overwrites_entry() {
    let h = harness_with_existing_caches(
        Some(obligations_doc()),
        Some(waiver_bindings()),
        fulfillment_fail(),
        fulfillment_fail(),
        WaiverVerdict::Waived {
            citation: EvidenceCitation::try_new("fresh waiver citation".to_owned()).unwrap(),
        },
        None,
        Some(cached_waiver_doc_with_fingerprint(
            WaiverVerdict::Pending,
            Some(VerifierPromptFingerprint::new(ContentHash::from_bytes([7u8; 32]))),
        )),
    );

    let outcome = run(h.interactor.execute(&command())).unwrap();

    assert_eq!(outcome.pass_count(), 1);
    assert_eq!(*h.waiver_driver.calls.lock().unwrap(), 1);
    let saved = h.waiver_cache.saved.lock().unwrap().clone().unwrap();
    assert!(matches!(saved.entries()[0].verdict(), WaiverVerdict::Waived { .. }));
    assert_eq!(saved.entries()[0].verifier_fingerprint(), Some(&waiver_verifier_fingerprint()));
}

#[test]
fn test_absent_waiver_fingerprint_reverifies_and_overwrites_entry() {
    let h = harness_with_existing_caches(
        Some(obligations_doc()),
        Some(waiver_bindings()),
        fulfillment_fail(),
        fulfillment_fail(),
        WaiverVerdict::Waived {
            citation: EvidenceCitation::try_new("fresh waiver citation".to_owned()).unwrap(),
        },
        None,
        Some(cached_waiver_doc_with_fingerprint(WaiverVerdict::Pending, None)),
    );

    let outcome = run(h.interactor.execute(&command())).unwrap();

    assert_eq!(outcome.pass_count(), 1);
    assert_eq!(*h.waiver_driver.calls.lock().unwrap(), 1);
    let saved = h.waiver_cache.saved.lock().unwrap().clone().unwrap();
    assert_eq!(saved.entries()[0].verifier_fingerprint(), Some(&waiver_verifier_fingerprint()));
}

#[test]
fn test_waiver_cache_save_failure_preserves_fulfillment_reevaluation_state() {
    let h = harness(
        Some(obligations_doc()),
        Some(fulfillment_bindings()),
        fulfilled(),
        fulfilled(),
        WaiverVerdict::Pending,
    );
    *h.waiver_cache.save_error.lock().unwrap() =
        Some(DiagnosticMessage::try_new("waiver write failed".to_owned()).unwrap());

    let result = run(h.interactor.execute(&command()));

    assert!(matches!(
        result,
        Err(ObligationEvaluateError::CachePersistence(VerifyCacheError::Io(_)))
    ));
    assert!(h.fulfillment_cache.saved.lock().unwrap().is_none());
    assert!(h.waiver_cache.saved.lock().unwrap().is_none());
}

#[test]
fn test_fulfillment_cache_save_failure_restores_prior_waiver_document() {
    let previous_waiver = cached_waiver_doc(WaiverVerdict::Waived {
        citation: EvidenceCitation::try_new("previous waiver citation".to_owned()).unwrap(),
    });
    let h = harness_with_existing_caches(
        Some(obligations_doc()),
        Some(fulfillment_bindings()),
        fulfilled(),
        fulfilled(),
        WaiverVerdict::Pending,
        None,
        Some(previous_waiver.clone()),
    );
    *h.fulfillment_cache.save_error.lock().unwrap() =
        Some(DiagnosticMessage::try_new("fulfillment write failed".to_owned()).unwrap());

    let result = run(h.interactor.execute(&command()));

    assert!(matches!(
        result,
        Err(ObligationEvaluateError::CachePersistence(VerifyCacheError::Io(_)))
    ));
    assert_eq!(
        h.waiver_cache.saved.lock().unwrap().clone(),
        Some(previous_waiver),
        "a failed final fulfillment save must not leave a newer waiver document behind"
    );
    assert!(h.fulfillment_cache.saved.lock().unwrap().is_none());
}

#[test]
fn test_absent_artifacts_yield_zero_pairs() {
    // IN-14: existence-based scope - both artifacts absent means zero pairs.
    let h = harness(None, None, fulfilled(), fulfilled(), WaiverVerdict::Pending);
    let outcome = run(h.interactor.execute(&command())).unwrap();
    assert_eq!(outcome.pass_count(), 0);
    assert_eq!(outcome.fail_count(), 0);
    assert_eq!(outcome.pending_count(), 0);
    assert_eq!(*h.fulfillment_driver.calibration_calls.lock().unwrap(), 0);
    assert!(h.fulfillment_cache.saved.lock().unwrap().clone().unwrap().entries().is_empty());
    assert!(h.waiver_cache.saved.lock().unwrap().clone().unwrap().entries().is_empty());
}

#[test]
fn test_obligations_absent_with_bindings_fails_closed() {
    // IN-14: a half-materialized scope is inconsistent and must not clear caches.
    let h = harness(
        None,
        Some(fulfillment_bindings()),
        fulfilled(),
        fulfilled(),
        WaiverVerdict::Pending,
    );

    let result = run(h.interactor.execute(&command()));

    match result {
        Err(ObligationEvaluateError::ArtifactLoad(ArtifactCodecError::DomainInvariant(
            message,
        ))) => {
            assert!(message.as_str().contains("obligations artifact is absent"));
        }
        other => panic!("unexpected result: {other:?}"),
    }
    assert!(h.fulfillment_cache.saved.lock().unwrap().is_none());
    assert!(h.waiver_cache.saved.lock().unwrap().is_none());
}

#[test]
fn test_bindings_absent_with_obligations_fails_closed() {
    // IN-14: a half-materialized scope is inconsistent and must not clear caches.
    let h =
        harness(Some(obligations_doc()), None, fulfilled(), fulfilled(), WaiverVerdict::Pending);

    let result = run(h.interactor.execute(&command()));

    match result {
        Err(ObligationEvaluateError::ArtifactLoad(ArtifactCodecError::DomainInvariant(
            message,
        ))) => {
            assert!(message.as_str().contains("test-bindings artifact is absent"));
        }
        other => panic!("unexpected result: {other:?}"),
    }
    assert!(h.fulfillment_cache.saved.lock().unwrap().is_none());
    assert!(h.waiver_cache.saved.lock().unwrap().is_none());
}

#[test]
fn test_source_scan_error_is_propagated() {
    struct FailingScanner;
    impl TestSourceScannerPort for FailingScanner {
        fn scan_test_body(&self, _l: &TestLocation) -> Result<Option<String>, TestSourceScanError> {
            Err(TestSourceScanError::Io(
                DiagnosticMessage::try_new("read failed".to_owned()).unwrap(),
            ))
        }

        fn hash_test_body(&self, _s: &str) -> TestBodySpanHash {
            TestBodySpanHash::new(ContentHash::from_bytes([0u8; 32]))
        }
    }

    let h = harness_with_scanner(
        Some(obligations_doc()),
        Some(fulfillment_bindings()),
        fulfilled(),
        fulfilled(),
        WaiverVerdict::Pending,
        Arc::new(FailingScanner),
    );
    let result = run(h.interactor.execute(&command()));

    assert!(matches!(
        result,
        Err(ObligationEvaluateError::TestSourceScan(TestSourceScanError::Io(_)))
    ));
}

#[test]
fn test_interactor_uses_its_injected_scanner_for_resolved_bound_tests() {
    struct FailingScanner;
    impl TestSourceScannerPort for FailingScanner {
        fn scan_test_body(&self, _l: &TestLocation) -> Result<Option<String>, TestSourceScanError> {
            Err(TestSourceScanError::Io(
                DiagnosticMessage::try_new("injected scanner failed".to_owned()).unwrap(),
            ))
        }

        fn hash_test_body(&self, _s: &str) -> TestBodySpanHash {
            TestBodySpanHash::new(ContentHash::from_bytes([0u8; 32]))
        }
    }

    let h = harness_with_read_models_and_config(
        Some(obligations_doc()),
        Some(fulfillment_bindings()),
        fulfilled(),
        fulfilled(),
        WaiverVerdict::Pending,
        Arc::new(FailingScanner),
        None,
        None,
        spec_doc(),
        money_catalogue(),
        config(),
    );

    assert!(matches!(
        run(h.interactor.execute(&command())),
        Err(ObligationEvaluateError::TestSourceScan(TestSourceScanError::Io(message)))
            if message.as_str() == "injected scanner failed"
    ));
}

#[test]
fn test_non_active_branch_is_rejected() {
    let h = harness(
        Some(obligations_doc()),
        Some(fulfillment_bindings()),
        fulfilled(),
        fulfilled(),
        WaiverVerdict::Pending,
    );
    let cmd = EvaluateTestObligationsCommand::new(
        track(),
        "main".to_owned(),
        vec![PathBuf::from("domain-types.json")],
        PathBuf::from("spec.json"),
    );
    assert!(matches!(
        run(h.interactor.execute(&cmd)),
        Err(ObligationEvaluateError::TrackNotActive { .. })
    ));
}
