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
fn test_parse_type_ref_const_char_argument_preserves_literal() {
    let ty = parse("Marker<'x'>");
    let Type::ResolvedPath(path) = ty else {
        panic!("expected Marker resolved path");
    };
    let Some(GenericArgs::AngleBracketed { args, .. }) = path.args.as_deref() else {
        panic!("expected angle-bracketed arguments");
    };
    let Some(GenericArg::Const(constant)) = args.first() else {
        panic!("expected const character argument");
    };
    assert_eq!(constant.expr, "'x'");
}

#[test]
fn test_parse_type_ref_const_byte_argument_preserves_literal() {
    let ty = parse("Marker<b'x'>");
    let Type::ResolvedPath(path) = ty else {
        panic!("expected Marker resolved path");
    };
    let Some(GenericArgs::AngleBracketed { args, .. }) = path.args.as_deref() else {
        panic!("expected angle-bracketed arguments");
    };
    let Some(GenericArg::Const(constant)) = args.first() else {
        panic!("expected const byte argument");
    };
    assert_eq!(constant.expr, "b'x'");
}

#[test]
fn test_parse_type_ref_const_string_argument_preserves_quotes_and_escapes() {
    let ty = parse(r#"Marker<"\x78">"#);
    let Type::ResolvedPath(path) = ty else {
        panic!("expected Marker resolved path");
    };
    let Some(GenericArgs::AngleBracketed { args, .. }) = path.args.as_deref() else {
        panic!("expected angle-bracketed arguments");
    };
    let Some(GenericArg::Const(constant)) = args.first() else {
        panic!("expected const string argument");
    };
    assert_eq!(constant.expr, r#""\x78""#);
}

#[test]
fn test_parse_type_ref_const_char_argument_preserves_escape_spelling() {
    let ty = parse(r#"Marker<'\x78'>"#);
    let Type::ResolvedPath(path) = ty else {
        panic!("expected Marker resolved path");
    };
    let Some(GenericArgs::AngleBracketed { args, .. }) = path.args.as_deref() else {
        panic!("expected angle-bracketed arguments");
    };
    let Some(GenericArg::Const(constant)) = args.first() else {
        panic!("expected const character argument");
    };
    assert_eq!(constant.expr, r#"'\x78'"#);
}

#[test]
fn test_parse_type_ref_simple_const_blocks_match_rustdoc_spelling() {
    let literal = parse_type_ref_with_generics_preserving_spelling(
        "Marker<{ 1 }>",
        &no_local,
        100,
        &HashMap::new(),
        &mut |_| 101,
        &[],
    )
    .unwrap();
    // A const block rooted at a DECLARED type parameter is rustc's
    // type-used-as-value error (E0423): the schema has no const-parameter
    // declaration, so `{ N }` with `N` declared cannot compile.
    assert!(
        parse_type_ref_with_generics_preserving_spelling(
            "Marker<{ N }>",
            &no_local,
            100,
            &HashMap::new(),
            &mut |_| 101,
            &["N"],
        )
        .is_err(),
        "a const block over a declared type parameter must be rejected"
    );
    // A free-standing const name stays representable (open-world consts).
    let generic = parse_type_ref_with_generics_preserving_spelling(
        "Marker<{ LEN }>",
        &no_local,
        100,
        &HashMap::new(),
        &mut |_| 101,
        &["N"],
    )
    .unwrap();
    // A unit const block would require an unstable const-parameter type
    // (stable rustc permits only integer, `bool`, and `char`), so it is no
    // longer representable on the preserving path.
    assert!(
        parse_type_ref_with_generics_preserving_spelling(
            "Marker<{ () }>",
            &no_local,
            100,
            &HashMap::new(),
            &mut |_| 101,
            &[],
        )
        .is_err(),
        "a unit const block must be rejected on the preserving path"
    );
    let const_expr = |ty: Type| {
        let Type::ResolvedPath(path) = ty else {
            panic!("expected Marker resolved path");
        };
        let Some(GenericArgs::AngleBracketed { args, .. }) = path.args.as_deref() else {
            panic!("expected angle-bracketed arguments");
        };
        let Some(GenericArg::Const(constant)) = args.first() else {
            panic!("expected const argument");
        };
        constant.expr.clone()
    };
    assert_eq!(const_expr(literal), "{ 1 }");
    assert_eq!(const_expr(generic), "{ LEN }");
}

#[test]
fn test_parse_type_ref_preserves_absolute_single_segment_paths() {
    let ty = parse_type_ref_with_generics_preserving_spelling(
        "Into<::Local>",
        &no_local,
        100,
        &HashMap::new(),
        &mut |_| 101,
        &[],
    )
    .unwrap();
    let Type::ResolvedPath(path) = ty else {
        panic!("expected Into resolved path");
    };
    let Some(GenericArgs::AngleBracketed { args, .. }) = path.args.as_deref() else {
        panic!("expected angle-bracketed arguments");
    };
    let Some(GenericArg::Type(Type::ResolvedPath(argument))) = args.first() else {
        panic!("expected resolved path argument");
    };
    assert_eq!(argument.path, "::Local");
}

#[test]
fn test_parse_type_ref_assoc_const_matches_rustdoc_metadata() {
    let ty = parse(r#"Trait<FLAG = true>"#);
    let Type::ResolvedPath(path) = ty else {
        panic!("expected Trait resolved path");
    };
    let Some(GenericArgs::AngleBracketed { constraints, .. }) = path.args.as_deref() else {
        panic!("expected angle-bracketed constraints");
    };
    let Some(constraint) = constraints.first() else {
        panic!("expected associated-const constraint");
    };
    let AssocItemConstraintKind::Equality(rustdoc_types::Term::Constant(constant)) =
        &constraint.binding
    else {
        panic!("expected associated-const equality");
    };
    assert_eq!(constant.expr, "true");
    assert_eq!(constant.value, None);
    assert!(!constant.is_literal);
}

#[test]
fn test_parse_type_ref_assoc_type_equality_remains_a_type() {
    // The equality term must stay a Type rather than being guessed as a
    // constant. (`Self` is no longer usable as the subject here: it is
    // rejected in alias declarations per rustc E0411.)
    let ty = parse_type_ref_with_generics_preserving_spelling(
        "Trait<Item = Foo>",
        &no_local,
        100,
        &HashMap::new(),
        &mut |_| 101,
        &[],
    )
    .unwrap();
    let Type::ResolvedPath(path) = ty else {
        panic!("expected Trait resolved path");
    };
    let Some(GenericArgs::AngleBracketed { constraints, .. }) = path.args.as_deref() else {
        panic!("expected associated-type constraint");
    };
    let Some(constraint) = constraints.first() else {
        panic!("expected associated-type constraint");
    };
    assert!(matches!(
        &constraint.binding,
        AssocItemConstraintKind::Equality(rustdoc_types::Term::Type(Type::ResolvedPath(path)))
            if path.path == "Foo"
    ));
}

#[test]
fn test_parse_type_ref_const_negative_integer_argument_preserves_literal() {
    let ty = parse("Marker<-1>");
    let Type::ResolvedPath(path) = ty else {
        panic!("expected Marker resolved path");
    };
    let Some(GenericArgs::AngleBracketed { args, .. }) = path.args.as_deref() else {
        panic!("expected angle-bracketed arguments");
    };
    let Some(GenericArg::Const(constant)) = args.first() else {
        panic!("expected const integer argument");
    };
    assert_eq!(constant.expr, "-1");
    assert!(constant.is_literal, "negative integer literals must retain rustdoc's literal flag");
}

#[test]
fn test_parse_type_ref_const_suffixed_integer_argument_preserves_literal() {
    let ty = parse("Marker<3usize>");
    let Type::ResolvedPath(path) = ty else {
        panic!("expected Marker resolved path");
    };
    let Some(GenericArgs::AngleBracketed { args, .. }) = path.args.as_deref() else {
        panic!("expected angle-bracketed arguments");
    };
    let Some(GenericArg::Const(constant)) = args.first() else {
        panic!("expected const integer argument");
    };
    assert_eq!(constant.expr, "3usize");
    assert!(constant.is_literal, "suffixed integer literals must retain rustdoc's literal flag");
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
fn test_parse_type_ref_explicit_rust_abi_matches_rustdoc_abi() {
    let ty = parse("Trait<extern \"Rust\" fn()>");
    let Type::ResolvedPath(path) = ty else {
        panic!("expected Trait resolved path");
    };
    let Some(GenericArgs::AngleBracketed { args, .. }) = path.args.as_deref() else {
        panic!("expected angle-bracketed arguments");
    };
    let Some(GenericArg::Type(Type::FunctionPointer(function_pointer))) = args.first() else {
        panic!("expected function pointer argument");
    };
    assert_eq!(function_pointer.header.abi, rustdoc_types::Abi::Rust);
}

#[test]
fn test_parse_type_ref_unknown_abi_preserves_rustdoc_literal_quotes() {
    let ty = parse("Trait<extern \"efiapi\" fn()>");
    let Type::ResolvedPath(path) = ty else {
        panic!("expected Trait resolved path");
    };
    let Some(GenericArgs::AngleBracketed { args, .. }) = path.args.as_deref() else {
        panic!("expected angle-bracketed arguments");
    };
    let Some(GenericArg::Type(Type::FunctionPointer(function_pointer))) = args.first() else {
        panic!("expected function pointer argument");
    };
    assert_eq!(function_pointer.header.abi, rustdoc_types::Abi::Other("\"efiapi\"".to_owned()));
}

#[test]
fn test_parse_type_ref_preserving_spelling_rejects_raw_identifiers() {
    let result = parse_type_ref_with_generics_preserving_spelling(
        "r#Clone",
        &no_local,
        100,
        &HashMap::new(),
        &mut |_| 101,
        &[],
    );
    assert!(result.is_err(), "raw identifiers must fail closed in lexical mode");
}

#[test]
fn test_parse_type_ref_preserving_spelling_keeps_ambiguous_argument_lexeme() {
    let ambiguous = parse_type_ref_with_generics_preserving_spelling(
        "Marker<N>",
        &no_local,
        100,
        &HashMap::new(),
        &mut |_| 101,
        &[],
    );
    let Type::ResolvedPath(path) = ambiguous.unwrap() else {
        panic!("expected Marker resolved path");
    };
    let Some(GenericArgs::AngleBracketed { args, .. }) = path.args.as_deref() else {
        panic!("expected angle-bracketed arguments");
    };
    assert!(matches!(
        args.first(),
        Some(GenericArg::Type(Type::ResolvedPath(path))) if path.path == "N"
    ));

    assert!(
        parse_type_ref_with_generics_preserving_spelling(
            "Into<UserId>",
            &simple_local,
            100,
            &HashMap::new(),
            &mut |_| 101,
            &[],
        )
        .is_ok()
    );
    assert!(validate_lexical_generic_bound("Marker<N>", &[]).is_ok());
    assert!(validate_lexical_generic_bound("Into<String>", &[]).is_ok());

    for modifier in ["[const] Clone", "[ const ] Clone", "for<'a> [const] Clone", "const Clone"] {
        let result = parse_generic_bound_with_generics_preserving_spelling(
            modifier,
            &no_local,
            100,
            &HashMap::new(),
            &mut |_| 101,
            &[],
        );
        assert!(
            result.is_err(),
            "unrepresentable const bound modifier must fail closed: {modifier}"
        );
    }
    // `~const` requires the unstable `const_trait_impl` feature, so no
    // stable compiler-validated rustdoc output can carry it.
    assert!(validate_lexical_generic_bound("~const Clone", &[]).is_err());
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
fn test_parse_generic_bound_raw_pointer_const_argument_is_supported() {
    assert!(validate_lexical_generic_bound("Outer<*const u8>", &[]).is_ok());
    let bound = parse_generic_bound_with_generics_preserving_spelling(
        "Outer<*const u8>",
        &no_local,
        100,
        &HashMap::new(),
        &mut |_| 101,
        &[],
    )
    .unwrap();
    let GenericBound::TraitBound { trait_, .. } = bound else {
        panic!("expected trait bound");
    };
    let Some(GenericArgs::AngleBracketed { args, .. }) = trait_.args.as_deref() else {
        panic!("expected Outer angle-bracketed arguments");
    };
    let Some(GenericArg::Type(Type::RawPointer { is_mutable, type_ })) = args.first() else {
        panic!("expected raw pointer generic argument");
    };
    assert!(!is_mutable, "expected `*const` raw pointer");
    assert!(matches!(type_.as_ref(), Type::Primitive(name) if name == "u8"));
}

#[test]
fn test_validate_generic_bound_rejects_deterministically_non_trait_paths() {
    for bound in ["u8", "bool", "str"] {
        assert!(
            validate_lexical_generic_bound(bound, &[]).is_err(),
            "primitive path must be rejected as a bound: {bound}"
        );
    }
    assert!(
        validate_lexical_generic_bound("U", &["U"]).is_err(),
        "generic-parameter-rooted bound must be rejected"
    );
    assert!(validate_lexical_generic_bound("str::pattern::Pattern", &[]).is_ok());
    assert!(validate_lexical_generic_bound("Clone", &[]).is_ok());

    for (bound, generic_params) in
        [("Outer<Item: u8>", &[] as &[&str]), ("Outer<Item: U>", &["U"] as &[&str])]
    {
        assert!(
            validate_lexical_generic_bound(bound, generic_params).is_err(),
            "nested non-trait bound must be rejected: {bound}"
        );
        let result = parse_generic_bound_with_generics_preserving_spelling(
            bound,
            &no_local,
            100,
            &HashMap::new(),
            &mut |_| 101,
            generic_params,
        );
        assert!(
            result.is_err(),
            "nested non-trait bound must fail closed in the preserving encoder: {bound}"
        );
    }

    let result = parse_generic_bound_with_generics_preserving_spelling(
        "u8",
        &no_local,
        100,
        &HashMap::new(),
        &mut |_| 101,
        &[],
    );
    assert!(result.is_err(), "primitive bound must fail closed in the preserving encoder");

    for (bound, generic_params) in
        [("Outer<Item: u8>", &[] as &[&str]), ("Outer<Item: U>", &["U"] as &[&str])]
    {
        let result = parse_generic_bound_with_generics(
            bound,
            &no_local,
            100,
            &HashMap::new(),
            &mut |_| 101,
            generic_params,
        );
        assert!(result.is_ok(), "legacy bound parser must retain its permissive behavior: {bound}");
    }
}

#[test]
fn test_validate_generic_bound_rejects_turbofish_arguments() {
    for bound in ["Tr::<u8>", "Tr<Vec::<u8>>"] {
        assert!(
            validate_lexical_generic_bound(bound, &[]).is_err(),
            "turbofish spelling must be rejected: {bound}"
        );
    }
    assert!(validate_lexical_generic_bound("Tr<Vec<u8>>", &[]).is_ok());

    let result = parse_generic_bound_with_generics_preserving_spelling(
        "Tr::<u8>",
        &no_local,
        100,
        &HashMap::new(),
        &mut |_| 101,
        &[],
    );
    assert!(result.is_err(), "turbofish bound must fail closed in the preserving encoder");
}

#[test]
fn test_validate_generic_bound_rejects_parenthesized_bounds() {
    for bound in ["(Clone)", "( Clone )"] {
        assert!(
            validate_lexical_generic_bound(bound, &[]).is_err(),
            "parenthesized bound spelling must be rejected: {bound}"
        );
    }
    // Grammatically required parentheses are a type-level node, not a
    // parenthesized bound, and must stay accepted.
    assert!(validate_lexical_generic_bound("Tr<&(dyn Fn() + Send)>", &[]).is_ok());

    let result = parse_generic_bound_with_generics_preserving_spelling(
        "(Clone)",
        &no_local,
        100,
        &HashMap::new(),
        &mut |_| 101,
        &[],
    );
    assert!(result.is_err(), "parenthesized bound must fail closed in the preserving encoder");
}

#[test]
fn test_validate_generic_bound_rejects_redundant_parenthesized_types() {
    for bound in ["Outer<(u8)>", "Tr<&(dyn Fn())>", "Tr<(dyn Fn() + Send)>"] {
        assert!(
            validate_lexical_generic_bound(bound, &[]).is_err(),
            "redundant parenthesized type spelling must be rejected: {bound}"
        );
    }
    // Grammatically required parentheses — a multi-bound trait object directly
    // behind a reference or raw pointer — carry no spelling variance.
    for bound in ["Tr<&(dyn Fn() + Send)>", "Outer<*const (dyn Fn() + Send)>"] {
        assert!(
            validate_lexical_generic_bound(bound, &[]).is_ok(),
            "grammatically required parentheses must stay accepted: {bound}"
        );
    }
    assert!(validate_lexical_type_ref("Outer<(u8)>", &["T"]).is_err());
    assert!(validate_lexical_type_ref("&(dyn Fn() + Send)", &["T"]).is_ok());

    let result = parse_generic_bound_with_generics_preserving_spelling(
        "Outer<(u8)>",
        &no_local,
        100,
        &HashMap::new(),
        &mut |_| 101,
        &[],
    );
    assert!(
        result.is_err(),
        "redundant parenthesized type must fail closed in the preserving encoder"
    );
}

#[test]
fn test_validate_generic_bound_rejects_precise_capture() {
    for bound in ["use<T>", "use<'a, T>"] {
        assert!(
            validate_lexical_generic_bound(bound, &["T"]).is_err(),
            "precise-capture bound must be rejected: {bound}"
        );
    }

    for bound in ["use<T>", "use<'a, T>"] {
        let result = parse_generic_bound_with_generics_preserving_spelling(
            bound,
            &no_local,
            100,
            &HashMap::new(),
            &mut |_| 101,
            &["T"],
        );
        assert!(
            result.is_err(),
            "precise-capture bound must fail closed in the preserving encoder: {bound}"
        );
    }
}

#[test]
fn test_validate_generic_bound_rejects_non_final_dyn_lifetimes() {
    for bound in ["Outer<dyn 'static + Tr>", "Outer<Box<dyn 'static + Tr>>"] {
        assert!(
            validate_lexical_generic_bound(bound, &[]).is_err(),
            "a dyn lifetime written before a trait bound must be rejected: {bound}"
        );
    }
    // The representable spelling — lifetime after the trait bounds — stays accepted.
    for bound in ["Outer<dyn Tr + 'static>", "Outer<dyn Tr>"] {
        assert!(
            validate_lexical_generic_bound(bound, &[]).is_ok(),
            "the canonical dyn spelling must stay accepted: {bound}"
        );
    }
    assert!(validate_lexical_type_ref("Outer<dyn 'static + Tr>", &["T"]).is_err());
    assert!(validate_lexical_type_ref("Outer<dyn Tr + 'static>", &["T"]).is_ok());

    let result = parse_generic_bound_with_generics_preserving_spelling(
        "Outer<dyn 'static + Tr>",
        &no_local,
        100,
        &HashMap::new(),
        &mut |_| 101,
        &[],
    );
    assert!(result.is_err(), "a non-final dyn lifetime must fail closed in the preserving encoder");
}

#[test]
fn test_validate_generic_bound_rejects_attributed_bare_fn_args() {
    for bound in ["Outer<fn(#[cfg(any())] u8)>", "Fn(fn(#[cfg(any())] u8))"] {
        assert!(
            validate_lexical_generic_bound(bound, &[]).is_err(),
            "attribute-bearing bare-fn arguments must be rejected: {bound}"
        );
    }
    assert!(validate_lexical_generic_bound("Outer<fn(u8)>", &[]).is_ok());
    assert!(validate_lexical_type_ref("Outer<fn(#[cfg(any())] u8)>", &["T"]).is_err());
    assert!(validate_lexical_type_ref("Outer<fn(u8)>", &["T"]).is_ok());

    let result = parse_generic_bound_with_generics_preserving_spelling(
        "Outer<fn(#[cfg(any())] u8)>",
        &no_local,
        100,
        &HashMap::new(),
        &mut |_| 101,
        &[],
    );
    assert!(
        result.is_err(),
        "attribute-bearing bare-fn arguments must fail closed in the preserving encoder"
    );
}

#[test]
fn test_validate_generic_bound_rejects_redundant_trailing_commas() {
    let redundant_spellings = [
        "Tr<u8,>",
        "Fn(u8,)",
        "Outer<fn(u8,)>",
        "Outer<(u8, u16,)>",
        "for<'a,> Fn(&'a u8)",
        "Outer<for<'a,> fn(&'a u8)>",
        "Outer<fn(u8, ...,)>",
    ];
    for spelling in redundant_spellings {
        assert!(
            validate_lexical_generic_bound(spelling, &[]).is_err(),
            "a redundant trailing comma must be rejected in a bound: {spelling}"
        );
        assert!(
            validate_lexical_type_ref(spelling, &["T"]).is_err(),
            "a redundant trailing comma must be rejected in a type: {spelling}"
        );
        assert!(
            parse_generic_bound_with_generics_preserving_spelling(
                spelling,
                &no_local,
                100,
                &HashMap::new(),
                &mut |_| 101,
                &[],
            )
            .is_err(),
            "a redundant trailing comma must fail closed in the preserving bound encoder: {spelling}"
        );
        assert!(
            parse_type_ref_with_generics_preserving_spelling(
                spelling,
                &no_local,
                100,
                &HashMap::new(),
                &mut |_| 101,
                &["T"],
            )
            .is_err(),
            "a redundant trailing comma must fail closed in the preserving type encoder: {spelling}"
        );
    }

    // Semantic trailing commas — a one-element tuple and a variadic marker — stay accepted.
    // The variadic comma stays a semantic (accepted) spelling, but only with a
    // compatible calling convention (E0045): the plain `fn(u8, ...)` form is
    // rejected by the closed grammar.
    for spelling in ["Tr<u8>", "Outer<(u8,)>", "Outer<extern \"C\" fn(u8, ...)>"] {
        assert!(
            validate_lexical_generic_bound(spelling, &[]).is_ok(),
            "semantic or canonical bound spellings must stay accepted: {spelling}"
        );
        assert!(
            validate_lexical_type_ref(spelling, &["T"]).is_ok(),
            "semantic or canonical type spellings must stay accepted: {spelling}"
        );
        assert!(
            parse_generic_bound_with_generics_preserving_spelling(
                spelling,
                &no_local,
                100,
                &HashMap::new(),
                &mut |_| 101,
                &[],
            )
            .is_ok(),
            "semantic or canonical bound spelling must stay encodable: {spelling}"
        );
        assert!(
            parse_type_ref_with_generics_preserving_spelling(
                spelling,
                &no_local,
                100,
                &HashMap::new(),
                &mut |_| 101,
                &["T"],
            )
            .is_ok(),
            "semantic or canonical type spelling must stay encodable: {spelling}"
        );
    }
}

#[test]
fn test_preserving_type_parser_rejects_precise_capture_trailing_comma() {
    let spelling = "impl Clone + use<T,>";
    assert!(
        validate_lexical_type_ref(spelling, &["T"]).is_err(),
        "a precise-capture trailing comma must be rejected in a type"
    );
    assert!(
        parse_type_ref_with_generics_preserving_spelling(
            spelling,
            &no_local,
            100,
            &HashMap::new(),
            &mut |_| 101,
            &["T"],
        )
        .is_err(),
        "a precise-capture trailing comma must fail closed in the preserving type encoder"
    );
}

#[test]
fn test_validate_generic_bound_rejects_undeclared_lifetime_bounds() {
    for bound in ["'a", "'_"] {
        assert!(
            validate_lexical_generic_bound(bound, &["T"]).is_err(),
            "an undeclared lifetime bound must be rejected: {bound}"
        );
    }
    assert!(
        validate_lexical_generic_bound("'static", &["T"]).is_ok(),
        "`'static` is always in scope and must stay accepted"
    );

    for bound in ["'a", "'_"] {
        assert!(
            parse_generic_bound_with_generics_preserving_spelling(
                bound,
                &no_local,
                100,
                &HashMap::new(),
                &mut |_| 101,
                &["T"],
            )
            .is_err(),
            "an undeclared lifetime bound must fail closed in the preserving encoder: {bound}"
        );
    }
    assert!(
        parse_generic_bound_with_generics_preserving_spelling(
            "'static",
            &no_local,
            100,
            &HashMap::new(),
            &mut |_| 101,
            &["T"],
        )
        .is_ok(),
        "`'static` must stay accepted in the preserving encoder"
    );
}

#[test]
fn test_validate_generic_bound_rejects_infer_placeholders() {
    for bound in ["Outer<_>", "Fn(_)", "Outer<Vec<_>>"] {
        assert!(
            validate_lexical_generic_bound(bound, &[]).is_err(),
            "an infer placeholder must be rejected: {bound}"
        );
        assert!(
            parse_generic_bound_with_generics_preserving_spelling(
                bound,
                &no_local,
                100,
                &HashMap::new(),
                &mut |_| 101,
                &[],
            )
            .is_err(),
            "an infer placeholder must fail closed in the preserving bound parser: {bound}"
        );
    }
    assert!(validate_lexical_generic_bound("Outer<u8>", &[]).is_ok());
    assert!(validate_lexical_type_ref("Outer<_>", &["T"]).is_err());

    let result = parse_generic_bound_with_generics_preserving_spelling(
        "Outer<_>",
        &no_local,
        100,
        &HashMap::new(),
        &mut |_| 101,
        &[],
    );
    assert!(result.is_err(), "an infer placeholder must fail closed in the preserving encoder");
}

#[test]
fn test_validate_generic_bound_rejects_attributed_binder_params() {
    for bound in
        ["for<#[allow(unused)] 'a> Tr<&'a u8>", "Outer<for<#[allow(unused)] 'a> fn(&'a u8)>"]
    {
        assert!(
            validate_lexical_generic_bound(bound, &[]).is_err(),
            "an attributed binder parameter must be rejected: {bound}"
        );
        assert!(
            parse_generic_bound_with_generics_preserving_spelling(
                bound,
                &no_local,
                100,
                &HashMap::new(),
                &mut |_| 101,
                &[],
            )
            .is_err(),
            "an attributed binder parameter must fail closed in the preserving bound parser: {bound}"
        );
    }
    for bound in ["for<'a> Tr<&'a u8>", "Outer<for<'a> fn(&'a u8)>"] {
        assert!(
            validate_lexical_generic_bound(bound, &[]).is_ok(),
            "plain binder spellings must stay accepted: {bound}"
        );
    }
    assert!(
        validate_lexical_type_ref("Outer<for<#[allow(unused)] 'a> fn(&'a u8)>", &["T"]).is_err()
    );

    let result = parse_generic_bound_with_generics_preserving_spelling(
        "for<#[allow(unused)] 'a> Tr<&'a u8>",
        &no_local,
        100,
        &HashMap::new(),
        &mut |_| 101,
        &[],
    );
    assert!(
        result.is_err(),
        "an attributed binder parameter must fail closed in the preserving encoder"
    );
}

#[test]
fn test_parse_type_ref_preserving_spelling_rejects_infer_and_attributed_binder_params() {
    for source in ["Outer<_>", "Outer<Vec<_>>"] {
        assert!(
            validate_lexical_type_ref(source, &[]).is_err(),
            "an infer placeholder must be rejected in a type: {source}"
        );
        assert!(
            parse_type_ref_with_generics_preserving_spelling(
                source,
                &no_local,
                100,
                &HashMap::new(),
                &mut |_| 101,
                &[],
            )
            .is_err(),
            "an infer placeholder must fail closed in the preserving type parser: {source}"
        );
        assert!(
            parse_type_ref_with_generics(
                source,
                &no_local,
                100,
                &HashMap::new(),
                &mut |_| 101,
                &[],
            )
            .is_ok(),
            "the legacy type parser must retain its permissive behavior: {source}"
        );
        assert!(
            parse_generic_bound_with_generics(
                source,
                &no_local,
                100,
                &HashMap::new(),
                &mut |_| 101,
                &[],
            )
            .is_ok(),
            "the legacy bound parser must retain its permissive behavior: {source}"
        );
    }

    let attributed_type = "Outer<for<#[allow(unused)] 'a> fn(&'a u8)>";
    assert!(validate_lexical_type_ref(attributed_type, &[]).is_err());
    assert!(
        parse_type_ref_with_generics_preserving_spelling(
            attributed_type,
            &no_local,
            100,
            &HashMap::new(),
            &mut |_| 101,
            &[],
        )
        .is_err(),
        "an attributed binder parameter must fail closed in the preserving type parser"
    );
    assert!(
        parse_type_ref_with_generics(
            attributed_type,
            &no_local,
            100,
            &HashMap::new(),
            &mut |_| 101,
            &[],
        )
        .is_ok(),
        "the legacy type parser must retain its permissive behavior"
    );
    assert!(
        parse_generic_bound_with_generics(
            "for<#[allow(unused)] 'a> Tr<&'a u8>",
            &no_local,
            100,
            &HashMap::new(),
            &mut |_| 101,
            &[],
        )
        .is_ok(),
        "the legacy bound parser must retain its permissive behavior"
    );
    assert!(
        parse_type_ref_with_generics_preserving_spelling(
            "Outer<for<'a> fn(&'a u8)>",
            &no_local,
            100,
            &HashMap::new(),
            &mut |_| 101,
            &[],
        )
        .is_ok(),
        "plain binder spelling must stay accepted in the preserving type parser"
    );
}

fn assert_alias_lexical_spelling_rejected_at_all_gates(spelling: &str) {
    assert!(
        validate_lexical_generic_bound(spelling, &[]).is_err(),
        "the lexical generic-bound validator must reject: {spelling}"
    );
    assert!(
        validate_lexical_type_ref(spelling, &["T"]).is_err(),
        "the lexical type validator must reject: {spelling}"
    );
    assert!(
        parse_generic_bound_with_generics_preserving_spelling(
            spelling,
            &no_local,
            100,
            &HashMap::new(),
            &mut |_| 101,
            &[],
        )
        .is_err(),
        "the preserving generic-bound encoder must reject: {spelling}"
    );
    assert!(
        parse_type_ref_with_generics_preserving_spelling(
            spelling,
            &no_local,
            100,
            &HashMap::new(),
            &mut |_| 101,
            &["T"],
        )
        .is_err(),
        "the preserving type encoder must reject: {spelling}"
    );
}

fn assert_alias_lexical_spelling_accepted_at_all_gates(spelling: &str) {
    assert!(
        validate_lexical_generic_bound(spelling, &[]).is_ok(),
        "the lexical generic-bound validator must accept: {spelling}"
    );
    assert!(
        validate_lexical_type_ref(spelling, &["T"]).is_ok(),
        "the lexical type validator must accept: {spelling}"
    );
    assert!(
        parse_generic_bound_with_generics_preserving_spelling(
            spelling,
            &no_local,
            100,
            &HashMap::new(),
            &mut |_| 101,
            &[],
        )
        .is_ok(),
        "the preserving generic-bound encoder must accept: {spelling}"
    );
    assert!(
        parse_type_ref_with_generics_preserving_spelling(
            spelling,
            &no_local,
            100,
            &HashMap::new(),
            &mut |_| 101,
            &["T"],
        )
        .is_ok(),
        "the preserving type encoder must accept: {spelling}"
    );
}

#[test]
fn test_alias_lexical_gates_reject_original_lossy_spellings() {
    for spelling in [
        "Outer<dyn Send +>",
        "Outer<fn(_: u8)>",
        "Outer<extern fn()>",
        "Outer<impl Clone>",
        "Outer<Self>",
        "Outer<Self::Assoc>",
    ] {
        assert_alias_lexical_spelling_rejected_at_all_gates(spelling);
    }

    for spelling in [
        "Outer<dyn Send>",
        "Outer<fn(u8)>",
        "Outer<fn(x: u8)>",
        "Outer<extern \"C\" fn()>",
        "Outer<u8>",
    ] {
        assert_alias_lexical_spelling_accepted_at_all_gates(spelling);
    }
}

#[test]
fn test_alias_lexical_gates_reject_reserved_bare_fn_parameter_names() {
    // Rustc allows a `self` parameter only in associated functions, strict and
    // reserved keyword names (including Rust 2024's `gen`, absent from syn's
    // edition-unaware table) are not declarable as bare-function parameters,
    // and raw spellings never appear in rustdoc output.
    for spelling in [
        "Outer<fn(self: u8)>",
        "Outer<fn(Self: u8)>",
        "Outer<fn(r#type: u8)>",
        "Outer<fn(gen: u8)>",
    ] {
        assert_alias_lexical_spelling_rejected_at_all_gates(spelling);
    }
    // Contextual keywords remain valid bare-function parameter names.
    for spelling in [
        "Outer<fn(value: u8)>",
        "Outer<fn(\u{5024}: u8)>",
        "Outer<fn(raw: u8)>",
        "Outer<fn(safe: u8)>",
        "Outer<fn(union: u8)>",
        "Outer<fn(macro_rules: u8)>",
    ] {
        assert_alias_lexical_spelling_accepted_at_all_gates(spelling);
    }
}

#[test]
fn test_alias_lexical_gates_reject_known_non_trait_types_as_bounds() {
    // Rustc rejects a bound naming a known standard-library struct / enum
    // (E0404, "expected trait, found struct"): only the exact std / core
    // canonical paths resolve DEFINITIVELY, so only those reject.
    assert_bound_spelling_rejected("std::vec::Vec<u8>");
    assert_bound_spelling_rejected("std::string::String");
    assert_bound_spelling_rejected("core::vec::Vec<u8>");
    assert_bound_spelling_rejected("core::option::Option<u8>");
    // Nested trait positions (trait objects) apply the same rule.
    assert_alias_lexical_spelling_rejected_at_all_gates("Outer<dyn std::vec::Vec<u8>>");
    assert_alias_lexical_spelling_rejected_at_all_gates("Outer<dyn core::vec::Vec<u8>>");
    // A bare short name stays open-world: the trait-path resolver checks
    // local catalogue items first, so `Vec` may name a local trait.
    assert_bound_spelling_accepted("Vec<u8>");
    // Known traits keep both spellings.
    assert_bound_spelling_accepted("Clone");
    assert_bound_spelling_accepted("std::clone::Clone");
}

#[test]
fn test_alias_lexical_gates_reject_type_params_used_as_const_values() {
    // The schema has no const-parameter declaration, so a const expression
    // rooted at a declared type parameter is rustc's type-used-as-value
    // error (E0423); free-standing const names stay representable.
    assert!(validate_lexical_type_ref("Marker<{ N }>", &["N"]).is_err());
    assert!(validate_lexical_type_ref("[u8; N]", &["N"]).is_err());
    assert!(validate_lexical_type_ref("Marker<{ LEN }>", &["T"]).is_ok());
    // Array lengths keep their existing literal-only allowlist.
    assert!(validate_lexical_type_ref("[u8; 4]", &["T"]).is_ok());
}

#[test]
fn test_alias_lexical_gates_reject_type_params_used_as_associated_const_values() {
    let spelling = "Outer<FLAG = { N }>";
    assert!(validate_lexical_generic_bound(spelling, &["N"]).is_err());
    assert!(validate_lexical_type_ref(spelling, &["N"]).is_err());
    assert!(
        parse_generic_bound_with_generics_preserving_spelling(
            spelling,
            &no_local,
            100,
            &HashMap::new(),
            &mut |_| 101,
            &["N"],
        )
        .is_err()
    );
    assert!(
        parse_type_ref_with_generics_preserving_spelling(
            spelling,
            &no_local,
            100,
            &HashMap::new(),
            &mut |_| 101,
            &["N"],
        )
        .is_err()
    );
}

#[test]
fn test_alias_lexical_gates_reject_nested_relaxed_bounds() {
    // Rustc permits a relaxed bound only directly on a type parameter of
    // the closest item: nested positions (trait objects, associated-item
    // constraints) are rejected.
    assert_alias_lexical_spelling_rejected_at_all_gates("Outer<dyn Tr + ?Sized>");
    assert_bound_spelling_rejected("Outer<Item: ?Sized>");
    assert_bound_spelling_accepted("?Sized");
}

#[test]
fn test_alias_lexical_gates_reject_unstable_const_literal_kinds() {
    // A byte literal has type `u8` and is therefore a stable const argument.
    // Strings and byte strings require unstable const-parameter types, so no
    // compiler-validated rustdoc output can carry them.
    assert_alias_lexical_spelling_rejected_at_all_gates("Outer<\"x\">");
    assert_alias_lexical_spelling_rejected_at_all_gates("Outer<b\"x\">");
    assert_alias_lexical_spelling_accepted_at_all_gates("Outer<b'x'>");
    assert_alias_lexical_spelling_accepted_at_all_gates("Marker<5>");
    assert_alias_lexical_spelling_accepted_at_all_gates("Marker<'\\x78'>");
    // `()` in argument position is the unit TYPE argument (`T: Into<()>` is
    // stable), not a const literal, and stays accepted.
    assert_alias_lexical_spelling_accepted_at_all_gates("Outer<()>");
}

#[test]
fn test_alias_lexical_gates_reject_non_nfc_identifier_spellings() {
    // Rustc normalizes identifiers to NFC before they reach rustdoc, so a
    // decomposed Unicode spelling in the catalogue can never match the
    // observed representation; the precomposed (NFC) spelling stays valid.
    assert_alias_lexical_spelling_rejected_at_all_gates("Outer<fn(e\u{0301}x: u8)>");
    assert_alias_lexical_spelling_accepted_at_all_gates("Outer<fn(\u{00e9}x: u8)>");
    // The same trap applies to lifetime names in alias targets.
    assert!(validate_lexical_alias_target("&'e\u{0301}x T", &["T"]).is_err());
    assert!(validate_lexical_alias_target("&'\u{00e9}x T", &["T"]).is_ok());
}

#[test]
fn test_alias_lexical_gates_reject_self_in_trait_bound_paths() {
    for spelling in ["Self", "Outer<Item: Self>"] {
        assert_bound_spelling_rejected(spelling);
    }
}

#[test]
fn test_alias_lexical_gates_reject_raw_identifiers_in_const_expressions() {
    assert_alias_lexical_spelling_rejected_at_all_gates("Marker<{ r#N }>");
}

#[test]
fn test_alias_lexical_gates_reject_additional_lossy_spellings() {
    for spelling in [
        "Outer<Item: Send +>",
        "Outer<for<'a: 'static +> fn(&'a ())>",
        "Outer<for<'a:> fn(&'a ())>",
        // rustc rejects lifetime bounds inside an HRTB binder ("bounds cannot
        // be used in this context"), so the spelling cannot round-trip through
        // compiler-validated rustdoc output.
        "Outer<for<'a: 'static> fn(&'a ())>",
        "Outer<extern \"Rust\" fn()>",
        r##"Outer<extern r#"C"# fn()>"##,
        "Outer<extern \"\\x43\" fn()>",
        "Outer<unsafe extern \"C\" fn(u8, args: ...)>",
        // rustc rejects unrecognized ABI names outright (E0703)
        "Outer<extern \"garbage\" fn()>",
        "Outer<extern \"garbage\" fn(u8, ...)>",
    ] {
        assert_alias_lexical_spelling_rejected_at_all_gates(spelling);
    }

    for spelling in [
        "Outer<Item: Send>",
        "Outer<for<'a> fn(&'a ())>",
        "Outer<fn()>",
        "Outer<extern \"C\" fn()>",
        "Outer<extern \"efiapi\" fn()>",
        "Outer<unsafe extern \"C\" fn(u8, ...)>",
        // C23 variadics (Rust 1.80) and the supported variadic ABIs,
        // including the Rust 1.93 `system` family.
        "Outer<extern \"C\" fn(...)>",
        "Outer<extern \"sysv64\" fn(u8, ...)>",
        "Outer<extern \"sysv64-unwind\" fn(u8, ...)>",
        "Outer<extern \"win64-unwind\" fn(u8, ...)>",
        "Outer<extern \"aapcs-unwind\" fn(u8, ...)>",
        "Outer<extern \"system\" fn(u8, ...)>",
        "Outer<extern \"system-unwind\" fn(u8, ...)>",
    ] {
        assert_alias_lexical_spelling_accepted_at_all_gates(spelling);
    }
}

#[test]
fn test_alias_lexical_gates_reject_empty_generic_argument_lists() {
    // Rustdoc emits `args: null` for both `Tr<>` and `Tr`, so the explicit
    // empty-list spelling must fail closed.
    for spelling in ["Tr<>", "Outer<Inner<>>"] {
        assert_alias_lexical_spelling_rejected_at_all_gates(spelling);
    }
    assert_alias_lexical_spelling_accepted_at_all_gates("Outer<Inner>");
}

#[test]
fn test_alias_target_grammar_accepts_source_declarable_named_lifetimes_only() {
    // The schema cannot declare lifetime parameters, so alias TARGETS carry
    // them lexically (established modeling convention): free named lifetimes
    // are in scope for the target grammar only.
    assert!(
        validate_lexical_alias_target(
            "std::pin::Pin<std::boxed::Box<dyn core::future::Future<Output = Result<V, E>> + Send + 'a>>",
            &["V", "E"],
        )
        .is_ok(),
        "a free named lifetime must stay valid in a generic alias target"
    );
    assert!(
        validate_lexical_alias_target("&'α T", &["T"]).is_ok(),
        "Unicode lifetime names valid in source declarations must stay valid in generic alias targets"
    );
    assert!(
        validate_lexical_alias_target("&'async T", &["T"]).is_ok(),
        "rustdoc-normalized keyword lifetime names must stay valid in generic alias targets"
    );
    // Raw spellings never appear in rustdoc output (`'r#async` normalizes to
    // `'async`), so retaining them would guarantee a chain mismatch: the
    // normalized spelling is the one accepted representation.
    assert!(
        validate_lexical_alias_target("&'r#async T", &["T"]).is_err(),
        "a raw lifetime spelling must be rejected in favor of its normalized form"
    );
    // Anonymous and reserved lifetime names are not valid in a type-alias
    // signature, while bound / subject positions keep the scoped rule.
    assert!(validate_lexical_alias_target("&'_ u8", &["T"]).is_err());
    assert!(validate_lexical_alias_target("&'self u8", &["T"]).is_err());
    assert!(validate_lexical_alias_target("&'r#self u8", &["T"]).is_err());
    assert!(validate_lexical_alias_target("&'r#static u8", &["T"]).is_err());
    assert!(validate_lexical_type_ref("&'a u8", &["T"]).is_err());
}

#[test]
fn test_lexical_gates_reject_reserved_lifetime_binder_declarations() {
    // Reserved binder names must fail at declaration time.  Checking only
    // lifetime uses would let an unused `for<'self>` through, and ScopedOnly
    // would otherwise consider a use in that binder in scope.
    assert!(validate_lexical_alias_target("for<'self> fn(T)", &["T"]).is_err());
    assert!(validate_lexical_generic_bound("for<'self> Fn(&'self T)", &["T"]).is_err());
    assert!(validate_lexical_type_ref("for<'self> fn(&'self T)", &["T"]).is_err());

    for reserved in
        ["self", "Self", "super", "crate", "r#self", "r#Self", "r#super", "r#crate", "r#static"]
    {
        let target = format!("for<'{reserved}> fn(T)");
        assert!(
            validate_lexical_alias_target(&target, &["T"]).is_err(),
            "a reserved lifetime binder must be rejected in a target: {target}"
        );
    }
}

#[test]
fn test_closed_grammar_rejects_raw_lifetime_spellings_in_hrtb_scopes() {
    // rustdoc 1.95 normalizes `'r#async` to `'async` in both the binder and
    // its uses, so a raw catalogue spelling can never match the observed
    // representation: the lexical boundary rejects it toward the normalized
    // form instead of retaining bytes that guarantee a chain mismatch.
    assert!(
        validate_lexical_generic_bound("for<'r#async> Tr<&'r#async u8>", &["T"]).is_err(),
        "a raw binder lifetime and its raw uses must be rejected"
    );
    assert!(
        validate_lexical_generic_bound("for<'r#a> Fn(&'a ())", &["T"]).is_err(),
        "a raw binder declaration must be rejected even when its uses are normalized"
    );
    assert!(
        validate_lexical_generic_bound("for<'a> Fn(&'r#a ())", &["T"]).is_err(),
        "a raw lifetime use must be rejected even under a normalized binder"
    );
    assert!(
        validate_lexical_generic_bound("for<'async> Tr<&'async u8>", &["T"]).is_ok(),
        "the rustdoc-normalized keyword spelling must stay accepted"
    );
}

#[test]
fn test_alias_lexical_gates_reject_multiple_dyn_lifetimes() {
    // Rust permits only one explicit trait-object lifetime bound (E0226), and
    // the converter keeps only the first, so a second lifetime must fail closed.
    assert_alias_lexical_spelling_rejected_at_all_gates("Outer<dyn Tr + 'static + 'static>");
    assert_alias_lexical_spelling_accepted_at_all_gates("Outer<dyn Tr + 'static>");
}

/// Bound-position-only corpus assertions: these spellings are not standalone
/// `syn::Type` syntax, so only the two bound gates apply.
fn assert_bound_spelling_rejected(spelling: &str) {
    assert!(
        validate_lexical_generic_bound(spelling, &[]).is_err(),
        "the lexical generic-bound validator must reject: {spelling}"
    );
    assert!(
        parse_generic_bound_with_generics_preserving_spelling(
            spelling,
            &no_local,
            100,
            &HashMap::new(),
            &mut |_| 101,
            &[],
        )
        .is_err(),
        "the preserving generic-bound encoder must reject: {spelling}"
    );
}

fn assert_bound_spelling_accepted(spelling: &str) {
    assert!(
        validate_lexical_generic_bound(spelling, &[]).is_ok(),
        "the lexical generic-bound validator must accept: {spelling}"
    );
    assert!(
        parse_generic_bound_with_generics_preserving_spelling(
            spelling,
            &no_local,
            100,
            &HashMap::new(),
            &mut |_| 101,
            &[],
        )
        .is_ok(),
        "the preserving generic-bound encoder must accept: {spelling}"
    );
}

/// Regression corpus for the 47 inline findings of PR #234 (2026-08-02 …
/// 2026-08-05). The review spiral ended by closing the acceptance grammar
/// (`closed_grammar`: canonical round-trip + syntax allowlist) instead of
/// growing a per-finding denylist; this corpus fixes every grammar-relevant
/// finding's input to its outcome under the two rules. Findings about
/// parameter names, the domain linter, and the contract-map renderer are
/// covered by their own dedicated tests and listed here only for
/// completeness of the mapping.
#[test]
fn test_pr234_findings_regression_corpus_rejections() {
    for spelling in [
        // syntactically invalid bound (finding 1: `<T>`)
        "<T>",
        // explicit unit output (finding 10) — canonical form is `Fn()`
        "Fn() -> ()",
        // evaluated array length (finding 11) — canonical form is `[u8; 3]`
        "Trait<[u8; 1 + 2]>",
        // complex anonymous const block (finding 12)
        "Trait<{ 1 + 2 }>",
        // const method call in array length (finding 14)
        "Trait<[u8; 10usize.pow(2)]>",
        // named constant array length (finding 15)
        "Trait<[u8; LEN]>",
        // type macro (finding 16)
        "Trait<ty!()>",
        // explicit "Rust" ABI (finding 21) — canonical form omits `extern`
        "Outer<extern \"Rust\" fn()>",
        // relaxed non-Sized bound (finding 24)
        "?Clone",
        // turbofish arguments (finding 29)
        "Tr::<u8>",
        // parenthesized bound (finding 30)
        "(Clone)",
        // nested redundant parentheses (finding 31)
        "Outer<(u8)>",
        // precise capture (finding 32)
        "use<T>",
        // non-final trait-object lifetime (finding 33)
        "Outer<dyn 'static + Tr>",
        // attributed bare-fn parameter (finding 34)
        "Outer<fn(#[cfg(any())] u8)>",
        // trailing comma (finding 35)
        "Tr<u8,>",
        // undeclared lifetime bound (finding 36)
        "'a",
        // infer placeholder (finding 37)
        "Outer<_>",
        // attributed HRTB binder parameter (finding 38)
        "for<#[allow(unused)] 'a> Tr<&'a u8>",
        // trailing plus in a trait object (finding 39)
        "Outer<dyn Send +>",
        // explicit wildcard parameter name (finding 40) — canonical `fn(u8)`
        "Outer<fn(_: u8)>",
        // implicit C ABI (finding 41) — canonical `extern \"C\" fn()`
        "Outer<extern fn()>",
        // impl Trait in generic arguments (finding 42)
        "Outer<impl Clone>",
        // Self in an alias declaration (finding 43)
        "Outer<Self>",
        // second trait-object lifetime (finding 44)
        "Outer<dyn Tr + 'static + 'static>",
        // explicitly empty argument list (finding 45)
        "Tr<>",
        // round-34 additions: reserved / duplicate binder lifetimes
        // (E0262 / E0403), binder lifetime bounds (invalid in HRTB context),
        // and C-variadics without a compatible ABI (E0045)
        "for<'static> Tr<&'static u8>",
        "for<'a, 'a> Tr<&'a u8>",
        "for<'a: 'static> Tr<&'a u8>",
        "Outer<fn(u8, ...)>",
        "Outer<extern \"Rust\" fn(u8, ...)>",
    ] {
        assert_alias_lexical_spelling_rejected_at_all_gates(spelling);
    }

    // Bound-position-only rejections (not standalone type syntax, or accepted
    // as a type):
    // placeholder reference lifetime (finding 27) — canonical `Fn(&str)`
    assert_bound_spelling_rejected("Fn(&'_ str)");
    // primitive as trait bound (finding 28) — `u8` stays a valid TYPE
    assert_bound_spelling_rejected("u8");
    assert!(validate_lexical_type_ref("u8", &["T"]).is_ok());
}

/// Accept side of the PR #234 corpus: the canonical counterparts and the
/// conversion-fidelity findings (whose inputs must round-trip and enter the
/// lexical comparison).
#[test]
fn test_pr234_findings_regression_corpus_acceptances() {
    // where-subject spelling (findings 20 / 22): `Vec<T>` stays `Vec<T>` in
    // TYPE positions; as a trait BOUND it is rustc E0404 (a later round) and
    // is therefore asserted for the type gate only.
    assert!(
        validate_lexical_type_ref("Vec<T>", &["T"]).is_ok(),
        "the lexical type validator must accept the where-subject spelling `Vec<T>`"
    );
    for spelling in [
        // prelude spelling preserved (finding 4)
        "Clone",
        // nested HRTB binder preserved (finding 6)
        "Into<Box<dyn for<'a> Fn(&'a str)>>",
        // unnamed function-pointer parameter (finding 7)
        "Trait<fn(u8)>",
        // generic argument shadowing (finding 8)
        "Into<T>",
        // associated-type-constraint binder (finding 9)
        "Outer<Item: for<'a> Tr<'a>>",
        // canonical counterparts of rejected spellings
        "Trait<[u8; 3]>",
        // simple anonymous const block (finding 12, supported form)
        "Trait<{ 1 }>",
        // absolute path spelling preserved (findings 13 / 17)
        "::std::clone::Clone",
        "Into<::std::vec::Vec<T>>",
        // signed and suffixed const literals (findings 18 / 19)
        "Outer<-3>",
        "Outer<3usize>",
        // quoted non-enumerated ABI (finding 23)
        "Outer<extern \"efiapi\" fn()>",
        // raw pointer const (finding 25)
        "Outer<*const u8>",
        // canonical bare-fn / ABI spellings (findings 40 / 41)
        "Outer<fn(u8)>",
        "Outer<extern \"C\" fn()>",
        // canonical trait-object spelling (findings 33 / 39 / 44)
        "Outer<dyn Tr + 'static>",
    ] {
        assert_alias_lexical_spelling_accepted_at_all_gates(spelling);
    }

    // Bound-position-only acceptances (not standalone type syntax):
    for spelling in [
        // canonical Fn-sugar spellings (findings 10 / 27)
        "Fn()", "Fn(&str)",
        // Sized relaxation stays expressible (finding 24, supported form)
        "?Sized", // static lifetime bound (finding 36, supported form)
        "'static",
    ] {
        assert_bound_spelling_accepted(spelling);
    }

    // `~const Clone` (finding 22's acceptance, superseded by a later PR
    // round): `~const` requires the unstable `const_trait_impl` feature and
    // is not permitted on type aliases, so the lexical validator now rejects
    // it — no stable compiler-validated rustdoc output can carry it.
    assert!(validate_lexical_generic_bound("~const Clone", &[]).is_err());
}

#[test]
fn test_legacy_parsers_accept_lexically_lossy_alias_spellings() {
    for spelling in [
        "Outer<Item: Send +>",
        "Outer<for<'a: 'static +> fn(&'a ())>",
        "Outer<extern \"Rust\" fn()>",
        "Outer<unsafe extern \"C\" fn(u8, args: ...)>",
    ] {
        assert!(
            parse_generic_bound_with_generics(
                spelling,
                &no_local,
                100,
                &HashMap::new(),
                &mut |_| 101,
                &[],
            )
            .is_ok(),
            "the legacy generic-bound parser must retain permissive behavior: {spelling}"
        );
        assert!(
            parse_type_ref_with_generics(
                spelling,
                &no_local,
                100,
                &HashMap::new(),
                &mut |_| 101,
                &["T"],
            )
            .is_ok(),
            "the legacy type parser must retain permissive behavior: {spelling}"
        );
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
    // The representation cannot distinguish `Fn() -> ()` from `Fn()` (rustdoc
    // emits an absent output for both), so the preserving gate rejects the
    // non-canonical explicit-unit spelling under the closed grammar.
    assert!(
        parse_generic_bound_with_generics_preserving_spelling(
            "Fn() -> ()",
            &no_local,
            100,
            &HashMap::new(),
            &mut |_| 101,
            &[],
        )
        .is_err(),
        "explicit unit output is a non-canonical spelling and must fail closed"
    );

    // The legacy permissive parser keeps the normalization claim: an explicit
    // unit output converts to rustdoc's absent output.
    let GenericBound::TraitBound { trait_, .. } = parse_generic_bound_with_generics(
        "Fn() -> ()",
        &no_local,
        100,
        &HashMap::new(),
        &mut |_| 101,
        &[],
    )
    .unwrap() else {
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
fn test_parse_type_ref_accepts_nonlexical_array_length_expressions() {
    for source in ["[u8; 10usize.pow(2)]", "[u8; -1]", "[u8; 1 as usize]"] {
        assert!(
            parse_type_ref(source, &no_local, 100, &HashMap::new(), &mut |_| 101).is_ok(),
            "nonlexical array length should remain encodable: {source}"
        );
    }
}

#[test]
fn test_parse_type_ref_preserving_spelling_rejects_nonlexical_array_length_expression() {
    let result = parse_type_ref_with_generics_preserving_spelling(
        "[u8; 1 as usize]",
        &no_local,
        100,
        &HashMap::new(),
        &mut |_| 101,
        &[],
    );
    let error = match result {
        Err(error) => error,
        Ok(value) => panic!("lexical array length unexpectedly parsed: {value:?}"),
    };
    assert!(error.contains("array length expressions"), "error: {error}");
}

#[test]
fn test_parse_type_ref_preserving_spelling_rejects_named_array_length() {
    let result = parse_type_ref_with_generics_preserving_spelling(
        "[u8; LEN]",
        &no_local,
        100,
        &HashMap::new(),
        &mut |_| 101,
        &[],
    );
    let error = match result {
        Err(error) => error,
        Ok(value) => panic!("named array length unexpectedly parsed: {value:?}"),
    };
    assert!(error.contains("array length expressions"), "error: {error}");
}

#[test]
fn test_parse_generic_bound_preserves_leading_colon() {
    for (source, expected) in
        [("::std::clone::Clone", "::std::clone::Clone"), ("::Clone", "::Clone")]
    {
        let bound = match parse_generic_bound_with_generics_preserving_spelling(
            source,
            &no_local,
            100,
            &HashMap::new(),
            &mut |_| 101,
            &[],
        ) {
            Ok(bound) => bound,
            Err(error) => panic!("absolute bound path should parse: {error}"),
        };
        let GenericBound::TraitBound { trait_, .. } = bound else {
            panic!("expected trait bound");
        };
        assert_eq!(trait_.path, expected);
    }
}

#[test]
fn test_parse_generic_bound_preserves_nested_leading_colon() {
    let bound = parse_generic_bound_with_generics_preserving_spelling(
        "Into<::std::vec::Vec<T>>",
        &no_local,
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
        panic!("expected angle-bracketed bound arguments");
    };
    let Some(GenericArg::Type(Type::ResolvedPath(path))) = args.first() else {
        panic!("expected nested resolved path argument");
    };
    assert_eq!(path.path, "::std::vec::Vec");
}

#[test]
fn test_parse_generic_bound_rejects_unsupported_array_length_expression() {
    let result = parse_generic_bound_with_generics_preserving_spelling(
        "Trait<[u8; 10usize.pow(2)]>",
        &no_local,
        100,
        &HashMap::new(),
        &mut |_| 101,
        &[],
    );
    let error = match result {
        Err(error) => error,
        Ok(value) => panic!("method-call array length in bound unexpectedly parsed: {value:?}"),
    };
    assert!(error.contains("array length expressions"), "error: {error}");
}

#[test]
fn test_parse_generic_bound_rejects_named_array_length() {
    let result = parse_generic_bound_with_generics_preserving_spelling(
        "Trait<[u8; LEN]>",
        &no_local,
        100,
        &HashMap::new(),
        &mut |_| 101,
        &[],
    );
    let error = match result {
        Err(error) => error,
        Ok(value) => panic!("named array length in bound unexpectedly parsed: {value:?}"),
    };
    assert!(error.contains("array length expressions"), "error: {error}");
}

#[test]
fn test_parse_generic_bound_rejects_type_macro() {
    let result = parse_generic_bound_with_generics_preserving_spelling(
        "Trait<ty!()>",
        &no_local,
        100,
        &HashMap::new(),
        &mut |_| 101,
        &[],
    );
    let error = match result {
        Err(error) => error,
        Ok(value) => panic!("type macro in bound unexpectedly parsed: {value:?}"),
    };
    assert!(error.contains("type macros"), "error: {error}");
}

#[test]
fn test_parse_generic_bound_rejects_unsupported_const_expression() {
    let result = parse_generic_bound_with_generics_preserving_spelling(
        "Marker<1.0>",
        &no_local,
        100,
        &HashMap::new(),
        &mut |_| 101,
        &[],
    );
    let error = match result {
        Err(error) => error,
        Ok(value) => panic!("float const argument unexpectedly parsed: {value:?}"),
    };
    assert!(error.contains("const generic argument expressions"), "error: {error}");
}

#[test]
fn test_parse_type_ref_ordinary_preserves_complex_const_tokens() {
    let ty = parse("Trait<{ 1 + 2 }>");
    let Type::ResolvedPath(path) = ty else {
        panic!("expected Trait resolved path");
    };
    let Some(GenericArgs::AngleBracketed { args, .. }) = path.args.as_deref() else {
        panic!("expected angle-bracketed arguments");
    };
    let Some(GenericArg::Const(constant)) = args.first() else {
        panic!("expected const argument");
    };
    assert_eq!(constant.expr, "{ 1 + 2 }");
}

#[test]
fn test_parse_type_ref_preserving_spelling_rejects_complex_const_block() {
    let result = parse_type_ref_with_generics_preserving_spelling(
        "Trait<{ 1 + 2 }>",
        &no_local,
        100,
        &HashMap::new(),
        &mut |_| 101,
        &[],
    );
    let error = match result {
        Err(error) => error,
        Ok(value) => format!("unexpected success: {value:?}"),
    };
    assert!(error.contains("anonymous const/block expressions"), "error: {error}");
}

#[test]
fn test_parse_type_ref_braces_in_comments_are_accepted() {
    let ty = parse("Vec</* { comment-only braces } */ u8>");
    assert!(matches!(ty, Type::ResolvedPath(_)), "expected a parsed Vec type, got: {ty:?}");
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

#[test]
fn test_qualified_path_preserves_absolute_trait_prefix_in_lexical_mode() {
    let ty = parse_type_ref_with_generics_preserving_spelling(
        "<T as ::std::ops::Deref>::Target",
        &no_local,
        100,
        &HashMap::new(),
        &mut |_| 101,
        &["T"],
    )
    .unwrap();
    let Type::QualifiedPath { trait_: Some(trait_path), .. } = ty else {
        panic!("expected qualified path with a trait prefix");
    };
    assert_eq!(trait_path.path, "::std::ops::Deref");
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
        CatalogueDocument, CatalogueEntryKey, CrateName, ModulePath, ParamName, TypeRef,
    };
    use domain::tddd::{CatalogueToExtendedCratePort, LayerId};
    use rustdoc_types::{Crate, FORMAT_VERSION, ItemEnum, ItemKind, ItemSummary, Target};

    let mut doc = CatalogueDocument::new(
        domain::tddd::catalogue_v2::document::CatalogueSchemaVersion::new(2),
        CrateName::new("domain").unwrap(),
        LayerId::try_new("domain").unwrap(),
    );
    doc.insert_trait(
        CatalogueEntryKey::try_new("MyTrait".to_owned()).unwrap(),
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

    let mut paths = std::collections::HashMap::new();
    paths.insert(
        rustdoc_types::Id(1),
        ItemSummary {
            crate_id: 0,
            path: vec!["domain".to_owned(), "MyTrait".to_owned()],
            kind: ItemKind::Trait,
        },
    );
    let authoritative = Crate {
        root: rustdoc_types::Id(0),
        crate_version: None,
        includes_private: false,
        index: std::collections::HashMap::new(),
        paths,
        external_crates: std::collections::HashMap::new(),
        format_version: FORMAT_VERSION,
        target: Target { triple: String::new(), target_features: vec![] },
    };
    let encoded =
        CatalogueToExtendedCrateCodec::new().encode(doc, &authoritative, &authoritative).unwrap();
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
