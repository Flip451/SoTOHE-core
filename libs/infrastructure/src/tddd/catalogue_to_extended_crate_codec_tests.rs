//! Tests for [`catalogue_to_extended_crate_codec`] (split out to keep the main module under the 200-400 line guideline).

use domain::tddd::CatalogueToExtendedCratePort;
use domain::tddd::LayerId;
use domain::tddd::catalogue_v2::composite::{StructKind, StructShape, TypeKindV2};
use domain::tddd::catalogue_v2::entries::{AssocConstDecl, AssocTypeDecl, TraitEntry, TypeEntry};
use domain::tddd::catalogue_v2::methods::{
    MethodDeclaration, MethodGenericParam, ParamDeclaration,
};
use domain::tddd::catalogue_v2::roles::{ContractRole, DataRole, ItemAction, SelfReceiver};
use domain::tddd::catalogue_v2::traits::TraitImplDeclV2;
use domain::tddd::catalogue_v2::variants::{FieldDecl, VariantDecl};
use domain::tddd::catalogue_v2::{
    AssocConstName, CatalogueDocument, CrateName, FieldName, MethodName, ModulePath, ParamName,
    TraitName, TypeName, TypeRef, VariantName,
};
use rustdoc_types::{
    AssocItemConstraintKind, GenericArg, GenericArgs, GenericBound, GenericParamDefKind, Id,
    ItemEnum, Term, Type, VariantKind, WherePredicate,
};

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
            role: DataRole::value_object(),
            kind: TypeKindV2::Enum { variants: vec![] },
            methods: vec![],

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
            assoc_types: vec![],
            assoc_consts: vec![],
            supertrait_bounds: vec![],
            generics: vec![],
            where_predicates: vec![],
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
            role: DataRole::value_object(),
            kind: TypeKindV2::Struct(StructKind::new(
                StructShape::Plain {
                    fields: vec![FieldDecl::new(
                        FieldName::new("value").unwrap(),
                        // "42invalid" is not a valid Rust type expression.
                        TypeRef::new("String").unwrap(),
                    )],
                    has_stripped_fields: false,
                },
                None,
            )),
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
            role: DataRole::value_object(),
            kind: TypeKindV2::Struct(StructKind::new(
                StructShape::Plain {
                    fields: vec![
                        FieldDecl::new(
                            FieldName::new("email").unwrap(),
                            TypeRef::new("String").unwrap(),
                        ),
                        FieldDecl::new(FieldName::new("id").unwrap(), TypeRef::new("u32").unwrap()),
                    ],
                    has_stripped_fields: false,
                },
                None,
            )),
            methods: vec![],

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
            role: DataRole::value_object(),
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
            role: DataRole::value_object(),
            kind: TypeKindV2::Struct(StructKind::new(
                StructShape::Plain { fields: vec![], has_stripped_fields: false },
                None,
            )),
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
            role: DataRole::value_object(),
            kind: TypeKindV2::Struct(StructKind::new(
                StructShape::Plain { fields: vec![], has_stripped_fields: false },
                None,
            )),
            methods: vec![],

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
            role: DataRole::value_object(),
            kind: TypeKindV2::Struct(StructKind::new(
                StructShape::Plain { fields: vec![], has_stripped_fields: false },
                None,
            )),
            methods: vec![],

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
            role: DataRole::value_object(),
            kind: TypeKindV2::Struct(StructKind::new(
                StructShape::Plain {
                    fields: vec![FieldDecl::new(
                        FieldName::new("items").unwrap(),
                        TypeRef::new("Vec<String>").unwrap(),
                    )],
                    has_stripped_fields: false,
                },
                None,
            )),
            methods: vec![],

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
            role: DataRole::value_object(),
            kind: TypeKindV2::Struct(StructKind::new(
                StructShape::Plain {
                    fields: vec![FieldDecl::new(
                        FieldName::new("name").unwrap(),
                        TypeRef::new("String").unwrap(),
                    )],
                    has_stripped_fields: false,
                },
                None,
            )),
            methods: vec![],

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
            role: DataRole::value_object(),
            kind: TypeKindV2::Struct(StructKind::new(
                StructShape::Plain {
                    fields: vec![FieldDecl::new(
                        FieldName::new("error").unwrap(),
                        TypeRef::new("DomainError").unwrap(),
                    )],
                    has_stripped_fields: false,
                },
                None,
            )),
            methods: vec![],

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
            role: DataRole::value_object(),
            kind: TypeKindV2::Struct(StructKind::new(
                StructShape::Plain { fields: vec![], has_stripped_fields: false },
                None,
            )),
            methods: vec![],

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
            role: DataRole::value_object(),
            kind: TypeKindV2::Struct(StructKind::new(
                StructShape::Plain { fields: vec![], has_stripped_fields: false },
                None,
            )),
            methods: vec![],
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        },
    );
    // ADR `2026-05-20-0048` D1: trait_impls are top-level on CatalogueDocument.
    doc.trait_impls.push(TraitImplDeclV2::new(
        TypeRef::new("serde::Serialize").unwrap(),
        TypeRef::new("Foo").unwrap(),
    ));

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
            assoc_types: vec![],
            assoc_consts: vec![],
            supertrait_bounds: vec![],
            generics: vec![],
            where_predicates: vec![],
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
            role: DataRole::value_object(),
            kind: TypeKindV2::TypeAlias {
                target: TypeRef::new("Result<User, DomainError>").unwrap(),
            },
            methods: vec![],

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
// generic_args in TraitImplDeclV2 → structured trait_.args (ADR 2026-05-20-0048 D2)
// -----------------------------------------------------------------------

#[test]
fn test_encode_trait_impl_with_generic_args_produces_impl_with_structured_trait_args() {
    // Per ADR `2026-05-20-0048` D2, the encoded Impl item's trait path is the canonical
    // BASE path (`"core::convert::From"`) and the generic args are carried structurally
    // in `trait_.args` — NOT re-inlined into the path string.  `build_impl_identity_map`
    // renders the structured args via `format_generic_args` at key-construction time,
    // producing `"RenderContractMapError: core::convert::From<CatalogueLoaderError>"` on
    // both the S-side (this codec) and the C-side (rustdoc).
    let mut doc = make_doc("usecase");
    doc.types.insert(
        TypeName::new("RenderContractMapError").unwrap(),
        TypeEntry {
            action: ItemAction::Modify,
            role: DataRole::ErrorType,
            kind: TypeKindV2::Enum { variants: vec![] },
            methods: vec![],
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        },
    );
    // ADR `2026-05-20-0048` D1/D2: trait_impls are top-level; generic args in trait_ref string.
    doc.trait_impls.push(TraitImplDeclV2::new(
        TypeRef::new("core::convert::From<CatalogueLoaderError>").unwrap(),
        TypeRef::new("RenderContractMapError").unwrap(),
    ));
    doc.trait_impls.push(TraitImplDeclV2::new(
        TypeRef::new("core::convert::From<ContractMapWriterError>").unwrap(),
        TypeRef::new("RenderContractMapError").unwrap(),
    ));

    let ec = CatalogueToExtendedCrateCodec::new().encode(doc).unwrap();
    let krate = ec.krate();

    // Collect (base_path, structured_args) from all Impl items that have a trait.
    let from_impls: Vec<(String, String)> = krate
        .index
        .values()
        .filter_map(|item| {
            if let ItemEnum::Impl(impl_) = &item.inner {
                let tp = impl_.trait_.as_ref()?;
                let args_joined = match tp.args.as_deref() {
                    Some(rustdoc_types::GenericArgs::AngleBracketed { args, .. }) => args
                        .iter()
                        .filter_map(|a| match a {
                            rustdoc_types::GenericArg::Type(Type::ResolvedPath(p)) => {
                                Some(p.path.clone())
                            }
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join(", "),
                    _ => String::new(),
                };
                Some((tp.path.clone(), args_joined))
            } else {
                None
            }
        })
        .collect();

    // ADR D2: the trait path is the bare base form — no inline generic args.
    assert!(
        from_impls.iter().all(|(path, _)| !path.contains('<')),
        "trait path must be the bare base form with no inline generic args, got: {from_impls:?}"
    );
    // The generic args are carried structurally in `trait_.args`.
    assert!(
        from_impls
            .iter()
            .any(|(path, args)| path == "core::convert::From"
                && args.contains("CatalogueLoaderError")),
        "expected impl with base path 'core::convert::From' and structured arg 'CatalogueLoaderError', got: {from_impls:?}"
    );
    assert!(
        from_impls
            .iter()
            .any(|(path, args)| path == "core::convert::From"
                && args.contains("ContractMapWriterError")),
        "expected impl with base path 'core::convert::From' and structured arg 'ContractMapWriterError', got: {from_impls:?}"
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
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        },
    );
    // ADR `2026-05-20-0048` D1/D2: trait_impls are top-level; full qualified path in trait_ref.
    doc.trait_impls.push(TraitImplDeclV2::new(
        TypeRef::new("core::convert::From").unwrap(),
        TypeRef::new("SomeError").unwrap(),
    ));

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
            role: DataRole::value_object(),
            kind: TypeKindV2::Struct(StructKind::new(
                StructShape::Plain { fields: vec![], has_stripped_fields: false },
                None,
            )),
            methods: vec![method],

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
            assoc_types: vec![],
            assoc_consts: vec![],
            supertrait_bounds: vec![],
            generics: vec![],
            where_predicates: vec![],
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
            assoc_types: vec![],
            assoc_consts: vec![],
            supertrait_bounds: vec![],
            generics: vec![],
            where_predicates: vec![],
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
            role: DataRole::value_object(),
            kind: TypeKindV2::Struct(StructKind::new(
                StructShape::Plain { fields: vec![], has_stripped_fields: false },
                None,
            )),
            methods: vec![method],

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
/// (`<T as Trait>::Assoc`) must be accepted at encode time under the permissive
/// principle (ADR `2026-05-20-0048`): any syn-parseable input is accepted.
/// The A-codec falls back to an unresolved placeholder for the qualified-path shape
/// it cannot reconstruct exactly — this is acceptable under the permissive principle.
#[test]
fn test_encode_function_with_qualified_path_lhs_in_where_predicate_succeeds() {
    use domain::tddd::catalogue_v2::FunctionName;
    use domain::tddd::catalogue_v2::entries::FunctionEntry;
    use domain::tddd::catalogue_v2::identifiers::FunctionPath;
    use domain::tddd::catalogue_v2::methods::{BoundOp, MethodGenericParam, WherePredicateDecl};
    use domain::tddd::catalogue_v2::roles::FunctionRole;
    use domain::tddd::catalogue_v2::{ParamName, TypeRef};

    let mut doc = make_doc("domain");
    let crate_n = CrateName::new("domain").unwrap();
    let fn_path = FunctionPath::at_root(crate_n, FunctionName::new("qpath_lhs_fn").unwrap());
    let entry = FunctionEntry {
        action: ItemAction::Add,
        role: FunctionRole::FreeFunction,
        params: vec![],
        returns: TypeRef::new("()").unwrap(),
        is_async: false,
        generics: vec![MethodGenericParam { name: ParamName::new("T").unwrap(), bounds: vec![] }],
        where_predicates: vec![WherePredicateDecl {
            // Qualified-path LHS: `<T as Iterator>::Item` — syn-parseable, accepted permissively.
            lhs: TypeRef::new("<T as Iterator>::Item").unwrap(),
            rhs: vec![TypeRef::new("Clone").unwrap()],
            operator: BoundOp::Bound,
        }],
        docs: None,
        spec_refs: vec![],
        informal_grounds: vec![],
    };
    doc.functions.insert(fn_path, entry);

    // Permissive: encoding must succeed (no shape validation rejection).
    let result = CatalogueToExtendedCrateCodec::new().encode(doc);
    assert!(
        result.is_ok(),
        "expected Ok for syn-parseable `<T as Trait>::Assoc` LHS under permissive principle, \
         got: {result:?}"
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
/// parameter (no `::`) must be accepted by `CatalogueToExtendedCrateCodec::encode`
/// (permissive principle, ADR `2026-05-20-0048`).  The JSON codec no longer enforces
/// the `::` invariant on Equal-predicate LHS values, so the encoder must match.
/// The resulting `WherePredicate::EqPredicate` carries a `Type::Generic("T")` LHS.
#[test]
fn test_encode_function_with_equal_predicate_bare_type_param_lhs_succeeds() {
    use domain::tddd::catalogue_v2::FunctionName;
    use domain::tddd::catalogue_v2::entries::FunctionEntry;
    use domain::tddd::catalogue_v2::identifiers::FunctionPath;
    use domain::tddd::catalogue_v2::methods::{BoundOp, MethodGenericParam, WherePredicateDecl};
    use domain::tddd::catalogue_v2::roles::FunctionRole;
    use domain::tddd::catalogue_v2::{ParamName, TypeRef};
    use rustdoc_types::{Term, Type, WherePredicate};

    let mut doc = make_doc("domain");
    let crate_n = CrateName::new("domain").unwrap();
    let fn_path = FunctionPath::at_root(crate_n, FunctionName::new("bare_lhs_eq_fn").unwrap());
    let entry = FunctionEntry {
        action: ItemAction::Add,
        role: FunctionRole::FreeFunction,
        params: vec![],
        returns: TypeRef::new("()").unwrap(),
        is_async: false,
        generics: vec![MethodGenericParam { name: ParamName::new("T").unwrap(), bounds: vec![] }],
        // Permissive: bare type parameter as Equal-predicate LHS (`where T = u32`).
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

    let ec = CatalogueToExtendedCrateCodec::new()
        .encode(doc)
        .expect("bare type param LHS must succeed under permissive principle");
    let fn_item = ec
        .krate()
        .index
        .values()
        .find(|item| {
            item.name.as_deref() == Some("bare_lhs_eq_fn")
                && matches!(item.inner, ItemEnum::Function(_))
        })
        .expect("expected Function item for bare_lhs_eq_fn");
    let ItemEnum::Function(ref f) = fn_item.inner else { panic!("expected Function") };
    assert_eq!(f.generics.where_predicates.len(), 1);
    // LHS must be Type::Generic("T"); RHS must be Term::Type.
    let WherePredicate::EqPredicate { ref lhs, ref rhs } = f.generics.where_predicates[0] else {
        panic!("expected EqPredicate, got {:?}", f.generics.where_predicates[0]);
    };
    assert!(
        matches!(lhs, Type::Generic(n) if n == "T"),
        "expected Type::Generic(\"T\"), got {lhs:?}"
    );
    assert!(matches!(rhs, Term::Type(_)), "expected Term::Type, got {rhs:?}");
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
    // The BoundPredicate for `F` must contain a GenericBound::Outlives("'static").
    // The apostrophe is included so that A-codec Outlives strings match the C-side
    // rustdoc representation (rustdoc stores `"'static"` not `"static"`).
    let has_static_outlives = f.generics.where_predicates.iter().any(|wp| {
        if let WherePredicate::BoundPredicate { type_, bounds, .. } = wp {
            matches!(type_, Type::Generic(g) if g == "F")
                && bounds.iter().any(|b| matches!(b, GenericBound::Outlives(lt) if lt == "'static"))
        } else {
            false
        }
    });
    assert!(
        has_static_outlives,
        "encoded where_predicates must contain Outlives(\"'static\") for `F: 'static`, \
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

    // The BoundPredicate for `T` must contain a GenericBound::Outlives("'a").
    // The apostrophe is included so that A-codec Outlives strings match the C-side
    // rustdoc representation (rustdoc stores `"'a"` not `"a"`).
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
                && bounds.iter().any(|b| matches!(b, GenericBound::Outlives(lt) if lt == "'a"))
        } else {
            false
        }
    });
    assert!(
        has_named_outlives,
        "encoded where_predicates must contain Outlives(\"'a\") for `T: 'a`, \
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

// -----------------------------------------------------------------------
// T007 / AC-08: A-codec — impl-block-level generics encoding
// (IN-09: InherentImplDeclV2.impl_generics, TraitImplDeclV2.impl_generics,
//  TraitEntry.generics)
// -----------------------------------------------------------------------

/// T007 AC-08 (a): `TraitEntry` with `generics: [T]` encodes the trait-level
/// generic as a `GenericParamDef` in the Trait item's `Generics`.
///
/// The codec must call `build_where_form_generics` for trait-level generics so that
/// `trait Foo<T>` produces a `Trait` item with one type param in its `generics.params`,
/// not `empty_generics()`.
#[test]
fn test_trait_decl_generics_encoded_correctly() {
    use domain::tddd::catalogue_v2::WherePredicateDecl;
    use rustdoc_types::{GenericParamDefKind, WherePredicate};

    let mut doc = make_doc("domain");
    let method_generic = MethodGenericParam {
        name: ParamName::new("T").unwrap(),
        bounds: vec![TypeRef::new("Clone").unwrap()],
    };
    let where_pred = WherePredicateDecl {
        lhs: TypeRef::new("T").unwrap(),
        operator: domain::tddd::catalogue_v2::BoundOp::Bound,
        rhs: vec![TypeRef::new("Send").unwrap()],
    };
    doc.traits.insert(
        TraitName::new("MyTrait").unwrap(),
        TraitEntry {
            action: ItemAction::Add,
            role: ContractRole::SpecificationPort,
            methods: vec![],
            assoc_types: vec![],
            assoc_consts: vec![],
            supertrait_bounds: vec![],
            generics: vec![method_generic],
            where_predicates: vec![where_pred],
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        },
    );

    let ec = CatalogueToExtendedCrateCodec::new().encode(doc).unwrap();
    let krate = ec.krate();

    // Find the Trait item for "MyTrait".
    let trait_item = krate
        .index
        .values()
        .find(|item| {
            item.name.as_deref() == Some("MyTrait") && matches!(item.inner, ItemEnum::Trait(_))
        })
        .expect("MyTrait trait item must be present");
    let ItemEnum::Trait(ref trait_inner) = trait_item.inner else {
        panic!("expected Trait inner");
    };

    // generics.params must have one type param "T".
    assert_eq!(
        trait_inner.generics.params.len(),
        1,
        "TraitEntry with generics:[T] must produce 1 GenericParamDef, got: {:?}",
        trait_inner.generics.params
    );
    assert_eq!(trait_inner.generics.params[0].name, "T");
    assert!(
        matches!(trait_inner.generics.params[0].kind, GenericParamDefKind::Type { .. }),
        "type param T must be GenericParamDefKind::Type"
    );

    // In where-form encoding: bounds from `generics[T].bounds` and `where_predicates`
    // are both emitted as WherePredicate::BoundPredicate entries.
    // The `T: Clone` inline bound and `T: Send` where_predicate should both appear.
    let wp_lhs_strings: Vec<String> = trait_inner
        .generics
        .where_predicates
        .iter()
        .filter_map(|wp| {
            if let WherePredicate::BoundPredicate {
                type_: rustdoc_types::Type::Generic(n), ..
            } = wp
            {
                Some(n.clone())
            } else {
                None
            }
        })
        .collect();
    // Both "T: Clone" (from generics.bounds) and "T: Send" (from where_predicates) land
    // in where_predicates as BoundPredicate entries. They may be merged into one or kept
    // separate depending on build_where_form_generics merging strategy.
    // At minimum, "T" must appear as the LHS of at least one where predicate.
    assert!(
        wp_lhs_strings.contains(&"T".to_string()),
        "T must appear as LHS of at least one WherePredicate::BoundPredicate, got: {:?}",
        trait_inner.generics.where_predicates
    );
}

/// T007 AC-08 (b): `TraitImplDeclV2` with `impl_generics: [T]` encodes the
/// impl-block-level generic as a `GenericParamDef` in the Impl item's `Generics`.
///
/// The codec must use `build_where_form_generics` for impl-block generics so that
/// `impl<T: Send> Trait for Foo` produces an Impl item with `generics.params = [T]`,
/// not `empty_generics()`.
#[test]
fn test_trait_impl_block_generics_encoded_correctly() {
    use rustdoc_types::{GenericParamDefKind, WherePredicate};

    let mut doc = make_doc("domain");
    // ADR `2026-05-20-0048` D1/D2: top-level trait_impls; new API: (trait_ref, for_type).
    let mut trait_impl = TraitImplDeclV2::new(
        TypeRef::new("std::marker::Send").unwrap(),
        TypeRef::new("Foo").unwrap(),
    );
    trait_impl.impl_generics = vec![MethodGenericParam {
        name: ParamName::new("T").unwrap(),
        bounds: vec![TypeRef::new("Clone").unwrap()],
    }];
    // impl_where_predicates: T: Send (explicit where predicate)
    trait_impl.impl_where_predicates = vec![domain::tddd::catalogue_v2::WherePredicateDecl {
        lhs: TypeRef::new("T").unwrap(),
        operator: domain::tddd::catalogue_v2::BoundOp::Bound,
        rhs: vec![TypeRef::new("Send").unwrap()],
    }];

    doc.types.insert(
        TypeName::new("Foo").unwrap(),
        TypeEntry {
            action: ItemAction::Add,
            role: DataRole::value_object(),
            kind: TypeKindV2::Struct(StructKind::new(
                StructShape::Plain { fields: vec![], has_stripped_fields: false },
                None,
            )),
            methods: vec![],
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        },
    );
    doc.trait_impls.push(trait_impl);

    let ec = CatalogueToExtendedCrateCodec::new().encode(doc).unwrap();
    let krate = ec.krate();

    // Find the trait Impl item (trait_ is Some).
    let trait_impl_item = krate
        .index
        .values()
        .find(|item| matches!(&item.inner, ItemEnum::Impl(i) if i.trait_.is_some()))
        .expect("must find a trait Impl item");
    let ItemEnum::Impl(ref impl_inner) = trait_impl_item.inner else {
        panic!("expected Impl inner");
    };

    // generics.params must have one type param "T".
    assert_eq!(
        impl_inner.generics.params.len(),
        1,
        "TraitImplDeclV2 with impl_generics:[T] must produce 1 GenericParamDef, got: {:?}",
        impl_inner.generics.params
    );
    assert_eq!(impl_inner.generics.params[0].name, "T");
    assert!(
        matches!(impl_inner.generics.params[0].kind, GenericParamDefKind::Type { .. }),
        "impl generic T must be GenericParamDefKind::Type"
    );

    // where_predicates must contain bound predicates for T (from both impl_generics.bounds
    // and impl_where_predicates).
    let has_t_predicate = impl_inner.generics.where_predicates.iter().any(|wp| {
        matches!(wp, WherePredicate::BoundPredicate { type_: rustdoc_types::Type::Generic(n), .. } if n == "T")
    });
    assert!(
        has_t_predicate,
        "T must appear as LHS of at least one WherePredicate::BoundPredicate in the impl block \
         generics, got: {:?}",
        impl_inner.generics.where_predicates
    );
}

#[test]
fn test_trait_impl_for_type_generic_shadows_same_named_local_type() {
    let mut doc = make_doc("domain");
    doc.types.insert(
        TypeName::new("T").unwrap(),
        TypeEntry {
            action: ItemAction::Add,
            role: DataRole::value_object(),
            kind: TypeKindV2::Struct(StructKind::new(
                StructShape::Plain { fields: vec![], has_stripped_fields: false },
                None,
            )),
            methods: vec![],
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        },
    );
    doc.traits.insert(
        TraitName::new("Port").unwrap(),
        TraitEntry {
            action: ItemAction::Add,
            role: ContractRole::SpecificationPort,
            methods: vec![],
            assoc_types: vec![],
            assoc_consts: vec![],
            supertrait_bounds: vec![],
            generics: vec![],
            where_predicates: vec![],
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        },
    );

    let mut trait_impl =
        TraitImplDeclV2::new(TypeRef::new("Port").unwrap(), TypeRef::new("T").unwrap());
    trait_impl.impl_generics =
        vec![MethodGenericParam { name: ParamName::new("T").unwrap(), bounds: vec![] }];
    doc.trait_impls.push(trait_impl);

    let ec = CatalogueToExtendedCrateCodec::new().encode(doc).unwrap();
    let trait_impl_item = ec
        .krate()
        .index
        .values()
        .find(|item| matches!(&item.inner, ItemEnum::Impl(i) if i.trait_.is_some()))
        .expect("must find a trait Impl item");
    let ItemEnum::Impl(ref impl_inner) = trait_impl_item.inner else {
        panic!("expected Impl inner");
    };

    assert_eq!(
        impl_inner.for_,
        Type::Generic("T".to_string()),
        "impl<T> Port for T must encode the impl target as generic T, not the local type named T"
    );
}

/// T007 AC-08 (c): `InherentImplDeclV2` entries in `CatalogueDocument::inherent_impls`
/// with `impl_generics: [L, R, W]` are encoded as separate Impl items whose
/// `generics.params` contains L, R, W.
///
/// When an `InherentImplDeclV2` is present, the codec must create a *separate*
/// inherent Impl item (in addition to the type's own TypeEntry-driven impl block,
/// if any). The new Impl item must carry the impl-block-level generics.
#[test]
fn test_inherent_impl_block_generics_encoded_correctly() {
    use domain::tddd::catalogue_v2::entries::InherentImplDeclV2;
    use rustdoc_types::GenericParamDefKind;

    let mut doc = make_doc("domain");

    // Register the type "Bar" so the impl block can reference it.
    doc.types.insert(
        TypeName::new("Bar").unwrap(),
        TypeEntry {
            action: ItemAction::Add,
            role: DataRole::value_object(),
            kind: TypeKindV2::Struct(StructKind::new(
                StructShape::Plain { fields: vec![], has_stripped_fields: false },
                None,
            )),
            methods: vec![],

            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        },
    );

    // InherentImplDeclV2 with impl_generics: [L, R, W].
    doc.inherent_impls.push(InherentImplDeclV2 {
        type_name: TypeName::new("Bar").unwrap(),
        impl_generics: vec![
            MethodGenericParam {
                name: ParamName::new("L").unwrap(),
                bounds: vec![TypeRef::new("Send").unwrap()],
            },
            MethodGenericParam { name: ParamName::new("R").unwrap(), bounds: vec![] },
            MethodGenericParam {
                name: ParamName::new("W").unwrap(),
                bounds: vec![TypeRef::new("Sync").unwrap()],
            },
        ],
        impl_where_predicates: vec![],
        methods: vec![],
    });

    let ec = CatalogueToExtendedCrateCodec::new().encode(doc).unwrap();
    let krate = ec.krate();

    // Find the inherent Impl item that was produced from the InherentImplDeclV2
    // (trait_ is None, generics.params is non-empty with [L, R, W]).
    let generic_inherent_impl = krate.index.values().find(|item| {
        if let ItemEnum::Impl(i) = &item.inner {
            i.trait_.is_none() && !i.generics.params.is_empty()
        } else {
            false
        }
    });

    let impl_item = generic_inherent_impl
        .expect("must find an inherent Impl item with non-empty generics from InherentImplDeclV2");
    let ItemEnum::Impl(ref impl_inner) = impl_item.inner else {
        panic!("expected Impl inner");
    };

    assert_eq!(
        impl_inner.generics.params.len(),
        3,
        "InherentImplDeclV2 with impl_generics:[L, R, W] must produce 3 GenericParamDefs, got: {:?}",
        impl_inner.generics.params
    );
    let names: Vec<&str> = impl_inner.generics.params.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, vec!["L", "R", "W"], "generic param names must be L, R, W in order");
    for p in &impl_inner.generics.params {
        assert!(
            matches!(p.kind, GenericParamDefKind::Type { .. }),
            "each impl generic must be GenericParamDefKind::Type, got: {:?}",
            p.kind
        );
    }

    // where_predicates coverage: L has bounds:[Send] and W has bounds:[Sync], so two
    // WherePredicate::BoundPredicate entries must be emitted. R has no bounds and must
    // produce a GenericParamDef but no WherePredicate.
    // (build_where_form_generics emits bounds inline on params only if non-empty)
    use rustdoc_types::WherePredicate;
    let wp_names: Vec<String> = impl_inner
        .generics
        .where_predicates
        .iter()
        .filter_map(|wp| {
            if let WherePredicate::BoundPredicate {
                type_: rustdoc_types::Type::Generic(n), ..
            } = wp
            {
                Some(n.clone())
            } else {
                None
            }
        })
        .collect();
    assert!(
        wp_names.contains(&"L".to_string()),
        "L (bounds:[Send]) must appear as WherePredicate LHS, got: {:?}",
        impl_inner.generics.where_predicates
    );
    assert!(
        wp_names.contains(&"W".to_string()),
        "W (bounds:[Sync]) must appear as WherePredicate LHS, got: {:?}",
        impl_inner.generics.where_predicates
    );
    assert!(
        !wp_names.contains(&"R".to_string()),
        "R (no bounds) must NOT appear as WherePredicate LHS, got: {:?}",
        impl_inner.generics.where_predicates
    );

    // Critical Phase-1 linkage check: the inherent impl's Id must appear in the
    // owning type ("Bar")'s `Struct.impls` list. Without this linkage the impl
    // item exists in the index but the type does not point to it, which means
    // downstream signal evaluation never compares it.
    let impl_id = impl_item.id;
    let bar_item = krate
        .index
        .values()
        .find(|item| {
            item.name.as_deref() == Some("Bar") && matches!(item.inner, ItemEnum::Struct(_))
        })
        .expect("Bar struct item must be present");
    let ItemEnum::Struct(ref bar_struct) = bar_item.inner else {
        panic!("expected Struct inner for Bar");
    };
    assert!(
        bar_struct.impls.contains(&impl_id),
        "inherent impl Id {:?} must be linked in Bar's Struct.impls, got: {:?}",
        impl_id,
        bar_struct.impls
    );
}

/// T007 AC-08 (regression): a catalogue without `TraitEntry.generics` / `impl_generics`
/// (legacy empty-Vec fields) must encode to an item with `empty_generics()` for trait
/// and impl blocks, preserving the existing (pre-T007) signal evaluation behaviour.
#[test]
fn test_existing_catalogue_no_change_in_signal_for_trait_no_generics() {
    let mut doc = make_doc("domain");
    doc.traits.insert(
        TraitName::new("MyPort").unwrap(),
        TraitEntry {
            action: ItemAction::Add,
            role: ContractRole::SpecificationPort,
            methods: vec![],
            assoc_types: vec![],
            assoc_consts: vec![],
            supertrait_bounds: vec![],
            generics: vec![],         // empty = old catalogue
            where_predicates: vec![], // empty = old catalogue
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        },
    );

    let ec = CatalogueToExtendedCrateCodec::new().encode(doc).unwrap();
    let krate = ec.krate();

    let trait_item = krate
        .index
        .values()
        .find(|item| {
            item.name.as_deref() == Some("MyPort") && matches!(item.inner, ItemEnum::Trait(_))
        })
        .expect("MyPort must be present");
    let ItemEnum::Trait(ref t) = trait_item.inner else { panic!("expected Trait") };

    // No generics declared → empty_generics().
    assert!(
        t.generics.params.is_empty(),
        "trait with no generics must encode to empty params, got: {:?}",
        t.generics.params
    );
    assert!(
        t.generics.where_predicates.is_empty(),
        "trait with no generics must encode to empty where_predicates, got: {:?}",
        t.generics.where_predicates
    );
}

#[test]
fn test_trait_assoc_items_encode_trait_generic_projection_types() {
    let mut doc = make_doc("domain");
    doc.traits.insert(
        TraitName::new("ProjectionPort").unwrap(),
        TraitEntry {
            action: ItemAction::Add,
            role: ContractRole::SpecificationPort,
            methods: vec![],
            assoc_types: vec![AssocTypeDecl {
                name: TypeName::new("Output").unwrap(),
                bounds: vec![],
                default: Some(TypeRef::new("Vec<T::Item>").unwrap()),
            }],
            assoc_consts: vec![AssocConstDecl {
                name: AssocConstName::new("ID").unwrap(),
                ty: TypeRef::new("<T as Iterator>::Item").unwrap(),
                default_value: None,
            }],
            supertrait_bounds: vec![],
            generics: vec![MethodGenericParam {
                name: ParamName::new("T").unwrap(),
                bounds: vec![],
            }],
            where_predicates: vec![],
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        },
    );

    let encoded = CatalogueToExtendedCrateCodec::new().encode(doc).unwrap();
    let krate = encoded.krate();
    let trait_item = krate
        .index
        .values()
        .find(|item| {
            item.name.as_deref() == Some("ProjectionPort")
                && matches!(item.inner, ItemEnum::Trait(_))
        })
        .expect("ProjectionPort trait must be encoded");
    let ItemEnum::Trait(trait_inner) = &trait_item.inner else { panic!("expected Trait") };

    let assoc_type = trait_inner
        .items
        .iter()
        .filter_map(|id| krate.index.get(id))
        .find(|item| item.name.as_deref() == Some("Output"))
        .expect("Output associated type must be linked from Trait.items");
    let ItemEnum::AssocType { type_: Some(Type::ResolvedPath(vec_path)), .. } = &assoc_type.inner
    else {
        panic!("expected assoc type default Vec<T::Item>, got {:?}", assoc_type.inner);
    };
    let Some(GenericArgs::AngleBracketed { args, .. }) = vec_path.args.as_deref() else {
        panic!("Vec default must carry generic args: {vec_path:?}");
    };
    let Some(GenericArg::Type(Type::QualifiedPath { name, self_type, trait_, .. })) = args.first()
    else {
        panic!("Vec<T::Item> arg must encode as QualifiedPath, got {args:?}");
    };
    assert_eq!(name, "Item");
    assert!(trait_.is_none(), "T::Item projection must have no explicit trait path");
    assert_eq!(self_type.as_ref(), &Type::Generic("T".to_string()));

    let assoc_const = trait_inner
        .items
        .iter()
        .filter_map(|id| krate.index.get(id))
        .find(|item| item.name.as_deref() == Some("ID"))
        .expect("ID associated const must be linked from Trait.items");
    let ItemEnum::AssocConst { type_: Type::QualifiedPath { name, self_type, trait_, .. }, .. } =
        &assoc_const.inner
    else {
        panic!("expected assoc const type <T as Iterator>::Item, got {:?}", assoc_const.inner);
    };
    assert_eq!(name, "Item");
    assert_eq!(self_type.as_ref(), &Type::Generic("T".to_string()));
    assert!(
        trait_.as_ref().is_some_and(|path| path.path.ends_with("Iterator")),
        "expected Iterator trait path, got {trait_:?}"
    );
}

#[test]
fn test_trait_assoc_items_reject_invalid_trait_generic_projection_name() {
    let mut doc = make_doc("domain");
    doc.traits.insert(
        TraitName::new("InvalidProjectionPort").unwrap(),
        TraitEntry {
            action: ItemAction::Add,
            role: ContractRole::SpecificationPort,
            methods: vec![],
            assoc_types: vec![AssocTypeDecl {
                name: TypeName::new("Output").unwrap(),
                bounds: vec![],
                default: Some(TypeRef::new("T::Item-foo").unwrap()),
            }],
            assoc_consts: vec![],
            supertrait_bounds: vec![],
            generics: vec![MethodGenericParam {
                name: ParamName::new("T").unwrap(),
                bounds: vec![],
            }],
            where_predicates: vec![],
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        },
    );

    let result = CatalogueToExtendedCrateCodec::new().encode(doc);
    assert!(
        matches!(result, Err(domain::tddd::NewTypeGraphCodecError::InvalidTypeRef(_))),
        "invalid associated projection names must fall through to parser validation, got {result:?}"
    );
}

#[test]
fn test_trait_assoc_items_resolve_external_ids_inside_explicit_qualified_paths() {
    let mut doc = make_doc("domain");
    doc.traits.insert(
        TraitName::new("ExternalProjectionPort").unwrap(),
        TraitEntry {
            action: ItemAction::Add,
            role: ContractRole::SpecificationPort,
            methods: vec![],
            assoc_types: vec![],
            assoc_consts: vec![AssocConstDecl {
                name: AssocConstName::new("EXTERNAL").unwrap(),
                ty: TypeRef::new("<ext::Foo as ext::Trait>::Assoc").unwrap(),
                default_value: None,
            }],
            supertrait_bounds: vec![],
            generics: vec![],
            where_predicates: vec![],
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        },
    );

    let encoded = CatalogueToExtendedCrateCodec::new().encode(doc).unwrap();
    let krate = encoded.krate();
    let trait_item = krate
        .index
        .values()
        .find(|item| {
            item.name.as_deref() == Some("ExternalProjectionPort")
                && matches!(item.inner, ItemEnum::Trait(_))
        })
        .expect("ExternalProjectionPort trait must be encoded");
    let ItemEnum::Trait(trait_inner) = &trait_item.inner else { panic!("expected Trait") };

    let assoc_const = trait_inner
        .items
        .iter()
        .filter_map(|id| krate.index.get(id))
        .find(|item| item.name.as_deref() == Some("EXTERNAL"))
        .expect("EXTERNAL associated const must be linked from Trait.items");
    let ItemEnum::AssocConst { type_: Type::QualifiedPath { self_type, trait_, .. }, .. } =
        &assoc_const.inner
    else {
        panic!("expected explicit qualified path type, got {:?}", assoc_const.inner);
    };
    let Type::ResolvedPath(self_path) = self_type.as_ref() else {
        panic!("expected external self type path, got {self_type:?}");
    };
    assert_eq!(self_path.path, "ext::Foo");
    assert_ne!(
        self_path.id,
        Id(UNRESOLVED_CRATE_ID),
        "qualified-path self_type external id must be resolved"
    );
    assert!(
        krate.paths.contains_key(&self_path.id),
        "resolved external self_type id must have a path summary"
    );

    let trait_path = trait_.as_ref().expect("qualified path must keep the explicit trait path");
    assert_eq!(trait_path.path, "ext::Trait");
    assert_ne!(
        trait_path.id,
        Id(UNRESOLVED_CRATE_ID),
        "qualified-path trait external id must be resolved"
    );
    assert!(
        krate.paths.contains_key(&trait_path.id),
        "resolved external trait id must have a path summary"
    );
}

#[test]
fn test_trait_assoc_items_rewrite_nested_trait_generic_projections() {
    fn assert_t_item_projection(ty: &Type, context: &str) {
        let Type::QualifiedPath { name, self_type, trait_, .. } = ty else {
            panic!("{context}: expected T::Item qualified projection, got {ty:?}");
        };
        assert_eq!(name, "Item", "{context}: associated item name");
        assert!(trait_.is_none(), "{context}: T::Item must have no explicit trait path");
        assert_eq!(
            self_type.as_ref(),
            &Type::Generic("T".to_string()),
            "{context}: projection self type"
        );
    }

    fn iterator_item_constraint_type<'a>(path: &'a rustdoc_types::Path, context: &str) -> &'a Type {
        let Some(GenericArgs::AngleBracketed { constraints, .. }) = path.args.as_deref() else {
            panic!("{context}: expected Iterator associated-item constraint args, got {path:?}");
        };
        let item_constraint = constraints
            .iter()
            .find(|constraint| constraint.name == "Item")
            .expect("Iterator<Item = ...> constraint must be present");
        let AssocItemConstraintKind::Equality(Term::Type(ty)) = &item_constraint.binding else {
            panic!("{context}: expected Item equality type, got {:?}", item_constraint.binding);
        };
        ty
    }

    fn assert_iterator_constraint_projects_t_item(path: &rustdoc_types::Path, context: &str) {
        let ty = iterator_item_constraint_type(path, context);
        assert_t_item_projection(ty, context);
    }

    let mut doc = make_doc("domain");
    doc.traits.insert(
        TraitName::new("NestedProjectionPort").unwrap(),
        TraitEntry {
            action: ItemAction::Add,
            role: ContractRole::SpecificationPort,
            methods: vec![],
            assoc_types: vec![
                AssocTypeDecl {
                    name: TypeName::new("FnOutput").unwrap(),
                    bounds: vec![],
                    default: Some(TypeRef::new("fn() -> T::Item").unwrap()),
                },
                AssocTypeDecl {
                    name: TypeName::new("ImplOutput").unwrap(),
                    bounds: vec![],
                    default: Some(TypeRef::new("impl Iterator<Item = T::Item>").unwrap()),
                },
                AssocTypeDecl {
                    name: TypeName::new("DynOutput").unwrap(),
                    bounds: vec![],
                    default: Some(TypeRef::new("dyn Iterator<Item = T::Item>").unwrap()),
                },
                AssocTypeDecl {
                    name: TypeName::new("BoundedOutput").unwrap(),
                    bounds: vec![TypeRef::new("Iterator<Item = T::Item>").unwrap()],
                    default: None,
                },
                AssocTypeDecl {
                    name: TypeName::new("ShadowedOutput").unwrap(),
                    bounds: vec![TypeRef::new("Iterator<Item = From>").unwrap()],
                    default: None,
                },
            ],
            assoc_consts: vec![],
            supertrait_bounds: vec![],
            generics: vec![
                MethodGenericParam { name: ParamName::new("T").unwrap(), bounds: vec![] },
                MethodGenericParam { name: ParamName::new("From").unwrap(), bounds: vec![] },
            ],
            where_predicates: vec![],
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        },
    );

    let encoded = CatalogueToExtendedCrateCodec::new().encode(doc).unwrap();
    let krate = encoded.krate();
    assert!(
        !krate.external_crates.values().any(|krate| krate.name == "T"),
        "trait generic projection T::Item must not register T as an external crate: {:?}",
        krate.external_crates
    );
    let trait_item = krate
        .index
        .values()
        .find(|item| {
            item.name.as_deref() == Some("NestedProjectionPort")
                && matches!(item.inner, ItemEnum::Trait(_))
        })
        .expect("NestedProjectionPort trait must be encoded");
    let ItemEnum::Trait(trait_inner) = &trait_item.inner else { panic!("expected Trait") };

    let find_assoc_type = |assoc_name: &str| {
        trait_inner
            .items
            .iter()
            .filter_map(|id| krate.index.get(id))
            .find(|item| item.name.as_deref() == Some(assoc_name))
            .unwrap_or_else(|| {
                panic!("{assoc_name} associated type must be linked from Trait.items")
            })
    };

    let ItemEnum::AssocType { type_: Some(Type::FunctionPointer(fn_ptr)), .. } =
        &find_assoc_type("FnOutput").inner
    else {
        panic!("FnOutput must encode as a function pointer");
    };
    let fn_output = fn_ptr.sig.output.as_ref().expect("function pointer must have output type");
    assert_t_item_projection(fn_output, "function pointer output");

    let ItemEnum::AssocType { type_: Some(Type::ImplTrait(bounds)), .. } =
        &find_assoc_type("ImplOutput").inner
    else {
        panic!("ImplOutput must encode as impl Trait");
    };
    let Some(GenericBound::TraitBound { trait_: impl_trait_path, .. }) = bounds.first() else {
        panic!("ImplOutput must carry an Iterator trait bound, got {bounds:?}");
    };
    assert_iterator_constraint_projects_t_item(impl_trait_path, "impl trait constraint");

    let ItemEnum::AssocType { type_: Some(Type::DynTrait(dyn_trait)), .. } =
        &find_assoc_type("DynOutput").inner
    else {
        panic!("DynOutput must encode as dyn Trait");
    };
    let Some(poly_trait) = dyn_trait.traits.first() else {
        panic!("DynOutput must carry an Iterator trait");
    };
    assert_iterator_constraint_projects_t_item(&poly_trait.trait_, "dyn trait constraint");

    let ItemEnum::AssocType { bounds, .. } = &find_assoc_type("BoundedOutput").inner else {
        panic!("BoundedOutput must encode as an associated type");
    };
    let Some(GenericBound::TraitBound { trait_: bounded_trait_path, .. }) = bounds.first() else {
        panic!("BoundedOutput must carry an Iterator trait bound, got {bounds:?}");
    };
    assert_iterator_constraint_projects_t_item(bounded_trait_path, "assoc type bound constraint");

    let ItemEnum::AssocType { bounds, .. } = &find_assoc_type("ShadowedOutput").inner else {
        panic!("ShadowedOutput must encode as an associated type");
    };
    let Some(GenericBound::TraitBound { trait_: shadowed_trait_path, .. }) = bounds.first() else {
        panic!("ShadowedOutput must carry an Iterator trait bound, got {bounds:?}");
    };
    let shadowed_item_type =
        iterator_item_constraint_type(shadowed_trait_path, "shadowed generic bound constraint");
    assert_eq!(
        shadowed_item_type,
        &Type::Generic("From".to_string()),
        "trait generic `From` must shadow the std prelude trait in assoc type bounds"
    );
}
