//! Type-reference collection helpers for Phase 1.5 and Phase 1.6.
//!
//! Provides:
//! - Unresolved-marker scanning (`item_has_unresolved_marker`, `type_has_unresolved_marker`)
//! - Type-reference collection (`collect_type_refs_from_item`, `collect_type_refs_from_generics`)
//! - Dangling-Id collection (`collect_referenced_ids`, `collect_ids_from_type`)
//!
//! These helpers are read-only traversals of `rustdoc_types` trees; they do not
//! rewrite any Ids.  Actual Id resolution lives in `resolve_type`.

use rustdoc_types::{
    AssocItemConstraintKind, GenericArg, GenericArgs, GenericBound, GenericParamDefKind, Generics,
    Id, Item, ItemEnum, Term, Type, WherePredicate,
};

use crate::tddd::type_ref_parser::UNRESOLVED_CRATE_ID;

use super::is_local_unresolved_path;

// ---------------------------------------------------------------------------
// Unresolved-marker scanning
// ---------------------------------------------------------------------------

/// Returns `true` if any `Type` field in the item references an unresolved marker
/// (`Id(UNRESOLVED_CRATE_ID)` as a local-style Id in `ResolvedPath`).
pub(super) fn item_has_unresolved_marker(item: &Item) -> bool {
    collect_type_refs_from_item(item).iter().any(type_has_unresolved_marker)
}

/// Returns `true` if a `Type` tree contains an unresolved marker.
pub(super) fn type_has_unresolved_marker(ty: &Type) -> bool {
    match ty {
        Type::ResolvedPath(p) => {
            if p.id == Id(UNRESOLVED_CRATE_ID) {
                // Only count as unresolved if it looks like a local or relative name.
                if is_local_unresolved_path(&p.path) {
                    return true;
                }
            }
            if let Some(args) = &p.args { args_have_unresolved_marker(args) } else { false }
        }
        Type::BorrowedRef { type_: inner, .. } => type_has_unresolved_marker(inner),
        Type::Slice(inner) => type_has_unresolved_marker(inner),
        Type::Array { type_: inner, .. } => type_has_unresolved_marker(inner),
        Type::Tuple(tys) => tys.iter().any(type_has_unresolved_marker),
        Type::RawPointer { type_: inner, .. } => type_has_unresolved_marker(inner),
        Type::FunctionPointer(fp) => {
            fp.sig.inputs.iter().any(|(_, t)| type_has_unresolved_marker(t))
                || fp.sig.output.as_ref().is_some_and(type_has_unresolved_marker)
                || hrtb_params_have_unresolved_marker(&fp.generic_params)
        }
        Type::QualifiedPath { self_type, trait_, args, .. } => {
            type_has_unresolved_marker(self_type)
                // Check the qualified-path's own generic args (e.g. `<T as Trait<U>>::Assoc<V>`
                // — the `<V>` part is in `args`, distinct from `trait_.args`).
                || args.as_deref().is_some_and(args_have_unresolved_marker)
                || trait_.as_ref().is_some_and(|p| {
                    // Check the trait Id itself for an unresolved marker.
                    (p.id == Id(UNRESOLVED_CRATE_ID) && is_local_unresolved_path(&p.path))
                        || p.args.as_deref().is_some_and(args_have_unresolved_marker)
                })
        }
        Type::ImplTrait(bounds) => bounds.iter().any(|b| match b {
            GenericBound::TraitBound { trait_, generic_params, .. } => {
                (trait_.id == Id(UNRESOLVED_CRATE_ID) && is_local_unresolved_path(&trait_.path))
                    || trait_.args.as_deref().is_some_and(args_have_unresolved_marker)
                    || hrtb_params_have_unresolved_marker(generic_params)
            }
            _ => false,
        }),
        Type::DynTrait(dyn_trait) => dyn_trait.traits.iter().any(|pt| {
            let p = &pt.trait_;
            (p.id == Id(UNRESOLVED_CRATE_ID) && is_local_unresolved_path(&p.path))
                || p.args.as_deref().is_some_and(args_have_unresolved_marker)
                || hrtb_params_have_unresolved_marker(&pt.generic_params)
        }),
        // Pattern types (RFC 3437): recurse into the underlying base type.
        Type::Pat { type_: inner, .. } => type_has_unresolved_marker(inner),
        _ => false,
    }
}

/// Returns `true` if any HRTB type-param binder in `generic_params` carries an
/// unresolved marker.  Recursively checks nested binders so that deeply nested
/// `for<T: for<U: LocalTrait>>` constructs are also detected.
fn hrtb_params_have_unresolved_marker(generic_params: &[rustdoc_types::GenericParamDef]) -> bool {
    generic_params.iter().any(|p| match &p.kind {
        GenericParamDefKind::Type { bounds, default, .. } => {
            bounds.iter().any(|b| match b {
                GenericBound::TraitBound { trait_, generic_params: nested, .. } => {
                    (trait_.id == Id(UNRESOLVED_CRATE_ID) && is_local_unresolved_path(&trait_.path))
                        || trait_.args.as_deref().is_some_and(args_have_unresolved_marker)
                        || hrtb_params_have_unresolved_marker(nested)
                }
                _ => false,
            }) || default.as_ref().is_some_and(type_has_unresolved_marker)
        }
        _ => false,
    })
}

fn args_have_unresolved_marker(args: &GenericArgs) -> bool {
    match args {
        GenericArgs::AngleBracketed { args, constraints } => {
            args.iter().any(|arg| match arg {
                GenericArg::Type(ty) => type_has_unresolved_marker(ty),
                _ => false,
            }) || constraints.iter().any(|c| match &c.binding {
                AssocItemConstraintKind::Equality(Term::Type(ty)) => type_has_unresolved_marker(ty),
                AssocItemConstraintKind::Constraint(bounds) => bounds.iter().any(|b| match b {
                    GenericBound::TraitBound { trait_, generic_params, .. } => {
                        (trait_.id == Id(UNRESOLVED_CRATE_ID)
                            && is_local_unresolved_path(&trait_.path))
                            || trait_.args.as_deref().is_some_and(args_have_unresolved_marker)
                            // Also check nested HRTB binders on constraint bounds.
                            || hrtb_params_have_unresolved_marker(generic_params)
                    }
                    _ => false,
                }),
                _ => false,
            })
        }
        GenericArgs::Parenthesized { inputs, output } => {
            inputs.iter().any(type_has_unresolved_marker)
                || output.as_ref().is_some_and(type_has_unresolved_marker)
        }
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Type-reference collection
// ---------------------------------------------------------------------------

/// Collects `Type` references from HRTB type-param binder params for `collect_type_refs_from_generics`.
///
/// Recursively handles nested binders so that deeply nested `for<T: for<U: LocalTrait>>`
/// constructs are fully scanned.
fn collect_type_refs_from_hrtb_params(
    generic_params: &[rustdoc_types::GenericParamDef],
    types: &mut Vec<Type>,
) {
    for hrtb_param in generic_params {
        if let GenericParamDefKind::Type { bounds: hrtb_bounds, default: hrtb_default, .. } =
            &hrtb_param.kind
        {
            for hrtb_bound in hrtb_bounds {
                if let GenericBound::TraitBound { trait_: ht, generic_params: nested, .. } =
                    hrtb_bound
                {
                    types.push(Type::ResolvedPath(ht.clone()));
                    // Recurse into nested binders.
                    collect_type_refs_from_hrtb_params(nested, types);
                }
            }
            if let Some(dt) = hrtb_default {
                types.push(dt.clone());
            }
        }
    }
}

/// Collects `Type` references from a `Generics` value (param bounds + where predicates).
///
/// Used to ensure that unresolved markers and dangling Ids inside generic bounds and
/// where-clause predicates are also checked during Phase 1.5 / Phase 1.6 validation.
pub(super) fn collect_type_refs_from_generics(generics: &Generics) -> Vec<Type> {
    let mut types = Vec::new();
    for param in &generics.params {
        match &param.kind {
            GenericParamDefKind::Type { bounds, default, .. } => {
                for bound in bounds {
                    if let GenericBound::TraitBound { trait_, generic_params, .. } = bound {
                        types.push(Type::ResolvedPath(trait_.clone()));
                        // Also scan HRTB type params inside the bound's own binder,
                        // recursing into nested binders.
                        collect_type_refs_from_hrtb_params(generic_params, &mut types);
                    }
                }
                if let Some(default_ty) = default {
                    types.push(default_ty.clone());
                }
            }
            GenericParamDefKind::Const { type_, .. } => {
                types.push(type_.clone());
            }
            GenericParamDefKind::Lifetime { .. } => {}
        }
    }
    for pred in &generics.where_predicates {
        match pred {
            WherePredicate::BoundPredicate { type_: ty, bounds, .. } => {
                types.push(ty.clone());
                for bound in bounds {
                    if let GenericBound::TraitBound { trait_, generic_params, .. } = bound {
                        types.push(Type::ResolvedPath(trait_.clone()));
                        // Also scan HRTB type params inside the bound's own binder,
                        // recursing into nested binders.
                        collect_type_refs_from_hrtb_params(generic_params, &mut types);
                    }
                }
            }
            WherePredicate::EqPredicate { lhs, rhs } => {
                types.push(lhs.clone());
                if let Term::Type(ty) = rhs {
                    types.push(ty.clone());
                }
            }
            WherePredicate::LifetimePredicate { .. } => {}
        }
    }
    types
}

/// Collects all top-level `Type` references from an item's inner variant.
pub(super) fn collect_type_refs_from_item(item: &Item) -> Vec<Type> {
    let mut types = Vec::new();
    match &item.inner {
        ItemEnum::StructField(ty) => types.push(ty.clone()),
        ItemEnum::TypeAlias(ta) => {
            types.push(ta.type_.clone());
            types.extend(collect_type_refs_from_generics(&ta.generics));
        }
        ItemEnum::Struct(s) => {
            types.extend(collect_type_refs_from_generics(&s.generics));
        }
        ItemEnum::Enum(e) => {
            types.extend(collect_type_refs_from_generics(&e.generics));
        }
        ItemEnum::Trait(t) => {
            types.extend(collect_type_refs_from_generics(&t.generics));
            // Also include supertrait bounds (e.g. `trait Foo: Bar + Baz`),
            // including any HRTB binders on each bound (e.g. `for<T: LocalTrait>`).
            for bound in &t.bounds {
                if let GenericBound::TraitBound { trait_, generic_params, .. } = bound {
                    types.push(Type::ResolvedPath(trait_.clone()));
                    collect_type_refs_from_hrtb_params(generic_params, &mut types);
                }
            }
        }
        ItemEnum::Function(f) => {
            types.extend(f.sig.inputs.iter().map(|(_, ty)| ty.clone()));
            if let Some(output) = &f.sig.output {
                types.push(output.clone());
            }
            types.extend(collect_type_refs_from_generics(&f.generics));
        }
        ItemEnum::Impl(impl_) => {
            // The `for` type may reference deleted/unresolved types.
            types.push(impl_.for_.clone());
            // The trait being implemented (if any) is referenced via a Path, not a Type,
            // so we synthesize a ResolvedPath wrapper to reuse the type-walker machinery.
            if let Some(trait_path) = &impl_.trait_ {
                types.push(Type::ResolvedPath(trait_path.clone()));
            }
            types.extend(collect_type_refs_from_generics(&impl_.generics));
        }
        ItemEnum::AssocType { generics, bounds, type_ } => {
            // Collect types from bounds (trait refs) and optional default type,
            // including HRTB binders on each bound (e.g. `type Assoc: for<T: LocalTrait>`).
            for bound in bounds {
                if let GenericBound::TraitBound { trait_, generic_params, .. } = bound {
                    types.push(Type::ResolvedPath(trait_.clone()));
                    collect_type_refs_from_hrtb_params(generic_params, &mut types);
                }
            }
            if let Some(default_ty) = type_ {
                types.push(default_ty.clone());
            }
            types.extend(collect_type_refs_from_generics(generics));
        }
        ItemEnum::AssocConst { type_, .. } => {
            types.push(type_.clone());
        }
        _ => {}
    }
    types
}

// ---------------------------------------------------------------------------
// Dangling-Id collection
// ---------------------------------------------------------------------------

/// Collects all `Id` values referenced inside an item's type fields.
///
/// Used for Phase 1.6 dangling Id check. Only collects `ResolvedPath` ids
/// (excluding external-crate markers with `UNRESOLVED_CRATE_ID`).
pub(super) fn collect_referenced_ids(item: &Item) -> Vec<Id> {
    let mut ids = Vec::new();
    for ty in collect_type_refs_from_item(item) {
        collect_ids_from_type(&ty, &mut ids);
    }
    ids
}

pub(super) fn collect_ids_from_type(ty: &Type, ids: &mut Vec<Id>) {
    match ty {
        Type::ResolvedPath(p) => {
            // Skip the sentinel marker: Id(UNRESOLVED_CRATE_ID) is not a real S-side
            // item reference and must not be passed to the Phase 1.6 dangling-Id check.
            if p.id != Id(UNRESOLVED_CRATE_ID) {
                ids.push(p.id);
            }
            if let Some(args) = &p.args {
                collect_ids_from_generic_args(args, ids);
            }
        }
        Type::BorrowedRef { type_: inner, .. } => collect_ids_from_type(inner, ids),
        Type::Slice(inner) => collect_ids_from_type(inner, ids),
        Type::Array { type_: inner, .. } => collect_ids_from_type(inner, ids),
        Type::Tuple(tys) => {
            for ty in tys {
                collect_ids_from_type(ty, ids);
            }
        }
        Type::RawPointer { type_: inner, .. } => collect_ids_from_type(inner, ids),
        Type::FunctionPointer(fp) => {
            for (_, t) in &fp.sig.inputs {
                collect_ids_from_type(t, ids);
            }
            if let Some(ret) = &fp.sig.output {
                collect_ids_from_type(ret, ids);
            }
            // Also scan HRTB generic_params for type bounds that may reference local types,
            // including nested binders inside each TraitBound's own generic_params.
            collect_ids_from_hrtb_params(&fp.generic_params, ids);
        }
        Type::QualifiedPath { self_type, trait_, args, .. } => {
            collect_ids_from_type(self_type, ids);
            if let Some(args) = args {
                collect_ids_from_generic_args(args, ids);
            }
            if let Some(p) = trait_ {
                // Include the trait Id so Phase 1.6 can catch dangling trait refs,
                // but skip the sentinel UNRESOLVED_CRATE_ID marker.
                if p.id != Id(UNRESOLVED_CRATE_ID) {
                    ids.push(p.id);
                }
                if let Some(args) = &p.args {
                    collect_ids_from_generic_args(args, ids);
                }
            }
        }
        Type::ImplTrait(bounds) => {
            for b in bounds {
                if let GenericBound::TraitBound { trait_, generic_params, .. } = b {
                    if trait_.id != Id(UNRESOLVED_CRATE_ID) {
                        ids.push(trait_.id);
                    }
                    if let Some(args) = &trait_.args {
                        collect_ids_from_generic_args(args, ids);
                    }
                    // Scan HRTB generic_params for local-type references, including
                    // nested binders inside each TraitBound's own generic_params.
                    collect_ids_from_hrtb_params(generic_params, ids);
                }
            }
        }
        Type::DynTrait(dyn_trait) => {
            for pt in &dyn_trait.traits {
                let p = &pt.trait_;
                if p.id != Id(UNRESOLVED_CRATE_ID) {
                    ids.push(p.id);
                }
                if let Some(args) = &p.args {
                    collect_ids_from_generic_args(args, ids);
                }
                // Scan HRTB generic_params for local-type references, including
                // nested binders inside each TraitBound's own generic_params.
                collect_ids_from_hrtb_params(&pt.generic_params, ids);
            }
        }
        // Pattern types (RFC 3437): recurse into the underlying base type.
        Type::Pat { type_: inner, .. } => collect_ids_from_type(inner, ids),
        _ => {}
    }
}

/// Recursively collects Ids from HRTB type-param binder params.
///
/// Scans `generic_params` (e.g. from `GenericBound::TraitBound.generic_params`)
/// for type bounds that may reference local types, including nested binders.
fn collect_ids_from_hrtb_params(
    generic_params: &[rustdoc_types::GenericParamDef],
    ids: &mut Vec<Id>,
) {
    for p in generic_params {
        if let GenericParamDefKind::Type { bounds, default, .. } = &p.kind {
            for bound in bounds {
                if let GenericBound::TraitBound { trait_, generic_params: nested, .. } = bound {
                    if trait_.id != Id(UNRESOLVED_CRATE_ID) {
                        ids.push(trait_.id);
                    }
                    if let Some(args) = &trait_.args {
                        collect_ids_from_generic_args(args, ids);
                    }
                    // Recurse into nested HRTB binders.
                    collect_ids_from_hrtb_params(nested, ids);
                }
            }
            if let Some(default_ty) = default {
                collect_ids_from_type(default_ty, ids);
            }
        }
    }
}

fn collect_ids_from_generic_args(args: &GenericArgs, ids: &mut Vec<Id>) {
    match args {
        GenericArgs::AngleBracketed { args: ga, constraints } => {
            for arg in ga {
                if let GenericArg::Type(t) = arg {
                    collect_ids_from_type(t, ids);
                }
            }
            for c in constraints {
                match &c.binding {
                    AssocItemConstraintKind::Equality(Term::Type(ty)) => {
                        collect_ids_from_type(ty, ids);
                    }
                    AssocItemConstraintKind::Constraint(bounds) => {
                        for b in bounds {
                            if let GenericBound::TraitBound { trait_, generic_params, .. } = b {
                                if trait_.id != Id(UNRESOLVED_CRATE_ID) {
                                    ids.push(trait_.id);
                                }
                                if let Some(args) = &trait_.args {
                                    collect_ids_from_generic_args(args, ids);
                                }
                                // Recurse into nested HRTB binders on the constraint bound.
                                collect_ids_from_hrtb_params(generic_params, ids);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        GenericArgs::Parenthesized { inputs, output } => {
            for t in inputs {
                collect_ids_from_type(t, ids);
            }
            if let Some(ret) = output {
                collect_ids_from_type(ret, ids);
            }
        }
        _ => {}
    }
}
