//! Function and generics structural equality helpers for Phase 2.
//!
//! Provides `generics_structurally_equal`, `build_generics_fingerprint_with_combined_canon`,
//! and `fn_sigs_structurally_equal` — used by `structural_eq::items_structurally_equal`
//! directly and indirectly via `traits_structurally_equal`.

use std::collections::HashMap;

use rustdoc_types::Generics;

use super::{build_generic_canon_map, format_type_with_canon, format_type_with_canon_occ};

use super::where_form::{build_where_form_view, contains_unsupported_sentinel};

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
pub(crate) fn build_generics_fingerprint_with_combined_canon(
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
/// non-`TraitBound` other than `Outlives`), equality returns `false` unconditionally —
/// even when both sides carry identical unsupported predicates.
/// `GenericBound::Outlives` is within D3 scope and is compared verbatim by lifetime
/// string so that `F: 'static + Fn(...)` compares correctly across A-codec and C-side.
pub(crate) fn generics_structurally_equal(a: &Generics, b: &Generics) -> bool {
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
// Function comparison
// ---------------------------------------------------------------------------

/// Returns `true` if two function signatures and headers are structurally equal.
pub(crate) fn fn_sigs_structurally_equal(
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
    // The synthetic_order list carries occurrence-ordered keys for impl Trait
    // params so that A-side Type::ImplTrait and C-side Type::Generic("impl ...") both
    // map to the same placeholder at the same argument position.
    let (canon_a, synthetic_a) = build_generic_canon_map(a_generics);
    let (canon_b, synthetic_b) = build_generic_canon_map(b_generics);
    // When one side has no synthetic params but the other does, we need to handle the
    // asymmetric case: the side without rustdoc-synthetic params (A-side, catalogue-declared
    // impl Trait as Type::ImplTrait) needs its own occurrence cursor that advances in sync
    // with the C-side cursor.  Both synthetic_order lists should have the same length when
    // the signatures describe the same function; if they differ, the param counts will also
    // differ and the length check above will catch it.
    let mut cursor_a: usize = 0;
    let mut cursor_b: usize = 0;
    // Format each parameter pair and check for unsupported-bound sentinels (D3 fail-closed).
    // `format_type_with_canon_occ` emits `<UNSUPPORTED:ImplTrait>` when an `impl Trait` type
    // carries bounds outside ADR `2026-05-13-1153` D3 scope.  Comparing sentinels from
    // both sides would yield a false positive because both produce the same `<UNSUPPORTED:…>`
    // string.  Checking here ensures such signatures fail closed (D3).
    // use_positional_fallback: when one side has no synthetic params (A-side, catalogue-declared
    // impl Trait as Type::ImplTrait), it needs to generate on-the-fly positional placeholders that
    // mirror the C-side assignment.  Set to `true` when the counterpart has synthetic params.
    // This enables A/C asymmetric comparison (ImplTrait vs Generic("impl ...")).
    // When both sides have no synthetic params (A-A comparison), set to `false` so that
    // Type::ImplTrait falls back to literal bound rendering, preserving order-sensitivity.
    let use_positional_fallback_a = !synthetic_b.is_empty();
    let use_positional_fallback_b = !synthetic_a.is_empty();
    let params_equal = a_sig.inputs.iter().zip(b_sig.inputs.iter()).all(|((_, at), (_, bt))| {
        let sa = format_type_with_canon_occ(
            at,
            &canon_a,
            &synthetic_a,
            use_positional_fallback_a,
            &mut cursor_a,
        );
        let sb = format_type_with_canon_occ(
            bt,
            &canon_b,
            &synthetic_b,
            use_positional_fallback_b,
            &mut cursor_b,
        );
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
