//! Type-ref Id rewriting helpers for Phase 1.
//!
//! `rewrite_type_ref_ids_in_item` is the main entry-point used in Phase 1.45 and
//! in the B-side insertion pass (T037) to remap `Type::ResolvedPath.id` values from
//! one Id space to another.

use std::collections::HashMap;

use rustdoc_types::{
    AssocItemConstraint, AssocItemConstraintKind, DynTrait, GenericArg, GenericArgs, GenericBound,
    GenericParamDef, GenericParamDefKind, Id, Item, ItemEnum, Path, PolyTrait, Term, Type,
    WherePredicate,
};

// ---------------------------------------------------------------------------
// Root-module helper
// ---------------------------------------------------------------------------

/// Creates a root `Module` item for a crate.
pub(crate) fn make_root_module_item(root_id: Id, crate_name: String, items: Vec<Id>) -> Item {
    Item {
        id: root_id,
        crate_id: 0,
        name: Some(crate_name),
        span: None,
        visibility: rustdoc_types::Visibility::Public,
        docs: None,
        links: HashMap::new(),
        attrs: vec![],
        deprecation: None,
        inner: ItemEnum::Module(rustdoc_types::Module {
            is_crate: true,
            items,
            is_stripped: false,
        }),
    }
}

// ---------------------------------------------------------------------------
// A-side type-ref id remapping (Phase 1.45)
// ---------------------------------------------------------------------------

/// Rewrites all `Type::ResolvedPath.id` (and nested path Ids in generic args, bounds,
/// etc.) within `item` using `id_map`.  Ids not present in the map are left unchanged.
///
/// This is used in Phase 1.45 to replace A-side local type Ids with their S-side
/// equivalents after all A items have been inserted into S, and in B-side insertion
/// (T037) to replace B-side type refs with the corresponding fresh S Ids from
/// `b_id_remap`.
pub(crate) fn rewrite_type_ref_ids_in_item(mut item: Item, id_map: &HashMap<Id, Id>) -> Item {
    item.inner = match item.inner {
        ItemEnum::StructField(ty) => ItemEnum::StructField(rewrite_type_ids(&ty, id_map)),
        ItemEnum::TypeAlias(mut ta) => {
            ta.type_ = rewrite_type_ids(&ta.type_, id_map);
            ta.generics = rewrite_generics_ids(ta.generics, id_map);
            ItemEnum::TypeAlias(ta)
        }
        ItemEnum::Struct(mut s) => {
            s.generics = rewrite_generics_ids(s.generics, id_map);
            ItemEnum::Struct(s)
        }
        ItemEnum::Enum(mut e) => {
            e.generics = rewrite_generics_ids(e.generics, id_map);
            ItemEnum::Enum(e)
        }
        ItemEnum::Trait(mut t) => {
            t.generics = rewrite_generics_ids(t.generics, id_map);
            t.bounds = t.bounds.into_iter().map(|b| rewrite_bound_ids(b, id_map)).collect();
            ItemEnum::Trait(t)
        }
        ItemEnum::Function(mut f) => {
            f.sig.inputs = f
                .sig
                .inputs
                .into_iter()
                .map(|(name, ty)| (name, rewrite_type_ids(&ty, id_map)))
                .collect();
            f.sig.output = f.sig.output.map(|ty| rewrite_type_ids(&ty, id_map));
            f.generics = rewrite_generics_ids(f.generics, id_map);
            ItemEnum::Function(f)
        }
        ItemEnum::Impl(mut i) => {
            i.for_ = rewrite_type_ids(&i.for_, id_map);
            if let Some(ref mut trait_path) = i.trait_ {
                rewrite_path_id(trait_path, id_map);
            }
            i.generics = rewrite_generics_ids(i.generics, id_map);
            ItemEnum::Impl(i)
        }
        ItemEnum::AssocType { generics, bounds, type_ } => {
            let generics = rewrite_generics_ids(generics, id_map);
            let bounds = bounds.into_iter().map(|b| rewrite_bound_ids(b, id_map)).collect();
            let type_ = type_.map(|ty| rewrite_type_ids(&ty, id_map));
            ItemEnum::AssocType { generics, bounds, type_ }
        }
        ItemEnum::AssocConst { type_, value } => {
            ItemEnum::AssocConst { type_: rewrite_type_ids(&type_, id_map), value }
        }
        other => other,
    };
    item
}

/// Rewrites all `ResolvedPath.id` and nested path Ids within a `Type`.
fn rewrite_type_ids(ty: &Type, id_map: &HashMap<Id, Id>) -> Type {
    match ty {
        Type::ResolvedPath(p) => {
            let new_id = id_map.get(&p.id).copied().unwrap_or(p.id);
            let new_args = p.args.as_deref().map(|a| Box::new(rewrite_generic_args_ids(a, id_map)));
            Type::ResolvedPath(Path { path: p.path.clone(), id: new_id, args: new_args })
        }
        Type::BorrowedRef { lifetime, is_mutable, type_: inner } => Type::BorrowedRef {
            lifetime: lifetime.clone(),
            is_mutable: *is_mutable,
            type_: Box::new(rewrite_type_ids(inner, id_map)),
        },
        Type::Slice(inner) => Type::Slice(Box::new(rewrite_type_ids(inner, id_map))),
        Type::Array { type_: inner, len } => {
            Type::Array { type_: Box::new(rewrite_type_ids(inner, id_map)), len: len.clone() }
        }
        Type::Tuple(tys) => Type::Tuple(tys.iter().map(|t| rewrite_type_ids(t, id_map)).collect()),
        Type::RawPointer { is_mutable, type_: inner } => Type::RawPointer {
            is_mutable: *is_mutable,
            type_: Box::new(rewrite_type_ids(inner, id_map)),
        },
        Type::ImplTrait(bounds) => {
            Type::ImplTrait(bounds.iter().map(|b| rewrite_bound_ids(b.clone(), id_map)).collect())
        }
        Type::DynTrait(dt) => {
            let new_traits: Vec<PolyTrait> = dt
                .traits
                .iter()
                .map(|pt| {
                    let mut new_path = pt.trait_.clone();
                    rewrite_path_id(&mut new_path, id_map);
                    PolyTrait { trait_: new_path, generic_params: pt.generic_params.clone() }
                })
                .collect();
            Type::DynTrait(DynTrait { traits: new_traits, lifetime: dt.lifetime.clone() })
        }
        Type::FunctionPointer(fp) => {
            let new_inputs = fp
                .sig
                .inputs
                .iter()
                .map(|(name, t)| (name.clone(), rewrite_type_ids(t, id_map)))
                .collect();
            let new_output = fp.sig.output.as_ref().map(|t| rewrite_type_ids(t, id_map));
            let new_fp = rustdoc_types::FunctionPointer {
                sig: rustdoc_types::FunctionSignature {
                    inputs: new_inputs,
                    output: new_output,
                    is_c_variadic: fp.sig.is_c_variadic,
                },
                generic_params: fp.generic_params.clone(),
                header: fp.header.clone(),
            };
            Type::FunctionPointer(Box::new(new_fp))
        }
        Type::QualifiedPath { name, self_type, trait_, args } => {
            let new_self = rewrite_type_ids(self_type, id_map);
            let new_trait = trait_.as_ref().map(|p| {
                let mut np = p.clone();
                rewrite_path_id(&mut np, id_map);
                np
            });
            let new_args = args.as_deref().map(|a| Box::new(rewrite_generic_args_ids(a, id_map)));
            Type::QualifiedPath {
                name: name.clone(),
                self_type: Box::new(new_self),
                trait_: new_trait,
                args: new_args,
            }
        }
        Type::Pat { type_: inner, __pat_unstable_do_not_use } => Type::Pat {
            type_: Box::new(rewrite_type_ids(inner, id_map)),
            __pat_unstable_do_not_use: __pat_unstable_do_not_use.clone(),
        },
        // Generic, Primitive, Infer — no Ids to remap.
        other => other.clone(),
    }
}

/// Rewrites path Ids inside a `GenericArgs` value.
fn rewrite_generic_args_ids(args: &GenericArgs, id_map: &HashMap<Id, Id>) -> GenericArgs {
    match args {
        GenericArgs::AngleBracketed { args: ga, constraints } => {
            let new_args = ga
                .iter()
                .map(|a| match a {
                    GenericArg::Type(t) => GenericArg::Type(rewrite_type_ids(t, id_map)),
                    other => other.clone(),
                })
                .collect();
            let new_constraints: Vec<AssocItemConstraint> = constraints
                .iter()
                .map(|c| {
                    let new_binding = match &c.binding {
                        AssocItemConstraintKind::Equality(Term::Type(ty)) => {
                            AssocItemConstraintKind::Equality(Term::Type(rewrite_type_ids(
                                ty, id_map,
                            )))
                        }
                        AssocItemConstraintKind::Constraint(bounds) => {
                            AssocItemConstraintKind::Constraint(
                                bounds
                                    .iter()
                                    .map(|b| rewrite_bound_ids(b.clone(), id_map))
                                    .collect(),
                            )
                        }
                        other => other.clone(),
                    };
                    AssocItemConstraint {
                        name: c.name.clone(),
                        args: c
                            .args
                            .as_deref()
                            .map(|a| Box::new(rewrite_generic_args_ids(a, id_map))),
                        binding: new_binding,
                    }
                })
                .collect();
            GenericArgs::AngleBracketed { args: new_args, constraints: new_constraints }
        }
        GenericArgs::Parenthesized { inputs, output } => GenericArgs::Parenthesized {
            inputs: inputs.iter().map(|t| rewrite_type_ids(t, id_map)).collect(),
            output: output.as_ref().map(|t| rewrite_type_ids(t, id_map)),
        },
        other => other.clone(),
    }
}

/// Rewrites a single `Path.id` in place using `id_map`.
fn rewrite_path_id(path: &mut Path, id_map: &HashMap<Id, Id>) {
    if let Some(&new_id) = id_map.get(&path.id) {
        path.id = new_id;
    }
    if let Some(ref mut args) = path.args {
        **args = rewrite_generic_args_ids(args, id_map);
    }
}

/// Rewrites path Ids inside a `GenericBound`.
fn rewrite_bound_ids(bound: GenericBound, id_map: &HashMap<Id, Id>) -> GenericBound {
    match bound {
        GenericBound::TraitBound { trait_, modifier, generic_params } => {
            let mut new_trait = trait_;
            rewrite_path_id(&mut new_trait, id_map);
            // Rewrite ids inside HRTB binder params (e.g. `for<T: LocalTrait>`).
            // Without this, local type ids nested in HRTB binders remain in A's id space
            // and are rejected by Phase 1.6 as dangling references.
            let new_generic_params =
                generic_params.into_iter().map(|p| rewrite_generic_param_ids(p, id_map)).collect();
            GenericBound::TraitBound {
                trait_: new_trait,
                modifier,
                generic_params: new_generic_params,
            }
        }
        other => other,
    }
}

/// Rewrites path Ids inside a `Generics` (param bounds + where predicates).
fn rewrite_generics_ids(
    mut generics: rustdoc_types::Generics,
    id_map: &HashMap<Id, Id>,
) -> rustdoc_types::Generics {
    generics.params =
        generics.params.into_iter().map(|p| rewrite_generic_param_ids(p, id_map)).collect();
    generics.where_predicates = generics
        .where_predicates
        .into_iter()
        .map(|pred| rewrite_where_predicate_ids(pred, id_map))
        .collect();
    generics
}

/// Rewrites path Ids inside a `GenericParamDef`.
fn rewrite_generic_param_ids(
    mut param: GenericParamDef,
    id_map: &HashMap<Id, Id>,
) -> GenericParamDef {
    param.kind = match param.kind {
        GenericParamDefKind::Type { bounds, default, is_synthetic } => {
            let new_bounds = bounds.into_iter().map(|b| rewrite_bound_ids(b, id_map)).collect();
            let new_default = default.map(|ty| rewrite_type_ids(&ty, id_map));
            GenericParamDefKind::Type { bounds: new_bounds, default: new_default, is_synthetic }
        }
        GenericParamDefKind::Const { type_, default } => {
            GenericParamDefKind::Const { type_: rewrite_type_ids(&type_, id_map), default }
        }
        other => other,
    };
    param
}

/// Rewrites path Ids inside a `WherePredicate`.
fn rewrite_where_predicate_ids(pred: WherePredicate, id_map: &HashMap<Id, Id>) -> WherePredicate {
    match pred {
        WherePredicate::BoundPredicate { type_: ty, bounds, generic_params } => {
            let new_ty = rewrite_type_ids(&ty, id_map);
            let new_bounds = bounds.into_iter().map(|b| rewrite_bound_ids(b, id_map)).collect();
            WherePredicate::BoundPredicate { type_: new_ty, bounds: new_bounds, generic_params }
        }
        WherePredicate::EqPredicate { lhs, rhs } => {
            let new_lhs = rewrite_type_ids(&lhs, id_map);
            let new_rhs = match rhs {
                Term::Type(ty) => Term::Type(rewrite_type_ids(&ty, id_map)),
                other => other,
            };
            WherePredicate::EqPredicate { lhs: new_lhs, rhs: new_rhs }
        }
        other => other,
    }
}
