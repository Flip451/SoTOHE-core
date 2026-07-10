//! Unit tests for [`super::EvaluateTestObligationsInteractor`] (T018).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

use domain::tddd::LayerId;
use domain::tddd::catalogue_v2::catalogue_impl_signals_ports::{
    CatalogueDocumentLoaderError, CatalogueDocumentLoaderPort,
};
use domain::tddd::catalogue_v2::roles::{ContractRole, DataRole};
use domain::tddd::catalogue_v2::{
    CatalogueDocument, CrateName, ModulePath, StructKind, StructShape, TraitImplDeclV2, TypeEntry,
    TypeKindV2, TypeName, TypeRef,
};
use domain::tddd::semantic_verify::{
    CatalogueEntryKey, CatalogueEntryRef, CatalogueSectionKey, ModelTier,
};
use domain::tddd::test_obligation::binding::{
    NonEmptyTestLocations, TestBindingRecord, TestBindingsDocument, TestLocation,
};
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
    ObligationFulfillmentCachePort, ObligationsArtifactPort, TestBindingsArtifactPort,
    TestSourceScannerPort, WaiverCachePort,
};

use crate::semantic_verdict_core::driver::{
    SemanticEscalationDriverPort, SemanticEscalationFuture,
};
use crate::test_obligation::hasher::ContentHasherPort;
use domain::tddd::test_obligation::pair::{ObligationFulfillmentPair, WaiverPair};
use domain::tddd::test_obligation::verdict::{
    ObligationFulfillmentCacheDocument, ObligationFulfillmentCacheEntry,
    ObligationFulfillmentCacheKey, ObligationFulfillmentVerdict, WaiverCacheDocument,
    WaiverCacheEntry, WaiverCacheKey, WaiverVerdict,
};
use domain::tddd::test_obligation::vocab::{
    FulfillmentFailCategory, TargetEntryRoleKind, TestObligationKind,
};
use domain::{
    ContentHash, EvidenceCitation, SpecDocument, SpecDocumentLoadError, SpecElementId,
    SpecRequirement, SpecScope, TrackId,
};

use domain::SpecDocumentLoaderPort;

use super::{
    EvaluateTestObligationsApplicationService, EvaluateTestObligationsCommand,
    EvaluateTestObligationsInteractor, TestObligationEvaluateConfig,
};

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

#[derive(Default)]
struct CapFulfillmentCache {
    loaded: Mutex<Option<ObligationFulfillmentCacheDocument>>,
    saved: Mutex<Option<ObligationFulfillmentCacheDocument>>,
}
impl ObligationFulfillmentCachePort for CapFulfillmentCache {
    fn load(
        &self,
        _t: &TrackId,
    ) -> Result<Option<ObligationFulfillmentCacheDocument>, VerifyCacheError> {
        Ok(self.loaded.lock().unwrap().clone())
    }
    fn save(&self, d: &ObligationFulfillmentCacheDocument) -> Result<(), DiagnosticMessage> {
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
        scanner,
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
        scanner,
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
    });
    let waiver_cache = Arc::new(CapWaiverCache {
        loaded: Mutex::new(existing_waiver),
        saved: Mutex::new(None),
        save_error: Mutex::new(None),
    });
    let interactor = EvaluateTestObligationsInteractor::new(
        Arc::new(StubObligations(obligations)),
        Arc::new(StubBindings(bindings)),
        scanner,
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
        vec![ObligationFulfillmentCacheEntry::new(
            edge(),
            obligation.id().clone(),
            key,
            verdict,
            verifier_fingerprint,
        )],
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
        vec![WaiverCacheEntry::new(edge(), key, verdict, verifier_fingerprint)],
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
fn test_voluntary_binding_for_derived_edge_uses_real_obligation_id() {
    let h = harness(
        Some(obligations_doc()),
        Some(voluntary_bindings()),
        fulfilled(),
        fulfillment_fail(),
        WaiverVerdict::Pending,
    );

    let outcome = run(h.interactor.execute(&command())).unwrap();

    assert_eq!(outcome.pass_count(), 1);
    let saved = h.fulfillment_cache.saved.lock().unwrap().clone().unwrap();
    let expected_obligation_id = obligation().id().clone();
    assert_eq!(saved.entries()[0].edge_id(), &edge());
    assert_eq!(saved.entries()[0].obligation_id(), &expected_obligation_id);
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
    assert!(matches!(
        result,
        Err(ObligationEvaluateError::SemanticFailuresConfirmed { records })
            if records.as_slice().len() == 1
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
            if records.as_slice().len() == 1
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
            if records.as_slice().len() == 1
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
fn test_waiver_cache_save_failure_rolls_back_fulfillment_cache() {
    let previous = cached_fulfillment_doc(ObligationFulfillmentVerdict::Pending);
    let h = harness_with_existing_caches(
        Some(obligations_doc()),
        Some(fulfillment_bindings()),
        fulfilled(),
        fulfilled(),
        WaiverVerdict::Pending,
        Some(previous.clone()),
        None,
    );
    *h.waiver_cache.save_error.lock().unwrap() =
        Some(DiagnosticMessage::try_new("waiver write failed".to_owned()).unwrap());

    let result = run(h.interactor.execute(&command()));

    assert!(matches!(
        result,
        Err(ObligationEvaluateError::CachePersistence(VerifyCacheError::Io(_)))
    ));
    assert_eq!(*h.fulfillment_cache.saved.lock().unwrap(), Some(previous));
    assert!(h.waiver_cache.saved.lock().unwrap().is_none());
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
