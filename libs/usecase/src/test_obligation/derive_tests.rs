//! Unit tests for [`super::DeriveTestObligationsInteractor`] (T016).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use domain::tddd::LayerId;
use domain::tddd::catalogue_v2::catalogue_impl_signals_ports::{
    CatalogueDocumentLoaderError, CatalogueDocumentLoaderPort,
};
use domain::tddd::catalogue_v2::roles::{
    ContractRole, DataRole, FunctionRole, InvariantDecl, InvariantPredicate, ItemAction,
};
use domain::tddd::catalogue_v2::{
    CatalogueDocument, CrateName, DocString, InvariantName, MethodDeclaration, MethodName,
    ModulePath, SelfReceiver, StructKind, StructShape, TraitEntry, TraitImplDeclV2, TraitName,
    TypeEntry, TypeKindV2, TypeName, TypeRef,
};
use domain::tddd::test_obligation::errors::{ArtifactCodecError, TestObligationRulesLoadError};
use domain::tddd::test_obligation::ids::DiagnosticMessage;
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
use domain::{SpecDocument, SpecDocumentLoadError, SpecElementId, SpecRef, SpecScope, TrackId};

use domain::SpecDocumentLoaderPort;

use super::{
    DeriveTestObligationsApplicationService, DeriveTestObligationsCommand,
    DeriveTestObligationsInteractor,
};
use crate::test_obligation::TestObligationCatalogueCommandInput;

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
}

impl CatalogueDocumentLoaderPort for StubCatalogue {
    fn load(&self, path: &Path) -> Result<CatalogueDocument, CatalogueDocumentLoaderError> {
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
    let data_roles: Vec<(DataRole, RoleObligationRules)> = DATA_ROLE_NAMES
        .iter()
        .map(|name| (name.parse::<DataRole>().unwrap(), RoleObligationRules::new(vec![])))
        .collect();
    let contract_roles: Vec<(ContractRole, RoleObligationRules)> = CONTRACT_ROLE_NAMES
        .iter()
        .map(|name| {
            let rules = if *name == "SecondaryPort" {
                RoleObligationRules::new(vec![rule(
                    TestObligationKind::Contract,
                    TestObligationPerAxis::TraitMethod,
                    None,
                )])
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

fn invariant(name: &str) -> InvariantDecl {
    InvariantDecl::new(
        InvariantName::new(name).unwrap(),
        InvariantPredicate::SelfMethod(MethodName::new(format!("is_{name}")).unwrap()),
    )
}

fn spec_ref(anchor: &str) -> SpecRef {
    SpecRef::new(PathBuf::from("track/items/x/spec.json"), SpecElementId::try_new(anchor).unwrap())
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
    doc.insert_type(TypeName::new(name).unwrap(), entry);
    doc
}

fn empty_catalogue(crate_name: &str, layer: &str) -> CatalogueDocument {
    CatalogueDocument::new(5, CrateName::new(crate_name).unwrap(), LayerId::try_new(layer).unwrap())
}

fn trait_entry(role: ContractRole, anchor: &str) -> TraitEntry {
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
        vec![spec_ref(anchor)],
        vec![],
    )
}

fn interactor_with_catalogues(
    rules: TestObligationRulesDocument,
    catalogues: Vec<(PathBuf, CatalogueDocument)>,
) -> (DeriveTestObligationsInteractor, Arc<RecordingObligations>) {
    let docs: HashMap<PathBuf, CatalogueDocument> = catalogues.into_iter().collect();
    let obligations = Arc::new(RecordingObligations::default());
    let interactor = DeriveTestObligationsInteractor::new(
        Arc::new(StubRules { doc: rules }),
        Arc::clone(&obligations) as Arc<dyn ObligationsArtifactPort + Send + Sync>,
        Arc::new(StubSpec),
        Arc::new(StubCatalogue { docs }),
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
        TraitName::new("ObligationsArtifactPort").unwrap(),
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
                    None,
                ),
                MethodDeclaration::new(
                    MethodName::new("save").unwrap(),
                    Some(SelfReceiver::SharedRef),
                    vec![],
                    TypeRef::new("Result<(), DiagnosticMessage>").unwrap(),
                    false,
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
        TraitName::new("SpecDocumentLoaderPort").unwrap(),
        TraitEntry::new(
            ItemAction::Add,
            ContractRole::SecondaryPort,
            vec![MethodDeclaration::new(
                MethodName::new("load").unwrap(),
                Some(SelfReceiver::SharedRef),
                vec![],
                TypeRef::new("Result<SpecDocument, SpecDocumentLoadError>").unwrap(),
                false,
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

fn obligations_artifact_port_catalogue(docs: Option<DocString>) -> CatalogueDocument {
    let mut catalogue = empty_catalogue("domain", "domain");
    catalogue.insert_trait(
        TraitName::new("ObligationsArtifactPort").unwrap(),
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
                    None,
                ),
                MethodDeclaration::new(
                    MethodName::new("save").unwrap(),
                    Some(SelfReceiver::SharedRef),
                    vec![],
                    TypeRef::new("Result<(), DiagnosticMessage>").unwrap(),
                    false,
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
    assert!(matches!(obligation.target_role(), TargetEntryRoleKind::DataRole(_)));
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
        TraitName::new("MyPort").unwrap(),
        TraitEntry::new(
            ItemAction::Add,
            ContractRole::SecondaryPort,
            vec![],
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
        TraitName::new("Display").unwrap(),
        TraitEntry::new(
            ItemAction::Add,
            ContractRole::SecondaryPort,
            vec![],
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
fn test_fs_spec_document_loader_trait_impl_derives_its_secondary_port_obligation() {
    // IN-07 / IN-08: the real filesystem loader declaration resolves the
    // domain port's SecondaryPort role across the catalogue boundary.
    let domain_path = PathBuf::from("domain-types.json");
    let infrastructure_path = PathBuf::from("infrastructure-types.json");
    let mut domain = empty_catalogue("domain", "domain");
    domain.insert_trait(
        TraitName::new("SpecDocumentLoaderPort").unwrap(),
        trait_entry(ContractRole::SecondaryPort, "IN-07"),
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
fn test_trait_impl_unsupported_axis_yields_zero_even_with_minimum() {
    let path = PathBuf::from("infrastructure-types.json");
    let mut doc = CatalogueDocument::new(
        5,
        CrateName::new("infrastructure").unwrap(),
        LayerId::try_new("infrastructure").unwrap(),
    );
    doc.insert_trait(
        TraitName::new("MyPort").unwrap(),
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
        TraitName::new("SharedPort").unwrap(),
        trait_entry(ContractRole::SecondaryPort, "IN-99"),
    );

    let mut usecase_doc = empty_catalogue("usecase", "usecase");
    usecase_doc.insert_trait(
        TraitName::new("SharedPort").unwrap(),
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
