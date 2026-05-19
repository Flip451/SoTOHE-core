//! Tests for [`catalogue_to_extended_crate_codec`] (split out to keep the main module under the 200-400 line guideline).

use domain::tddd::CatalogueToExtendedCratePort;
use domain::tddd::LayerId;
use domain::tddd::catalogue_v2::composite::TypeKindV2;
use domain::tddd::catalogue_v2::entries::{TraitEntry, TypeEntry};
use domain::tddd::catalogue_v2::methods::{
    MethodDeclaration, MethodGenericParam, ParamDeclaration,
};
use domain::tddd::catalogue_v2::roles::{ContractRole, DataRole, ItemAction, SelfReceiver};
use domain::tddd::catalogue_v2::traits::TraitImplDeclV2;
use domain::tddd::catalogue_v2::variants::{FieldDecl, VariantDecl};
use domain::tddd::catalogue_v2::{
    CatalogueDocument, CrateName, FieldName, MethodName, ModulePath, ParamName, TraitName,
    TypeName, TypeRef, VariantName,
};
use rustdoc_types::{Id, ItemEnum, Type};

use super::*;
use crate::tddd::type_ref_parser::UNRESOLVED_CRATE_ID;

fn make_doc(crate_name: &str) -> CatalogueDocument {
    CatalogueDocument::new(
        2,
        CrateName::new(crate_name).unwrap(),
        LayerId::try_new("domain").expect("static valid"),
    )
}

// -----------------------------------------------------------------------
// Error path: AmbiguousIdentifier
// -----------------------------------------------------------------------

#[test]
fn test_encode_returns_ambiguous_identifier_when_type_and_trait_share_name() {
    // A type named "Foo" and a trait named "Foo" in the same catalogue collide
    // in the short-name index, triggering AmbiguousIdentifier.
    let mut doc = make_doc("domain");
    doc.types.insert(
        TypeName::new("Foo").unwrap(),
        TypeEntry {
            action: ItemAction::Add,
            role: DataRole::ValueObject,
            kind: TypeKindV2::Enum { variants: vec![] },
            methods: vec![],
            trait_impls: vec![],
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        },
    );
    doc.traits.insert(
        TraitName::new("Foo").unwrap(),
        TraitEntry {
            action: ItemAction::Add,
            role: ContractRole::SpecificationPort,
            methods: vec![],
            supertrait_bounds: vec![],
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        },
    );

    let result = CatalogueToExtendedCrateCodec::new().encode(doc);
    assert!(result.is_err(), "expected error due to name collision between type Foo and trait Foo");
    // The domain error should be AmbiguousTypeName (converted from AmbiguousIdentifier).
    let err = result.unwrap_err();
    assert!(
        matches!(err, domain::tddd::NewTypeGraphCodecError::AmbiguousTypeName(_)),
        "expected AmbiguousTypeName error, got: {err:?}"
    );
}

// -----------------------------------------------------------------------
// Error path: InvalidTypeRef
// -----------------------------------------------------------------------

#[test]
fn test_encode_returns_invalid_type_ref_for_unparseable_field_type() {
    // A struct field with a TypeRef that syn cannot parse triggers InvalidTypeRef.
    let mut doc = make_doc("domain");
    doc.types.insert(
        TypeName::new("BadType").unwrap(),
        TypeEntry {
            action: ItemAction::Add,
            role: DataRole::ValueObject,
            kind: TypeKindV2::PlainStruct {
                fields: vec![FieldDecl::new(
                    FieldName::new("value").unwrap(),
                    // "42invalid" is not a valid Rust type expression.
                    TypeRef::new("String").unwrap(),
                )],
                has_stripped_fields: false,
                typestate: None,
            },
            methods: vec![MethodDeclaration {
                name: MethodName::new("get_value").unwrap(),
                receiver: Some(SelfReceiver::SharedRef),
                params: vec![],
                // TypeRef::new accepts any non-empty string; the codec rejects it at syn parse time.
                returns: TypeRef::new("42invalid").unwrap(),
                is_async: false,
                has_default_impl: false,
                generics: vec![],
                where_predicates: vec![],
                docs: None,
            }],
            trait_impls: vec![],
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        },
    );

    let result = CatalogueToExtendedCrateCodec::new().encode(doc);
    assert!(result.is_err(), "expected InvalidTypeRef error for unparseable return type");
    let err = result.unwrap_err();
    assert!(
        matches!(err, domain::tddd::NewTypeGraphCodecError::InvalidTypeRef(_)),
        "expected InvalidTypeRef error, got: {err:?}"
    );
}

// -----------------------------------------------------------------------
// AC-05: inline → id-ref conversion — struct fields
// -----------------------------------------------------------------------

#[test]
fn test_encode_struct_fields_are_promoted_to_struct_field_items() {
    let mut doc = make_doc("domain");
    doc.types.insert(
        TypeName::new("User").unwrap(),
        TypeEntry {
            action: ItemAction::Add,
            role: DataRole::ValueObject,
            kind: TypeKindV2::PlainStruct {
                fields: vec![
                    FieldDecl::new(
                        FieldName::new("email").unwrap(),
                        TypeRef::new("String").unwrap(),
                    ),
                    FieldDecl::new(FieldName::new("id").unwrap(), TypeRef::new("u32").unwrap()),
                ],
                has_stripped_fields: false,
                typestate: None,
            },
            methods: vec![],
            trait_impls: vec![],
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        },
    );

    let ec = CatalogueToExtendedCrateCodec::new().encode(doc).unwrap();
    let struct_field_count = ec
        .krate()
        .index
        .values()
        .filter(|item| matches!(item.inner, ItemEnum::StructField(_)))
        .count();
    assert_eq!(struct_field_count, 2);
}

// -----------------------------------------------------------------------
// AC-05: inline → id-ref conversion — enum variants
// -----------------------------------------------------------------------

#[test]
fn test_encode_enum_variants_are_promoted_to_variant_items() {
    let mut doc = make_doc("domain");
    doc.types.insert(
        TypeName::new("ItemAction").unwrap(),
        TypeEntry {
            action: ItemAction::Add,
            role: DataRole::ValueObject,
            kind: TypeKindV2::Enum {
                variants: vec![
                    VariantDecl::unit(VariantName::new("Add").unwrap()),
                    VariantDecl::tuple(
                        VariantName::new("Error").unwrap(),
                        vec![TypeRef::new("String").unwrap()],
                    ),
                ],
            },
            methods: vec![],
            trait_impls: vec![],
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        },
    );

    let ec = CatalogueToExtendedCrateCodec::new().encode(doc).unwrap();
    let variant_count =
        ec.krate().index.values().filter(|item| matches!(item.inner, ItemEnum::Variant(_))).count();
    assert_eq!(variant_count, 2);
}

// -----------------------------------------------------------------------
// AC-05: 1 type = 1 Inherent Impl block
// -----------------------------------------------------------------------

#[test]
fn test_encode_type_with_methods_produces_single_inherent_impl_block() {
    let mut doc = make_doc("domain");
    doc.types.insert(
        TypeName::new("Email").unwrap(),
        TypeEntry {
            action: ItemAction::Add,
            role: DataRole::ValueObject,
            kind: TypeKindV2::PlainStruct {
                fields: vec![],
                has_stripped_fields: false,
                typestate: None,
            },
            methods: vec![
                MethodDeclaration::new(
                    MethodName::new("new").unwrap(),
                    None,
                    vec![],
                    TypeRef::new("Self").unwrap(),
                    false,
                    None,
                ),
                MethodDeclaration::new(
                    MethodName::new("as_str").unwrap(),
                    Some(SelfReceiver::SharedRef),
                    vec![],
                    TypeRef::new("str").unwrap(),
                    false,
                    None,
                ),
            ],
            trait_impls: vec![],
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        },
    );

    let ec = CatalogueToExtendedCrateCodec::new().encode(doc).unwrap();
    let krate = ec.krate();

    // Exactly 1 inherent Impl block.
    let inherent_impl_count = krate
        .index
        .values()
        .filter(|item| matches!(&item.inner, ItemEnum::Impl(i) if i.trait_.is_none()))
        .count();
    assert_eq!(inherent_impl_count, 1, "expected 1 inherent Impl block");

    // 2 Function items for the methods.
    let fn_count =
        krate.index.values().filter(|item| matches!(item.inner, ItemEnum::Function(_))).count();
    assert_eq!(fn_count, 2);
}

// -----------------------------------------------------------------------
// AC-05: Crate.paths — module_path included
// -----------------------------------------------------------------------

#[test]
fn test_encode_paths_includes_module_path_segments() {
    let mut doc = make_doc("domain");
    doc.types.insert(
        TypeName::new("Draft").unwrap(),
        TypeEntry {
            action: ItemAction::Add,
            role: DataRole::ValueObject,
            kind: TypeKindV2::PlainStruct {
                fields: vec![],
                has_stripped_fields: false,
                typestate: None,
            },
            methods: vec![],
            trait_impls: vec![],
            module_path: ModulePath::from_segments(vec!["review".to_string()]).unwrap(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        },
    );

    let ec = CatalogueToExtendedCrateCodec::new().encode(doc).unwrap();
    let summary = ec
        .krate()
        .paths
        .values()
        .find(|s| s.path.last().map(|n| n == "Draft").unwrap_or(false))
        .expect("Draft not found in paths");
    assert_eq!(summary.path, vec!["domain", "review", "Draft"]);
}

#[test]
fn test_encode_paths_crate_root_type_has_two_segment_path() {
    let mut doc = make_doc("domain");
    doc.types.insert(
        TypeName::new("UserId").unwrap(),
        TypeEntry {
            action: ItemAction::Add,
            role: DataRole::ValueObject,
            kind: TypeKindV2::PlainStruct {
                fields: vec![],
                has_stripped_fields: false,
                typestate: None,
            },
            methods: vec![],
            trait_impls: vec![],
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        },
    );

    let ec = CatalogueToExtendedCrateCodec::new().encode(doc).unwrap();
    let summary = ec
        .krate()
        .paths
        .values()
        .find(|s| s.path.last().map(|n| n == "UserId").unwrap_or(false))
        .expect("UserId not found in paths");
    assert_eq!(summary.path, vec!["domain", "UserId"]);
}

// -----------------------------------------------------------------------
// AC-06: TypeRef generics parse
// -----------------------------------------------------------------------

#[test]
fn test_encode_field_with_generic_type_ref_creates_resolved_path_with_args() {
    let mut doc = make_doc("domain");
    doc.types.insert(
        TypeName::new("Cart").unwrap(),
        TypeEntry {
            action: ItemAction::Add,
            role: DataRole::ValueObject,
            kind: TypeKindV2::PlainStruct {
                fields: vec![FieldDecl::new(
                    FieldName::new("items").unwrap(),
                    TypeRef::new("Vec<String>").unwrap(),
                )],
                has_stripped_fields: false,
                typestate: None,
            },
            methods: vec![],
            trait_impls: vec![],
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        },
    );

    let ec = CatalogueToExtendedCrateCodec::new().encode(doc).unwrap();
    let field_with_args = ec.krate().index.values().find(|item| {
        matches!(&item.inner, ItemEnum::StructField(Type::ResolvedPath(p)) if p.path.contains("Vec") && p.args.is_some())
    });
    assert!(field_with_args.is_some(), "expected Vec<String> field with generic args");
}

// -----------------------------------------------------------------------
// AC-06: std prelude auto-resolution
// -----------------------------------------------------------------------

#[test]
fn test_encode_std_prelude_type_creates_std_external_crate_entry() {
    let mut doc = make_doc("domain");
    doc.types.insert(
        TypeName::new("Foo").unwrap(),
        TypeEntry {
            action: ItemAction::Add,
            role: DataRole::ValueObject,
            kind: TypeKindV2::PlainStruct {
                fields: vec![FieldDecl::new(
                    FieldName::new("name").unwrap(),
                    TypeRef::new("String").unwrap(),
                )],
                has_stripped_fields: false,
                typestate: None,
            },
            methods: vec![],
            trait_impls: vec![],
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        },
    );

    let ec = CatalogueToExtendedCrateCodec::new().encode(doc).unwrap();
    let has_std = ec.krate().external_crates.values().any(|e| e.name == "std");
    assert!(has_std, "expected 'std' in external_crates");
}

// -----------------------------------------------------------------------
// AC-06: unresolved marker for undeclared types
// -----------------------------------------------------------------------

#[test]
fn test_encode_undeclared_type_ref_field_gets_unresolved_marker_id() {
    let mut doc = make_doc("domain");
    doc.types.insert(
        TypeName::new("Foo").unwrap(),
        TypeEntry {
            action: ItemAction::Add,
            role: DataRole::ValueObject,
            kind: TypeKindV2::PlainStruct {
                fields: vec![FieldDecl::new(
                    FieldName::new("error").unwrap(),
                    TypeRef::new("DomainError").unwrap(),
                )],
                has_stripped_fields: false,
                typestate: None,
            },
            methods: vec![],
            trait_impls: vec![],
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        },
    );

    let ec = CatalogueToExtendedCrateCodec::new().encode(doc).unwrap();
    let unresolved = ec.krate().index.values().find(|item| {
        matches!(&item.inner, ItemEnum::StructField(Type::ResolvedPath(p)) if p.id == Id(UNRESOLVED_CRATE_ID))
    });
    assert!(unresolved.is_some(), "expected unresolved marker field item");
}

// -----------------------------------------------------------------------
// AC-05: item_actions populated
// -----------------------------------------------------------------------

#[test]
fn test_encode_item_actions_contains_declared_action() {
    let mut doc = make_doc("domain");
    doc.types.insert(
        TypeName::new("Email").unwrap(),
        TypeEntry {
            action: ItemAction::Modify,
            role: DataRole::ValueObject,
            kind: TypeKindV2::PlainStruct {
                fields: vec![],
                has_stripped_fields: false,
                typestate: None,
            },
            methods: vec![],
            trait_impls: vec![],
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        },
    );

    let ec = CatalogueToExtendedCrateCodec::new().encode(doc).unwrap();
    let has_modify = ec.item_actions().values().any(|a| *a == ItemAction::Modify);
    assert!(has_modify);
}

// -----------------------------------------------------------------------
// AC-05: external_crates from TraitImplDeclV2::origin_crate
// -----------------------------------------------------------------------

#[test]
fn test_encode_trait_impl_origin_crate_registered_in_external_crates() {
    let mut doc = make_doc("domain");
    doc.types.insert(
        TypeName::new("Foo").unwrap(),
        TypeEntry {
            action: ItemAction::Add,
            role: DataRole::ValueObject,
            kind: TypeKindV2::PlainStruct {
                fields: vec![],
                has_stripped_fields: false,
                typestate: None,
            },
            methods: vec![],
            trait_impls: vec![TraitImplDeclV2::new(
                TraitName::new("Serialize").unwrap(),
                CrateName::new("serde").unwrap(),
            )],
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        },
    );

    let ec = CatalogueToExtendedCrateCodec::new().encode(doc).unwrap();
    let has_serde = ec.krate().external_crates.values().any(|e| e.name == "serde");
    assert!(has_serde, "expected 'serde' in external_crates");
}

// -----------------------------------------------------------------------
// AC-05: trait entry encoding
// -----------------------------------------------------------------------

#[test]
fn test_encode_trait_entry_produces_trait_item() {
    let mut doc = make_doc("domain");
    doc.traits.insert(
        TraitName::new("UserRepository").unwrap(),
        TraitEntry {
            action: ItemAction::Add,
            role: ContractRole::SecondaryPort,
            methods: vec![],
            supertrait_bounds: vec![],
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        },
    );

    let ec = CatalogueToExtendedCrateCodec::new().encode(doc).unwrap();
    let trait_item = ec.krate().index.values().find(|item| {
        matches!(&item.inner, ItemEnum::Trait(_)) && item.name.as_deref() == Some("UserRepository")
    });
    assert!(trait_item.is_some(), "expected Trait item for UserRepository");
}

// -----------------------------------------------------------------------
// Type alias
// -----------------------------------------------------------------------

#[test]
fn test_encode_type_alias_produces_type_alias_item() {
    let mut doc = make_doc("domain");
    doc.types.insert(
        TypeName::new("UserResult").unwrap(),
        TypeEntry {
            action: ItemAction::Add,
            role: DataRole::ValueObject,
            kind: TypeKindV2::TypeAlias {
                target: TypeRef::new("Result<User, DomainError>").unwrap(),
            },
            methods: vec![],
            trait_impls: vec![],
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        },
    );

    let ec = CatalogueToExtendedCrateCodec::new().encode(doc).unwrap();
    let alias_item = ec.krate().index.values().find(|item| {
        matches!(&item.inner, ItemEnum::TypeAlias(_)) && item.name.as_deref() == Some("UserResult")
    });
    assert!(alias_item.is_some(), "expected TypeAlias item for UserResult");
}

// -----------------------------------------------------------------------
// Empty catalogue
// -----------------------------------------------------------------------

#[test]
fn test_encode_empty_catalogue_produces_root_module() {
    let doc = make_doc("domain");
    let ec = CatalogueToExtendedCrateCodec::new().encode(doc).unwrap();
    assert!(ec.krate().index.contains_key(&Id(0)), "expected root Id(0)");
}

// -----------------------------------------------------------------------
// generic_args in TraitImplDeclV2 → trait_path_str includes <X>
// -----------------------------------------------------------------------

#[test]
fn test_encode_trait_impl_with_generic_args_produces_impl_with_parameterised_trait_path() {
    // When `generic_args` is Some, the Impl item's trait path must be
    // `"From<CatalogueLoaderError>"` so that `build_impl_identity_map` produces
    // the key `"RenderContractMapError: From<CatalogueLoaderError>"`, matching
    // the C-side rustdoc key exactly.
    let mut doc = make_doc("usecase");
    doc.types.insert(
        TypeName::new("RenderContractMapError").unwrap(),
        TypeEntry {
            action: ItemAction::Modify,
            role: DataRole::ErrorType,
            kind: TypeKindV2::Enum { variants: vec![] },
            methods: vec![],
            trait_impls: vec![
                TraitImplDeclV2::new_with_generic_args(
                    TraitName::new("From").unwrap(),
                    CrateName::new("core").unwrap(),
                    "CatalogueLoaderError".to_string(),
                )
                .unwrap(),
                TraitImplDeclV2::new_with_generic_args(
                    TraitName::new("From").unwrap(),
                    CrateName::new("core").unwrap(),
                    "ContractMapWriterError".to_string(),
                )
                .unwrap(),
            ],
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        },
    );

    let ec = CatalogueToExtendedCrateCodec::new().encode(doc).unwrap();
    let krate = ec.krate();

    // Collect trait impl paths from all Impl items that have a trait.
    let trait_paths: Vec<String> = krate
        .index
        .values()
        .filter_map(|item| {
            if let ItemEnum::Impl(impl_) = &item.inner {
                impl_.trait_.as_ref().map(|tp| tp.path.clone())
            } else {
                None
            }
        })
        .collect();

    // `core::From` with generic_args: emit the fully-qualified `core::convert::From`
    // path with generic args appended.  `build_impl_identity_map` resolves C-side via
    // `krate.paths`, obtaining `"core::convert::From"` as the canonical qualified form.
    // S-side uses `core_canonical_path("From")` = `"core::convert::From"` so both
    // sides produce the same identity key.
    assert!(
        trait_paths.iter().any(|p| p == "core::convert::From<CatalogueLoaderError>"),
        "expected impl trait path 'core::convert::From<CatalogueLoaderError>', got: {trait_paths:?}"
    );
    assert!(
        trait_paths.iter().any(|p| p == "core::convert::From<ContractMapWriterError>"),
        "expected impl trait path 'core::convert::From<ContractMapWriterError>', got: {trait_paths:?}"
    );
}

#[test]
fn test_encode_trait_impl_without_generic_args_produces_impl_with_qualified_core_trait_path() {
    // When `generic_args` is None and `origin_crate` is `"core"`, the impl trait path
    // must be the fully-qualified canonical path (`"core::convert::From"` not bare `"From"`).
    // `build_impl_identity_map` uses `krate.paths` to resolve C-side trait paths to
    // their canonical qualified form (e.g. `"core::convert::From"`) so S-side must
    // emit the same form to avoid identity-key mismatches.
    let mut doc = make_doc("domain");
    doc.types.insert(
        TypeName::new("SomeError").unwrap(),
        TypeEntry {
            action: ItemAction::Add,
            role: DataRole::ErrorType,
            kind: TypeKindV2::Enum { variants: vec![] },
            methods: vec![],
            trait_impls: vec![TraitImplDeclV2::new(
                TraitName::new("From").unwrap(),
                CrateName::new("core").unwrap(),
            )],
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        },
    );

    let ec = CatalogueToExtendedCrateCodec::new().encode(doc).unwrap();
    let krate = ec.krate();

    let trait_paths: Vec<String> = krate
        .index
        .values()
        .filter_map(|item| {
            if let ItemEnum::Impl(impl_) = &item.inner {
                impl_.trait_.as_ref().map(|tp| tp.path.clone())
            } else {
                None
            }
        })
        .collect();

    assert!(
        trait_paths.iter().any(|p| p == "core::convert::From"),
        "expected qualified 'core::convert::From' trait path when generic_args is None, got: {trait_paths:?}"
    );
}

// -----------------------------------------------------------------------
// Struct variant with named fields
// -----------------------------------------------------------------------

#[test]
fn test_encode_enum_struct_variant_produces_named_struct_field_items() {
    let mut doc = make_doc("domain");
    doc.types.insert(
        TypeName::new("ParseError").unwrap(),
        TypeEntry {
            action: ItemAction::Add,
            role: DataRole::ErrorType,
            kind: TypeKindV2::Enum {
                variants: vec![VariantDecl::struct_variant(
                    VariantName::new("InvalidToken").unwrap(),
                    vec![FieldDecl::new(
                        FieldName::new("message").unwrap(),
                        TypeRef::new("String").unwrap(),
                    )],
                )],
            },
            methods: vec![],
            trait_impls: vec![],
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        },
    );

    let ec = CatalogueToExtendedCrateCodec::new().encode(doc).unwrap();
    let struct_variant = ec.krate().index.values().find(|item| {
        if let ItemEnum::Variant(v) = &item.inner {
            matches!(&v.kind, VariantKind::Struct { fields, .. } if !fields.is_empty())
        } else {
            false
        }
    });
    assert!(struct_variant.is_some(), "expected struct Variant with fields");
}

// -----------------------------------------------------------------------
// AC-method-generics: method generic params are encoded as Type::Generic
// -----------------------------------------------------------------------

/// A method with `generics: [{ name: "T", bounds: ["Into<String>"] }]` and
/// a parameter of type `"T"` must encode that parameter as `Type::Generic("T")`,
/// not as a `ResolvedPath`.  Rustdoc emits `Type::Generic` for method-level
/// generic type parameters, so the S-side must match.
#[test]
fn test_encode_method_generic_param_type_emits_type_generic() {
    let mut doc = make_doc("domain");
    let mut method = MethodDeclaration::new(
        MethodName::new("set_value").unwrap(),
        Some(SelfReceiver::ExclusiveRef),
        vec![ParamDeclaration::new(ParamName::new("value").unwrap(), TypeRef::new("T").unwrap())],
        TypeRef::new("()").unwrap(),
        false,
        None,
    );
    method.generics = vec![MethodGenericParam {
        name: ParamName::new("T").unwrap(),
        bounds: vec![TypeRef::new("Into<String>").unwrap()],
    }];
    doc.types.insert(
        TypeName::new("ValueHolder").unwrap(),
        TypeEntry {
            action: ItemAction::Add,
            role: DataRole::ValueObject,
            kind: TypeKindV2::PlainStruct {
                fields: vec![],
                has_stripped_fields: false,
                typestate: None,
            },
            methods: vec![method],
            trait_impls: vec![],
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        },
    );

    let ec = CatalogueToExtendedCrateCodec::new().encode(doc).unwrap();
    let krate = ec.krate();
    // Find the method Function item (set_value).
    let fn_item = krate.index.values().find(|item| {
        item.name.as_deref() == Some("set_value") && matches!(item.inner, ItemEnum::Function(_))
    });
    assert!(fn_item.is_some(), "expected Function item for set_value");
    let ItemEnum::Function(ref f) = fn_item.unwrap().inner else { panic!("expected Function") };
    // The first input is "self" (ExclusiveRef); the second is the "value: T" param.
    let value_param = f.sig.inputs.iter().find(|(name, _)| name == "value");
    assert!(value_param.is_some(), "expected input named 'value'");
    let (_, ty) = value_param.unwrap();
    assert!(
        matches!(ty, Type::Generic(g) if g == "T"),
        "expected Type::Generic(\"T\") for generic param type, got: {ty:?}"
    );
}

// -----------------------------------------------------------------------
// ADR 0248 D13: per-method `has_body` from `has_default_impl` (Gap 1)
// -----------------------------------------------------------------------

/// A trait method declared with `has_default_impl: true` (provided default impl)
/// must encode to `rustdoc_types::Function.has_body = true` so that A-side and
/// C-side fingerprints both emit `;body` and `structurally_equal` returns true.
#[test]
fn test_encode_trait_method_with_has_default_impl_true_produces_has_body_true() {
    let mut doc = make_doc("usecase");
    let mut method = MethodDeclaration::new(
        MethodName::new("describe").unwrap(),
        Some(SelfReceiver::SharedRef),
        vec![],
        TypeRef::new("String").unwrap(),
        false,
        None,
    );
    method.has_default_impl = true;
    doc.traits.insert(
        TraitName::new("Describable").unwrap(),
        TraitEntry {
            action: ItemAction::Add,
            role: ContractRole::SpecificationPort,
            methods: vec![method],
            supertrait_bounds: vec![],
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        },
    );

    let ec = CatalogueToExtendedCrateCodec::new().encode(doc).unwrap();
    let fn_item = ec
        .krate()
        .index
        .values()
        .find(|item| {
            item.name.as_deref() == Some("describe") && matches!(item.inner, ItemEnum::Function(_))
        })
        .expect("expected Function item for describe");
    let ItemEnum::Function(ref f) = fn_item.inner else { panic!("expected Function") };
    assert!(
        f.has_body,
        "trait method with has_default_impl=true must encode has_body=true (ADR 0248 D13)"
    );
}

/// A trait method declared with `has_default_impl: false` (required / abstract)
/// must encode to `rustdoc_types::Function.has_body = false` so that A-side and
/// C-side fingerprints both emit `;abstract`.
#[test]
fn test_encode_trait_method_with_has_default_impl_false_produces_has_body_false() {
    let mut doc = make_doc("usecase");
    let method = MethodDeclaration::new(
        MethodName::new("required_op").unwrap(),
        Some(SelfReceiver::SharedRef),
        vec![],
        TypeRef::new("()").unwrap(),
        false,
        None,
    );
    // has_default_impl defaults to false via MethodDeclaration::new.
    assert!(!method.has_default_impl);
    doc.traits.insert(
        TraitName::new("RequiredOps").unwrap(),
        TraitEntry {
            action: ItemAction::Add,
            role: ContractRole::SpecificationPort,
            methods: vec![method],
            supertrait_bounds: vec![],
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        },
    );

    let ec = CatalogueToExtendedCrateCodec::new().encode(doc).unwrap();
    let fn_item = ec
        .krate()
        .index
        .values()
        .find(|item| {
            item.name.as_deref() == Some("required_op")
                && matches!(item.inner, ItemEnum::Function(_))
        })
        .expect("expected Function item for required_op");
    let ItemEnum::Function(ref f) = fn_item.inner else { panic!("expected Function") };
    assert!(
        !f.has_body,
        "trait method with has_default_impl=false must encode has_body=false (ADR 0248 D13)"
    );
}

/// Inherent method `has_body` is forced to `true` regardless of the
/// `has_default_impl` field (which is not semantically meaningful for inherent
/// methods). This preserves the pre-D13 invariant for struct inherent impls.
#[test]
fn test_encode_inherent_method_always_has_body_true_regardless_of_has_default_impl() {
    let mut doc = make_doc("domain");
    // Even if the catalogue accidentally sets has_default_impl=false on an
    // inherent method, the encoder must still emit has_body=true.
    let method = MethodDeclaration::new(
        MethodName::new("compute").unwrap(),
        Some(SelfReceiver::SharedRef),
        vec![],
        TypeRef::new("u32").unwrap(),
        false,
        None,
    );
    assert!(!method.has_default_impl);
    doc.types.insert(
        TypeName::new("Calculator").unwrap(),
        TypeEntry {
            action: ItemAction::Add,
            role: DataRole::ValueObject,
            kind: TypeKindV2::PlainStruct {
                fields: vec![],
                has_stripped_fields: false,
                typestate: None,
            },
            methods: vec![method],
            trait_impls: vec![],
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        },
    );

    let ec = CatalogueToExtendedCrateCodec::new().encode(doc).unwrap();
    let fn_item = ec
        .krate()
        .index
        .values()
        .find(|item| {
            item.name.as_deref() == Some("compute") && matches!(item.inner, ItemEnum::Function(_))
        })
        .expect("expected Function item for compute");
    let ItemEnum::Function(ref f) = fn_item.inner else { panic!("expected Function") };
    assert!(
        f.has_body,
        "inherent method must always encode has_body=true (force_has_body invariant)"
    );
}

// -----------------------------------------------------------------------
// ADR 0248 D14: FunctionEntry.generics → Function.generics (Gap 2)
// -----------------------------------------------------------------------

/// A free function with generic parameters must encode `entry.generics` as
/// `Function.generics`, and any param/return type that names one of those
/// generics must be emitted as `Type::Generic(_)` rather than as an
/// unresolved path. Mirrors `MethodDeclaration.generics` handling.
#[test]
fn test_encode_function_with_generics_emits_type_generic_in_signature() {
    use domain::tddd::catalogue_v2::FunctionName;
    use domain::tddd::catalogue_v2::entries::FunctionEntry;
    use domain::tddd::catalogue_v2::identifiers::FunctionPath;
    use domain::tddd::catalogue_v2::roles::FunctionRole;

    let mut doc = make_doc("domain");
    let crate_n = CrateName::new("domain").unwrap();
    let fn_path = FunctionPath::at_root(crate_n, FunctionName::new("generic_fn").unwrap());
    let entry = FunctionEntry {
        action: ItemAction::Add,
        role: FunctionRole::FreeFunction,
        params: vec![ParamDeclaration::new(
            ParamName::new("value").unwrap(),
            TypeRef::new("T").unwrap(),
        )],
        returns: TypeRef::new("T").unwrap(),
        is_async: false,
        generics: vec![MethodGenericParam {
            name: ParamName::new("T").unwrap(),
            bounds: vec![TypeRef::new("Clone").unwrap()],
        }],
        where_predicates: vec![],
        docs: None,
        spec_refs: vec![],
        informal_grounds: vec![],
    };
    doc.functions.insert(fn_path, entry);

    let ec = CatalogueToExtendedCrateCodec::new().encode(doc).unwrap();
    let fn_item = ec
        .krate()
        .index
        .values()
        .find(|item| {
            item.name.as_deref() == Some("generic_fn")
                && matches!(item.inner, ItemEnum::Function(_))
        })
        .expect("expected Function item for generic_fn");
    let ItemEnum::Function(ref f) = fn_item.inner else { panic!("expected Function") };

    // generics participates: 1 type-param `T` with bound `Clone`.
    assert_eq!(f.generics.params.len(), 1, "expected 1 generic param, got {:?}", f.generics.params);
    assert_eq!(f.generics.params[0].name, "T");

    // The first input is `value: T` — must be Type::Generic("T").
    let (pname, pty) = &f.sig.inputs[0];
    assert_eq!(pname, "value");
    assert!(
        matches!(pty, Type::Generic(g) if g == "T"),
        "expected Type::Generic(\"T\") for `value` param, got {pty:?}"
    );

    // Return type is `T` — must be Type::Generic("T").
    let output = f.sig.output.as_ref().expect("expected Some output");
    assert!(
        matches!(output, Type::Generic(g) if g == "T"),
        "expected Type::Generic(\"T\") for return, got {output:?}"
    );
}

/// A free function with no generics emits `empty_generics()` (no params,
/// no where_predicates). This preserves the pre-D14 baseline for the vast
/// majority of free functions in the workspace.
#[test]
fn test_encode_function_without_generics_emits_empty_generics() {
    use domain::tddd::catalogue_v2::FunctionName;
    use domain::tddd::catalogue_v2::entries::FunctionEntry;
    use domain::tddd::catalogue_v2::identifiers::FunctionPath;
    use domain::tddd::catalogue_v2::roles::FunctionRole;

    let mut doc = make_doc("domain");
    let crate_n = CrateName::new("domain").unwrap();
    let fn_path = FunctionPath::at_root(crate_n, FunctionName::new("simple").unwrap());
    let entry = FunctionEntry {
        action: ItemAction::Add,
        role: FunctionRole::FreeFunction,
        params: vec![],
        returns: TypeRef::new("()").unwrap(),
        is_async: false,
        generics: vec![],
        where_predicates: vec![],
        docs: None,
        spec_refs: vec![],
        informal_grounds: vec![],
    };
    doc.functions.insert(fn_path, entry);

    let ec = CatalogueToExtendedCrateCodec::new().encode(doc).unwrap();
    let fn_item = ec
        .krate()
        .index
        .values()
        .find(|item| {
            item.name.as_deref() == Some("simple") && matches!(item.inner, ItemEnum::Function(_))
        })
        .expect("expected Function item for simple");
    let ItemEnum::Function(ref f) = fn_item.inner else { panic!("expected Function") };
    assert!(
        f.generics.params.is_empty() && f.generics.where_predicates.is_empty(),
        "function without generics must emit empty Generics"
    );
}

/// A catalogue `WherePredicateDecl.rhs[i]` whose string form starts with `use<`
/// must be accepted at encode time (ADR 2026-05-18-1223 D1 supersedes ADR
/// 2026-05-13-1153 D3).  `validate_supported_bound` and the syntactic pre-check
/// for `use<...>` are both removed; `parse_generic_bound` encodes `use<...>` as a
/// best-effort placeholder `GenericBound::TraitBound` and the encode must succeed.
#[test]
fn test_encode_function_with_use_capture_bound_in_where_predicate_succeeds() {
    use domain::tddd::catalogue_v2::FunctionName;
    use domain::tddd::catalogue_v2::entries::FunctionEntry;
    use domain::tddd::catalogue_v2::identifiers::FunctionPath;
    use domain::tddd::catalogue_v2::methods::{BoundOp, MethodGenericParam, WherePredicateDecl};
    use domain::tddd::catalogue_v2::roles::FunctionRole;
    use domain::tddd::catalogue_v2::{ParamName, TypeRef};

    let mut doc = make_doc("domain");
    let crate_n = CrateName::new("domain").unwrap();
    let fn_path = FunctionPath::at_root(crate_n, FunctionName::new("ok_use").unwrap());
    let entry = FunctionEntry {
        action: ItemAction::Add,
        role: FunctionRole::FreeFunction,
        params: vec![],
        returns: TypeRef::new("()").unwrap(),
        is_async: false,
        generics: vec![
            MethodGenericParam { name: ParamName::new("T").unwrap(), bounds: vec![] },
            MethodGenericParam { name: ParamName::new("U").unwrap(), bounds: vec![] },
        ],
        where_predicates: vec![WherePredicateDecl {
            lhs: TypeRef::new("T").unwrap(),
            rhs: vec![TypeRef::new("use<U>").unwrap()],
            operator: BoundOp::Bound,
        }],
        docs: None,
        spec_refs: vec![],
        informal_grounds: vec![],
    };
    doc.functions.insert(fn_path, entry);

    let result = CatalogueToExtendedCrateCodec::new().encode(doc);
    assert!(
        result.is_ok(),
        "precise-capture bound `use<U>` must be accepted without error, got: {result:?}"
    );
}

/// Same as the previous test but the precise-capture bound has a space between the
/// `use` keyword and the `<` token (i.e. `use <U>`).  This variant must also be
/// accepted after the syntactic pre-check removal (ADR 2026-05-18-1223 D1).
#[test]
fn test_encode_function_with_use_capture_bound_with_space_succeeds() {
    use domain::tddd::catalogue_v2::FunctionName;
    use domain::tddd::catalogue_v2::entries::FunctionEntry;
    use domain::tddd::catalogue_v2::identifiers::FunctionPath;
    use domain::tddd::catalogue_v2::methods::{BoundOp, MethodGenericParam, WherePredicateDecl};
    use domain::tddd::catalogue_v2::roles::FunctionRole;
    use domain::tddd::catalogue_v2::{ParamName, TypeRef};

    let mut doc = make_doc("domain");
    let crate_n = CrateName::new("domain").unwrap();
    let fn_path = FunctionPath::at_root(crate_n, FunctionName::new("ok_use_space").unwrap());
    let entry = FunctionEntry {
        action: ItemAction::Add,
        role: FunctionRole::FreeFunction,
        params: vec![],
        returns: TypeRef::new("()").unwrap(),
        is_async: false,
        generics: vec![
            MethodGenericParam { name: ParamName::new("T").unwrap(), bounds: vec![] },
            MethodGenericParam { name: ParamName::new("U").unwrap(), bounds: vec![] },
        ],
        where_predicates: vec![WherePredicateDecl {
            lhs: TypeRef::new("T").unwrap(),
            // Precise-capture with whitespace between `use` and `<`.
            rhs: vec![TypeRef::new("use <U>").unwrap()],
            operator: BoundOp::Bound,
        }],
        docs: None,
        spec_refs: vec![],
        informal_grounds: vec![],
    };
    doc.functions.insert(fn_path, entry);

    let result = CatalogueToExtendedCrateCodec::new().encode(doc);
    assert!(
        result.is_ok(),
        "precise-capture bound `use <U>` (spaced) must be accepted without error, got: {result:?}"
    );
}

/// A `WherePredicateDecl` whose `lhs` is a qualified-path form
/// (`<T as Trait>::Assoc`) must be rejected at encode time. The A-codec
/// cannot reconstruct the `Type::QualifiedPath` shape that rustdoc emits for
/// such predicates — `parse_type_ref_str` degrades it to an unresolved
/// placeholder which silently breaks structural equality.
#[test]
fn test_encode_function_with_qualified_path_lhs_in_where_predicate_returns_error() {
    use domain::tddd::catalogue_v2::FunctionName;
    use domain::tddd::catalogue_v2::entries::FunctionEntry;
    use domain::tddd::catalogue_v2::identifiers::FunctionPath;
    use domain::tddd::catalogue_v2::methods::{BoundOp, MethodGenericParam, WherePredicateDecl};
    use domain::tddd::catalogue_v2::roles::FunctionRole;
    use domain::tddd::catalogue_v2::{ParamName, TypeRef};

    let mut doc = make_doc("domain");
    let crate_n = CrateName::new("domain").unwrap();
    let fn_path = FunctionPath::at_root(crate_n, FunctionName::new("bad_qpath_lhs").unwrap());
    let entry = FunctionEntry {
        action: ItemAction::Add,
        role: FunctionRole::FreeFunction,
        params: vec![],
        returns: TypeRef::new("()").unwrap(),
        is_async: false,
        generics: vec![MethodGenericParam { name: ParamName::new("T").unwrap(), bounds: vec![] }],
        where_predicates: vec![WherePredicateDecl {
            // Qualified-path LHS: `<T as Iterator>::Item`.
            lhs: TypeRef::new("<T as Iterator>::Item").unwrap(),
            rhs: vec![TypeRef::new("Clone").unwrap()],
            operator: BoundOp::Bound,
        }],
        docs: None,
        spec_refs: vec![],
        informal_grounds: vec![],
    };
    doc.functions.insert(fn_path, entry);

    let result = CatalogueToExtendedCrateCodec::new().encode(doc);
    assert!(
        matches!(result, Err(NewTypeGraphCodecError::InvalidTypeRef(_))),
        "expected InvalidTypeRef for `<T as Trait>::Assoc` LHS, got: {result:?}"
    );
}

// -----------------------------------------------------------------------
// ADR 2026-05-13-1153 D1: explicit WherePredicateDecl → where_predicates
// -----------------------------------------------------------------------

/// A `FunctionEntry` with an explicit `WherePredicateDecl` (`where T: Clone`)
/// must emit a `WherePredicate::BoundPredicate` in `Function.generics.where_predicates`
/// with `type_ = Type::Generic("T")`, and the `GenericParamDef.bounds` for that
/// parameter must be empty (ADR D1 — all bounds lifted to where form).
#[test]
fn test_encode_function_with_explicit_where_predicate_emits_bound_predicate() {
    use domain::tddd::catalogue_v2::FunctionName;
    use domain::tddd::catalogue_v2::entries::FunctionEntry;
    use domain::tddd::catalogue_v2::identifiers::FunctionPath;
    use domain::tddd::catalogue_v2::methods::{BoundOp, MethodGenericParam, WherePredicateDecl};
    use domain::tddd::catalogue_v2::roles::FunctionRole;
    use domain::tddd::catalogue_v2::{ParamName, TypeRef};

    let mut doc = make_doc("domain");
    let crate_n = CrateName::new("domain").unwrap();
    let fn_path = FunctionPath::at_root(crate_n, FunctionName::new("where_fn").unwrap());
    let entry = FunctionEntry {
        action: ItemAction::Add,
        role: FunctionRole::FreeFunction,
        params: vec![],
        returns: TypeRef::new("()").unwrap(),
        is_async: false,
        // generic param `T` with no inline bounds
        generics: vec![MethodGenericParam { name: ParamName::new("T").unwrap(), bounds: vec![] }],
        // explicit where predicate: `where T: Clone`
        where_predicates: vec![WherePredicateDecl {
            lhs: TypeRef::new("T").unwrap(),
            rhs: vec![TypeRef::new("Clone").unwrap()],
            operator: BoundOp::Bound,
        }],
        docs: None,
        spec_refs: vec![],
        informal_grounds: vec![],
    };
    doc.functions.insert(fn_path, entry);

    let ec = CatalogueToExtendedCrateCodec::new().encode(doc).unwrap();
    let fn_item = ec
        .krate()
        .index
        .values()
        .find(|item| {
            item.name.as_deref() == Some("where_fn") && matches!(item.inner, ItemEnum::Function(_))
        })
        .expect("expected Function item for where_fn");
    let ItemEnum::Function(ref f) = fn_item.inner else { panic!("expected Function") };

    // One type param `T` with empty inline bounds (all bounds lifted to where form).
    assert_eq!(f.generics.params.len(), 1, "expected 1 generic param");
    let param = &f.generics.params[0];
    assert_eq!(param.name, "T");
    let GenericParamDefKind::Type { bounds, .. } = &param.kind else {
        panic!("expected Type kind for param T");
    };
    assert!(
        bounds.is_empty(),
        "GenericParamDef.bounds must be empty (D1: bounds lifted to where form)"
    );

    // One BoundPredicate for `T: Clone` in where_predicates.
    assert_eq!(
        f.generics.where_predicates.len(),
        1,
        "expected 1 where predicate, got {:?}",
        f.generics.where_predicates
    );
    let WherePredicate::BoundPredicate { type_, bounds, .. } = &f.generics.where_predicates[0]
    else {
        panic!("expected BoundPredicate, got {:?}", f.generics.where_predicates[0]);
    };
    assert!(
        matches!(type_, Type::Generic(g) if g == "T"),
        "BoundPredicate LHS must be Type::Generic(\"T\"), got {type_:?}"
    );
    assert!(!bounds.is_empty(), "BoundPredicate bounds must be non-empty");
}

/// A `FunctionEntry` with a non-trivial LHS in a `WherePredicateDecl`
/// (`where Vec<T>: Clone`) must emit a `WherePredicate::BoundPredicate` whose
/// `type_` is NOT `Type::Generic` (it is a resolved-path or generic array).
/// Verifies the non-bare-generic-name branch of `build_where_form_generics`.
#[test]
fn test_encode_function_with_non_trivial_lhs_where_predicate_emits_bound_predicate() {
    use domain::tddd::catalogue_v2::FunctionName;
    use domain::tddd::catalogue_v2::entries::FunctionEntry;
    use domain::tddd::catalogue_v2::identifiers::FunctionPath;
    use domain::tddd::catalogue_v2::methods::{BoundOp, MethodGenericParam, WherePredicateDecl};
    use domain::tddd::catalogue_v2::roles::FunctionRole;
    use domain::tddd::catalogue_v2::{ParamName, TypeRef};

    let mut doc = make_doc("domain");
    let crate_n = CrateName::new("domain").unwrap();
    let fn_path = FunctionPath::at_root(crate_n, FunctionName::new("vec_where_fn").unwrap());
    let entry = FunctionEntry {
        action: ItemAction::Add,
        role: FunctionRole::FreeFunction,
        params: vec![],
        returns: TypeRef::new("()").unwrap(),
        is_async: false,
        generics: vec![MethodGenericParam { name: ParamName::new("T").unwrap(), bounds: vec![] }],
        // explicit where predicate: `where Vec<T>: Clone` — non-trivial LHS
        where_predicates: vec![WherePredicateDecl {
            lhs: TypeRef::new("Vec<T>").unwrap(),
            rhs: vec![TypeRef::new("Clone").unwrap()],
            operator: BoundOp::Bound,
        }],
        docs: None,
        spec_refs: vec![],
        informal_grounds: vec![],
    };
    doc.functions.insert(fn_path, entry);

    let ec = CatalogueToExtendedCrateCodec::new().encode(doc).unwrap();
    let fn_item = ec
        .krate()
        .index
        .values()
        .find(|item| {
            item.name.as_deref() == Some("vec_where_fn")
                && matches!(item.inner, ItemEnum::Function(_))
        })
        .expect("expected Function item for vec_where_fn");
    let ItemEnum::Function(ref f) = fn_item.inner else { panic!("expected Function") };

    // Must have exactly one where predicate (the Vec<T>: Clone entry).
    assert_eq!(
        f.generics.where_predicates.len(),
        1,
        "expected 1 where predicate for `where Vec<T>: Clone`"
    );
    let WherePredicate::BoundPredicate { type_, bounds, .. } = &f.generics.where_predicates[0]
    else {
        panic!("expected BoundPredicate, got {:?}", f.generics.where_predicates[0]);
    };
    // LHS must not be a bare generic; it should be some compound type.
    assert!(
        !matches!(type_, Type::Generic(g) if g == "T"),
        "LHS for `Vec<T>: Clone` must not be Type::Generic(\"T\")"
    );
    assert!(!bounds.is_empty(), "BoundPredicate bounds must be non-empty for Clone");
}

/// A `FunctionEntry` with a `WherePredicateDecl` using `BoundOp::Equal` must produce
/// a `WherePredicate::EqPredicate` (not `BoundPredicate`) in the extended-crate output.
/// Verifies the `Equal` branch of `build_where_form_generics`.
#[test]
fn test_encode_function_with_equal_where_predicate_emits_eq_predicate() {
    use domain::tddd::catalogue_v2::FunctionName;
    use domain::tddd::catalogue_v2::entries::FunctionEntry;
    use domain::tddd::catalogue_v2::identifiers::FunctionPath;
    use domain::tddd::catalogue_v2::methods::{BoundOp, MethodGenericParam, WherePredicateDecl};
    use domain::tddd::catalogue_v2::roles::FunctionRole;
    use domain::tddd::catalogue_v2::{ParamName, TypeRef};
    use rustdoc_types::{Term, WherePredicate};

    let mut doc = make_doc("domain");
    let crate_n = CrateName::new("domain").unwrap();
    let fn_path = FunctionPath::at_root(crate_n, FunctionName::new("eq_where_fn").unwrap());
    let entry = FunctionEntry {
        action: ItemAction::Add,
        role: FunctionRole::FreeFunction,
        params: vec![],
        returns: TypeRef::new("()").unwrap(),
        is_async: false,
        generics: vec![MethodGenericParam { name: ParamName::new("T").unwrap(), bounds: vec![] }],
        // explicit where predicate: `where T::Assoc = u32` (Equal operator)
        where_predicates: vec![WherePredicateDecl {
            lhs: TypeRef::new("T::Assoc").unwrap(),
            rhs: vec![TypeRef::new("u32").unwrap()],
            operator: BoundOp::Equal,
        }],
        docs: None,
        spec_refs: vec![],
        informal_grounds: vec![],
    };
    doc.functions.insert(fn_path, entry);

    let ec = CatalogueToExtendedCrateCodec::new().encode(doc).unwrap();
    let fn_item = ec
        .krate()
        .index
        .values()
        .find(|item| {
            item.name.as_deref() == Some("eq_where_fn")
                && matches!(item.inner, ItemEnum::Function(_))
        })
        .expect("expected Function item for eq_where_fn");
    let ItemEnum::Function(ref f) = fn_item.inner else { panic!("expected Function") };

    assert_eq!(
        f.generics.where_predicates.len(),
        1,
        "expected 1 where predicate for Equal predicate"
    );
    // Must be EqPredicate, not BoundPredicate.
    assert!(
        matches!(f.generics.where_predicates[0], WherePredicate::EqPredicate { .. }),
        "Equal operator must emit WherePredicate::EqPredicate, got {:?}",
        f.generics.where_predicates[0]
    );
    let WherePredicate::EqPredicate { ref rhs, .. } = f.generics.where_predicates[0] else {
        panic!("expected EqPredicate");
    };
    // RHS must be Term::Type (not Term::Const).
    assert!(matches!(rhs, Term::Type(_)), "EqPredicate rhs must be Term::Type, got {rhs:?}");
}

/// A `FunctionEntry` with a `BoundOp::Equal` predicate and multiple RHS entries
/// must be rejected by `CatalogueToExtendedCrateCodec::encode` with an error.
/// Verifies the defensive rhs.len() == 1 check in `build_where_form_generics`.
#[test]
fn test_encode_function_with_equal_predicate_multiple_rhs_returns_error() {
    use domain::tddd::catalogue_v2::FunctionName;
    use domain::tddd::catalogue_v2::entries::FunctionEntry;
    use domain::tddd::catalogue_v2::identifiers::FunctionPath;
    use domain::tddd::catalogue_v2::methods::{BoundOp, MethodGenericParam, WherePredicateDecl};
    use domain::tddd::catalogue_v2::roles::FunctionRole;
    use domain::tddd::catalogue_v2::{ParamName, TypeRef};

    let mut doc = make_doc("domain");
    let crate_n = CrateName::new("domain").unwrap();
    let fn_path = FunctionPath::at_root(crate_n, FunctionName::new("bad_eq_fn").unwrap());
    let entry = FunctionEntry {
        action: ItemAction::Add,
        role: FunctionRole::FreeFunction,
        params: vec![],
        returns: TypeRef::new("()").unwrap(),
        is_async: false,
        generics: vec![MethodGenericParam { name: ParamName::new("T").unwrap(), bounds: vec![] }],
        // Invalid: Equal with two RHS entries.
        where_predicates: vec![WherePredicateDecl {
            lhs: TypeRef::new("T::Assoc").unwrap(),
            rhs: vec![TypeRef::new("u32").unwrap(), TypeRef::new("String").unwrap()],
            operator: BoundOp::Equal,
        }],
        docs: None,
        spec_refs: vec![],
        informal_grounds: vec![],
    };
    doc.functions.insert(fn_path, entry);

    let result = CatalogueToExtendedCrateCodec::new().encode(doc);
    assert!(
        result.is_err(),
        "Equal predicate with multiple rhs must return an error, got: {result:?}"
    );
}

/// A `FunctionEntry` with a `BoundOp::Equal` predicate whose LHS is a bare type
/// parameter (no `::`) must be rejected by `CatalogueToExtendedCrateCodec::encode`
/// with an error.  `where T = u32` is not valid Rust; the LHS must be an
/// associated-type projection such as `T::Assoc`.
#[test]
fn test_encode_function_with_equal_predicate_bare_type_param_lhs_returns_error() {
    use domain::tddd::catalogue_v2::FunctionName;
    use domain::tddd::catalogue_v2::entries::FunctionEntry;
    use domain::tddd::catalogue_v2::identifiers::FunctionPath;
    use domain::tddd::catalogue_v2::methods::{BoundOp, MethodGenericParam, WherePredicateDecl};
    use domain::tddd::catalogue_v2::roles::FunctionRole;
    use domain::tddd::catalogue_v2::{ParamName, TypeRef};

    let mut doc = make_doc("domain");
    let crate_n = CrateName::new("domain").unwrap();
    let fn_path = FunctionPath::at_root(crate_n, FunctionName::new("bad_bare_lhs_eq_fn").unwrap());
    let entry = FunctionEntry {
        action: ItemAction::Add,
        role: FunctionRole::FreeFunction,
        params: vec![],
        returns: TypeRef::new("()").unwrap(),
        is_async: false,
        generics: vec![MethodGenericParam { name: ParamName::new("T").unwrap(), bounds: vec![] }],
        // Invalid: Equal predicate with bare type parameter as LHS (`where T = u32`).
        where_predicates: vec![WherePredicateDecl {
            lhs: TypeRef::new("T").unwrap(),
            rhs: vec![TypeRef::new("u32").unwrap()],
            operator: BoundOp::Equal,
        }],
        docs: None,
        spec_refs: vec![],
        informal_grounds: vec![],
    };
    doc.functions.insert(fn_path, entry);

    let result = CatalogueToExtendedCrateCodec::new().encode(doc);
    assert!(
        result.is_err(),
        "Equal predicate with bare type param LHS must return an error, got: {result:?}"
    );
}

// -----------------------------------------------------------------------
// ADR 2026-05-18-1223 D1: validate_supported_bound 撤廃 — lifetime / HRTB /
// precise-capture bounds must be accepted (AC-02)
// -----------------------------------------------------------------------

/// A `FunctionEntry` with a lifetime bound (`'static`) on an inline
/// `MethodGenericParam.bounds` entry must be accepted without error
/// (ADR 2026-05-18-1223 D1 — `validate_supported_bound` abolished).
/// The `GenericBound::Outlives("static")` produced by `parse_generic_bound`
/// must appear in the encoded `BoundPredicate.bounds` for that parameter.
#[test]
fn test_encode_function_with_lifetime_bound_static_succeeds() {
    use domain::tddd::catalogue_v2::FunctionName;
    use domain::tddd::catalogue_v2::entries::FunctionEntry;
    use domain::tddd::catalogue_v2::identifiers::FunctionPath;
    use domain::tddd::catalogue_v2::methods::MethodGenericParam;
    use domain::tddd::catalogue_v2::roles::FunctionRole;
    use domain::tddd::catalogue_v2::{ParamName, TypeRef};

    let mut doc = make_doc("domain");
    let crate_n = CrateName::new("domain").unwrap();
    let fn_path = FunctionPath::at_root(crate_n, FunctionName::new("lifetime_bound_fn").unwrap());
    // `<F: Fn() + Send + Sync + 'static>` — inline bounds include a lifetime bound.
    let entry = FunctionEntry {
        action: ItemAction::Add,
        role: FunctionRole::FreeFunction,
        params: vec![],
        returns: TypeRef::new("()").unwrap(),
        is_async: false,
        generics: vec![MethodGenericParam {
            name: ParamName::new("F").unwrap(),
            bounds: vec![
                TypeRef::new("Fn()").unwrap(),
                TypeRef::new("Send").unwrap(),
                TypeRef::new("Sync").unwrap(),
                // Lifetime bound: must be accepted after validate_supported_bound removal.
                TypeRef::new("'static").unwrap(),
            ],
        }],
        where_predicates: vec![],
        docs: None,
        spec_refs: vec![],
        informal_grounds: vec![],
    };
    doc.functions.insert(fn_path, entry);

    let result = CatalogueToExtendedCrateCodec::new().encode(doc);
    assert!(
        result.is_ok(),
        "lifetime bound `'static` must be accepted without error, got: {result:?}"
    );

    // Verify the encoded BoundPredicate contains a GenericBound::Outlives("static") entry.
    let ec = result.unwrap();
    let fn_item = ec
        .krate()
        .index
        .values()
        .find(|item| {
            item.name.as_deref() == Some("lifetime_bound_fn")
                && matches!(item.inner, ItemEnum::Function(_))
        })
        .expect("expected Function item for lifetime_bound_fn");
    let ItemEnum::Function(ref f) = fn_item.inner else { panic!("expected Function") };

    // All bounds are lifted to where form (ADR 2026-05-13-1153 D1).
    // The BoundPredicate for `F` must contain a GenericBound::Outlives("static").
    let has_static_outlives = f.generics.where_predicates.iter().any(|wp| {
        if let WherePredicate::BoundPredicate { type_, bounds, .. } = wp {
            matches!(type_, Type::Generic(g) if g == "F")
                && bounds.iter().any(|b| matches!(b, GenericBound::Outlives(lt) if lt == "static"))
        } else {
            false
        }
    });
    assert!(
        has_static_outlives,
        "encoded where_predicates must contain Outlives(\"static\") for `F: 'static`, \
         got: {:?}",
        f.generics.where_predicates
    );
}

/// A `FunctionEntry` with a named lifetime bound (`'a`) on an inline
/// `MethodGenericParam.bounds` entry must be accepted without error
/// (ADR 2026-05-18-1223 D1).
#[test]
fn test_encode_function_with_lifetime_bound_named_succeeds() {
    use domain::tddd::catalogue_v2::FunctionName;
    use domain::tddd::catalogue_v2::entries::FunctionEntry;
    use domain::tddd::catalogue_v2::identifiers::FunctionPath;
    use domain::tddd::catalogue_v2::methods::MethodGenericParam;
    use domain::tddd::catalogue_v2::roles::FunctionRole;
    use domain::tddd::catalogue_v2::{ParamName, TypeRef};

    let mut doc = make_doc("domain");
    let crate_n = CrateName::new("domain").unwrap();
    let fn_path =
        FunctionPath::at_root(crate_n, FunctionName::new("named_lifetime_bound_fn").unwrap());
    let entry = FunctionEntry {
        action: ItemAction::Add,
        role: FunctionRole::FreeFunction,
        params: vec![],
        returns: TypeRef::new("()").unwrap(),
        is_async: false,
        // `<T: Clone + 'a>` — named lifetime bound.
        generics: vec![MethodGenericParam {
            name: ParamName::new("T").unwrap(),
            bounds: vec![TypeRef::new("Clone").unwrap(), TypeRef::new("'a").unwrap()],
        }],
        where_predicates: vec![],
        docs: None,
        spec_refs: vec![],
        informal_grounds: vec![],
    };
    doc.functions.insert(fn_path, entry);

    let result = CatalogueToExtendedCrateCodec::new().encode(doc);
    assert!(
        result.is_ok(),
        "named lifetime bound `'a` must be accepted without error, got: {result:?}"
    );

    // The BoundPredicate for `T` must contain a GenericBound::Outlives("a").
    let ec = result.unwrap();
    let fn_item = ec
        .krate()
        .index
        .values()
        .find(|item| {
            item.name.as_deref() == Some("named_lifetime_bound_fn")
                && matches!(item.inner, ItemEnum::Function(_))
        })
        .expect("expected Function item for named_lifetime_bound_fn");
    let ItemEnum::Function(ref f) = fn_item.inner else { panic!("expected Function") };
    let has_named_outlives = f.generics.where_predicates.iter().any(|wp| {
        if let WherePredicate::BoundPredicate { type_, bounds, .. } = wp {
            matches!(type_, Type::Generic(g) if g == "T")
                && bounds.iter().any(|b| matches!(b, GenericBound::Outlives(lt) if lt == "a"))
        } else {
            false
        }
    });
    assert!(
        has_named_outlives,
        "encoded where_predicates must contain Outlives(\"a\") for `T: 'a`, \
         got: {:?}",
        f.generics.where_predicates
    );
}

/// A `FunctionEntry` with an HRTB trait bound (`for<'a> Fn(&'a ())`) on an inline
/// `MethodGenericParam.bounds` entry must be accepted without error
/// (ADR 2026-05-18-1223 D1).  The encoded `GenericBound::TraitBound` must carry
/// the `generic_params` field populated with the HRTB binder lifetime.
#[test]
fn test_encode_function_with_hrtb_trait_bound_succeeds() {
    use domain::tddd::catalogue_v2::FunctionName;
    use domain::tddd::catalogue_v2::entries::FunctionEntry;
    use domain::tddd::catalogue_v2::identifiers::FunctionPath;
    use domain::tddd::catalogue_v2::methods::MethodGenericParam;
    use domain::tddd::catalogue_v2::roles::FunctionRole;
    use domain::tddd::catalogue_v2::{ParamName, TypeRef};

    let mut doc = make_doc("domain");
    let crate_n = CrateName::new("domain").unwrap();
    let fn_path = FunctionPath::at_root(crate_n, FunctionName::new("hrtb_bound_fn").unwrap());
    let entry = FunctionEntry {
        action: ItemAction::Add,
        role: FunctionRole::FreeFunction,
        params: vec![],
        returns: TypeRef::new("()").unwrap(),
        is_async: false,
        // `<F: for<'a> Fn(&'a ())>` — HRTB on inline bound.
        generics: vec![MethodGenericParam {
            name: ParamName::new("F").unwrap(),
            bounds: vec![TypeRef::new("for<'a> Fn(&'a ())").unwrap()],
        }],
        where_predicates: vec![],
        docs: None,
        spec_refs: vec![],
        informal_grounds: vec![],
    };
    doc.functions.insert(fn_path, entry);

    let result = CatalogueToExtendedCrateCodec::new().encode(doc);
    assert!(
        result.is_ok(),
        "HRTB trait bound `for<'a> Fn(&'a ())` must be accepted without error, got: {result:?}"
    );

    // The BoundPredicate for `F` must contain a TraitBound with non-empty generic_params
    // (the HRTB binder).
    let ec = result.unwrap();
    let fn_item = ec
        .krate()
        .index
        .values()
        .find(|item| {
            item.name.as_deref() == Some("hrtb_bound_fn")
                && matches!(item.inner, ItemEnum::Function(_))
        })
        .expect("expected Function item for hrtb_bound_fn");
    let ItemEnum::Function(ref f) = fn_item.inner else { panic!("expected Function") };
    let has_hrtb_trait_bound = f.generics.where_predicates.iter().any(|wp| {
        if let WherePredicate::BoundPredicate { type_, bounds, .. } = wp {
            matches!(type_, Type::Generic(g) if g == "F")
                && bounds.iter().any(|b| {
                    matches!(b, GenericBound::TraitBound { generic_params, .. }
                        if !generic_params.is_empty())
                })
        } else {
            false
        }
    });
    assert!(
        has_hrtb_trait_bound,
        "encoded where_predicates must contain a TraitBound with non-empty generic_params \
         for `for<'a> Fn(&'a ())`, got: {:?}",
        f.generics.where_predicates
    );
}
