//! Unit tests for [`super::DeriveTestObligationsInteractor`] (T016).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use domain::tddd::LayerId;
use domain::tddd::catalogue_v2::catalogue_impl_signals_ports::{
    CatalogueDocumentLoaderError, CatalogueDocumentLoaderPort, TrackStatusReadError,
    TrackStatusReaderPort,
};
use domain::tddd::catalogue_v2::roles::{
    ContractRole, DataRole, FunctionRole, InvariantDecl, InvariantPredicate, ItemAction,
};
use domain::tddd::catalogue_v2::{
    CatalogueDocument, CrateName, DocString, InherentImplDeclV2, InvariantName, MethodDeclaration,
    MethodName, ModulePath, SelfReceiver, StructKind, StructShape, TraitEntry, TraitImplDeclV2,
    TypeEntry, TypeKindV2, TypeRef,
};
use domain::tddd::semantic_verify::CatalogueEntryKey;
use domain::tddd::test_obligation::errors::{
    ArtifactCodecError, TestObligationRulesLoadError, TrackStatusReadFailureKind,
};
use domain::tddd::test_obligation::ids::{DiagnosticMessage, TestObligationAnchorId};
use domain::tddd::test_obligation::obligations::ObligationsDocument;
use domain::tddd::test_obligation::ports::{
    ObligationsArtifactPort, TestObligationRulesLoaderPort,
};
use domain::tddd::test_obligation::projection::RoleObligationItemsProjector;
use domain::tddd::test_obligation::rules::{
    RoleObligationRules, TestObligationMinimum, TestObligationRule, TestObligationRulesDocument,
};
use domain::tddd::test_obligation::vocab::{
    TargetEntryRoleKind, TestObligationKind, TestObligationPatternKind, TestObligationPerAxis,
};
use domain::{
    FrozenTrackStatus, SpecDocument, SpecDocumentLoadError, SpecElementId, SpecRef, SpecScope,
    TrackId, TrackStatus,
};

use domain::SpecDocumentLoaderPort;

use super::{
    DeriveTestObligationsApplicationService, DeriveTestObligationsCommand,
    DeriveTestObligationsInteractor, validate_named_catalogue_methods,
};
use crate::test_obligation::TestObligationCatalogueCommandInput;
use crate::test_obligation::obligation_declaration_text;

// ---------------------------------------------------------------------------
// Test doubles
// ---------------------------------------------------------------------------

struct StubRules {
    doc: TestObligationRulesDocument,
}

impl TestObligationRulesLoaderPort for StubRules {
    fn load(&self) -> Result<TestObligationRulesDocument, TestObligationRulesLoadError> {
        Ok(self.doc.clone())
    }
}

struct StubCatalogue {
    docs: HashMap<PathBuf, CatalogueDocument>,
    loads: Mutex<Vec<PathBuf>>,
}

impl StubCatalogue {
    fn new(docs: HashMap<PathBuf, CatalogueDocument>) -> Self {
        Self { docs, loads: Mutex::new(Vec::new()) }
    }
}

impl CatalogueDocumentLoaderPort for StubCatalogue {
    fn load(&self, path: &Path) -> Result<CatalogueDocument, CatalogueDocumentLoaderError> {
        self.loads.lock().unwrap().push(path.to_path_buf());
        self.docs
            .get(path)
            .cloned()
            .ok_or_else(|| CatalogueDocumentLoaderError::NotFound { path: path.to_path_buf() })
    }
}

struct StubSpec;

impl SpecDocumentLoaderPort for StubSpec {
    fn load(&self, _spec_path: &Path) -> Result<SpecDocument, SpecDocumentLoadError> {
        // derive only needs the spec to exist as a precondition; a minimal
        // document suffices (anchors come from the catalogue entries' spec_refs).
        Ok(SpecDocument::new(
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
        .unwrap())
    }
}

struct StubTrackStatus {
    result: Result<TrackStatus, TrackStatusReadError>,
}

impl TrackStatusReaderPort for StubTrackStatus {
    fn read_status(
        &self,
        _items_dir: &Path,
        _track_id: &str,
    ) -> Result<TrackStatus, TrackStatusReadError> {
        self.result.clone()
    }
}

#[derive(Default)]
struct RecordingObligations {
    saved: Mutex<Option<ObligationsDocument>>,
}

impl ObligationsArtifactPort for RecordingObligations {
    fn load(&self, _track_id: &TrackId) -> Result<Option<ObligationsDocument>, ArtifactCodecError> {
        Ok(None)
    }

    fn save(&self, doc: &ObligationsDocument) -> Result<(), DiagnosticMessage> {
        *self.saved.lock().unwrap() = Some(doc.clone());
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Fixture builders
// ---------------------------------------------------------------------------

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

const CONTRACT_ROLE_NAMES: &[&str] =
    &["SpecificationPort", "ApplicationService", "SecondaryPort", "Repository"];

fn rule(
    kind: TestObligationKind,
    per: TestObligationPerAxis,
    min: Option<usize>,
) -> TestObligationRule {
    TestObligationRule::new(
        kind,
        per,
        min.map(|m| TestObligationMinimum::try_new(m).unwrap()),
        None,
    )
}

/// A complete rules document: every role covered, with a handful carrying rules.
fn rules_doc() -> TestObligationRulesDocument {
    rules_doc_with_secondary_port_trait_impl_rules(vec![rule(
        TestObligationKind::ContractConformance,
        TestObligationPerAxis::TraitImpl,
        None,
    )])
}

fn rules_doc_with_secondary_port_trait_impl_rules(
    secondary_port_rules: Vec<TestObligationRule>,
) -> TestObligationRulesDocument {
    let data_roles: Vec<(DataRole, RoleObligationRules)> = DATA_ROLE_NAMES
        .iter()
        .map(|name| {
            let role = name.parse::<DataRole>().unwrap();
            let rules = match *name {
                "ValueObject" => RoleObligationRules::new(vec![rule(
                    TestObligationKind::Boundary,
                    TestObligationPerAxis::Invariant,
                    None,
                )]),
                "UseCase" => RoleObligationRules::new(vec![rule(
                    TestObligationKind::Result,
                    TestObligationPerAxis::Handles,
                    Some(1),
                )]),
                _ => RoleObligationRules::new(vec![]),
            };
            (role, rules)
        })
        .collect();

    let contract_roles: Vec<(ContractRole, RoleObligationRules)> = CONTRACT_ROLE_NAMES
        .iter()
        .map(|name| (name.parse::<ContractRole>().unwrap(), RoleObligationRules::new(vec![])))
        .collect();

    let function_roles = vec![
        (FunctionRole::FreeFunction, RoleObligationRules::new(vec![])),
        (FunctionRole::UseCaseFunction, RoleObligationRules::new(vec![])),
    ];

    let patterns = vec![(
        TestObligationPatternKind::Typestate,
        RoleObligationRules::new(vec![rule(
            TestObligationKind::Transition,
            TestObligationPerAxis::Transition,
            None,
        )]),
    )];

    let trait_impls: Vec<(ContractRole, RoleObligationRules)> = CONTRACT_ROLE_NAMES
        .iter()
        .map(|name| {
            let rules = if *name == "SecondaryPort" {
                RoleObligationRules::new(secondary_port_rules.clone())
            } else {
                RoleObligationRules::new(vec![])
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

fn rules_doc_with_secondary_port_contract_rules() -> TestObligationRulesDocument {
    rules_doc_with_secondary_port_axis_rules(TestObligationPerAxis::TraitMethod)
}

fn rules_doc_with_secondary_port_method_rules() -> TestObligationRulesDocument {
    rules_doc_with_secondary_port_axis_rules(TestObligationPerAxis::Method)
}

fn rules_doc_with_secondary_port_rules(
    secondary_port_rules: Vec<TestObligationRule>,
) -> TestObligationRulesDocument {
    let data_roles: Vec<(DataRole, RoleObligationRules)> = DATA_ROLE_NAMES
        .iter()
        .map(|name| (name.parse::<DataRole>().unwrap(), RoleObligationRules::new(vec![])))
        .collect();
    let contract_roles: Vec<(ContractRole, RoleObligationRules)> = CONTRACT_ROLE_NAMES
        .iter()
        .map(|name| {
            let rules = if *name == "SecondaryPort" {
                RoleObligationRules::new(secondary_port_rules.clone())
            } else {
                RoleObligationRules::new(vec![])
            };
            (name.parse::<ContractRole>().unwrap(), rules)
        })
        .collect();
    let function_roles = vec![
        (FunctionRole::FreeFunction, RoleObligationRules::new(vec![])),
        (FunctionRole::UseCaseFunction, RoleObligationRules::new(vec![])),
    ];
    let patterns = vec![(TestObligationPatternKind::Typestate, RoleObligationRules::new(vec![]))];
    let trait_impls: Vec<(ContractRole, RoleObligationRules)> = CONTRACT_ROLE_NAMES
        .iter()
        .map(|name| (name.parse::<ContractRole>().unwrap(), RoleObligationRules::new(vec![])))
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

fn rules_doc_with_secondary_port_axis_rules(
    per_axis: TestObligationPerAxis,
) -> TestObligationRulesDocument {
    rules_doc_with_secondary_port_rules(vec![rule(TestObligationKind::Contract, per_axis, None)])
}

fn rules_doc_with_domain_service_rules(
    domain_service_rules: Vec<TestObligationRule>,
) -> TestObligationRulesDocument {
    let data_roles: Vec<(DataRole, RoleObligationRules)> = DATA_ROLE_NAMES
        .iter()
        .map(|name| {
            let rules = if *name == "DomainService" {
                RoleObligationRules::new(domain_service_rules.clone())
            } else {
                RoleObligationRules::new(vec![])
            };
            (name.parse::<DataRole>().unwrap(), rules)
        })
        .collect();
    let contract_roles: Vec<(ContractRole, RoleObligationRules)> = CONTRACT_ROLE_NAMES
        .iter()
        .map(|name| (name.parse::<ContractRole>().unwrap(), RoleObligationRules::new(vec![])))
        .collect();
    let function_roles = vec![
        (FunctionRole::FreeFunction, RoleObligationRules::new(vec![])),
        (FunctionRole::UseCaseFunction, RoleObligationRules::new(vec![])),
    ];
    let patterns = vec![(TestObligationPatternKind::Typestate, RoleObligationRules::new(vec![]))];
    let trait_impls: Vec<(ContractRole, RoleObligationRules)> = CONTRACT_ROLE_NAMES
        .iter()
        .map(|name| (name.parse::<ContractRole>().unwrap(), RoleObligationRules::new(vec![])))
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

fn domain_service_type(
    action: ItemAction,
    methods: Vec<MethodDeclaration>,
    spec_refs: Vec<SpecRef>,
) -> TypeEntry {
    TypeEntry::new(
        action,
        DataRole::domain_service(),
        TypeKindV2::Struct(StructKind::new(StructShape::Unit, None)),
        methods,
        vec![],
        vec![],
        ModulePath::root(),
        None,
        spec_refs,
        vec![],
    )
}

fn invariant(name: &str) -> InvariantDecl {
    InvariantDecl::new(
        InvariantName::new(name).unwrap(),
        InvariantPredicate::SelfMethod(MethodName::new(format!("is_{name}")).unwrap()),
    )
}

fn spec_ref(anchor: &str) -> SpecRef {
    SpecRef::new(PathBuf::from("track/items/x/spec.json"), SpecElementId::try_new(anchor).unwrap())
}

fn method_with_spec_refs(name: &str, spec_refs: Vec<SpecRef>) -> MethodDeclaration {
    method_with_action(name, ItemAction::Add, spec_refs)
}

fn method_with_action(
    name: &str,
    action: ItemAction,
    spec_refs: Vec<SpecRef>,
) -> MethodDeclaration {
    MethodDeclaration::new(
        MethodName::new(name).unwrap(),
        Some(SelfReceiver::SharedRef),
        vec![],
        TypeRef::new("()").unwrap(),
        false,
        false,
        vec![],
        vec![],
        spec_refs,
        action,
        None,
    )
}

fn value_object_entry(action: ItemAction, invariants: Vec<InvariantDecl>) -> TypeEntry {
    TypeEntry::new(
        action,
        DataRole::ValueObject { invariants },
        TypeKindV2::Struct(StructKind::new(StructShape::Unit, None)),
        vec![],
        vec![],
        vec![],
        ModulePath::root(),
        None,
        vec![spec_ref("IN-05")],
        vec![],
    )
}

fn catalogue_with_type(name: &str, entry: TypeEntry) -> CatalogueDocument {
    let mut doc = CatalogueDocument::new(
        5,
        CrateName::new("domain").unwrap(),
        LayerId::try_new("domain").unwrap(),
    );
    doc.insert_type(CatalogueEntryKey::try_new(name.to_owned()).unwrap(), entry);
    doc
}

fn empty_catalogue(crate_name: &str, layer: &str) -> CatalogueDocument {
    CatalogueDocument::new(5, CrateName::new(crate_name).unwrap(), LayerId::try_new(layer).unwrap())
}

fn secondary_port_catalogue(
    action: ItemAction,
    methods: Vec<MethodDeclaration>,
    spec_refs: Vec<SpecRef>,
) -> CatalogueDocument {
    let mut catalogue = empty_catalogue("domain", "domain");
    catalogue.insert_trait(
        CatalogueEntryKey::try_new("TestPort".to_owned()).unwrap(),
        TraitEntry::new(
            action,
            ContractRole::SecondaryPort,
            methods,
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            spec_refs,
            vec![],
        ),
    );
    catalogue
}

fn trait_entry(role: ContractRole, anchor: &str) -> TraitEntry {
    TraitEntry::new(
        ItemAction::Add,
        role,
        vec![method_with_spec_refs("execute", vec![spec_ref(anchor)])],
        vec![],
        vec![],
        vec![],
        vec![],
        vec![],
        ModulePath::root(),
        None,
        vec![spec_ref(anchor)],
        vec![],
    )
}

fn trait_entry_in_module(role: ContractRole, anchor: &str, module: &str) -> TraitEntry {
    TraitEntry::new(
        ItemAction::Add,
        role,
        vec![method_with_spec_refs("execute", vec![spec_ref(anchor)])],
        vec![],
        vec![],
        vec![],
        vec![],
        vec![],
        ModulePath::from_segments(vec![module.to_owned()]).unwrap(),
        None,
        vec![spec_ref(anchor)],
        vec![],
    )
}

fn fulfillment_cache_port_entry(docs: Option<DocString>) -> TraitEntry {
    TraitEntry::new(
        ItemAction::Add,
        ContractRole::SecondaryPort,
        vec![
            MethodDeclaration::new(
                MethodName::new("load").unwrap(),
                Some(SelfReceiver::SharedRef),
                vec![],
                TypeRef::new("Result<ObligationFulfillmentCacheLoad, VerifyCacheError>").unwrap(),
                false,
                false,
                vec![],
                vec![],
                vec![spec_ref("AC-06")],
                ItemAction::Add,
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
                vec![spec_ref("AC-06")],
                ItemAction::Add,
                None,
            ),
        ],
        vec![],
        vec![],
        vec![],
        vec![],
        vec![],
        ModulePath::root(),
        docs,
        vec![spec_ref("AC-06")],
        vec![],
    )
}

fn interactor_with_catalogues(
    rules: TestObligationRulesDocument,
    catalogues: Vec<(PathBuf, CatalogueDocument)>,
) -> (DeriveTestObligationsInteractor, Arc<RecordingObligations>) {
    interactor_with_catalogues_and_status(rules, catalogues, Ok(TrackStatus::InProgress))
}

fn interactor_with_catalogues_and_status(
    rules: TestObligationRulesDocument,
    catalogues: Vec<(PathBuf, CatalogueDocument)>,
    track_status: Result<TrackStatus, TrackStatusReadError>,
) -> (DeriveTestObligationsInteractor, Arc<RecordingObligations>) {
    let docs: HashMap<PathBuf, CatalogueDocument> = catalogues.into_iter().collect();
    let obligations = Arc::new(RecordingObligations::default());
    let interactor = DeriveTestObligationsInteractor::new(
        Arc::new(StubRules { doc: rules }),
        Arc::clone(&obligations) as Arc<dyn ObligationsArtifactPort + Send + Sync>,
        Arc::new(StubSpec),
        Arc::new(StubCatalogue::new(docs)),
        Arc::new(StubTrackStatus { result: track_status }),
        PathBuf::from("track/items"),
        RoleObligationItemsProjector::new(),
    );
    (interactor, obligations)
}

fn interactor(
    rules: TestObligationRulesDocument,
    catalogue: CatalogueDocument,
    path: &Path,
) -> (DeriveTestObligationsInteractor, Arc<RecordingObligations>) {
    interactor_with_catalogues(rules, vec![(path.to_path_buf(), catalogue)])
}

fn command(paths: Vec<PathBuf>) -> DeriveTestObligationsCommand {
    DeriveTestObligationsCommand::new(TestObligationCatalogueCommandInput::new(
        TrackId::try_new("my-track").unwrap(),
        "track/my-track".to_owned(),
        paths,
    ))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn test_value_object_emits_one_obligation_per_invariant() {
    // CN-16 / CN-15 / IN-07: n invariants -> n boundary obligations.
    let path = PathBuf::from("domain-types.json");
    let entry =
        value_object_entry(ItemAction::Add, vec![invariant("positive"), invariant("bounded")]);
    let (interactor, sink) = interactor(rules_doc(), catalogue_with_type("Money", entry), &path);

    interactor.execute(&command(vec![path])).unwrap();

    let saved = sink.saved.lock().unwrap().clone().unwrap();
    assert_eq!(saved.obligations().len(), 2);
    let items: Vec<&str> =
        saved.obligations().iter().map(|o| o.id().item_identifier().as_str()).collect();
    assert!(items.contains(&"invariant:positive"));
    assert!(items.contains(&"invariant:bounded"));
    // AC-03 / CN-01: id is (entry_key, kind, item) — the kind is Boundary.
    assert!(
        saved
            .obligations()
            .iter()
            .all(|o| *o.id().obligation_kind() == TestObligationKind::Boundary)
    );
}

#[test]
fn test_derivation_reads_secondary_port_methods_and_saves_stable_obligations() {
    // IN-07 / AC-03: deriving a declared SecondaryPort creates one stable
    // obligation for each declared method, then persists the resulting artifact.
    let path = PathBuf::from("domain-types.json");
    let mut catalogue = empty_catalogue("domain", "domain");
    catalogue.insert_trait(
        CatalogueEntryKey::try_new("ObligationsArtifactPort".to_owned()).unwrap(),
        TraitEntry::new(
            ItemAction::Add,
            ContractRole::SecondaryPort,
            vec![
                MethodDeclaration::new(
                    MethodName::new("load").unwrap(),
                    Some(SelfReceiver::SharedRef),
                    vec![],
                    TypeRef::new("Result<Option<ObligationsDocument>, ArtifactCodecError>")
                        .unwrap(),
                    false,
                    false,
                    vec![],
                    vec![],
                    vec![spec_ref("AC-03")],
                    ItemAction::Add,
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
                    vec![spec_ref("AC-03")],
                    ItemAction::Add,
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
            vec![spec_ref("AC-03")],
            vec![],
        ),
    );
    let (interactor, sink) =
        interactor(rules_doc_with_secondary_port_contract_rules(), catalogue, &path);

    interactor.execute(&command(vec![path])).unwrap();

    let saved = sink.saved.lock().unwrap().clone().unwrap();
    let items: Vec<&str> = saved
        .obligations()
        .iter()
        .map(|obligation| obligation.id().item_identifier().as_str())
        .collect();
    assert_eq!(items, vec!["trait_method:load", "trait_method:save"]);
    assert!(
        saved
            .obligations()
            .iter()
            .all(|obligation| obligation.id().entry_key().as_str() == "ObligationsArtifactPort")
    );
}

#[test]
fn test_obligations_artifact_port_declaration_change_keeps_method_ids() {
    let path = PathBuf::from("domain-types.json");
    let first_catalogue = obligations_artifact_port_catalogue(None);
    let (first_interactor, first_sink) =
        interactor(rules_doc_with_secondary_port_contract_rules(), first_catalogue, &path);
    first_interactor.execute(&command(vec![path.clone()])).unwrap();
    let first = first_sink.saved.lock().unwrap().clone().unwrap();

    let second_catalogue = obligations_artifact_port_catalogue(Some(DocString::new(
        "changed declaration body".to_owned(),
    )));
    let (second_interactor, second_sink) =
        interactor(rules_doc_with_secondary_port_contract_rules(), second_catalogue, &path);
    second_interactor.execute(&command(vec![path])).unwrap();
    let second = second_sink.saved.lock().unwrap().clone().unwrap();

    let first_items: Vec<_> =
        first.obligations().iter().map(|obligation| obligation.id().clone()).collect();
    let second_items: Vec<_> =
        second.obligations().iter().map(|obligation| obligation.id().clone()).collect();
    assert_eq!(first_items, second_items);
    assert!(
        first
            .obligations()
            .iter()
            .zip(second.obligations())
            .all(|(before, after)| before.declaration_hash() != after.declaration_hash())
    );
}

#[test]
fn test_spec_document_loader_port_load_method_derives_contract_obligation() {
    let path = PathBuf::from("domain-types.json");
    let mut catalogue = empty_catalogue("domain", "domain");
    catalogue.insert_trait(
        CatalogueEntryKey::try_new("SpecDocumentLoaderPort".to_owned()).unwrap(),
        TraitEntry::new(
            ItemAction::Add,
            ContractRole::SecondaryPort,
            vec![MethodDeclaration::new(
                MethodName::new("load").unwrap(),
                Some(SelfReceiver::SharedRef),
                vec![],
                TypeRef::new("Result<SpecDocument, SpecDocumentLoadError>").unwrap(),
                false,
                false,
                vec![],
                vec![],
                vec![spec_ref("IN-07")],
                ItemAction::Add,
                None,
            )],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![spec_ref("IN-07")],
            vec![],
        ),
    );
    let (interactor, sink) =
        interactor(rules_doc_with_secondary_port_contract_rules(), catalogue, &path);

    interactor.execute(&command(vec![path])).unwrap();

    let saved = sink.saved.lock().unwrap().clone().unwrap();
    assert_eq!(saved.obligations().len(), 1);
    let obligation = &saved.obligations()[0];
    assert_eq!(obligation.id().entry_key().as_str(), "SpecDocumentLoaderPort");
    assert_eq!(obligation.id().item_identifier().as_str(), "trait_method:load");
    assert_eq!(obligation.spec_refs()[0].element_id(), "IN-07");
}

#[test]
fn test_trait_method_obligations_own_only_each_method_spec_refs() {
    let path = PathBuf::from("domain-types.json");
    let catalogue = secondary_port_catalogue(
        ItemAction::Add,
        vec![
            method_with_spec_refs("load", vec![spec_ref("IN-01")]),
            method_with_spec_refs("save", vec![spec_ref("IN-01"), spec_ref("AC-01")]),
        ],
        vec![spec_ref("IN-01"), spec_ref("AC-01")],
    );
    let (interactor, sink) =
        interactor(rules_doc_with_secondary_port_contract_rules(), catalogue, &path);

    interactor.execute(&command(vec![path])).unwrap();

    let saved = sink.saved.lock().unwrap().clone().unwrap();
    let load = saved
        .obligations()
        .iter()
        .find(|obligation| obligation.id().item_identifier().as_str() == "trait_method:load")
        .unwrap();
    let save = saved
        .obligations()
        .iter()
        .find(|obligation| obligation.id().item_identifier().as_str() == "trait_method:save")
        .unwrap();
    assert_eq!(
        load.spec_refs().iter().map(TestObligationAnchorId::element_id).collect::<Vec<_>>(),
        vec!["IN-01"]
    );
    assert_eq!(
        save.spec_refs().iter().map(TestObligationAnchorId::element_id).collect::<Vec<_>>(),
        vec!["IN-01", "AC-01"]
    );
}

#[test]
fn test_method_axis_obligations_also_use_each_method_spec_refs() {
    let path = PathBuf::from("domain-types.json");
    let catalogue = secondary_port_catalogue(
        ItemAction::Add,
        vec![
            method_with_spec_refs("load", vec![spec_ref("IN-01")]),
            method_with_spec_refs("save", vec![spec_ref("AC-01")]),
        ],
        vec![spec_ref("IN-01"), spec_ref("AC-01")],
    );
    let (interactor, sink) =
        interactor(rules_doc_with_secondary_port_method_rules(), catalogue, &path);

    interactor.execute(&command(vec![path])).unwrap();

    let saved = sink.saved.lock().unwrap().clone().unwrap();
    let refs_by_item: HashMap<_, _> = saved
        .obligations()
        .iter()
        .map(|obligation| {
            (
                obligation.id().item_identifier().as_str(),
                obligation
                    .spec_refs()
                    .iter()
                    .map(TestObligationAnchorId::element_id)
                    .collect::<Vec<_>>(),
            )
        })
        .collect();
    assert_eq!(refs_by_item.get("method:load"), Some(&vec!["IN-01"]));
    assert_eq!(refs_by_item.get("method:save"), Some(&vec!["AC-01"]));
}

#[test]
fn test_trait_method_without_spec_refs_is_rejected_for_add_method() {
    let path = PathBuf::from("domain-types.json");
    let catalogue = secondary_port_catalogue(
        ItemAction::Add,
        vec![
            method_with_spec_refs("load", vec![spec_ref("IN-01")]),
            method_with_spec_refs("save", vec![]),
        ],
        vec![spec_ref("IN-01")],
    );
    let (interactor, _) =
        interactor(rules_doc_with_secondary_port_contract_rules(), catalogue, &path);

    let result = interactor.execute(&command(vec![path]));
    assert!(matches!(
        result,
        Err(domain::tddd::test_obligation::errors::ObligationDeriveError::InvalidCatalogueState(_))
    ));
}

#[test]
fn test_trait_method_anchor_outside_entry_is_accepted() {
    let path = PathBuf::from("domain-types.json");
    let catalogue = secondary_port_catalogue(
        ItemAction::Add,
        vec![
            method_with_spec_refs("load", vec![spec_ref("IN-02")]),
            method_with_spec_refs("save", vec![spec_ref("IN-01")]),
        ],
        vec![spec_ref("IN-01")],
    );
    let (interactor, sink) =
        interactor(rules_doc_with_secondary_port_contract_rules(), catalogue, &path);

    interactor.execute(&command(vec![path])).unwrap();
    assert!(sink.saved.lock().unwrap().is_some());
}

#[test]
fn test_trait_method_anchor_union_need_not_cover_entry_catalogue() {
    let path = PathBuf::from("domain-types.json");
    let catalogue = secondary_port_catalogue(
        ItemAction::Add,
        vec![
            method_with_spec_refs("load", vec![spec_ref("IN-01")]),
            method_with_spec_refs("save", vec![spec_ref("IN-01")]),
        ],
        vec![spec_ref("IN-01"), spec_ref("AC-01")],
    );
    let (interactor, sink) =
        interactor(rules_doc_with_secondary_port_contract_rules(), catalogue, &path);

    interactor.execute(&command(vec![path])).unwrap();
    assert!(sink.saved.lock().unwrap().is_some());
}

#[test]
fn test_single_method_partial_entry_refs_are_accepted() {
    let path = PathBuf::from("domain-types.json");
    let catalogue = secondary_port_catalogue(
        ItemAction::Add,
        vec![method_with_spec_refs("load", vec![spec_ref("IN-01")])],
        vec![spec_ref("IN-01"), spec_ref("AC-01")],
    );
    let (interactor, sink) =
        interactor(rules_doc_with_secondary_port_contract_rules(), catalogue, &path);

    interactor.execute(&command(vec![path])).unwrap();
    assert!(sink.saved.lock().unwrap().is_some());
}

#[test]
fn test_modify_parent_empty_add_method_spec_refs_is_rejected() {
    let path = PathBuf::from("domain-types.json");
    let catalogue = secondary_port_catalogue(
        ItemAction::Modify,
        vec![method_with_spec_refs("load", vec![])],
        vec![spec_ref("IN-01"), spec_ref("AC-01")],
    );
    let (interactor, _) =
        interactor(rules_doc_with_secondary_port_contract_rules(), catalogue, &path);

    let result = interactor.execute(&command(vec![path]));

    assert!(matches!(
        result,
        Err(domain::tddd::test_obligation::errors::ObligationDeriveError::InvalidCatalogueState(_))
    ));
}

#[test]
fn test_modify_method_without_spec_refs_is_rejected() {
    let path = PathBuf::from("domain-types.json");
    let catalogue = secondary_port_catalogue(
        ItemAction::Modify,
        vec![method_with_action("load", ItemAction::Modify, vec![])],
        vec![spec_ref("IN-01")],
    );
    let (interactor, _) =
        interactor(rules_doc_with_secondary_port_contract_rules(), catalogue, &path);

    let result = interactor.execute(&command(vec![path]));
    assert!(matches!(
        result,
        Err(domain::tddd::test_obligation::errors::ObligationDeriveError::InvalidCatalogueState(_))
    ));
}

#[test]
fn test_method_action_does_not_inherit_parent_and_defaults_to_add() {
    let path = PathBuf::from("domain-types.json");
    let catalogue = secondary_port_catalogue(
        ItemAction::Reference,
        vec![method_with_action("load", ItemAction::Add, vec![])],
        vec![spec_ref("IN-01")],
    );
    let (interactor, _) =
        interactor(rules_doc_with_secondary_port_contract_rules(), catalogue, &path);

    let result = interactor.execute(&command(vec![path]));
    assert!(matches!(
        result,
        Err(domain::tddd::test_obligation::errors::ObligationDeriveError::InvalidCatalogueState(_))
    ));
}

#[test]
fn test_add_method_on_reference_trait_is_rejected() {
    let path = PathBuf::from("domain-types.json");
    let catalogue = secondary_port_catalogue(
        ItemAction::Reference,
        vec![method_with_action("load", ItemAction::Add, vec![spec_ref("IN-01")])],
        vec![spec_ref("IN-01")],
    );
    let (interactor, _) =
        interactor(rules_doc_with_secondary_port_contract_rules(), catalogue, &path);

    let result = interactor.execute(&command(vec![path]));
    assert!(matches!(
        result,
        Err(domain::tddd::test_obligation::errors::ObligationDeriveError::InvalidCatalogueState(_))
    ));
}

#[test]
fn test_reference_parent_forbids_method_spec_refs_regardless_of_anchor() {
    let path = PathBuf::from("domain-types.json");
    let catalogue = secondary_port_catalogue(
        ItemAction::Reference,
        vec![method_with_action("load", ItemAction::Add, vec![spec_ref("IN-99")])],
        vec![spec_ref("IN-01")],
    );
    let (interactor, _) =
        interactor(rules_doc_with_secondary_port_contract_rules(), catalogue, &path);

    let result = interactor.execute(&command(vec![path]));
    assert!(matches!(
        result,
        Err(domain::tddd::test_obligation::errors::ObligationDeriveError::InvalidCatalogueState(_))
    ));
}

#[test]
fn test_reference_trait_with_method_spec_refs_is_rejected() {
    let path = PathBuf::from("domain-types.json");
    let catalogue = secondary_port_catalogue(
        ItemAction::Reference,
        vec![method_with_action("load", ItemAction::Add, vec![spec_ref("IN-01")])],
        vec![spec_ref("IN-01")],
    );
    let rules = rules_doc_with_secondary_port_rules(vec![
        rule(TestObligationKind::Contract, TestObligationPerAxis::TraitMethod, None),
        rule(TestObligationKind::Result, TestObligationPerAxis::Entry, None),
    ]);
    let (interactor, _) = interactor(rules, catalogue, &path);

    let result = interactor.execute(&command(vec![path]));
    assert!(matches!(
        result,
        Err(domain::tddd::test_obligation::errors::ObligationDeriveError::InvalidCatalogueState(_))
    ));
}

#[test]
fn test_method_axes_only_does_not_emit_minimum_fillers() {
    let path = PathBuf::from("domain-types.json");
    let catalogue = secondary_port_catalogue(
        ItemAction::Reference,
        vec![method_with_action("load", ItemAction::Reference, vec![])],
        vec![spec_ref("IN-01")],
    );
    let rules = rules_doc_with_secondary_port_rules(vec![rule(
        TestObligationKind::Contract,
        TestObligationPerAxis::TraitMethod,
        Some(1),
    )]);
    let (interactor, sink) = interactor(rules, catalogue, &path);

    interactor.execute(&command(vec![path])).unwrap();
    let saved = sink.saved.lock().unwrap().clone().unwrap();
    assert!(saved.obligations().is_empty());
}

#[test]
fn test_add_method_on_reference_type_is_rejected() {
    let path = PathBuf::from("domain-types.json");
    let catalogue = catalogue_with_type(
        "Compute",
        domain_service_type(
            ItemAction::Reference,
            vec![method_with_action("compute", ItemAction::Add, vec![spec_ref("IN-13")])],
            vec![spec_ref("IN-13")],
        ),
    );
    let rules = rules_doc_with_domain_service_rules(vec![
        rule(TestObligationKind::LogicResult, TestObligationPerAxis::Method, None),
        rule(TestObligationKind::Result, TestObligationPerAxis::Entry, None),
    ]);
    let (interactor, _) = interactor(rules, catalogue, &path);

    let result = interactor.execute(&command(vec![path]));
    assert!(matches!(
        result,
        Err(domain::tddd::test_obligation::errors::ObligationDeriveError::InvalidCatalogueState(_))
    ));
}

#[test]
fn test_type_method_obligations_own_only_each_method_spec_refs() {
    let path = PathBuf::from("domain-types.json");
    let catalogue = catalogue_with_type(
        "Compute",
        domain_service_type(
            ItemAction::Add,
            vec![
                method_with_spec_refs("load", vec![spec_ref("IN-01")]),
                method_with_spec_refs("save", vec![spec_ref("AC-01")]),
            ],
            vec![spec_ref("IN-01"), spec_ref("AC-01")],
        ),
    );
    let rules = rules_doc_with_domain_service_rules(vec![rule(
        TestObligationKind::LogicResult,
        TestObligationPerAxis::Method,
        None,
    )]);
    let (interactor, sink) = interactor(rules, catalogue, &path);

    interactor.execute(&command(vec![path])).unwrap();
    let saved = sink.saved.lock().unwrap().clone().unwrap();
    let refs_by_item: HashMap<_, _> = saved
        .obligations()
        .iter()
        .map(|obligation| {
            (
                obligation.id().item_identifier().as_str(),
                obligation
                    .spec_refs()
                    .iter()
                    .map(TestObligationAnchorId::element_id)
                    .collect::<Vec<_>>(),
            )
        })
        .collect();
    assert_eq!(refs_by_item.get("method:load"), Some(&vec!["IN-01"]));
    assert_eq!(refs_by_item.get("method:save"), Some(&vec!["AC-01"]));
}

#[test]
fn test_reference_method_on_add_trait_is_not_derived() {
    let path = PathBuf::from("domain-types.json");
    let catalogue = secondary_port_catalogue(
        ItemAction::Add,
        vec![
            method_with_spec_refs("load", vec![spec_ref("IN-01")]),
            method_with_action("save", ItemAction::Reference, vec![]),
        ],
        vec![spec_ref("IN-01")],
    );
    let (interactor, sink) =
        interactor(rules_doc_with_secondary_port_contract_rules(), catalogue, &path);

    interactor.execute(&command(vec![path])).unwrap();
    let saved = sink.saved.lock().unwrap().clone().unwrap();
    let items: Vec<&str> =
        saved.obligations().iter().map(|o| o.id().item_identifier().as_str()).collect();
    assert_eq!(items, vec!["trait_method:load"]);
}

#[test]
fn test_delete_method_on_add_trait_is_not_derived() {
    let path = PathBuf::from("domain-types.json");
    let catalogue = secondary_port_catalogue(
        ItemAction::Add,
        vec![
            method_with_spec_refs("load", vec![spec_ref("IN-01")]),
            method_with_action("save", ItemAction::Delete, vec![]),
        ],
        vec![spec_ref("IN-01")],
    );
    let (interactor, sink) =
        interactor(rules_doc_with_secondary_port_contract_rules(), catalogue, &path);

    interactor.execute(&command(vec![path])).unwrap();
    let saved = sink.saved.lock().unwrap().clone().unwrap();
    let items: Vec<&str> =
        saved.obligations().iter().map(|o| o.id().item_identifier().as_str()).collect();
    assert_eq!(items, vec!["trait_method:load"]);
}

#[test]
fn test_empty_add_method_is_rejected_even_when_sibling_has_spec_refs() {
    let path = PathBuf::from("domain-types.json");
    let catalogue = secondary_port_catalogue(
        ItemAction::Add,
        vec![
            method_with_spec_refs("save", vec![spec_ref("IN-01")]),
            method_with_spec_refs("load", vec![]),
        ],
        vec![spec_ref("IN-01")],
    );
    let (interactor, _) =
        interactor(rules_doc_with_secondary_port_contract_rules(), catalogue, &path);

    let result = interactor.execute(&command(vec![path]));
    assert!(matches!(
        result,
        Err(domain::tddd::test_obligation::errors::ObligationDeriveError::InvalidCatalogueState(_))
    ));
}

#[test]
fn test_reference_method_does_not_steal_later_add_method_anchors() {
    let path = PathBuf::from("domain-types.json");
    let catalogue = secondary_port_catalogue(
        ItemAction::Add,
        vec![
            method_with_action("skip", ItemAction::Reference, vec![]),
            method_with_spec_refs("save", vec![spec_ref("IN-01")]),
        ],
        vec![spec_ref("IN-01")],
    );
    let (interactor, sink) =
        interactor(rules_doc_with_secondary_port_contract_rules(), catalogue, &path);

    interactor.execute(&command(vec![path])).unwrap();
    let saved = sink.saved.lock().unwrap().clone().unwrap();
    let save = saved
        .obligations()
        .iter()
        .find(|o| o.id().item_identifier().as_str() == "trait_method:save")
        .expect("save method obligation");
    let anchors: Vec<&str> = save.spec_refs().iter().map(|a| a.element_id()).collect();
    assert_eq!(anchors, vec!["IN-01"]);
}

#[test]
fn test_type_entry_add_method_without_spec_refs_is_rejected() {
    let path = PathBuf::from("domain-types.json");
    let entry = TypeEntry::new(
        ItemAction::Add,
        DataRole::domain_service(),
        TypeKindV2::Struct(StructKind::new(StructShape::Unit, None)),
        vec![method_with_action("compute", ItemAction::Add, vec![])],
        vec![],
        vec![],
        ModulePath::root(),
        None,
        vec![spec_ref("IN-13")],
        vec![],
    );
    let catalogue = catalogue_with_type("Compute", entry);
    let (interactor, _) =
        interactor(rules_doc_with_secondary_port_contract_rules(), catalogue, &path);

    let result = interactor.execute(&command(vec![path]));
    assert!(matches!(
        result,
        Err(domain::tddd::test_obligation::errors::ObligationDeriveError::InvalidCatalogueState(_))
    ));
}

#[test]
fn test_type_entry_omitted_method_action_does_not_inherit_parent() {
    let path = PathBuf::from("domain-types.json");
    let omitted_add = MethodDeclaration::associated_function(
        MethodName::new("compute").unwrap(),
        vec![],
        TypeRef::new("()").unwrap(),
    );
    let entry = TypeEntry::new(
        ItemAction::Reference,
        DataRole::domain_service(),
        TypeKindV2::Struct(StructKind::new(StructShape::Unit, None)),
        vec![omitted_add],
        vec![],
        vec![],
        ModulePath::root(),
        None,
        vec![spec_ref("IN-13")],
        vec![],
    );
    let catalogue = catalogue_with_type("Compute", entry);
    let (interactor, _) =
        interactor(rules_doc_with_secondary_port_contract_rules(), catalogue, &path);

    let result = interactor.execute(&command(vec![path]));
    assert!(matches!(
        result,
        Err(domain::tddd::test_obligation::errors::ObligationDeriveError::InvalidCatalogueState(_))
    ));
}

#[test]
fn test_inherent_impl_add_method_without_spec_refs_is_rejected() {
    let path = PathBuf::from("domain-types.json");
    let mut catalogue = catalogue_with_type("Compute", value_object_entry(ItemAction::Add, vec![]));
    catalogue.push_inherent_impl(InherentImplDeclV2::new(
        CatalogueEntryKey::try_new("Compute".to_owned()).unwrap(),
        vec![],
        vec![],
        vec![method_with_action("compute", ItemAction::Add, vec![])],
    ));
    let (interactor, _) =
        interactor(rules_doc_with_secondary_port_contract_rules(), catalogue, &path);

    let result = interactor.execute(&command(vec![path]));
    assert!(matches!(
        result,
        Err(domain::tddd::test_obligation::errors::ObligationDeriveError::InvalidCatalogueState(_))
    ));
}

#[test]
fn test_named_catalogue_validation_resolves_short_inherent_owner() {
    let mut catalogue = empty_catalogue("domain", "domain");
    catalogue.insert_type(
        CatalogueEntryKey::try_new("a::Compute".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::Struct(StructKind::new(StructShape::Unit, None)),
            vec![],
            vec![],
            vec![],
            ModulePath::from_segments(vec!["a".to_owned()]).unwrap(),
            None,
            vec![spec_ref("IN-05")],
            vec![],
        ),
    );
    catalogue.push_inherent_impl(InherentImplDeclV2::new(
        CatalogueEntryKey::try_new("Compute".to_owned()).unwrap(),
        vec![],
        vec![],
        vec![method_with_spec_refs("compute", vec![spec_ref("IN-13")])],
    ));

    assert!(validate_named_catalogue_methods(&catalogue).is_ok());
}

#[test]
fn test_inherent_impl_omitted_method_action_does_not_inherit_parent() {
    let path = PathBuf::from("domain-types.json");
    let mut catalogue =
        catalogue_with_type("Compute", value_object_entry(ItemAction::Reference, vec![]));
    catalogue.push_inherent_impl(InherentImplDeclV2::new(
        CatalogueEntryKey::try_new("Compute".to_owned()).unwrap(),
        vec![],
        vec![],
        vec![MethodDeclaration::associated_function(
            MethodName::new("compute").unwrap(),
            vec![],
            TypeRef::new("()").unwrap(),
        )],
    ));
    let (interactor, _) =
        interactor(rules_doc_with_secondary_port_contract_rules(), catalogue, &path);

    let result = interactor.execute(&command(vec![path]));
    assert!(matches!(
        result,
        Err(domain::tddd::test_obligation::errors::ObligationDeriveError::InvalidCatalogueState(_))
    ));
}

#[test]
fn test_single_method_empty_entry_rejects_add_method_without_spec_refs() {
    let path = PathBuf::from("domain-types.json");
    let catalogue = secondary_port_catalogue(
        ItemAction::Add,
        vec![method_with_spec_refs("load", vec![])],
        vec![],
    );
    let (interactor, _) =
        interactor(rules_doc_with_secondary_port_contract_rules(), catalogue, &path);

    let result = interactor.execute(&command(vec![path]));
    assert!(matches!(
        result,
        Err(domain::tddd::test_obligation::errors::ObligationDeriveError::InvalidCatalogueState(_))
    ));
}

#[test]
fn test_reference_and_delete_methods_without_spec_refs_are_accepted() {
    for action in [ItemAction::Reference, ItemAction::Delete] {
        let path = PathBuf::from("domain-types.json");
        let catalogue = secondary_port_catalogue(
            action,
            vec![method_with_action("load", action, vec![])],
            vec![spec_ref("IN-01")],
        );
        let (interactor, sink) =
            interactor(rules_doc_with_secondary_port_contract_rules(), catalogue, &path);

        interactor.execute(&command(vec![path])).unwrap();

        assert!(sink.saved.lock().unwrap().is_some());
    }
}

#[test]
fn test_derive_validates_only_catalogues_supplied_on_the_command() {
    let valid_path = PathBuf::from("domain-types.json");
    let unused_invalid_path = PathBuf::from("other-track-types.json");
    let valid = secondary_port_catalogue(
        ItemAction::Add,
        vec![method_with_spec_refs("load", vec![spec_ref("IN-01")])],
        vec![spec_ref("IN-01")],
    );
    let unused_invalid = secondary_port_catalogue(
        ItemAction::Add,
        vec![method_with_spec_refs("load", vec![])],
        vec![spec_ref("IN-01")],
    );
    let docs =
        HashMap::from([(valid_path.clone(), valid), (unused_invalid_path.clone(), unused_invalid)]);
    let catalogues = Arc::new(StubCatalogue::new(docs));
    let sink = Arc::new(RecordingObligations::default());
    let interactor = DeriveTestObligationsInteractor::new(
        Arc::new(StubRules { doc: rules_doc_with_secondary_port_contract_rules() }),
        Arc::clone(&sink) as Arc<dyn ObligationsArtifactPort + Send + Sync>,
        Arc::new(StubSpec),
        Arc::clone(&catalogues) as Arc<dyn CatalogueDocumentLoaderPort + Send + Sync>,
        Arc::new(StubTrackStatus { result: Ok(TrackStatus::InProgress) }),
        PathBuf::from("track/items"),
        RoleObligationItemsProjector::new(),
    );

    interactor.execute(&command(vec![valid_path.clone()])).unwrap();

    assert!(sink.saved.lock().unwrap().is_some());
    let loaded = catalogues.loads.lock().unwrap().clone();
    assert!(loaded.contains(&valid_path));
    assert!(!loaded.contains(&unused_invalid_path));
}

fn obligations_artifact_port_catalogue(docs: Option<DocString>) -> CatalogueDocument {
    let mut catalogue = empty_catalogue("domain", "domain");
    catalogue.insert_trait(
        CatalogueEntryKey::try_new("ObligationsArtifactPort".to_owned()).unwrap(),
        TraitEntry::new(
            ItemAction::Add,
            ContractRole::SecondaryPort,
            vec![
                MethodDeclaration::new(
                    MethodName::new("load").unwrap(),
                    Some(SelfReceiver::SharedRef),
                    vec![],
                    TypeRef::new("Result<Option<ObligationsDocument>, ArtifactCodecError>")
                        .unwrap(),
                    false,
                    false,
                    vec![],
                    vec![],
                    vec![spec_ref("AC-03")],
                    ItemAction::Add,
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
                    vec![spec_ref("AC-03")],
                    ItemAction::Add,
                    None,
                ),
            ],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            docs,
            vec![spec_ref("AC-03")],
            vec![],
        ),
    );
    catalogue
}

#[test]
fn test_derivation_is_repeatable_and_populates_required_obligation_fields() {
    let path = PathBuf::from("domain-types.json");
    let catalogue = catalogue_with_type(
        "Money",
        value_object_entry(ItemAction::Add, vec![invariant("positive")]),
    );

    let (first_interactor, first_sink) = interactor(rules_doc(), catalogue.clone(), &path);
    first_interactor.execute(&command(vec![path.clone()])).unwrap();
    let first = first_sink.saved.lock().unwrap().clone().unwrap();

    let (second_interactor, second_sink) = interactor(rules_doc(), catalogue, &path);
    second_interactor.execute(&command(vec![path])).unwrap();
    let second = second_sink.saved.lock().unwrap().clone().unwrap();

    assert_eq!(first, second);
    let obligation = &first.obligations()[0];
    assert_eq!(obligation.id().entry_key().as_str(), "Money");
    assert_eq!(obligation.id().item_identifier().as_str(), "invariant:positive");
    assert_eq!(obligation.target_entry().entry_key.as_str(), "Money");
    assert_eq!(
        obligation.target_role(),
        &TargetEntryRoleKind::DataRole("ValueObject".parse().unwrap())
    );
    assert!(!obligation.brief().as_str().is_empty());
    assert_eq!(obligation.spec_refs()[0].element_id(), "IN-05");
}

#[test]
fn test_derivation_with_anchored_catalogue_path_persists_project_relative_path() {
    let relative_path = PathBuf::from("track/items/my-track/domain-types.json");
    let anchored_path = PathBuf::from("/checkout/project/track/items/my-track/domain-types.json");
    let catalogue = catalogue_with_type(
        "Money",
        value_object_entry(ItemAction::Add, vec![invariant("positive")]),
    );
    let (relative_interactor, relative_sink) =
        interactor(rules_doc(), catalogue.clone(), &relative_path);
    let (anchored_interactor, anchored_sink) = interactor(rules_doc(), catalogue, &anchored_path);

    relative_interactor.execute(&command(vec![relative_path])).unwrap();
    anchored_interactor.execute(&command(vec![anchored_path])).unwrap();

    let relative = relative_sink.saved.lock().unwrap().clone().unwrap();
    let anchored = anchored_sink.saved.lock().unwrap().clone().unwrap();
    assert_eq!(
        anchored.obligations()[0].target_entry().file_path,
        "track/items/my-track/domain-types.json"
    );
    assert_eq!(
        anchored.obligations()[0].declaration_hash(),
        relative.obligations()[0].declaration_hash()
    );
    assert_eq!(anchored.obligations()[0].spec_refs(), relative.obligations()[0].spec_refs());
}

#[test]
fn test_value_object_without_invariants_emits_zero() {
    // CN-16: the type system makes the invalid state unrepresentable -> 0.
    let path = PathBuf::from("domain-types.json");
    let entry = value_object_entry(ItemAction::Add, vec![]);
    let (interactor, sink) = interactor(rules_doc(), catalogue_with_type("Unit", entry), &path);

    interactor.execute(&command(vec![path])).unwrap();

    assert!(sink.saved.lock().unwrap().clone().unwrap().obligations().is_empty());
}

#[test]
fn test_reference_and_delete_entries_are_skipped() {
    // CN-11 / AC-17: Reference is outside the edge universe; Delete yields zero.
    for action in [ItemAction::Reference, ItemAction::Delete] {
        let path = PathBuf::from("domain-types.json");
        let entry = value_object_entry(action, vec![invariant("positive")]);
        let (interactor, sink) =
            interactor(rules_doc(), catalogue_with_type("Money", entry), &path);
        interactor.execute(&command(vec![path])).unwrap();
        assert!(sink.saved.lock().unwrap().clone().unwrap().obligations().is_empty());
    }
}

#[test]
fn test_declaration_change_keeps_id_but_changes_declaration_hash() {
    // AC-03 / CN-01: id is content-hash-free; only decl_hash tracks content.
    let path = PathBuf::from("domain-types.json");
    let (i1, s1) = interactor(
        rules_doc(),
        catalogue_with_type(
            "Money",
            value_object_entry(ItemAction::Add, vec![invariant("positive")]),
        ),
        &path,
    );
    i1.execute(&command(vec![path.clone()])).unwrap();
    let first = s1.saved.lock().unwrap().clone().unwrap();

    // Same identity, different declaration content (an added method changes Debug).
    let mut entry = value_object_entry(ItemAction::Modify, vec![invariant("positive")]);
    entry = with_docs(entry);
    let (i2, s2) = interactor(rules_doc(), catalogue_with_type("Money", entry), &path);
    i2.execute(&command(vec![path])).unwrap();
    let second = s2.saved.lock().unwrap().clone().unwrap();

    let a = &first.obligations()[0];
    let b = &second.obligations()[0];
    assert_eq!(a.id(), b.id());
    assert_ne!(a.declaration_hash(), b.declaration_hash());
}

fn with_docs(entry: TypeEntry) -> TypeEntry {
    // Rebuild the entry with a doc string so its `Debug` canonical form differs.
    TypeEntry::new(
        entry.action(),
        entry.role().clone(),
        entry.kind().clone(),
        entry.methods().to_vec(),
        entry.generics().to_vec(),
        entry.where_predicates().to_vec(),
        entry.module_path().clone(),
        Some(DocString::new("changed body".to_owned())),
        entry.spec_refs().to_vec(),
        entry.informal_grounds().to_vec(),
    )
}

#[test]
fn test_trait_impl_resolves_role_and_external_trait_yields_zero() {
    // IN-17 / AC-16: trait_ref -> catalogue ContractRole; external -> 0.
    let path = PathBuf::from("infrastructure-types.json");
    let mut doc = CatalogueDocument::new(
        5,
        CrateName::new("infrastructure").unwrap(),
        LayerId::try_new("infrastructure").unwrap(),
    );
    // A local port trait (SecondaryPort) with an internal impl, plus an external impl.
    doc.insert_trait(
        CatalogueEntryKey::try_new("MyPort".to_owned()).unwrap(),
        TraitEntry::new(
            ItemAction::Add,
            ContractRole::SecondaryPort,
            vec![method_with_spec_refs("execute", vec![spec_ref("IN-06")])],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![spec_ref("IN-06")],
            vec![],
        ),
    );
    doc.insert_trait(
        CatalogueEntryKey::try_new("Display".to_owned()).unwrap(),
        TraitEntry::new(
            ItemAction::Add,
            ContractRole::SecondaryPort,
            vec![method_with_spec_refs("execute", vec![spec_ref("IN-99")])],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![spec_ref("IN-99")],
            vec![],
        ),
    );
    doc.push_trait_impl(TraitImplDeclV2::new(
        TypeRef::new("MyPort").unwrap(),
        TypeRef::new("MyAdapter").unwrap(),
    ));
    doc.push_trait_impl(TraitImplDeclV2::new(
        TypeRef::new("std::fmt::Display").unwrap(),
        TypeRef::new("MyAdapter").unwrap(),
    ));

    let (interactor, sink) = interactor(rules_doc(), doc, &path);
    interactor.execute(&command(vec![path])).unwrap();
    let saved = sink.saved.lock().unwrap().clone().unwrap();

    // Exactly one contract-conformance obligation (from MyPort's impl; Display is external).
    let conformance: Vec<_> = saved
        .obligations()
        .iter()
        .filter(|o| *o.id().obligation_kind() == TestObligationKind::ContractConformance)
        .collect();
    assert_eq!(conformance.len(), 1);
    assert_eq!(conformance[0].id().entry_key().as_str(), "MyAdapter");
    assert_eq!(conformance[0].spec_refs().len(), 1);
    assert_eq!(conformance[0].spec_refs()[0].element_id(), "IN-06");
}

#[test]
fn test_trait_impl_declaration_resolves_self_crate_trait_from_other_catalogue() {
    let trait_path = PathBuf::from("infrastructure-traits.json");
    let impl_path = PathBuf::from("infrastructure-impls.json");
    let mut trait_catalogue = empty_catalogue("infrastructure", "infrastructure");
    trait_catalogue.insert_trait(
        CatalogueEntryKey::try_new("MyPort".to_owned()).unwrap(),
        trait_entry(ContractRole::SecondaryPort, "IN-06"),
    );
    let mut impl_catalogue = empty_catalogue("infrastructure", "infrastructure");
    impl_catalogue.push_trait_impl(TraitImplDeclV2::new(
        TypeRef::new("MyPort").unwrap(),
        TypeRef::new("MyAdapter").unwrap(),
    ));

    let (interactor, sink) = interactor_with_catalogues(
        rules_doc(),
        vec![
            (trait_path.clone(), trait_catalogue.clone()),
            (impl_path.clone(), impl_catalogue.clone()),
        ],
    );
    interactor.execute(&command(vec![trait_path, impl_path])).unwrap();
    let saved = sink.saved.lock().unwrap().clone().unwrap();
    let obligation = saved
        .obligations()
        .iter()
        .find(|obligation| {
            obligation.id().entry_key().as_str() == "MyAdapter"
                && *obligation.id().obligation_kind() == TestObligationKind::ContractConformance
        })
        .unwrap();

    let declaration =
        obligation_declaration_text(&[trait_catalogue, impl_catalogue], obligation).unwrap();
    assert!(declaration.contains("trait_declaration:"));
}

#[test]
fn test_fs_spec_document_loader_trait_impl_derives_its_secondary_port_obligation() {
    // IN-07 / IN-08: the real filesystem loader declaration resolves the
    // domain port's SecondaryPort role across the catalogue boundary.
    let domain_path = PathBuf::from("domain-types.json");
    let infrastructure_path = PathBuf::from("infrastructure-types.json");
    let mut domain = empty_catalogue("domain", "domain");
    domain.insert_trait(
        CatalogueEntryKey::try_new("SpecDocumentLoaderPort".to_owned()).unwrap(),
        trait_entry_in_module(ContractRole::SecondaryPort, "IN-07", "spec_document_loader_port"),
    );
    let mut infrastructure = empty_catalogue("infrastructure", "infrastructure");
    infrastructure.push_trait_impl(TraitImplDeclV2::new(
        TypeRef::new("domain::spec_document_loader_port::SpecDocumentLoaderPort").unwrap(),
        TypeRef::new("FsSpecDocumentLoader").unwrap(),
    ));

    let (interactor, sink) = interactor_with_catalogues(
        rules_doc(),
        vec![(domain_path.clone(), domain), (infrastructure_path.clone(), infrastructure)],
    );
    interactor.execute(&command(vec![domain_path, infrastructure_path])).unwrap();

    let saved = sink.saved.lock().unwrap().clone().unwrap();
    let conformance: Vec<_> = saved
        .obligations()
        .iter()
        .filter(|obligation| {
            obligation.id().entry_key().as_str() == "FsSpecDocumentLoader"
                && *obligation.id().obligation_kind() == TestObligationKind::ContractConformance
        })
        .collect();
    assert_eq!(conformance.len(), 1);
    assert_eq!(
        conformance[0].id().item_identifier().as_str(),
        "trait_impl:domain::spec_document_loader_port::SpecDocumentLoaderPort"
    );
    assert_eq!(conformance[0].spec_refs()[0].element_id(), "IN-07");
}

#[test]
fn test_trait_impl_declaration_hash_includes_resolved_port_contract() {
    let usecase_path = PathBuf::from("usecase-types.json");
    let infrastructure_path = PathBuf::from("infrastructure-types.json");
    let mut usecase = empty_catalogue("usecase", "usecase");
    usecase.insert_trait(
        CatalogueEntryKey::try_new("ObligationFulfillmentCachePort".to_owned()).unwrap(),
        fulfillment_cache_port_entry(None),
    );
    let mut infrastructure = empty_catalogue("infrastructure", "infrastructure");
    infrastructure.push_trait_impl(TraitImplDeclV2::new(
        TypeRef::new("usecase::ObligationFulfillmentCachePort").unwrap(),
        TypeRef::new("JsonObligationFulfillmentCacheCodec").unwrap(),
    ));

    let (first_interactor, first_sink) = interactor_with_catalogues(
        rules_doc(),
        vec![
            (usecase_path.clone(), usecase.clone()),
            (infrastructure_path.clone(), infrastructure.clone()),
        ],
    );
    first_interactor
        .execute(&command(vec![usecase_path.clone(), infrastructure_path.clone()]))
        .unwrap();
    let first = first_sink.saved.lock().unwrap().clone().unwrap();
    let first_obligation = first
        .obligations()
        .iter()
        .find(|obligation| {
            obligation.id().entry_key().as_str() == "JsonObligationFulfillmentCacheCodec"
        })
        .unwrap();
    let declaration =
        obligation_declaration_text(&[usecase.clone(), infrastructure.clone()], first_obligation)
            .unwrap();
    assert!(declaration.contains("load"));
    assert!(declaration.contains("save"));
    let duplicate_declaration = obligation_declaration_text(
        &[usecase.clone(), usecase.clone(), infrastructure.clone()],
        first_obligation,
    )
    .unwrap();
    assert_eq!(duplicate_declaration, declaration);

    let mut changed_usecase = empty_catalogue("usecase", "usecase");
    changed_usecase.insert_trait(
        CatalogueEntryKey::try_new("ObligationFulfillmentCachePort".to_owned()).unwrap(),
        fulfillment_cache_port_entry(Some(DocString::new("changed port contract".to_owned()))),
    );
    let (second_interactor, second_sink) = interactor_with_catalogues(
        rules_doc(),
        vec![
            (usecase_path.clone(), changed_usecase),
            (infrastructure_path.clone(), infrastructure),
        ],
    );
    second_interactor.execute(&command(vec![usecase_path, infrastructure_path])).unwrap();
    let second = second_sink.saved.lock().unwrap().clone().unwrap();
    let second_obligation = second.obligations().first().unwrap();

    assert_eq!(first_obligation.id(), second_obligation.id());
    assert_ne!(first_obligation.declaration_hash(), second_obligation.declaration_hash());
}

#[test]
fn test_trait_impl_unsupported_axis_yields_zero_even_with_minimum() {
    let path = PathBuf::from("infrastructure-types.json");
    let mut doc = CatalogueDocument::new(
        5,
        CrateName::new("infrastructure").unwrap(),
        LayerId::try_new("infrastructure").unwrap(),
    );
    doc.insert_trait(
        CatalogueEntryKey::try_new("MyPort".to_owned()).unwrap(),
        trait_entry(ContractRole::SecondaryPort, "IN-06"),
    );
    doc.push_trait_impl(TraitImplDeclV2::new(
        TypeRef::new("MyPort").unwrap(),
        TypeRef::new("MyAdapter").unwrap(),
    ));

    let rules = rules_doc_with_secondary_port_trait_impl_rules(vec![rule(
        TestObligationKind::ContractConformance,
        TestObligationPerAxis::Method,
        Some(1),
    )]);
    let (interactor, sink) = interactor(rules, doc, &path);

    interactor.execute(&command(vec![path])).unwrap();
    let saved = sink.saved.lock().unwrap().clone().unwrap();

    assert!(saved.obligations().is_empty());
}

#[test]
fn test_workspace_qualified_trait_impl_uses_qualified_crate_trait() {
    // IN-17: a workspace-qualified trait ref must not collapse to the first
    // matching bare trait name from another crate.
    let domain_path = PathBuf::from("domain-types.json");
    let usecase_path = PathBuf::from("usecase-types.json");
    let infrastructure_path = PathBuf::from("infrastructure-types.json");

    let mut domain_doc = empty_catalogue("domain", "domain");
    domain_doc.insert_trait(
        CatalogueEntryKey::try_new("SharedPort".to_owned()).unwrap(),
        trait_entry(ContractRole::SecondaryPort, "IN-99"),
    );

    let mut usecase_doc = empty_catalogue("usecase", "usecase");
    usecase_doc.insert_trait(
        CatalogueEntryKey::try_new("SharedPort".to_owned()).unwrap(),
        trait_entry(ContractRole::SecondaryPort, "IN-06"),
    );

    let mut infrastructure_doc = empty_catalogue("infrastructure", "infrastructure");
    infrastructure_doc.push_trait_impl(TraitImplDeclV2::new(
        TypeRef::new("usecase::SharedPort").unwrap(),
        TypeRef::new("MyAdapter").unwrap(),
    ));

    let (interactor, sink) = interactor_with_catalogues(
        rules_doc(),
        vec![
            (domain_path.clone(), domain_doc),
            (usecase_path.clone(), usecase_doc),
            (infrastructure_path.clone(), infrastructure_doc),
        ],
    );

    interactor.execute(&command(vec![domain_path, usecase_path, infrastructure_path])).unwrap();
    let saved = sink.saved.lock().unwrap().clone().unwrap();
    let conformance: Vec<_> = saved
        .obligations()
        .iter()
        .filter(|o| *o.id().obligation_kind() == TestObligationKind::ContractConformance)
        .collect();

    assert_eq!(conformance.len(), 1);
    assert_eq!(conformance[0].id().entry_key().as_str(), "MyAdapter");
    assert_eq!(conformance[0].spec_refs()[0].element_id(), "IN-06");
    assert_eq!(conformance[0].id().item_identifier().as_str(), "trait_impl:usecase::SharedPort");
}

#[test]
fn test_trait_role_index_distinguishes_same_name_traits_by_module_path() {
    let path = PathBuf::from("infrastructure-types.json");
    let mut doc = empty_catalogue("infrastructure", "infrastructure");
    doc.insert_trait(
        CatalogueEntryKey::try_new("infrastructure::alpha::SharedPort".to_owned()).unwrap(),
        trait_entry_in_module(ContractRole::SecondaryPort, "IN-06", "alpha"),
    );
    doc.insert_trait(
        CatalogueEntryKey::try_new("infrastructure::beta::SharedPort".to_owned()).unwrap(),
        trait_entry_in_module(ContractRole::SecondaryPort, "IN-99", "beta"),
    );
    doc.push_trait_impl(TraitImplDeclV2::new(
        TypeRef::new("infrastructure::alpha::SharedPort").unwrap(),
        TypeRef::new("AlphaAdapter").unwrap(),
    ));
    doc.push_trait_impl(TraitImplDeclV2::new(
        TypeRef::new("infrastructure::beta::SharedPort").unwrap(),
        TypeRef::new("BetaAdapter").unwrap(),
    ));

    let (interactor, sink) = interactor(rules_doc(), doc, &path);
    interactor.execute(&command(vec![path])).unwrap();
    let saved = sink.saved.lock().unwrap().clone().unwrap();
    let conformance: HashMap<_, _> = saved
        .obligations()
        .iter()
        .filter(|obligation| {
            *obligation.id().obligation_kind() == TestObligationKind::ContractConformance
        })
        .map(|obligation| {
            (
                obligation.id().entry_key().as_str().to_owned(),
                obligation.spec_refs()[0].element_id().to_owned(),
            )
        })
        .collect();

    assert_eq!(conformance.get("AlphaAdapter").map(String::as_str), Some("IN-06"));
    assert_eq!(conformance.get("BetaAdapter").map(String::as_str), Some("IN-99"));
}

#[test]
fn test_trait_role_index_rejects_ambiguous_bare_same_name_trait() {
    let path = PathBuf::from("infrastructure-types.json");
    let mut doc = empty_catalogue("infrastructure", "infrastructure");
    doc.insert_trait(
        CatalogueEntryKey::try_new("infrastructure::alpha::SharedPort".to_owned()).unwrap(),
        trait_entry_in_module(ContractRole::SecondaryPort, "IN-06", "alpha"),
    );
    doc.insert_trait(
        CatalogueEntryKey::try_new("infrastructure::beta::SharedPort".to_owned()).unwrap(),
        trait_entry_in_module(ContractRole::SecondaryPort, "IN-99", "beta"),
    );
    doc.push_trait_impl(TraitImplDeclV2::new(
        TypeRef::new("SharedPort").unwrap(),
        TypeRef::new("AmbiguousAdapter").unwrap(),
    ));

    let (interactor, sink) = interactor(rules_doc(), doc, &path);
    interactor.execute(&command(vec![path])).unwrap();
    let saved = sink.saved.lock().unwrap().clone().unwrap();

    assert!(!saved.obligations().iter().any(|obligation| {
        *obligation.id().obligation_kind() == TestObligationKind::ContractConformance
            && obligation.id().entry_key().as_str() == "AmbiguousAdapter"
    }));
}

#[test]
fn test_trait_role_index_attributes_same_name_traits_across_catalogue_crates() {
    let domain_path = PathBuf::from("domain-types.json");
    let usecase_path = PathBuf::from("usecase-types.json");
    let infrastructure_path = PathBuf::from("infrastructure-types.json");

    let mut domain_doc = empty_catalogue("domain", "domain");
    domain_doc.insert_trait(
        CatalogueEntryKey::try_new("domain::alpha::SharedPort".to_owned()).unwrap(),
        trait_entry_in_module(ContractRole::SecondaryPort, "IN-06", "alpha"),
    );

    let mut usecase_doc = empty_catalogue("usecase", "usecase");
    usecase_doc.insert_trait(
        CatalogueEntryKey::try_new("usecase::beta::SharedPort".to_owned()).unwrap(),
        trait_entry_in_module(ContractRole::SecondaryPort, "IN-99", "beta"),
    );

    let mut infrastructure_doc = empty_catalogue("infrastructure", "infrastructure");
    infrastructure_doc.push_trait_impl(TraitImplDeclV2::new(
        TypeRef::new("domain::alpha::SharedPort").unwrap(),
        TypeRef::new("infrastructure::alpha::AlphaAdapter").unwrap(),
    ));
    infrastructure_doc.push_trait_impl(TraitImplDeclV2::new(
        TypeRef::new("usecase::beta::SharedPort").unwrap(),
        TypeRef::new("infrastructure::beta::BetaAdapter").unwrap(),
    ));

    let (interactor, sink) = interactor_with_catalogues(
        rules_doc(),
        vec![
            (domain_path.clone(), domain_doc),
            (usecase_path.clone(), usecase_doc),
            (infrastructure_path.clone(), infrastructure_doc),
        ],
    );

    interactor.execute(&command(vec![domain_path, usecase_path, infrastructure_path])).unwrap();
    let saved = sink.saved.lock().unwrap().clone().unwrap();
    let conformance: HashMap<_, _> = saved
        .obligations()
        .iter()
        .filter(|obligation| {
            *obligation.id().obligation_kind() == TestObligationKind::ContractConformance
        })
        .map(|obligation| {
            (
                obligation.id().entry_key().as_str().to_owned(),
                obligation.spec_refs()[0].element_id().to_owned(),
            )
        })
        .collect();

    assert_eq!(conformance.len(), 2);
    assert_eq!(
        conformance.get("infrastructure::alpha::AlphaAdapter").map(String::as_str),
        Some("IN-06")
    );
    assert_eq!(
        conformance.get("infrastructure::beta::BetaAdapter").map(String::as_str),
        Some("IN-99")
    );
}

#[test]
fn test_non_active_branch_is_rejected() {
    let path = PathBuf::from("domain-types.json");
    let entry = value_object_entry(ItemAction::Add, vec![invariant("positive")]);
    let (interactor, _) = interactor(rules_doc(), catalogue_with_type("Money", entry), &path);

    let cmd = DeriveTestObligationsCommand::new(TestObligationCatalogueCommandInput::new(
        TrackId::try_new("my-track").unwrap(),
        "main".to_owned(),
        vec![path],
    ));
    let result = interactor.execute(&cmd);
    assert!(matches!(
        result,
        Err(domain::tddd::test_obligation::errors::ObligationDeriveError::TrackNotActive { .. })
    ));
}

#[test]
fn test_frozen_track_is_rejected_before_obligation_artifact_write() {
    let path = PathBuf::from("domain-types.json");
    let entry = value_object_entry(ItemAction::Add, vec![invariant("positive")]);
    let (interactor, sink) = interactor_with_catalogues_and_status(
        rules_doc(),
        vec![(path.clone(), catalogue_with_type("Money", entry))],
        Ok(TrackStatus::Done),
    );

    let result = interactor.execute(&command(vec![path]));

    assert!(matches!(
        result,
        Err(domain::tddd::test_obligation::errors::ObligationDeriveError::TrackFrozen {
            status: FrozenTrackStatus::Done
        })
    ));
    assert!(
        sink.saved.lock().unwrap().is_none(),
        "a frozen track must be rejected before its obligations artifact is written"
    );
}

#[test]
fn test_archived_track_is_rejected_before_obligation_artifact_write() {
    let path = PathBuf::from("domain-types.json");
    let entry = value_object_entry(ItemAction::Add, vec![invariant("positive")]);
    let (interactor, sink) = interactor_with_catalogues_and_status(
        rules_doc(),
        vec![(path.clone(), catalogue_with_type("Money", entry))],
        Ok(TrackStatus::Archived),
    );

    let result = interactor.execute(&command(vec![path]));

    assert!(matches!(
        result,
        Err(domain::tddd::test_obligation::errors::ObligationDeriveError::TrackFrozen {
            status: FrozenTrackStatus::Archived
        })
    ));
    assert!(
        sink.saved.lock().unwrap().is_none(),
        "an archived track must be rejected before its obligations artifact is written"
    );
}

#[test]
fn test_track_status_read_failure_prevents_obligation_artifact_write() {
    let path = PathBuf::from("domain-types.json");
    let entry = value_object_entry(ItemAction::Add, vec![invariant("positive")]);
    let (interactor, sink) = interactor_with_catalogues_and_status(
        rules_doc(),
        vec![(path.clone(), catalogue_with_type("Money", entry))],
        Err(TrackStatusReadError("cannot read /private/track/items/metadata.json".to_owned())),
    );

    let result = interactor.execute(&command(vec![path]));

    assert!(matches!(
        result,
        Err(domain::tddd::test_obligation::errors::ObligationDeriveError::TrackStatusRead(
            TrackStatusReadFailureKind::Unavailable
        ))
    ));
    assert!(
        !format!("{result:?}").contains("/private/track/items/metadata.json"),
        "raw track-status read details must not escape the usecase boundary"
    );
    assert!(
        sink.saved.lock().unwrap().is_none(),
        "a status-read failure must prevent the obligations artifact write"
    );
}
