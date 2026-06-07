//! Canon-aware where-predicate formatter.

use std::collections::HashMap;

use rustdoc_types::{Term, WherePredicate};

use super::ty_base::format_hrtb_type_params;
use super::ty_canon::{format_generic_bounds_with_canon, format_type_with_canon};

/// Canon-aware formatter for a `WherePredicate`. Applies `canon` to the
/// predicate LHS (`Type::Generic` names) and to any inner `Type::Generic`
/// occurrences inside trait-bound args so that renaming a type parameter
/// (`T → U`) does not change the formatted string. Pass an empty `HashMap`
/// when canonicalization is not desired.
///
/// Used by `build_where_form_view` (ADR `2026-05-13-1153` D1) so that A-side
/// (where-form, name = catalogue-author choice) and C-side (where-form virtual
/// view, name = source `T0`/`T1` for APIT) produce the same string when their
/// constraints are positionally identical.
pub(crate) fn format_where_predicate_with_canon(
    pred: &WherePredicate,
    canon: &HashMap<String, String>,
) -> String {
    match pred {
        WherePredicate::BoundPredicate { type_: ty, bounds, generic_params } => {
            let ty_str = format_type_with_canon(ty, canon);
            let bounds_str = format_generic_bounds_with_canon(bounds, canon);
            // Include HRTB type params from the predicate's own binder (e.g. `for<T: Foo>
            // Fn(T): Bar`) so that predicates differing only by their HRTB binder produce
            // distinct strings.
            let hrtb_str = format_hrtb_type_params(generic_params);
            format!("{hrtb_str}{ty_str}:{bounds_str}")
        }
        // Both `LifetimePredicate` and `EqPredicate` are outside ADR `2026-05-13-1153`
        // D3 scope. `build_where_form_view` flags them via `has_unsupported`, but the
        // formatted string is still consumed by `build_generics_fingerprint` which keys
        // `build_trait_method_map`. To preserve distinctness across two methods whose
        // only difference is an unsupported clause, the prefix marker is followed by the
        // predicate's actual content. The `[UNSUPPORTED:` prefix never collides with a
        // well-formed `BoundPredicate` string (which starts with the formatted LHS type).
        WherePredicate::LifetimePredicate { lifetime, outlives } => {
            let bounds_str = outlives.join("+");
            format!("[UNSUPPORTED:Lifetime]{lifetime}:{bounds_str}")
        }
        WherePredicate::EqPredicate { lhs, rhs } => {
            let lhs_str = format_type_with_canon(lhs, canon);
            let rhs_str = match rhs {
                Term::Type(ty) => format_type_with_canon(ty, canon),
                Term::Constant(c) => c.expr.replace("::", "."),
            };
            format!("[UNSUPPORTED:Eq]{lhs_str}={rhs_str}")
        }
    }
}
