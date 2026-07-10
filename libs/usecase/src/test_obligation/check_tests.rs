//! Unit tests for [`super::CheckTestObligationsInteractor`] (T017).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use domain::tddd::LayerId;
use domain::tddd::catalogue_v2::catalogue_impl_signals_ports::{
    CatalogueDocumentLoaderError, CatalogueDocumentLoaderPort,
};
use domain::tddd::catalogue_v2::roles::{ContractRole, DataRole, FunctionRole, ItemAction};
use domain::tddd::catalogue_v2::{
    CatalogueDocument, CrateName, DeletionRecord, ModulePath, StructKind, StructShape, TraitEntry,
    TraitImplDeclV2, TraitName, TypeEntry, TypeKindV2, TypeName, TypeRef,
};
use domain::tddd::semantic_verify::{
    CatalogueEntryKey, CatalogueEntryRef, CatalogueSectionKey, SpecSectionKind,
};
use domain::tddd::test_obligation::binding::{
    NonEmptyTestLocations, TestBindingRecord, TestBindingsDocument, TestLocation,
};
use domain::tddd::test_obligation::errors::{
    ArtifactCodecError, ObligationCheckError, TestObligationRulesLoadError, TestSourceScanError,
    VerifyCacheError,
};
use domain::tddd::test_obligation::hashes::{
    AnchorTextHash, BoundTestsSetHash, DeclarationHash, TestBodySpanHash, WaivedReasonHash,
};
use domain::tddd::test_obligation::ids::{
    DiagnosticMessage, RoleName, TestFunctionName, TestModulePath, TestObligationAnchorId,
    TestObligationBrief, TestObligationEdgeId, TestObligationId, TestObligationItemIdentifier,
    WaivedReason,
};
use domain::tddd::test_obligation::obligations::{ObligationsDocument, TestObligation};
use domain::tddd::test_obligation::ports::{
    ObligationFulfillmentCachePort, ObligationsArtifactPort, TestBindingsArtifactPort,
    TestObligationRulesLoaderPort, TestSourceScannerPort, WaiverCachePort,
};
use domain::tddd::test_obligation::rules::{RoleObligationRules, TestObligationRulesDocument};
use domain::tddd::test_obligation::scope::UncitedSpecElementFinding;
use domain::tddd::test_obligation::verdict::{
    ObligationFulfillmentCacheDocument, ObligationFulfillmentCacheEntry,
    ObligationFulfillmentCacheKey, ObligationFulfillmentVerdict, WaiverCacheDocument,
    WaiverCacheEntry, WaiverCacheKey, WaiverVerdict,
};
use domain::tddd::test_obligation::vocab::{
    TargetEntryRoleKind, TestObligationKind, TestObligationPatternKind,
};
use domain::{
    ContentHash, EvidenceCitation, SpecDocument, SpecDocumentLoadError, SpecElementId, SpecRef,
    SpecRequirement, SpecScope, TrackId,
};

use domain::SpecDocumentLoaderPort;

use super::{
    CheckTestObligationsApplicationService, CheckTestObligationsCommand,
    CheckTestObligationsInteractor,
};
use crate::test_obligation::{
    LoadedCatalogueDocument, TestObligationCatalogueCommandInput, obligation_declaration_text,
    obligation_declaration_text_from_loaded, sha256_content_hash,
};

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
impl ObligationFulfillmentCachePort for StubFulfillmentCache {
    fn load(
        &self,
        _t: &TrackId,
    ) -> Result<Option<ObligationFulfillmentCacheDocument>, VerifyCacheError> {
        Ok(self.0.clone())
    }
    fn save(&self, _d: &ObligationFulfillmentCacheDocument) -> Result<(), DiagnosticMessage> {
        Ok(())
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

struct FailingCatalogue;
impl CatalogueDocumentLoaderPort for FailingCatalogue {
    fn load(&self, path: &Path) -> Result<CatalogueDocument, CatalogueDocumentLoaderError> {
        Err(CatalogueDocumentLoaderError::NotFound { path: path.to_path_buf() })
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

/// Minimal valid rules document: every role covered with an explicitly empty
/// rule list. The check gate's rules-load contract is load-and-validate only
/// (IN-08); no downstream lane inspects the rules content.
fn rules_doc() -> TestObligationRulesDocument {
    let empty = || RoleObligationRules::new(vec![]);
    let data_roles: Vec<(DataRole, RoleObligationRules)> =
        DATA_ROLE_NAMES.iter().map(|n| (n.parse::<DataRole>().unwrap(), empty())).collect();
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

fn track() -> TrackId {
    TrackId::try_new("my-track").unwrap()
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
    obligation_with_item("invariant:positive")
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
    TestObligation::new(
        TestObligationId::new(
            entry_key(),
            TestObligationKind::ContractConformance,
            TestObligationItemIdentifier::try_new("trait_impl:MyPort".to_owned()).unwrap(),
        ),
        CatalogueEntryRef::new(
            "domain-types.json".to_owned(),
            CatalogueSectionKey::Traits,
            entry_key(),
        ),
        TargetEntryRoleKind::TraitImpl(ContractRole::SecondaryPort),
        TestObligationBrief::try_new("cover impl".to_owned()).unwrap(),
        DeclarationHash::new(sha256_content_hash(
            obligation_declaration_text(&[money_catalogue()], &trait_impl_obligation_seed())
                .unwrap()
                .as_bytes(),
        )),
        vec![anchor()],
    )
}

fn trait_impl_obligation_seed() -> TestObligation {
    TestObligation::new(
        TestObligationId::new(
            entry_key(),
            TestObligationKind::ContractConformance,
            TestObligationItemIdentifier::try_new("trait_impl:MyPort".to_owned()).unwrap(),
        ),
        CatalogueEntryRef::new(
            "domain-types.json".to_owned(),
            CatalogueSectionKey::Traits,
            entry_key(),
        ),
        TargetEntryRoleKind::TraitImpl(ContractRole::SecondaryPort),
        TestObligationBrief::try_new("cover impl".to_owned()).unwrap(),
        DeclarationHash::new(ContentHash::from_bytes([0u8; 32])),
        vec![anchor()],
    )
}

fn location() -> TestLocation {
    TestLocation::new(
        LayerId::try_new("domain").unwrap(),
        TestModulePath::try_new("domain::money::tests".to_owned()).unwrap(),
        TestFunctionName::try_new("test_positive".to_owned()).unwrap(),
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
        TypeName::new("Money").unwrap(),
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
        name: TypeName::new("OldMoney").unwrap(),
        module_path: ModulePath::root(),
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

fn money_catalogue_with_role(role: DataRole) -> CatalogueDocument {
    let mut doc = CatalogueDocument::new(
        5,
        CrateName::new("domain").unwrap(),
        LayerId::try_new("domain").unwrap(),
    );
    doc.insert_type(
        TypeName::new("Money").unwrap(),
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

fn trait_entry(role: ContractRole) -> TraitEntry {
    TraitEntry::new(
        ItemAction::Add,
        role,
        vec![],
        vec![],
        vec![],
        vec![],
        vec![],
        vec![],
        ModulePath::root(),
        None,
        vec![SpecRef::new(PathBuf::from("spec.json"), SpecElementId::try_new("IN-05").unwrap())],
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
    let bound = BoundTestsSetHash::new(sha256_content_hash(format!("{BODY}\n").as_bytes()));
    let decl = DeclarationHash::new(sha256_content_hash(
        obligation_declaration_text(&[money_catalogue()], obligation).unwrap().as_bytes(),
    ));
    let anchor_hash = AnchorTextHash::new(sha256_content_hash(b"Money positive"));
    let entry = ObligationFulfillmentCacheEntry::new(
        edge(),
        obligation.id().clone(),
        ObligationFulfillmentCacheKey::new(bound, decl, anchor_hash),
        ObligationFulfillmentVerdict::Fulfilled {
            citation: EvidenceCitation::try_new("asserts positivity".to_owned()).unwrap(),
        },
    );
    ObligationFulfillmentCacheDocument::new(track(), vec![entry])
}

fn fresh_waiver_cache_for(obligation: &TestObligation) -> WaiverCacheDocument {
    let reason = WaivedReason::try_new("covered elsewhere".to_owned()).unwrap();
    let reason_hash = WaivedReasonHash::new(sha256_content_hash(reason.as_str().as_bytes()));
    let decl = DeclarationHash::new(sha256_content_hash(
        obligation_declaration_text(&[money_catalogue()], obligation).unwrap().as_bytes(),
    ));
    let anchor_hash = AnchorTextHash::new(sha256_content_hash(b"Money positive"));
    let entry = WaiverCacheEntry::new(
        edge(),
        WaiverCacheKey::new(reason_hash, decl, anchor_hash),
        WaiverVerdict::Waived {
            citation: EvidenceCitation::try_new("waived by policy".to_owned()).unwrap(),
        },
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
    CheckTestObligationsInteractor::new(
        Arc::new(StubRules::valid()),
        Arc::new(StubObligations(obligations)),
        Arc::new(StubBindings(bindings)),
        scanner,
        Arc::new(StubFulfillmentCache(fulfillment)),
        Arc::new(StubWaiverCache(waiver)),
        Arc::new(StubSpec(spec_doc())),
        Arc::new(StubCatalogue(money_catalogue())),
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
        Arc::new(StubSpec(spec_doc())),
        Arc::new(StubCatalogue(money_catalogue())),
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
        Arc::new(FailingSpec),
        Arc::new(FailingCatalogue),
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
fn test_empty_scope_passes_without_materialized_catalogue_or_spec() {
    // AC-10 / IN-14: both artifacts absent → zero pairs pass by artifact
    // existence alone, even before spec/catalogues are materialized.
    let outcome = empty_scope_interactor_without_readers().execute(&command()).unwrap();
    assert!(outcome.resolved_edges().is_empty());
    assert!(outcome.uncited_findings().is_empty());
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
fn test_obligation_declaration_text_uses_target_section_for_same_key() {
    let mut catalogue = money_catalogue();
    catalogue
        .insert_trait(TraitName::new("Money").unwrap(), trait_entry(ContractRole::SecondaryPort));
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
fn test_obligation_declaration_text_uses_recorded_catalogue_path() {
    let entry_key = entry_key();
    let obligation = TestObligation::new(
        TestObligationId::new(
            entry_key.clone(),
            TestObligationKind::Boundary,
            TestObligationItemIdentifier::try_new("invariant:positive".to_owned()).unwrap(),
        ),
        CatalogueEntryRef::new(
            "usecase-types.json".to_owned(),
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
            Path::new("domain-types.json"),
            money_catalogue_with_role(DataRole::entity().unwrap()),
        ),
        LoadedCatalogueDocument::new(
            Path::new("usecase-types.json"),
            money_catalogue_with_role(DataRole::value_object()),
        ),
    ];

    let declaration = obligation_declaration_text_from_loaded(&catalogues, &obligation).unwrap();

    assert!(declaration.contains("ValueObject"));
    assert!(!declaration.contains("Entity"));
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
    let result = interactor(Some(obligations), Some(bindings), None, None).execute(&command());
    assert!(matches!(result, Err(ObligationCheckError::DriftsDetected { .. })));
}

#[test]
fn test_orphaned_waiver_binding_is_a_drift() {
    // IN-13 / CN-11: a Waiver binding for a no-longer-derived edge is `orphaned`.
    let obligations = ObligationsDocument::new(track(), vec![]);
    let bindings = TestBindingsDocument::new(track(), vec![waiver_binding_for(unknown_edge())]);
    let result = interactor(Some(obligations), Some(bindings), None, None).execute(&command());
    assert!(matches!(result, Err(ObligationCheckError::DriftsDetected { .. })));
}

#[test]
fn test_zero_obligation_cited_edge_without_binding_is_unresolved() {
    // CN-02 / AC-04: a cited active entry edge remains in the totality universe
    // even when the decision table derives zero obligations for it.
    let obligations = ObligationsDocument::new(track(), vec![]);
    let bindings = TestBindingsDocument::new(track(), vec![]);
    let result = interactor(Some(obligations), Some(bindings), None, None).execute(&command());
    assert!(matches!(result, Err(ObligationCheckError::UnresolvedEdges { .. })));
}

#[test]
fn test_zero_obligation_cited_edge_waiver_resolves() {
    // D4 / CN-02: a zero-obligation edge is resolved by a fresh waived verdict.
    let obligations = ObligationsDocument::new(track(), vec![]);
    let bindings = TestBindingsDocument::new(track(), vec![waiver_binding()]);
    let cache = fresh_waiver_cache_for(&obligation());

    let outcome = interactor(Some(obligations), Some(bindings), None, Some(cache))
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
fn test_voluntary_binding_migrates_to_newly_derived_obligation() {
    // AC-12: a prior edge-level binding becomes fulfillment for the derived
    // obligation once the decision table starts producing one for that edge.
    let obligations = ObligationsDocument::new(track(), vec![obligation()]);
    let bindings = TestBindingsDocument::new(track(), vec![voluntary_binding()]);
    let outcome =
        interactor(Some(obligations), Some(bindings), Some(fresh_fulfillment_cache()), None)
            .execute(&command())
            .unwrap();

    assert_eq!(outcome.resolved_edges(), &[edge()]);
}

#[test]
fn test_fulfillment_cache_entry_must_match_obligation_id() {
    let first = obligation_with_item("invariant:first");
    let second = obligation_with_item("invariant:second");
    let obligations = ObligationsDocument::new(track(), vec![first.clone(), second.clone()]);
    let bindings = TestBindingsDocument::new(
        track(),
        vec![fulfillment_binding_for(&first), fulfillment_binding_for(&second)],
    );
    let cache = fresh_fulfillment_cache_for(&first);

    let result =
        interactor(Some(obligations), Some(bindings), Some(cache), None).execute(&command());

    assert!(matches!(result, Err(ObligationCheckError::StaleVerdicts { .. })));
}

#[test]
fn test_trait_impl_declaration_hash_resolves_from_impl_decl() {
    let obligation = trait_impl_obligation();
    let obligations = ObligationsDocument::new(track(), vec![obligation.clone()]);
    let bindings = TestBindingsDocument::new(track(), vec![fulfillment_binding_for(&obligation)]);
    let cache = fresh_fulfillment_cache_for(&obligation);

    let outcome = interactor(Some(obligations), Some(bindings), Some(cache), None)
        .execute(&command())
        .unwrap();

    assert_eq!(outcome.resolved_edges(), &[edge()]);
}

#[test]
fn test_trait_impl_waiver_declaration_hash_resolves_from_impl_decl() {
    let obligation = trait_impl_obligation();
    let obligations = ObligationsDocument::new(track(), vec![obligation.clone()]);
    let bindings = TestBindingsDocument::new(track(), vec![waiver_binding()]);
    let cache = fresh_waiver_cache_for(&obligation);

    let outcome = interactor(Some(obligations), Some(bindings), None, Some(cache))
        .execute(&command())
        .unwrap();

    assert_eq!(outcome.resolved_edges(), &[edge()]);
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
    let entry = ObligationFulfillmentCacheEntry::new(
        edge(),
        obligation().id().clone(),
        ObligationFulfillmentCacheKey::new(bound, stale_decl, anchor_hash),
        ObligationFulfillmentVerdict::Fulfilled {
            citation: EvidenceCitation::try_new("asserts positivity".to_owned()).unwrap(),
        },
    );
    let cache = ObligationFulfillmentCacheDocument::new(track(), vec![entry]);
    let result =
        interactor(Some(obligations), Some(bindings), Some(cache), None).execute(&command());
    assert!(matches!(result, Err(ObligationCheckError::DriftsDetected { .. })));
}

#[test]
fn test_bound_edge_without_verdict_is_stale() {
    // A bound edge with no frozen verdict is stale (no fresh pass).
    let obligations = ObligationsDocument::new(track(), vec![obligation()]);
    let bindings = TestBindingsDocument::new(track(), vec![fulfillment_binding()]);
    let empty_cache = ObligationFulfillmentCacheDocument::new(track(), vec![]);
    let result =
        interactor(Some(obligations), Some(bindings), Some(empty_cache), None).execute(&command());
    assert!(matches!(result, Err(ObligationCheckError::StaleVerdicts { .. })));
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
fn test_missing_bound_test_source_maps_to_io_absence() {
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

    assert!(matches!(result, Err(ObligationCheckError::SourceScan(TestSourceScanError::Io(_)))));
}

#[test]
fn test_runs_regardless_of_branch() {
    // `check` is pure-read (IN-08) and has no active-branch guard, unlike the
    // write-side derive / evaluate commands: an empty scope passes on any branch.
    let cmd = CheckTestObligationsCommand::new(TestObligationCatalogueCommandInput::new(
        track(),
        "main".to_owned(),
        vec![PathBuf::from("domain-types.json")],
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
