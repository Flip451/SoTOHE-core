//! Syntax-validation helpers for the catalogue document codec.
//!
//! Each function validates a string against a `syn` grammar rule and returns a
//! human-readable error on failure. Used at the decode boundary to surface
//! malformed inputs before they reach `CatalogueToExtendedCrateCodec`.

use domain::tddd::catalogue_v2::{BoundOp, MethodGenericParam, WherePredicateDecl};

use crate::tddd::type_ref_parser::{
    is_plain_generic_param_name, validate_legacy_type_ref, validate_lexical_alias_target,
    validate_lexical_generic_bound, validate_lexical_type_ref,
};

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
/// The generic context is NOT re-validated here: shared (non-alias) callers
/// keep the parent's tolerance of keyword parameter names (spec OUT-01), and
/// the alias lexical gates run their own context validation.
pub(super) fn validate_bound_str_with_generics(
    bound_str: &str,
    _generic_params: &[&str],
) -> Result<(), String> {
    // `~const` is not special-cased: the prefix fails the syn parse below,
    // restoring the parent's rejection (const trait bounds are unstable and
    // cannot appear in compiler-validated rustdoc output).
    syn::parse_str::<syn::TypeParamBound>(bound_str)
        .map(|_| ())
        .map_err(|e| format!("invalid bound syntax '{}': {e}", bound_str))
}

/// Returns whether `name` is a plain, non-keyword generic type-parameter
/// identifier. Raw-identifier spellings and keywords are rejected so the
/// lexical comparison boundary never needs Rust grammar classification.
pub(super) fn is_valid_generic_param_name(name: &str) -> bool {
    is_plain_generic_param_name(name)
}

/// Validates that all type-alias generic parameter names are Rust identifiers.
///
/// Type aliases share the domain `MethodGenericParam` declaration with methods. The decoder
/// invokes this adapter-boundary validation after conversion so raw, keyword,
/// and wildcard spellings produce the same `InvalidEntry` error shape as other
/// malformed fields.
/// Rejects more than one relaxed (`?`-prefixed) bound per alias parameter.
/// Rustc refuses `T: ?Sized + ?Sized` (E0203) whether the duplicates are
/// inline, in the where clause, or split across both, so the codec must not
/// admit an alias contract no implementation can compile. Non-alias entries
/// keep the legacy per-bound surface (spec OUT-01).
pub(super) fn validate_type_alias_relaxed_bounds(
    entry_name: &str,
    generics: &[MethodGenericParam],
    where_predicates: &[WherePredicateDecl],
) -> Result<(), CatalogueDocumentCodecError> {
    for generic in generics {
        let inline = generic
            .bounds
            .iter()
            .map(|bound| is_relaxed_bound(bound.as_str()))
            .try_fold(0usize, |count, relaxed| relaxed.map(|relaxed| count + usize::from(relaxed)))
            .map_err(|error| invalid_relaxed_bound_syntax(entry_name, error))?;
        let in_where = where_predicates
            .iter()
            .try_fold(0usize, |count, predicate| {
                if !matches!(predicate.operator, BoundOp::Bound) {
                    return Ok(count);
                }
                predicate_targets_generic_param(predicate.lhs.as_str(), generic.name.as_str())
                    .and_then(|matches| {
                        if !matches {
                            return Ok(count);
                        }
                        predicate.rhs.iter().try_fold(count, |count, bound| {
                            is_relaxed_bound(bound.as_str())
                                .map(|relaxed| count + usize::from(relaxed))
                        })
                    })
            })
            .map_err(|error| invalid_relaxed_bound_syntax(entry_name, error))?;
        if inline + in_where > 1 {
            return Err(CatalogueDocumentCodecError::InvalidEntry {
                entry_name: entry_name.to_owned(),
                reason: format!(
                    "type parameter `{}` has more than one relaxed bound (rustc E0203); \
                     duplicate `?Sized` bounds cannot compile",
                    generic.name.as_str()
                ),
            });
        }
    }
    Ok(())
}

fn invalid_relaxed_bound_syntax(entry_name: &str, error: String) -> CatalogueDocumentCodecError {
    CatalogueDocumentCodecError::InvalidEntry {
        entry_name: entry_name.to_owned(),
        reason: format!("invalid type alias relaxed bound: {error}"),
    }
}

/// Returns whether `bound` is a relaxed bound according to its parsed Rust
/// syntax, rather than its source bytes. The closed grammar deliberately
/// accepts token-equivalent comments and whitespace, neither of which may
/// change whether `?Sized` counts toward rustc E0203.
fn is_relaxed_bound(bound: &str) -> Result<bool, String> {
    let parsed = syn::parse_str::<syn::TypeParamBound>(bound)
        .map_err(|error| format!("invalid bound syntax '{bound}': {error}"))?;
    Ok(matches!(
        parsed,
        syn::TypeParamBound::Trait(syn::TraitBound {
            modifier: syn::TraitBoundModifier::Maybe(_),
            ..
        })
    ))
}

/// Returns whether `lhs` parses as precisely the declared generic parameter.
/// This semantic comparison accepts the same token-equivalent whitespace and
/// comments as the lexical grammar while excluding paths, projections, and
/// type arguments that do not name the parameter itself.
fn predicate_targets_generic_param(lhs: &str, generic_name: &str) -> Result<bool, String> {
    let parsed = syn::parse_str::<syn::Type>(lhs)
        .map_err(|error| format!("invalid where predicate lhs '{lhs}': {error}"))?;
    let syn::Type::Path(type_path) = parsed else {
        return Ok(false);
    };
    let path = type_path.path;
    let Some(segment) = path.segments.first() else {
        return Ok(false);
    };
    Ok(type_path.qself.is_none()
        && path.leading_colon.is_none()
        && path.segments.len() == 1
        && segment.ident == generic_name
        && matches!(segment.arguments, syn::PathArguments::None))
}

/// Rejects keyword / raw generic parameter names for a type alias BEFORE the
/// shared DTO conversion runs, so the alias-specific name error fires
/// regardless of the entry's bound contents. The shared conversion itself
/// stays keyword-tolerant for non-alias entries (spec OUT-01).
pub(super) fn validate_type_alias_generic_name_strs<'names>(
    entry_name: &str,
    names: impl Iterator<Item = &'names str>,
) -> Result<(), CatalogueDocumentCodecError> {
    for name in names {
        if !is_valid_generic_param_name(name) {
            return Err(CatalogueDocumentCodecError::InvalidEntry {
                entry_name: entry_name.to_owned(),
                reason: format!(
                    "generic param name '{name}' is not a plain non-keyword Rust identifier"
                ),
            });
        }
    }
    Ok(())
}

pub(super) fn validate_type_alias_generic_names(
    entry_name: &str,
    generics: &[MethodGenericParam],
) -> Result<(), CatalogueDocumentCodecError> {
    let generic_names = generics.iter().map(|generic| generic.name.as_str()).collect::<Vec<_>>();
    for generic in generics {
        if !is_valid_generic_param_name(generic.name.as_str()) {
            return Err(CatalogueDocumentCodecError::InvalidEntry {
                entry_name: entry_name.to_owned(),
                reason: format!(
                    "generic param name '{}' is not a plain non-keyword Rust identifier",
                    generic.name.as_str()
                ),
            });
        }
        for (idx, bound) in generic.bounds.iter().enumerate() {
            validate_lexical_generic_bound(bound.as_str(), &generic_names).map_err(|error| {
                CatalogueDocumentCodecError::InvalidEntry {
                    entry_name: entry_name.to_owned(),
                    reason: format!("invalid type alias generic param bound[{idx}]: {error}"),
                }
            })?;
        }
    }
    Ok(())
}

/// Validates alias `:` where-clause bounds with the same fail-closed lexical
/// rules used by the preserving-spelling encoder.  Non-alias declarations keep
/// the general `syn::TypeParamBound` grammar and are intentionally unchanged.
pub(super) fn validate_type_alias_where_predicates(
    entry_name: &str,
    predicates: &[WherePredicateDecl],
    generic_params: &[&str],
) -> Result<(), CatalogueDocumentCodecError> {
    for predicate in predicates {
        let validate_where_type = |type_ref: &str| {
            if generic_params.is_empty() {
                validate_legacy_type_ref(type_ref, generic_params)
            } else {
                validate_lexical_type_ref(type_ref, generic_params)
            }
        };
        validate_where_type(predicate.lhs.as_str()).map_err(|error| {
            CatalogueDocumentCodecError::InvalidEntry {
                entry_name: entry_name.to_owned(),
                reason: format!("invalid type alias where predicate lhs: {error}"),
            }
        })?;
        for (idx, bound) in predicate.rhs.iter().enumerate() {
            let validation = match predicate.operator {
                BoundOp::Bound => validate_lexical_generic_bound(bound.as_str(), generic_params),
                BoundOp::Equal => validate_where_type(bound.as_str()),
            };
            validation.map_err(|error| CatalogueDocumentCodecError::InvalidEntry {
                entry_name: entry_name.to_owned(),
                reason: format!("invalid type alias where predicate rhs[{idx}]: {error}"),
            })?;
        }
    }
    Ok(())
}

/// Validates a type-alias target. A generic alias routes through the closed
/// lexical grammar — the same generic/legacy fork the where-predicate
/// subjects use — so a target that applies type arguments to a declared
/// parameter (`T<u8>`, rustc E0109) or any other unrepresentable form fails
/// at decode instead of converting to an unresolved marker. Targets use the
/// dedicated lifetime policy of [`validate_lexical_alias_target`]: the schema
/// cannot declare lifetime parameters, so targets carry source-declared
/// lifetimes lexically. A non-generic alias keeps the general type-reference
/// rules used by its encoder.
pub(super) fn validate_type_alias_target(
    entry_name: &str,
    target: &str,
    generic_params: &[&str],
) -> Result<(), CatalogueDocumentCodecError> {
    let validation = if generic_params.is_empty() {
        validate_legacy_type_ref(target, generic_params)
    } else {
        validate_lexical_alias_target(target, generic_params)
    };
    validation.map_err(|error| CatalogueDocumentCodecError::InvalidEntry {
        entry_name: entry_name.to_owned(),
        reason: format!("invalid type alias target: {error}"),
    })?;
    validate_type_alias_params_used(entry_name, target, generic_params)
}

/// Rejects declared type parameters that the alias TARGET never uses: rustc
/// refuses the declaration outright (E0091), even when the parameter appears
/// in bounds or where predicates, so the codec must not admit a contract no
/// implementation can compile.
fn validate_type_alias_params_used(
    entry_name: &str,
    target: &str,
    generic_params: &[&str],
) -> Result<(), CatalogueDocumentCodecError> {
    if generic_params.is_empty() {
        return Ok(());
    }
    let target_type = syn::parse_str::<syn::Type>(target).map_err(|error| {
        CatalogueDocumentCodecError::InvalidEntry {
            entry_name: entry_name.to_owned(),
            reason: format!("invalid type alias target: {error}"),
        }
    })?;
    struct PathRootCollector {
        used: std::collections::BTreeSet<String>,
    }
    impl<'ast> syn::visit::Visit<'ast> for PathRootCollector {
        fn visit_path(&mut self, node: &'ast syn::Path) {
            if node.leading_colon.is_none() {
                if let Some(first) = node.segments.first() {
                    self.used.insert(first.ident.to_string());
                }
            }
            syn::visit::visit_path(self, node);
        }
    }
    let mut collector = PathRootCollector { used: std::collections::BTreeSet::new() };
    syn::visit::Visit::visit_type(&mut collector, &target_type);
    for param in generic_params {
        if !collector.used.contains(*param) {
            return Err(CatalogueDocumentCodecError::InvalidEntry {
                entry_name: entry_name.to_owned(),
                reason: format!(
                    "type parameter `{param}` is not used in the alias target (rustc E0091)"
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
/// Validates the supplied type expression directly with `syn`; no semantic
/// normalization is performed at this boundary.
pub(super) fn validate_type_ref_str_with_generics(
    type_str: &str,
    _generic_params: &[&str],
) -> Result<(), String> {
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
