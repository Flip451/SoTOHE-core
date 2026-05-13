//! Generics, function, and trait structural equality helpers for Phase 2.
//!
//! Provides `generics_structurally_equal`, `build_trait_method_map`,
//! and `fn_sigs_structurally_equal`.
//! These are used by `structural_eq::items_structurally_equal` indirectly via
//! `traits_structurally_equal` and directly for function and generics comparisons.

use std::collections::{BTreeMap, HashMap};

use rustdoc_types::{
    GenericBound, GenericParamDefKind, Generics, Id, Item, ItemEnum, Type, WherePredicate,
};

use super::format::{
    build_generic_canon_map, format_generic_bounds, format_generic_bounds_with_canon,
    format_type_with_canon, format_where_predicate_with_canon,
};

// ---------------------------------------------------------------------------
// Where-form normalization (ADR 2026-05-13-1153 D1)
// ---------------------------------------------------------------------------

/// Builds a canonical bound-set list for `generics_structurally_equal` /
/// `build_generics_fingerprint_with_combined_canon`.
///
/// Returns `(param_identity_signatures_in_order, sorted_where_form_predicates,
/// sorted_unsupported_raw_strings)`.
///
/// Where-form predicates include both inline bounds (lifted from `params[T].bounds`)
/// and the original `where_predicates`. Inline bounds on a `Type` param are excluded
/// from the param identity signature (they live in the where-form list instead).
/// Bounds sharing the same LHS (either lifted inline or explicit) are merged into a
/// single entry so that `<T: A + B>` (one inline predicate with two bounds) and
/// `<T> where T: A, T: B` (two explicit predicates each with one bound) produce the
/// same fingerprint.
///
/// The where-form predicates are formatted with `format_where_predicate_with_canon`,
/// applying a positional canon map built from `generics.params` (or `combined_canon`
/// when provided) so that the LHS `Type::Generic(name)` of each virtual where-predicate
/// is rendered as `#N`. This makes the comparison name-independent — a
/// catalogue-declared `<S: Into<String>>` (literal name `S`) and a rustdoc-derived
/// synthetic `<T0: Into<String>>` (literal name `T0`) at the same positional index
/// produce identical where-form strings.
///
/// When `combined_canon` is `Some`, it overrides the locally-built canon. This allows
/// callers in a trait/impl method context to pass the combined parent+method canonical
/// map (built by `build_combined_canon_map`) so that parent-trait generic names
/// appearing in method where-predicates (e.g. `where M: Into<T>` where `T` is a
/// parent-trait param) are also canonicalized.
///
/// The bounds inside each merged predicate are also canonicalized via the active canon
/// so that a bound referencing a generic name (e.g. `Into<U>`) is rendered as
/// `Into<#1>`, making signatures like `<T, U> where T: Into<U>` and
/// `<A, B> where A: Into<B>` compare equal.
///
/// The third return value is a non-empty sorted `Vec<String>` when any predicate or
/// bound falls outside ADR `2026-05-13-1153` D3 scope: `WherePredicate::LifetimePredicate` /
/// `WherePredicate::EqPredicate` / `WherePredicate::BoundPredicate.generic_params`
/// non-empty (HRTB binder) / `GenericBound::Outlives` / non-`TraitBound` variants
/// (e.g. `Use`) / `TraitBound.generic_params` non-empty (HRTB on TraitBound).
/// Each unsupported item contributes a raw string that preserves enough information
/// to distinguish distinct unsupported clauses (e.g. `T: 'static` vs `T: 'a`) so
/// that `build_generics_fingerprint_with_combined_canon` does not collide on them.
/// `generics_structurally_equal` returns `false` unconditionally when either side
/// has a non-empty unsupported list (D3 fail-closed) — even when both sides carry
/// identical unsupported predicates.
///
/// (ADR `2026-05-13-1153-tddd-where-form-generics-normalization` D1 + D3)
fn build_where_form_view(
    generics: &Generics,
    combined_canon: Option<&HashMap<String, String>>,
) -> (Vec<String>, Vec<String>, Vec<String>) {
    let local_canon;
    let canon: &HashMap<String, String> = match combined_canon {
        Some(c) => c,
        None => {
            local_canon = build_generic_canon_map(generics);
            &local_canon
        }
    };
    let mut unsupported_raw: Vec<String> = Vec::new();
    let mut param_sigs: Vec<String> = Vec::new();
    for p in &generics.params {
        match &p.kind {
            GenericParamDefKind::Type { default, .. } => {
                // Bounds intentionally omitted from param identity — they are lifted
                // into the where-form predicate list so that inline and explicit-where
                // forms produce the same fingerprint.
                let default_str =
                    default.as_ref().map_or_else(String::new, |t| format_type_with_canon(t, canon));
                // D3 fail-closed: a type-parameter default that formats to an unsupported
                // sentinel means the default type is outside D3 scope.  Flag it so the
                // caller can reject the pair unconditionally rather than comparing them as
                // equal (two matching sentinel strings would otherwise pass).
                if contains_unsupported_sentinel(&default_str) {
                    unsupported_raw.push(format!("T:default:{default_str}"));
                }
                param_sigs.push(format!("T:={default_str}"));
            }
            GenericParamDefKind::Const { type_, default } => {
                let type_str = format_type_with_canon(type_, canon);
                // D3 fail-closed: an unsupported const-generic type (e.g. a const over
                // `impl Trait`) is outside D3 scope — flag it.
                if contains_unsupported_sentinel(&type_str) {
                    unsupported_raw.push(format!("C:type:{type_str}"));
                }
                param_sigs.push(format!(
                    "C:{}={}",
                    type_str,
                    // Apply the canon map to the default expression so that a const generic
                    // default that references another generic name (e.g. `const SIZE: usize = N`
                    // where `N` is a positional parameter) is canonicalized consistently.
                    // Without this, renaming `N` would change the fingerprint even though the
                    // comparison is supposed to be name-independent.  Mirrors the identical
                    // treatment of `AssocConst` default expressions below (line ~444).
                    apply_canon_to_str(default.as_deref().unwrap_or(""), canon)
                ));
            }
            GenericParamDefKind::Lifetime { .. } => {}
        }
    }
    // Merge bounds by LHS so the inline `<T: A + B>` form and the split
    // `where T: A, T: B` form produce the same fingerprint.
    let mut where_form_map: BTreeMap<String, Vec<GenericBound>> = BTreeMap::new();
    // (1) Inline `params[T].bounds` → merged by canonical LHS.
    for p in &generics.params {
        if let GenericParamDefKind::Type { bounds, .. } = &p.kind {
            if bounds.is_empty() {
                continue;
            }
            if !bounds_supported(bounds) {
                // Collect a raw string so distinct unsupported inline bound sets produce
                // distinct fingerprints (e.g. `T: 'static` vs `T: 'a`).
                let raw_lhs = format_type_with_canon(&Type::Generic(p.name.clone()), canon);
                let raw_bounds = format_generic_bounds_with_canon(bounds, canon);
                unsupported_raw.push(format!("{raw_lhs}:{raw_bounds}"));
                continue;
            }
            let lhs = format_type_with_canon(&Type::Generic(p.name.clone()), canon);
            where_form_map.entry(lhs).or_default().extend(bounds.clone());
        }
    }
    // (2) Explicit where_predicates: merge BoundPredicate bounds by canonical LHS,
    // flag everything else as unsupported (ADR D3 fail-closed).
    for wp in &generics.where_predicates {
        match wp {
            WherePredicate::BoundPredicate { type_, bounds, generic_params } => {
                if !generic_params.is_empty() {
                    // HRTB binder on BoundPredicate — include a raw string for fingerprint
                    // distinctness. Use format_where_predicate_with_canon with empty canon
                    // so the raw names are preserved verbatim.
                    unsupported_raw.push(format_where_predicate_with_canon(wp, &HashMap::new()));
                    continue;
                }
                if !bounds_supported(bounds) {
                    // Unsupported bound shape inside a regular BoundPredicate — canonicalize
                    // the LHS but render the raw bounds so different unsupported shapes differ.
                    let raw_lhs = format_type_with_canon(type_, canon);
                    let raw_bounds = format_generic_bounds_with_canon(bounds, canon);
                    unsupported_raw.push(format!("{raw_lhs}:{raw_bounds}"));
                    continue;
                }
                let lhs = format_type_with_canon(type_, canon);
                // D3 fail-closed: if the LHS itself formats to an unsupported sentinel
                // (e.g. `dyn for<'a> Trait<'a>` → `<UNSUPPORTED:DynTrait>`), treating
                // it as a normal where-form key would cause two predicates with the same
                // unsupported LHS to compare equal.  Flag it as unsupported instead.
                if contains_unsupported_sentinel(&lhs) {
                    let raw_bounds = format_generic_bounds_with_canon(bounds, canon);
                    unsupported_raw.push(format!("{lhs}:{raw_bounds}"));
                    continue;
                }
                where_form_map.entry(lhs).or_default().extend(bounds.clone());
            }
            WherePredicate::LifetimePredicate { .. } | WherePredicate::EqPredicate { .. } => {
                // Collect the raw formatted string so distinct unsupported lifetime /
                // equality predicates produce distinct fingerprints.
                unsupported_raw.push(format_where_predicate_with_canon(wp, &HashMap::new()));
            }
        }
    }
    let mut where_form: Vec<String> = Vec::new();
    for (lhs, bounds) in where_form_map {
        // `lhs` is already the canonical placeholder (`#N`) because it was produced
        // by `format_type_with_canon(…, canon)` above. Passing `canon` to
        // `format_where_predicate_with_canon` canonicalizes generic names that appear
        // inside the bounds (e.g. `Into<U>` → `Into<#1>`), which is necessary so that
        // signatures differing only in generic parameter names compare equal.
        let merged = WherePredicate::BoundPredicate {
            type_: Type::Generic(lhs),
            bounds,
            generic_params: Vec::<rustdoc_types::GenericParamDef>::new(),
        };
        let sig = format_where_predicate_with_canon(&merged, canon);
        // D3 fail-closed: `format_generic_bounds_with_canon` may emit `<UNSUPPORTED:...>`
        // sentinels for nested unsupported types inside otherwise-valid `TraitBound`
        // generic args (e.g. `T: Foo<impl for<'a> Trait<'a>>`).  If the formatted
        // signature contains such a sentinel, push it to `unsupported_raw` so that
        // identical nested-unsupported forms on both sides still produce `false`.
        if contains_unsupported_sentinel(&sig) {
            unsupported_raw.push(sig);
        } else {
            where_form.push(sig);
        }
    }
    where_form.sort_unstable();
    unsupported_raw.sort_unstable();
    (param_sigs, where_form, unsupported_raw)
}

/// Returns `true` if every bound in `bounds` is a `GenericBound::TraitBound` with
/// an empty `generic_params` (no HRTB binder on the trait bound). Per ADR
/// `2026-05-13-1153` D3, any other bound shape (Outlives, Use, HRTB-on-TraitBound)
/// triggers fail-closed.
fn bounds_supported(bounds: &[GenericBound]) -> bool {
    bounds.iter().all(|b| match b {
        GenericBound::TraitBound { generic_params, .. } => generic_params.is_empty(),
        _ => false,
    })
}

/// Returns `true` if `bound` is a `GenericBound::Outlives` (lifetime-bound) entry.
/// Reserved for future filtering inside the bound-set normalization. Outlives bounds
/// in supertrait positions are rejected up-front by `bounds_supported` before
/// `format_generic_bounds` is called, so they do not reach the string-format path.
#[allow(dead_code)]
fn is_outlives(bound: &GenericBound) -> bool {
    matches!(bound, GenericBound::Outlives(_))
}

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
    // D3 fail-closed: supertrait bounds containing `Outlives`, `Use`, or HRTB
    // (`TraitBound` with non-empty `generic_params`) are outside the supported
    // comparison scope. Return `false` unconditionally when either side carries
    // such a bound — even when both sides are identical — to avoid silent equality.
    if !bounds_supported(&a.bounds) || !bounds_supported(&b.bounds) {
        return false;
    }
    let a_bounds = format_generic_bounds(&a.bounds);
    let b_bounds = format_generic_bounds(&b.bounds);
    // D3 fail-closed: `format_generic_bounds` can emit `<UNSUPPORTED:...>` sentinels
    // for nested unsupported types inside otherwise-supported `TraitBound` generic args
    // (e.g. supertrait `Foo<impl for<'a> Trait<'a>>`).  Two identical sentinels would
    // compare equal even though they are outside D3 scope — reject them here.
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

/// Applies a canonical generic-name map to a plain expression string by replacing
/// each whole-word occurrence of a generic name with its positional placeholder.
///
/// Used to canonicalize associated const default values that may reference parent
/// generic parameters (e.g. `const SIZE: usize = N` where `N` is a const generic).
/// Replacement is whole-word only (bounded by non-alphanumeric / non-`_` characters)
/// so that `"N"` in `"N + 1"` is replaced by `"#0"` but `"Nested"` is left intact.
///
/// If `canon` is empty, the original string is returned unchanged.
fn apply_canon_to_str(s: &str, canon: &HashMap<String, String>) -> String {
    if canon.is_empty() || s.is_empty() {
        return s.to_owned();
    }
    let mut result = s.to_owned();
    for (name, placeholder) in canon {
        if name.is_empty() {
            continue;
        }
        // Scan for whole-word occurrences of `name` and replace them.
        let mut out = String::with_capacity(result.len());
        let mut remaining = result.as_str();
        while let Some(pos) = remaining.find(name.as_str()) {
            // Check word boundary: char before the match must not be alphanumeric or `_`.
            let before_ok = pos == 0
                || remaining
                    .get(..pos)
                    .and_then(|s| s.chars().next_back())
                    .is_none_or(|c| !c.is_alphanumeric() && c != '_');
            let after_pos = pos + name.len();
            // Check word boundary: char after the match must not be alphanumeric or `_`.
            let after_ok = remaining
                .get(after_pos..)
                .and_then(|s| s.chars().next())
                .is_none_or(|c| !c.is_alphanumeric() && c != '_');
            if before_ok && after_ok {
                out.push_str(&remaining[..pos]);
                out.push_str(placeholder);
                remaining = &remaining[after_pos..];
            } else {
                // Not a whole-word match; advance past this occurrence to avoid infinite loop.
                let advance = pos + 1;
                out.push_str(&remaining[..advance]);
                remaining = &remaining[advance..];
            }
        }
        out.push_str(remaining);
        result = out;
    }
    result
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
pub(super) fn build_trait_method_map(
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
                        // D3 fail-closed: `format_type_with_canon` emits `<UNSUPPORTED:ImplTrait>`
                        // for `impl Trait` types carrying unsupported bounds.  The sentinel would
                        // compare equal on both sides, yielding a false positive.  Detect it here
                        // and raise the unsupported flag so the trait pair is rejected.
                        if params.iter().any(|s| contains_unsupported_sentinel(s))
                            || contains_unsupported_sentinel(&ret)
                        {
                            has_any_unsupported = true;
                        }
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
                        let assoc_canon = build_combined_canon_map(parent_generics, generics);
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
                        let parent_canon =
                            parent_generics.map(build_generic_canon_map).unwrap_or_default();
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

// ---------------------------------------------------------------------------
// Generics comparison
// ---------------------------------------------------------------------------

/// Returns a string fingerprint of `Generics` for use in method/item signature strings.
///
/// The fingerprint encodes non-lifetime param identity and the normalized where-form
/// predicate set so that structurally different generic signatures produce distinct
/// strings — but the same constraint expressed in inline (`<T: Bound>`) and explicit
/// where (`<T> where T: Bound`) syntax produces the **same** fingerprint.
///
/// `combined_canon` is a pre-built positional name map that covers both the enclosing
/// parent (trait/impl) params and the method-local params. Passing the combined map
/// ensures that where-predicates referencing a parent-trait generic
/// (e.g. `where M: Into<T>` where `T` is the enclosing trait's type param) are
/// canonicalized the same way as the parameter and return-type strings, so that
/// renaming the parent param (`T` → `V`) does not produce a different fingerprint.
///
/// Predicates / bounds outside ADR D3 scope (LifetimePredicate, EqPredicate, HRTB
/// binder, Outlives, non-TraitBound) contribute their raw formatted strings to a
/// `;UNSUPPORTED:…` suffix so that methods with distinct unsupported clauses
/// (e.g. `T: 'static` vs `T: 'a`) produce different fingerprints and do not collide
/// in `build_trait_method_map`.
///
/// (ADR `2026-05-13-1153-tddd-where-form-generics-normalization` D1 + D3)
fn build_generics_fingerprint_with_combined_canon(
    generics: &Generics,
    combined_canon: &HashMap<String, String>,
) -> String {
    let (param_parts, where_parts, unsupported_raw) =
        build_where_form_view(generics, Some(combined_canon));
    let where_part = where_parts.join(";");
    let param_part = param_parts.join(",");
    let base =
        if where_part.is_empty() { param_part } else { format!("{param_part} where {where_part}") };
    if unsupported_raw.is_empty() {
        base
    } else {
        format!("{base};UNSUPPORTED:{}", unsupported_raw.join(","))
    }
}

/// Compares two `Generics` values for structural equality (name-independent).
///
/// Lifetime parameters are excluded because they don't affect type identity at L1.
/// Type params and const params contribute their identity (kind + default) but bounds
/// are lifted into the where-form predicate set. The where-form set is compared as
/// sorted formatted strings.
///
/// **Where-form normalization** (ADR `2026-05-13-1153` D1): inline `<T: Bound>` and
/// explicit `<T> where T: Bound` produce the same predicate set, so equality is
/// representation-independent. Parameter order is preserved (positional).
///
/// **Fail-closed** (ADR `2026-05-13-1153` D3): if either side carries a predicate or
/// bound outside the supported scope (`LifetimePredicate`, `EqPredicate`, HRTB binder,
/// `Outlives`, non-`TraitBound`), equality returns `false` unconditionally — even
/// when both sides carry identical unsupported predicates.
pub(super) fn generics_structurally_equal(a: &Generics, b: &Generics) -> bool {
    let (param_sigs_a, where_a, unsupported_a) = build_where_form_view(a, None);
    let (param_sigs_b, where_b, unsupported_b) = build_where_form_view(b, None);
    // D3 fail-closed: any unsupported predicate/bound on either side → false, even
    // when both sides carry identical unsupported predicates.
    if !unsupported_a.is_empty() || !unsupported_b.is_empty() {
        return false;
    }
    if param_sigs_a != param_sigs_b {
        return false;
    }
    where_a == where_b
}

// ---------------------------------------------------------------------------
// Sentinel helpers
// ---------------------------------------------------------------------------

/// Returns `true` when a formatted type string contains an unsupported-bound sentinel
/// prefix (`<UNSUPPORTED:`).
///
/// `format_type_with_canon` emits `<UNSUPPORTED:ImplTrait>` when an `impl Trait` type
/// in a function signature carries bounds outside ADR `2026-05-13-1153` D3 scope
/// (e.g. `Outlives` / `Use` bounds inside `impl Copy + 'a`).  A sentinel on both
/// sides of a comparison would compare equal and produce a false positive.  Callers
/// must check this function on every formatted string and treat any sentinel hit as
/// a fail-closed signal (D3: return `false`/`has_any_unsupported = true`).
#[inline]
fn contains_unsupported_sentinel(s: &str) -> bool {
    s.contains("<UNSUPPORTED:")
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
    // Format each parameter pair and check for unsupported-bound sentinels (D3 fail-closed).
    // `format_type_with_canon` emits `<UNSUPPORTED:ImplTrait>` when an `impl Trait` type
    // carries bounds outside ADR `2026-05-13-1153` D3 scope.  Comparing sentinels from
    // both sides would yield a false positive because both produce the same `<UNSUPPORTED:…>`
    // string.  Checking here ensures such signatures fail closed (D3).
    let params_equal = a_sig.inputs.iter().zip(b_sig.inputs.iter()).all(|((_, at), (_, bt))| {
        let sa = format_type_with_canon(at, &canon_a);
        let sb = format_type_with_canon(bt, &canon_b);
        // D3 fail-closed: any unsupported sentinel in either side → not equal.
        if contains_unsupported_sentinel(&sa) || contains_unsupported_sentinel(&sb) {
            return false;
        }
        sa == sb
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
    // D3 fail-closed: unsupported sentinel in return type → not equal.
    if contains_unsupported_sentinel(&ret_a) || contains_unsupported_sentinel(&ret_b) {
        return false;
    }
    if ret_a != ret_b {
        return false;
    }
    // Generic parameter count and where-clause predicates.
    generics_structurally_equal(a_generics, b_generics)
}

// ---------------------------------------------------------------------------
// Where-form normalization tests (ADR 2026-05-13-1153 D1 + D3)
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use rustdoc_types::{
        GenericArgs, GenericBound, GenericParamDef, GenericParamDefKind, Generics, Path,
        TraitBoundModifier, Type, WherePredicate,
    };

    use super::generics_structurally_equal;

    // Helper: a simple named type (no generic args).
    fn ty(name: &str) -> Type {
        Type::ResolvedPath(Path { path: name.to_string(), id: rustdoc_types::Id(999), args: None })
    }

    // Helper: a type-param `<T>` with the given inline bounds.
    fn type_param(name: &str, bounds: Vec<GenericBound>) -> GenericParamDef {
        GenericParamDef {
            name: name.to_string(),
            kind: GenericParamDefKind::Type { bounds, default: None, is_synthetic: false },
        }
    }

    // Helper: a `WherePredicate::BoundPredicate` entry for `<T: bounds...>`.
    fn where_bound(type_name: &str, bounds: Vec<GenericBound>) -> WherePredicate {
        WherePredicate::BoundPredicate {
            type_: Type::Generic(type_name.to_string()),
            bounds,
            generic_params: vec![],
        }
    }

    // Helper: a `GenericBound::TraitBound` for a simple named trait (no args, no HRTB).
    fn trait_bound(trait_name: &str) -> GenericBound {
        GenericBound::TraitBound {
            trait_: Path { path: trait_name.to_string(), id: rustdoc_types::Id(0), args: None },
            generic_params: vec![],
            modifier: TraitBoundModifier::None,
        }
    }

    // Helper: generics with no params and no where predicates.
    fn empty_generics() -> Generics {
        Generics { params: vec![], where_predicates: vec![] }
    }

    // --- Normalization: inline bounds ≡ explicit where predicates ---

    /// `<T: Clone>` and `<T> where T: Clone` must compare equal
    /// (inline bound lifted to where-form).
    #[test]
    fn test_inline_bound_equals_explicit_where_predicate() {
        let inline = Generics {
            params: vec![type_param("T", vec![trait_bound("Clone")])],
            where_predicates: vec![],
        };
        let where_form = Generics {
            params: vec![type_param("T", vec![])],
            where_predicates: vec![where_bound("T", vec![trait_bound("Clone")])],
        };
        assert!(
            generics_structurally_equal(&inline, &where_form),
            "<T: Clone> must equal <T> where T: Clone"
        );
    }

    /// `<T: A + B>` and `<T> where T: A + B` must compare equal.
    #[test]
    fn test_inline_multi_bound_equals_explicit_where_predicate() {
        let inline = Generics {
            params: vec![type_param("T", vec![trait_bound("A"), trait_bound("B")])],
            where_predicates: vec![],
        };
        let where_form = Generics {
            params: vec![type_param("T", vec![])],
            where_predicates: vec![where_bound("T", vec![trait_bound("A"), trait_bound("B")])],
        };
        assert!(
            generics_structurally_equal(&inline, &where_form),
            "<T: A + B> must equal <T> where T: A + B"
        );
    }

    /// `<T: A>` and `<T: B>` must compare unequal (different bound sets).
    #[test]
    fn test_different_bounds_compare_unequal() {
        let a = Generics {
            params: vec![type_param("T", vec![trait_bound("A")])],
            where_predicates: vec![],
        };
        let b = Generics {
            params: vec![type_param("T", vec![trait_bound("B")])],
            where_predicates: vec![],
        };
        assert!(!generics_structurally_equal(&a, &b), "<T: A> must NOT equal <T: B>");
    }

    /// Empty generics compare equal to empty generics.
    #[test]
    fn test_empty_generics_equal_empty() {
        assert!(
            generics_structurally_equal(&empty_generics(), &empty_generics()),
            "empty generics must compare equal"
        );
    }

    // --- Fail-closed: unsupported predicates / bounds → false even when identical ---

    /// Two identical `Outlives` bounds (e.g. `T: 'static`) must compare unequal
    /// (D3 fail-closed: Outlives is outside the supported comparison scope).
    #[test]
    fn test_outlives_bound_is_fail_closed() {
        let make = || Generics {
            params: vec![type_param("T", vec![GenericBound::Outlives("'static".to_string())])],
            where_predicates: vec![],
        };
        assert!(
            !generics_structurally_equal(&make(), &make()),
            "identical Outlives bounds must still return false (D3 fail-closed)"
        );
    }

    /// Two identical `LifetimePredicate` where entries must compare unequal (D3 fail-closed).
    #[test]
    fn test_lifetime_predicate_is_fail_closed() {
        let make = || Generics {
            params: vec![],
            where_predicates: vec![WherePredicate::LifetimePredicate {
                lifetime: "'a".to_string(),
                outlives: vec!["'b".to_string()],
            }],
        };
        assert!(
            !generics_structurally_equal(&make(), &make()),
            "identical LifetimePredicates must still return false (D3 fail-closed)"
        );
    }

    /// Two identical HRTB-on-TraitBound entries (`for<'a> Fn(&'a str)`) must compare
    /// unequal (D3 fail-closed: HRTB on TraitBound has non-empty `generic_params`).
    #[test]
    fn test_hrtb_on_trait_bound_is_fail_closed() {
        let hrtb_bound = GenericBound::TraitBound {
            trait_: Path {
                path: "Fn".to_string(),
                id: rustdoc_types::Id(0),
                args: Some(Box::new(GenericArgs::Parenthesized {
                    inputs: vec![Type::BorrowedRef {
                        lifetime: Some("'a".to_string()),
                        is_mutable: false,
                        type_: Box::new(ty("str")),
                    }],
                    output: None,
                })),
            },
            generic_params: vec![GenericParamDef {
                name: "'a".to_string(),
                kind: GenericParamDefKind::Lifetime { outlives: vec![] },
            }],
            modifier: TraitBoundModifier::None,
        };
        let make = || Generics {
            params: vec![type_param("F", vec![hrtb_bound.clone()])],
            where_predicates: vec![],
        };
        assert!(
            !generics_structurally_equal(&make(), &make()),
            "identical HRTB-on-TraitBound must still return false (D3 fail-closed)"
        );
    }

    /// Two identical HRTB binders on `BoundPredicate` (`for<'a> &'a T: Iterator`)
    /// must compare unequal (D3 fail-closed: BoundPredicate.generic_params non-empty).
    #[test]
    fn test_hrtb_binder_on_bound_predicate_is_fail_closed() {
        let make = || Generics {
            params: vec![type_param("T", vec![])],
            where_predicates: vec![WherePredicate::BoundPredicate {
                type_: Type::BorrowedRef {
                    lifetime: Some("'a".to_string()),
                    is_mutable: false,
                    type_: Box::new(Type::Generic("T".to_string())),
                },
                bounds: vec![trait_bound("Iterator")],
                generic_params: vec![GenericParamDef {
                    name: "'a".to_string(),
                    kind: GenericParamDefKind::Lifetime { outlives: vec![] },
                }],
            }],
        };
        assert!(
            !generics_structurally_equal(&make(), &make()),
            "identical HRTB binder on BoundPredicate must still return false (D3 fail-closed)"
        );
    }

    /// Two identical inline `Outlives` lifetime bounds (`<T: 'a>`) must compare
    /// unequal (D3 fail-closed: inline param bounds with Outlives).
    #[test]
    fn test_inline_outlives_bound_is_fail_closed() {
        // Test both the inline-param path AND the where-predicate path explicitly
        // because they go through different code branches in `build_where_form_view`.
        let inline_only = || Generics {
            params: vec![type_param("T", vec![GenericBound::Outlives("'a".to_string())])],
            where_predicates: vec![],
        };
        assert!(
            !generics_structurally_equal(&inline_only(), &inline_only()),
            "identical inline `<T: 'a>` must still return false (D3 fail-closed)"
        );
    }

    /// `GenericBound::Use(...)` inside a where predicate's bounds (e.g. `where T: use<U>`)
    /// must trigger fail-closed (D3: non-`TraitBound` variant).
    #[test]
    fn test_use_bound_in_where_predicate_is_fail_closed() {
        let use_bound =
            GenericBound::Use(vec![rustdoc_types::PreciseCapturingArg::Param("U".to_string())]);
        let make = || Generics {
            params: vec![type_param("T", vec![]), type_param("U", vec![])],
            where_predicates: vec![where_bound("T", vec![use_bound.clone()])],
        };
        assert!(
            !generics_structurally_equal(&make(), &make()),
            "identical `where T: use<U>` must still return false (D3 fail-closed)"
        );
    }

    /// Two identical `EqPredicate` where entries (`where T::Assoc = U`) must compare
    /// unequal (D3 fail-closed: EqPredicate is outside the supported scope).
    #[test]
    fn test_eq_predicate_is_fail_closed() {
        let make = || Generics {
            params: vec![type_param("T", vec![]), type_param("U", vec![])],
            where_predicates: vec![WherePredicate::EqPredicate {
                lhs: Type::QualifiedPath {
                    name: "Assoc".to_string(),
                    args: Some(Box::new(GenericArgs::AngleBracketed {
                        args: vec![],
                        constraints: vec![],
                    })),
                    self_type: Box::new(Type::Generic("T".to_string())),
                    trait_: None,
                },
                rhs: rustdoc_types::Term::Type(Type::Generic("U".to_string())),
            }],
        };
        assert!(
            !generics_structurally_equal(&make(), &make()),
            "identical EqPredicate must still return false (D3 fail-closed)"
        );
    }

    /// `where T: Iterator<Item: for<'a> Foo<&'a str>>` — an HRTB binder appears
    /// as a nested constraint bound inside an associated-type argument (not at the
    /// top-level `BoundPredicate`). Both sides are identical, yet the comparison
    /// must return `false` (D3 fail-closed for HRTB bounds in any position).
    ///
    /// Without the `!generic_params.is_empty()` sentinel in
    /// `format_generic_bounds_with_canon`, `format_hrtb_type_params` would silently
    /// drop the `for<'a>` lifetime, causing `for<'a> Foo<&'a str>` to compare equal
    /// to `Foo<&str>`.
    #[test]
    fn test_hrtb_inside_assoc_type_constraint_is_fail_closed() {
        use rustdoc_types::{AssocItemConstraint, AssocItemConstraintKind, GenericArg};

        let hrtb_bound = GenericBound::TraitBound {
            trait_: Path {
                path: "Foo".to_string(),
                id: rustdoc_types::Id(0),
                args: Some(Box::new(GenericArgs::AngleBracketed {
                    args: vec![GenericArg::Type(Type::BorrowedRef {
                        lifetime: Some("'a".to_string()),
                        is_mutable: false,
                        type_: Box::new(ty("str")),
                    })],
                    constraints: vec![],
                })),
            },
            // Non-empty generic_params = HRTB binder `for<'a>`.
            generic_params: vec![GenericParamDef {
                name: "'a".to_string(),
                kind: GenericParamDefKind::Lifetime { outlives: vec![] },
            }],
            modifier: TraitBoundModifier::None,
        };
        // `where T: Iterator<Item: for<'a> Foo<&'a str>>`
        let iterator_with_hrtb_item = GenericBound::TraitBound {
            trait_: Path {
                path: "Iterator".to_string(),
                id: rustdoc_types::Id(1),
                args: Some(Box::new(GenericArgs::AngleBracketed {
                    args: vec![],
                    constraints: vec![AssocItemConstraint {
                        name: "Item".to_string(),
                        args: Some(Box::new(GenericArgs::AngleBracketed {
                            args: vec![],
                            constraints: vec![],
                        })),
                        binding: AssocItemConstraintKind::Constraint(vec![hrtb_bound]),
                    }],
                })),
            },
            generic_params: vec![],
            modifier: TraitBoundModifier::None,
        };
        let make = || Generics {
            params: vec![type_param("T", vec![])],
            where_predicates: vec![WherePredicate::BoundPredicate {
                type_: Type::Generic("T".to_string()),
                bounds: vec![iterator_with_hrtb_item.clone()],
                generic_params: vec![],
            }],
        };
        assert!(
            !generics_structurally_equal(&make(), &make()),
            "HRTB bound inside associated-type constraint must still return false (D3 fail-closed)"
        );
    }
}
