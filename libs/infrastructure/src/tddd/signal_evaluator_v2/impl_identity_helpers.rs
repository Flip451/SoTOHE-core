//! Private type-identity rendering helpers for impl identity construction.

use std::collections::{BTreeMap, HashMap};

use domain::tddd::Phase1Error;
use domain::tddd::catalogue_v2::identifiers::CrateName;
use rustdoc_types::{
    AssocItemConstraintKind, GenericArg, GenericArgs, GenericBound, GenericParamDefKind, Generics,
    Id, Item, ItemEnum, ItemSummary, Path, Term, Type,
};

use crate::tddd::canonical_type_identity::{DefinitionPathAuthority, canonicalize_rustdoc_path};
use crate::tddd::type_ref_parser::{render_bound, render_type};

use super::format::format_type;

pub(super) fn render_identity_generic_args(args: &GenericArgs) -> Result<String, Phase1Error> {
    match args {
        GenericArgs::AngleBracketed { args, constraints } => {
            let mut parts = args
                .iter()
                .map(|arg| match arg {
                    GenericArg::Type(ty) => render_type(ty),
                    GenericArg::Lifetime(name) => Some(name.clone()),
                    GenericArg::Const(value) => Some(value.expr.replace("::", ".")),
                    GenericArg::Infer => Some("_".to_owned()),
                })
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| {
                    Phase1Error::rustdoc_root_resolution(
                        "trait impl generic identity contains an unsupported type argument",
                    )
                })?;
            for constraint in constraints {
                let constraint_args = constraint
                    .args
                    .as_deref()
                    .map(render_identity_generic_args)
                    .transpose()?
                    .filter(|rendered| !rendered.is_empty())
                    .map_or_else(String::new, |rendered| format!("<{rendered}>"));
                let binding = match &constraint.binding {
                    AssocItemConstraintKind::Equality(Term::Type(ty)) => format!(
                        " = {}",
                        render_type(ty).ok_or_else(|| {
                            Phase1Error::rustdoc_root_resolution(
                                "trait impl associated-type constraint has no authoritative rendering",
                            )
                        })?
                    ),
                    AssocItemConstraintKind::Equality(Term::Constant(value)) => {
                        format!(" = {}", value.expr.replace("::", "."))
                    }
                    AssocItemConstraintKind::Constraint(bounds) => {
                        let rendered = bounds
                            .iter()
                            .map(|bound| {
                                render_bound(bound).ok_or_else(|| {
                                    Phase1Error::rustdoc_root_resolution(
                                        "trait impl associated-type bound has no authoritative rendering",
                                    )
                                })
                            })
                            .collect::<Result<Vec<_>, _>>()?;
                        format!(": {}", rendered.join(" + "))
                    }
                };
                parts.push(format!("{}{constraint_args}{binding}", constraint.name));
            }
            Ok(parts.join(", "))
        }
        GenericArgs::Parenthesized { inputs, output } => {
            let inputs = inputs
                .iter()
                .map(|ty| {
                    render_type(ty).ok_or_else(|| {
                        Phase1Error::rustdoc_root_resolution(
                            "trait impl parenthesized argument has no authoritative rendering",
                        )
                    })
                })
                .collect::<Result<Vec<_>, _>>()?
                .join(",");
            let output = match output {
                Some(ty) => render_type(ty).ok_or_else(|| {
                    Phase1Error::rustdoc_root_resolution(
                        "trait impl parenthesized output has no authoritative rendering",
                    )
                })?,
                None => "()".to_owned(),
            };
            Ok(format!("({inputs})->{output}"))
        }
        GenericArgs::ReturnTypeNotation => Ok(String::new()),
    }
}

/// Selects the representation whose paths participate in the corresponding
/// structural comparison. Inherent impl grouping is normalized by the merged
/// method map, so an inherent impl contributes only its method children.
pub(super) fn path_identity_value(item: &Item) -> Option<serde_json::Value> {
    match &item.inner {
        ItemEnum::Impl(implementation) if implementation.trait_.is_some() => {
            serde_json::to_value((&implementation.for_, &implementation.trait_)).ok()
        }
        ItemEnum::Impl(_) => Some(serde_json::Value::Null),
        ItemEnum::Struct(_) | ItemEnum::Enum(_) | ItemEnum::Trait(_) => {
            Some(serde_json::Value::Null)
        }
        ItemEnum::Function(function) => serde_json::to_value(&function.sig).ok(),
        ItemEnum::TypeAlias(alias) => serde_json::to_value(&alias.type_).ok(),
        _ => serde_json::to_value(&item.inner).ok(),
    }
}

/// Collects generic paths after applying the same representation-independent
/// grouping used by `generics_structurally_equal`: inline and where-form
/// bounds for one parameter share a context, lifetime and synthetic parameters
/// do not create identity slots, and each bound set is compared as unordered.
pub(super) fn collect_normalized_generic_paths(
    item: &Item,
    context: &str,
    paths: &HashMap<Id, ItemSummary>,
    crate_name: &CrateName,
    authority: &DefinitionPathAuthority,
    identities: &mut BTreeMap<String, Vec<String>>,
) -> bool {
    let generics = match &item.inner {
        ItemEnum::Struct(structure) => Some(&structure.generics),
        ItemEnum::Enum(enumeration) => Some(&enumeration.generics),
        ItemEnum::Trait(trait_) => Some(&trait_.generics),
        ItemEnum::Function(function) => Some(&function.generics),
        ItemEnum::TypeAlias(alias) => Some(&alias.generics),
        ItemEnum::Impl(_) => None,
        _ => None,
    };
    let Some(generics) = generics else {
        if let ItemEnum::Trait(trait_) = &item.inner {
            return collect_trait_bound_paths(
                &trait_.bounds,
                context,
                paths,
                crate_name,
                authority,
                identities,
            );
        }
        return true;
    };

    collect_generics_paths(generics, context, paths, crate_name, authority, identities)
        && if let ItemEnum::Trait(trait_) = &item.inner {
            collect_trait_bound_paths(
                &trait_.bounds,
                context,
                paths,
                crate_name,
                authority,
                identities,
            )
        } else {
            true
        }
}

fn collect_generics_paths(
    generics: &Generics,
    context: &str,
    paths: &HashMap<Id, ItemSummary>,
    crate_name: &CrateName,
    authority: &DefinitionPathAuthority,
    identities: &mut BTreeMap<String, Vec<String>>,
) -> bool {
    let mut parameter_slots = HashMap::new();
    let mut slot = 0usize;
    let mut synthetic_slot = 0usize;
    for parameter in &generics.params {
        match &parameter.kind {
            GenericParamDefKind::Type { bounds, default, is_synthetic } => {
                if *is_synthetic {
                    let parameter_context =
                        format!("{context}.impl_trait[{synthetic_slot}].bounds");
                    if !collect_bound_paths(
                        bounds,
                        &parameter_context,
                        paths,
                        crate_name,
                        authority,
                        identities,
                    ) {
                        return false;
                    }
                    synthetic_slot += 1;
                    continue;
                }
                parameter_slots.insert(parameter.name.clone(), slot);
                let parameter_context = format!("{context}/generic_param[{slot}]");
                if let Some(default) = default {
                    let Ok(value) = serde_json::to_value(default) else {
                        return false;
                    };
                    if !collect_path_identities(
                        &value,
                        paths,
                        crate_name,
                        authority,
                        &format!("{parameter_context}.default"),
                        identities,
                    ) {
                        return false;
                    }
                }
                if !collect_bound_paths(
                    bounds,
                    &format!("{parameter_context}.bounds"),
                    paths,
                    crate_name,
                    authority,
                    identities,
                ) {
                    return false;
                }
                slot += 1;
            }
            GenericParamDefKind::Const { type_, .. } => {
                parameter_slots.insert(parameter.name.clone(), slot);
                let parameter_context = format!("{context}/generic_param[{slot}]");
                let Ok(value) = serde_json::to_value(type_) else {
                    return false;
                };
                if !collect_path_identities(
                    &value,
                    paths,
                    crate_name,
                    authority,
                    &format!("{parameter_context}.const_type"),
                    identities,
                ) {
                    return false;
                }
                slot += 1;
            }
            GenericParamDefKind::Lifetime { .. } => {}
        }
    }

    for predicate in &generics.where_predicates {
        let rustdoc_types::WherePredicate::BoundPredicate { type_, bounds, .. } = predicate else {
            continue;
        };
        let predicate_context = match type_ {
            Type::Generic(name) => parameter_slots.get(name).map_or_else(
                || format!("{context}/where_lhs:{name}"),
                |slot| format!("{context}/generic_param[{slot}]"),
            ),
            other => format!("{context}/where_lhs:{}", format_type(other)),
        };
        let Ok(value) = serde_json::to_value(type_) else {
            return false;
        };
        if !collect_path_identities(
            &value,
            paths,
            crate_name,
            authority,
            &format!("{predicate_context}.where_lhs"),
            identities,
        ) || !collect_bound_paths(
            bounds,
            &format!("{predicate_context}.bounds"),
            paths,
            crate_name,
            authority,
            identities,
        ) {
            return false;
        }
    }
    true
}

fn collect_bound_paths(
    bounds: &[GenericBound],
    context: &str,
    paths: &HashMap<Id, ItemSummary>,
    crate_name: &CrateName,
    authority: &DefinitionPathAuthority,
    identities: &mut BTreeMap<String, Vec<String>>,
) -> bool {
    bounds.iter().all(|bound| {
        let Ok(value) = serde_json::to_value(bound) else {
            return false;
        };
        collect_path_identities(&value, paths, crate_name, authority, context, identities)
    })
}

fn collect_trait_bound_paths(
    bounds: &[GenericBound],
    context: &str,
    paths: &HashMap<Id, ItemSummary>,
    crate_name: &CrateName,
    authority: &DefinitionPathAuthority,
    identities: &mut BTreeMap<String, Vec<String>>,
) -> bool {
    collect_bound_paths(
        bounds,
        &format!("{context}.supertrait.bounds"),
        paths,
        crate_name,
        authority,
        identities,
    )
}

pub(super) fn path_identity_sequences_match(
    left: &BTreeMap<String, Vec<String>>,
    right: &BTreeMap<String, Vec<String>>,
) -> bool {
    left.len() == right.len()
        && left.iter().all(|(context, left_values)| {
            let Some(right_values) = right.get(context) else {
                return false;
            };
            let mut left_values = left_values.clone();
            let mut right_values = right_values.clone();
            left_values.sort_unstable();
            right_values.sort_unstable();
            left_values == right_values
        })
}

pub(super) fn collect_path_identities(
    value: &serde_json::Value,
    paths: &HashMap<Id, ItemSummary>,
    crate_name: &CrateName,
    authority: &DefinitionPathAuthority,
    context: &str,
    identities: &mut BTreeMap<String, Vec<String>>,
) -> bool {
    let mut impl_trait_occurrence = 0;
    collect_path_identities_with_context(
        value,
        paths,
        crate_name,
        authority,
        context,
        context,
        &mut impl_trait_occurrence,
        identities,
    )
}

#[allow(clippy::too_many_arguments)]
fn collect_path_identities_with_context(
    value: &serde_json::Value,
    paths: &HashMap<Id, ItemSummary>,
    crate_name: &CrateName,
    authority: &DefinitionPathAuthority,
    context: &str,
    root_context: &str,
    impl_trait_occurrence: &mut usize,
    identities: &mut BTreeMap<String, Vec<String>>,
) -> bool {
    match value {
        serde_json::Value::Array(values) => {
            for (index, value) in values.iter().enumerate() {
                let child_context = if unordered_path_array(context) {
                    context.to_owned()
                } else {
                    format!("{context}[{index}]")
                };
                if !collect_path_identities_with_context(
                    value,
                    paths,
                    crate_name,
                    authority,
                    &child_context,
                    root_context,
                    impl_trait_occurrence,
                    identities,
                ) {
                    return false;
                }
            }
            true
        }
        serde_json::Value::Object(values) => {
            if let Some(bounds) = values.get("impl_trait") {
                let occurrence = *impl_trait_occurrence;
                *impl_trait_occurrence += 1;
                let bounds_context = format!("{root_context}.impl_trait[{occurrence}].bounds");
                return collect_path_identities_with_context(
                    bounds,
                    paths,
                    crate_name,
                    authority,
                    &bounds_context,
                    root_context,
                    impl_trait_occurrence,
                    identities,
                );
            }
            if values.contains_key("path") {
                let Some(serde_json::Value::String(raw_path)) = values.get("path") else {
                    return false;
                };
                // Rustdoc uses an empty path for the unnamed trait in a
                // QualifiedPath self projection such as <Self>::Input. It is
                // a projection marker, not a named definition to resolve
                // through the catalogue identity universe.
                // `Self` is a local type marker, not a definition identity. The
                // catalogue codec represents a self receiver as a resolved path
                // while rustdoc represents it as `Type::Generic("Self")`; recording
                // only the former would make equivalent signatures look different.
                if !raw_path.is_empty() && raw_path != "Self" && raw_path != "::Self" {
                    let Some(id) = values
                        .get("id")
                        .and_then(serde_json::Value::as_u64)
                        .and_then(|id| u32::try_from(id).ok())
                    else {
                        return false;
                    };
                    let path = Path { path: raw_path.clone(), id: Id(id), args: None };
                    let Ok(identity) =
                        canonicalize_rustdoc_path(&path, crate_name, paths, authority)
                    else {
                        return false;
                    };
                    identities.entry(context.to_owned()).or_default().push(identity);
                }
            }
            for (key, value) in values {
                if !collect_path_identities_with_context(
                    value,
                    paths,
                    crate_name,
                    authority,
                    &format!("{context}.{key}"),
                    root_context,
                    impl_trait_occurrence,
                    identities,
                ) {
                    return false;
                }
            }
            true
        }
        serde_json::Value::Null
        | serde_json::Value::Bool(_)
        | serde_json::Value::Number(_)
        | serde_json::Value::String(_) => true,
    }
}

fn unordered_path_array(context: &str) -> bool {
    context.ends_with(".bounds")
        || context.ends_with(".where_predicates")
        || context.ends_with(".constraints")
        || context.ends_with(".traits")
        || context.ends_with(".Plain.fields")
        || context.ends_with(".Struct.fields")
}
