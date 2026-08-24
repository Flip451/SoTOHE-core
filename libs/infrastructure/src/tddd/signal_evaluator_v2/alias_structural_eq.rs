//! Type-alias structural comparison helpers.
//!
//! Alias declarations are compared as a lexical document contract while the
//! surrounding evaluator keeps its established structural shape comparison.

use std::collections::BTreeSet;

use super::alias_lexical::type_alias_lexical_signature;
use super::format::format_type;
use super::target_lifetimes::collect_type_lifetimes;

pub(super) fn type_alias_targets_lexically_equal(
    a: &rustdoc_types::Type,
    b: &rustdoc_types::Type,
) -> bool {
    let (Ok(a_signature), Ok(b_signature)) =
        (type_alias_lexical_signature(a), type_alias_lexical_signature(b))
    else {
        return false;
    };
    let (Ok(mut a_value), Ok(mut b_value)) = (
        serde_json::from_str::<serde_json::Value>(&a_signature),
        serde_json::from_str::<serde_json::Value>(&b_signature),
    ) else {
        return false;
    };
    normalize_alias_target_path_pairs(&mut a_value, &mut b_value);
    a_value == b_value
}

/// Normalizes only the qualified-vs-short path representation difference.
///
/// The caller has already produced the alias lexical signature, so rustdoc
/// IDs and the signature's other deliberate normalizations remain intact.
/// A serialized `Path` is the only target object with both `path` and `args`;
/// this covers resolved paths and trait paths in dyn, impl-trait, qualified,
/// and nested bound forms without changing other target strings.  Qualified
/// paths are shortened only when their counterpart is already short.  Two
/// qualified paths retain their crate/module identity and therefore cannot
/// compare equal merely because their final segments happen to match.
fn normalize_alias_target_path_pairs(a: &mut serde_json::Value, b: &mut serde_json::Value) {
    match (a, b) {
        (serde_json::Value::Array(a_values), serde_json::Value::Array(b_values)) => {
            for (a_value, b_value) in a_values.iter_mut().zip(b_values.iter_mut()) {
                normalize_alias_target_path_pairs(a_value, b_value);
            }
        }
        (serde_json::Value::Object(a_values), serde_json::Value::Object(b_values)) => {
            let a_path = a_values.get("path").and_then(serde_json::Value::as_str);
            let b_path = b_values.get("path").and_then(serde_json::Value::as_str);
            if a_values.contains_key("args")
                && b_values.contains_key("args")
                && let (Some(a_path), Some(b_path)) = (a_path, b_path)
                && a_path.contains("::") != b_path.contains("::")
            {
                let qualified_path = if a_path.contains("::") { a_path } else { b_path };
                if let Some(short_name) = qualified_path.rsplit("::").next() {
                    let short_name = serde_json::Value::String(short_name.to_owned());
                    if a_path.contains("::") {
                        a_values.insert("path".to_owned(), short_name);
                    } else {
                        b_values.insert("path".to_owned(), short_name);
                    }
                }
            }

            let keys: Vec<String> = a_values.keys().cloned().collect();
            for key in keys {
                if let (Some(a_value), Some(b_value)) =
                    (a_values.get_mut(&key), b_values.get_mut(&key))
                {
                    normalize_alias_target_path_pairs(a_value, b_value);
                }
            }
        }
        _ => {}
    }
}

/// Compares type-alias generics as the catalogue declares them.
///
/// Function, trait, and impl generics use the name-independent where-form
/// comparison because their bindings are structural. Alias declarations are a
/// document-level contract instead: parameter names, parameter order, and
/// bound order are all observable. The catalogue encoder places inline bounds
/// into `where_predicates`, while rustdoc may retain them on the parameter, so
/// this helper preserves their order while accepting those equivalent storage
/// locations.
pub(super) fn type_alias_generics_lexically_equal(
    a: &rustdoc_types::Generics,
    a_target: &rustdoc_types::Type,
    b: &rustdoc_types::Generics,
    b_target: &rustdoc_types::Type,
) -> bool {
    // The catalogue schema cannot declare lifetime parameters: an alias whose
    // source declares them records the lifetimes lexically in the target
    // (accepted target lifetime policy). A lifetime parameter is therefore
    // excluded from this comparison ONLY when its name appears in that side's
    // target — a lifetime parameter the target does not carry (an unused
    // declaration) is unrecorded information and stays a mismatch.
    let a_params = comparable_alias_params(a, a_target);
    let b_params = comparable_alias_params(b, b_target);
    if a_params.len() != b_params.len()
        || !a_params
            .iter()
            .zip(&b_params)
            .all(|(left, right)| type_alias_param_lexically_equal(left, right))
    {
        return false;
    }

    let parameter_names: BTreeSet<&str> =
        a_params.iter().map(|param| param.name.as_str()).collect();
    for (left, right) in a_params.iter().zip(&b_params) {
        let (Ok(left_bounds), Ok(right_bounds)) = (
            type_alias_bounds_for_parameter(a, &left.name),
            type_alias_bounds_for_parameter(b, &right.name),
        ) else {
            return false;
        };
        if left_bounds != right_bounds {
            return false;
        }
    }

    matches!(
        (
            type_alias_non_parameter_predicates(a, &parameter_names),
            type_alias_non_parameter_predicates(b, &parameter_names),
        ),
        (Ok(left), Ok(right)) if left == right
    )
}

/// The alias generic parameters that participate in the lexical comparison.
///
/// A lifetime parameter is excluded only when the alias TARGET carries its
/// name (the catalogue schema cannot declare lifetime parameters, so a
/// source-declared lifetime is recorded lexically in the target). A lifetime
/// parameter the target does not mention — an unused declaration — stays in
/// the list and therefore surfaces as a mismatch against a side that does not
/// declare it.
fn comparable_alias_params<'generics>(
    generics: &'generics rustdoc_types::Generics,
    target: &rustdoc_types::Type,
) -> Vec<&'generics rustdoc_types::GenericParamDef> {
    let mut target_lifetimes: BTreeSet<String> = BTreeSet::new();
    collect_type_lifetimes(target, &mut target_lifetimes);
    generics
        .params
        .iter()
        .filter(|param| match &param.kind {
            // Only the plain `<'a>` declaration is representable by the
            // lexical target convention: a lifetime parameter carrying
            // outlives metadata (`<'a: 'static>`) holds unrecorded
            // declaration information and stays in the comparison.
            rustdoc_types::GenericParamDefKind::Lifetime { outlives } => {
                !(outlives.is_empty() && target_lifetimes.contains(&param.name))
            }
            _ => true,
        })
        .collect()
}

fn type_alias_param_lexically_equal(
    a: &rustdoc_types::GenericParamDef,
    b: &rustdoc_types::GenericParamDef,
) -> bool {
    use rustdoc_types::GenericParamDefKind;

    if a.name != b.name {
        return false;
    }
    match (&a.kind, &b.kind) {
        (
            GenericParamDefKind::Type { default: a_default, is_synthetic: a_synthetic, .. },
            GenericParamDefKind::Type { default: b_default, is_synthetic: b_synthetic, .. },
        ) => {
            a_synthetic == b_synthetic
                && a_default.as_ref().map(format_type) == b_default.as_ref().map(format_type)
        }
        (
            GenericParamDefKind::Const { type_: a_type, default: a_default },
            GenericParamDefKind::Const { type_: b_type, default: b_default },
        ) => format_type(a_type) == format_type(b_type) && a_default == b_default,
        (
            GenericParamDefKind::Lifetime { outlives: a_outlives },
            GenericParamDefKind::Lifetime { outlives: b_outlives },
        ) => a_outlives == b_outlives,
        _ => false,
    }
}

fn type_alias_bounds_for_parameter(
    generics: &rustdoc_types::Generics,
    name: &str,
) -> Result<Vec<String>, serde_json::Error> {
    use rustdoc_types::{GenericBound, GenericParamDef, GenericParamDefKind, Type, WherePredicate};

    // Each entry's signature pairs the bound with the predicate-level HRTB
    // binder (`where for<'a> T: Clone` records `'a` in
    // `BoundPredicate.generic_params`), so a binder difference is a lexical
    // mismatch. An inline parameter bound has no predicate binder and pairs
    // with the empty list, keeping the accepted inline-vs-where storage
    // equivalence intact.
    fn binder_scoped_signature(
        binder: &[GenericParamDef],
        bound: &GenericBound,
    ) -> Result<String, serde_json::Error> {
        type_alias_lexical_signature(&(binder, bound))
    }

    let inline_bounds = generics
        .params
        .iter()
        .find(|param| param.name == name)
        .and_then(|param| match &param.kind {
            GenericParamDefKind::Type { bounds, .. } => Some(bounds),
            GenericParamDefKind::Lifetime { .. } | GenericParamDefKind::Const { .. } => None,
        })
        .into_iter()
        .flatten()
        .map(|bound| binder_scoped_signature(&[], bound));
    let where_bounds = generics.where_predicates.iter().filter_map(|predicate| match predicate {
        WherePredicate::BoundPredicate {
            type_: Type::Generic(predicate_name),
            bounds,
            generic_params,
        } if predicate_name == name => {
            Some(bounds.iter().map(|bound| binder_scoped_signature(generic_params, bound)))
        }
        _ => None,
    });

    inline_bounds.chain(where_bounds.flatten()).collect()
}

fn type_alias_non_parameter_predicates(
    generics: &rustdoc_types::Generics,
    parameter_names: &BTreeSet<&str>,
) -> Result<Vec<String>, serde_json::Error> {
    use rustdoc_types::{Type, WherePredicate};

    generics
        .where_predicates
        .iter()
        .filter(|predicate| {
            !matches!(
                predicate,
                WherePredicate::BoundPredicate { type_: Type::Generic(name), .. }
                    if parameter_names.contains(name.as_str())
            )
        })
        .map(type_alias_lexical_signature)
        .collect()
}
