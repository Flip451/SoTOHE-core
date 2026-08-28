//! Recursive rustdoc type canonicalization.

use std::collections::{BTreeSet, HashMap};

use domain::tddd::NewTypeGraphCodecError;
use domain::tddd::catalogue_v2::identifiers::{CrateName, FullyQualifiedItemPath, TypeRef};
use domain::tddd::catalogue_v2::identity_resolution::{
    CatalogueIdentityResolutionError, resolve_catalogue_identity,
};
use rustdoc_types::{
    AssocItemConstraint, AssocItemConstraintKind, DynTrait, GenericArg, GenericArgs, GenericBound,
    Id, ItemSummary, Path, Term, Type,
};

use super::super::type_ref_parser::{render_bound, render_type};
use super::DefinitionPathAuthority;
use super::rustdoc_paths::{canonicalize_path, summary_identity};

pub(super) fn unique_resolved_id(
    short_name: &str,
    catalogue_crate: &CrateName,
    universe: &BTreeSet<FullyQualifiedItemPath>,
    rustdoc_paths: &HashMap<Id, ItemSummary>,
) -> Option<Id> {
    let reference = TypeRef::new(short_name.to_owned()).ok()?;
    let identity = match resolve_catalogue_identity(&reference, catalogue_crate, universe) {
        Ok(identity) => identity,
        Err(CatalogueIdentityResolutionError::AmbiguousIdentifier(_, candidates))
            if candidates
                .as_slice()
                .iter()
                .all(|candidate| candidate.crate_name() == catalogue_crate) =>
        {
            // Keep the parser on the source-spelling path so the final
            // canonicalization pass reports the complete local candidate set.
            candidates.as_slice().first()?.clone()
        }
        Err(_) => return None,
    };
    if identity.crate_name() != catalogue_crate {
        return None;
    }
    rustdoc_paths.iter().find_map(|(id, summary)| {
        summary_identity(summary).filter(|candidate| candidate == &identity).map(|_| *id)
    })
}

pub(super) fn render_identity_type(ty: &Type) -> Option<String> {
    match ty {
        Type::ImplTrait(bounds) => {
            let rendered = bounds.iter().map(render_bound).collect::<Option<Vec<_>>>()?;
            Some(format!("impl {}", rendered.join(" + ")))
        }
        Type::Infer => None,
        Type::Pat { type_, .. } => render_identity_type(type_),
        _ => render_type(ty),
    }
}

pub(super) fn canonicalize_type(
    ty: Type,
    source: &TypeRef,
    catalogue_crate: &CrateName,
    authority: &DefinitionPathAuthority,
    rustdoc_paths: Option<&HashMap<Id, ItemSummary>>,
) -> Result<Type, NewTypeGraphCodecError> {
    match ty {
        Type::ResolvedPath(path) => {
            let args =
                canonicalize_args(path.args, source, catalogue_crate, authority, rustdoc_paths)?;
            let path_name = canonicalize_path(
                &path.path,
                source,
                catalogue_crate,
                authority,
                rustdoc_paths.map(|_| path.id),
                rustdoc_paths,
            )?;
            Ok(Type::ResolvedPath(Path { path: path_name, id: path.id, args }))
        }
        Type::Tuple(elements) => Ok(Type::Tuple(
            elements
                .into_iter()
                .map(|element| {
                    canonicalize_type(element, source, catalogue_crate, authority, rustdoc_paths)
                })
                .collect::<Result<Vec<_>, _>>()?,
        )),
        Type::Slice(inner) => Ok(Type::Slice(Box::new(canonicalize_type(
            *inner,
            source,
            catalogue_crate,
            authority,
            rustdoc_paths,
        )?))),
        Type::Array { type_, len } => Ok(Type::Array {
            type_: Box::new(canonicalize_type(
                *type_,
                source,
                catalogue_crate,
                authority,
                rustdoc_paths,
            )?),
            len,
        }),
        Type::Pat { type_, __pat_unstable_do_not_use } => Ok(Type::Pat {
            type_: Box::new(canonicalize_type(
                *type_,
                source,
                catalogue_crate,
                authority,
                rustdoc_paths,
            )?),
            __pat_unstable_do_not_use,
        }),
        Type::BorrowedRef { lifetime, is_mutable, type_ } => Ok(Type::BorrowedRef {
            lifetime,
            is_mutable,
            type_: Box::new(canonicalize_type(
                *type_,
                source,
                catalogue_crate,
                authority,
                rustdoc_paths,
            )?),
        }),
        Type::RawPointer { is_mutable, type_ } => Ok(Type::RawPointer {
            is_mutable,
            type_: Box::new(canonicalize_type(
                *type_,
                source,
                catalogue_crate,
                authority,
                rustdoc_paths,
            )?),
        }),
        Type::ImplTrait(bounds) => Ok(Type::ImplTrait(
            bounds
                .into_iter()
                .map(|bound| {
                    canonicalize_bound(bound, source, catalogue_crate, authority, rustdoc_paths)
                })
                .collect::<Result<Vec<_>, _>>()?,
        )),
        Type::DynTrait(dyn_trait) => Ok(Type::DynTrait(DynTrait {
            traits: dyn_trait
                .traits
                .into_iter()
                .map(|poly| {
                    let path = canonicalize_path(
                        &poly.trait_.path,
                        source,
                        catalogue_crate,
                        authority,
                        rustdoc_paths.map(|_| poly.trait_.id),
                        rustdoc_paths,
                    )?;
                    let args = canonicalize_args(
                        poly.trait_.args,
                        source,
                        catalogue_crate,
                        authority,
                        rustdoc_paths,
                    )?;
                    Ok(rustdoc_types::PolyTrait {
                        trait_: Path { path, id: poly.trait_.id, args },
                        generic_params: poly.generic_params,
                    })
                })
                .collect::<Result<Vec<_>, NewTypeGraphCodecError>>()?,
            lifetime: dyn_trait.lifetime,
        })),
        Type::FunctionPointer(function_pointer) => {
            let inputs = function_pointer
                .sig
                .inputs
                .into_iter()
                .map(|(name, input)| {
                    Ok((
                        name,
                        canonicalize_type(
                            input,
                            source,
                            catalogue_crate,
                            authority,
                            rustdoc_paths,
                        )?,
                    ))
                })
                .collect::<Result<Vec<_>, NewTypeGraphCodecError>>()?;
            let output = function_pointer
                .sig
                .output
                .map(|output| {
                    canonicalize_type(output, source, catalogue_crate, authority, rustdoc_paths)
                })
                .transpose()?;
            Ok(Type::FunctionPointer(Box::new(rustdoc_types::FunctionPointer {
                sig: rustdoc_types::FunctionSignature {
                    inputs,
                    output,
                    is_c_variadic: function_pointer.sig.is_c_variadic,
                },
                generic_params: function_pointer.generic_params,
                header: function_pointer.header,
            })))
        }
        Type::QualifiedPath { name, args, self_type, trait_ } => Ok(Type::QualifiedPath {
            name,
            args: args
                .map(|args| {
                    canonicalize_args(Some(args), source, catalogue_crate, authority, rustdoc_paths)
                })
                .transpose()?
                .flatten(),
            self_type: Box::new(canonicalize_type(
                *self_type,
                source,
                catalogue_crate,
                authority,
                rustdoc_paths,
            )?),
            trait_: trait_
                .map(|path| {
                    Ok::<Path, NewTypeGraphCodecError>(Path {
                        path: canonicalize_path(
                            &path.path,
                            source,
                            catalogue_crate,
                            authority,
                            rustdoc_paths.map(|_| path.id),
                            rustdoc_paths,
                        )?,
                        id: path.id,
                        args: canonicalize_args(
                            path.args,
                            source,
                            catalogue_crate,
                            authority,
                            rustdoc_paths,
                        )?,
                    })
                })
                .transpose()?,
        }),
        other => Ok(other),
    }
}

fn canonicalize_args(
    args: Option<Box<GenericArgs>>,
    source: &TypeRef,
    catalogue_crate: &CrateName,
    authority: &DefinitionPathAuthority,
    rustdoc_paths: Option<&HashMap<Id, ItemSummary>>,
) -> Result<Option<Box<GenericArgs>>, NewTypeGraphCodecError> {
    args.map(|args| {
        canonicalize_generic_args(*args, source, catalogue_crate, authority, rustdoc_paths)
            .map(Box::new)
    })
    .transpose()
}

pub(super) fn canonicalize_generic_args(
    args: GenericArgs,
    source: &TypeRef,
    catalogue_crate: &CrateName,
    authority: &DefinitionPathAuthority,
    rustdoc_paths: Option<&HashMap<Id, ItemSummary>>,
) -> Result<GenericArgs, NewTypeGraphCodecError> {
    match args {
        GenericArgs::AngleBracketed { args, constraints } => Ok(GenericArgs::AngleBracketed {
            args: args
                .into_iter()
                .map(|arg| canonicalize_arg(arg, source, catalogue_crate, authority, rustdoc_paths))
                .collect::<Result<Vec<_>, _>>()?,
            constraints: constraints
                .into_iter()
                .map(|constraint| {
                    let args = canonicalize_args(
                        constraint.args,
                        source,
                        catalogue_crate,
                        authority,
                        rustdoc_paths,
                    )?;
                    let binding = match constraint.binding {
                        AssocItemConstraintKind::Equality(term) => {
                            AssocItemConstraintKind::Equality(canonicalize_term(
                                term,
                                source,
                                catalogue_crate,
                                authority,
                                rustdoc_paths,
                            )?)
                        }
                        AssocItemConstraintKind::Constraint(bounds) => {
                            AssocItemConstraintKind::Constraint(
                                bounds
                                    .into_iter()
                                    .map(|bound| {
                                        canonicalize_bound(
                                            bound,
                                            source,
                                            catalogue_crate,
                                            authority,
                                            rustdoc_paths,
                                        )
                                    })
                                    .collect::<Result<Vec<_>, _>>()?,
                            )
                        }
                    };
                    Ok(AssocItemConstraint { args, binding, ..constraint })
                })
                .collect::<Result<Vec<_>, NewTypeGraphCodecError>>()?,
        }),
        GenericArgs::Parenthesized { inputs, output } => Ok(GenericArgs::Parenthesized {
            inputs: inputs
                .into_iter()
                .map(|input| {
                    canonicalize_type(input, source, catalogue_crate, authority, rustdoc_paths)
                })
                .collect::<Result<Vec<_>, _>>()?,
            output: output
                .map(|output| {
                    canonicalize_type(output, source, catalogue_crate, authority, rustdoc_paths)
                })
                .transpose()?,
        }),
        GenericArgs::ReturnTypeNotation => Ok(GenericArgs::ReturnTypeNotation),
    }
}

fn canonicalize_arg(
    arg: GenericArg,
    source: &TypeRef,
    catalogue_crate: &CrateName,
    authority: &DefinitionPathAuthority,
    rustdoc_paths: Option<&HashMap<Id, ItemSummary>>,
) -> Result<GenericArg, NewTypeGraphCodecError> {
    match arg {
        GenericArg::Type(ty) => Ok(GenericArg::Type(canonicalize_type(
            ty,
            source,
            catalogue_crate,
            authority,
            rustdoc_paths,
        )?)),
        other => Ok(other),
    }
}

fn canonicalize_bound(
    bound: GenericBound,
    source: &TypeRef,
    catalogue_crate: &CrateName,
    authority: &DefinitionPathAuthority,
    rustdoc_paths: Option<&HashMap<Id, ItemSummary>>,
) -> Result<GenericBound, NewTypeGraphCodecError> {
    match bound {
        GenericBound::TraitBound { trait_, generic_params, modifier } => {
            Ok(GenericBound::TraitBound {
                trait_: Path {
                    path: canonicalize_path(
                        &trait_.path,
                        source,
                        catalogue_crate,
                        authority,
                        rustdoc_paths.map(|_| trait_.id),
                        rustdoc_paths,
                    )?,
                    id: trait_.id,
                    args: canonicalize_args(
                        trait_.args,
                        source,
                        catalogue_crate,
                        authority,
                        rustdoc_paths,
                    )?,
                },
                generic_params,
                modifier,
            })
        }
        other => Ok(other),
    }
}

fn canonicalize_term(
    term: Term,
    source: &TypeRef,
    catalogue_crate: &CrateName,
    authority: &DefinitionPathAuthority,
    rustdoc_paths: Option<&HashMap<Id, ItemSummary>>,
) -> Result<Term, NewTypeGraphCodecError> {
    match term {
        Term::Type(ty) => Ok(Term::Type(canonicalize_type(
            ty,
            source,
            catalogue_crate,
            authority,
            rustdoc_paths,
        )?)),
        other => Ok(other),
    }
}
