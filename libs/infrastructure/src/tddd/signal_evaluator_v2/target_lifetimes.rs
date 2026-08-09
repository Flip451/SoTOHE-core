//! Named-lifetime collection over rustdoc target types.
//!
//! Supports the alias generics comparison in the sibling `structural_eq`
//! module: a lifetime parameter is excluded from that comparison only when
//! the alias TARGET carries its name (the catalogue schema cannot declare
//! lifetime parameters, so source-declared lifetimes are recorded lexically
//! in the target).

use std::collections::BTreeSet;

/// Collects every named-lifetime occurrence in a rustdoc target type.
/// Binder DECLARATIONS (`for<'x>`) are not collected — only uses — but a use
/// of a binder-scoped lifetime is indistinguishable here; rustc's shadowing
/// rejection (E0496) keeps that ambiguity unrepresentable in source.
pub(super) fn collect_type_lifetimes(ty: &rustdoc_types::Type, out: &mut BTreeSet<String>) {
    use rustdoc_types::Type;
    match ty {
        Type::ResolvedPath(path) => collect_path_lifetimes(path, out),
        Type::DynTrait(dyn_trait) => {
            if let Some(lifetime) = &dyn_trait.lifetime {
                out.insert(lifetime.clone());
            }
            for poly in &dyn_trait.traits {
                collect_path_lifetimes(&poly.trait_, out);
                collect_binder_lifetimes(&poly.generic_params, out);
            }
        }
        Type::BorrowedRef { lifetime, type_, .. } => {
            if let Some(lifetime) = lifetime {
                out.insert(lifetime.clone());
            }
            collect_type_lifetimes(type_, out);
        }
        Type::Tuple(elements) => {
            for element in elements {
                collect_type_lifetimes(element, out);
            }
        }
        Type::Slice(inner) => collect_type_lifetimes(inner, out),
        Type::Array { type_, .. } | Type::RawPointer { type_, .. } => {
            collect_type_lifetimes(type_, out);
        }
        Type::FunctionPointer(fp) => {
            collect_binder_lifetimes(&fp.generic_params, out);
            for (_, input) in &fp.sig.inputs {
                collect_type_lifetimes(input, out);
            }
            if let Some(output) = &fp.sig.output {
                collect_type_lifetimes(output, out);
            }
        }
        Type::QualifiedPath { self_type, trait_, args, .. } => {
            collect_type_lifetimes(self_type, out);
            if let Some(trait_) = trait_ {
                collect_path_lifetimes(trait_, out);
            }
            if let Some(args) = args.as_deref() {
                collect_generic_args_lifetimes(args, out);
            }
        }
        Type::ImplTrait(bounds) => {
            for bound in bounds {
                collect_bound_lifetimes(bound, out);
            }
        }
        Type::Generic(_) | Type::Primitive(_) | Type::Infer | Type::Pat { .. } => {}
    }
}

fn collect_path_lifetimes(path: &rustdoc_types::Path, out: &mut BTreeSet<String>) {
    if let Some(args) = &path.args {
        collect_generic_args_lifetimes(args, out);
    }
}

fn collect_generic_args_lifetimes(args: &rustdoc_types::GenericArgs, out: &mut BTreeSet<String>) {
    use rustdoc_types::{AssocItemConstraintKind, GenericArg, GenericArgs, Term};
    match args {
        GenericArgs::AngleBracketed { args, constraints } => {
            for arg in args {
                match arg {
                    GenericArg::Lifetime(lifetime) => {
                        out.insert(lifetime.clone());
                    }
                    GenericArg::Type(ty) => collect_type_lifetimes(ty, out),
                    GenericArg::Const(_) | GenericArg::Infer => {}
                }
            }
            for constraint in constraints {
                if let Some(args) = &constraint.args {
                    collect_generic_args_lifetimes(args, out);
                }
                match &constraint.binding {
                    AssocItemConstraintKind::Equality(Term::Type(ty)) => {
                        collect_type_lifetimes(ty, out);
                    }
                    AssocItemConstraintKind::Equality(Term::Constant(_)) => {}
                    AssocItemConstraintKind::Constraint(bounds) => {
                        for bound in bounds {
                            collect_bound_lifetimes(bound, out);
                        }
                    }
                }
            }
        }
        GenericArgs::Parenthesized { inputs, output } => {
            for input in inputs {
                collect_type_lifetimes(input, out);
            }
            if let Some(output) = output {
                collect_type_lifetimes(output, out);
            }
        }
        GenericArgs::ReturnTypeNotation => {}
    }
}

fn collect_bound_lifetimes(bound: &rustdoc_types::GenericBound, out: &mut BTreeSet<String>) {
    use rustdoc_types::GenericBound;
    match bound {
        GenericBound::TraitBound { trait_, generic_params, .. } => {
            collect_path_lifetimes(trait_, out);
            collect_binder_lifetimes(generic_params, out);
        }
        GenericBound::Outlives(lifetime) => {
            out.insert(lifetime.clone());
        }
        GenericBound::Use(captures) => {
            for capture in captures {
                if let rustdoc_types::PreciseCapturingArg::Lifetime(lifetime) = capture {
                    out.insert(lifetime.clone());
                }
            }
        }
    }
}

/// Traverses uses nested in an HRTB binder's parameter metadata.
///
/// The parameter declarations themselves are intentionally not inserted. Their
/// bounds, defaults, and const types can nevertheless carry an outer alias
/// lifetime and are part of the target's observable structure.
fn collect_binder_lifetimes(params: &[rustdoc_types::GenericParamDef], out: &mut BTreeSet<String>) {
    use rustdoc_types::GenericParamDefKind;
    for param in params {
        match &param.kind {
            GenericParamDefKind::Lifetime { outlives } => {
                for lifetime in outlives {
                    out.insert(lifetime.clone());
                }
            }
            GenericParamDefKind::Type { bounds, default, .. } => {
                for bound in bounds {
                    collect_bound_lifetimes(bound, out);
                }
                if let Some(default) = default {
                    collect_type_lifetimes(default, out);
                }
            }
            GenericParamDefKind::Const { type_, .. } => collect_type_lifetimes(type_, out),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::collections::BTreeSet;

    use rustdoc_types::{
        Abi, DynTrait, FunctionHeader, FunctionPointer, FunctionSignature, GenericArg, GenericArgs,
        GenericBound, GenericParamDef, GenericParamDefKind, Id, Path, PolyTrait,
        TraitBoundModifier, Type,
    };

    use super::collect_type_lifetimes;

    fn binder_type_parameter_with_outer_lifetime_use() -> GenericParamDef {
        GenericParamDef {
            name: "T".to_owned(),
            kind: GenericParamDefKind::Type {
                bounds: vec![GenericBound::TraitBound {
                    trait_: Path {
                        path: "UsesLifetime".to_owned(),
                        id: Id(1),
                        args: Some(Box::new(GenericArgs::AngleBracketed {
                            args: vec![GenericArg::Type(Type::BorrowedRef {
                                lifetime: Some("'a".to_owned()),
                                is_mutable: false,
                                type_: Box::new(Type::Primitive("str".to_owned())),
                            })],
                            constraints: vec![],
                        })),
                    },
                    generic_params: vec![],
                    modifier: TraitBoundModifier::None,
                }],
                default: None,
                is_synthetic: false,
            },
        }
    }

    #[test]
    fn test_collect_type_lifetimes_collects_uses_nested_in_binder_metadata() {
        let plain_trait_path = || Path { path: "Bound".to_owned(), id: Id(2), args: None };
        let targets = vec![
            Type::DynTrait(DynTrait {
                traits: vec![PolyTrait {
                    trait_: plain_trait_path(),
                    generic_params: vec![binder_type_parameter_with_outer_lifetime_use()],
                }],
                lifetime: None,
            }),
            Type::ImplTrait(vec![GenericBound::TraitBound {
                trait_: plain_trait_path(),
                generic_params: vec![binder_type_parameter_with_outer_lifetime_use()],
                modifier: TraitBoundModifier::None,
            }]),
            Type::FunctionPointer(Box::new(FunctionPointer {
                sig: FunctionSignature { inputs: vec![], output: None, is_c_variadic: false },
                generic_params: vec![binder_type_parameter_with_outer_lifetime_use()],
                header: FunctionHeader {
                    is_const: false,
                    is_unsafe: false,
                    is_async: false,
                    abi: Abi::Rust,
                },
            })),
        ];

        for target in targets {
            let mut lifetimes = BTreeSet::new();
            collect_type_lifetimes(&target, &mut lifetimes);
            assert_eq!(lifetimes, BTreeSet::from(["'a".to_owned()]));
        }
    }

    #[test]
    fn test_collect_type_lifetimes_does_not_collect_binder_declarations() {
        let target = Type::DynTrait(DynTrait {
            traits: vec![PolyTrait {
                trait_: Path { path: "Bound".to_owned(), id: Id(3), args: None },
                generic_params: vec![GenericParamDef {
                    name: "'binder".to_owned(),
                    kind: GenericParamDefKind::Lifetime { outlives: vec![] },
                }],
            }],
            lifetime: None,
        });
        let mut lifetimes = BTreeSet::new();
        collect_type_lifetimes(&target, &mut lifetimes);
        assert!(lifetimes.is_empty());
    }
}
