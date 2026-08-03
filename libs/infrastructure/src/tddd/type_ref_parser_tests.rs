//! Tests for [`type_ref_parser`] (split out to keep the main module under the 200-400 line guideline).

use std::collections::HashMap;

use rustdoc_types::{
    AssocItemConstraintKind, GenericArg, GenericArgs, GenericBound, Id, Path, Type,
};

use super::*;

fn no_local(_name: &str) -> Option<Id> {
    None
}

fn simple_local(name: &str) -> Option<Id> {
    match name {
        "User" => Some(Id(10)),
        "DomainError" => Some(Id(11)),
        "UserId" => Some(Id(12)),
        _ => None,
    }
}

fn parse_with<F>(s: &str, resolve_local: F, std_crate_id: u32) -> Type
where
    F: Fn(&str) -> Option<Id>,
{
    let mut ext_ids: HashMap<String, u32> = HashMap::new();
    let mut counter = std_crate_id + 1;
    parse_type_ref(s, &resolve_local, std_crate_id, &ext_ids.clone(), &mut |name: String| {
        let id = counter;
        counter += 1;
        ext_ids.insert(name, id);
        id
    })
    .unwrap()
}

fn parse(s: &str) -> Type {
    parse_with(s, no_local, 100)
}

fn parse_local(s: &str) -> Type {
    parse_with(s, simple_local, 100)
}

fn parse_generic_projection(s: &str, generic_params: &[&str]) -> Type {
    let mut emitted = |_name: String| 1_u32;
    parse_type_ref_with_generics(s, &no_local, 100, &HashMap::new(), &mut emitted, generic_params)
        .unwrap()
}

// -----------------------------------------------------------------------
// AC-06: std prelude type auto-resolution
// -----------------------------------------------------------------------

#[test]
fn test_parse_type_ref_vec_resolves_to_std_resolved_path() {
    let ty = parse_with("Vec<UserId>", simple_local, 100);
    match ty {
        Type::ResolvedPath(p) => {
            assert!(p.path.contains("Vec"), "path: {}", p.path);
        }
        other => panic!("expected ResolvedPath, got: {other:?}"),
    }
}

#[test]
fn test_parse_type_ref_option_resolves_to_std_resolved_path() {
    let ty = parse_with("Option<User>", simple_local, 100);
    match ty {
        Type::ResolvedPath(p) => {
            assert!(p.path.contains("Option"), "path: {}", p.path);
        }
        other => panic!("expected ResolvedPath, got: {other:?}"),
    }
}

#[test]
fn test_parse_type_ref_result_with_generic_args_succeeds() {
    let ty = parse_with("Result<Option<User>, DomainError>", simple_local, 100);
    match &ty {
        Type::ResolvedPath(p) => {
            assert!(p.path.contains("Result"), "path: {}", p.path);
            assert!(p.args.is_some(), "expected generic args");
            match p.args.as_deref() {
                Some(GenericArgs::AngleBracketed { args, .. }) => {
                    assert_eq!(args.len(), 2, "expected 2 generic args");
                }
                other => panic!("expected AngleBracketed, got: {other:?}"),
            }
        }
        other => panic!("expected ResolvedPath for Result, got: {other:?}"),
    }
}

#[test]
fn test_parse_type_ref_nested_dyn_trait_preserves_hrtb_binder() {
    let ty = parse("Box<dyn for<'a> Fn(&'a str)>");
    let Type::ResolvedPath(path) = ty else {
        panic!("expected Box resolved path");
    };
    let Some(GenericArgs::AngleBracketed { args, .. }) = path.args.as_deref() else {
        panic!("expected Box angle-bracketed arguments");
    };
    let Some(GenericArg::Type(Type::DynTrait(dyn_trait))) = args.first() else {
        panic!("expected dyn trait argument");
    };
    let Some(poly_trait) = dyn_trait.traits.first() else {
        panic!("expected nested Fn poly-trait");
    };
    assert_eq!(poly_trait.generic_params.len(), 1);
    assert_eq!(poly_trait.generic_params.first().map(|param| param.name.as_str()), Some("'a"));
}

#[test]
fn test_parse_type_ref_unnamed_function_pointer_argument_uses_rustdoc_placeholder() {
    let ty = parse("Trait<fn(u8)>");
    let Type::ResolvedPath(path) = ty else {
        panic!("expected Trait resolved path");
    };
    let Some(GenericArgs::AngleBracketed { args, .. }) = path.args.as_deref() else {
        panic!("expected Trait angle-bracketed arguments");
    };
    let Some(GenericArg::Type(Type::FunctionPointer(function_pointer))) = args.first() else {
        panic!("expected function pointer argument");
    };
    assert_eq!(function_pointer.sig.inputs.first().map(|input| input.0.as_str()), Some("_"));
}

#[test]
fn test_parse_type_ref_generic_parameter_shadows_catalogue_type() {
    let parsed = parse_type_ref_with_generics(
        "T",
        &|name| (name == "T").then_some(Id(42)),
        100,
        &HashMap::new(),
        &mut |_| 101,
        &["T"],
    )
    .unwrap();
    assert!(matches!(parsed, Type::Generic(name) if name == "T"));
}

#[test]
fn test_parse_type_ref_generic_parameter_with_arguments_is_unresolved() {
    for type_ref in ["T<U>", "T::<U>", "T<U>::Item"] {
        let ty = parse_with_generics(type_ref, &["T"]);
        let Type::ResolvedPath(path) = ty else {
            panic!("expected unresolved path for {type_ref}, got: {ty:?}");
        };
        assert_eq!(path.path, "<generic_with_arguments>");
        assert_eq!(path.id, Id(UNRESOLVED_CRATE_ID));
    }
}

#[test]
fn test_parse_generic_bound_generic_argument_shadows_catalogue_type() {
    let bound = parse_generic_bound_with_generics_preserving_spelling(
        "Into<T>",
        &|name| (name == "T").then_some(Id(42)),
        100,
        &HashMap::new(),
        &mut |_| 101,
        &["T"],
    )
    .unwrap();
    let GenericBound::TraitBound { trait_, .. } = bound else {
        panic!("expected trait bound");
    };
    let Some(GenericArgs::AngleBracketed { args, .. }) = trait_.args.as_deref() else {
        panic!("expected Into angle-bracketed arguments");
    };
    assert!(matches!(args.first(), Some(GenericArg::Type(Type::Generic(name))) if name == "T"));
}

#[test]
fn test_parse_type_ref_associated_constraint_preserves_hrtb_binder() {
    let ty = parse("Outer<Item: for<'a> Tr<&'a str>>");
    let Type::ResolvedPath(path) = ty else {
        panic!("expected Outer resolved path");
    };
    let Some(GenericArgs::AngleBracketed { constraints, .. }) = path.args.as_deref() else {
        panic!("expected Outer angle-bracketed arguments");
    };
    let Some(constraint) = constraints.first() else {
        panic!("expected associated-type constraint");
    };
    let AssocItemConstraintKind::Constraint(bounds) = &constraint.binding else {
        panic!("expected associated-type bound constraint");
    };
    let Some(GenericBound::TraitBound { generic_params, .. }) = bounds.first() else {
        panic!("expected HRTB trait bound");
    };
    assert_eq!(generic_params.first().map(|param| param.name.as_str()), Some("'a"));
}

#[test]
fn test_parse_type_ref_fn_bound_explicit_unit_output_normalizes_to_none() {
    let GenericBound::TraitBound { trait_, .. } =
        parse_generic_bound_with_generics_preserving_spelling(
            "Fn() -> ()",
            &no_local,
            100,
            &HashMap::new(),
            &mut |_| 101,
            &[],
        )
        .unwrap()
    else {
        panic!("expected Fn trait bound");
    };
    let Some(GenericArgs::Parenthesized { output, .. }) = trait_.args.as_deref() else {
        panic!("expected Fn parenthesized arguments");
    };
    assert!(output.is_none(), "explicit unit output must match rustdoc's absent output");
}

#[test]
fn test_parse_type_ref_array_length_constant_expression_is_evaluated() {
    let ty = parse("[u8; 1 + 2]");
    let Type::Array { len, .. } = ty else {
        panic!("expected array type");
    };
    assert_eq!(len, "3");
}

#[test]
fn test_parse_type_ref_array_length_expression_uses_usize_shift_semantics() {
    let ty = parse("[u8; 0x8000000000000000 << 1]");
    let Type::Array { len, .. } = ty else {
        panic!("expected array type");
    };
    assert_eq!(len, "0");
}

#[test]
fn test_parse_type_ref_associated_constraint_preserves_associated_item_arguments() {
    let ty = parse("Outer<Item<'a>: Bound>");
    let Type::ResolvedPath(path) = ty else {
        panic!("expected Outer resolved path");
    };
    let Some(GenericArgs::AngleBracketed { constraints, .. }) = path.args.as_deref() else {
        panic!("expected Outer angle-bracketed arguments");
    };
    let Some(constraint) = constraints.first() else {
        panic!("expected associated-type constraint");
    };
    let Some(GenericArgs::AngleBracketed { args, .. }) = constraint.args.as_deref() else {
        panic!("expected associated-item arguments");
    };
    assert!(matches!(args.first(), Some(GenericArg::Lifetime(lifetime)) if lifetime == "'a"));
}

// -----------------------------------------------------------------------
// AC-06: primitive types
// -----------------------------------------------------------------------

#[test]
fn test_parse_type_ref_bool_returns_primitive() {
    let ty = parse("bool");
    assert!(matches!(&ty, Type::Primitive(p) if p == "bool"), "got: {ty:?}");
}

#[test]
fn test_parse_type_ref_u32_returns_primitive() {
    let ty = parse("u32");
    assert!(matches!(&ty, Type::Primitive(p) if p == "u32"), "got: {ty:?}");
}

#[test]
fn test_parse_type_ref_str_returns_primitive() {
    let ty = parse("str");
    assert!(matches!(&ty, Type::Primitive(p) if p == "str"), "got: {ty:?}");
}

// -----------------------------------------------------------------------
// AC-06: local catalogue types
// -----------------------------------------------------------------------

#[test]
fn test_parse_type_ref_local_type_resolves_with_correct_id() {
    let ty = parse_local("User");
    match ty {
        Type::ResolvedPath(p) => {
            assert_eq!(p.path, "User");
            assert_eq!(p.id, Id(10));
        }
        other => panic!("expected ResolvedPath(User), got: {other:?}"),
    }
}

// -----------------------------------------------------------------------
// AC-06: unresolved marker for undeclared types
// -----------------------------------------------------------------------

#[test]
fn test_parse_type_ref_unknown_type_produces_unresolved_marker() {
    let ty = parse("UnknownType");
    match ty {
        Type::ResolvedPath(p) => {
            assert_eq!(p.id, Id(UNRESOLVED_CRATE_ID));
            assert_eq!(p.path, "UnknownType");
        }
        other => panic!("expected unresolved ResolvedPath, got: {other:?}"),
    }
}

// -----------------------------------------------------------------------
// External crate prefixed reference
// -----------------------------------------------------------------------

#[test]
fn test_parse_type_ref_crate_prefixed_emits_external_crate() {
    let mut emitted: Vec<String> = vec![];
    let mut counter = 200u32;
    let result = parse_type_ref(
        "domain_core::UserId",
        &no_local,
        100,
        &HashMap::new(),
        &mut |name: String| {
            emitted.push(name.clone());
            counter += 1;
            counter
        },
    );
    let ty = result.unwrap();
    assert!(emitted.contains(&"domain_core".to_string()), "emitted: {emitted:?}");
    match ty {
        Type::ResolvedPath(p) => {
            assert!(p.path.contains("domain_core"), "path: {}", p.path);
        }
        other => panic!("expected ResolvedPath, got: {other:?}"),
    }
}

// -----------------------------------------------------------------------
// Tuple type
// -----------------------------------------------------------------------

#[test]
fn test_parse_type_ref_unit_tuple_succeeds() {
    let ty = parse("()");
    assert!(matches!(&ty, Type::Tuple(items) if items.is_empty()), "got: {ty:?}");
}

#[test]
fn test_parse_type_ref_tuple_with_elements() {
    let ty = parse("(u32, u64)");
    match &ty {
        Type::Tuple(items) => assert_eq!(items.len(), 2),
        other => panic!("expected Tuple, got: {other:?}"),
    }
}

// -----------------------------------------------------------------------
// Reference type
// -----------------------------------------------------------------------

#[test]
fn test_parse_type_ref_shared_reference() {
    let ty = parse("&str");
    assert!(matches!(&ty, Type::BorrowedRef { is_mutable, .. } if !is_mutable), "got: {ty:?}");
}

#[test]
fn test_parse_type_ref_mutable_reference() {
    let ty = parse_with("&mut String", no_local, 100);
    assert!(matches!(&ty, Type::BorrowedRef { is_mutable, .. } if *is_mutable), "got: {ty:?}");
}

// -----------------------------------------------------------------------
// Slice type
// -----------------------------------------------------------------------

#[test]
fn test_parse_type_ref_slice_succeeds() {
    let ty = parse("[u8]");
    assert!(matches!(&ty, Type::Slice(_)), "got: {ty:?}");
}

// -----------------------------------------------------------------------
// Invalid TypeRef
// -----------------------------------------------------------------------

#[test]
fn test_parse_type_ref_invalid_syntax_returns_err() {
    let result = parse_type_ref("Result<", &no_local, 100, &HashMap::new(), &mut |_: String| 1u32);
    assert!(result.is_err(), "expected parse error for 'Result<'");
}

// -----------------------------------------------------------------------
// T014: QualifiedPath — `<T as Trait>::Assoc` builder (ADR D1)
// -----------------------------------------------------------------------

/// `<Self as ChainIdentity>::Input<'_>` → QualifiedPath with name="Input",
/// self_type=ResolvedPath("Self"), trait_=Some(ChainIdentity path), args=Some(lifetime '_).
#[test]
fn test_qualified_path_self_as_trait_with_generic_args() {
    // simple_local doesn't know "ChainIdentity", so it becomes UNRESOLVED_CRATE_ID.
    let ty = parse_local("<Self as ChainIdentity>::Input<'_>");
    assert_eq!(
        ty,
        Type::QualifiedPath {
            name: "Input".to_owned(),
            self_type: Box::new(Type::ResolvedPath(Path {
                path: "Self".to_owned(),
                id: Id(0),
                args: None,
            })),
            trait_: Some(Path {
                path: "ChainIdentity".to_owned(),
                id: Id(UNRESOLVED_CRATE_ID),
                args: None,
            }),
            args: Some(Box::new(GenericArgs::AngleBracketed {
                args: vec![GenericArg::Lifetime("'_".to_owned())],
                constraints: vec![],
            })),
        }
    );
}

/// `<T as Trait>::Assoc` without generic args → QualifiedPath with args=None.
#[test]
fn test_qualified_path_without_generic_args() {
    let ty = parse("<T as Trait>::Assoc");
    assert_eq!(
        ty,
        Type::QualifiedPath {
            name: "Assoc".to_owned(),
            self_type: Box::new(Type::ResolvedPath(Path {
                path: "T".to_owned(),
                id: Id(UNRESOLVED_CRATE_ID),
                args: None,
            })),
            trait_: Some(Path {
                path: "Trait".to_owned(),
                id: Id(UNRESOLVED_CRATE_ID),
                args: None,
            }),
            args: None,
        }
    );
}

/// `Vec<<T as Trait>::Item>` — QualifiedPath nested inside generic args of Vec.
#[test]
fn test_qualified_path_nested_in_generic_args() {
    let ty = parse("Vec<<T as Trait>::Item>");
    assert_eq!(
        ty,
        Type::ResolvedPath(Path {
            path: "std::vec::Vec".to_owned(),
            id: Id(UNRESOLVED_CRATE_ID),
            args: Some(Box::new(GenericArgs::AngleBracketed {
                args: vec![GenericArg::Type(Type::QualifiedPath {
                    name: "Item".to_owned(),
                    self_type: Box::new(Type::ResolvedPath(Path {
                        path: "T".to_owned(),
                        id: Id(UNRESOLVED_CRATE_ID),
                        args: None,
                    })),
                    trait_: Some(Path {
                        path: "Trait".to_owned(),
                        id: Id(UNRESOLVED_CRATE_ID),
                        args: None,
                    }),
                    args: None,
                })],
                constraints: vec![],
            })),
        })
    );
}

/// Boundary case: `<Self>::Assoc` — qself.position == 0 means no trait prefix → trait_ = None.
#[test]
fn test_qualified_path_position_zero_gives_none_trait() {
    // `<Self>::Assoc` is `qself.position == 0`: no segments before the assoc name.
    let ty = parse("<Self>::Assoc");
    assert_eq!(
        ty,
        Type::QualifiedPath {
            name: "Assoc".to_owned(),
            self_type: Box::new(Type::ResolvedPath(Path {
                path: "Self".to_owned(),
                id: Id(0),
                args: None,
            })),
            trait_: None,
            args: None,
        }
    );
}

#[test]
fn test_qualified_path_with_trailing_segments_returns_unresolved_marker() {
    let ty = parse("<T as Trait>::Assoc::Nested");
    match ty {
        Type::ResolvedPath(p) => {
            assert_eq!(p.path, "<qualified_path_trailing_segments>");
            assert_eq!(p.id, Id(UNRESOLVED_CRATE_ID));
        }
        other => panic!("expected unresolved marker, got: {other:?}"),
    }
}

#[test]
fn test_generic_projection_preserves_all_nested_segments() {
    let ty = parse_generic_projection("T::Assoc::Nested", &["T"]);
    assert_eq!(
        ty,
        Type::QualifiedPath {
            name: "Nested".to_owned(),
            self_type: Box::new(Type::QualifiedPath {
                name: "Assoc".to_owned(),
                self_type: Box::new(Type::Generic("T".to_owned())),
                trait_: None,
                args: None,
            }),
            trait_: None,
            args: None,
        }
    );
}

#[test]
fn test_absolute_path_with_generic_spelling_is_external_crate() {
    let mut emitted = Vec::new();
    let ty = parse_type_ref_with_generics(
        "::T::option::Option<u8>",
        &no_local,
        100,
        &HashMap::new(),
        &mut |name: String| {
            emitted.push(name);
            101
        },
        &["T"],
    )
    .unwrap();

    assert_eq!(emitted, vec!["T"]);
    assert!(matches!(ty, Type::ResolvedPath(path) if path.path == "T::option::Option"));
}

// -----------------------------------------------------------------------
// T015: Generic type parameter recognition (ADR 2026-06-18-0822 D2)
// -----------------------------------------------------------------------

/// Helper: parse with explicit generic_params slice.
fn parse_with_generics(s: &str, generic_params: &[&str]) -> Type {
    let mut ext_ids: HashMap<String, u32> = HashMap::new();
    let mut counter = 101u32;
    parse_type_ref_with_generics(
        s,
        &no_local,
        100,
        &ext_ids.clone(),
        &mut |name: String| {
            let id = counter;
            counter += 1;
            ext_ids.insert(name, id);
            id
        },
        generic_params,
    )
    .unwrap()
}

/// `for_type: "T"` with `generic_params: &["T"]` → `Type::Generic("T")`.
#[test]
fn test_generic_param_name_produces_type_generic() {
    let ty = parse_with_generics("T", &["T"]);
    assert_eq!(ty, Type::Generic("T".to_owned()), "got: {ty:?}");
}

/// `for_type: "T"` with `generic_params: &[]` (empty) → falls through to
/// `Type::ResolvedPath { path: "T", id: UNRESOLVED_CRATE_ID }`.
/// Preserves existing behaviour for non-generic-impl contexts.
#[test]
fn test_generic_param_name_without_generic_params_is_unresolved() {
    let ty = parse_with_generics("T", &[]);
    match ty {
        Type::ResolvedPath(p) => {
            assert_eq!(p.path, "T");
            assert_eq!(p.id, Id(UNRESOLVED_CRATE_ID));
        }
        other => panic!("expected unresolved ResolvedPath, got: {other:?}"),
    }
}

/// `for_type: "MyType"` with `generic_params: &["T"]` → NOT generic; falls through
/// to unresolved-marker because `"MyType"` does not match `"T"`.
#[test]
fn test_non_matching_name_with_generic_params_is_unresolved() {
    let ty = parse_with_generics("MyType", &["T"]);
    match ty {
        Type::ResolvedPath(p) => {
            assert_eq!(p.path, "MyType");
            assert_eq!(p.id, Id(UNRESOLVED_CRATE_ID));
        }
        other => panic!("expected unresolved ResolvedPath, got: {other:?}"),
    }
}

/// Smoke test: a multi-param impl `for_type: "U"` with `generic_params: &["T", "U"]`
/// → `Type::Generic("U")`.
#[test]
fn test_second_generic_param_produces_type_generic() {
    let ty = parse_with_generics("U", &["T", "U"]);
    assert_eq!(ty, Type::Generic("U".to_owned()), "got: {ty:?}");
}

/// Codec smoke test: `TraitImplDeclV2 { impl_generics: [T], for_type: "T" }`
/// encodes the produced rustdoc impl target as `Type::Generic("T")`.
#[test]
fn test_trait_impl_decl_for_type_generic_param_encodes_type_generic() {
    use crate::tddd::catalogue_to_extended_crate_codec::CatalogueToExtendedCrateCodec;
    use domain::tddd::catalogue_v2::entries::TraitEntry;
    use domain::tddd::catalogue_v2::methods::MethodGenericParam;
    use domain::tddd::catalogue_v2::roles::{ContractRole, ItemAction};
    use domain::tddd::catalogue_v2::traits::TraitImplDeclV2;
    use domain::tddd::catalogue_v2::{
        CatalogueDocument, CrateName, ModulePath, ParamName, TraitName, TypeRef,
    };
    use domain::tddd::{CatalogueToExtendedCratePort, LayerId};
    use rustdoc_types::ItemEnum;

    let mut doc = CatalogueDocument::new(
        2,
        CrateName::new("domain").unwrap(),
        LayerId::try_new("domain").unwrap(),
    );
    doc.insert_trait(
        TraitName::new("MyTrait").unwrap(),
        TraitEntry::new(
            ItemAction::Add,
            ContractRole::SpecificationPort,
            vec![],
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

    let trait_impl = TraitImplDeclV2::from_parts(
        domain::tddd::catalogue_v2::ItemAction::Add,
        TypeRef::new("MyTrait").unwrap(),
        TypeRef::new("T").unwrap(),
        vec![MethodGenericParam { name: ParamName::new("T").unwrap(), bounds: vec![] }],
        vec![],
    );
    doc.push_trait_impl(trait_impl);

    let encoded = CatalogueToExtendedCrateCodec::new().encode(doc).unwrap();
    let for_type = encoded
        .krate()
        .index
        .values()
        .find_map(|item| match &item.inner {
            ItemEnum::Impl(impl_) if impl_.trait_.is_some() => Some(&impl_.for_),
            _ => None,
        })
        .unwrap();

    assert_eq!(for_type, &Type::Generic("T".to_owned()), "got: {for_type:?}");
}
