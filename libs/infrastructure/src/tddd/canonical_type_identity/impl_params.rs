//! Structural normalization of impl-block parameters in type identities.

use std::collections::BTreeSet;

use rustdoc_types::{
    AssocItemConstraint, AssocItemConstraintKind, GenericArg, GenericArgs, GenericBound, Path,
    Term, Type,
};

pub(crate) fn strip_impl_params_type(ty: Type, impl_params: &BTreeSet<String>) -> Type {
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

pub(crate) fn strip_impl_params_args(
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
