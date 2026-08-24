//! Unit tests for [`super::CheckTestObligationsInteractor`] (T017).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use std::collections::{BTreeMap, HashMap};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Context, Poll, Waker};

use domain::task_contract::{ContractedEntryRef, TaskContractDocument};
use domain::tddd::LayerId;
use domain::tddd::catalogue_v2::catalogue_impl_signals_ports::{
    CatalogueDocumentLoaderError, CatalogueDocumentLoaderPort,
};
use domain::tddd::catalogue_v2::roles::{ContractRole, DataRole, FunctionRole, ItemAction};
use domain::tddd::catalogue_v2::{
    CatalogueDocument, CrateName, DeletionRecord, InherentImplDeclV2, MethodDeclaration,
    MethodName, ModulePath, SelfReceiver, StructKind, StructShape, TraitEntry, TraitImplDeclV2,
    TypeEntry, TypeKindV2, TypeRef,
};
use domain::tddd::semantic_verify::{
    CatalogueEntryKey, CatalogueEntryRef, CatalogueSectionKey, ModelTier, SpecSectionKind,
};
use domain::tddd::test_obligation::binding::{
    NonEmptyTestLocations, TestBindingRecord, TestBindingsDocument, TestLocation,
};
use domain::tddd::test_obligation::drift::TestObligationDrift;
use domain::tddd::test_obligation::errors::{
    ArtifactCodecError, ObligationCheckError, TestObligationRulesLoadError, TestSourceScanError,
    VerifyCacheError,
};
use domain::tddd::test_obligation::hashes::{
    AnchorTextHash, BoundTestsSetHash, DeclarationHash, TestBodySpanHash,
    VerifierPromptFingerprint, WaivedReasonHash,
};
use domain::tddd::test_obligation::ids::{
    DiagnosticMessage, RoleName, TestFunctionName, TestModulePath, TestObligationAnchorId,
    TestObligationBrief, TestObligationEdgeId, TestObligationId, TestObligationItemIdentifier,
    WaivedReason,
};
use domain::tddd::test_obligation::obligations::{ObligationsDocument, TestObligation};
use domain::tddd::test_obligation::ports::{
    ObligationsArtifactPort, TestBindingsArtifactPort, TestObligationRulesLoaderPort,
    TestSourceScannerPort, WaiverCachePort,
};
use domain::tddd::test_obligation::projection::RoleObligationItemsProjector;
use domain::tddd::test_obligation::rules::{
    RoleObligationRules, TestObligationRule, TestObligationRulesDocument,
};
use domain::tddd::test_obligation::scope::UncitedSpecElementFinding;
use domain::tddd::test_obligation::verdict::{
    FulfillmentCacheLookupError, ObligationFulfillmentCacheDocument,
    ObligationFulfillmentCacheEntry, ObligationFulfillmentCacheEntryState,
    ObligationFulfillmentCacheKey, ObligationFulfillmentVerdict, WaiverCacheDocument,
    WaiverCacheEntry, WaiverCacheKey, WaiverVerdict,
};
use domain::tddd::test_obligation::vocab::{
    TargetEntryRoleKind, TestObligationKind, TestObligationPatternKind, TestObligationPerAxis,
};
use domain::{
    ContentHash, EvidenceCitation, SpecDocument, SpecDocumentLoadError, SpecElementId, SpecRef,
    SpecRequirement, SpecScope, TaskId, TaskStatusKind, TrackId,
};

use domain::SpecDocumentLoaderPort;

use crate::pre_review_gate::{
    ImplPlanReadError, ImplPlanReaderPort, TaskContractReadError, TaskContractReaderPort,
};

use super::{
    CheckTestObligationsApplicationService, CheckTestObligationsCommand,
    CheckTestObligationsInteractor, CheckTestObligationsOutcome,
};
use crate::semantic_verdict_core::driver::{
    SemanticEscalationDriverPort, SemanticEscalationFuture,
};
use crate::test_obligation::derive::derive_obligations_document;
use crate::test_obligation::evaluate::{
    EvaluateTestObligationsApplicationService, EvaluateTestObligationsCommand,
    EvaluateTestObligationsInteractor, TestObligationEvaluateConfig,
};
use crate::test_obligation::hasher::ContentHasherPort;
use crate::test_obligation::ports::ObligationFulfillmentCachePort;
use crate::test_obligation::results::TestObligationStatusLaneSummary;
use crate::test_obligation::{
    LoadedCatalogueDocument, TestObligationCatalogueCommandInput, declaration_with_obligation_item,
    find_declaration_text, obligation_declaration_text, obligation_declaration_text_from_loaded,
    sha256_content_hash,
};
use domain::tddd::test_obligation::pair::{ObligationFulfillmentPair, WaiverPair};

const BODY: &str = "assert!(money.is_positive());";

// ---------------------------------------------------------------------------
// Test doubles
// ---------------------------------------------------------------------------

struct StubRules {
    doc: TestObligationRulesDocument,
}
impl StubRules {
    fn valid() -> Self {
        Self { doc: rules_doc() }
    }
}
impl TestObligationRulesLoaderPort for StubRules {
    fn load(&self) -> Result<TestObligationRulesDocument, TestObligationRulesLoadError> {
        // IN-08: `check` loads-and-validates the rules doc; the drift / totality
        // / freshness lanes downstream are rule-content-independent, so a valid
        // doc is enough for the existing test fixtures.
        Ok(self.doc.clone())
    }
}

struct FailingRules;
impl TestObligationRulesLoaderPort for FailingRules {
    fn load(&self) -> Result<TestObligationRulesDocument, TestObligationRulesLoadError> {
        Err(TestObligationRulesLoadError::RoleNotCovered {
            role_name: RoleName::try_new("ValueObject".to_owned()).unwrap(),
        })
    }
}

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

struct MalformedObligationsArtifact;
impl ObligationsArtifactPort for MalformedObligationsArtifact {
    fn load(&self, _t: &TrackId) -> Result<Option<ObligationsDocument>, ArtifactCodecError> {
        Err(ArtifactCodecError::MalformedJson(
            DiagnosticMessage::try_new("malformed obligations artifact".to_owned()).unwrap(),
        ))
    }

    fn save(&self, _d: &ObligationsDocument) -> Result<(), DiagnosticMessage> {
        Ok(())
    }
}

struct MalformedBindingsArtifact;
impl TestBindingsArtifactPort for MalformedBindingsArtifact {
    fn load(&self, _t: &TrackId) -> Result<Option<TestBindingsDocument>, ArtifactCodecError> {
        Err(ArtifactCodecError::MalformedJson(
            DiagnosticMessage::try_new("malformed bindings artifact".to_owned()).unwrap(),
        ))
    }

    fn save(&self, _d: &TestBindingsDocument) -> Result<(), DiagnosticMessage> {
        Ok(())
    }
}

struct ReadOnlyBindings {
    document: TestBindingsDocument,
    save_calls: AtomicUsize,
}

impl ReadOnlyBindings {
    fn new(document: TestBindingsDocument) -> Self {
        Self { document, save_calls: AtomicUsize::new(0) }
    }

    fn save_calls(&self) -> usize {
        self.save_calls.load(Ordering::SeqCst)
    }
}

impl TestBindingsArtifactPort for ReadOnlyBindings {
    fn load(&self, _t: &TrackId) -> Result<Option<TestBindingsDocument>, ArtifactCodecError> {
        Ok(Some(self.document.clone()))
    }

    fn save(&self, _d: &TestBindingsDocument) -> Result<(), DiagnosticMessage> {
        self.save_calls.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

struct StubScanner;
impl TestSourceScannerPort for StubScanner {
    fn scan_test_body(&self, _l: &TestLocation) -> Result<Option<String>, TestSourceScanError> {
        Ok(Some(BODY.to_owned()))
    }
    fn hash_test_body(&self, _s: &str) -> TestBodySpanHash {
        TestBodySpanHash::new(ContentHash::from_bytes([0u8; 32]))
    }
}

struct StubFulfillmentCache(Option<ObligationFulfillmentCacheDocument>);

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
        track_id: &TrackId,
    ) -> Result<Option<ObligationFulfillmentCacheDocument>, VerifyCacheError> {
        Ok(Some(self.0.clone().unwrap_or_else(|| {
            ObligationFulfillmentCacheDocument::new(track_id.clone(), Vec::new())
        })))
    }
    fn save(&self, _d: &ObligationFulfillmentCacheDocument) -> Result<(), DiagnosticMessage> {
        Ok(())
    }
}

struct LegacyRowsFulfillmentCache;
impl ObligationFulfillmentCachePort for LegacyRowsFulfillmentCache {
    fn load(
        &self,
        _t: &TrackId,
    ) -> Result<Option<ObligationFulfillmentCacheDocument>, VerifyCacheError> {
        Ok(None)
    }

    fn save(&self, _d: &ObligationFulfillmentCacheDocument) -> Result<(), DiagnosticMessage> {
        Ok(())
    }
}

struct RecoverableFulfillmentCache {
    saved: Mutex<Option<ObligationFulfillmentCacheDocument>>,
}

impl RecoverableFulfillmentCache {
    fn stale() -> Self {
        Self {
            saved: Mutex::new(Some(fresh_fulfillment_cache_for_fingerprint(
                &obligation(),
                Some(VerifierPromptFingerprint::new(ContentHash::from_bytes([7u8; 32]))),
            ))),
        }
    }
}

impl ObligationFulfillmentCachePort for RecoverableFulfillmentCache {
    fn load(
        &self,
        _t: &TrackId,
    ) -> Result<Option<ObligationFulfillmentCacheDocument>, VerifyCacheError> {
        Ok(self.saved.lock().unwrap().clone())
    }

    fn save(&self, document: &ObligationFulfillmentCacheDocument) -> Result<(), DiagnosticMessage> {
        *self.saved.lock().unwrap() = Some(document.clone());
        Ok(())
    }
}

struct AlwaysFulfilledDriver;

impl
    SemanticEscalationDriverPort<
        ObligationFulfillmentPair,
        ObligationFulfillmentCacheKey,
        ObligationFulfillmentVerdict,
        domain::tddd::test_obligation::errors::SemanticVerifierError,
    > for AlwaysFulfilledDriver
{
    fn evaluate_with_escalation<'a>(
        &'a self,
        _pair: &'a ObligationFulfillmentPair,
        _key: &'a ObligationFulfillmentCacheKey,
        _initial_tier: ModelTier,
    ) -> SemanticEscalationFuture<
        'a,
        ObligationFulfillmentVerdict,
        domain::tddd::test_obligation::errors::SemanticVerifierError,
    > {
        Box::pin(async {
            Ok(ObligationFulfillmentVerdict::Fulfilled {
                citation: EvidenceCitation::try_new("recovered by evaluator".to_owned()).unwrap(),
            })
        })
    }
}

struct AlwaysWaivedDriver;

impl
    SemanticEscalationDriverPort<
        WaiverPair,
        WaiverCacheKey,
        WaiverVerdict,
        domain::tddd::test_obligation::errors::SemanticVerifierError,
    > for AlwaysWaivedDriver
{
    fn evaluate_with_escalation<'a>(
        &'a self,
        _pair: &'a WaiverPair,
        _key: &'a WaiverCacheKey,
        _initial_tier: ModelTier,
    ) -> SemanticEscalationFuture<
        'a,
        WaiverVerdict,
        domain::tddd::test_obligation::errors::SemanticVerifierError,
    > {
        Box::pin(async {
            Ok(WaiverVerdict::Waived {
                citation: EvidenceCitation::try_new("not exercised".to_owned()).unwrap(),
            })
        })
    }
}

struct Sha256Hasher;

impl ContentHasherPort for Sha256Hasher {
    fn sha256(&self, bytes: &[u8]) -> ContentHash {
        sha256_content_hash(bytes)
    }
}

struct StubWaiverCache(Option<WaiverCacheDocument>);
impl WaiverCachePort for StubWaiverCache {
    fn load(&self, _t: &TrackId) -> Result<Option<WaiverCacheDocument>, VerifyCacheError> {
        Ok(self.0.clone())
    }
    fn save(&self, _d: &WaiverCacheDocument) -> Result<(), DiagnosticMessage> {
        Ok(())
    }
}

struct StubSpec(SpecDocument);
impl SpecDocumentLoaderPort for StubSpec {
    fn load(&self, _p: &Path) -> Result<SpecDocument, SpecDocumentLoadError> {
        Ok(self.0.clone())
    }
}

struct FailingSpec;
impl SpecDocumentLoaderPort for FailingSpec {
    fn load(&self, path: &Path) -> Result<SpecDocument, SpecDocumentLoadError> {
        Err(SpecDocumentLoadError::NotFound { path: path.to_path_buf() })
    }
}

struct StubCatalogue(CatalogueDocument);
impl CatalogueDocumentLoaderPort for StubCatalogue {
    fn load(&self, _p: &Path) -> Result<CatalogueDocument, CatalogueDocumentLoaderError> {
        Ok(self.0.clone())
    }
}

struct StubCatalogues(Vec<(PathBuf, CatalogueDocument)>);
impl CatalogueDocumentLoaderPort for StubCatalogues {
    fn load(&self, path: &Path) -> Result<CatalogueDocument, CatalogueDocumentLoaderError> {
        self.0
            .iter()
            .find(|(known_path, _)| known_path == path)
            .map(|(_, document)| document.clone())
            .ok_or_else(|| CatalogueDocumentLoaderError::NotFound { path: path.to_path_buf() })
    }
}

struct FailingCatalogue;
impl CatalogueDocumentLoaderPort for FailingCatalogue {
    fn load(&self, path: &Path) -> Result<CatalogueDocument, CatalogueDocumentLoaderError> {
        Err(CatalogueDocumentLoaderError::NotFound { path: path.to_path_buf() })
    }
}

struct StubTaskContractReader(TaskContractDocument);
impl TaskContractReaderPort for StubTaskContractReader {
    fn read(&self, _track_id: &TrackId) -> Result<TaskContractDocument, TaskContractReadError> {
        Ok(self.0.clone())
    }
}

struct StubImplPlanReader(HashMap<TaskId, TaskStatusKind>);
impl ImplPlanReaderPort for StubImplPlanReader {
    fn read_task_statuses(
        &self,
        _track_id: &TrackId,
    ) -> Result<HashMap<TaskId, TaskStatusKind>, ImplPlanReadError> {
        Ok(self.0.clone())
    }
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// Canonical `DataRole` variant names the rules document must cover.
///
/// Kept in-sync with `EXPECTED_DATA_ROLE_NAMES` in
/// `domain::tddd::test_obligation::rules`. A totality gap here surfaces as a
/// `RoleNotCovered` load error, so the fixture stays load-valid without
/// implying any obligation-derivation content (the check gate's rules-load
/// contract is load-and-validate only — IN-08).
const DATA_ROLE_NAMES: &[&str] = &[
    "ValueObject",
    "Entity",
    "AggregateRoot",
    "DomainService",
    "Specification",
    "Factory",
    "UseCase",
    "Interactor",
    "Command",
    "Query",
    "Dto",
    "ErrorType",
    "SecondaryAdapter",
    "EventPolicy",
    "DomainEvent",
    "CompositionRoot",
    "PrimaryAdapter",
];

/// Canonical `ContractRole` variant names the rules document must cover.
const CONTRACT_ROLE_NAMES: &[&str] =
    &["SpecificationPort", "ApplicationService", "SecondaryPort", "Repository"];

/// Minimal valid rules document with one obligation for the fixture catalogue.
///
/// Keeping the fixture artifact derived from the same shared projection used by
/// `check` makes the non-staleness tests exercise their intended gate lanes.
fn rules_doc() -> TestObligationRulesDocument {
    rules_doc_with_value_object_rules(vec![TestObligationRule::new(
        TestObligationKind::Boundary,
        TestObligationPerAxis::Entry,
        None,
        None,
    )])
}

fn rules_doc_with_value_object_rules(
    value_object_rules: Vec<TestObligationRule>,
) -> TestObligationRulesDocument {
    let empty = || RoleObligationRules::new(vec![]);
    let data_roles: Vec<(DataRole, RoleObligationRules)> = DATA_ROLE_NAMES
        .iter()
        .map(|name| {
            let rules = if *name == "ValueObject" {
                RoleObligationRules::new(value_object_rules.clone())
            } else {
                empty()
            };
            (name.parse::<DataRole>().unwrap(), rules)
        })
        .collect();
    let contract_roles: Vec<(ContractRole, RoleObligationRules)> =
        CONTRACT_ROLE_NAMES.iter().map(|n| (n.parse::<ContractRole>().unwrap(), empty())).collect();
    let function_roles =
        vec![(FunctionRole::FreeFunction, empty()), (FunctionRole::UseCaseFunction, empty())];
    let patterns = vec![(TestObligationPatternKind::Typestate, empty())];
    let trait_impls: Vec<(ContractRole, RoleObligationRules)> =
        CONTRACT_ROLE_NAMES.iter().map(|n| (n.parse::<ContractRole>().unwrap(), empty())).collect();
    TestObligationRulesDocument::try_new(
        data_roles,
        contract_roles,
        function_roles,
        patterns,
        trait_impls,
    )
    .unwrap()
}

/// A valid rules document that derives no obligations from the fixture catalogue.
fn empty_rules_doc() -> TestObligationRulesDocument {
    let empty = || RoleObligationRules::new(vec![]);
    let data_roles: Vec<(DataRole, RoleObligationRules)> =
        DATA_ROLE_NAMES.iter().map(|name| (name.parse::<DataRole>().unwrap(), empty())).collect();
    let contract_roles: Vec<(ContractRole, RoleObligationRules)> = CONTRACT_ROLE_NAMES
        .iter()
        .map(|name| (name.parse::<ContractRole>().unwrap(), empty()))
        .collect();
    let function_roles =
        vec![(FunctionRole::FreeFunction, empty()), (FunctionRole::UseCaseFunction, empty())];
    let patterns = vec![(TestObligationPatternKind::Typestate, empty())];
    let trait_impls: Vec<(ContractRole, RoleObligationRules)> = CONTRACT_ROLE_NAMES
        .iter()
        .map(|name| (name.parse::<ContractRole>().unwrap(), empty()))
        .collect();
    TestObligationRulesDocument::try_new(
        data_roles,
        contract_roles,
        function_roles,
        patterns,
        trait_impls,
    )
    .unwrap()
}

fn secondary_port_trait_method_rules_doc() -> TestObligationRulesDocument {
    let empty = || RoleObligationRules::new(vec![]);
    let data_roles: Vec<(DataRole, RoleObligationRules)> =
        DATA_ROLE_NAMES.iter().map(|name| (name.parse::<DataRole>().unwrap(), empty())).collect();
    let contract_roles: Vec<(ContractRole, RoleObligationRules)> = CONTRACT_ROLE_NAMES
        .iter()
        .map(|name| {
            let rules = if *name == "SecondaryPort" {
                RoleObligationRules::new(vec![TestObligationRule::new(
                    TestObligationKind::Contract,
                    TestObligationPerAxis::TraitMethod,
                    None,
                    None,
                )])
            } else {
                empty()
            };
            (name.parse::<ContractRole>().unwrap(), rules)
        })
        .collect();
    let function_roles =
        vec![(FunctionRole::FreeFunction, empty()), (FunctionRole::UseCaseFunction, empty())];
    let patterns = vec![(TestObligationPatternKind::Typestate, empty())];
    let trait_impls: Vec<(ContractRole, RoleObligationRules)> = CONTRACT_ROLE_NAMES
        .iter()
        .map(|name| (name.parse::<ContractRole>().unwrap(), empty()))
        .collect();
    TestObligationRulesDocument::try_new(
        data_roles,
        contract_roles,
        function_roles,
        patterns,
        trait_impls,
    )
    .unwrap()
}

fn track() -> TrackId {
    TrackId::try_new("my-track").unwrap()
}

fn task_contract_reader() -> Arc<dyn TaskContractReaderPort> {
    let task_id = TaskId::try_new("T001".to_owned()).unwrap();
    let entry = ContractedEntryRef::new(LayerId::try_new("domain").unwrap(), entry_key());
    let mut entries = BTreeMap::new();
    entries.insert(task_id, vec![entry]);
    Arc::new(StubTaskContractReader(TaskContractDocument::new(track(), entries).unwrap()))
}

fn impl_plan_reader() -> Arc<dyn ImplPlanReaderPort> {
    impl_plan_reader_with(&[("T001", TaskStatusKind::Done)])
}

fn impl_plan_reader_with(statuses: &[(&str, TaskStatusKind)]) -> Arc<dyn ImplPlanReaderPort> {
    let mut task_statuses = HashMap::new();
    for (task_id, status) in statuses {
        task_statuses.insert(TaskId::try_new((*task_id).to_owned()).unwrap(), *status);
    }
    Arc::new(StubImplPlanReader(task_statuses))
}

fn task_contract_reader_with(task_ids: &[&str]) -> Arc<dyn TaskContractReaderPort> {
    let entry = ContractedEntryRef::new(LayerId::try_new("domain").unwrap(), entry_key());
    let mut entries = BTreeMap::new();
    for task_id in task_ids {
        entries.insert(TaskId::try_new((*task_id).to_owned()).unwrap(), vec![entry.clone()]);
    }
    Arc::new(StubTaskContractReader(TaskContractDocument::new(track(), entries).unwrap()))
}

fn test_port_task_contract_reader() -> Arc<dyn TaskContractReaderPort> {
    let task_id = TaskId::try_new("T001".to_owned()).unwrap();
    let entry = ContractedEntryRef::new(
        LayerId::try_new("domain").unwrap(),
        CatalogueEntryKey::try_new("TestPort".to_owned()).unwrap(),
    );
    let mut entries = BTreeMap::new();
    entries.insert(task_id, vec![entry]);
    Arc::new(StubTaskContractReader(TaskContractDocument::new(track(), entries).unwrap()))
}

fn anchorless_port_interactor(
    obligations: ObligationsDocument,
    bindings: TestBindingsDocument,
    rules: TestObligationRulesDocument,
    catalogue: CatalogueDocument,
) -> CheckTestObligationsInteractor {
    CheckTestObligationsInteractor::new(
        Arc::new(StubRules { doc: rules }),
        Arc::new(StubObligations(Some(obligations))),
        Arc::new(StubBindings(Some(bindings))),
        Arc::new(StubScanner),
        Arc::new(StubFulfillmentCache(Some(ObligationFulfillmentCacheDocument::new(
            track(),
            Vec::new(),
        )))),
        Arc::new(StubWaiverCache(None)),
        fulfillment_verifier_fingerprint(),
        waiver_verifier_fingerprint(),
        Arc::new(StubSpec(spec_doc())),
        Arc::new(StubCatalogue(catalogue)),
        test_port_task_contract_reader(),
        impl_plan_reader(),
    )
}

fn missing_binding_interactor_with_statuses(
    statuses: &[(&str, TaskStatusKind)],
) -> CheckTestObligationsInteractor {
    interactor_with_task_statuses(
        Some(ObligationsDocument::new(track(), vec![obligation()])),
        Some(TestBindingsDocument::new(track(), Vec::new())),
        None,
        None,
        statuses,
    )
}

fn interactor_with_task_statuses(
    obligations: Option<ObligationsDocument>,
    bindings: Option<TestBindingsDocument>,
    fulfillment: Option<ObligationFulfillmentCacheDocument>,
    waiver: Option<WaiverCacheDocument>,
    statuses: &[(&str, TaskStatusKind)],
) -> CheckTestObligationsInteractor {
    interactor_with_scanner_and_task_statuses(
        obligations,
        bindings,
        fulfillment,
        waiver,
        Arc::new(StubScanner),
        statuses,
    )
}

#[allow(clippy::too_many_arguments)]
fn interactor_with_scanner_and_task_statuses(
    obligations: Option<ObligationsDocument>,
    bindings: Option<TestBindingsDocument>,
    fulfillment: Option<ObligationFulfillmentCacheDocument>,
    waiver: Option<WaiverCacheDocument>,
    scanner: Arc<dyn TestSourceScannerPort + Send + Sync>,
    statuses: &[(&str, TaskStatusKind)],
) -> CheckTestObligationsInteractor {
    CheckTestObligationsInteractor::new(
        Arc::new(StubRules::valid()),
        Arc::new(StubObligations(obligations)),
        Arc::new(StubBindings(bindings)),
        scanner,
        Arc::new(StubFulfillmentCache(fulfillment)),
        Arc::new(StubWaiverCache(waiver)),
        fulfillment_verifier_fingerprint(),
        waiver_verifier_fingerprint(),
        Arc::new(StubSpec(spec_doc())),
        Arc::new(StubCatalogue(money_catalogue())),
        task_contract_reader_with(
            &statuses.iter().map(|(task_id, _)| *task_id).collect::<Vec<_>>(),
        ),
        impl_plan_reader_with(statuses),
    )
}

fn interactor_with_legacy_fulfillment_cache(
    obligations: Option<ObligationsDocument>,
    bindings: Option<TestBindingsDocument>,
) -> CheckTestObligationsInteractor {
    CheckTestObligationsInteractor::new(
        Arc::new(StubRules::valid()),
        Arc::new(StubObligations(obligations)),
        Arc::new(StubBindings(bindings)),
        Arc::new(StubScanner),
        Arc::new(LegacyRowsFulfillmentCache),
        Arc::new(StubWaiverCache(None)),
        fulfillment_verifier_fingerprint(),
        waiver_verifier_fingerprint(),
        Arc::new(StubSpec(spec_doc())),
        Arc::new(StubCatalogue(money_catalogue())),
        task_contract_reader(),
        impl_plan_reader(),
    )
}

fn interactor_with_recoverable_fulfillment_cache(
    cache: Arc<RecoverableFulfillmentCache>,
) -> CheckTestObligationsInteractor {
    CheckTestObligationsInteractor::new(
        Arc::new(StubRules::valid()),
        Arc::new(StubObligations(Some(ObligationsDocument::new(track(), vec![obligation()])))),
        Arc::new(StubBindings(Some(TestBindingsDocument::new(
            track(),
            vec![fulfillment_binding()],
        )))),
        Arc::new(StubScanner),
        cache as Arc<dyn ObligationFulfillmentCachePort + Send + Sync>,
        Arc::new(StubWaiverCache(None)),
        fulfillment_verifier_fingerprint(),
        waiver_verifier_fingerprint(),
        Arc::new(StubSpec(spec_doc())),
        Arc::new(StubCatalogue(money_catalogue())),
        task_contract_reader(),
        impl_plan_reader(),
    )
}

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

fn fulfillment_verifier_fingerprint() -> VerifierPromptFingerprint {
    VerifierPromptFingerprint::new(ContentHash::from_bytes([8u8; 32]))
}

fn waiver_verifier_fingerprint() -> VerifierPromptFingerprint {
    VerifierPromptFingerprint::new(ContentHash::from_bytes([9u8; 32]))
}

fn entry_key() -> CatalogueEntryKey {
    CatalogueEntryKey::try_new("Money".to_owned()).unwrap()
}

fn anchor() -> TestObligationAnchorId {
    TestObligationAnchorId::try_new("spec.json".to_owned(), "IN-05".to_owned()).unwrap()
}

fn edge() -> TestObligationEdgeId {
    TestObligationEdgeId::new(entry_key(), anchor())
}

fn unknown_edge() -> TestObligationEdgeId {
    TestObligationEdgeId::new(CatalogueEntryKey::try_new("Ghost".to_owned()).unwrap(), anchor())
}

fn obligation() -> TestObligation {
    derived_obligations(rules_doc(), money_catalogue()).obligations().first().cloned().unwrap()
}

fn derived_obligations(
    rules: TestObligationRulesDocument,
    catalogue: CatalogueDocument,
) -> ObligationsDocument {
    derive_obligations_document(
        track(),
        &rules,
        &[(PathBuf::from("domain-types.json"), catalogue)],
        &RoleObligationItemsProjector::new(),
    )
    .unwrap()
}

fn obligation_with_item(item: &str) -> TestObligation {
    TestObligation::new(
        TestObligationId::new(
            entry_key(),
            TestObligationKind::Boundary,
            TestObligationItemIdentifier::try_new(item.to_owned()).unwrap(),
        ),
        CatalogueEntryRef::new(
            "domain-types.json".to_owned(),
            CatalogueSectionKey::Types,
            entry_key(),
        ),
        TargetEntryRoleKind::DataRole(DataRole::value_object()),
        TestObligationBrief::try_new("cover positivity".to_owned()).unwrap(),
        DeclarationHash::new(ContentHash::from_bytes([9u8; 32])),
        vec![anchor()],
    )
}

fn trait_impl_obligation() -> TestObligation {
    derived_obligations(trait_impl_rules_doc(), trait_impl_catalogue())
        .obligations()
        .first()
        .cloned()
        .unwrap()
}

fn location() -> TestLocation {
    TestLocation::new(
        LayerId::try_new("domain").unwrap(),
        TestModulePath::try_new("domain::money::tests".to_owned()).unwrap(),
        TestFunctionName::try_new("test_positive".to_owned()).unwrap(),
    )
}

fn missing_location() -> TestLocation {
    TestLocation::new(
        LayerId::try_new("domain").unwrap(),
        TestModulePath::try_new("domain::money::tests".to_owned()).unwrap(),
        TestFunctionName::try_new("test_renamed".to_owned()).unwrap(),
    )
}

fn fulfillment_binding() -> TestBindingRecord {
    fulfillment_binding_for(&obligation())
}

fn fulfillment_binding_for(obligation: &TestObligation) -> TestBindingRecord {
    TestBindingRecord::Fulfillment {
        obligation_id: obligation.id().clone(),
        tests: NonEmptyTestLocations::try_new(vec![location()]).unwrap(),
    }
}

fn waiver_binding() -> TestBindingRecord {
    waiver_binding_for(edge())
}

fn waiver_binding_for(edge_id: TestObligationEdgeId) -> TestBindingRecord {
    TestBindingRecord::Waiver {
        edge_id,
        reason: WaivedReason::try_new("covered elsewhere".to_owned()).unwrap(),
    }
}

fn voluntary_binding() -> TestBindingRecord {
    TestBindingRecord::VoluntaryBinding {
        edge_id: edge(),
        tests: NonEmptyTestLocations::try_new(vec![location()]).unwrap(),
    }
}

fn money_catalogue() -> CatalogueDocument {
    let mut doc = CatalogueDocument::new(
        5,
        CrateName::new("domain").unwrap(),
        LayerId::try_new("domain").unwrap(),
    );
    doc.insert_type(
        CatalogueEntryKey::try_new("Money".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::Struct(StructKind::new(StructShape::Unit, None)),
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![SpecRef::new(
                PathBuf::from("spec.json"),
                SpecElementId::try_new("IN-05").unwrap(),
            )],
            vec![],
        ),
    );
    doc.push_deletion(DeletionRecord::Type {
        name: CatalogueEntryKey::try_new("OldMoney".to_owned()).unwrap(),
        spec_refs: vec![SpecRef::new(
            PathBuf::from("spec.json"),
            SpecElementId::try_new("AC-01").unwrap(),
        )],
        informal_grounds: vec![],
    });
    doc.push_trait_impl(TraitImplDeclV2::new(
        TypeRef::new("MyPort").unwrap(),
        TypeRef::new("Money").unwrap(),
    ));
    doc
}

fn qualified_money_catalogue() -> CatalogueDocument {
    let mut doc = CatalogueDocument::new(
        5,
        CrateName::new("domain").unwrap(),
        LayerId::try_new("domain").unwrap(),
    );
    doc.insert_type(
        CatalogueEntryKey::try_new("domain::alpha::Money".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::Struct(StructKind::new(StructShape::Unit, None)),
            vec![],
            vec![],
            vec![],
            ModulePath::from_segments(vec!["alpha".to_owned()]).unwrap(),
            None,
            vec![SpecRef::new(
                PathBuf::from("spec.json"),
                SpecElementId::try_new("IN-05").unwrap(),
            )],
            vec![],
        ),
    );
    doc
}

fn money_catalogue_in_layer(
    crate_name: &str,
    layer: &str,
    action: ItemAction,
) -> CatalogueDocument {
    let mut doc = CatalogueDocument::new(
        5,
        CrateName::new(crate_name).unwrap(),
        LayerId::try_new(layer).unwrap(),
    );
    doc.insert_type(
        CatalogueEntryKey::try_new("Money".to_owned()).unwrap(),
        TypeEntry::new(
            action,
            DataRole::value_object(),
            TypeKindV2::Struct(StructKind::new(StructShape::Unit, None)),
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![SpecRef::new(
                PathBuf::from("spec.json"),
                SpecElementId::try_new("IN-05").unwrap(),
            )],
            vec![],
        ),
    );
    doc
}

fn money_catalogue_with_role(role: DataRole) -> CatalogueDocument {
    let mut doc = CatalogueDocument::new(
        5,
        CrateName::new("domain").unwrap(),
        LayerId::try_new("domain").unwrap(),
    );
    doc.insert_type(
        CatalogueEntryKey::try_new("Money".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            role,
            TypeKindV2::Struct(StructKind::new(StructShape::Unit, None)),
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![SpecRef::new(
                PathBuf::from("spec.json"),
                SpecElementId::try_new("IN-05").unwrap(),
            )],
            vec![],
        ),
    );
    doc
}

fn covering_method(name: &str, anchor: &str) -> MethodDeclaration {
    MethodDeclaration::new(
        MethodName::new(name).unwrap(),
        None,
        vec![],
        TypeRef::new("()").unwrap(),
        false,
        false,
        vec![],
        vec![],
        vec![SpecRef::new(PathBuf::from("spec.json"), SpecElementId::try_new(anchor).unwrap())],
        ItemAction::Add,
        None,
    )
}

fn method_without_spec_refs(name: &str) -> MethodDeclaration {
    MethodDeclaration::new(
        MethodName::new(name).unwrap(),
        None,
        vec![],
        TypeRef::new("()").unwrap(),
        false,
        false,
        vec![],
        vec![],
        vec![],
        ItemAction::Add,
        None,
    )
}

fn incomplete_coverage_catalogue(action: ItemAction) -> CatalogueDocument {
    let mut catalogue = CatalogueDocument::new(
        5,
        CrateName::new("domain").unwrap(),
        LayerId::try_new("domain").unwrap(),
    );
    catalogue.insert_trait(
        CatalogueEntryKey::try_new("TestPort".to_owned()).unwrap(),
        TraitEntry::new(
            action,
            ContractRole::SecondaryPort,
            vec![method_without_spec_refs("load")],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![SpecRef::new(
                PathBuf::from("spec.json"),
                SpecElementId::try_new("IN-05").unwrap(),
            )],
            vec![],
        ),
    );
    catalogue
}

fn anchorless_secondary_port_catalogue() -> CatalogueDocument {
    let mut catalogue = CatalogueDocument::new(
        5,
        CrateName::new("domain").unwrap(),
        LayerId::try_new("domain").unwrap(),
    );
    catalogue.insert_trait(
        CatalogueEntryKey::try_new("TestPort".to_owned()).unwrap(),
        TraitEntry::new(
            ItemAction::Add,
            ContractRole::SecondaryPort,
            vec![method_without_spec_refs("load")],
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
    catalogue
}

fn trait_impl_catalogue() -> CatalogueDocument {
    let mut catalogue = money_catalogue_with_role(DataRole::value_object());
    let entry_refs =
        vec![SpecRef::new(PathBuf::from("spec.json"), SpecElementId::try_new("IN-05").unwrap())];
    catalogue.insert_trait(
        CatalogueEntryKey::try_new("MyPort".to_owned()).unwrap(),
        TraitEntry::new(
            ItemAction::Reference,
            ContractRole::SecondaryPort,
            vec![covering_method("load", "IN-05")],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            entry_refs,
            vec![],
        ),
    );
    catalogue.push_trait_impl(TraitImplDeclV2::new(
        TypeRef::new("MyPort").unwrap(),
        TypeRef::new("Money").unwrap(),
    ));
    catalogue
}

fn trait_impl_rules_doc() -> TestObligationRulesDocument {
    let empty = || RoleObligationRules::new(vec![]);
    let data_roles: Vec<(DataRole, RoleObligationRules)> =
        DATA_ROLE_NAMES.iter().map(|name| (name.parse::<DataRole>().unwrap(), empty())).collect();
    let contract_roles: Vec<(ContractRole, RoleObligationRules)> = CONTRACT_ROLE_NAMES
        .iter()
        .map(|name| (name.parse::<ContractRole>().unwrap(), empty()))
        .collect();
    let function_roles =
        vec![(FunctionRole::FreeFunction, empty()), (FunctionRole::UseCaseFunction, empty())];
    let patterns = vec![(TestObligationPatternKind::Typestate, empty())];
    let trait_impls: Vec<(ContractRole, RoleObligationRules)> = CONTRACT_ROLE_NAMES
        .iter()
        .map(|name| {
            let rules = if *name == "SecondaryPort" {
                RoleObligationRules::new(vec![TestObligationRule::new(
                    TestObligationKind::ContractConformance,
                    TestObligationPerAxis::TraitImpl,
                    None,
                    None,
                )])
            } else {
                empty()
            };
            (name.parse::<ContractRole>().unwrap(), rules)
        })
        .collect();
    TestObligationRulesDocument::try_new(
        data_roles,
        contract_roles,
        function_roles,
        patterns,
        trait_impls,
    )
    .unwrap()
}

fn shared_edge_waiver_rules_doc() -> TestObligationRulesDocument {
    let empty = || RoleObligationRules::new(vec![]);
    let data_roles: Vec<(DataRole, RoleObligationRules)> = DATA_ROLE_NAMES
        .iter()
        .map(|name| {
            let rules = if *name == "ValueObject" {
                RoleObligationRules::new(vec![TestObligationRule::new(
                    TestObligationKind::Boundary,
                    TestObligationPerAxis::Entry,
                    None,
                    None,
                )])
            } else {
                empty()
            };
            (name.parse::<DataRole>().unwrap(), rules)
        })
        .collect();
    let contract_roles: Vec<(ContractRole, RoleObligationRules)> = CONTRACT_ROLE_NAMES
        .iter()
        .map(|name| (name.parse::<ContractRole>().unwrap(), empty()))
        .collect();
    let function_roles =
        vec![(FunctionRole::FreeFunction, empty()), (FunctionRole::UseCaseFunction, empty())];
    let patterns = vec![(TestObligationPatternKind::Typestate, empty())];
    let trait_impls: Vec<(ContractRole, RoleObligationRules)> = CONTRACT_ROLE_NAMES
        .iter()
        .map(|name| {
            let rules = if *name == "SecondaryPort" {
                RoleObligationRules::new(vec![TestObligationRule::new(
                    TestObligationKind::ContractConformance,
                    TestObligationPerAxis::TraitImpl,
                    None,
                    None,
                )])
            } else {
                empty()
            };
            (name.parse::<ContractRole>().unwrap(), rules)
        })
        .collect();
    TestObligationRulesDocument::try_new(
        data_roles,
        contract_roles,
        function_roles,
        patterns,
        trait_impls,
    )
    .unwrap()
}

fn trait_entry(role: ContractRole) -> TraitEntry {
    let entry_refs =
        vec![SpecRef::new(PathBuf::from("spec.json"), SpecElementId::try_new("IN-05").unwrap())];
    TraitEntry::new(
        ItemAction::Add,
        role,
        vec![covering_method("verify", "IN-05")],
        vec![],
        vec![],
        vec![],
        vec![],
        vec![],
        ModulePath::root(),
        None,
        entry_refs,
        vec![],
    )
}

/// A parsed spec with `IN-05` (cited), plus an uncited `AC-01` and `CN-01`.
fn spec_doc() -> SpecDocument {
    let in_scope = vec![
        SpecRequirement::new(
            SpecElementId::try_new("IN-05").unwrap(),
            "Money positive",
            vec![],
            vec![],
            vec![],
        )
        .unwrap(),
    ];
    let constraints = vec![
        SpecRequirement::new(SpecElementId::try_new("CN-01").unwrap(), "c", vec![], vec![], vec![])
            .unwrap(),
    ];
    let acceptance = vec![
        SpecRequirement::new(SpecElementId::try_new("AC-01").unwrap(), "a", vec![], vec![], vec![])
            .unwrap(),
    ];
    SpecDocument::new(
        "Test spec",
        "1.0",
        vec![],
        SpecScope::new(in_scope, vec![]),
        constraints,
        acceptance,
        vec![],
        vec![],
        None,
    )
    .unwrap()
}

fn fresh_fulfillment_cache() -> ObligationFulfillmentCacheDocument {
    fresh_fulfillment_cache_for(&obligation())
}

fn fresh_fulfillment_cache_for(obligation: &TestObligation) -> ObligationFulfillmentCacheDocument {
    fresh_fulfillment_cache_for_catalogue(
        obligation,
        &money_catalogue(),
        Some(fulfillment_verifier_fingerprint()),
    )
}

fn fresh_voluntary_fulfillment_cache() -> ObligationFulfillmentCacheDocument {
    let catalogue = money_catalogue();
    let bound = BoundTestsSetHash::new(sha256_content_hash(format!("{BODY}\n").as_bytes()));
    let synthetic = TestObligationId::new(
        entry_key(),
        TestObligationKind::Logic,
        TestObligationItemIdentifier::try_new("voluntary:IN-05".to_owned()).unwrap(),
    );
    let declaration = DeclarationHash::new(sha256_content_hash(
        declaration_with_obligation_item(
            &obligation_declaration_text(std::slice::from_ref(&catalogue), &obligation()).unwrap(),
            synthetic.item_identifier().as_str(),
        )
        .as_bytes(),
    ));
    let anchor_hash = AnchorTextHash::new(sha256_content_hash(b"Money positive"));
    let entry = cache_entry(
        edge(),
        TestObligationId::new(
            entry_key(),
            TestObligationKind::Logic,
            TestObligationItemIdentifier::try_new("voluntary:IN-05".to_owned()).unwrap(),
        ),
        ObligationFulfillmentCacheKey::new(bound, declaration, anchor_hash),
        ObligationFulfillmentVerdict::Fulfilled {
            citation: EvidenceCitation::try_new("asserts positivity".to_owned()).unwrap(),
        },
        Some(fulfillment_verifier_fingerprint()),
    );
    ObligationFulfillmentCacheDocument::new(track(), vec![entry])
}

fn fresh_fulfillment_cache_for_fingerprint(
    obligation: &TestObligation,
    verifier_fingerprint: Option<VerifierPromptFingerprint>,
) -> ObligationFulfillmentCacheDocument {
    fresh_fulfillment_cache_for_catalogue(obligation, &money_catalogue(), verifier_fingerprint)
}

fn fresh_fulfillment_cache_for_catalogue(
    obligation: &TestObligation,
    catalogue: &CatalogueDocument,
    verifier_fingerprint: Option<VerifierPromptFingerprint>,
) -> ObligationFulfillmentCacheDocument {
    let bound = BoundTestsSetHash::new(sha256_content_hash(format!("{BODY}\n").as_bytes()));
    let decl = DeclarationHash::new(sha256_content_hash(
        declaration_with_obligation_item(
            &obligation_declaration_text(std::slice::from_ref(catalogue), obligation).unwrap(),
            obligation.id().item_identifier().as_str(),
        )
        .as_bytes(),
    ));
    let anchor_hash = AnchorTextHash::new(sha256_content_hash(b"Money positive"));
    let entry = cache_entry(
        edge(),
        obligation.id().clone(),
        ObligationFulfillmentCacheKey::new(bound, decl, anchor_hash),
        ObligationFulfillmentVerdict::Fulfilled {
            citation: EvidenceCitation::try_new("asserts positivity".to_owned()).unwrap(),
        },
        verifier_fingerprint,
    );
    ObligationFulfillmentCacheDocument::new(track(), vec![entry])
}

/// Waiver cache for a zero-obligation (edge-direct) waiver: evaluate stores the
/// synthetic voluntary obligation id for ownerless edges, so the frozen entry must
/// carry that id for the (edge × obligation) resolution to match.
fn fresh_direct_waiver_cache() -> WaiverCacheDocument {
    let reason = WaivedReason::try_new("covered elsewhere".to_owned()).unwrap();
    let reason_hash = WaivedReasonHash::new(sha256_content_hash(reason.as_str().as_bytes()));
    let synthetic = TestObligationId::new(
        edge().entry_key().clone(),
        TestObligationKind::Logic,
        TestObligationItemIdentifier::try_new(format!(
            "voluntary:{}",
            edge().anchor_id().element_id()
        ))
        .unwrap(),
    );
    let declaration = declaration_with_obligation_item(
        &find_declaration_text(std::slice::from_ref(&money_catalogue()), "Money").unwrap(),
        synthetic.item_identifier().as_str(),
    );
    let decl = DeclarationHash::new(sha256_content_hash(declaration.as_bytes()));
    let anchor_hash = AnchorTextHash::new(sha256_content_hash(b"Money positive"));
    let entry = WaiverCacheEntry::new(
        edge(),
        Some(synthetic),
        WaiverCacheKey::new(reason_hash, decl, anchor_hash),
        WaiverVerdict::Waived {
            citation: EvidenceCitation::try_new("waived by policy".to_owned()).unwrap(),
        },
        Some(waiver_verifier_fingerprint()),
    );
    WaiverCacheDocument::new(track(), vec![entry])
}

fn fresh_waiver_cache_for_fingerprint(
    obligation: &TestObligation,
    verifier_fingerprint: Option<VerifierPromptFingerprint>,
) -> WaiverCacheDocument {
    fresh_waiver_cache_for_catalogue(obligation, &money_catalogue(), verifier_fingerprint)
}

fn fresh_waiver_cache_for_catalogue(
    obligation: &TestObligation,
    catalogue: &CatalogueDocument,
    verifier_fingerprint: Option<VerifierPromptFingerprint>,
) -> WaiverCacheDocument {
    let reason = WaivedReason::try_new("covered elsewhere".to_owned()).unwrap();
    let reason_hash = WaivedReasonHash::new(sha256_content_hash(reason.as_str().as_bytes()));
    let declaration = declaration_with_obligation_item(
        &obligation_declaration_text(std::slice::from_ref(catalogue), obligation).unwrap(),
        obligation.id().item_identifier().as_str(),
    );
    let decl = DeclarationHash::new(sha256_content_hash(declaration.as_bytes()));
    let anchor_hash = AnchorTextHash::new(sha256_content_hash(b"Money positive"));
    let entry = WaiverCacheEntry::new(
        edge(),
        Some(obligation.id().clone()),
        WaiverCacheKey::new(reason_hash, decl, anchor_hash),
        WaiverVerdict::Waived {
            citation: EvidenceCitation::try_new("waived by policy".to_owned()).unwrap(),
        },
        verifier_fingerprint,
    );
    WaiverCacheDocument::new(track(), vec![entry])
}

#[allow(clippy::too_many_arguments)]
fn interactor(
    obligations: Option<ObligationsDocument>,
    bindings: Option<TestBindingsDocument>,
    fulfillment: Option<ObligationFulfillmentCacheDocument>,
    waiver: Option<WaiverCacheDocument>,
) -> CheckTestObligationsInteractor {
    interactor_with_scanner(obligations, bindings, fulfillment, waiver, Arc::new(StubScanner))
}

#[allow(clippy::too_many_arguments)]
fn interactor_with_scanner(
    obligations: Option<ObligationsDocument>,
    bindings: Option<TestBindingsDocument>,
    fulfillment: Option<ObligationFulfillmentCacheDocument>,
    waiver: Option<WaiverCacheDocument>,
    scanner: Arc<dyn TestSourceScannerPort + Send + Sync>,
) -> CheckTestObligationsInteractor {
    interactor_with_rules_and_catalogue(
        obligations,
        bindings,
        fulfillment,
        waiver,
        rules_doc(),
        money_catalogue(),
        scanner,
    )
}

#[allow(clippy::too_many_arguments)]
fn interactor_with_rules_and_catalogue(
    obligations: Option<ObligationsDocument>,
    bindings: Option<TestBindingsDocument>,
    fulfillment: Option<ObligationFulfillmentCacheDocument>,
    waiver: Option<WaiverCacheDocument>,
    rules: TestObligationRulesDocument,
    catalogue: CatalogueDocument,
    scanner: Arc<dyn TestSourceScannerPort + Send + Sync>,
) -> CheckTestObligationsInteractor {
    CheckTestObligationsInteractor::new(
        Arc::new(StubRules { doc: rules }),
        Arc::new(StubObligations(obligations)),
        Arc::new(StubBindings(bindings)),
        scanner,
        Arc::new(StubFulfillmentCache(fulfillment)),
        Arc::new(StubWaiverCache(waiver)),
        fulfillment_verifier_fingerprint(),
        waiver_verifier_fingerprint(),
        Arc::new(StubSpec(spec_doc())),
        Arc::new(StubCatalogue(catalogue)),
        task_contract_reader(),
        impl_plan_reader(),
    )
}

fn interactor_with_rules(
    obligations: Option<ObligationsDocument>,
    bindings: Option<TestBindingsDocument>,
    fulfillment: Option<ObligationFulfillmentCacheDocument>,
    waiver: Option<WaiverCacheDocument>,
    rules: TestObligationRulesDocument,
) -> CheckTestObligationsInteractor {
    interactor_with_rules_and_catalogue(
        obligations,
        bindings,
        fulfillment,
        waiver,
        rules,
        money_catalogue(),
        Arc::new(StubScanner),
    )
}

fn interactor_with_rules_loader(
    rules_loader: Arc<dyn TestObligationRulesLoaderPort + Send + Sync>,
) -> CheckTestObligationsInteractor {
    CheckTestObligationsInteractor::new(
        rules_loader,
        Arc::new(StubObligations(None)),
        Arc::new(StubBindings(None)),
        Arc::new(StubScanner),
        Arc::new(StubFulfillmentCache(None)),
        Arc::new(StubWaiverCache(None)),
        fulfillment_verifier_fingerprint(),
        waiver_verifier_fingerprint(),
        Arc::new(StubSpec(spec_doc())),
        Arc::new(StubCatalogue(money_catalogue())),
        task_contract_reader(),
        impl_plan_reader(),
    )
}

fn interactor_with_rules_loader_and_status(
    rules_loader: Arc<dyn TestObligationRulesLoaderPort + Send + Sync>,
    status: TaskStatusKind,
) -> CheckTestObligationsInteractor {
    CheckTestObligationsInteractor::new(
        rules_loader,
        Arc::new(StubObligations(None)),
        Arc::new(StubBindings(None)),
        Arc::new(StubScanner),
        Arc::new(StubFulfillmentCache(None)),
        Arc::new(StubWaiverCache(None)),
        fulfillment_verifier_fingerprint(),
        waiver_verifier_fingerprint(),
        Arc::new(StubSpec(spec_doc())),
        Arc::new(StubCatalogue(money_catalogue())),
        task_contract_reader(),
        impl_plan_reader_with(&[("T001", status)]),
    )
}

fn unattributable_interactor_with_status(status: TaskStatusKind) -> CheckTestObligationsInteractor {
    let mut entries = BTreeMap::new();
    entries.insert(
        TaskId::try_new("T001".to_owned()).unwrap(),
        vec![ContractedEntryRef::new(
            LayerId::try_new("domain".to_owned()).unwrap(),
            CatalogueEntryKey::try_new("OtherEntry".to_owned()).unwrap(),
        )],
    );
    let mut statuses = HashMap::new();
    statuses.insert(TaskId::try_new("T001".to_owned()).unwrap(), status);
    CheckTestObligationsInteractor::new(
        Arc::new(StubRules::valid()),
        Arc::new(StubObligations(Some(ObligationsDocument::new(track(), vec![obligation()])))),
        Arc::new(StubBindings(Some(TestBindingsDocument::new(track(), Vec::new())))),
        Arc::new(StubScanner),
        Arc::new(StubFulfillmentCache(None)),
        Arc::new(StubWaiverCache(None)),
        fulfillment_verifier_fingerprint(),
        waiver_verifier_fingerprint(),
        Arc::new(StubSpec(spec_doc())),
        Arc::new(StubCatalogue(money_catalogue())),
        Arc::new(StubTaskContractReader(TaskContractDocument::new(track(), entries).unwrap())),
        Arc::new(StubImplPlanReader(statuses)),
    )
}

fn empty_scope_interactor_without_readers() -> CheckTestObligationsInteractor {
    CheckTestObligationsInteractor::new(
        Arc::new(StubRules::valid()),
        Arc::new(StubObligations(None)),
        Arc::new(StubBindings(None)),
        Arc::new(StubScanner),
        Arc::new(StubFulfillmentCache(None)),
        Arc::new(StubWaiverCache(None)),
        fulfillment_verifier_fingerprint(),
        waiver_verifier_fingerprint(),
        Arc::new(FailingSpec),
        Arc::new(FailingCatalogue),
        task_contract_reader(),
        impl_plan_reader(),
    )
}

fn command() -> CheckTestObligationsCommand {
    CheckTestObligationsCommand::new(TestObligationCatalogueCommandInput::new(
        track(),
        "track/my-track".to_owned(),
        vec![PathBuf::from("domain-types.json")],
    ))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn test_check_catalogue_absent_artifacts_absent_passes_empty_scope() {
    // AC-03: a catalogue-free track remains an empty scope without loading
    // the spec or materializing obligation artifacts.
    let outcome = empty_scope_interactor_without_readers().execute(&command()).unwrap();
    assert!(outcome.resolved_edges().is_empty());
    assert!(outcome.uncited_findings().is_empty());
}

#[test]
fn test_check_catalogue_present_artifacts_absent_fails_closed() {
    // AC-02: finding a catalogue prevents missing enrollment artifacts from
    // being treated as a zero-pair scope.
    let result = interactor(None, None, None, None).execute(&command());

    assert!(matches!(result, Err(ObligationCheckError::ObligationsAbsent)));
}

#[test]
fn test_check_catalogue_present_empty_artifacts_passes_verified_scope() {
    // AC-02 / CN-03: present artifacts with a zero derivation are distinct
    // from absent artifacts and therefore proceed through the check gate.
    let empty_catalogue = CatalogueDocument::new(
        5,
        CrateName::new("domain").unwrap(),
        LayerId::try_new("domain").unwrap(),
    );
    let outcome = interactor_with_rules_and_catalogue(
        Some(ObligationsDocument::new(track(), vec![])),
        Some(TestBindingsDocument::new(track(), vec![])),
        None,
        None,
        empty_rules_doc(),
        empty_catalogue,
        Arc::new(StubScanner),
    )
    .execute(&command())
    .unwrap();

    assert!(outcome.resolved_edges().is_empty());
}

#[test]
fn test_materialized_scope_reports_uncited_findings() {
    // AC-13 / IN-16: uncited AC / CN are surfaced in a materialized scope; cited
    // IN and goal are not. A deletion tombstone citation for AC-01 does not
    // count as an active citation.
    let obligations = ObligationsDocument::new(track(), vec![obligation()]);
    let bindings = TestBindingsDocument::new(track(), vec![fulfillment_binding()]);
    let outcome =
        interactor(Some(obligations), Some(bindings), Some(fresh_fulfillment_cache()), None)
            .execute(&command())
            .unwrap();
    assert!(outcome.uncited_findings().contains(&UncitedSpecElementFinding::new(
        SpecElementId::try_new("AC-01").unwrap(),
        SpecSectionKind::AcceptanceCriteria
    )));
    assert!(outcome.uncited_findings().contains(&UncitedSpecElementFinding::new(
        SpecElementId::try_new("CN-01").unwrap(),
        SpecSectionKind::Constraint
    )));
    // IN-05 is cited by the Money entry → not a finding.
    assert_eq!(outcome.uncited_findings().len(), 2);
}

#[test]
fn test_cited_anchor_ids_includes_method_only_refs() {
    let mut catalogue = CatalogueDocument::new(
        5,
        CrateName::new("domain").unwrap(),
        LayerId::try_new("domain").unwrap(),
    );
    catalogue.insert_trait(
        CatalogueEntryKey::try_new("TestPort".to_owned()).unwrap(),
        TraitEntry::new(
            ItemAction::Add,
            ContractRole::SecondaryPort,
            vec![covering_method("load", "AC-01")],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![SpecRef::new(
                PathBuf::from("spec.json"),
                SpecElementId::try_new("IN-05").unwrap(),
            )],
            vec![],
        ),
    );
    let cited = crate::test_obligation::cited_anchor_ids(std::slice::from_ref(&catalogue));
    assert!(cited.contains(&"IN-05".to_owned()));
    assert!(cited.contains(&"AC-01".to_owned()));
}

#[test]
fn test_cited_anchor_ids_skips_reference_method_refs() {
    let mut catalogue = CatalogueDocument::new(
        5,
        CrateName::new("domain").unwrap(),
        LayerId::try_new("domain").unwrap(),
    );
    catalogue.insert_trait(
        CatalogueEntryKey::try_new("TestPort".to_owned()).unwrap(),
        TraitEntry::new(
            ItemAction::Add,
            ContractRole::SecondaryPort,
            vec![MethodDeclaration::new(
                MethodName::new("shape").unwrap(),
                None,
                vec![],
                TypeRef::new("()").unwrap(),
                false,
                false,
                vec![],
                vec![],
                vec![SpecRef::new(
                    PathBuf::from("spec.json"),
                    SpecElementId::try_new("AC-98").unwrap(),
                )],
                ItemAction::Reference,
                None,
            )],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![SpecRef::new(
                PathBuf::from("spec.json"),
                SpecElementId::try_new("IN-05").unwrap(),
            )],
            vec![],
        ),
    );
    let cited = crate::test_obligation::cited_anchor_ids(std::slice::from_ref(&catalogue));
    assert!(cited.contains(&"IN-05".to_owned()));
    assert!(!cited.contains(&"AC-98".to_owned()));
}

#[test]
fn test_cited_anchor_ids_skips_reference_entry_refs() {
    let catalogue = money_catalogue_in_layer("domain", "domain", ItemAction::Reference);
    let cited = crate::test_obligation::cited_anchor_ids(std::slice::from_ref(&catalogue));
    assert!(!cited.contains(&"IN-05".to_owned()));
}

#[test]
fn test_cited_anchor_ids_skips_delete_method_refs() {
    let mut catalogue = CatalogueDocument::new(
        5,
        CrateName::new("domain").unwrap(),
        LayerId::try_new("domain").unwrap(),
    );
    catalogue.insert_trait(
        CatalogueEntryKey::try_new("TestPort".to_owned()).unwrap(),
        TraitEntry::new(
            ItemAction::Add,
            ContractRole::SecondaryPort,
            vec![MethodDeclaration::new(
                MethodName::new("drop").unwrap(),
                None,
                vec![],
                TypeRef::new("()").unwrap(),
                false,
                false,
                vec![],
                vec![],
                vec![SpecRef::new(
                    PathBuf::from("spec.json"),
                    SpecElementId::try_new("AC-99").unwrap(),
                )],
                ItemAction::Delete,
                None,
            )],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![SpecRef::new(
                PathBuf::from("spec.json"),
                SpecElementId::try_new("IN-05").unwrap(),
            )],
            vec![],
        ),
    );
    let cited = crate::test_obligation::cited_anchor_ids(std::slice::from_ref(&catalogue));
    assert!(cited.contains(&"IN-05".to_owned()));
    assert!(!cited.contains(&"AC-99".to_owned()));
}

#[test]
fn test_cited_anchor_ids_includes_type_entry_method_only_refs() {
    let mut catalogue = CatalogueDocument::new(
        5,
        CrateName::new("domain").unwrap(),
        LayerId::try_new("domain").unwrap(),
    );
    catalogue.insert_type(
        CatalogueEntryKey::try_new("Compute".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::domain_service(),
            TypeKindV2::Struct(StructKind::new(StructShape::Unit, None)),
            vec![covering_method("compute", "AC-02")],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![SpecRef::new(
                PathBuf::from("spec.json"),
                SpecElementId::try_new("IN-05").unwrap(),
            )],
            vec![],
        ),
    );
    let cited = crate::test_obligation::cited_anchor_ids(std::slice::from_ref(&catalogue));
    assert!(cited.contains(&"IN-05".to_owned()));
    assert!(cited.contains(&"AC-02".to_owned()));
}

#[test]
fn test_cited_anchor_ids_includes_inherent_method_only_refs() {
    let mut catalogue = money_catalogue();
    catalogue.push_inherent_impl(InherentImplDeclV2::new(
        CatalogueEntryKey::try_new("Money".to_owned()).unwrap(),
        vec![],
        vec![],
        vec![covering_method("amount", "AC-03")],
    ));
    let cited = crate::test_obligation::cited_anchor_ids(std::slice::from_ref(&catalogue));
    assert!(cited.contains(&"IN-05".to_owned()));
    assert!(cited.contains(&"AC-03".to_owned()));
}

#[test]
fn test_cited_anchor_ids_resolves_module_inherent_owner_for_qualified_type_key() {
    let mut catalogue = qualified_money_catalogue();
    catalogue.push_inherent_impl(InherentImplDeclV2::new(
        CatalogueEntryKey::try_new("alpha::Money".to_owned()).unwrap(),
        vec![],
        vec![],
        vec![covering_method("amount", "AC-03")],
    ));
    let cited = crate::test_obligation::cited_anchor_ids(std::slice::from_ref(&catalogue));

    assert!(cited.contains(&"IN-05".to_owned()));
    assert!(cited.contains(&"AC-03".to_owned()));
}

#[test]
fn test_cited_anchor_ids_skips_inherent_methods_on_delete_owner() {
    let mut catalogue = money_catalogue_in_layer("domain", "domain", ItemAction::Delete);
    catalogue.push_inherent_impl(InherentImplDeclV2::new(
        CatalogueEntryKey::try_new("Money".to_owned()).unwrap(),
        vec![],
        vec![],
        vec![covering_method("amount", "AC-04")],
    ));
    let cited = crate::test_obligation::cited_anchor_ids(std::slice::from_ref(&catalogue));
    assert!(!cited.contains(&"IN-05".to_owned()));
    assert!(!cited.contains(&"AC-04".to_owned()));
}

#[test]
fn test_obligation_declaration_text_uses_target_section_for_same_key() {
    let mut catalogue = money_catalogue();
    catalogue.insert_trait(
        CatalogueEntryKey::try_new("Money".to_owned()).unwrap(),
        trait_entry(ContractRole::SecondaryPort),
    );
    let entry_key = entry_key();
    let obligation = TestObligation::new(
        TestObligationId::new(
            entry_key.clone(),
            TestObligationKind::Contract,
            TestObligationItemIdentifier::try_new("trait_method:verify".to_owned()).unwrap(),
        ),
        CatalogueEntryRef::new(
            "domain-types.json".to_owned(),
            CatalogueSectionKey::Traits,
            entry_key,
        ),
        TargetEntryRoleKind::ContractRole(ContractRole::SecondaryPort),
        TestObligationBrief::try_new("cover trait".to_owned()).unwrap(),
        DeclarationHash::new(ContentHash::from_bytes([0u8; 32])),
        vec![anchor()],
    );

    let declaration = obligation_declaration_text(&[catalogue], &obligation).unwrap();

    assert!(declaration.contains("SecondaryPort"));
    assert!(!declaration.contains("ValueObject"));
}

#[test]
fn test_obligation_declaration_text_with_relative_catalogue_identity_matches_anchored_read_path() {
    let entry_key = entry_key();
    let obligation = TestObligation::new(
        TestObligationId::new(
            entry_key.clone(),
            TestObligationKind::Boundary,
            TestObligationItemIdentifier::try_new("invariant:positive".to_owned()).unwrap(),
        ),
        CatalogueEntryRef::new(
            "track/items/my-track/usecase-types.json".to_owned(),
            CatalogueSectionKey::Types,
            entry_key,
        ),
        TargetEntryRoleKind::DataRole(DataRole::value_object()),
        TestObligationBrief::try_new("cover positivity".to_owned()).unwrap(),
        DeclarationHash::new(ContentHash::from_bytes([0u8; 32])),
        vec![anchor()],
    );
    let catalogues = vec![
        LoadedCatalogueDocument::new(
            Path::new("/checkout/project/track/items/my-track/domain-types.json"),
            money_catalogue_with_role(DataRole::entity().unwrap()),
        ),
        LoadedCatalogueDocument::new(
            Path::new("/checkout/project/track/items/my-track/usecase-types.json"),
            money_catalogue_with_role(DataRole::value_object()),
        ),
    ];

    let declaration = obligation_declaration_text_from_loaded(&catalogues, &obligation).unwrap();

    assert!(declaration.contains("ValueObject"));
    assert!(!declaration.contains("Entity"));
}

#[test]
fn test_obligation_declaration_text_with_workspace_qualified_trait_resolves_declaration() {
    let mut impl_catalogue = money_catalogue_with_role(DataRole::value_object());
    impl_catalogue.push_trait_impl(TraitImplDeclV2::new(
        TypeRef::new("infrastructure::alpha::SharedPort").unwrap(),
        TypeRef::new("Money").unwrap(),
    ));

    let mut trait_catalogue = CatalogueDocument::new(
        5,
        CrateName::new("infrastructure").unwrap(),
        LayerId::try_new("infrastructure").unwrap(),
    );
    trait_catalogue.insert_trait(
        CatalogueEntryKey::try_new("infrastructure::alpha::SharedPort".to_owned()).unwrap(),
        TraitEntry::new(
            ItemAction::Add,
            ContractRole::SecondaryPort,
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            ModulePath::from_segments(vec!["alpha".to_owned()]).unwrap(),
            None,
            vec![],
            vec![],
        ),
    );

    let entry_key = entry_key();
    let obligation = TestObligation::new(
        TestObligationId::new(
            entry_key.clone(),
            TestObligationKind::Contract,
            TestObligationItemIdentifier::try_new(
                "trait_impl:infrastructure::alpha::SharedPort".to_owned(),
            )
            .unwrap(),
        ),
        CatalogueEntryRef::new(
            "domain-types.json".to_owned(),
            CatalogueSectionKey::Traits,
            entry_key,
        ),
        TargetEntryRoleKind::TraitImpl(ContractRole::SecondaryPort),
        TestObligationBrief::try_new("cover workspace trait impl".to_owned()).unwrap(),
        DeclarationHash::new(ContentHash::from_bytes([0u8; 32])),
        vec![anchor()],
    );

    let declaration =
        obligation_declaration_text(&[impl_catalogue, trait_catalogue], &obligation).unwrap();

    assert!(declaration.contains("trait_impl:"));
    assert!(declaration.contains("trait_declaration:"));
    assert!(declaration.contains("SecondaryPort"));
}

#[test]
fn test_obligation_declaration_text_with_local_module_qualified_trait_resolves_declaration() {
    let mut impl_catalogue = money_catalogue_with_role(DataRole::value_object());
    impl_catalogue.push_trait_impl(TraitImplDeclV2::new(
        TypeRef::new("alpha::SharedPort").unwrap(),
        TypeRef::new("Money").unwrap(),
    ));

    let mut trait_catalogue = CatalogueDocument::new(
        5,
        CrateName::new("domain").unwrap(),
        LayerId::try_new("domain").unwrap(),
    );
    trait_catalogue.insert_trait(
        CatalogueEntryKey::try_new("domain::alpha::SharedPort".to_owned()).unwrap(),
        TraitEntry::new(
            ItemAction::Add,
            ContractRole::SecondaryPort,
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            ModulePath::from_segments(vec!["alpha".to_owned()]).unwrap(),
            None,
            vec![],
            vec![],
        ),
    );

    let entry_key = entry_key();
    let obligation = TestObligation::new(
        TestObligationId::new(
            entry_key.clone(),
            TestObligationKind::Contract,
            TestObligationItemIdentifier::try_new("trait_impl:alpha::SharedPort".to_owned())
                .unwrap(),
        ),
        CatalogueEntryRef::new(
            "domain-types.json".to_owned(),
            CatalogueSectionKey::Traits,
            entry_key,
        ),
        TargetEntryRoleKind::TraitImpl(ContractRole::SecondaryPort),
        TestObligationBrief::try_new("cover local trait impl".to_owned()).unwrap(),
        DeclarationHash::new(ContentHash::from_bytes([0u8; 32])),
        vec![anchor()],
    );

    let declaration =
        obligation_declaration_text(&[impl_catalogue, trait_catalogue], &obligation).unwrap();

    assert!(declaration.contains("trait_declaration:"));
    assert!(declaration.contains("SecondaryPort"));
}

#[test]
fn test_obligation_declaration_text_freezes_inherent_impl_for_inherent_only_method() {
    let mut catalogue = money_catalogue();
    catalogue.push_inherent_impl(InherentImplDeclV2::new(
        CatalogueEntryKey::try_new("Money".to_owned()).unwrap(),
        vec![],
        vec![],
        vec![MethodDeclaration::new(
            MethodName::new("compute").unwrap(),
            Some(SelfReceiver::SharedRef),
            vec![],
            TypeRef::new("()").unwrap(),
            false,
            false,
            vec![],
            vec![],
            vec![],
            ItemAction::Add,
            None,
        )],
    ));
    let entry_key = entry_key();
    let obligation = TestObligation::new(
        TestObligationId::new(
            entry_key.clone(),
            TestObligationKind::LogicResult,
            TestObligationItemIdentifier::try_new("method:compute".to_owned()).unwrap(),
        ),
        CatalogueEntryRef::new(
            "domain-types.json".to_owned(),
            CatalogueSectionKey::Types,
            entry_key,
        ),
        TargetEntryRoleKind::DataRole(DataRole::value_object()),
        TestObligationBrief::try_new("cover compute".to_owned()).unwrap(),
        DeclarationHash::new(ContentHash::from_bytes([0u8; 32])),
        vec![anchor()],
    );

    let declaration = obligation_declaration_text(&[catalogue], &obligation).unwrap();

    assert!(declaration.contains("inherent_impl:"));
    assert!(declaration.contains("compute"));
}

#[test]
fn test_obligation_declaration_text_freezes_module_inherent_owner_for_qualified_type_key() {
    let mut catalogue = qualified_money_catalogue();
    catalogue.push_inherent_impl(InherentImplDeclV2::new(
        CatalogueEntryKey::try_new("alpha::Money".to_owned()).unwrap(),
        vec![],
        vec![],
        vec![covering_method("compute", "AC-03")],
    ));
    let entry_key = CatalogueEntryKey::try_new("domain::alpha::Money".to_owned()).unwrap();
    let obligation = TestObligation::new(
        TestObligationId::new(
            entry_key.clone(),
            TestObligationKind::LogicResult,
            TestObligationItemIdentifier::try_new("method:compute".to_owned()).unwrap(),
        ),
        CatalogueEntryRef::new(
            "domain-types.json".to_owned(),
            CatalogueSectionKey::Types,
            entry_key,
        ),
        TargetEntryRoleKind::DataRole(DataRole::value_object()),
        TestObligationBrief::try_new("cover compute".to_owned()).unwrap(),
        DeclarationHash::new(ContentHash::from_bytes([0u8; 32])),
        vec![anchor()],
    );

    let declaration = obligation_declaration_text(&[catalogue], &obligation).unwrap();

    assert!(declaration.contains("inherent_impl:"));
    assert!(declaration.contains("compute"));
}

#[test]
fn test_obligation_declaration_text_resolves_module_trait_impl_owner_for_qualified_type_key() {
    let mut catalogue = qualified_money_catalogue();
    catalogue.insert_trait(
        CatalogueEntryKey::try_new("MyPort".to_owned()).unwrap(),
        trait_entry(ContractRole::SecondaryPort),
    );
    catalogue.push_trait_impl(TraitImplDeclV2::new(
        TypeRef::new("MyPort").unwrap(),
        TypeRef::new("alpha::Money").unwrap(),
    ));
    let entry_key = CatalogueEntryKey::try_new("domain::alpha::Money".to_owned()).unwrap();
    let obligation = TestObligation::new(
        TestObligationId::new(
            entry_key.clone(),
            TestObligationKind::Contract,
            TestObligationItemIdentifier::try_new("trait_impl:MyPort".to_owned()).unwrap(),
        ),
        CatalogueEntryRef::new(
            "domain-types.json".to_owned(),
            CatalogueSectionKey::Traits,
            entry_key,
        ),
        TargetEntryRoleKind::TraitImpl(ContractRole::SecondaryPort),
        TestObligationBrief::try_new("cover trait implementation".to_owned()).unwrap(),
        DeclarationHash::new(ContentHash::from_bytes([0u8; 32])),
        vec![anchor()],
    );

    let declaration = obligation_declaration_text(&[catalogue], &obligation).unwrap();

    assert!(declaration.contains("trait_impl:"));
    assert!(declaration.contains("trait_declaration:"));
    assert!(declaration.contains("SecondaryPort"));
}

#[test]
fn test_check_same_entry_in_two_layers_uses_obligation_origin_for_status_lane() {
    let domain_catalogue = money_catalogue_in_layer("domain", "domain", ItemAction::Reference);
    let usecase_catalogue = money_catalogue_in_layer("usecase", "usecase", ItemAction::Add);
    let catalogue_paths =
        vec![PathBuf::from("domain-types.json"), PathBuf::from("usecase-types.json")];
    let obligations = derive_obligations_document(
        track(),
        &rules_doc(),
        &[
            (catalogue_paths[0].clone(), domain_catalogue.clone()),
            (catalogue_paths[1].clone(), usecase_catalogue.clone()),
        ],
        &RoleObligationItemsProjector::new(),
    )
    .unwrap();

    let mut entries = BTreeMap::new();
    entries.insert(
        TaskId::try_new("T001".to_owned()).unwrap(),
        vec![ContractedEntryRef::new(LayerId::try_new("domain").unwrap(), entry_key())],
    );
    entries.insert(
        TaskId::try_new("T002".to_owned()).unwrap(),
        vec![ContractedEntryRef::new(LayerId::try_new("usecase").unwrap(), entry_key())],
    );
    let mut statuses = HashMap::new();
    statuses.insert(TaskId::try_new("T001".to_owned()).unwrap(), TaskStatusKind::Done);
    statuses.insert(TaskId::try_new("T002".to_owned()).unwrap(), TaskStatusKind::Todo);
    let interactor = CheckTestObligationsInteractor::new(
        Arc::new(StubRules::valid()),
        Arc::new(StubObligations(Some(obligations))),
        Arc::new(StubBindings(Some(TestBindingsDocument::new(track(), Vec::new())))),
        Arc::new(StubScanner),
        Arc::new(StubFulfillmentCache(None)),
        Arc::new(StubWaiverCache(None)),
        fulfillment_verifier_fingerprint(),
        waiver_verifier_fingerprint(),
        Arc::new(StubSpec(spec_doc())),
        Arc::new(StubCatalogues(vec![
            (catalogue_paths[0].clone(), domain_catalogue),
            (catalogue_paths[1].clone(), usecase_catalogue),
        ])),
        Arc::new(StubTaskContractReader(TaskContractDocument::new(track(), entries).unwrap())),
        Arc::new(StubImplPlanReader(statuses)),
    );
    let command = CheckTestObligationsCommand::new(TestObligationCatalogueCommandInput::new(
        track(),
        "track/my-track".to_owned(),
        catalogue_paths,
    ));

    let outcome = interactor.execute(&command).unwrap();
    let todo = outcome
        .status_lane_summaries()
        .iter()
        .find(|summary| summary.task_status() == TaskStatusKind::Todo)
        .unwrap();
    let done = outcome
        .status_lane_summaries()
        .iter()
        .find(|summary| summary.task_status() == TaskStatusKind::Done)
        .unwrap();
    assert_eq!(todo.missing_count(), 1);
    assert_eq!(done.missing_count(), 0);
}

#[test]
fn test_obligations_only_is_fail_closed() {
    // AC-10: half-materialised scope is fail-closed.
    let result =
        interactor(Some(ObligationsDocument::new(track(), vec![obligation()])), None, None, None)
            .execute(&command());
    // Obligations present but bindings absent → the bindings side is absent.
    assert!(matches!(result, Err(ObligationCheckError::BindingsAbsent)));
}

#[test]
fn test_bindings_only_is_fail_closed() {
    let result = interactor(
        None,
        Some(TestBindingsDocument::new(track(), vec![fulfillment_binding()])),
        None,
        None,
    )
    .execute(&command());
    // Bindings present but obligations absent → the obligations side is absent.
    assert!(matches!(result, Err(ObligationCheckError::ObligationsAbsent)));
}

#[test]
fn test_missing_obligation_is_a_drift() {
    // IN-13: an obligation with no binding is `missing`.
    let obligations = ObligationsDocument::new(track(), vec![obligation()]);
    let bindings = TestBindingsDocument::new(track(), vec![]);
    let result = interactor(Some(obligations), Some(bindings), None, None).execute(&command());
    assert!(matches!(result, Err(ObligationCheckError::DriftsDetected { .. })));
}

#[test]
fn test_orphaned_binding_is_a_drift() {
    // IN-13 / CN-11: a Fulfillment binding for a non-derived obligation is `orphaned`.
    let obligations = ObligationsDocument::new(track(), vec![]);
    let bindings = TestBindingsDocument::new(track(), vec![fulfillment_binding()]);
    let result =
        interactor_with_rules(Some(obligations), Some(bindings), None, None, empty_rules_doc())
            .execute(&command());
    assert!(matches!(result, Err(ObligationCheckError::DriftsDetected { .. })));
}

#[test]
fn test_orphaned_waiver_binding_is_a_drift() {
    // IN-13 / CN-11: a Waiver binding for a no-longer-derived edge is `orphaned`.
    let obligations = ObligationsDocument::new(track(), vec![]);
    let bindings = TestBindingsDocument::new(track(), vec![waiver_binding_for(unknown_edge())]);
    let result =
        interactor_with_rules(Some(obligations), Some(bindings), None, None, empty_rules_doc())
            .execute(&command());
    assert!(matches!(result, Err(ObligationCheckError::DriftsDetected { .. })));
}

#[test]
fn test_fulfillment_on_obligation_that_owns_no_anchors_is_orphaned() {
    // D1: a derived obligation with empty spec_refs is not a fulfillment target.
    // Presence of a fulfillment record must not pass check while evaluate plans
    // zero verifier actions.
    let catalogue = anchorless_secondary_port_catalogue();
    let rules = secondary_port_trait_method_rules_doc();
    let obligations = derived_obligations(rules.clone(), catalogue.clone());
    assert_eq!(obligations.obligations().len(), 1);
    assert!(obligations.obligations()[0].spec_refs().is_empty());
    let bindings = TestBindingsDocument::new(
        track(),
        vec![fulfillment_binding_for(&obligations.obligations()[0])],
    );

    let result =
        anchorless_port_interactor(obligations, bindings, rules, catalogue).execute(&command());

    let Err(ObligationCheckError::DriftsDetected { drifts }) = result else {
        panic!("fulfillment on an anchorless obligation must be a structural drift");
    };
    assert_eq!(drifts.as_slice().len(), 1);
    let rendered = format!("{drifts:?}");
    assert!(rendered.contains("Orphaned"));
    assert!(rendered.contains("owns no anchors"));
}

#[test]
fn test_check_does_not_reject_historical_add_catalogue_as_invalid_state() {
    // D1: read-only check re-derives without applying write-side coverage, so
    // an enrolled Add catalogue that predates method-level refs is not
    // InvalidCatalogueState.
    let catalogue = incomplete_coverage_catalogue(ItemAction::Add);
    let rules = secondary_port_trait_method_rules_doc();
    let obligations = derived_obligations(rules.clone(), catalogue.clone());
    let bindings = TestBindingsDocument::new(track(), vec![]);

    let result =
        anchorless_port_interactor(obligations, bindings, rules, catalogue).execute(&command());

    assert!(!matches!(result, Err(ObligationCheckError::InvalidCatalogueState(_))));
}

#[test]
fn test_check_does_not_reject_reference_trait_incomplete_coverage() {
    let catalogue = incomplete_coverage_catalogue(ItemAction::Reference);
    let rules = secondary_port_trait_method_rules_doc();
    let obligations = derived_obligations(rules.clone(), catalogue.clone());
    let bindings = TestBindingsDocument::new(track(), vec![]);

    let result =
        anchorless_port_interactor(obligations, bindings, rules, catalogue).execute(&command());

    assert!(!matches!(result, Err(ObligationCheckError::InvalidCatalogueState(_))));
}

#[test]
fn test_anchorless_obligation_without_fulfillment_is_not_missing() {
    // D1: owning no anchors means there is nothing to fulfill; absence of a
    // fulfillment record is not `missing`.
    let catalogue = anchorless_secondary_port_catalogue();
    let rules = secondary_port_trait_method_rules_doc();
    let obligations = derived_obligations(rules.clone(), catalogue.clone());
    let bindings = TestBindingsDocument::new(track(), vec![]);

    let outcome = anchorless_port_interactor(obligations, bindings, rules, catalogue)
        .execute(&command())
        .unwrap();

    assert!(outcome.resolved_edges().is_empty());
}

#[test]
fn test_zero_obligation_cited_edge_without_binding_is_unresolved() {
    // CN-02 / AC-04: a cited active entry edge remains in the totality universe
    // even when the decision table derives zero obligations for it.
    let obligations = ObligationsDocument::new(track(), vec![]);
    let bindings = TestBindingsDocument::new(track(), vec![]);
    let result =
        interactor_with_rules(Some(obligations), Some(bindings), None, None, empty_rules_doc())
            .execute(&command());
    assert!(matches!(result, Err(ObligationCheckError::UnresolvedEdges { .. })));
}

#[test]
fn test_zero_obligation_cited_edge_waiver_resolves() {
    // D4 / CN-02: a zero-obligation edge is resolved by a fresh waived verdict.
    let obligations = ObligationsDocument::new(track(), vec![]);
    let bindings = TestBindingsDocument::new(track(), vec![waiver_binding()]);
    let cache = fresh_direct_waiver_cache();

    let outcome = interactor_with_rules(
        Some(obligations),
        Some(bindings),
        None,
        Some(cache),
        empty_rules_doc(),
    )
    .execute(&command())
    .unwrap();

    assert_eq!(outcome.resolved_edges(), &[edge()]);
}

#[test]
fn test_zero_obligation_cited_edge_voluntary_binding_resolves() {
    // AC-04 / D9: an edge-direct voluntary binding is valid without a derived obligation.
    let obligations = ObligationsDocument::new(track(), vec![]);
    let bindings = TestBindingsDocument::new(track(), vec![voluntary_binding()]);
    let cache = fresh_voluntary_fulfillment_cache();

    let outcome = interactor_with_rules(
        Some(obligations),
        Some(bindings),
        Some(cache),
        None,
        empty_rules_doc(),
    )
    .execute(&command())
    .unwrap();

    assert_eq!(outcome.resolved_edges(), &[edge()]);
}

#[test]
fn test_fresh_fulfilled_verdict_resolves_edge() {
    // AC-04: a fresh fulfilled verdict resolves the edge and the gate passes.
    let obligations = ObligationsDocument::new(track(), vec![obligation()]);
    let bindings = TestBindingsDocument::new(track(), vec![fulfillment_binding()]);
    let outcome =
        interactor(Some(obligations), Some(bindings), Some(fresh_fulfillment_cache()), None)
            .execute(&command())
            .unwrap();
    assert_eq!(outcome.resolved_edges(), &[edge()]);
}

#[test]
fn test_check_todo_missing_binding_warns_without_blocking() {
    let outcome = missing_binding_interactor_with_statuses(&[("T001", TaskStatusKind::Todo)])
        .execute(&command())
        .unwrap();

    let todo = outcome
        .status_lane_summaries()
        .iter()
        .find(|summary| summary.task_status() == TaskStatusKind::Todo)
        .unwrap();
    assert_eq!(todo.missing_count(), 1);
    assert_eq!(todo.stale_count(), 0);
    assert_eq!(todo.verdict_absent_count(), 0);
}

#[test]
fn test_check_strictest_non_todo_attribution_blocks_missing_binding() {
    let result = missing_binding_interactor_with_statuses(&[
        ("T001", TaskStatusKind::Todo),
        ("T002", TaskStatusKind::InProgress),
    ])
    .execute(&command());

    assert!(matches!(result, Err(ObligationCheckError::DriftsDetected { .. })));
}

#[test]
fn test_check_new_resolves_task_contract_and_impl_plan_strictest_wins() {
    let todo_task = TaskId::try_new("T001".to_owned()).unwrap();
    let in_progress_task = TaskId::try_new("T002".to_owned()).unwrap();
    let contracted_entry =
        ContractedEntryRef::new(LayerId::try_new("domain").unwrap(), entry_key());
    let mut entries = BTreeMap::new();
    entries.insert(todo_task.clone(), vec![contracted_entry.clone()]);
    entries.insert(in_progress_task.clone(), vec![contracted_entry]);
    let task_contract = TaskContractDocument::new(track(), entries).unwrap();
    let mut statuses = HashMap::new();
    statuses.insert(todo_task, TaskStatusKind::Todo);
    statuses.insert(in_progress_task, TaskStatusKind::InProgress);

    let interactor = CheckTestObligationsInteractor::new(
        Arc::new(StubRules::valid()),
        Arc::new(StubObligations(Some(ObligationsDocument::new(track(), vec![obligation()])))),
        Arc::new(StubBindings(Some(TestBindingsDocument::new(track(), Vec::new())))),
        Arc::new(StubScanner),
        Arc::new(StubFulfillmentCache(None)),
        Arc::new(StubWaiverCache(None)),
        fulfillment_verifier_fingerprint(),
        waiver_verifier_fingerprint(),
        Arc::new(StubSpec(spec_doc())),
        Arc::new(StubCatalogue(money_catalogue())),
        Arc::new(StubTaskContractReader(task_contract)),
        Arc::new(StubImplPlanReader(statuses)),
    );

    let result = interactor.execute(&command());

    assert!(matches!(result, Err(ObligationCheckError::DriftsDetected { .. })));
}

#[test]
fn test_check_in_progress_blocks_while_todo_warns_and_passes() {
    let in_progress =
        missing_binding_interactor_with_statuses(&[("T001", TaskStatusKind::InProgress)])
            .execute(&command());
    assert!(matches!(in_progress, Err(ObligationCheckError::DriftsDetected { .. })));

    let todo = missing_binding_interactor_with_statuses(&[("T001", TaskStatusKind::Todo)])
        .execute(&command())
        .unwrap();
    let todo_summary = todo
        .status_lane_summaries()
        .iter()
        .find(|summary| summary.task_status() == TaskStatusKind::Todo)
        .unwrap();
    assert_eq!(todo_summary.missing_count(), 1);
    assert_eq!(todo_summary.stale_count(), 0);
    assert_eq!(todo_summary.verdict_absent_count(), 0);
}

#[test]
fn test_check_new_injected_readers_choose_blocking_and_warning_lanes() {
    let entry = ContractedEntryRef::new(LayerId::try_new("domain").unwrap(), entry_key());
    let in_progress_task = TaskId::try_new("T001".to_owned()).unwrap();
    let mut in_progress_entries = BTreeMap::new();
    in_progress_entries.insert(in_progress_task.clone(), vec![entry.clone()]);
    let mut in_progress_statuses = HashMap::new();
    in_progress_statuses.insert(in_progress_task, TaskStatusKind::InProgress);

    let in_progress = CheckTestObligationsInteractor::new(
        Arc::new(StubRules::valid()),
        Arc::new(StubObligations(Some(ObligationsDocument::new(track(), vec![obligation()])))),
        Arc::new(StubBindings(Some(TestBindingsDocument::new(track(), Vec::new())))),
        Arc::new(StubScanner),
        Arc::new(StubFulfillmentCache(None)),
        Arc::new(StubWaiverCache(None)),
        fulfillment_verifier_fingerprint(),
        waiver_verifier_fingerprint(),
        Arc::new(StubSpec(spec_doc())),
        Arc::new(StubCatalogue(money_catalogue())),
        Arc::new(StubTaskContractReader(
            TaskContractDocument::new(track(), in_progress_entries).unwrap(),
        )),
        Arc::new(StubImplPlanReader(in_progress_statuses)),
    )
    .execute(&command());
    assert!(matches!(in_progress, Err(ObligationCheckError::DriftsDetected { .. })));

    let todo_task = TaskId::try_new("T001".to_owned()).unwrap();
    let mut todo_entries = BTreeMap::new();
    todo_entries.insert(todo_task.clone(), vec![entry]);
    let mut todo_statuses = HashMap::new();
    todo_statuses.insert(todo_task, TaskStatusKind::Todo);

    let todo = CheckTestObligationsInteractor::new(
        Arc::new(StubRules::valid()),
        Arc::new(StubObligations(Some(ObligationsDocument::new(track(), vec![obligation()])))),
        Arc::new(StubBindings(Some(TestBindingsDocument::new(track(), Vec::new())))),
        Arc::new(StubScanner),
        Arc::new(StubFulfillmentCache(None)),
        Arc::new(StubWaiverCache(None)),
        fulfillment_verifier_fingerprint(),
        waiver_verifier_fingerprint(),
        Arc::new(StubSpec(spec_doc())),
        Arc::new(StubCatalogue(money_catalogue())),
        Arc::new(StubTaskContractReader(TaskContractDocument::new(track(), todo_entries).unwrap())),
        Arc::new(StubImplPlanReader(todo_statuses)),
    )
    .execute(&command())
    .unwrap();
    let todo_summary = todo
        .status_lane_summaries()
        .iter()
        .find(|summary| summary.task_status() == TaskStatusKind::Todo)
        .unwrap();
    assert_eq!(todo_summary.missing_count(), 1);
}

#[test]
fn test_check_done_attribution_blocks_every_unresolved_kind() {
    let missing = missing_binding_interactor_with_statuses(&[("T001", TaskStatusKind::Done)])
        .execute(&command());
    assert!(matches!(missing, Err(ObligationCheckError::DriftsDetected { .. })));

    let bindings = TestBindingsDocument::new(track(), vec![fulfillment_binding()]);
    let stale_cache = fresh_fulfillment_cache_for_fingerprint(
        &obligation(),
        Some(VerifierPromptFingerprint::new(ContentHash::from_bytes([7u8; 32]))),
    );
    let stale = interactor_with_task_statuses(
        Some(ObligationsDocument::new(track(), vec![obligation()])),
        Some(bindings.clone()),
        Some(stale_cache),
        None,
        &[("T001", TaskStatusKind::Done)],
    )
    .execute(&command());
    assert!(matches!(stale, Err(ObligationCheckError::StaleVerdicts { .. })));

    let verdict_absent = interactor_with_task_statuses(
        Some(ObligationsDocument::new(track(), vec![obligation()])),
        Some(bindings),
        None,
        None,
        &[("T001", TaskStatusKind::Done)],
    )
    .execute(&command());
    assert!(matches!(verdict_absent, Err(ObligationCheckError::StaleVerdicts { .. })));
}

#[test]
fn test_check_stale_classification_is_status_independent_before_final_gate() {
    for status in [
        TaskStatusKind::Todo,
        TaskStatusKind::InProgress,
        TaskStatusKind::Done,
        TaskStatusKind::Skipped,
    ] {
        let result = interactor_with_task_statuses(
            Some(ObligationsDocument::new(track(), vec![obligation()])),
            Some(TestBindingsDocument::new(track(), vec![fulfillment_binding()])),
            Some(fresh_fulfillment_cache_for_fingerprint(
                &obligation(),
                Some(VerifierPromptFingerprint::new(ContentHash::from_bytes([7u8; 32]))),
            )),
            None,
            &[("T001", status)],
        )
        .execute(&command());

        if status == TaskStatusKind::Todo {
            let outcome = result.unwrap();
            let todo = outcome
                .status_lane_summaries()
                .iter()
                .find(|summary| summary.task_status() == TaskStatusKind::Todo)
                .unwrap();
            assert_eq!(todo.verdict_absent_count(), 1);
        } else {
            assert!(matches!(result, Err(ObligationCheckError::StaleVerdicts { .. })));
        }
    }
}

#[test]
fn test_check_todo_stale_verdict_warns_without_blocking() {
    let stale_cache = fresh_fulfillment_cache_for_fingerprint(
        &obligation(),
        Some(VerifierPromptFingerprint::new(ContentHash::from_bytes([7u8; 32]))),
    );

    let outcome = interactor_with_task_statuses(
        Some(ObligationsDocument::new(track(), vec![obligation()])),
        Some(TestBindingsDocument::new(track(), vec![fulfillment_binding()])),
        Some(stale_cache),
        None,
        &[("T001", TaskStatusKind::Todo)],
    )
    .execute(&command())
    .unwrap();

    let todo = outcome
        .status_lane_summaries()
        .iter()
        .find(|summary| summary.task_status() == TaskStatusKind::Todo)
        .unwrap();
    assert_eq!(todo.missing_count(), 0);
    assert_eq!(todo.stale_count(), 0);
    assert_eq!(todo.verdict_absent_count(), 1);
}

#[test]
fn test_check_skipped_attribution_blocks_missing_binding() {
    let result = missing_binding_interactor_with_statuses(&[("T001", TaskStatusKind::Skipped)])
        .execute(&command());

    assert!(matches!(result, Err(ObligationCheckError::DriftsDetected { .. })));
}

#[test]
fn test_check_skipped_attribution_blocks_stale_verdict() {
    let bindings = TestBindingsDocument::new(track(), vec![fulfillment_binding()]);
    let stale_cache = fresh_fulfillment_cache_for_fingerprint(
        &obligation(),
        Some(VerifierPromptFingerprint::new(ContentHash::from_bytes([7u8; 32]))),
    );

    let result = interactor_with_task_statuses(
        Some(ObligationsDocument::new(track(), vec![obligation()])),
        Some(bindings),
        Some(stale_cache),
        None,
        &[("T001", TaskStatusKind::Skipped)],
    )
    .execute(&command());

    assert!(matches!(result, Err(ObligationCheckError::StaleVerdicts { .. })));
}

#[test]
fn test_check_new_fails_closed_for_skipped_missing_and_absent_verdicts() {
    let task_id = TaskId::try_new("T001".to_owned()).unwrap();
    let mut entries = BTreeMap::new();
    entries.insert(
        task_id.clone(),
        vec![ContractedEntryRef::new(LayerId::try_new("domain").unwrap(), entry_key())],
    );
    let mut statuses = HashMap::new();
    statuses.insert(task_id, TaskStatusKind::Skipped);
    let missing = CheckTestObligationsInteractor::new(
        Arc::new(StubRules::valid()),
        Arc::new(StubObligations(Some(ObligationsDocument::new(track(), vec![obligation()])))),
        Arc::new(StubBindings(Some(TestBindingsDocument::new(track(), Vec::new())))),
        Arc::new(StubScanner),
        Arc::new(StubFulfillmentCache(None)),
        Arc::new(StubWaiverCache(None)),
        fulfillment_verifier_fingerprint(),
        waiver_verifier_fingerprint(),
        Arc::new(StubSpec(spec_doc())),
        Arc::new(StubCatalogue(money_catalogue())),
        Arc::new(StubTaskContractReader(TaskContractDocument::new(track(), entries).unwrap())),
        Arc::new(StubImplPlanReader(statuses)),
    )
    .execute(&command());
    assert!(matches!(missing, Err(ObligationCheckError::DriftsDetected { .. })));

    let verdict_absent = interactor_with_task_statuses(
        Some(ObligationsDocument::new(track(), vec![obligation()])),
        Some(TestBindingsDocument::new(track(), vec![fulfillment_binding()])),
        None,
        None,
        &[("T001", TaskStatusKind::Skipped)],
    )
    .execute(&command());
    assert!(matches!(verdict_absent, Err(ObligationCheckError::StaleVerdicts { .. })));
}

#[test]
fn test_check_outcome_retains_skipped_lane_summary() {
    let outcome = CheckTestObligationsOutcome::new_verified_scope(
        Vec::new(),
        Vec::new(),
        vec![TestObligationStatusLaneSummary::new(TaskStatusKind::Skipped, 1, 2, 3)],
    );

    let skipped = outcome
        .status_lane_summaries()
        .iter()
        .find(|summary| summary.task_status() == TaskStatusKind::Skipped)
        .unwrap();
    assert_eq!(skipped.missing_count(), 1);
    assert_eq!(skipped.stale_count(), 2);
    assert_eq!(skipped.verdict_absent_count(), 3);
}

#[test]
fn test_check_verdict_absence_warns_for_todo_but_blocks_skipped() {
    let obligations = ObligationsDocument::new(track(), vec![obligation()]);
    let bindings = TestBindingsDocument::new(track(), vec![fulfillment_binding()]);

    let todo_outcome = interactor_with_task_statuses(
        Some(obligations.clone()),
        Some(bindings.clone()),
        None,
        None,
        &[("T001", TaskStatusKind::Todo)],
    )
    .execute(&command())
    .unwrap();
    let todo = todo_outcome
        .status_lane_summaries()
        .iter()
        .find(|summary| summary.task_status() == TaskStatusKind::Todo)
        .unwrap();
    assert_eq!(todo.verdict_absent_count(), 1);

    let skipped_result = interactor_with_task_statuses(
        Some(obligations),
        Some(bindings),
        None,
        None,
        &[("T001", TaskStatusKind::Skipped)],
    )
    .execute(&command());
    assert!(matches!(skipped_result, Err(ObligationCheckError::StaleVerdicts { .. })));
}

#[test]
fn test_check_orphaned_binding_fails_closed_for_todo() {
    let expected_obligation = obligation();
    let orphaned_obligation = obligation_with_item("orphaned");
    let bindings = TestBindingsDocument::new(
        track(),
        vec![
            fulfillment_binding_for(&expected_obligation),
            fulfillment_binding_for(&orphaned_obligation),
        ],
    );

    let result = interactor_with_task_statuses(
        Some(ObligationsDocument::new(track(), vec![expected_obligation])),
        Some(bindings),
        Some(fresh_fulfillment_cache()),
        None,
        &[("T001", TaskStatusKind::Todo)],
    )
    .execute(&command());

    assert!(matches!(result, Err(ObligationCheckError::DriftsDetected { .. })));
}

#[test]
fn test_check_orphaned_binding_across_task_statuses_returns_same_drift() {
    let mut classifications = Vec::new();

    for status in [TaskStatusKind::Todo, TaskStatusKind::Done] {
        let expected_obligation = obligation();
        let orphaned_obligation = obligation_with_item("orphaned");
        let bindings = TestBindingsDocument::new(
            track(),
            vec![
                fulfillment_binding_for(&expected_obligation),
                fulfillment_binding_for(&orphaned_obligation),
            ],
        );
        let result = interactor_with_task_statuses(
            Some(ObligationsDocument::new(track(), vec![expected_obligation])),
            Some(bindings),
            Some(fresh_fulfillment_cache()),
            None,
            &[("T001", status)],
        )
        .execute(&command());

        let Err(ObligationCheckError::DriftsDetected { drifts }) = result else {
            panic!("orphaned binding must remain a structural drift for every status");
        };
        assert_eq!(drifts.as_slice().len(), 1);
        let classification = format!("{drifts:?}");
        assert!(classification.contains("Orphaned"));
        classifications.push(classification);
    }

    assert_eq!(classifications[0], classifications[1]);
}

#[test]
fn test_check_malformed_artifacts_block_across_task_statuses() {
    for status in [
        TaskStatusKind::Todo,
        TaskStatusKind::InProgress,
        TaskStatusKind::Done,
        TaskStatusKind::Skipped,
    ] {
        let malformed_obligations = CheckTestObligationsInteractor::new(
            Arc::new(StubRules::valid()),
            Arc::new(MalformedObligationsArtifact),
            Arc::new(StubBindings(None)),
            Arc::new(StubScanner),
            Arc::new(StubFulfillmentCache(None)),
            Arc::new(StubWaiverCache(None)),
            fulfillment_verifier_fingerprint(),
            waiver_verifier_fingerprint(),
            Arc::new(StubSpec(spec_doc())),
            Arc::new(StubCatalogue(money_catalogue())),
            task_contract_reader(),
            impl_plan_reader_with(&[("T001", status)]),
        )
        .execute(&command());
        assert!(matches!(
            malformed_obligations,
            Err(ObligationCheckError::ArtifactCodec(ArtifactCodecError::MalformedJson(_)))
        ));

        let malformed_bindings = CheckTestObligationsInteractor::new(
            Arc::new(StubRules::valid()),
            Arc::new(StubObligations(None)),
            Arc::new(MalformedBindingsArtifact),
            Arc::new(StubScanner),
            Arc::new(StubFulfillmentCache(None)),
            Arc::new(StubWaiverCache(None)),
            fulfillment_verifier_fingerprint(),
            waiver_verifier_fingerprint(),
            Arc::new(StubSpec(spec_doc())),
            Arc::new(StubCatalogue(money_catalogue())),
            task_contract_reader(),
            impl_plan_reader_with(&[("T001", status)]),
        )
        .execute(&command());
        assert!(matches!(
            malformed_bindings,
            Err(ObligationCheckError::ArtifactCodec(ArtifactCodecError::MalformedJson(_)))
        ));
    }
}

#[test]
fn test_check_coverage_broken_entry_blocks_across_task_statuses() {
    for status in [
        TaskStatusKind::Todo,
        TaskStatusKind::InProgress,
        TaskStatusKind::Done,
        TaskStatusKind::Skipped,
    ] {
        // The contract omits Money, which makes this fixture fail the required
        // task-contract coverage precondition before status interpretation.
        let mut entries = BTreeMap::new();
        entries.insert(
            TaskId::try_new("T001".to_owned()).unwrap(),
            vec![ContractedEntryRef::new(
                LayerId::try_new("domain".to_owned()).unwrap(),
                CatalogueEntryKey::try_new("OtherEntry".to_owned()).unwrap(),
            )],
        );
        let mut statuses = HashMap::new();
        statuses.insert(TaskId::try_new("T001".to_owned()).unwrap(), status);
        let interactor = CheckTestObligationsInteractor::new(
            Arc::new(StubRules::valid()),
            Arc::new(StubObligations(Some(ObligationsDocument::new(track(), vec![obligation()])))),
            Arc::new(StubBindings(Some(TestBindingsDocument::new(track(), Vec::new())))),
            Arc::new(StubScanner),
            Arc::new(StubFulfillmentCache(None)),
            Arc::new(StubWaiverCache(None)),
            fulfillment_verifier_fingerprint(),
            waiver_verifier_fingerprint(),
            Arc::new(StubSpec(spec_doc())),
            Arc::new(StubCatalogue(money_catalogue())),
            Arc::new(StubTaskContractReader(TaskContractDocument::new(track(), entries).unwrap())),
            Arc::new(StubImplPlanReader(statuses)),
        );

        let result = interactor.execute(&command());

        assert!(matches!(result, Err(ObligationCheckError::TaskAttribution(_))));
    }
}

#[test]
fn test_check_unattributable_entry_returns_task_attribution_error() {
    let mut entries = BTreeMap::new();
    entries.insert(
        TaskId::try_new("T001".to_owned()).unwrap(),
        vec![ContractedEntryRef::new(
            LayerId::try_new("domain".to_owned()).unwrap(),
            CatalogueEntryKey::try_new("OtherEntry".to_owned()).unwrap(),
        )],
    );
    let task_contract = TaskContractDocument::new(track(), entries).unwrap();
    let mut statuses = HashMap::new();
    statuses.insert(TaskId::try_new("T001".to_owned()).unwrap(), TaskStatusKind::Todo);
    let interactor = CheckTestObligationsInteractor::new(
        Arc::new(StubRules::valid()),
        Arc::new(StubObligations(Some(ObligationsDocument::new(track(), vec![obligation()])))),
        Arc::new(StubBindings(Some(TestBindingsDocument::new(track(), Vec::new())))),
        Arc::new(StubScanner),
        Arc::new(StubFulfillmentCache(None)),
        Arc::new(StubWaiverCache(None)),
        fulfillment_verifier_fingerprint(),
        waiver_verifier_fingerprint(),
        Arc::new(StubSpec(spec_doc())),
        Arc::new(StubCatalogue(money_catalogue())),
        Arc::new(StubTaskContractReader(task_contract)),
        Arc::new(StubImplPlanReader(statuses)),
    );

    let result = interactor.execute(&command());

    assert!(matches!(result, Err(ObligationCheckError::TaskAttribution(_))));
}

#[test]
fn test_check_structural_failures_block_regardless_of_task_status() {
    for status in [
        TaskStatusKind::Todo,
        TaskStatusKind::InProgress,
        TaskStatusKind::Done,
        TaskStatusKind::Skipped,
    ] {
        let unattributable = unattributable_interactor_with_status(status).execute(&command());
        assert!(matches!(unattributable, Err(ObligationCheckError::TaskAttribution(_))));

        let invalid_rules = interactor_with_rules_loader_and_status(Arc::new(FailingRules), status)
            .execute(&command());
        assert!(matches!(
            invalid_rules,
            Err(ObligationCheckError::RulesLoad(
                TestObligationRulesLoadError::RoleNotCovered { .. }
            ))
        ));
    }
}

#[test]
fn test_check_with_skipped_and_todo_attribution_blocks() {
    let result = missing_binding_interactor_with_statuses(&[
        ("T001", TaskStatusKind::Todo),
        ("T002", TaskStatusKind::Skipped),
    ])
    .execute(&command());

    assert!(matches!(result, Err(ObligationCheckError::DriftsDetected { .. })));
}

#[test]
fn test_new_accepts_and_wires_declared_dependencies() {
    let obligations = derived_obligations(rules_doc(), money_catalogue());
    let bindings = TestBindingsDocument::new(track(), vec![fulfillment_binding()]);
    let fulfillment_fingerprint =
        VerifierPromptFingerprint::new(ContentHash::from_bytes([17u8; 32]));
    let waiver_fingerprint = VerifierPromptFingerprint::new(ContentHash::from_bytes([23u8; 32]));
    let interactor = CheckTestObligationsInteractor::new(
        Arc::new(StubRules::valid()),
        Arc::new(StubObligations(Some(obligations.clone()))),
        Arc::new(StubBindings(Some(bindings))),
        Arc::new(StubScanner),
        Arc::new(StubFulfillmentCache(Some(fresh_fulfillment_cache_for_fingerprint(
            &obligation(),
            Some(fulfillment_fingerprint.clone()),
        )))),
        Arc::new(StubWaiverCache(None)),
        fulfillment_fingerprint.clone(),
        waiver_fingerprint.clone(),
        Arc::new(StubSpec(spec_doc())),
        Arc::new(StubCatalogue(money_catalogue())),
        task_contract_reader(),
        impl_plan_reader(),
    );

    let outcome = interactor.execute(&command()).unwrap();

    assert_eq!(outcome.resolved_edges(), &[edge()]);

    let waiver_interactor = CheckTestObligationsInteractor::new(
        Arc::new(StubRules::valid()),
        Arc::new(StubObligations(Some(obligations))),
        Arc::new(StubBindings(Some(TestBindingsDocument::new(track(), vec![waiver_binding()])))),
        Arc::new(StubScanner),
        Arc::new(StubFulfillmentCache(None)),
        Arc::new(StubWaiverCache(Some(fresh_waiver_cache_for_fingerprint(
            &obligation(),
            Some(waiver_fingerprint.clone()),
        )))),
        fulfillment_fingerprint,
        waiver_fingerprint,
        Arc::new(StubSpec(spec_doc())),
        Arc::new(StubCatalogue(money_catalogue())),
        task_contract_reader(),
        impl_plan_reader(),
    );

    let waiver_outcome = waiver_interactor.execute(&command()).unwrap();

    assert_eq!(waiver_outcome.resolved_edges(), &[edge()]);
}

#[test]
fn test_matching_obligations_artifact_passes_check() {
    let obligations = derived_obligations(rules_doc(), money_catalogue());
    let bindings = TestBindingsDocument::new(track(), vec![fulfillment_binding()]);

    let outcome =
        interactor(Some(obligations), Some(bindings), Some(fresh_fulfillment_cache()), None)
            .execute(&command())
            .unwrap();

    assert_eq!(outcome.resolved_edges(), &[edge()]);
}

#[test]
fn test_rules_adding_obligation_marks_artifact_stale() {
    let persisted = derived_obligations(rules_doc(), money_catalogue());
    let bindings = TestBindingsDocument::new(track(), vec![]);
    let rules = rules_doc_with_value_object_rules(vec![
        TestObligationRule::new(
            TestObligationKind::Boundary,
            TestObligationPerAxis::Entry,
            None,
            None,
        ),
        TestObligationRule::new(
            TestObligationKind::Result,
            TestObligationPerAxis::Entry,
            None,
            None,
        ),
    ]);

    let result = interactor_with_rules(Some(persisted), Some(bindings), None, None, rules)
        .execute(&command());

    assert!(matches!(
        result,
        Err(ObligationCheckError::StaleObligationsArtifact { detail })
            if detail.as_str().contains("added=1")
    ));
}

#[test]
fn test_removed_obligation_marks_artifact_stale() {
    let persisted = derived_obligations(rules_doc(), money_catalogue());
    let bindings = TestBindingsDocument::new(track(), vec![fulfillment_binding()]);

    let result =
        interactor_with_rules(Some(persisted), Some(bindings), None, None, empty_rules_doc())
            .execute(&command());

    assert!(matches!(
        result,
        Err(ObligationCheckError::StaleObligationsArtifact { detail })
            if detail.as_str().contains("removed=1")
    ));
}

#[test]
fn test_changed_obligation_declaration_hash_marks_artifact_stale() {
    let expected = obligation();
    let stale = TestObligation::new(
        expected.id().clone(),
        expected.target_entry().clone(),
        expected.target_role().clone(),
        expected.brief().clone(),
        DeclarationHash::new(ContentHash::from_bytes([7u8; 32])),
        expected.spec_refs().to_vec(),
    );
    let bindings = TestBindingsDocument::new(track(), vec![fulfillment_binding_for(&stale)]);

    let result = interactor(
        Some(ObligationsDocument::new(track(), vec![stale])),
        Some(bindings),
        None,
        None,
    )
    .execute(&command());

    assert!(matches!(
        result,
        Err(ObligationCheckError::StaleObligationsArtifact { detail })
            if detail.as_str().contains("changed=1")
    ));
}

#[test]
fn test_check_reads_bindings_without_mutating_the_artifact() {
    let obligations = ObligationsDocument::new(track(), vec![obligation()]);
    let bindings = TestBindingsDocument::new(track(), vec![fulfillment_binding()]);
    let bindings_port = Arc::new(ReadOnlyBindings::new(bindings));
    let interactor = CheckTestObligationsInteractor::new(
        Arc::new(StubRules::valid()),
        Arc::new(StubObligations(Some(obligations))),
        bindings_port.clone(),
        Arc::new(StubScanner),
        Arc::new(StubFulfillmentCache(Some(fresh_fulfillment_cache()))),
        Arc::new(StubWaiverCache(None)),
        fulfillment_verifier_fingerprint(),
        waiver_verifier_fingerprint(),
        Arc::new(StubSpec(spec_doc())),
        Arc::new(StubCatalogue(money_catalogue())),
        task_contract_reader(),
        impl_plan_reader(),
    );

    let outcome = interactor.execute(&command()).unwrap();

    assert_eq!(outcome.resolved_edges(), &[edge()]);
    assert_eq!(bindings_port.save_calls(), 0);
}

#[test]
fn test_mismatched_fulfillment_fingerprint_is_a_missing_stale_verdict() {
    struct CountingScanner(AtomicUsize);
    impl TestSourceScannerPort for CountingScanner {
        fn scan_test_body(
            &self,
            _location: &TestLocation,
        ) -> Result<Option<String>, TestSourceScanError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(Some(BODY.to_owned()))
        }

        fn hash_test_body(&self, _source: &str) -> TestBodySpanHash {
            TestBodySpanHash::new(ContentHash::from_bytes([0u8; 32]))
        }
    }

    let obligations = ObligationsDocument::new(track(), vec![obligation()]);
    let bindings = TestBindingsDocument::new(track(), vec![fulfillment_binding()]);
    let cache = fresh_fulfillment_cache_for_fingerprint(
        &obligation(),
        Some(VerifierPromptFingerprint::new(ContentHash::from_bytes([7u8; 32]))),
    );
    let scanner = Arc::new(CountingScanner(AtomicUsize::new(0)));

    let result = interactor_with_scanner(
        Some(obligations),
        Some(bindings),
        Some(cache),
        None,
        scanner.clone(),
    )
    .execute(&command());

    assert!(matches!(result, Err(ObligationCheckError::StaleVerdicts { .. })));
    assert_eq!(scanner.0.load(Ordering::SeqCst), 1);
}

#[test]
fn test_absent_fulfillment_fingerprint_is_a_missing_stale_verdict() {
    let obligations = ObligationsDocument::new(track(), vec![obligation()]);
    let bindings = TestBindingsDocument::new(track(), vec![fulfillment_binding()]);
    let cache = fresh_fulfillment_cache_for_fingerprint(&obligation(), None);

    let result =
        interactor(Some(obligations), Some(bindings), Some(cache), None).execute(&command());

    assert!(matches!(result, Err(ObligationCheckError::StaleVerdicts { .. })));
}

#[test]
fn test_mismatched_waiver_fingerprint_is_a_missing_stale_verdict() {
    let obligations = ObligationsDocument::new(track(), vec![obligation()]);
    let bindings = TestBindingsDocument::new(track(), vec![waiver_binding()]);
    let cache = fresh_waiver_cache_for_fingerprint(
        &obligation(),
        Some(VerifierPromptFingerprint::new(ContentHash::from_bytes([7u8; 32]))),
    );

    let result =
        interactor(Some(obligations), Some(bindings), None, Some(cache)).execute(&command());

    assert!(matches!(result, Err(ObligationCheckError::StaleVerdicts { .. })));
}

#[test]
fn test_absent_waiver_fingerprint_is_a_missing_stale_verdict() {
    let obligations = ObligationsDocument::new(track(), vec![obligation()]);
    let bindings = TestBindingsDocument::new(track(), vec![waiver_binding()]);
    let cache = fresh_waiver_cache_for_fingerprint(&obligation(), None);

    let result =
        interactor(Some(obligations), Some(bindings), None, Some(cache)).execute(&command());

    assert!(matches!(result, Err(ObligationCheckError::StaleVerdicts { .. })));
}

#[test]
fn test_voluntary_binding_for_derived_edge_returns_consistency_finding() {
    let obligations = ObligationsDocument::new(track(), vec![obligation()]);
    let bindings = TestBindingsDocument::new(track(), vec![voluntary_binding()]);
    let result =
        interactor(Some(obligations), Some(bindings), Some(fresh_fulfillment_cache()), None)
            .execute(&command());

    assert!(matches!(
        result,
        Err(ObligationCheckError::BindingConsistency(
            domain::tddd::test_obligation::errors::TestBindingConsistencyError::VoluntaryBindingOwnsDerivedObligation { edge_id }
        )) if edge_id == edge()
    ));
}

#[test]
fn test_fulfillment_cache_entry_must_match_obligation_id() {
    let expected = obligation();
    let wrong_obligation_id = obligation_with_item("invariant:first");
    let obligations = ObligationsDocument::new(track(), vec![expected.clone()]);
    let bindings = TestBindingsDocument::new(track(), vec![fulfillment_binding_for(&expected)]);
    let cache = fresh_fulfillment_cache_for(&wrong_obligation_id);

    let result =
        interactor(Some(obligations), Some(bindings), Some(cache), None).execute(&command());

    assert!(matches!(result, Err(ObligationCheckError::StaleVerdicts { .. })));
}

#[test]
fn test_trait_impl_declaration_hash_resolves_from_impl_decl() {
    let obligation = trait_impl_obligation();
    let obligations = ObligationsDocument::new(track(), vec![obligation.clone()]);
    let bindings = TestBindingsDocument::new(track(), vec![fulfillment_binding_for(&obligation)]);
    let cache = fresh_fulfillment_cache_for_catalogue(
        &obligation,
        &trait_impl_catalogue(),
        Some(fulfillment_verifier_fingerprint()),
    );

    let outcome = interactor_with_rules_and_catalogue(
        Some(obligations),
        Some(bindings),
        Some(cache),
        None,
        trait_impl_rules_doc(),
        trait_impl_catalogue(),
        Arc::new(StubScanner),
    )
    .execute(&command())
    .unwrap();

    assert_eq!(outcome.resolved_edges(), &[edge()]);
}

#[test]
fn test_trait_impl_waiver_declaration_hash_resolves_from_impl_decl() {
    let obligation = trait_impl_obligation();
    let obligations = ObligationsDocument::new(track(), vec![obligation.clone()]);
    let bindings = TestBindingsDocument::new(track(), vec![waiver_binding()]);
    let cache = fresh_waiver_cache_for_catalogue(
        &obligation,
        &trait_impl_catalogue(),
        Some(waiver_verifier_fingerprint()),
    );

    let outcome = interactor_with_rules_and_catalogue(
        Some(obligations),
        Some(bindings),
        None,
        Some(cache),
        trait_impl_rules_doc(),
        trait_impl_catalogue(),
        Arc::new(StubScanner),
    )
    .execute(&command())
    .unwrap();

    assert_eq!(outcome.resolved_edges(), &[edge()]);
}

#[test]
fn test_waiver_cache_matches_each_owner_when_shared_edge_has_different_declarations() {
    let value_owner = obligation();
    let trait_impl_owner = trait_impl_obligation();
    let catalogue = trait_impl_catalogue();
    let cache_entries = [value_owner.clone(), trait_impl_owner.clone()]
        .into_iter()
        .map(|owner| {
            fresh_waiver_cache_for_catalogue(
                &owner,
                &catalogue,
                Some(waiver_verifier_fingerprint()),
            )
            .entries()[0]
                .clone()
        })
        .collect();
    let cache = WaiverCacheDocument::new(track(), cache_entries);
    let obligations = ObligationsDocument::new(track(), vec![value_owner, trait_impl_owner]);
    let bindings = TestBindingsDocument::new(track(), vec![waiver_binding()]);

    let outcome = interactor_with_rules_and_catalogue(
        Some(obligations),
        Some(bindings),
        None,
        Some(cache),
        shared_edge_waiver_rules_doc(),
        catalogue,
        Arc::new(StubScanner),
    )
    .execute(&command())
    .unwrap();

    assert_eq!(outcome.resolved_edges().len(), 2, "both waiver owners must resolve");
}

#[test]
fn test_declaration_change_stales_verdict_as_drift() {
    // AC-04 / CN-04: a decl-hash mismatch surfaces `decl_changed` freshness drift.
    let obligations = ObligationsDocument::new(track(), vec![obligation()]);
    let bindings = TestBindingsDocument::new(track(), vec![fulfillment_binding()]);
    // A cache entry whose declaration_hash does not match the current catalogue.
    let bound = BoundTestsSetHash::new(sha256_content_hash(format!("{BODY}\n").as_bytes()));
    let stale_decl = DeclarationHash::new(sha256_content_hash(b"an older declaration"));
    let anchor_hash = AnchorTextHash::new(sha256_content_hash(b"Money positive"));
    let entry = cache_entry(
        edge(),
        obligation().id().clone(),
        ObligationFulfillmentCacheKey::new(bound, stale_decl, anchor_hash),
        ObligationFulfillmentVerdict::Fulfilled {
            citation: EvidenceCitation::try_new("asserts positivity".to_owned()).unwrap(),
        },
        Some(fulfillment_verifier_fingerprint()),
    );
    let cache = ObligationFulfillmentCacheDocument::new(track(), vec![entry]);
    let result =
        interactor(Some(obligations), Some(bindings), Some(cache), None).execute(&command());
    assert!(matches!(result, Err(ObligationCheckError::DriftsDetected { .. })));
}

#[test]
fn test_check_reports_bound_test_or_anchor_hash_changes_as_freshness_drift() {
    // IN-08 / AC-04: `check` remains pure-read and refuses a frozen verdict
    // whenever either the bound-test claim or anchor evidence no longer matches.
    let fresh = fresh_fulfillment_cache();
    let entry = fresh.entries().first().unwrap();
    let current_declaration = entry.key().declaration_hash().clone();
    let current_anchor = entry.key().anchor_text_hash().clone();
    let current_bound_tests = entry.key().bound_tests_set_hash().clone();

    for key in [
        ObligationFulfillmentCacheKey::new(
            BoundTestsSetHash::new(ContentHash::from_bytes([1u8; 32])),
            current_declaration.clone(),
            current_anchor.clone(),
        ),
        ObligationFulfillmentCacheKey::new(
            current_bound_tests.clone(),
            current_declaration.clone(),
            AnchorTextHash::new(ContentHash::from_bytes([2u8; 32])),
        ),
    ] {
        let cache = ObligationFulfillmentCacheDocument::new(
            track(),
            vec![cache_entry(
                edge(),
                obligation().id().clone(),
                key,
                ObligationFulfillmentVerdict::Fulfilled {
                    citation: EvidenceCitation::try_new("asserts positivity".to_owned()).unwrap(),
                },
                Some(fulfillment_verifier_fingerprint()),
            )],
        );
        let result = interactor(
            Some(ObligationsDocument::new(track(), vec![obligation()])),
            Some(TestBindingsDocument::new(track(), vec![fulfillment_binding()])),
            Some(cache),
            None,
        )
        .execute(&command());

        assert!(matches!(result, Err(ObligationCheckError::DriftsDetected { .. })));
    }
}

#[test]
fn test_bound_edge_without_verdict_is_stale() {
    // A valid bound edge with no frozen verdict is stale (no fresh pass).
    let obligations = ObligationsDocument::new(track(), vec![obligation()]);
    let bindings = TestBindingsDocument::new(track(), vec![fulfillment_binding()]);
    let empty_cache = ObligationFulfillmentCacheDocument::new(track(), vec![]);
    let result =
        interactor(Some(obligations), Some(bindings), Some(empty_cache), None).execute(&command());
    assert!(matches!(result, Err(ObligationCheckError::StaleVerdicts { .. })));
}

#[test]
fn test_check_absent_fulfillment_cache_requires_evaluation() {
    let result = interactor_with_legacy_fulfillment_cache(
        Some(ObligationsDocument::new(track(), vec![obligation()])),
        Some(TestBindingsDocument::new(track(), vec![fulfillment_binding()])),
    )
    .execute(&command());

    assert!(matches!(result, Err(ObligationCheckError::FulfillmentCacheRequiresEvaluation)));
}

#[test]
fn test_stale_cache_check_evaluate_check_recovery_succeeds_without_manual_mutation() {
    let cache = Arc::new(RecoverableFulfillmentCache::stale());
    let initial_check =
        interactor_with_recoverable_fulfillment_cache(Arc::clone(&cache)).execute(&command());
    assert!(matches!(initial_check, Err(ObligationCheckError::StaleVerdicts { .. })));

    let evaluator = EvaluateTestObligationsInteractor::new(
        Arc::new(StubObligations(Some(ObligationsDocument::new(track(), vec![obligation()])))),
        Arc::new(StubBindings(Some(TestBindingsDocument::new(
            track(),
            vec![fulfillment_binding()],
        )))),
        Arc::new(StubScanner),
        Arc::new(AlwaysFulfilledDriver),
        Arc::new(AlwaysWaivedDriver),
        Arc::clone(&cache) as Arc<dyn ObligationFulfillmentCachePort + Send + Sync>,
        Arc::new(StubWaiverCache(None)),
        fulfillment_verifier_fingerprint(),
        waiver_verifier_fingerprint(),
        TestObligationEvaluateConfig::try_new(0, 90, 1).unwrap(),
        Arc::new(StubSpec(spec_doc())),
        Arc::new(StubCatalogue(money_catalogue())),
        Arc::new(Sha256Hasher),
    );
    let evaluation = run(evaluator.execute(&EvaluateTestObligationsCommand::new(
        track(),
        "track/my-track".to_owned(),
        vec![PathBuf::from("domain-types.json")],
        PathBuf::from("track/items/my-track/spec.json"),
    )))
    .unwrap();
    assert_eq!(evaluation.pass_count(), 1);
    assert!(
        cache.saved.lock().unwrap().is_some(),
        "evaluate must persist a replacement fulfillment cache for the next check"
    );

    let recovered_check =
        interactor_with_recoverable_fulfillment_cache(cache).execute(&command()).unwrap();
    assert_eq!(recovered_check.resolved_edges(), &[edge()]);
}

#[test]
fn test_check_with_historical_fulfillment_row_uses_current_full_key() {
    let current = fresh_fulfillment_cache().entries()[0].clone();
    let historical = cache_entry(
        edge(),
        obligation().id().clone(),
        ObligationFulfillmentCacheKey::new(
            BoundTestsSetHash::new(ContentHash::from_bytes([7u8; 32])),
            current.key().declaration_hash().clone(),
            current.key().anchor_text_hash().clone(),
        ),
        ObligationFulfillmentVerdict::Fulfilled {
            citation: EvidenceCitation::try_new("historical cite".to_owned()).unwrap(),
        },
        Some(fulfillment_verifier_fingerprint()),
    );
    for entries in
        [vec![historical.clone(), current.clone()], vec![current.clone(), historical.clone()]]
    {
        let cache = ObligationFulfillmentCacheDocument::new(track(), entries);
        let obligations = ObligationsDocument::new(track(), vec![obligation()]);
        let bindings = TestBindingsDocument::new(track(), vec![fulfillment_binding()]);

        let outcome = interactor(Some(obligations), Some(bindings), Some(cache), None)
            .execute(&command())
            .unwrap();

        assert_eq!(outcome.resolved_edges(), &[edge()]);
    }
}

#[test]
fn test_check_aggregates_historical_fulfillment_key_drifts_independent_of_row_order() {
    let current = fresh_fulfillment_cache().entries()[0].clone();
    let test_changed = cache_entry(
        edge(),
        obligation().id().clone(),
        ObligationFulfillmentCacheKey::new(
            BoundTestsSetHash::new(ContentHash::from_bytes([7u8; 32])),
            current.key().declaration_hash().clone(),
            current.key().anchor_text_hash().clone(),
        ),
        ObligationFulfillmentVerdict::Pending,
        Some(fulfillment_verifier_fingerprint()),
    );
    let declaration_changed = cache_entry(
        edge(),
        obligation().id().clone(),
        ObligationFulfillmentCacheKey::new(
            current.key().bound_tests_set_hash().clone(),
            DeclarationHash::new(ContentHash::from_bytes([7u8; 32])),
            current.key().anchor_text_hash().clone(),
        ),
        ObligationFulfillmentVerdict::Pending,
        Some(fulfillment_verifier_fingerprint()),
    );
    let expected = vec![
        TestObligationDrift::test_changed_edge(
            edge(),
            DiagnosticMessage::try_new(
                "bound test bodies changed since the verdict was frozen".to_owned(),
            )
            .unwrap(),
        ),
        TestObligationDrift::decl_changed_edge(
            edge(),
            DiagnosticMessage::try_new(
                "entry declaration changed since the verdict was frozen".to_owned(),
            )
            .unwrap(),
        ),
    ];

    for entries in [
        vec![test_changed.clone(), declaration_changed.clone()],
        vec![declaration_changed.clone(), test_changed.clone()],
    ] {
        let result = interactor(
            Some(ObligationsDocument::new(track(), vec![obligation()])),
            Some(TestBindingsDocument::new(track(), vec![fulfillment_binding()])),
            Some(ObligationFulfillmentCacheDocument::new(track(), entries)),
            None,
        )
        .execute(&command());

        let Err(ObligationCheckError::DriftsDetected { drifts }) = result else {
            panic!("expected deterministic freshness drifts");
        };
        assert_eq!(drifts.as_slice(), expected.as_slice());
    }
}

#[test]
fn test_check_with_duplicate_current_fulfillment_rows_returns_lookup_error() {
    let fulfilled = fresh_fulfillment_cache().entries()[0].clone();
    let expected_key = fulfilled.key().clone();
    let pending = cache_entry(
        edge(),
        obligation().id().clone(),
        expected_key.clone(),
        ObligationFulfillmentVerdict::Pending,
        Some(fulfillment_verifier_fingerprint()),
    );

    for entries in
        [vec![fulfilled.clone(), pending.clone()], vec![pending.clone(), fulfilled.clone()]]
    {
        let cache = ObligationFulfillmentCacheDocument::new(track(), entries);
        let obligations = ObligationsDocument::new(track(), vec![obligation()]);
        let bindings = TestBindingsDocument::new(track(), vec![fulfillment_binding()]);
        let result =
            interactor(Some(obligations), Some(bindings), Some(cache), None).execute(&command());

        match result {
            Err(ObligationCheckError::FulfillmentCacheLookup(
                FulfillmentCacheLookupError::AmbiguousCurrentEntries {
                    edge_id: actual_edge_id,
                    obligation_id: actual_obligation_id,
                    key: actual_key,
                },
            )) => {
                assert_eq!(actual_edge_id, edge());
                assert_eq!(actual_obligation_id, obligation().id().clone());
                assert_eq!(actual_key, expected_key);
            }
            other => {
                panic!("expected ambiguity error with the complete cache identity, got {other:?}")
            }
        }
    }
}

#[test]
fn test_missing_bound_test_without_cached_verdict_is_missing_drift() {
    struct MissingTestScanner;
    impl TestSourceScannerPort for MissingTestScanner {
        fn scan_test_body(
            &self,
            _location: &TestLocation,
        ) -> Result<Option<String>, TestSourceScanError> {
            Ok(None)
        }

        fn hash_test_body(&self, _source: &str) -> TestBodySpanHash {
            TestBodySpanHash::new(ContentHash::from_bytes([0u8; 32]))
        }
    }

    let obligation = obligation();
    let bindings = TestBindingsDocument::new(
        track(),
        vec![TestBindingRecord::Fulfillment {
            obligation_id: obligation.id().clone(),
            tests: NonEmptyTestLocations::try_new(vec![missing_location()]).unwrap(),
        }],
    );
    let result = interactor_with_scanner(
        Some(ObligationsDocument::new(track(), vec![obligation.clone()])),
        Some(bindings),
        None,
        None,
        Arc::new(MissingTestScanner),
    )
    .execute(&command());

    let expected = TestObligationDrift::missing_obligation(
        obligation.id().clone(),
        DiagnosticMessage::try_new("bound test source not found".to_owned()).unwrap(),
    );
    match result {
        Err(ObligationCheckError::DriftsDetected { drifts }) => {
            assert_eq!(drifts.as_slice(), &[expected]);
        }
        other => panic!("expected missing-test drift, got {other:?}"),
    }
}

#[test]
fn test_missing_bound_test_with_fresh_cache_respects_final_status_lane() {
    struct MissingTestScanner;
    impl TestSourceScannerPort for MissingTestScanner {
        fn scan_test_body(
            &self,
            _location: &TestLocation,
        ) -> Result<Option<String>, TestSourceScanError> {
            Ok(None)
        }

        fn hash_test_body(&self, _source: &str) -> TestBodySpanHash {
            TestBodySpanHash::new(ContentHash::from_bytes([0u8; 32]))
        }
    }

    let obligations = ObligationsDocument::new(track(), vec![obligation()]);
    let bindings = TestBindingsDocument::new(track(), vec![fulfillment_binding()]);

    let todo = interactor_with_scanner_and_task_statuses(
        Some(obligations.clone()),
        Some(bindings.clone()),
        Some(fresh_fulfillment_cache()),
        None,
        Arc::new(MissingTestScanner),
        &[("T001", TaskStatusKind::Todo)],
    )
    .execute(&command())
    .unwrap();
    let todo_summary = todo
        .status_lane_summaries()
        .iter()
        .find(|summary| summary.task_status() == TaskStatusKind::Todo)
        .unwrap();
    assert_eq!(todo_summary.missing_count(), 1);
    assert_eq!(todo_summary.stale_count(), 0);
    assert_eq!(todo_summary.verdict_absent_count(), 0);

    let strict = interactor_with_scanner_and_task_statuses(
        Some(obligations),
        Some(bindings),
        Some(fresh_fulfillment_cache()),
        None,
        Arc::new(MissingTestScanner),
        &[("T001", TaskStatusKind::InProgress)],
    )
    .execute(&command());
    assert!(matches!(strict, Err(ObligationCheckError::DriftsDetected { .. })));
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

    let obligations = ObligationsDocument::new(track(), vec![obligation()]);
    let bindings = TestBindingsDocument::new(track(), vec![fulfillment_binding()]);
    let result = interactor_with_scanner(
        Some(obligations),
        Some(bindings),
        Some(fresh_fulfillment_cache()),
        None,
        Arc::new(FailingScanner),
    )
    .execute(&command());

    assert!(matches!(result, Err(ObligationCheckError::SourceScan(TestSourceScanError::Io(_)))));
}

#[test]
fn test_missing_bound_test_source_with_fresh_cache_is_missing_drift() {
    struct MissingScanner;
    impl TestSourceScannerPort for MissingScanner {
        fn scan_test_body(&self, _l: &TestLocation) -> Result<Option<String>, TestSourceScanError> {
            Ok(None)
        }

        fn hash_test_body(&self, _s: &str) -> TestBodySpanHash {
            TestBodySpanHash::new(ContentHash::from_bytes([0u8; 32]))
        }
    }

    let obligations = ObligationsDocument::new(track(), vec![obligation()]);
    let bindings = TestBindingsDocument::new(track(), vec![fulfillment_binding()]);
    let result = interactor_with_scanner(
        Some(obligations),
        Some(bindings),
        Some(fresh_fulfillment_cache()),
        None,
        Arc::new(MissingScanner),
    )
    .execute(&command());

    assert!(matches!(result, Err(ObligationCheckError::DriftsDetected { .. })));
}

#[test]
fn test_runs_regardless_of_branch() {
    // `check` is pure-read (IN-08) and has no active-branch guard, unlike the
    // write-side derive / evaluate commands: a catalogue-free empty scope passes
    // on any branch.
    let cmd = CheckTestObligationsCommand::new(TestObligationCatalogueCommandInput::new(
        track(),
        "main".to_owned(),
        Vec::new(),
    ));
    let outcome = interactor(None, None, None, None).execute(&cmd).unwrap();
    assert!(outcome.resolved_edges().is_empty());
}

#[test]
fn test_rules_load_failure_fails_closed_before_any_stage() {
    // IN-08: a malformed / role-incomplete rules config aborts `check` up front
    // so a stale obligations / bindings / cache set can never silently pass.
    // The downstream stages must not be reached — the fail-closed load runs
    // before the artifact-existence scope resolution and any drift / totality
    // / freshness gate below.
    let result = interactor_with_rules_loader(Arc::new(FailingRules)).execute(&command());
    assert!(matches!(
        result,
        Err(ObligationCheckError::RulesLoad(TestObligationRulesLoadError::RoleNotCovered { .. }))
    ));
}
