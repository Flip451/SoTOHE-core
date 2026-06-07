//! Trait and associated-item structural equality helpers for Phase 2.
//!
//! Provides `traits_structurally_equal` and `build_trait_method_map`, used by
//! `structural_eq::items_structurally_equal` via `traits_structurally_equal`.

use std::collections::{BTreeMap, HashMap};

use rustdoc_types::{Generics, Id, Item, ItemEnum};

use super::{
    apply_canon_to_str, build_generic_canon_map, build_generic_canon_map_from_groups, format_abi,
    format_generic_bounds_with_canon, format_type_with_canon, format_type_with_canon_occ,
};

use super::fn_eq::{build_generics_fingerprint_with_combined_canon, generics_structurally_equal};
use super::where_form::{bounds_supported, build_where_form_view, contains_unsupported_sentinel};

// ---------------------------------------------------------------------------
// Trait comparison
// ---------------------------------------------------------------------------

/// Returns `true` if two trait items are structurally equal (same generics,
/// supertrait bounds, and method/assoc-item shapes).
pub(crate) fn traits_structurally_equal(
    a: &rustdoc_types::Trait,
    b: &rustdoc_types::Trait,
    a_index: &HashMap<Id, Item>,
    b_index: &HashMap<Id, Item>,
) -> bool {
    // Compare generics and supertrait bounds.
    if !generics_structurally_equal(&a.generics, &b.generics) {
        return false;
    }
    // D3 fail-closed: supertrait bounds containing `Outlives`, `Use`, or HRTB
    // (`TraitBound` with non-empty `generic_params`) are outside the supported
    // comparison scope. Return `false` unconditionally when either side carries
    // such a bound — even when both sides are identical — to avoid silent equality.
    if !bounds_supported(&a.bounds) || !bounds_supported(&b.bounds) {
        return false;
    }
    // Build per-side positional canon maps from the trait's own generic parameters
    // so that supertrait bounds that reference a trait generic (e.g. `From<T>` in
    // `trait Repo<T>: From<T>`) are rendered as `From<#0>` on both sides, making
    // a rename of `T` → `U` invisible to the comparison.  This mirrors the
    // canon-aware treatment already applied to method and associated-type paths.
    // Supertrait bounds use named generic params (not synthetic impl Trait params),
    // so only the name_map part of build_generic_canon_map is needed here.
    let (a_trait_canon, _) = build_generic_canon_map(&a.generics);
    let (b_trait_canon, _) = build_generic_canon_map(&b.generics);
    let a_bounds = format_generic_bounds_with_canon(&a.bounds, &a_trait_canon);
    let b_bounds = format_generic_bounds_with_canon(&b.bounds, &b_trait_canon);
    // D3 fail-closed: `format_generic_bounds_with_canon` can emit `<UNSUPPORTED:...>`
    // sentinels for nested unsupported types inside otherwise-supported `TraitBound`
    // generic args (e.g. supertrait `Foo<impl for<'a> Trait<'a>>`).  Two identical
    // sentinels would compare equal even though they are outside D3 scope — reject
    // them here.
    if contains_unsupported_sentinel(&a_bounds) || contains_unsupported_sentinel(&b_bounds) {
        return false;
    }
    if a_bounds != b_bounds {
        return false;
    }
    // Compare method/associated-item maps by name and signature.
    if a.items.len() != b.items.len() {
        return false;
    }
    let (a_methods, a_items_unsupported) =
        build_trait_method_map(&a.items, a_index, Some(&a.generics));
    let (b_methods, b_items_unsupported) =
        build_trait_method_map(&b.items, b_index, Some(&b.generics));
    // D3 fail-closed: any method or associated item with unsupported generics on
    // either side makes the trait pair unconditionally unequal.
    if a_items_unsupported || b_items_unsupported {
        return false;
    }
    a_methods == b_methods
}

/// Builds a combined canonical name map and synthetic occurrence list for
/// method-signature comparison across trait definitions.
///
/// Delegates to [`build_generic_canon_map_from_groups`] with parent generics
/// (if any) prepended to method generics.  Parent params are assigned `#0`,
/// `#1`, … and method-local params continue from `#N`, `#N+1`, … so that
/// a method referencing the enclosing trait's type parameter (e.g. `T` in
/// `trait Repo<T>`) is canonicalized consistently regardless of parameter name.
///
/// When `parent_generics` is `None`, the method-only group is passed directly.
///
/// Returns `(name_map, synthetic_order)` where:
/// - `name_map`: non-synthetic type/const parameter name → `#idx` placeholder.
/// - `synthetic_order`: occurrence-ordered synthetic `impl Trait` occurrence keys
///   combined from parent + method params in declaration order.
pub(crate) fn build_combined_canon_map(
    parent_generics: Option<&Generics>,
    method_generics: &Generics,
) -> (HashMap<String, String>, Vec<String>) {
    // Delegate to the shared multi-group builder.
    match parent_generics {
        Some(pg) => build_generic_canon_map_from_groups(&[pg, method_generics]),
        None => build_generic_canon_map_from_groups(&[method_generics]),
    }
}

/// Builds a `method_name → sig_string` map for trait items.
///
/// Functions are keyed by name and valued by their signature string (parameter
/// types + return type, excluding parameter binding names and docs).
/// Non-function items are included as `name → ""` so that added/removed
/// associated types also register as a structural change.
///
/// `parent_generics` is the enclosing trait's or impl's `Generics`.  When provided,
/// the canonical map is built from parent params first (so that trait type parameters
/// referenced in method signatures are canonicalized consistently across renames).
///
/// Returns `(method_map, has_any_unsupported)`. When `has_any_unsupported` is `true`,
/// at least one method or associated item in the list carries a generic predicate or
/// bound outside ADR `2026-05-13-1153` D3 scope. Callers should reject the trait pair
/// unconditionally when either side sets this flag (D3 fail-closed).
pub(crate) fn build_trait_method_map(
    item_ids: &[Id],
    index: &HashMap<Id, Item>,
    parent_generics: Option<&Generics>,
) -> (BTreeMap<String, String>, bool) {
    let mut map = BTreeMap::new();
    let mut has_any_unsupported = false;
    for id in item_ids {
        if let Some(item) = index.get(id) {
            if let Some(name) = &item.name {
                let sig_str = match &item.inner {
                    ItemEnum::Function(f) => {
                        // Build a combined canonical map from parent (trait/impl) params and
                        // method-local params so that trait type parameters referenced in the
                        // method body are canonicalized consistently, even when renamed.
                        // The synthetic_order list carries occurrence keys for impl Trait params
                        // in declaration order, enabling occurrence-based A/C-side symmetry.
                        let (canon, synthetic_order) =
                            build_combined_canon_map(parent_generics, &f.generics);
                        // Format each param with the occurrence-aware formatter so that
                        // Type::ImplTrait (A-side) and Type::Generic("impl ...") (C-side)
                        // consume the same cursor position and produce the same placeholder.
                        //
                        // use_positional_fallback: when synthetic_order is empty, this item
                        // is the A-side (catalogue, Type::ImplTrait).  Setting `true` causes
                        // Type::ImplTrait to generate on-the-fly positional placeholders (#N)
                        // that mirror what the C-side produces from its synthetic_order list.
                        // When synthetic_order is non-empty (C-side), the list is consumed
                        // directly and the flag is not needed.
                        let use_positional_fallback = synthetic_order.is_empty();
                        let mut cursor: usize = 0;
                        let params: Vec<String> = f
                            .sig
                            .inputs
                            .iter()
                            .map(|(_, ty)| {
                                format_type_with_canon_occ(
                                    ty,
                                    &canon,
                                    &synthetic_order,
                                    use_positional_fallback,
                                    &mut cursor,
                                )
                            })
                            .collect();
                        let ret = f.sig.output.as_ref().map_or_else(
                            || "()".to_string(),
                            |t| format_type_with_canon(t, &canon),
                        );
                        // D3 fail-closed: `format_type_with_canon_occ` / `format_type_with_canon`
                        // emit `<UNSUPPORTED:ImplTrait>` for `impl Trait` types carrying unsupported
                        // bounds.  The sentinel would compare equal on both sides, yielding a false
                        // positive.  Detect it here and raise the unsupported flag.
                        if params.iter().any(|s| contains_unsupported_sentinel(s))
                            || contains_unsupported_sentinel(&ret)
                        {
                            has_any_unsupported = true;
                        }
                        let variadic = if f.sig.is_c_variadic { "..." } else { "" };
                        // Include ABI so that `extern "C" fn` and `fn` compare as distinct.
                        let qualifiers = format!(
                            "{}{}{}{}",
                            format_abi(&f.header.abi),
                            if f.header.is_async { "async " } else { "" },
                            if f.header.is_unsafe { "unsafe " } else { "" },
                            if f.header.is_const { "const " } else { "" },
                        );
                        // Check for D3 unsupported generics on this method. Use the
                        // combined canon so that parent-trait generic names in
                        // where-predicates are also canonicalized.
                        let (_, _, unsupported_raw) =
                            build_where_form_view(&f.generics, Some(&canon));
                        if !unsupported_raw.is_empty() {
                            has_any_unsupported = true;
                        }
                        // Include a generics fingerprint with the combined canon so that
                        // methods differing only by generic parameters or where-clause
                        // bounds compare unequal, and parent-generic names in predicates
                        // are canonicalized the same way as in parameter/return types.
                        let generic_fp =
                            build_generics_fingerprint_with_combined_canon(&f.generics, &canon);
                        // Include has_body so that changing a required method (no body)
                        // to a provided method (with default body) registers as a mismatch.
                        let body_marker = if f.has_body { ";body" } else { ";abstract" };
                        format!(
                            "{qualifiers}[{generic_fp}]({}{}) -> {ret}{body_marker}",
                            params.join(","),
                            variadic
                        )
                    }
                    ItemEnum::AssocType { generics, bounds, type_ } => {
                        // Build combined canon for associated type (parent + assoc-type-local).
                        // AssocType defaults / bounds do not use impl Trait params, so only the
                        // name_map is needed here.
                        let (assoc_canon, _assoc_synthetic) =
                            build_combined_canon_map(parent_generics, generics);
                        // Check for D3 unsupported generics on this associated type, using the
                        // combined canon so parent-generic references in predicates are seen.
                        let (_, _, unsupported_raw) =
                            build_where_form_view(generics, Some(&assoc_canon));
                        if !unsupported_raw.is_empty() {
                            has_any_unsupported = true;
                        }
                        // D3 fail-closed: unsupported bounds (Outlives, Use, HRTB-on-TraitBound)
                        // on the associated type itself make the trait pair unconditionally
                        // unequal — same policy as supertrait bounds in `traits_structurally_equal`.
                        if !bounds_supported(bounds) {
                            has_any_unsupported = true;
                        }
                        // Capture bounds and optional default type so that changes are detected.
                        // Use the canon-aware formatters so associated-type bounds and default
                        // types that reference parent generic parameters compare equal regardless
                        // of parameter name (e.g. `trait Foo<T>: type Item = Vec<T>` vs
                        // `trait Foo<U>: type Item = Vec<U>` — `T` and `U` both map to `#0`).
                        let bounds_str = format_generic_bounds_with_canon(bounds, &assoc_canon);
                        // D3 fail-closed: `format_generic_bounds_with_canon` can emit
                        // `<UNSUPPORTED:...>` for nested unsupported types inside an
                        // otherwise-valid `TraitBound` (e.g. `type Item: Foo<impl Trait>`).
                        // Two identical sentinels would compare equal — flag it.
                        if contains_unsupported_sentinel(&bounds_str) {
                            has_any_unsupported = true;
                        }
                        let default_str = type_
                            .as_ref()
                            .map_or_else(String::new, |t| format_type_with_canon(t, &assoc_canon));
                        // D3 fail-closed: an associated-type default that contains an
                        // unsupported sentinel (e.g. `impl for<'a> Trait<'a>`) would
                        // compare equal on both sides even though it is outside D3 scope.
                        // Detect it here and raise the unsupported flag.
                        if contains_unsupported_sentinel(&default_str) {
                            has_any_unsupported = true;
                        }
                        let generic_fp =
                            build_generics_fingerprint_with_combined_canon(generics, &assoc_canon);
                        format!("assoc_type[{generic_fp}]:{bounds_str}={default_str}")
                    }
                    ItemEnum::AssocConst { type_, value } => {
                        // Build parent-only canon so that associated const types and default
                        // value expressions that reference parent generic parameters are
                        // canonicalized consistently, even when those parameters are renamed
                        // (e.g. `trait Foo<N: usize>: const SIZE: usize = N` — `N` maps to
                        // `#0` so renaming `N → M` does not change the signature string).
                        // AssocConst has no local generics, so only the parent map is needed.
                        // build_generic_canon_map returns (name_map, synthetic_order); take only
                        // name_map (synthetic params don't appear in associated const types).
                        let parent_canon = parent_generics
                            .map(|pg| build_generic_canon_map(pg).0)
                            .unwrap_or_default();
                        let ty_str = format_type_with_canon(type_, &parent_canon);
                        // D3 fail-closed: an associated-const type containing an unsupported
                        // sentinel means the type itself is outside D3 scope.
                        if contains_unsupported_sentinel(&ty_str) {
                            has_any_unsupported = true;
                        }
                        // Apply the canon map to the value string: replace each generic name
                        // that appears as a whole word with its positional placeholder.
                        let val_str =
                            apply_canon_to_str(value.as_deref().unwrap_or(""), &parent_canon);
                        format!("assoc_const:{ty_str}={val_str}")
                    }
                    _ => String::new(),
                };
                map.insert(name.clone(), sig_str);
            }
        }
    }
    (map, has_any_unsupported)
}
