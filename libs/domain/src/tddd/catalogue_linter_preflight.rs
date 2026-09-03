//! Rule-independent TypeRef inspection for catalogue-lint.
//!
//! The catalogue-lint rule evaluator is intentionally selective: each rule
//! examines only the slots needed for its own predicate. This preflight owns
//! the separate completion contract and therefore walks every TypeRef-bearing
//! catalogue slot before any rule is evaluated. Extraction and identity
//! classification remain delegated to the existing T013 adapter boundary.

use std::collections::BTreeMap;

use super::eval::{
    CatalogueTypeRefIdentityContext, build_type_ref_identity_context, inspect_type_ref,
};
use super::{CatalogueLinterError, FreeText, TypeRefPathExtractorPort};
use crate::tddd::catalogue_v2::CatalogueDocument;
use crate::tddd::catalogue_v2::composite::{StructShape, TypeKindV2};
use crate::tddd::catalogue_v2::entries::{
    AssocConstDecl, AssocTypeDecl, FunctionEntry, InherentImplDeclV2, TraitEntry, TypeEntry,
};
use crate::tddd::catalogue_v2::identifiers::{CatalogueItemNamespace, ParamName, TypeRef};
use crate::tddd::catalogue_v2::methods::{
    BoundOp, MethodDeclaration, MethodGenericParam, ParamDeclaration, WherePredicateDecl,
};
use crate::tddd::catalogue_v2::roles::{ContractRole, DataRole, ItemAction};
use crate::tddd::catalogue_v2::traits::TraitImplDeclV2;
use crate::tddd::catalogue_v2::variants::{FieldDecl, VariantPayload};
use crate::tddd::layer_id::LayerId;

pub(super) fn inspect_all_catalogue_type_refs<E: TypeRefPathExtractorPort>(
    all_catalogues: &BTreeMap<LayerId, CatalogueDocument>,
    extractor: &E,
) -> Result<(), CatalogueLinterError> {
    for catalogue in all_catalogues.values() {
        let context = build_type_ref_identity_context(all_catalogues, catalogue.crate_name())?;
        inspect_catalogue(catalogue, &context, extractor)?;
    }
    Ok(())
}

fn inspect_catalogue<E: TypeRefPathExtractorPort>(
    catalogue: &CatalogueDocument,
    context: &CatalogueTypeRefIdentityContext,
    extractor: &E,
) -> Result<(), CatalogueLinterError> {
    for entry in catalogue.types().values().filter(|entry| is_live(entry.action())) {
        inspect_type_entry(entry, context, extractor)?;
    }
    for entry in catalogue.traits().values().filter(|entry| is_live(entry.action())) {
        inspect_trait_entry(entry, context, extractor)?;
    }
    for entry in catalogue.functions().values().filter(|entry| is_live(entry.action())) {
        inspect_function_entry(entry, context, extractor)?;
    }
    for impl_decl in catalogue.trait_impls().iter().filter(|decl| is_live(decl.action())) {
        inspect_trait_impl(impl_decl, context, extractor)?;
    }
    for impl_decl in catalogue.inherent_impls() {
        inspect_inherent_impl(impl_decl, context, extractor)?;
    }
    Ok(())
}

fn is_live(action: ItemAction) -> bool {
    matches!(action, ItemAction::Add | ItemAction::Modify | ItemAction::Reference)
}

fn inspect_type_entry<E: TypeRefPathExtractorPort>(
    entry: &TypeEntry,
    context: &CatalogueTypeRefIdentityContext,
    extractor: &E,
) -> Result<(), CatalogueLinterError> {
    let mut type_parameters = parameter_names(entry.generics());
    inspect_type_kind(entry.kind(), &mut type_parameters, context, extractor)?;
    inspect_generic_bounds(entry.generics(), &type_parameters, context, extractor)?;
    inspect_where_predicates(entry.where_predicates(), &type_parameters, context, extractor)?;
    inspect_data_role(entry.role(), &type_parameters, context, extractor)?;
    inspect_methods(entry.methods(), &type_parameters, context, extractor)
}

fn inspect_type_kind<E: TypeRefPathExtractorPort>(
    kind: &TypeKindV2,
    type_parameters: &mut Vec<ParamName>,
    context: &CatalogueTypeRefIdentityContext,
    extractor: &E,
) -> Result<(), CatalogueLinterError> {
    match kind {
        TypeKindV2::Struct(struct_kind) => match &struct_kind.shape {
            StructShape::Unit => {}
            StructShape::Tuple { fields, .. } => {
                for type_ref in fields {
                    inspect(
                        type_ref,
                        type_parameters,
                        context,
                        CatalogueItemNamespace::Type,
                        extractor,
                    )?;
                }
            }
            StructShape::Plain { fields, .. } => {
                for field in fields {
                    inspect_field(field, type_parameters, context, extractor)?;
                }
            }
        },
        TypeKindV2::Enum { variants } => {
            for variant in variants {
                match &variant.payload {
                    VariantPayload::Unit => {}
                    VariantPayload::Tuple(fields) => {
                        for type_ref in fields {
                            inspect(
                                type_ref,
                                type_parameters,
                                context,
                                CatalogueItemNamespace::Type,
                                extractor,
                            )?;
                        }
                    }
                    VariantPayload::Struct(fields) => {
                        for field in fields {
                            inspect_field(field, type_parameters, context, extractor)?;
                        }
                    }
                }
            }
        }
        TypeKindV2::TypeAlias { target, generics } => {
            extend_parameter_names(type_parameters, generics);
            inspect(target, type_parameters, context, CatalogueItemNamespace::Type, extractor)?;
            inspect_generic_bounds(generics, type_parameters, context, extractor)?;
        }
    }
    Ok(())
}

fn inspect_trait_entry<E: TypeRefPathExtractorPort>(
    entry: &TraitEntry,
    context: &CatalogueTypeRefIdentityContext,
    extractor: &E,
) -> Result<(), CatalogueLinterError> {
    let type_parameters = parameter_names(entry.generics());
    inspect_generic_bounds(entry.generics(), &type_parameters, context, extractor)?;
    inspect_where_predicates(entry.where_predicates(), &type_parameters, context, extractor)?;
    for bound in entry.supertrait_bounds() {
        inspect(bound, &type_parameters, context, CatalogueItemNamespace::Trait, extractor)?;
    }
    for assoc_type in entry.assoc_types() {
        inspect_assoc_type(assoc_type, &type_parameters, context, extractor)?;
    }
    for assoc_const in entry.assoc_consts() {
        inspect_assoc_const(assoc_const, &type_parameters, context, extractor)?;
    }
    inspect_contract_role(entry.role(), &type_parameters, context, extractor)?;
    inspect_methods(entry.methods(), &type_parameters, context, extractor)
}

fn inspect_function_entry<E: TypeRefPathExtractorPort>(
    entry: &FunctionEntry,
    context: &CatalogueTypeRefIdentityContext,
    extractor: &E,
) -> Result<(), CatalogueLinterError> {
    let type_parameters = parameter_names(entry.generics());
    inspect_generic_bounds(entry.generics(), &type_parameters, context, extractor)?;
    inspect_where_predicates(entry.where_predicates(), &type_parameters, context, extractor)?;
    inspect_params_and_return(entry.params(), entry.returns(), &type_parameters, context, extractor)
}

fn inspect_trait_impl<E: TypeRefPathExtractorPort>(
    impl_decl: &TraitImplDeclV2,
    context: &CatalogueTypeRefIdentityContext,
    extractor: &E,
) -> Result<(), CatalogueLinterError> {
    let type_parameters = parameter_names(impl_decl.impl_generics());
    inspect(
        impl_decl.trait_ref(),
        &type_parameters,
        context,
        CatalogueItemNamespace::Trait,
        extractor,
    )?;
    inspect(
        impl_decl.for_type(),
        &type_parameters,
        context,
        CatalogueItemNamespace::Type,
        extractor,
    )?;
    inspect_generic_bounds(impl_decl.impl_generics(), &type_parameters, context, extractor)?;
    inspect_where_predicates(
        impl_decl.impl_where_predicates(),
        &type_parameters,
        context,
        extractor,
    )
}

fn inspect_inherent_impl<E: TypeRefPathExtractorPort>(
    impl_decl: &InherentImplDeclV2,
    context: &CatalogueTypeRefIdentityContext,
    extractor: &E,
) -> Result<(), CatalogueLinterError> {
    let type_parameters = parameter_names(impl_decl.impl_generics());
    let owner = TypeRef::new(impl_decl.type_name().as_str().to_owned()).map_err(|error| {
        CatalogueLinterError::InvalidRuleConfig(FreeText::new(format!(
            "invalid inherent_impl type_name '{}': {error}",
            impl_decl.type_name().as_str()
        )))
    })?;
    inspect(&owner, &type_parameters, context, CatalogueItemNamespace::Type, extractor)?;
    inspect_generic_bounds(impl_decl.impl_generics(), &type_parameters, context, extractor)?;
    inspect_where_predicates(
        impl_decl.impl_where_predicates(),
        &type_parameters,
        context,
        extractor,
    )?;
    inspect_methods(impl_decl.methods(), &type_parameters, context, extractor)
}

fn inspect_methods<E: TypeRefPathExtractorPort>(
    methods: &[MethodDeclaration],
    parent_parameters: &[ParamName],
    context: &CatalogueTypeRefIdentityContext,
    extractor: &E,
) -> Result<(), CatalogueLinterError> {
    for method in methods {
        let mut type_parameters = parent_parameters.to_vec();
        extend_parameter_names(&mut type_parameters, method.generics());
        inspect_params_and_return(
            method.params(),
            method.returns(),
            &type_parameters,
            context,
            extractor,
        )?;
        inspect_generic_bounds(method.generics(), &type_parameters, context, extractor)?;
        inspect_where_predicates(method.where_predicates(), &type_parameters, context, extractor)?;
    }
    Ok(())
}

fn inspect_params_and_return<E: TypeRefPathExtractorPort>(
    params: &[ParamDeclaration],
    returns: &TypeRef,
    type_parameters: &[ParamName],
    context: &CatalogueTypeRefIdentityContext,
    extractor: &E,
) -> Result<(), CatalogueLinterError> {
    for param in params {
        inspect(&param.ty, type_parameters, context, CatalogueItemNamespace::Type, extractor)?;
    }
    inspect(returns, type_parameters, context, CatalogueItemNamespace::Type, extractor)
}

fn inspect_generic_bounds<E: TypeRefPathExtractorPort>(
    generics: &[MethodGenericParam],
    type_parameters: &[ParamName],
    context: &CatalogueTypeRefIdentityContext,
    extractor: &E,
) -> Result<(), CatalogueLinterError> {
    for generic in generics {
        for bound in &generic.bounds {
            inspect(bound, type_parameters, context, CatalogueItemNamespace::Trait, extractor)?;
        }
    }
    Ok(())
}

fn inspect_where_predicates<E: TypeRefPathExtractorPort>(
    predicates: &[WherePredicateDecl],
    type_parameters: &[ParamName],
    context: &CatalogueTypeRefIdentityContext,
    extractor: &E,
) -> Result<(), CatalogueLinterError> {
    for predicate in predicates {
        inspect(&predicate.lhs, type_parameters, context, CatalogueItemNamespace::Type, extractor)?;
        let rhs_namespace = match predicate.operator {
            BoundOp::Bound => CatalogueItemNamespace::Trait,
            BoundOp::Equal => CatalogueItemNamespace::Type,
        };
        for bound in &predicate.rhs {
            inspect(bound, type_parameters, context, rhs_namespace, extractor)?;
        }
    }
    Ok(())
}

fn inspect_assoc_type<E: TypeRefPathExtractorPort>(
    assoc_type: &AssocTypeDecl,
    type_parameters: &[ParamName],
    context: &CatalogueTypeRefIdentityContext,
    extractor: &E,
) -> Result<(), CatalogueLinterError> {
    for bound in &assoc_type.bounds {
        inspect(bound, type_parameters, context, CatalogueItemNamespace::Trait, extractor)?;
    }
    if let Some(default) = &assoc_type.default {
        inspect(default, type_parameters, context, CatalogueItemNamespace::Type, extractor)?;
    }
    Ok(())
}

fn inspect_assoc_const<E: TypeRefPathExtractorPort>(
    assoc_const: &AssocConstDecl,
    type_parameters: &[ParamName],
    context: &CatalogueTypeRefIdentityContext,
    extractor: &E,
) -> Result<(), CatalogueLinterError> {
    inspect(&assoc_const.ty, type_parameters, context, CatalogueItemNamespace::Type, extractor)
}

fn inspect_field<E: TypeRefPathExtractorPort>(
    field: &FieldDecl,
    type_parameters: &[ParamName],
    context: &CatalogueTypeRefIdentityContext,
    extractor: &E,
) -> Result<(), CatalogueLinterError> {
    inspect(&field.ty, type_parameters, context, CatalogueItemNamespace::Type, extractor)
}

fn inspect_data_role<E: TypeRefPathExtractorPort>(
    role: &DataRole,
    type_parameters: &[ParamName],
    context: &CatalogueTypeRefIdentityContext,
    extractor: &E,
) -> Result<(), CatalogueLinterError> {
    match role {
        DataRole::AggregateRoot { exclusive_members, shared_value_objects, emits, .. } => {
            inspect_all(exclusive_members, type_parameters, context, extractor)?;
            inspect_all(shared_value_objects, type_parameters, context, extractor)?;
            inspect_all(emits, type_parameters, context, extractor)?;
        }
        DataRole::DomainService { emits } => {
            inspect_all(emits, type_parameters, context, extractor)?;
        }
        DataRole::UseCase { handles } => {
            inspect_all(handles, type_parameters, context, extractor)?;
        }
        DataRole::EventPolicy { reacts_to } => {
            inspect_all(reacts_to.as_slice(), type_parameters, context, extractor)?;
        }
        DataRole::ValueObject { .. }
        | DataRole::Entity { .. }
        | DataRole::Specification
        | DataRole::Factory
        | DataRole::Interactor
        | DataRole::Command
        | DataRole::Query
        | DataRole::Dto
        | DataRole::ErrorType
        | DataRole::SecondaryAdapter
        | DataRole::DomainEvent
        | DataRole::CompositionRoot
        | DataRole::PrimaryAdapter => {}
    }
    Ok(())
}

fn inspect_contract_role<E: TypeRefPathExtractorPort>(
    role: &ContractRole,
    type_parameters: &[ParamName],
    context: &CatalogueTypeRefIdentityContext,
    extractor: &E,
) -> Result<(), CatalogueLinterError> {
    if let ContractRole::Repository { aggregate } = role {
        inspect(aggregate, type_parameters, context, CatalogueItemNamespace::Type, extractor)?;
    }
    Ok(())
}

fn inspect_all<E: TypeRefPathExtractorPort>(
    type_refs: &[TypeRef],
    type_parameters: &[ParamName],
    context: &CatalogueTypeRefIdentityContext,
    extractor: &E,
) -> Result<(), CatalogueLinterError> {
    for type_ref in type_refs {
        inspect(type_ref, type_parameters, context, CatalogueItemNamespace::Type, extractor)?;
    }
    Ok(())
}

fn inspect<E: TypeRefPathExtractorPort>(
    type_ref: &TypeRef,
    type_parameters: &[ParamName],
    context: &CatalogueTypeRefIdentityContext,
    namespace: CatalogueItemNamespace,
    extractor: &E,
) -> Result<(), CatalogueLinterError> {
    inspect_type_ref(context, type_ref, type_parameters, &[], &[], namespace, extractor)
}

fn parameter_names(generics: &[MethodGenericParam]) -> Vec<ParamName> {
    generics.iter().map(|generic| generic.name.clone()).collect()
}

fn extend_parameter_names(parameters: &mut Vec<ParamName>, generics: &[MethodGenericParam]) {
    for generic in generics {
        if !parameters.contains(&generic.name) {
            parameters.push(generic.name.clone());
        }
    }
}

#[cfg(test)]
#[path = "catalogue_linter_preflight_tests.rs"]
mod tests;
