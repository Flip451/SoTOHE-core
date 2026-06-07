//! Syntax-validation helpers for the catalogue document codec.
//!
//! Each function validates a string against a `syn` grammar rule and returns a
//! human-readable error on failure. Used at the decode boundary to surface
//! malformed inputs before they reach `CatalogueToExtendedCrateCodec`.

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
