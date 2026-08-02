//! Syntax-validation helpers for the catalogue document codec.
//!
//! Each function validates a string against a `syn` grammar rule and returns a
//! human-readable error on failure. Used at the decode boundary to surface
//! malformed inputs before they reach `CatalogueToExtendedCrateCodec`.

use domain::tddd::catalogue_v2::MethodGenericParam;

use super::CatalogueDocumentCodecError;

/// Validates that `bound_str` is syntactically well-formed as a Rust type param bound
/// using `syn::parse_str::<syn::TypeParamBound>`.
///
/// Using `TypeParamBound` (not `syn::Type`) accepts the relaxed bound `?Sized` which
/// `syn::Type` would reject. Valid inputs include `"Send"`, `"Into<String>"`, `"?Sized"`.
///
/// Used to validate `MethodGenericParam.bounds[]` and `TraitEntry.supertrait_bounds[]`
/// at the codec boundary so that malformed bound syntax (e.g. `"<T>"`, `"T U"`) is
/// rejected here rather than failing later inside `CatalogueToExtendedCrateCodec`.
/// `TypeRef::new` only rejects empty strings and does not validate syntax; this
/// function provides the stronger structural check.
///
/// # Errors
///
/// Returns an error string with the `syn` parse error message if `bound_str` is
/// not a valid Rust type param bound syntax.
pub(super) fn validate_bound_str(bound_str: &str) -> Result<(), String> {
    syn::parse_str::<syn::TypeParamBound>(bound_str)
        .map(|_| ())
        .map_err(|e| format!("invalid bound syntax '{}': {e}", bound_str))
}

/// Returns whether `name` is a valid Rust generic type-parameter identifier.
///
/// Generic declarations are decoded from rustdoc-normalized names, so raw-identifier spellings
/// are unavailable at this boundary. Reject strict and reserved keywords, the Rust 2024 `gen`
/// keyword, and the bare wildcard `_`, which is a pattern rather than a generic parameter name.
pub(super) fn is_valid_generic_param_name(name: &str) -> bool {
    name != "_"
        && name != "gen"
        && !matches!(
            name,
            "as" | "async"
                | "await"
                | "break"
                | "const"
                | "continue"
                | "crate"
                | "dyn"
                | "else"
                | "enum"
                | "extern"
                | "false"
                | "fn"
                | "for"
                | "if"
                | "impl"
                | "in"
                | "let"
                | "loop"
                | "match"
                | "mod"
                | "move"
                | "mut"
                | "pub"
                | "ref"
                | "return"
                | "self"
                | "Self"
                | "static"
                | "struct"
                | "super"
                | "trait"
                | "true"
                | "type"
                | "unsafe"
                | "use"
                | "where"
                | "while"
                | "abstract"
                | "become"
                | "box"
                | "do"
                | "final"
                | "macro"
                | "override"
                | "priv"
                | "typeof"
                | "unsized"
                | "virtual"
                | "yield"
                | "try"
        )
}

/// Validates that all type-alias generic parameter names are Rust identifiers.
///
/// Type aliases share the domain `MethodGenericParam` declaration with methods. The decoder
/// invokes this adapter-boundary validation after conversion so keyword spellings produce the
/// same `InvalidEntry` error shape as other malformed catalogue fields.
pub(super) fn validate_type_alias_generic_names(
    entry_name: &str,
    generics: &[MethodGenericParam],
) -> Result<(), CatalogueDocumentCodecError> {
    for generic in generics {
        if !is_valid_generic_param_name(generic.name.as_str()) {
            return Err(CatalogueDocumentCodecError::InvalidEntry {
                entry_name: entry_name.to_owned(),
                reason: format!(
                    "generic param name '{}' is not a valid Rust identifier \
                     (must match [a-zA-Z_][a-zA-Z0-9_]* and must not be a Rust keyword)",
                    generic.name.as_str()
                ),
            });
        }
    }
    Ok(())
}

/// Validates that `type_str` is syntactically well-formed as a Rust type expression
/// using `syn::parse_str::<syn::Type>`.
///
/// Used to validate `WherePredicateDecl.lhs` at the codec boundary so that malformed
/// type syntax (e.g. `"Vec<"`, `"T U"`, `"<invalid>"`) is rejected at decode time
/// rather than failing later inside `CatalogueToExtendedCrateCodec`.
/// `TypeRef::new` only rejects empty strings and does not validate syntax; this
/// function provides the stronger structural check for where-predicate LHS values.
///
/// Note: HRTB-prefixed LHS strings (e.g. `"for<'a> T"`) are accepted by `syn::Type`
/// because they parse as `syn::Type::TraitObject` or similar constructs.
///
/// # Errors
///
/// Returns an error string with the `syn` parse error message if `type_str` is not
/// a valid Rust type expression.
pub(super) fn validate_type_ref_str(type_str: &str) -> Result<(), String> {
    syn::parse_str::<syn::Type>(type_str)
        .map(|_| ())
        .map_err(|e| format!("invalid type syntax '{}': {e}", type_str))
}

/// Validates that `trait_ref_str` is a Rust path expression (i.e. parseable as
/// `syn::Path`), rejecting non-path types such as `&Foo`, `[u8]`, `(A, B)`.
///
/// A trait reference must be a bare path (optionally with generic args), never a
/// reference, slice, tuple, or pointer.  The downstream codec
/// (`resolve_trait_ref_for_top_level`) enforces the same invariant by matching only
/// on `Type::ResolvedPath`; rejecting non-path forms here surfaces the error at the
/// DTO decode boundary with a clearer message.
///
/// # Errors
///
/// Returns an error string with the `syn` parse error message if `trait_ref_str`
/// is not a valid `syn::Path` expression.
pub(super) fn validate_trait_ref_is_path(trait_ref_str: &str) -> Result<(), String> {
    // `syn::parse_str::<syn::Path>` accepts angle-bracket generic args (e.g. `A<B>`)
    // natively, so no pre-stripping of generic args is needed.
    syn::parse_str::<syn::Path>(trait_ref_str).map(|_| ()).map_err(|e| {
        format!(
            "trait_ref '{}' is not a valid path (must be a plain type path, not a reference, \
             slice, or other non-path type): {e}",
            trait_ref_str
        )
    })
}
