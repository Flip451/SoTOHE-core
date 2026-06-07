//! Canon-aware type and bounds formatters, generic canon map builder.
//!
//! Contains all formatters that thread a `HashMap<String,String>` canon map:
//! - `build_generic_canon_map` — builds the canon map from a `Generics` struct
//! - `format_type_with_canon` — type formatter with generic-param canonicalization
//! - `format_generic_args_with_canon` — generic-args formatter with canonicalization
//! - `format_generic_bounds_with_canon` — bounds formatter with HRTB-D5 support
//! - `count_binder_lifetime_in_*` — helpers for HRTB Fn-desugaring detection
//!
//! Note: `format_where_predicate_with_canon` lives in `where_pred.rs`.
//! Note: `format_generic_bounds_strip_type_params` lives in `ty_strip.rs`.

use std::collections::HashMap;

use rustdoc_types::{GenericArg, GenericArgs, GenericBound, GenericParamDefKind, Generics, Type};

use super::canon::{apply_canon_to_str, format_impl_trait_occurrence_key};
use super::ty_base::{format_generic_args_impl, format_type_common_arms};

/// Builds the canonical name map and synthetic-param occurrence list from an ordered
/// sequence of `Generics` groups.
///
/// Groups are processed in the order given: each group's non-synthetic type and const
/// params are assigned positional `#0`, `#1`, … placeholders continuing from the
/// previous group. Synthetic `impl Trait` params are recorded in `synthetic_order` in
/// declaration order across all groups.
///
/// This is the shared implementation used by both [`build_generic_canon_map`] (single
/// group) and `build_combined_canon_map` in `trait_eq` (parent + method groups).
pub(crate) fn build_generic_canon_map_from_groups(
    groups: &[&Generics],
) -> (HashMap<String, String>, Vec<String>) {
    let mut map = HashMap::new();
    let mut idx: usize = 0;
    // First pass: assign placeholders to non-synthetic type/const params across all groups.
    for generics in groups {
        for p in &generics.params {
            match &p.kind {
                GenericParamDefKind::Type { is_synthetic, .. } => {
                    let placeholder = format!("#{idx}");
                    if !*is_synthetic {
                        map.insert(p.name.clone(), placeholder);
                    }
                    idx += 1;
                }
                GenericParamDefKind::Const { .. } => {
                    map.insert(p.name.clone(), format!("#{idx}"));
                    idx += 1;
                }
                GenericParamDefKind::Lifetime { .. } => {}
            }
        }
    }
    // Second pass: record synthetic occurrence keys in declaration order.
    let mut synthetic_order: Vec<String> = Vec::new();
    idx = 0;
    for generics in groups {
        for p in &generics.params {
            match &p.kind {
                GenericParamDefKind::Type { bounds, is_synthetic, .. } => {
                    if *is_synthetic {
                        let placeholder = format!("#{idx}");
                        let bound_sig =
                            format_type_with_canon(&Type::ImplTrait(bounds.clone()), &map);
                        synthetic_order
                            .push(format_impl_trait_occurrence_key(&placeholder, &bound_sig));
                    }
                    idx += 1;
                }
                GenericParamDefKind::Const { .. } => {
                    idx += 1;
                }
                GenericParamDefKind::Lifetime { .. } => {}
            }
        }
    }
    (map, synthetic_order)
}

/// Builds the canonical name map and synthetic-param occurrence list.
/// See module-level docs for full design rationale.
pub(crate) fn build_generic_canon_map(
    generics: &Generics,
) -> (HashMap<String, String>, Vec<String>) {
    build_generic_canon_map_from_groups(&[generics])
}

/// Formats a `rustdoc_types::Type` at L1 resolution with generic-param canonicalization.
///
/// `Type::Generic(name)` values are replaced by positional placeholders stored in
/// `canon` (e.g. `"#0"`, `"#1"`) so that signatures differing only in generic
/// parameter names compare as structurally equal.
///
/// For `Type::ImplTrait`, bounds are rendered literally.  For occurrence-aware
/// formatting (A-side vs C-side `impl Trait` symmetry), use `format_type_with_canon_occ`.
///
/// All structural arms (path shortening, reference lifetimes, slices, arrays, tuples,
/// raw pointers, dyn trait, function pointers, pattern types, qualified paths) delegate
/// to [`format_type_common_arms`] so that canon-aware and occurrence-aware renderers share
/// a single traversal implementation.
pub(crate) fn format_type_with_canon(ty: &Type, canon: &HashMap<String, String>) -> String {
    match ty {
        Type::Generic(name) => canon.get(name.as_str()).cloned().unwrap_or_else(|| name.clone()),
        Type::ImplTrait(bounds) => {
            // D3 fail-closed: Outlives, Use, and HRTB binders are outside scope.
            let has_unsupported = bounds.iter().any(|b| match b {
                rustdoc_types::GenericBound::Outlives(_) | rustdoc_types::GenericBound::Use(_) => {
                    true
                }
                rustdoc_types::GenericBound::TraitBound { generic_params, .. } => {
                    !generic_params.is_empty()
                }
            });
            if has_unsupported {
                return "<UNSUPPORTED:ImplTrait>".to_string();
            }
            let mut parts: Vec<String> = bounds
                .iter()
                .filter_map(|b| match b {
                    rustdoc_types::GenericBound::TraitBound {
                        trait_,
                        modifier,
                        generic_params,
                    } => {
                        let short =
                            trait_.path.rsplit("::").next().unwrap_or(&trait_.path).to_string();
                        let args_str = trait_
                            .args
                            .as_deref()
                            .map(|a| {
                                let s = format_generic_args_with_canon(a, canon);
                                if s.is_empty() { String::new() } else { format!("<{s}>") }
                            })
                            .unwrap_or_default();
                        use rustdoc_types::TraitBoundModifier;
                        let modifier_str = match modifier {
                            TraitBoundModifier::None => "",
                            TraitBoundModifier::Maybe => "?",
                            TraitBoundModifier::MaybeConst => "~const ",
                        };
                        let _ = generic_params; // empty (guarded above)
                        Some(format!("{modifier_str}{short}{args_str}"))
                    }
                    rustdoc_types::GenericBound::Outlives(_)
                    | rustdoc_types::GenericBound::Use(_) => None,
                })
                .collect();
            parts.sort_unstable();
            let rendered = parts.join(" + ");
            if rendered.is_empty() { "impl _".to_string() } else { format!("impl {rendered}") }
        }
        other => format_type_common_arms(
            other,
            canon,
            &mut |t| format_type_with_canon(t, canon),
            &mut |args| format_generic_args_with_canon(args, canon),
        ),
    }
}

/// Formats `GenericArgs` with generic parameter name canonicalization.
///
/// Delegates to [`format_generic_args_impl`] with canon-aware callbacks so that
/// generic-param names are replaced by positional placeholders (`#0`, `#1`, …)
/// and const expressions are canonicalized via `apply_canon_to_str`.
pub(crate) fn format_generic_args_with_canon(
    args: &GenericArgs,
    canon: &HashMap<String, String>,
) -> String {
    format_generic_args_impl(
        args,
        &|arg| {
            Some(match arg {
                GenericArg::Type(t) => format_type_with_canon(t, canon),
                GenericArg::Lifetime(lt) => {
                    canon.get(lt.as_str()).cloned().unwrap_or_else(|| lt.clone())
                }
                GenericArg::Const(c) => apply_canon_to_str(&c.expr.replace("::", "."), canon),
                GenericArg::Infer => "_".to_string(),
            })
        },
        &|t| format_type_with_canon(t, canon),
        &|s| apply_canon_to_str(s, canon),
        &|bounds| format_generic_bounds_with_canon(bounds, canon),
    )
}

/// Counts how many `BorrowedRef::lifetime` values in `types` match any name in
/// `binder_names` (with or without the leading `'`).
///
/// Used to determine whether a single-binder HRTB lifetime appears more than once in
/// the Parenthesized Fn-trait args: if the count is > 1 the binder introduces a
/// shared-lifetime constraint and must be retained rather than dropped.
fn count_binder_lifetime_in_types(types: &[Type], binder_names: &[&str]) -> usize {
    types.iter().map(|t| count_binder_lifetime_in_type(t, binder_names)).sum()
}

fn count_binder_lifetime_in_type(ty: &Type, binder_names: &[&str]) -> usize {
    match ty {
        Type::BorrowedRef { lifetime, type_: inner, .. } => {
            let matched = lifetime.as_deref().is_some_and(|lt| {
                binder_names.iter().any(|name| {
                    *name == lt
                        || name.strip_prefix('\'') == Some(lt)
                        || lt.strip_prefix('\'') == Some(*name)
                })
            });
            (matched as usize) + count_binder_lifetime_in_type(inner, binder_names)
        }
        Type::Slice(inner)
        | Type::Array { type_: inner, .. }
        | Type::RawPointer { type_: inner, .. } => {
            count_binder_lifetime_in_type(inner, binder_names)
        }
        Type::Tuple(tys) => count_binder_lifetime_in_types(tys, binder_names),
        Type::ResolvedPath(path) => {
            if let Some(args) = path.args.as_deref() {
                count_binder_lifetime_in_generic_args(args, binder_names)
            } else {
                0
            }
        }
        _ => 0,
    }
}

fn count_binder_lifetime_in_generic_args(args: &GenericArgs, binder_names: &[&str]) -> usize {
    match args {
        GenericArgs::AngleBracketed { args, .. } => args
            .iter()
            .map(|arg| match arg {
                GenericArg::Type(t) => count_binder_lifetime_in_type(t, binder_names),
                _ => 0,
            })
            .sum(),
        GenericArgs::Parenthesized { inputs, output } => {
            let in_count: usize =
                inputs.iter().map(|t| count_binder_lifetime_in_type(t, binder_names)).sum();
            let out_count =
                output.as_ref().map_or(0, |t| count_binder_lifetime_in_type(t, binder_names));
            in_count + out_count
        }
        _ => 0,
    }
}

/// Canon-aware variant of `format_generic_bounds`. Applies `canon` to inner
/// `Type::Generic` occurrences inside trait-bound generic args.
///
/// D5 scope (ADR 2026-05-18-1223 D5):
/// - `TraitBound { generic_params: empty }` — fully supported.
/// - `TraitBound { generic_params: lifetime-only }` — supported; binder lifetime names
///   are normalized positionally for A/C symmetry.
/// - `TraitBound { generic_params: type-param binders }` — `<UNSUPPORTED:HRTB>`.
/// - `Outlives(lt)` — rendered verbatim.
/// - `Use` — `<UNSUPPORTED:Use>`.
pub(crate) fn format_generic_bounds_with_canon(
    bounds: &[GenericBound],
    canon: &HashMap<String, String>,
) -> String {
    let mut strs: Vec<String> = bounds
        .iter()
        .map(|b| match b {
            GenericBound::TraitBound { trait_, modifier, generic_params } => {
                let has_type_binders = generic_params.iter().any(|hp| {
                    matches!(
                        hp.kind,
                        GenericParamDefKind::Type { .. } | GenericParamDefKind::Const { .. }
                    )
                });
                if has_type_binders {
                    return "<UNSUPPORTED:HRTB>".to_owned();
                }
                let has_outlives_binder = generic_params.iter().any(|hp| match &hp.kind {
                    GenericParamDefKind::Lifetime { outlives } => !outlives.is_empty(),
                    _ => false,
                });
                if has_outlives_binder {
                    return "<UNSUPPORTED:HRTB>".to_owned();
                }
                let binder_lifetimes: Vec<&str> = generic_params
                    .iter()
                    .filter(|hp| matches!(hp.kind, GenericParamDefKind::Lifetime { .. }))
                    .map(|hp| hp.name.as_str())
                    .collect();
                let lt_count = binder_lifetimes.len();
                let parenthesized_parts: Option<(&[Type], Option<&Type>)> =
                    match trait_.args.as_deref() {
                        Some(GenericArgs::Parenthesized { inputs, output }) => {
                            let out: Option<&Type> = match output {
                                Some(b) => Some(b),
                                None => None,
                            };
                            Some((inputs.as_slice(), out))
                        }
                        _ => None,
                    };
                let parenthesized_inputs: Option<&[Type]> =
                    parenthesized_parts.map(|(inputs, _)| inputs);
                let is_parenthesized = parenthesized_inputs.is_some();
                // Fn-desugaring normalization (D5): a binder lifetime that appears exactly
                // once across inputs+output is an independent elided lifetime.  When all
                // binder lifetimes qualify, treat as if no binder (A/C symmetry).
                let fn_desugar = if is_parenthesized && lt_count >= 1 {
                    let (inputs, output) = parenthesized_parts.unwrap_or((&[], None));
                    binder_lifetimes.iter().all(|lt| {
                        let in_count = count_binder_lifetime_in_types(inputs, &[lt]);
                        let out_count =
                            output.map_or(0, |o| count_binder_lifetime_in_type(o, &[lt]));
                        in_count + out_count <= 1
                    })
                } else {
                    false
                };
                let binder_prefix = if fn_desugar || lt_count <= 1 {
                    String::new()
                } else {
                    format!("#L{lt_count}:")
                };
                // Build positional canon map for binder lifetimes.
                // Keys: bare name + apostrophized name (both forms for robustness).
                // `@BR:` sentinel signals HRTB context to `format_type_with_canon`.
                let args_canon: std::borrow::Cow<HashMap<String, String>> = if lt_count >= 1 {
                    let mut merged = canon.clone();
                    for (i, lt_name) in binder_lifetimes.iter().enumerate() {
                        let positional_label = format!("#L{i}");
                        let apostrophe_name: String;
                        let (bare, apostrophized) = if let Some(b) = lt_name.strip_prefix('\'') {
                            apostrophe_name = (*lt_name).to_owned();
                            (b, apostrophe_name.as_str())
                        } else {
                            apostrophe_name = format!("'{lt_name}");
                            (*lt_name, apostrophe_name.as_str())
                        };
                        merged.insert(bare.to_owned(), positional_label.clone());
                        merged.insert(apostrophized.to_owned(), positional_label.clone());
                        let br_label =
                            if fn_desugar { String::new() } else { positional_label.clone() };
                        merged.insert(format!("@BR:{bare}"), br_label.clone());
                        merged.insert(format!("@BR:{apostrophized}"), br_label);
                    }
                    merged.insert("@BR:".to_owned(), String::new());
                    std::borrow::Cow::Owned(merged)
                } else {
                    std::borrow::Cow::Borrowed(canon)
                };
                let short = trait_.path.rsplit("::").next().unwrap_or(&trait_.path).to_string();
                let args_str = trait_
                    .args
                    .as_deref()
                    .map(|a| {
                        let s = format_generic_args_with_canon(a, &args_canon);
                        if s.is_empty() { String::new() } else { format!("<{s}>") }
                    })
                    .unwrap_or_default();
                use rustdoc_types::TraitBoundModifier;
                let modifier_str = match modifier {
                    TraitBoundModifier::None => "",
                    TraitBoundModifier::Maybe => "?",
                    TraitBoundModifier::MaybeConst => "~const ",
                };
                format!("{binder_prefix}{modifier_str}{short}{args_str}")
            }
            GenericBound::Outlives(lt) => lt.clone(),
            GenericBound::Use(_) => "<UNSUPPORTED:Use>".to_owned(),
        })
        .collect();
    strs.sort();
    strs.join("+")
}

// ---------------------------------------------------------------------------
// Tests for `format_generic_bounds_with_canon` HRTB D5 support
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use rustdoc_types::{
        GenericBound, GenericParamDef, GenericParamDefKind, Id, Path, TraitBoundModifier, Type,
    };

    use super::format_generic_bounds_with_canon;

    fn lt_param(name: &str) -> GenericParamDef {
        GenericParamDef {
            name: name.to_owned(),
            kind: GenericParamDefKind::Lifetime { outlives: vec![] },
        }
    }

    // --- HRTB 2-binder arity distinguishes lifetime usage ---

    #[test]
    fn test_hrtb_two_binder_lifetimes_distinct_usage_produces_different_fingerprints() {
        use std::collections::HashMap;

        // for<'a,'b> Fn(&'a str, &'b str)
        let bound_ab = GenericBound::TraitBound {
            trait_: Path {
                path: "Fn".to_owned(),
                id: Id(0),
                args: Some(Box::new(rustdoc_types::GenericArgs::Parenthesized {
                    inputs: vec![
                        Type::BorrowedRef {
                            lifetime: Some("'a".to_owned()),
                            is_mutable: false,
                            type_: Box::new(Type::Primitive("str".to_owned())),
                        },
                        Type::BorrowedRef {
                            lifetime: Some("'b".to_owned()),
                            is_mutable: false,
                            type_: Box::new(Type::Primitive("str".to_owned())),
                        },
                    ],
                    output: None,
                })),
            },
            generic_params: vec![lt_param("'a"), lt_param("'b")],
            modifier: TraitBoundModifier::None,
        };

        // for<'a,'b> Fn(&'a str, &'a str) — same arity but both params use 'a
        let bound_aa = GenericBound::TraitBound {
            trait_: Path {
                path: "Fn".to_owned(),
                id: Id(0),
                args: Some(Box::new(rustdoc_types::GenericArgs::Parenthesized {
                    inputs: vec![
                        Type::BorrowedRef {
                            lifetime: Some("'a".to_owned()),
                            is_mutable: false,
                            type_: Box::new(Type::Primitive("str".to_owned())),
                        },
                        Type::BorrowedRef {
                            lifetime: Some("'a".to_owned()),
                            is_mutable: false,
                            type_: Box::new(Type::Primitive("str".to_owned())),
                        },
                    ],
                    output: None,
                })),
            },
            generic_params: vec![lt_param("'a"), lt_param("'b")],
            modifier: TraitBoundModifier::None,
        };

        let canon: HashMap<String, String> = HashMap::new();
        let fp_ab = format_generic_bounds_with_canon(&[bound_ab], &canon);
        let fp_aa = format_generic_bounds_with_canon(&[bound_aa], &canon);
        assert_ne!(
            fp_ab, fp_aa,
            "D5: `for<'a,'b> Fn(&'a str, &'b str)` and \
             `for<'a,'b> Fn(&'a str, &'a str)` must produce distinct fingerprints \
             to avoid false Blue comparisons; got: fp_ab={fp_ab:?} fp_aa={fp_aa:?}"
        );
    }

    #[test]
    fn test_hrtb_one_binder_different_name_same_fingerprint() {
        use std::collections::HashMap;

        use rustdoc_types::{GenericArg, GenericArgs};

        let make_bound = |binder_name: &str, arg_lt: &str| GenericBound::TraitBound {
            trait_: Path {
                path: "Foo".to_owned(),
                id: Id(0),
                args: Some(Box::new(GenericArgs::AngleBracketed {
                    args: vec![GenericArg::Lifetime(arg_lt.to_owned())],
                    constraints: vec![],
                })),
            },
            generic_params: vec![GenericParamDef {
                name: binder_name.to_owned(),
                kind: GenericParamDefKind::Lifetime { outlives: vec![] },
            }],
            modifier: TraitBoundModifier::None,
        };

        let canon: HashMap<String, String> = HashMap::new();
        let fp_a = format_generic_bounds_with_canon(&[make_bound("'a", "'a")], &canon);
        let fp_b = format_generic_bounds_with_canon(&[make_bound("'b", "'b")], &canon);
        assert_eq!(
            fp_a, fp_b,
            "D5: `for<'a> Foo<'a>` and `for<'b> Foo<'b>` must produce the same fingerprint \
             (binder lifetime name is insignificant); got: fp_a={fp_a:?} fp_b={fp_b:?}"
        );
    }

    #[test]
    fn test_hrtb_two_binder_concrete_lifetime_not_equal_to_elided() {
        use std::collections::HashMap;

        // for<'a,'b> Fn(&'static str)
        let bound_static = GenericBound::TraitBound {
            trait_: Path {
                path: "Fn".to_owned(),
                id: Id(0),
                args: Some(Box::new(rustdoc_types::GenericArgs::Parenthesized {
                    inputs: vec![Type::BorrowedRef {
                        lifetime: Some("'static".to_owned()),
                        is_mutable: false,
                        type_: Box::new(Type::Primitive("str".to_owned())),
                    }],
                    output: None,
                })),
            },
            generic_params: vec![lt_param("'a"), lt_param("'b")],
            modifier: TraitBoundModifier::None,
        };

        // for<'a,'b> Fn(&str) — elided lifetime
        let bound_elided = GenericBound::TraitBound {
            trait_: Path {
                path: "Fn".to_owned(),
                id: Id(0),
                args: Some(Box::new(rustdoc_types::GenericArgs::Parenthesized {
                    inputs: vec![Type::BorrowedRef {
                        lifetime: None,
                        is_mutable: false,
                        type_: Box::new(Type::Primitive("str".to_owned())),
                    }],
                    output: None,
                })),
            },
            generic_params: vec![lt_param("'a"), lt_param("'b")],
            modifier: TraitBoundModifier::None,
        };

        let canon: HashMap<String, String> = HashMap::new();
        let fp_static = format_generic_bounds_with_canon(&[bound_static], &canon);
        let fp_elided = format_generic_bounds_with_canon(&[bound_elided], &canon);
        assert_ne!(
            fp_static, fp_elided,
            "D5: `for<'a,'b> Fn(&'static str)` and `for<'a,'b> Fn(&str)` must produce \
             distinct fingerprints (concrete vs elided lifetime in 2+-binder context); \
             got: fp_static={fp_static:?} fp_elided={fp_elided:?}"
        );
    }
}
