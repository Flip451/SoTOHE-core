//! Coverage for the rule-independent catalogue TypeRef preflight.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use super::inspect_all_catalogue_type_refs;
use crate::tddd::catalogue_linter::{
    ExtractedTypeRefPath, TypeRefPathExtractionError, TypeRefPathExtractorPort,
};
use crate::tddd::catalogue_v2::composite::{StructKind, StructShape, TypeKindV2};
use crate::tddd::catalogue_v2::entries::{
    AssocConstDecl, AssocTypeDecl, FunctionEntry, InherentImplDeclV2, TraitEntry, TypeEntry,
};
use crate::tddd::catalogue_v2::identifiers::{
    AssocConstName, CrateName, FieldName, FunctionName, FunctionPath, MethodName, ModulePath,
    ParamName, TypeName, TypeRef, VariantName,
};
use crate::tddd::catalogue_v2::methods::{
    BoundOp, MethodDeclaration, MethodGenericParam, ParamDeclaration, WherePredicateDecl,
};
use crate::tddd::catalogue_v2::roles::{
    ContractRole, DataRole, FunctionRole, IdentityAccessor, ItemAction, NonEmptyVec,
};
use crate::tddd::catalogue_v2::traits::TraitImplDeclV2;
use crate::tddd::catalogue_v2::variants::VariantDecl;
use crate::tddd::catalogue_v2::{CatalogueDocument, CatalogueEntryKey};
use crate::tddd::layer_id::LayerId;

struct RecordingExtractor {
    seen: Arc<Mutex<Vec<String>>>,
}

impl TypeRefPathExtractorPort for RecordingExtractor {
    fn extract(
        &self,
        type_ref: &TypeRef,
        _type_parameters: &[ParamName],
        _lifetime_parameters: &[ParamName],
        _const_parameters: &[ParamName],
    ) -> Result<Vec<ExtractedTypeRefPath>, TypeRefPathExtractionError> {
        self.seen
            .lock()
            .expect("recording mutex is not poisoned")
            .push(type_ref.as_str().to_owned());
        Ok(vec![])
    }
}

fn type_ref(name: &str) -> TypeRef {
    TypeRef::new(name.to_owned()).expect("fixture TypeRef is valid")
}

fn method(
    name: &str,
    param: &str,
    returns: &str,
    bound: &str,
    lhs: &str,
    rhs: &str,
) -> MethodDeclaration {
    MethodDeclaration::new(
        MethodName::new(name).expect("fixture method name is valid"),
        None,
        vec![ParamDeclaration::new(ParamName::new("value").unwrap(), type_ref(param))],
        type_ref(returns),
        false,
        false,
        vec![MethodGenericParam {
            name: ParamName::new("M").unwrap(),
            bounds: vec![type_ref(bound)],
        }],
        vec![WherePredicateDecl {
            lhs: type_ref(lhs),
            rhs: vec![type_ref(rhs)],
            operator: BoundOp::Bound,
        }],
        vec![],
        ItemAction::Add,
        None,
    )
}

fn unit_entry(role: DataRole) -> TypeEntry {
    TypeEntry::new(
        ItemAction::Add,
        role,
        TypeKindV2::Struct(StructKind::new(StructShape::Unit, None)),
        vec![],
        vec![],
        vec![],
        Some(ModulePath::root()),
        None,
        vec![],
        vec![],
    )
}

fn identity_accessor(name: &str) -> IdentityAccessor {
    IdentityAccessor::new(MethodName::new(name).unwrap())
}

fn fixture_catalogue() -> CatalogueDocument {
    let mut catalogue = CatalogueDocument::new(
        3,
        CrateName::new("domain").unwrap(),
        LayerId::try_new("domain").unwrap(),
    );
    catalogue.insert_type(
        CatalogueEntryKey::try_new("TypeWithSlots".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::AggregateRoot {
                identity: identity_accessor("id"),
                invariants: vec![],
                exclusive_members: vec![type_ref("role_exclusive")],
                shared_value_objects: vec![type_ref("role_shared")],
                emits: vec![type_ref("role_emits")],
            },
            TypeKindV2::Struct(StructKind::new(
                StructShape::Plain {
                    fields: vec![crate::tddd::catalogue_v2::variants::FieldDecl::new(
                        FieldName::new("value").unwrap(),
                        type_ref("named_field"),
                    )],
                    has_stripped_fields: false,
                },
                None,
            )),
            vec![method(
                "type_method",
                "method_param",
                "method_return",
                "method_bound",
                "method_where_lhs",
                "method_where_rhs",
            )],
            vec![MethodGenericParam {
                name: ParamName::new("T").unwrap(),
                bounds: vec![type_ref("entry_bound")],
            }],
            vec![WherePredicateDecl {
                lhs: type_ref("entry_where_lhs"),
                rhs: vec![type_ref("entry_where_rhs")],
                operator: BoundOp::Bound,
            }],
            Some(ModulePath::root()),
            None,
            vec![],
            vec![],
        ),
    );
    catalogue.insert_type(
        CatalogueEntryKey::try_new("EnumSlots".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::Enum {
                variants: vec![
                    VariantDecl::tuple(
                        VariantName::new("Tuple").unwrap(),
                        vec![type_ref("variant_tuple")],
                    ),
                    VariantDecl::struct_variant(
                        VariantName::new("Struct").unwrap(),
                        vec![crate::tddd::catalogue_v2::variants::FieldDecl::new(
                            FieldName::new("value").unwrap(),
                            type_ref("variant_struct"),
                        )],
                    ),
                ],
            },
            vec![],
            vec![],
            vec![],
            Some(ModulePath::root()),
            None,
            vec![],
            vec![],
        ),
    );
    catalogue.insert_type(
        CatalogueEntryKey::try_new("AliasSlots".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::TypeAlias {
                target: type_ref("alias_target"),
                generics: vec![MethodGenericParam {
                    name: ParamName::new("A").unwrap(),
                    bounds: vec![type_ref("alias_bound")],
                }],
            },
            vec![],
            vec![],
            vec![],
            Some(ModulePath::root()),
            None,
            vec![],
            vec![],
        ),
    );
    for (name, role) in [
        ("DomainServiceSlots", DataRole::DomainService { emits: vec![type_ref("domain_emits")] }),
        ("UseCaseSlots", DataRole::UseCase { handles: vec![type_ref("usecase_handles")] }),
        (
            "EventPolicySlots",
            DataRole::EventPolicy { reacts_to: NonEmptyVec::new(type_ref("event_reacts"), vec![]) },
        ),
    ] {
        catalogue
            .insert_type(CatalogueEntryKey::try_new(name.to_owned()).unwrap(), unit_entry(role));
    }
    catalogue.insert_trait(
        CatalogueEntryKey::try_new("TraitWithSlots".to_owned()).unwrap(),
        TraitEntry::new(
            ItemAction::Add,
            ContractRole::Repository { aggregate: type_ref("trait_role_aggregate") },
            vec![method(
                "trait_method",
                "trait_method_param",
                "trait_method_return",
                "trait_method_bound",
                "trait_method_where_lhs",
                "trait_method_where_rhs",
            )],
            vec![AssocTypeDecl {
                name: TypeName::new("Output").unwrap(),
                bounds: vec![type_ref("assoc_bound")],
                default: Some(type_ref("assoc_default")),
            }],
            vec![AssocConstDecl {
                name: AssocConstName::new("ID").unwrap(),
                ty: type_ref("assoc_const_ty"),
                default_value: None,
            }],
            vec![type_ref("trait_super")],
            vec![MethodGenericParam {
                name: ParamName::new("T").unwrap(),
                bounds: vec![type_ref("trait_bound")],
            }],
            vec![WherePredicateDecl {
                lhs: type_ref("trait_where_lhs"),
                rhs: vec![type_ref("trait_where_rhs")],
                operator: BoundOp::Bound,
            }],
            Some(ModulePath::root()),
            None,
            vec![],
            vec![],
        ),
    );
    catalogue.insert_trait(
        CatalogueEntryKey::try_new("ReferencedTrait".to_owned()).unwrap(),
        TraitEntry::new(
            ItemAction::Reference,
            ContractRole::Repository { aggregate: type_ref("reference_trait_aggregate") },
            vec![method(
                "referenced_trait_method",
                "reference_trait_method_param",
                "reference_trait_method_return",
                "reference_trait_method_bound",
                "reference_trait_method_where_lhs",
                "reference_trait_method_where_rhs",
            )],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            Some(ModulePath::root()),
            None,
            vec![],
            vec![],
        ),
    );
    catalogue.insert_function(
        FunctionPath::at_root(
            CrateName::new("domain").unwrap(),
            FunctionName::new("function_with_slots").unwrap(),
        ),
        FunctionEntry::new(
            ItemAction::Add,
            FunctionRole::FreeFunction,
            vec![ParamDeclaration::new(
                ParamName::new("value").unwrap(),
                type_ref("function_param"),
            )],
            type_ref("function_return"),
            false,
            vec![MethodGenericParam {
                name: ParamName::new("F").unwrap(),
                bounds: vec![type_ref("function_bound")],
            }],
            vec![WherePredicateDecl {
                lhs: type_ref("function_where_lhs"),
                rhs: vec![type_ref("function_where_rhs")],
                operator: BoundOp::Bound,
            }],
            None,
            vec![],
            vec![],
        ),
    );
    catalogue.push_trait_impl(TraitImplDeclV2::from_parts(
        ItemAction::Add,
        type_ref("trait_impl_trait"),
        type_ref("trait_impl_for"),
        vec![MethodGenericParam {
            name: ParamName::new("I").unwrap(),
            bounds: vec![type_ref("trait_impl_bound")],
        }],
        vec![WherePredicateDecl {
            lhs: type_ref("trait_impl_where_lhs"),
            rhs: vec![type_ref("trait_impl_where_rhs")],
            operator: BoundOp::Bound,
        }],
    ));
    catalogue.push_trait_impl(TraitImplDeclV2::from_parts(
        ItemAction::Reference,
        type_ref("reference_impl_trait"),
        type_ref("reference_impl_for"),
        vec![],
        vec![],
    ));
    catalogue.push_inherent_impl(InherentImplDeclV2::new(
        CatalogueEntryKey::try_new("TypeWithSlots".to_owned()).unwrap(),
        vec![MethodGenericParam {
            name: ParamName::new("J").unwrap(),
            bounds: vec![type_ref("inherent_bound")],
        }],
        vec![WherePredicateDecl {
            lhs: type_ref("inherent_where_lhs"),
            rhs: vec![type_ref("inherent_where_rhs")],
            operator: BoundOp::Bound,
        }],
        vec![method(
            "inherent_method",
            "inherent_method_param",
            "inherent_method_return",
            "inherent_method_bound",
            "inherent_method_where_lhs",
            "inherent_method_where_rhs",
        )],
    ));
    catalogue
}

#[test]
fn test_preflight_inspects_every_catalogue_type_ref_slot_without_rules() {
    let catalogue = fixture_catalogue();
    let target_layer = catalogue.layer().clone();
    let mut all_catalogues = BTreeMap::new();
    all_catalogues.insert(target_layer, catalogue);
    let seen = Arc::new(Mutex::new(Vec::new()));

    inspect_all_catalogue_type_refs(&all_catalogues, &RecordingExtractor { seen: seen.clone() })
        .expect("complete preflight fixture should inspect successfully");

    let actual = seen
        .lock()
        .expect("recording mutex is not poisoned")
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let expected = [
        "named_field",
        "entry_bound",
        "entry_where_lhs",
        "entry_where_rhs",
        "role_exclusive",
        "role_shared",
        "role_emits",
        "method_param",
        "method_return",
        "method_bound",
        "method_where_lhs",
        "method_where_rhs",
        "variant_tuple",
        "variant_struct",
        "alias_target",
        "alias_bound",
        "domain_emits",
        "usecase_handles",
        "event_reacts",
        "trait_role_aggregate",
        "trait_method_param",
        "trait_method_return",
        "trait_method_bound",
        "trait_method_where_lhs",
        "trait_method_where_rhs",
        "reference_trait_aggregate",
        "reference_trait_method_param",
        "reference_trait_method_return",
        "reference_trait_method_bound",
        "reference_trait_method_where_lhs",
        "reference_trait_method_where_rhs",
        "trait_super",
        "trait_bound",
        "trait_where_lhs",
        "trait_where_rhs",
        "assoc_bound",
        "assoc_default",
        "assoc_const_ty",
        "function_param",
        "function_return",
        "function_bound",
        "function_where_lhs",
        "function_where_rhs",
        "trait_impl_trait",
        "trait_impl_for",
        "trait_impl_bound",
        "trait_impl_where_lhs",
        "trait_impl_where_rhs",
        "reference_impl_trait",
        "reference_impl_for",
        "TypeWithSlots",
        "inherent_bound",
        "inherent_where_lhs",
        "inherent_where_rhs",
        "inherent_method_param",
        "inherent_method_return",
        "inherent_method_bound",
        "inherent_method_where_lhs",
        "inherent_method_where_rhs",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<BTreeSet<_>>();

    assert_eq!(actual, expected);
}
