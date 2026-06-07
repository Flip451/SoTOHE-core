//! Where-form normalization helpers for generics structural equality.
//!
//! Provides `build_where_form_view`, `bounds_supported`, and
//! `contains_unsupported_sentinel` — the building blocks used by
//! `generics_structurally_equal` and `build_generics_fingerprint_with_combined_canon`.

use std::collections::{BTreeMap, HashMap};

use rustdoc_types::{GenericBound, GenericParamDefKind, Generics, Type, WherePredicate};

use super::{
    apply_canon_to_str, build_generic_canon_map, format_generic_bounds_with_canon,
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
/// non-empty (HRTB binder) / non-`TraitBound` variants other than `Outlives`
/// (e.g. `Use`) / `TraitBound.generic_params` non-empty (HRTB on TraitBound).
/// `GenericBound::Outlives` is within D3 scope and is compared verbatim by lifetime
/// string (e.g. `'static`, `'a`) so that `F: 'static + Fn(...)` produces matching
/// fingerprints on both the A-codec side and the C-side (rustdoc) output.
/// Each unsupported item contributes a raw string that preserves enough information
/// to distinguish distinct unsupported clauses so that
/// `build_generics_fingerprint_with_combined_canon` does not collide on them.
/// `generics_structurally_equal` returns `false` unconditionally when either side
/// has a non-empty unsupported list (D3 fail-closed) — even when both sides carry
/// identical unsupported predicates.
///
/// (ADR `2026-05-13-1153-tddd-where-form-generics-normalization` D1 + D3)
pub(crate) fn build_where_form_view(
    generics: &Generics,
    combined_canon: Option<&HashMap<String, String>>,
) -> (Vec<String>, Vec<String>, Vec<String>) {
    let local_canon;
    let canon: &HashMap<String, String> = match combined_canon {
        Some(c) => c,
        None => {
            // build_generic_canon_map returns (name_map, synthetic_order).
            // build_where_form_view only needs the name_map for where-clause rendering
            // (where-predicates use Type::Generic with normal names, not ImplTrait).
            let (name_map, _synthetic_order) = build_generic_canon_map(generics);
            local_canon = name_map;
            &local_canon
        }
    };
    let mut unsupported_raw: Vec<String> = Vec::new();
    let mut param_sigs: Vec<String> = Vec::new();
    for p in &generics.params {
        match &p.kind {
            GenericParamDefKind::Type { default, is_synthetic, .. } => {
                // Synthetic type params are an internal rustdoc artifact from desugaring
                // `impl Trait` function arguments.  They do not correspond to user-declared
                // generics and must be excluded from the where-form fingerprint so that the
                // A-side (catalogue, no synthetic params) and C-side (rustdoc, with synthetic
                // params) produce the same fingerprint for structurally identical signatures.
                if *is_synthetic {
                    continue;
                }
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
                    // treatment of `AssocConst` default expressions below.
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
    // Skip synthetic params: their bounds describe the `impl Trait` constraint, not a
    // user-declared generic bound.  The A-side has no synthetic params, so including
    // them on the C-side would break A/C symmetry (see also the param_sigs loop above).
    for p in &generics.params {
        if let GenericParamDefKind::Type { bounds, is_synthetic, .. } = &p.kind {
            if *is_synthetic {
                continue;
            }
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

/// Returns `true` if every bound in `bounds` is within D5 supported scope:
/// - A `GenericBound::TraitBound` with an empty `generic_params` (no HRTB), or
/// - A `GenericBound::TraitBound` with a lifetime-only `generic_params` binder
///   (HRTB-on-TraitBound, e.g. `for<'_> Fn(&'_ str)`).  Rustdoc desugars elided
///   Fn trait bounds into this form; the binder is normalized away in
///   `format_generic_bounds_with_canon` so both sides compare equal.
///   Type-param binders (`for<T: Foo>`) are still outside scope (fail-closed).
///   Lifetime binders with `outlives` constraints (e.g. `for<'a: 'b>`) are outside
///   scope (fail-closed): the outlives constraint is not represented in the fingerprint,
///   so accepting such binders would silently discard the constraint and risk false Blue.
/// - A `GenericBound::Outlives` (lifetime bound, e.g. `'static` or `'a`).
///
/// Outlives bounds are compared verbatim by their lifetime string so that
/// `F: 'static + Fn(...)` correctly produces the same fingerprint on both the
/// A-codec side and the C-side rustdoc output.
///
/// Any other bound shape (Use, HRTB-on-TraitBound with type or const binders, or
/// HRTB-on-TraitBound with lifetime binders carrying `outlives` constraints) triggers
/// fail-closed.
///
/// (ADR 2026-05-18-1223 D5)
pub(crate) fn bounds_supported(bounds: &[GenericBound]) -> bool {
    bounds.iter().all(|b| match b {
        GenericBound::TraitBound { generic_params, .. } => {
            // Allow empty binder (no HRTB) or lifetime-only binder (HRTB-on-TraitBound)
            // where each lifetime param has no `outlives` constraints.
            // Reject if any binder param is a Type or Const param, or if any lifetime
            // binder param carries non-empty `outlives` constraints.
            generic_params.iter().all(|hp| match &hp.kind {
                GenericParamDefKind::Lifetime { outlives } => outlives.is_empty(),
                GenericParamDefKind::Type { .. } | GenericParamDefKind::Const { .. } => false,
            })
        }
        GenericBound::Outlives(_) => true,
        _ => false,
    })
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
pub(crate) fn contains_unsupported_sentinel(s: &str) -> bool {
    s.contains("<UNSUPPORTED:")
}
