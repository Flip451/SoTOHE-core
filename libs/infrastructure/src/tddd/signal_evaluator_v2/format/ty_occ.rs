//! Occurrence-aware type formatters.
//!
//! Provides `format_type_with_canon_occ` and its inner helper
//! `format_type_with_canon_occ_inner`, plus the occurrence-aware generic-args
//! formatter `format_generic_args_with_canon_occ`.  These are used when A-side
//! (`Type::ImplTrait`) and C-side (`Type::Generic("impl …")`) representations
//! must produce the same placeholder for the same argument position.

use std::collections::HashMap;

use rustdoc_types::{AssocItemConstraintKind, GenericArg, GenericArgs, Term, Type};

use super::canon::{apply_canon_to_str, format_impl_trait_occurrence_key, occurrence_placeholder};
use super::ty_base::{format_generic_args_impl, format_type_common_arms};
use super::ty_canon::{format_generic_bounds_with_canon, format_type_with_canon};

/// Formats a `rustdoc_types::Type` at L1 resolution with an occurrence cursor for
/// positional `impl Trait` placeholder resolution.
///
/// This is the occurrence-aware variant of [`format_type_with_canon`].  It must be used
/// when the caller needs A-side (`Type::ImplTrait`) and C-side (`Type::Generic("impl ...")`)
/// representations to produce the same placeholder for the same argument position.
///
/// `canon` is the **non-synthetic** name→placeholder map from `build_generic_canon_map`.
/// `synthetic_order` is the occurrence-ordered synthetic occurrence-key list (also from
/// `build_generic_canon_map`) for synthetic `impl Trait` params.
/// `cursor` is a shared mutable counter incremented each time an `impl Trait` occurrence
/// is consumed.
///
/// **`use_positional_fallback`:** controls how `Type::ImplTrait` and
/// `Type::Generic("impl ...")` are rendered when `synthetic_order` is empty:
/// - `true` (A-side in an A/C asymmetric comparison): generates on-the-fly positional
///   placeholders as `#(canon.len() + cursor)`.
/// - `false` (A-A symmetric comparison): falls back to `format_type_with_canon` which
///   renders `Type::ImplTrait` as its literal bound string.
///
/// **Supported bounds check:** if an `ImplTrait` carries unsupported bounds
/// (`Outlives`, `Use`, or HRTB `TraitBound`), the cursor is NOT consumed and the call
/// falls through to `format_type_with_canon` which returns the `<UNSUPPORTED:ImplTrait>`
/// sentinel.
pub(crate) fn format_type_with_canon_occ(
    ty: &Type,
    canon: &HashMap<String, String>,
    synthetic_order: &[String],
    use_positional_fallback: bool,
    cursor: &mut usize,
) -> String {
    match ty {
        Type::Generic(name) => {
            if name.starts_with("impl ") {
                if !synthetic_order.is_empty() {
                    let cur = *cursor;
                    if let Some(occurrence_key) = synthetic_order.get(cur) {
                        *cursor += 1;
                        return occurrence_key.clone();
                    }
                }
                if use_positional_fallback {
                    let placeholder = format!("#{}", canon.len() + *cursor);
                    *cursor += 1;
                    let bound_sig = apply_canon_to_str(name, canon);
                    return format_impl_trait_occurrence_key(&placeholder, &bound_sig);
                }
            }
            if let Some(pos) = canon.get(name.as_str()) { pos.clone() } else { name.clone() }
        }
        Type::ImplTrait(bounds) => {
            let has_unsupported = bounds.iter().any(|b| match b {
                rustdoc_types::GenericBound::Outlives(_) | rustdoc_types::GenericBound::Use(_) => {
                    true
                }
                rustdoc_types::GenericBound::TraitBound { generic_params, .. } => {
                    !generic_params.is_empty()
                }
            });
            if has_unsupported {
                return format_type_with_canon(ty, canon);
            }
            let bound_sig = format_type_with_canon(ty, canon);
            if !synthetic_order.is_empty() {
                let cur = *cursor;
                if let Some(occurrence_key) = synthetic_order.get(cur) {
                    *cursor += 1;
                    let placeholder = occurrence_placeholder(occurrence_key);
                    return format_impl_trait_occurrence_key(placeholder, &bound_sig);
                }
            }
            if use_positional_fallback {
                let placeholder = format!("#{}", canon.len() + *cursor);
                *cursor += 1;
                return format_impl_trait_occurrence_key(&placeholder, &bound_sig);
            }
            bound_sig
        }
        other => format_type_with_canon_occ_inner(
            other,
            canon,
            synthetic_order,
            use_positional_fallback,
            cursor,
        ),
    }
}

/// Inner recursive helper for [`format_type_with_canon_occ`].
///
/// Handles all `Type` variants except `Type::Generic` and `Type::ImplTrait` (which are
/// handled by the outer function).  Delegates to [`format_type_common_arms`] with
/// occurrence-aware callbacks so that nested `ImplTrait` or `Generic("impl ...")` values
/// inside composite types also consume the shared cursor with the correct behavior.
///
/// Uses `Cell<usize>` to share the occurrence cursor across the two `FnMut` callbacks
/// without violating Rust's borrow checker.
fn format_type_with_canon_occ_inner(
    ty: &Type,
    canon: &HashMap<String, String>,
    synthetic_order: &[String],
    use_positional_fallback: bool,
    cursor: &mut usize,
) -> String {
    use std::cell::Cell;
    let cursor_cell = Cell::new(*cursor);
    let result = format_type_common_arms(
        ty,
        canon,
        &mut |t| {
            let mut c = cursor_cell.get();
            let r = format_type_with_canon_occ(
                t,
                canon,
                synthetic_order,
                use_positional_fallback,
                &mut c,
            );
            cursor_cell.set(c);
            r
        },
        &mut |args| {
            let mut c = cursor_cell.get();
            let r = format_generic_args_with_canon_occ(
                args,
                canon,
                synthetic_order,
                use_positional_fallback,
                &mut c,
            );
            cursor_cell.set(c);
            r
        },
    );
    *cursor = cursor_cell.get();
    result
}

/// Formats `GenericArgs` with occurrence-aware canonicalization.
///
/// Delegates to [`format_generic_args_impl`] with occurrence-aware callbacks via
/// `Cell<usize>` interior mutability.  For `AngleBracketed` args, pre-sorts
/// constraints by their canonical key (computed via
/// [`format_assoc_constraint_sort_key_with_canon`]) before passing them to
/// [`format_generic_args_impl`], so that occurrence placeholders are assigned to
/// constraints in a stable order that does not depend on their source-declaration
/// order.  After the pre-sort the post-format sort inside
/// [`format_generic_args_impl`] is a no-op because the canonical key ordering
/// matches the alphabetical ordering of the formatted strings.
fn format_generic_args_with_canon_occ(
    args: &GenericArgs,
    canon: &HashMap<String, String>,
    synthetic_order: &[String],
    use_positional_fallback: bool,
    cursor: &mut usize,
) -> String {
    use std::cell::Cell;
    // For AngleBracketed args, pre-sort constraints by canonical key so the
    // occurrence cursor is consumed in a deterministic order.
    let pre_sorted: Option<GenericArgs> = match args {
        GenericArgs::AngleBracketed { args: pos_args, constraints } => {
            let mut sorted: Vec<_> = constraints.to_vec();
            sorted.sort_by_cached_key(|c| format_assoc_constraint_sort_key_with_canon(c, canon));
            Some(GenericArgs::AngleBracketed { args: pos_args.clone(), constraints: sorted })
        }
        _ => None,
    };
    let effective_args: &GenericArgs = pre_sorted.as_ref().unwrap_or(args);
    let cursor_cell = Cell::new(*cursor);
    let fmt_type_occ = |t: &Type| {
        let mut c = cursor_cell.get();
        let r =
            format_type_with_canon_occ(t, canon, synthetic_order, use_positional_fallback, &mut c);
        cursor_cell.set(c);
        r
    };
    let result = format_generic_args_impl(
        effective_args,
        &|arg| {
            Some(match arg {
                GenericArg::Type(t) => fmt_type_occ(t),
                GenericArg::Lifetime(lt) => {
                    canon.get(lt.as_str()).cloned().unwrap_or_else(|| lt.clone())
                }
                GenericArg::Const(c) => apply_canon_to_str(&c.expr.replace("::", "."), canon),
                GenericArg::Infer => "_".to_string(),
            })
        },
        &fmt_type_occ,
        &|s| apply_canon_to_str(s, canon),
        &|bounds| format_generic_bounds_with_canon(bounds, canon),
    );
    *cursor = cursor_cell.get();
    result
}

fn format_assoc_constraint_sort_key_with_canon(
    c: &rustdoc_types::AssocItemConstraint,
    canon: &HashMap<String, String>,
) -> String {
    match &c.binding {
        AssocItemConstraintKind::Equality(Term::Type(ty)) => {
            let rhs = format_type_with_canon(ty, canon);
            if rhs.is_empty() { c.name.clone() } else { format!("{}={}", c.name, rhs) }
        }
        AssocItemConstraintKind::Equality(Term::Constant(cv)) => {
            let rhs = apply_canon_to_str(&cv.expr.replace("::", "."), canon);
            if rhs.is_empty() { c.name.clone() } else { format!("{}={}", c.name, rhs) }
        }
        AssocItemConstraintKind::Constraint(bounds) => {
            let rhs = format_generic_bounds_with_canon(bounds, canon);
            if rhs.is_empty() { c.name.clone() } else { format!("{}:{}", c.name, rhs) }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests for occurrence-aware formatters
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use rustdoc_types::{
        AssocItemConstraint, AssocItemConstraintKind, GenericArgs, Id, Path, Term, Type,
    };

    use super::format_type_with_canon_occ;

    fn trait_bound(name: &str) -> rustdoc_types::GenericBound {
        rustdoc_types::GenericBound::TraitBound {
            trait_: Path { path: name.to_owned(), id: Id(0), args: None },
            generic_params: vec![],
            modifier: rustdoc_types::TraitBoundModifier::None,
        }
    }

    fn impl_constraint(name: &str, bound_name: &str) -> AssocItemConstraint {
        AssocItemConstraint {
            name: name.to_owned(),
            args: None,
            binding: AssocItemConstraintKind::Equality(Term::Type(Type::ImplTrait(vec![
                trait_bound(bound_name),
            ]))),
        }
    }

    fn resolved_with_constraints(constraints: Vec<AssocItemConstraint>) -> Type {
        Type::ResolvedPath(Path {
            path: "Foo".to_owned(),
            id: Id(0),
            args: Some(Box::new(GenericArgs::AngleBracketed { args: vec![], constraints })),
        })
    }

    #[test]
    fn test_format_type_with_canon_occ_reordered_constraints_use_same_placeholders() {
        let ordered = resolved_with_constraints(vec![
            impl_constraint("A", "Display"),
            impl_constraint("B", "Debug"),
        ]);
        let reordered = resolved_with_constraints(vec![
            impl_constraint("B", "Debug"),
            impl_constraint("A", "Display"),
        ]);
        let canon = std::collections::HashMap::new();
        let mut ordered_cursor = 0;
        let mut reordered_cursor = 0;

        let ordered_rendered =
            format_type_with_canon_occ(&ordered, &canon, &[], true, &mut ordered_cursor);
        let reordered_rendered =
            format_type_with_canon_occ(&reordered, &canon, &[], true, &mut reordered_cursor);

        assert_eq!(ordered_cursor, 2, "ordered constraints must consume both impl Trait entries");
        assert_eq!(
            reordered_cursor, 2,
            "reordered constraints must consume both impl Trait entries"
        );
        assert_eq!(
            ordered_rendered, "Foo<A=#0:impl Display, B=#1:impl Debug>",
            "constraints must be rendered in canonical order before placeholders are assigned"
        );
        assert_eq!(
            ordered_rendered, reordered_rendered,
            "constraint source order must not change occurrence placeholder assignment"
        );
    }

    // --- DynTrait bound-order canonicalization stability ---
    //
    // Verifies that the occurrence cursor is never consumed inside `dyn Trait`'s
    // generic args (so sorting bounds after formatting them is safe).

    fn make_dyn_trait_type(trait_names: &[&str]) -> Type {
        use rustdoc_types::{DynTrait, PolyTrait};
        Type::DynTrait(DynTrait {
            traits: trait_names
                .iter()
                .map(|name| PolyTrait {
                    trait_: Path { path: name.to_string(), id: Id(0), args: None },
                    generic_params: vec![],
                })
                .collect(),
            lifetime: None,
        })
    }

    #[test]
    fn test_dyn_trait_bound_order_canonicalization_is_stable() {
        use std::collections::HashMap;

        let dyn_ab = make_dyn_trait_type(&["Display", "Debug"]);
        let dyn_ba = make_dyn_trait_type(&["Debug", "Display"]);
        let canon: HashMap<String, String> = HashMap::new();
        let mut cursor_ab = 0usize;
        let mut cursor_ba = 0usize;

        let result_ab = format_type_with_canon_occ(&dyn_ab, &canon, &[], false, &mut cursor_ab);
        let result_ba = format_type_with_canon_occ(&dyn_ba, &canon, &[], false, &mut cursor_ba);

        assert_eq!(
            result_ab, result_ba,
            "dyn Trait bound order must not affect canonicalized string; \
             ab={result_ab:?} ba={result_ba:?}"
        );
        assert_eq!(
            cursor_ab, 0,
            "dyn Trait formatting must not consume the impl Trait occurrence cursor; \
             cursor advanced to {cursor_ab}"
        );
        assert_eq!(
            cursor_ba, 0,
            "dyn Trait formatting must not consume the impl Trait occurrence cursor; \
             cursor advanced to {cursor_ba}"
        );
    }

    #[test]
    fn test_dyn_trait_with_generic_args_does_not_consume_cursor() {
        use std::collections::HashMap;

        use rustdoc_types::{DynTrait, GenericArg, GenericArgs, PolyTrait};

        fn make_poly_trait(name: &str, arg_ty: &str) -> PolyTrait {
            PolyTrait {
                trait_: Path {
                    path: name.to_string(),
                    id: Id(0),
                    args: Some(Box::new(GenericArgs::AngleBracketed {
                        args: vec![GenericArg::Type(Type::Primitive(arg_ty.to_string()))],
                        constraints: vec![],
                    })),
                },
                generic_params: vec![],
            }
        }

        let dyn_ab = Type::DynTrait(DynTrait {
            traits: vec![make_poly_trait("Foo", "u8"), make_poly_trait("Bar", "u16")],
            lifetime: None,
        });
        let dyn_ba = Type::DynTrait(DynTrait {
            traits: vec![make_poly_trait("Bar", "u16"), make_poly_trait("Foo", "u8")],
            lifetime: None,
        });

        let canon: HashMap<String, String> = HashMap::new();
        let mut cursor_ab = 0usize;
        let mut cursor_ba = 0usize;

        let result_ab = format_type_with_canon_occ(&dyn_ab, &canon, &[], false, &mut cursor_ab);
        let result_ba = format_type_with_canon_occ(&dyn_ba, &canon, &[], false, &mut cursor_ba);

        assert_eq!(
            result_ab, result_ba,
            "dyn Trait with primitive generic args: bound order must not affect output; \
             ab={result_ab:?} ba={result_ba:?}"
        );
        assert_eq!(
            cursor_ab, 0,
            "dyn Trait with primitive args must not consume cursor; advanced to {cursor_ab}"
        );
    }
}
