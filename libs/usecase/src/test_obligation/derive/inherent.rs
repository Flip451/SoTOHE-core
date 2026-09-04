//! Obligation projection for `CatalogueDocument::inherent_impls` methods.

use std::collections::BTreeMap;

use domain::tddd::catalogue_v2::{CatalogueDocument, MethodDeclaration, TypeEntry};
use domain::tddd::semantic_verify::{CatalogueEntryRef, CatalogueSectionKey};
use domain::tddd::test_obligation::ids::DiagnosticMessage;
use domain::tddd::test_obligation::obligations::TestObligation;
use domain::tddd::test_obligation::projection::RoleObligationItemsProjector;
use domain::tddd::test_obligation::rules::TestObligationRulesDocument;
use domain::tddd::test_obligation::vocab::TargetEntryRoleKind;

use crate::test_obligation::diag;

use super::{
    anchors_from_spec_refs, declaration_hash, emit_rules, find_data_role_rules,
    resolve_named_type_entry,
};

/// Derives method-axis obligations for Add/Modify methods on inherent impls.
pub(super) fn derive_inherent_impl_obligations(
    rules: &TestObligationRulesDocument,
    projector: &RoleObligationItemsProjector,
    catalogue: &CatalogueDocument,
    file_path: &str,
    out: &mut Vec<TestObligation>,
) -> Result<(), DiagnosticMessage> {
    let mut seen_by_type: BTreeMap<String, BTreeMap<String, MethodDeclaration>> = BTreeMap::new();
    for (name, entry) in catalogue.types() {
        seen_by_type.insert(
            name.as_str().to_owned(),
            entry
                .methods()
                .iter()
                .map(|method| (method.name().as_str().to_owned(), method.clone()))
                .collect(),
        );
    }

    for inherent in catalogue.inherent_impls() {
        let Some((owner_key, type_entry)) =
            resolve_named_type_entry(catalogue, inherent.type_name())?
        else {
            continue;
        };
        let type_name = owner_key.as_str();
        let seen = seen_by_type.entry(type_name.to_owned()).or_default();
        let methods = unique_inherent_methods(type_name, inherent.methods(), seen)?;
        if methods.is_empty() {
            continue;
        }
        let Some(role_rules) = find_data_role_rules(rules, type_entry.role()) else {
            continue;
        };
        let entry_key = owner_key.clone();
        let target = CatalogueEntryRef::new(
            file_path.to_owned(),
            CatalogueSectionKey::Types,
            entry_key.clone(),
        );
        let role_kind = TargetEntryRoleKind::DataRole(type_entry.role().clone());
        let decl_hash = declaration_hash(inherent);
        let anchors = anchors_from_spec_refs(type_entry.spec_refs())?;
        let projection_entry = type_entry_for_inherent_methods(type_entry, methods.clone());
        emit_rules(
            role_rules,
            &entry_key,
            &target,
            &role_kind,
            &decl_hash,
            &anchors,
            &methods,
            out,
            true,
            |axis| projector.data_role_items(&projection_entry, axis),
        )?;
    }
    Ok(())
}

fn unique_inherent_methods(
    type_name: &str,
    methods: &[MethodDeclaration],
    seen: &mut BTreeMap<String, MethodDeclaration>,
) -> Result<Vec<MethodDeclaration>, DiagnosticMessage> {
    let mut unique = Vec::new();
    for method in methods {
        let name = method.name().as_str().to_owned();
        if let Some(existing) = seen.get(&name) {
            if existing != method {
                return Err(diag(&format!(
                    "method '{name}' for type '{type_name}' has inconsistent duplicate declarations"
                )));
            }
            continue;
        }
        seen.insert(name, method.clone());
        unique.push(method.clone());
    }
    Ok(unique)
}

fn type_entry_for_inherent_methods(
    type_entry: &TypeEntry,
    methods: Vec<MethodDeclaration>,
) -> TypeEntry {
    TypeEntry::new(
        type_entry.action(),
        type_entry.role().clone(),
        type_entry.kind().clone(),
        methods,
        type_entry.generics().to_vec(),
        type_entry.where_predicates().to_vec(),
        type_entry.module_path().cloned(),
        type_entry.docs().cloned(),
        type_entry.spec_refs().to_vec(),
        type_entry.informal_grounds().to_vec(),
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

    use domain::tddd::LayerId;
    use domain::tddd::catalogue_v2::roles::{ContractRole, DataRole, FunctionRole, ItemAction};
    use domain::tddd::catalogue_v2::{
        CatalogueDocument, CrateName, InherentImplDeclV2, MethodDeclaration, MethodName,
        ModulePath, SelfReceiver, StructKind, StructShape, TypeEntry, TypeKindV2, TypeRef,
    };
    use domain::tddd::semantic_verify::CatalogueEntryKey;
    use domain::tddd::test_obligation::ids::DiagnosticMessage;
    use domain::tddd::test_obligation::obligations::TestObligation;
    use domain::tddd::test_obligation::projection::RoleObligationItemsProjector;
    use domain::tddd::test_obligation::rules::{
        RoleObligationRules, TestObligationRule, TestObligationRulesDocument,
    };
    use domain::tddd::test_obligation::vocab::{
        TestObligationKind, TestObligationPatternKind, TestObligationPerAxis,
    };
    use domain::{SpecElementId, SpecRef};
    use std::path::PathBuf;

    use super::derive_inherent_impl_obligations;

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

    fn spec_ref(anchor: &str) -> SpecRef {
        SpecRef::new(
            PathBuf::from("track/items/x/spec.json"),
            SpecElementId::try_new(anchor).unwrap(),
        )
    }

    fn method(name: &str, action: ItemAction, refs: Vec<SpecRef>) -> MethodDeclaration {
        MethodDeclaration::new(
            MethodName::new(name).unwrap(),
            Some(SelfReceiver::SharedRef),
            vec![],
            TypeRef::new("()").unwrap(),
            false,
            false,
            vec![],
            vec![],
            refs,
            action,
            None,
        )
    }

    fn domain_service(action: ItemAction, methods: Vec<MethodDeclaration>) -> TypeEntry {
        TypeEntry::new(
            action,
            DataRole::domain_service(),
            TypeKindV2::Struct(StructKind::new(StructShape::Unit, None)),
            methods,
            vec![],
            vec![],
            Some(ModulePath::root()),
            None,
            vec![spec_ref("IN-13")],
            vec![],
        )
    }

    fn catalogue_with(
        entry: TypeEntry,
        inherent_methods: Vec<MethodDeclaration>,
    ) -> CatalogueDocument {
        let mut catalogue = CatalogueDocument::new(
            5,
            CrateName::new("domain").unwrap(),
            LayerId::try_new("domain").unwrap(),
        );
        catalogue.insert_type(CatalogueEntryKey::try_new("Compute".to_owned()).unwrap(), entry);
        catalogue.push_inherent_impl(InherentImplDeclV2::new(
            CatalogueEntryKey::try_new("Compute".to_owned()).unwrap(),
            vec![],
            vec![],
            inherent_methods,
        ));
        catalogue
    }

    fn rules() -> TestObligationRulesDocument {
        let data_roles = DATA_ROLE_NAMES
            .iter()
            .map(|name| {
                let role_rules = if *name == "DomainService" {
                    RoleObligationRules::new(vec![TestObligationRule::new(
                        TestObligationKind::LogicResult,
                        TestObligationPerAxis::Method,
                        None,
                        None,
                    )])
                } else {
                    RoleObligationRules::new(vec![])
                };
                (name.parse::<DataRole>().unwrap(), role_rules)
            })
            .collect();
        let contract_roles = CONTRACT_ROLE_NAMES
            .iter()
            .map(|name| (name.parse::<ContractRole>().unwrap(), RoleObligationRules::new(vec![])))
            .collect();
        TestObligationRulesDocument::try_new(
            data_roles,
            contract_roles,
            vec![
                (FunctionRole::FreeFunction, RoleObligationRules::new(vec![])),
                (FunctionRole::UseCaseFunction, RoleObligationRules::new(vec![])),
            ],
            vec![(TestObligationPatternKind::Typestate, RoleObligationRules::new(vec![]))],
            CONTRACT_ROLE_NAMES
                .iter()
                .map(|name| {
                    (name.parse::<ContractRole>().unwrap(), RoleObligationRules::new(vec![]))
                })
                .collect(),
        )
        .unwrap()
    }

    fn derive(catalogue: &CatalogueDocument) -> Result<Vec<TestObligation>, DiagnosticMessage> {
        let mut out = Vec::new();
        derive_inherent_impl_obligations(
            &rules(),
            &RoleObligationItemsProjector::new(),
            catalogue,
            "domain-types.json",
            &mut out,
        )?;
        Ok(out)
    }

    #[test]
    fn test_inherent_only_add_method_emits_method_obligation() {
        let catalogue = catalogue_with(
            domain_service(ItemAction::Reference, vec![]),
            vec![method("compute", ItemAction::Add, vec![spec_ref("IN-13")])],
        );
        let out = derive(&catalogue).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].id().item_identifier().as_str(), "method:compute");
        assert_eq!(out[0].spec_refs()[0].element_id(), "IN-13");
    }

    #[test]
    fn test_qualified_type_key_and_module_inherent_owner_emit_method_obligation() {
        let mut catalogue = CatalogueDocument::new(
            5,
            CrateName::new("domain").unwrap(),
            LayerId::try_new("domain").unwrap(),
        );
        catalogue.insert_type(
            CatalogueEntryKey::try_new("a::Compute".to_owned()).unwrap(),
            TypeEntry::new(
                ItemAction::Reference,
                DataRole::domain_service(),
                TypeKindV2::Struct(StructKind::new(StructShape::Unit, None)),
                vec![],
                vec![],
                vec![],
                Some(ModulePath::from_segments(vec!["a".to_owned()]).unwrap()),
                None,
                vec![spec_ref("IN-13")],
                vec![],
            ),
        );
        catalogue.push_inherent_impl(InherentImplDeclV2::new(
            CatalogueEntryKey::try_new("a::Compute".to_owned()).unwrap(),
            vec![],
            vec![],
            vec![method("compute", ItemAction::Add, vec![spec_ref("IN-13")])],
        ));

        let out = derive(&catalogue).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].id().item_identifier().as_str(), "method:compute");
    }

    #[test]
    fn test_identical_type_and_inherent_method_emits_once() {
        let shared = method("compute", ItemAction::Add, vec![spec_ref("IN-13")]);
        let catalogue =
            catalogue_with(domain_service(ItemAction::Add, vec![shared.clone()]), vec![shared]);
        let out = derive(&catalogue).unwrap();
        assert!(out.is_empty());
    }

    #[test]
    fn test_conflicting_duplicate_method_fails_closed() {
        let catalogue = catalogue_with(
            domain_service(
                ItemAction::Add,
                vec![method("compute", ItemAction::Add, vec![spec_ref("IN-13")])],
            ),
            vec![method("compute", ItemAction::Modify, vec![spec_ref("IN-13")])],
        );
        assert!(derive(&catalogue).is_err());
    }

    #[test]
    fn test_orphan_inherent_impl_is_external_and_skipped() {
        let mut catalogue = CatalogueDocument::new(
            5,
            CrateName::new("domain").unwrap(),
            LayerId::try_new("domain").unwrap(),
        );
        catalogue.push_inherent_impl(InherentImplDeclV2::new(
            CatalogueEntryKey::try_new("Missing".to_owned()).unwrap(),
            vec![],
            vec![],
            vec![method("compute", ItemAction::Add, vec![spec_ref("IN-13")])],
        ));
        assert!(derive(&catalogue).unwrap().is_empty());
    }

    #[test]
    fn test_ambiguous_inherent_impl_owner_fails_closed() {
        let mut catalogue = CatalogueDocument::new(
            5,
            CrateName::new("domain").unwrap(),
            LayerId::try_new("domain").unwrap(),
        );
        for module in ["alpha", "beta"] {
            catalogue.insert_type(
                CatalogueEntryKey::try_new(format!("{}::Compute", module)).unwrap(),
                TypeEntry::new(
                    ItemAction::Reference,
                    DataRole::domain_service(),
                    TypeKindV2::Struct(StructKind::new(StructShape::Unit, None)),
                    vec![],
                    vec![],
                    vec![],
                    Some(ModulePath::from_segments(vec![module.to_owned()]).unwrap()),
                    None,
                    vec![spec_ref("IN-13")],
                    vec![],
                ),
            );
        }
        catalogue.push_inherent_impl(InherentImplDeclV2::new(
            CatalogueEntryKey::try_new("Compute".to_owned()).unwrap(),
            vec![],
            vec![],
            vec![method("compute", ItemAction::Add, vec![spec_ref("IN-13")])],
        ));

        assert!(derive(&catalogue).is_err());
    }
}
