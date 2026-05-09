//! Generics, function, and trait structural equality helpers for Phase 2.
//!
//! Provides `generics_structurally_equal`, `build_trait_method_map`,
//! `build_generics_fingerprint`, and `fn_sigs_structurally_equal`.
//! These are used by `structural_eq::items_structurally_equal` indirectly via
//! `traits_structurally_equal` and directly for function and generics comparisons.

use std::collections::{BTreeMap, HashMap};

use rustdoc_types::{GenericParamDefKind, Generics, Id, Item, ItemEnum};

use super::format::{
    build_generic_canon_map, format_generic_bounds, format_type, format_type_with_canon,
    format_where_predicate,
};

// ---------------------------------------------------------------------------
// Trait comparison
// ---------------------------------------------------------------------------

/// Returns `true` if two trait items are structurally equal (same generics,
/// supertrait bounds, and method/assoc-item shapes).
pub(super) fn traits_structurally_equal(
    a: &rustdoc_types::Trait,
    b: &rustdoc_types::Trait,
    a_index: &HashMap<Id, Item>,
    b_index: &HashMap<Id, Item>,
) -> bool {
    // Compare generics and supertrait bounds.
    if !generics_structurally_equal(&a.generics, &b.generics) {
        return false;
    }
    let a_bounds = format_generic_bounds(&a.bounds);
    let b_bounds = format_generic_bounds(&b.bounds);
    if a_bounds != b_bounds {
        return false;
    }
    // Compare method/associated-item maps by name and signature.
    if a.items.len() != b.items.len() {
        return false;
    }
    let a_methods = build_trait_method_map(&a.items, a_index, Some(&a.generics));
    let b_methods = build_trait_method_map(&b.items, b_index, Some(&b.generics));
    a_methods == b_methods
}

/// Builds a combined canonical name map from parent (trait/impl) generics and
/// method-local generics.
///
/// Parent params are assigned `#0`, `#1`, … and method-local params continue
/// from there (`#N`, `#N+1`, …).  This ensures that a method body that refers
/// to the enclosing trait's type parameter (e.g. `T` in `trait Repo<T>`) is
/// mapped to the same positional placeholder as in the other trait definition,
/// regardless of whether the enclosing parameter is named `T` or `U`.
fn build_combined_canon_map(
    parent_generics: Option<&Generics>,
    method_generics: &Generics,
) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let mut idx: usize = 0;
    // Parent params first.
    if let Some(pg) = parent_generics {
        for p in &pg.params {
            match &p.kind {
                GenericParamDefKind::Type { .. } | GenericParamDefKind::Const { .. } => {
                    map.insert(p.name.clone(), format!("#{idx}"));
                    idx += 1;
                }
                GenericParamDefKind::Lifetime { .. } => {}
            }
        }
    }
    // Method-local params (continuing the positional sequence).
    for p in &method_generics.params {
        match &p.kind {
            GenericParamDefKind::Type { .. } | GenericParamDefKind::Const { .. } => {
                map.insert(p.name.clone(), format!("#{idx}"));
                idx += 1;
            }
            GenericParamDefKind::Lifetime { .. } => {}
        }
    }
    map
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
pub(super) fn build_trait_method_map(
    item_ids: &[Id],
    index: &HashMap<Id, Item>,
    parent_generics: Option<&Generics>,
) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for id in item_ids {
        if let Some(item) = index.get(id) {
            if let Some(name) = &item.name {
                let sig_str = match &item.inner {
                    ItemEnum::Function(f) => {
                        // Build a combined canonical map from parent (trait/impl) params and
                        // method-local params so that trait type parameters referenced in the
                        // method body are canonicalized consistently, even when renamed.
                        let canon = build_combined_canon_map(parent_generics, &f.generics);
                        let params: Vec<String> = f
                            .sig
                            .inputs
                            .iter()
                            .map(|(_, ty)| format_type_with_canon(ty, &canon))
                            .collect();
                        let ret = f.sig.output.as_ref().map_or_else(
                            || "()".to_string(),
                            |t| format_type_with_canon(t, &canon),
                        );
                        let variadic = if f.sig.is_c_variadic { "..." } else { "" };
                        // Include ABI so that `extern "C" fn` and `fn` compare as distinct.
                        use super::format::format_abi;
                        let qualifiers = format!(
                            "{}{}{}{}",
                            format_abi(&f.header.abi),
                            if f.header.is_async { "async " } else { "" },
                            if f.header.is_unsafe { "unsafe " } else { "" },
                            if f.header.is_const { "const " } else { "" },
                        );
                        // Include a generics fingerprint so that methods differing only
                        // by generic parameters or where-clause bounds compare unequal.
                        let generic_fp = build_generics_fingerprint(&f.generics);
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
                        // Capture bounds and optional default type so that changes are detected.
                        let bounds_str = format_generic_bounds(bounds);
                        let default_str = type_.as_ref().map_or_else(String::new, format_type);
                        let generic_fp = build_generics_fingerprint(generics);
                        format!("assoc_type[{generic_fp}]:{bounds_str}={default_str}")
                    }
                    ItemEnum::AssocConst { type_, value } => {
                        // Capture the const type and optional default value.
                        let ty_str = format_type(type_);
                        let val_str = value.as_deref().unwrap_or("");
                        format!("assoc_const:{ty_str}={val_str}")
                    }
                    _ => String::new(),
                };
                map.insert(name.clone(), sig_str);
            }
        }
    }
    map
}

// ---------------------------------------------------------------------------
// Generics comparison
// ---------------------------------------------------------------------------

/// Returns a string fingerprint of `Generics` for use in method signature strings.
///
/// The fingerprint encodes non-lifetime param bounds/types and where-clause predicates
/// so that structurally different generic signatures produce distinct strings.
///
/// All bound variants — `TraitBound`, `Outlives`, and `Use` — are included via
/// `format_generic_bounds` so that lifetime-bound changes (`T: 'a` → `T: 'static`)
/// and use-capture-bound changes are also reflected in the fingerprint.
pub(super) fn build_generics_fingerprint(generics: &Generics) -> String {
    let param_parts: Vec<String> = generics
        .params
        .iter()
        .filter_map(|p| match &p.kind {
            GenericParamDefKind::Type { bounds, default, .. } => {
                // Delegate to format_generic_bounds to include ALL bound variants
                // (TraitBound with modifier + HRTB, Outlives, and Use).
                let bounds_str = format_generic_bounds(bounds);
                let default_str = default.as_ref().map_or_else(String::new, format_type);
                Some(format!("T:{bounds_str}={default_str}"))
            }
            GenericParamDefKind::Const { type_, default } => {
                Some(format!("C:{}={}", format_type(type_), default.as_deref().unwrap_or("")))
            }
            GenericParamDefKind::Lifetime { .. } => None,
        })
        .collect();
    let mut where_parts: Vec<String> =
        generics.where_predicates.iter().map(format_where_predicate).collect();
    where_parts.sort_unstable();
    let where_part = where_parts.join(";");
    let param_part = param_parts.join(",");
    if where_part.is_empty() { param_part } else { format!("{param_part} where {where_part}") }
}

/// Compares two `Generics` values for structural equality (name-independent).
///
/// Lifetime parameters are excluded because they don't affect type identity at L1.
/// Type params are compared by their bounds (sorted); const params are compared by
/// their type.  Parameter order is preserved (positional), not sorted.
/// Where-clause predicates are compared as sorted formatted strings.
pub(super) fn generics_structurally_equal(a: &Generics, b: &Generics) -> bool {
    // Build a list of (param kind + bounds/type) strings for non-lifetime params, in order.
    // Delegate to format_generic_bounds for all bound variants (TraitBound, Outlives, Use)
    // so that lifetime bounds (`T: 'a`) and use-capture bounds also trigger inequality.
    let format_param = |p: &rustdoc_types::GenericParamDef| match &p.kind {
        GenericParamDefKind::Type { bounds, default, .. } => {
            let bounds_str = format_generic_bounds(bounds);
            let default_str = default.as_ref().map_or_else(String::new, format_type);
            Some(format!("T:{bounds_str}={default_str}"))
        }
        GenericParamDefKind::Const { type_, default } => {
            Some(format!("C:{}={}", format_type(type_), default.as_deref().unwrap_or("")))
        }
        GenericParamDefKind::Lifetime { .. } => None,
    };
    let param_sigs_a: Vec<String> = a.params.iter().filter_map(format_param).collect();
    let param_sigs_b: Vec<String> = b.params.iter().filter_map(format_param).collect();
    if param_sigs_a != param_sigs_b {
        return false;
    }
    // Compare where predicates as sorted formatted strings.
    let format_where = |g: &Generics| {
        let mut preds: Vec<String> =
            g.where_predicates.iter().map(format_where_predicate).collect();
        preds.sort_unstable();
        preds
    };
    format_where(a) == format_where(b)
}

// ---------------------------------------------------------------------------
// Function comparison
// ---------------------------------------------------------------------------

/// Returns `true` if two function signatures and headers are structurally equal.
pub(super) fn fn_sigs_structurally_equal(
    a_sig: &rustdoc_types::FunctionSignature,
    b_sig: &rustdoc_types::FunctionSignature,
    a_hdr: &rustdoc_types::FunctionHeader,
    b_hdr: &rustdoc_types::FunctionHeader,
    a_generics: &Generics,
    b_generics: &Generics,
) -> bool {
    // Header qualifiers: async, unsafe, const, and ABI all affect the type.
    if a_hdr.is_async != b_hdr.is_async
        || a_hdr.is_unsafe != b_hdr.is_unsafe
        || a_hdr.is_const != b_hdr.is_const
        || a_hdr.abi != b_hdr.abi
    {
        return false;
    }
    // Variadic C-style ABI.
    if a_sig.is_c_variadic != b_sig.is_c_variadic {
        return false;
    }
    if a_sig.inputs.len() != b_sig.inputs.len() {
        return false;
    }
    // Canonicalize generic parameter names on each side independently before
    // comparing types, so that renaming a type parameter (e.g. `T` → `U`)
    // does not cause a false mismatch.  Both sides map their type params to
    // positional placeholders (`#0`, `#1`, …) via `build_generic_canon_map`.
    let canon_a = build_generic_canon_map(a_generics);
    let canon_b = build_generic_canon_map(b_generics);
    let params_equal = a_sig.inputs.iter().zip(b_sig.inputs.iter()).all(|((_, at), (_, bt))| {
        format_type_with_canon(at, &canon_a) == format_type_with_canon(bt, &canon_b)
    });
    if !params_equal {
        return false;
    }
    let ret_a = a_sig
        .output
        .as_ref()
        .map_or_else(|| "()".to_string(), |t| format_type_with_canon(t, &canon_a));
    let ret_b = b_sig
        .output
        .as_ref()
        .map_or_else(|| "()".to_string(), |t| format_type_with_canon(t, &canon_b));
    if ret_a != ret_b {
        return false;
    }
    // Generic parameter count and where-clause predicates.
    generics_structurally_equal(a_generics, b_generics)
}
