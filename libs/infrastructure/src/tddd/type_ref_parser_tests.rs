//! Tests for [`type_ref_parser`] (split out to keep the main module under the 200-400 line guideline).

use std::collections::HashMap;

use rustdoc_types::{GenericArg, GenericArgs, Id, Path, Type};

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
