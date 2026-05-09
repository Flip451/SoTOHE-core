//! Phase 1.5 — unresolved-marker Id resolution.
//!
//! Rewrites `Id(UNRESOLVED_CRATE_ID)` placeholders inside `rustdoc_types` trees
//! to real S-side Ids by looking up the bare type name in the S-universe name map.
//!
//! Read-only scanning (unresolved-marker detection, type-reference collection, and
//! dangling-Id collection) lives in the sibling `collect_refs` module.

use std::collections::{BTreeMap, HashSet};

use domain::tddd::Phase1Error;
use rustdoc_types::{
    AssocItemConstraintKind, GenericArg, GenericArgs, GenericBound, GenericParamDefKind, Generics,
    Id, Term, Type, WherePredicate,
};

use crate::tddd::type_ref_parser::UNRESOLVED_CRATE_ID;

use super::is_local_unresolved_path;

// ---------------------------------------------------------------------------
// Local-path normalization helper
// ---------------------------------------------------------------------------

/// Extracts the short (last-segment) name from a local unresolved path.
///
/// A catalogue `TypeRef` may use a relative path like `crate::nested::User`.
/// Phase 1.5 looks up names in `s_name_to_id`, which is keyed by **short names**
/// (e.g. `"User"`).  Stripping only one `crate::` prefix leaves `nested::User`,
/// which would never match.  Instead we always take the last `::` segment.
///
/// # Examples
///
/// ```text
/// "User"                 → "User"
/// "crate::User"          → "User"
/// "crate::nested::User"  → "User"
/// "self::MyTrait"        → "MyTrait"
/// ```
fn local_path_short_name(path: &str) -> &str {
    path.rsplit("::").next().unwrap_or(path)
}

// ---------------------------------------------------------------------------
// Resolution functions
// ---------------------------------------------------------------------------

/// Resolves unresolved markers in a single `GenericBound`.
pub(super) fn resolve_generic_bound(
    bound: GenericBound,
    s_known_names: &HashSet<String>,
    s_name_to_id: &BTreeMap<String, Id>,
) -> Result<GenericBound, Phase1Error> {
    match bound {
        GenericBound::TraitBound { trait_, modifier, generic_params } => {
            // Resolve the trait Id if it's a local unresolved marker.
            let mut new_id = trait_.id;
            if new_id == Id(UNRESOLVED_CRATE_ID) && is_local_unresolved_path(&trait_.path) {
                let bare_name = local_path_short_name(&trait_.path);
                if s_known_names.contains(bare_name) {
                    if let Some(&resolved_id) = s_name_to_id.get(bare_name) {
                        new_id = resolved_id;
                    }
                } else {
                    return Err(Phase1Error::UnresolvedTypeRef(trait_.path.clone()));
                }
            }
            let new_args = match trait_.args {
                Some(boxed) => {
                    Some(Box::new(resolve_generic_args(*boxed, s_known_names, s_name_to_id)?))
                }
                None => None,
            };
            // Also resolve unresolved markers in HRTB binder params (`for<T: LocalTrait>`).
            let new_generic_params = {
                let temp_generics =
                    Generics { params: generic_params, where_predicates: Vec::new() };
                resolve_generics(temp_generics, s_known_names, s_name_to_id)?.params
            };
            Ok(GenericBound::TraitBound {
                trait_: rustdoc_types::Path { path: trait_.path, id: new_id, args: new_args },
                modifier,
                generic_params: new_generic_params,
            })
        }
        other => Ok(other),
    }
}

/// Resolves unresolved markers in a `Generics` value (param bounds and where predicates).
pub(super) fn resolve_generics(
    generics: Generics,
    s_known_names: &HashSet<String>,
    s_name_to_id: &BTreeMap<String, Id>,
) -> Result<Generics, Phase1Error> {
    let new_params: Result<Vec<_>, _> = generics
        .params
        .into_iter()
        .map(|mut p| {
            p.kind = match p.kind {
                GenericParamDefKind::Type { bounds, default, is_synthetic } => {
                    let new_bounds: Result<Vec<_>, _> = bounds
                        .into_iter()
                        .map(|b| resolve_generic_bound(b, s_known_names, s_name_to_id))
                        .collect();
                    let new_default = match default {
                        Some(ty) => Some(resolve_type(ty, s_known_names, s_name_to_id)?),
                        None => None,
                    };
                    GenericParamDefKind::Type {
                        bounds: new_bounds?,
                        default: new_default,
                        is_synthetic,
                    }
                }
                GenericParamDefKind::Const { type_, default } => GenericParamDefKind::Const {
                    type_: resolve_type(type_, s_known_names, s_name_to_id)?,
                    default,
                },
                other => other,
            };
            Ok(p)
        })
        .collect();
    let new_preds: Result<Vec<_>, _> = generics
        .where_predicates
        .into_iter()
        .map(|pred| match pred {
            WherePredicate::BoundPredicate { type_: ty, bounds, generic_params } => {
                let new_ty = resolve_type(ty, s_known_names, s_name_to_id)?;
                let new_bounds: Result<Vec<_>, _> = bounds
                    .into_iter()
                    .map(|b| resolve_generic_bound(b, s_known_names, s_name_to_id))
                    .collect();
                // Also resolve HRTB binder params in the predicate's own binder
                // (e.g. `for<T: LocalTrait> Fn(T): Bar`).
                let new_generic_params = {
                    let temp = Generics { params: generic_params, where_predicates: Vec::new() };
                    resolve_generics(temp, s_known_names, s_name_to_id)?.params
                };
                Ok(WherePredicate::BoundPredicate {
                    type_: new_ty,
                    bounds: new_bounds?,
                    generic_params: new_generic_params,
                })
            }
            WherePredicate::EqPredicate { lhs, rhs } => {
                let new_lhs = resolve_type(lhs, s_known_names, s_name_to_id)?;
                let new_rhs = match rhs {
                    Term::Type(ty) => Term::Type(resolve_type(ty, s_known_names, s_name_to_id)?),
                    other => other,
                };
                Ok(WherePredicate::EqPredicate { lhs: new_lhs, rhs: new_rhs })
            }
            other => Ok(other),
        })
        .collect();
    Ok(Generics { params: new_params?, where_predicates: new_preds? })
}

/// Resolves unresolved markers in a single `Type`.
pub(super) fn resolve_type(
    ty: Type,
    s_known_names: &HashSet<String>,
    s_name_to_id: &BTreeMap<String, Id>,
) -> Result<Type, Phase1Error> {
    match ty {
        Type::ResolvedPath(mut p) => {
            if p.id == Id(UNRESOLVED_CRATE_ID) && is_local_unresolved_path(&p.path) {
                // Local/relative unresolved marker: look up the short name in S.
                // Use the last segment so that `crate::nested::User` resolves the
                // same as bare `User` — the name map only has short names.
                let bare_name = local_path_short_name(&p.path);
                if s_known_names.contains(bare_name) {
                    if let Some(&resolved_id) = s_name_to_id.get(bare_name) {
                        p.id = resolved_id;
                    }
                } else {
                    return Err(Phase1Error::UnresolvedTypeRef(p.path.clone()));
                }
            }
            // Recurse into args.
            let new_args = match p.args {
                Some(boxed_args) => {
                    Some(Box::new(resolve_generic_args(*boxed_args, s_known_names, s_name_to_id)?))
                }
                None => None,
            };
            Ok(Type::ResolvedPath(rustdoc_types::Path { path: p.path, id: p.id, args: new_args }))
        }
        Type::BorrowedRef { lifetime, is_mutable, type_: inner } => Ok(Type::BorrowedRef {
            lifetime,
            is_mutable,
            type_: Box::new(resolve_type(*inner, s_known_names, s_name_to_id)?),
        }),
        Type::Slice(inner) => {
            Ok(Type::Slice(Box::new(resolve_type(*inner, s_known_names, s_name_to_id)?)))
        }
        Type::Array { type_: inner, len } => Ok(Type::Array {
            type_: Box::new(resolve_type(*inner, s_known_names, s_name_to_id)?),
            len,
        }),
        Type::Tuple(tys) => {
            let resolved: Result<Vec<_>, _> =
                tys.into_iter().map(|t| resolve_type(t, s_known_names, s_name_to_id)).collect();
            Ok(Type::Tuple(resolved?))
        }
        Type::RawPointer { is_mutable, type_: inner } => Ok(Type::RawPointer {
            is_mutable,
            type_: Box::new(resolve_type(*inner, s_known_names, s_name_to_id)?),
        }),
        Type::FunctionPointer(fp) => {
            let new_inputs: Result<Vec<_>, _> = fp
                .sig
                .inputs
                .into_iter()
                .map(|(name, t)| resolve_type(t, s_known_names, s_name_to_id).map(|t| (name, t)))
                .collect();
            let new_output = match fp.sig.output {
                Some(t) => Some(resolve_type(t, s_known_names, s_name_to_id)?),
                None => None,
            };
            // Resolve HRTB generic_params: a `for<'a, T: LocalTrait>` bound may carry
            // an `UNRESOLVED_CRATE_ID` marker in the trait bound Id.  Wrap in a
            // temporary `Generics` to reuse the existing resolve_generics logic.
            let temp_generics =
                Generics { params: fp.generic_params, where_predicates: Vec::new() };
            let resolved_generics = resolve_generics(temp_generics, s_known_names, s_name_to_id)?;
            Ok(Type::FunctionPointer(Box::new(rustdoc_types::FunctionPointer {
                sig: rustdoc_types::FunctionSignature {
                    inputs: new_inputs?,
                    output: new_output,
                    is_c_variadic: fp.sig.is_c_variadic,
                },
                generic_params: resolved_generics.params,
                header: fp.header,
            })))
        }
        Type::QualifiedPath { name, args, self_type, trait_ } => {
            let new_args = match args {
                Some(boxed) => {
                    Some(Box::new(resolve_generic_args(*boxed, s_known_names, s_name_to_id)?))
                }
                None => None,
            };
            let new_self_type = resolve_type(*self_type, s_known_names, s_name_to_id)?;
            // Also resolve the trait path Id if it's an unresolved marker.
            let new_trait = match trait_ {
                Some(mut p) => {
                    if p.id == Id(UNRESOLVED_CRATE_ID) && is_local_unresolved_path(&p.path) {
                        let bare_name = local_path_short_name(&p.path);
                        if s_known_names.contains(bare_name) {
                            if let Some(&resolved_id) = s_name_to_id.get(bare_name) {
                                p.id = resolved_id;
                            }
                        } else {
                            return Err(Phase1Error::UnresolvedTypeRef(p.path.clone()));
                        }
                    }
                    let new_path_args = match p.args {
                        Some(boxed) => Some(Box::new(resolve_generic_args(
                            *boxed,
                            s_known_names,
                            s_name_to_id,
                        )?)),
                        None => None,
                    };
                    Some(rustdoc_types::Path { path: p.path, id: p.id, args: new_path_args })
                }
                None => None,
            };
            Ok(Type::QualifiedPath {
                name,
                args: new_args,
                self_type: Box::new(new_self_type),
                trait_: new_trait,
            })
        }
        Type::ImplTrait(bounds) => {
            let new_bounds: Result<Vec<_>, _> = bounds
                .into_iter()
                .map(|b| match b {
                    GenericBound::TraitBound { trait_, modifier, generic_params } => {
                        let new_args = match trait_.args {
                            Some(boxed) => Some(Box::new(resolve_generic_args(
                                *boxed,
                                s_known_names,
                                s_name_to_id,
                            )?)),
                            None => None,
                        };
                        // Resolve the trait Id if it's a local unresolved marker.
                        let mut new_id = trait_.id;
                        if new_id == Id(UNRESOLVED_CRATE_ID)
                            && is_local_unresolved_path(&trait_.path)
                        {
                            let bare_name = local_path_short_name(&trait_.path);
                            if s_known_names.contains(bare_name) {
                                if let Some(&resolved_id) = s_name_to_id.get(bare_name) {
                                    new_id = resolved_id;
                                }
                            } else {
                                return Err(Phase1Error::UnresolvedTypeRef(trait_.path.clone()));
                            }
                        }
                        // Resolve HRTB binder params (e.g. `for<T: LocalTrait>`).
                        let new_generic_params = {
                            let temp =
                                Generics { params: generic_params, where_predicates: Vec::new() };
                            resolve_generics(temp, s_known_names, s_name_to_id)?.params
                        };
                        Ok(GenericBound::TraitBound {
                            trait_: rustdoc_types::Path {
                                path: trait_.path,
                                id: new_id,
                                args: new_args,
                            },
                            modifier,
                            generic_params: new_generic_params,
                        })
                    }
                    other => Ok(other),
                })
                .collect();
            Ok(Type::ImplTrait(new_bounds?))
        }
        Type::DynTrait(dyn_trait) => {
            let new_traits: Result<Vec<_>, _> = dyn_trait
                .traits
                .into_iter()
                .map(|poly_trait| {
                    let p = poly_trait.trait_;
                    let new_args = match p.args {
                        Some(boxed) => Some(Box::new(resolve_generic_args(
                            *boxed,
                            s_known_names,
                            s_name_to_id,
                        )?)),
                        None => None,
                    };
                    let mut new_id = p.id;
                    if new_id == Id(UNRESOLVED_CRATE_ID) && is_local_unresolved_path(&p.path) {
                        let bare_name = local_path_short_name(&p.path);
                        if s_known_names.contains(bare_name) {
                            if let Some(&resolved_id) = s_name_to_id.get(bare_name) {
                                new_id = resolved_id;
                            }
                        } else {
                            return Err(Phase1Error::UnresolvedTypeRef(p.path.clone()));
                        }
                    }
                    // Resolve HRTB binder params (e.g. `for<T: LocalTrait>`).
                    let new_generic_params = {
                        let temp = Generics {
                            params: poly_trait.generic_params,
                            where_predicates: Vec::new(),
                        };
                        resolve_generics(temp, s_known_names, s_name_to_id)?.params
                    };
                    Ok(rustdoc_types::PolyTrait {
                        trait_: rustdoc_types::Path { path: p.path, id: new_id, args: new_args },
                        generic_params: new_generic_params,
                    })
                })
                .collect();
            Ok(Type::DynTrait(rustdoc_types::DynTrait {
                traits: new_traits?,
                lifetime: dyn_trait.lifetime,
            }))
        }
        // Pattern types (RFC 3437): recurse into the underlying base type so that
        // unresolved markers nested inside a `Type::Pat` are resolved in Phase 1.5.
        Type::Pat { type_: inner, __pat_unstable_do_not_use } => Ok(Type::Pat {
            type_: Box::new(resolve_type(*inner, s_known_names, s_name_to_id)?),
            __pat_unstable_do_not_use,
        }),
        other => Ok(other),
    }
}

pub(super) fn resolve_generic_args(
    args: GenericArgs,
    s_known_names: &HashSet<String>,
    s_name_to_id: &BTreeMap<String, Id>,
) -> Result<GenericArgs, Phase1Error> {
    match args {
        GenericArgs::AngleBracketed { args: ga, constraints } => {
            let resolved: Result<Vec<_>, _> = ga
                .into_iter()
                .map(|arg| match arg {
                    GenericArg::Type(ty) => {
                        resolve_type(ty, s_known_names, s_name_to_id).map(GenericArg::Type)
                    }
                    other => Ok(other),
                })
                .collect();
            let resolved_constraints: Result<Vec<_>, _> = constraints
                .into_iter()
                .map(|mut c| {
                    c.binding = match c.binding {
                        AssocItemConstraintKind::Equality(Term::Type(ty)) => {
                            AssocItemConstraintKind::Equality(Term::Type(resolve_type(
                                ty,
                                s_known_names,
                                s_name_to_id,
                            )?))
                        }
                        AssocItemConstraintKind::Constraint(bounds) => {
                            let new_bounds: Result<Vec<_>, _> = bounds
                                .into_iter()
                                .map(|b| resolve_generic_bound(b, s_known_names, s_name_to_id))
                                .collect();
                            AssocItemConstraintKind::Constraint(new_bounds?)
                        }
                        other => other,
                    };
                    Ok(c)
                })
                .collect();
            Ok(GenericArgs::AngleBracketed { args: resolved?, constraints: resolved_constraints? })
        }
        GenericArgs::Parenthesized { inputs, output } => {
            let new_inputs: Result<Vec<_>, _> =
                inputs.into_iter().map(|t| resolve_type(t, s_known_names, s_name_to_id)).collect();
            let new_output = match output {
                Some(t) => Some(resolve_type(t, s_known_names, s_name_to_id)?),
                None => None,
            };
            Ok(GenericArgs::Parenthesized { inputs: new_inputs?, output: new_output })
        }
        other => Ok(other),
    }
}
