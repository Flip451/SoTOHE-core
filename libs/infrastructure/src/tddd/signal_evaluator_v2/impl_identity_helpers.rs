//! Private type-identity rendering helpers for impl identity construction.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use domain::tddd::Phase1Error;
use domain::tddd::catalogue_v2::identifiers::CrateName;
use rustdoc_types::{
    AssocItemConstraint, AssocItemConstraintKind, GenericArg, GenericArgs, GenericBound,
    GenericParamDefKind, Generics, Id, Item, ItemEnum, ItemSummary, Path, Term, Type,
};

use crate::tddd::canonical_type_identity::{DefinitionPathAuthority, canonicalize_rustdoc_path};
use crate::tddd::type_ref_parser::{render_bound, render_type};

use super::format::format_type;

pub(super) fn strip_impl_params_type(ty: Type, impl_params: &BTreeSet<String>) -> Type {
    match ty {
        Type::ResolvedPath(path) => Type::ResolvedPath(strip_impl_params_path(path, impl_params)),
        Type::Generic(name) if is_impl_param(&name, impl_params) => Type::Generic("_".to_owned()),
        Type::BorrowedRef { lifetime, is_mutable, type_ } => Type::BorrowedRef {
            lifetime: lifetime.filter(|name| !is_impl_param(name, impl_params)),
            is_mutable,
            type_: Box::new(strip_impl_params_type(*type_, impl_params)),
        },
        Type::Slice(inner) => Type::Slice(Box::new(strip_impl_params_type(*inner, impl_params))),
        Type::Array { type_, len } => Type::Array {
            type_: Box::new(strip_impl_params_type(*type_, impl_params)),
            len: if is_impl_param(&len.replace("::", "."), impl_params) {
                "_".to_owned()
            } else {
                len
            },
        },
        Type::Tuple(types) => Type::Tuple(
            types.into_iter().map(|ty| strip_impl_params_type(ty, impl_params)).collect(),
        ),
        Type::RawPointer { is_mutable, type_ } => Type::RawPointer {
            is_mutable,
            type_: Box::new(strip_impl_params_type(*type_, impl_params)),
        },
        Type::ImplTrait(bounds) => Type::ImplTrait(
            bounds.into_iter().map(|bound| strip_impl_params_bound(bound, impl_params)).collect(),
        ),
        Type::DynTrait(mut dyn_trait) => {
            dyn_trait.traits = dyn_trait
                .traits
                .into_iter()
                .map(|mut poly_trait| {
                    poly_trait.trait_ = strip_impl_params_path(poly_trait.trait_, impl_params);
                    poly_trait
                })
                .collect();
            dyn_trait.lifetime =
                dyn_trait.lifetime.filter(|name| !is_impl_param(name, impl_params));
            Type::DynTrait(dyn_trait)
        }
        Type::FunctionPointer(mut function_pointer) => {
            function_pointer.sig.inputs = function_pointer
                .sig
                .inputs
                .into_iter()
                .map(|(name, ty)| (name, strip_impl_params_type(ty, impl_params)))
                .collect();
            function_pointer.sig.output =
                function_pointer.sig.output.map(|ty| strip_impl_params_type(ty, impl_params));
            Type::FunctionPointer(function_pointer)
        }
        Type::QualifiedPath { name, args, self_type, trait_ } => Type::QualifiedPath {
            name,
            args: args.map(|args| Box::new(strip_impl_params_args(*args, impl_params))),
            self_type: Box::new(strip_impl_params_type(*self_type, impl_params)),
            trait_: trait_.map(|path| strip_impl_params_path(path, impl_params)),
        },
        Type::Pat { type_, __pat_unstable_do_not_use } => Type::Pat {
            type_: Box::new(strip_impl_params_type(*type_, impl_params)),
            __pat_unstable_do_not_use,
        },
        other => other,
    }
}

fn is_impl_param(name: &str, impl_params: &BTreeSet<String>) -> bool {
    impl_params.contains(name)
        || impl_params.contains(name.strip_prefix('\'').unwrap_or(name))
        || impl_params.iter().any(|param| param.strip_prefix('\'').unwrap_or(param) == name)
}

fn strip_impl_params_path(mut path: Path, impl_params: &BTreeSet<String>) -> Path {
    path.args = path.args.and_then(|args| {
        let stripped = strip_impl_params_args(*args, impl_params);
        match stripped {
            GenericArgs::AngleBracketed { args, constraints }
                if args.is_empty() && constraints.is_empty() =>
            {
                None
            }
            stripped => Some(Box::new(stripped)),
        }
    });
    path
}

fn strip_impl_params_bound(bound: GenericBound, impl_params: &BTreeSet<String>) -> GenericBound {
    match bound {
        GenericBound::TraitBound { trait_, modifier, generic_params } => GenericBound::TraitBound {
            trait_: strip_impl_params_path(trait_, impl_params),
            modifier,
            generic_params,
        },
        other => other,
    }
}

pub(super) fn strip_impl_params_args(
    args: GenericArgs,
    impl_params: &BTreeSet<String>,
) -> GenericArgs {
    match args {
        GenericArgs::AngleBracketed { args, constraints } => GenericArgs::AngleBracketed {
            args: args
                .into_iter()
                .filter_map(|arg| strip_impl_params_arg(arg, impl_params))
                .collect(),
            constraints: constraints
                .into_iter()
                .map(|constraint| strip_impl_params_constraint(constraint, impl_params))
                .collect(),
        },
        GenericArgs::Parenthesized { inputs, output } => GenericArgs::Parenthesized {
            inputs: inputs.into_iter().map(|ty| strip_impl_params_type(ty, impl_params)).collect(),
            output: output.map(|ty| strip_impl_params_type(ty, impl_params)),
        },
        GenericArgs::ReturnTypeNotation => GenericArgs::ReturnTypeNotation,
    }
}

fn strip_impl_params_arg(arg: GenericArg, impl_params: &BTreeSet<String>) -> Option<GenericArg> {
    match arg {
        GenericArg::Type(Type::Generic(name)) if is_impl_param(&name, impl_params) => None,
        GenericArg::Type(ty) => Some(GenericArg::Type(strip_impl_params_type(ty, impl_params))),
        GenericArg::Lifetime(name) if is_impl_param(&name, impl_params) => None,
        GenericArg::Lifetime(name) => Some(GenericArg::Lifetime(name)),
        GenericArg::Const(mut value) => {
            if is_impl_param(&value.expr.replace("::", "."), impl_params) {
                value.expr = "_".to_owned();
            }
            Some(GenericArg::Const(value))
        }
        GenericArg::Infer => Some(GenericArg::Infer),
    }
}

fn strip_impl_params_constraint(
    mut constraint: AssocItemConstraint,
    impl_params: &BTreeSet<String>,
) -> AssocItemConstraint {
    constraint.args =
        constraint.args.map(|args| Box::new(strip_impl_params_args(*args, impl_params)));
    constraint.binding = match constraint.binding {
        AssocItemConstraintKind::Equality(Term::Type(ty)) => {
            AssocItemConstraintKind::Equality(Term::Type(strip_impl_params_type(ty, impl_params)))
        }
        AssocItemConstraintKind::Equality(Term::Constant(mut value)) => {
            if is_impl_param(&value.expr.replace("::", "."), impl_params) {
                value.expr = "_".to_owned();
            }
            AssocItemConstraintKind::Equality(Term::Constant(value))
        }
        AssocItemConstraintKind::Constraint(bounds) => AssocItemConstraintKind::Constraint(
            bounds.into_iter().map(|bound| strip_impl_params_bound(bound, impl_params)).collect(),
        ),
    };
    constraint
}

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
                let Some(id) = values
                    .get("id")
                    .and_then(serde_json::Value::as_u64)
                    .and_then(|id| u32::try_from(id).ok())
                else {
                    return false;
                };
                let path = Path { path: raw_path.clone(), id: Id(id), args: None };
                let Ok(identity) = canonicalize_rustdoc_path(&path, crate_name, paths, authority)
                else {
                    return false;
                };
                identities.entry(context.to_owned()).or_default().push(identity);
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
