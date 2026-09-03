//! Composition-root public-surface checks for `CompositionRootPureDi`.

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use super::super::helpers::{
    canonical_catalogue_identity, canonical_catalogue_trait_identity, collect_methods_for_type,
    declared_trait_identities, declared_type_identities, entry_role_kind, inherent_impls_for_type,
    resolve_catalogue_entry_reference, standard_external_trait_identities, type_entries_for_target,
};
use super::super::identity_helpers::root_path_occurrence;
use super::eval_helpers::sig_type_contains_entry;
use super::{
    CatalogueLintViolation, CatalogueLinterError, CatalogueLinterRule, FreeText, RoleKind,
    TypeRefPathExtractorPort,
};
use crate::tddd::catalogue_v2::CatalogueDocument;
use crate::tddd::catalogue_v2::composite::{StructShape, TypeKindV2};
use crate::tddd::catalogue_v2::entries::TypeEntry;
use crate::tddd::catalogue_v2::identifiers::{
    CrateName, FullyQualifiedItemPath, ParamName, TypeRef,
};
use crate::tddd::catalogue_v2::identity_resolution::{
    CatalogueIdentityResolutionError, resolve_catalogue_identity,
};
use crate::tddd::catalogue_v2::methods::{
    MethodDeclaration, MethodGenericParam, WherePredicateDecl,
};
use crate::tddd::catalogue_v2::roles::ItemAction;
use crate::tddd::catalogue_v2::variants::VariantPayload;
use crate::tddd::layer_id::LayerId;

fn signature_exposes_prohibited_role(
    signature: &str,
    role: RoleKind,
    all_catalogues: &BTreeMap<LayerId, CatalogueDocument>,
    target_layer_id: &LayerId,
) -> bool {
    all_catalogues.iter().any(|(layer_id, catalogue)| {
        catalogue.types().iter().filter(|(_, entry)| entry.action() != ItemAction::Delete).any(
            |(name, entry)| {
                entry_role_kind(entry) == role
                    && !(role == RoleKind::PrimaryAdapter
                        || (role == RoleKind::ErrorType && layer_id == target_layer_id))
                    && sig_type_contains_entry(
                        signature,
                        name.as_str(),
                        layer_id,
                        target_layer_id,
                        all_catalogues,
                    )
            },
        ) || catalogue
            .traits()
            .iter()
            .filter(|(_, entry)| entry.action() != ItemAction::Delete)
            .any(|(name, entry)| {
                RoleKind::from_contract_role(entry.role()) == role
                    && sig_type_contains_entry(
                        signature,
                        name.as_str(),
                        layer_id,
                        target_layer_id,
                        all_catalogues,
                    )
            })
    })
}

fn signature_exposes_role(
    signature: &str,
    role: RoleKind,
    all_catalogues: &BTreeMap<LayerId, CatalogueDocument>,
    target_layer_id: &LayerId,
) -> bool {
    all_catalogues.iter().any(|(layer_id, catalogue)| {
        catalogue.types().iter().filter(|(_, entry)| entry.action() != ItemAction::Delete).any(
            |(name, entry)| {
                entry_role_kind(entry) == role
                    && sig_type_contains_entry(
                        signature,
                        name.as_str(),
                        layer_id,
                        target_layer_id,
                        all_catalogues,
                    )
            },
        )
    })
}

fn collect_public_methods<'a, E: TypeRefPathExtractorPort>(
    catalogue: &'a CatalogueDocument,
    all_catalogues: &'a BTreeMap<LayerId, CatalogueDocument>,
    entry: &'a TypeEntry,
    type_name: &str,
    trait_identities: &BTreeSet<FullyQualifiedItemPath>,
    extractor: &E,
) -> Result<Vec<&'a MethodDeclaration>, CatalogueLinterError> {
    let mut methods = collect_methods_for_type(catalogue, entry, type_name)?;
    for trait_impl in trait_impls_for_type(catalogue, entry, type_name, extractor)? {
        let type_parameters = impl_type_parameters(trait_impl);
        let Some(trait_identity) = resolve_declared_trait_identity(
            catalogue,
            trait_impl.trait_ref(),
            trait_identities,
            extractor,
            &type_parameters,
        )?
        else {
            continue;
        };
        for trait_catalogue in all_catalogues.values() {
            for (trait_name, trait_entry) in trait_catalogue.traits().iter() {
                if trait_entry.action() == ItemAction::Delete {
                    continue;
                }
                if canonical_catalogue_trait_identity(
                    trait_catalogue,
                    trait_name.as_str(),
                    trait_entry.module_path(),
                )? == trait_identity
                {
                    methods.extend(trait_entry.methods().iter());
                }
            }
        }
    }
    Ok(methods)
}

fn trait_impls_for_type<'a, E: TypeRefPathExtractorPort>(
    catalogue: &'a CatalogueDocument,
    entry: &'a TypeEntry,
    type_name: &str,
    extractor: &E,
) -> Result<Vec<&'a crate::tddd::catalogue_v2::traits::TraitImplDeclV2>, CatalogueLinterError> {
    let type_identity = canonical_catalogue_identity(catalogue, type_name, entry.module_path())?;
    let type_identities = declared_type_identities(catalogue)?;
    let mut matching = Vec::new();

    for trait_impl in catalogue.trait_impls() {
        if trait_impl.action() == ItemAction::Delete {
            continue;
        }
        let type_parameters = impl_type_parameters(trait_impl);
        let owner = root_path_occurrence(trait_impl.for_type(), extractor, &type_parameters)?;
        match resolve_catalogue_entry_reference(catalogue, owner.as_str(), &type_identities, true) {
            Ok(Some(owner_identity)) if owner_identity == type_identity => {
                matching.push(trait_impl);
            }
            Ok(_) => {}
            Err(CatalogueLinterError::IdentityResolutionFailed(
                CatalogueIdentityResolutionError::UnresolvedIdentifier(_),
            )) => {}
            Err(error) => return Err(error),
        }
    }

    Ok(matching)
}

fn declared_trait_identity_universe(
    all_catalogues: &BTreeMap<LayerId, CatalogueDocument>,
) -> Result<BTreeSet<FullyQualifiedItemPath>, CatalogueLinterError> {
    let mut identities = BTreeSet::new();
    for catalogue in all_catalogues.values() {
        identities.extend(declared_trait_identities(catalogue)?);
    }
    Ok(identities)
}

fn impl_type_parameters(
    trait_impl: &crate::tddd::catalogue_v2::traits::TraitImplDeclV2,
) -> Vec<ParamName> {
    trait_impl.impl_generics().iter().map(|generic| generic.name.clone()).collect()
}

fn resolve_declared_trait_identity<E: TypeRefPathExtractorPort>(
    catalogue: &CatalogueDocument,
    trait_ref: &TypeRef,
    trait_identities: &BTreeSet<FullyQualifiedItemPath>,
    extractor: &E,
    type_parameters: &[ParamName],
) -> Result<Option<FullyQualifiedItemPath>, CatalogueLinterError> {
    let path = root_path_occurrence(trait_ref, extractor, type_parameters)?;
    match resolve_catalogue_identity(&path, catalogue.crate_name(), trait_identities) {
        Ok(identity) => Ok(Some(identity)),
        Err(CatalogueIdentityResolutionError::UnresolvedIdentifier(_)) => Ok(None),
        Err(error) => Err(CatalogueLinterError::IdentityResolutionFailed(error)),
    }
}

fn trait_impl_has_resolved_catalogue_entry<E: TypeRefPathExtractorPort>(
    trait_impl: &crate::tddd::catalogue_v2::traits::TraitImplDeclV2,
    catalogue: &CatalogueDocument,
    trait_identities: &BTreeSet<FullyQualifiedItemPath>,
    extractor: &E,
) -> Result<bool, CatalogueLinterError> {
    let type_parameters = impl_type_parameters(trait_impl);
    Ok(resolve_declared_trait_identity(
        catalogue,
        trait_impl.trait_ref(),
        trait_identities,
        extractor,
        &type_parameters,
    )?
    .is_some())
}

const NON_EXECUTION_TRAIT_PATHS: &str = concat!(
    "core::clone::Clone ",
    "core::cmp::PartialEq core::cmp::Eq core::cmp::PartialOrd core::cmp::Ord ",
    "core::default::Default core::fmt::Debug core::hash::Hash ",
    "core::convert::From core::convert::TryFrom",
);

fn known_non_execution_trait_identities(
    catalogue_crate: &CrateName,
) -> Result<BTreeSet<FullyQualifiedItemPath>, CatalogueLinterError> {
    let standard_identities = standard_external_trait_identities()?;
    let mut identities = BTreeSet::new();
    for path in NON_EXECUTION_TRAIT_PATHS.split_whitespace() {
        let reference = TypeRef::new(path.to_owned()).map_err(|error| {
            CatalogueLinterError::InvalidRuleConfig(FreeText::new(format!(
                "invalid non-execution trait path '{path}': {error}"
            )))
        })?;
        let identity =
            resolve_catalogue_identity(&reference, catalogue_crate, &standard_identities).map_err(
                |error| {
                    CatalogueLinterError::InvalidRuleConfig(FreeText::new(format!(
                        "invalid non-execution trait path '{path}': {error}"
                    )))
                },
            )?;
        identities.insert(identity);
    }
    Ok(identities)
}

fn resolves_to_known_non_execution_trait<E: TypeRefPathExtractorPort>(
    catalogue: &CatalogueDocument,
    trait_ref: &TypeRef,
    known_identities: &BTreeSet<FullyQualifiedItemPath>,
    extractor: &E,
    type_parameters: &[ParamName],
) -> Result<bool, CatalogueLinterError> {
    let path = root_path_occurrence(trait_ref, extractor, type_parameters)?;
    match resolve_catalogue_identity(&path, catalogue.crate_name(), known_identities) {
        Ok(identity) => Ok(known_identities.contains(&identity)),
        Err(CatalogueIdentityResolutionError::UnresolvedIdentifier(_)) => Ok(false),
        Err(error) => Err(CatalogueLinterError::IdentityResolutionFailed(error)),
    }
}

fn generic_bound_types<'a>(
    generics: &'a [MethodGenericParam],
    where_predicates: &'a [WherePredicateDecl],
) -> Vec<&'a str> {
    generics
        .iter()
        .flat_map(|generic| generic.bounds.iter().map(|bound| bound.as_str()))
        .chain(where_predicates.iter().flat_map(|predicate| {
            std::iter::once(predicate.lhs.as_str())
                .chain(predicate.rhs.iter().map(|bound| bound.as_str()))
        }))
        .collect()
}

fn type_surface_types<'a>(
    catalogue: &'a CatalogueDocument,
    entry: &'a TypeEntry,
    type_name: &str,
    extractor: &impl TypeRefPathExtractorPort,
) -> Result<Vec<&'a str>, CatalogueLinterError> {
    let mut types = match entry.kind() {
        TypeKindV2::Struct(struct_kind) => match &struct_kind.shape {
            StructShape::Unit => vec![],
            StructShape::Plain { fields, .. } => {
                fields.iter().map(|field| field.ty.as_str()).collect()
            }
            StructShape::Tuple { fields, .. } => {
                fields.iter().map(|field| field.as_str()).collect()
            }
        },
        TypeKindV2::Enum { variants } => variants
            .iter()
            .flat_map(|variant| match &variant.payload {
                VariantPayload::Unit => vec![],
                VariantPayload::Tuple(fields) => {
                    fields.iter().map(|field| field.as_str()).collect()
                }
                VariantPayload::Struct(fields) => {
                    fields.iter().map(|field| field.ty.as_str()).collect()
                }
            })
            .collect(),
        TypeKindV2::TypeAlias { target, generics } => {
            let mut types = vec![target.as_str()];
            types.extend(generic_bound_types(generics, &[]));
            types
        }
    };
    types.extend(generic_bound_types(entry.generics(), entry.where_predicates()));
    for inherent_impl in inherent_impls_for_type(catalogue, entry, type_name)? {
        types.extend(generic_bound_types(
            inherent_impl.impl_generics(),
            inherent_impl.impl_where_predicates(),
        ));
    }
    for trait_impl in trait_impls_for_type(catalogue, entry, type_name, extractor)? {
        types.push(trait_impl.trait_ref().as_str());
        types.extend(generic_bound_types(
            trait_impl.impl_generics(),
            trait_impl.impl_where_predicates(),
        ));
    }
    Ok(types)
}

fn method_surface_types(method: &MethodDeclaration) -> Vec<&str> {
    let mut types: Vec<&str> = method
        .params
        .iter()
        .map(|param| param.ty.as_str())
        .chain(std::iter::once(method.returns.as_str()))
        .collect();
    types.extend(generic_bound_types(
        method.generics.as_slice(),
        method.where_predicates.as_slice(),
    ));
    types
}

fn is_wiring_constructor(method: &MethodDeclaration) -> bool {
    let return_type = method.returns.as_str().replace(char::is_whitespace, "");
    method.receiver.is_none()
        && (return_type == "Self"
            || return_type.starts_with("Result<Self,")
            || return_type.starts_with("std::result::Result<Self,"))
}

pub(super) fn evaluate_composition_root_pure_di<E: TypeRefPathExtractorPort>(
    rule: &CatalogueLinterRule,
    catalogue: &CatalogueDocument,
    all_catalogues: &BTreeMap<LayerId, CatalogueDocument>,
    target_layer_id: &LayerId,
    extractor: &E,
) -> Result<Vec<CatalogueLintViolation>, CatalogueLinterError> {
    let mut violations = Vec::new();
    let trait_identities = declared_trait_identity_universe(all_catalogues)?;
    let known_non_execution_traits = known_non_execution_trait_identities(catalogue.crate_name())?;
    for (name, entry) in type_entries_for_target(catalogue, rule.target()) {
        for signature in type_surface_types(catalogue, entry, name.as_str(), extractor)? {
            for role in RoleKind::all().iter().copied() {
                if signature_exposes_prohibited_role(
                    signature,
                    role,
                    all_catalogues,
                    target_layer_id,
                ) {
                    violations.push(CatalogueLintViolation::new(
                        rule.kind().discriminant_name(),
                        name.as_str(),
                        format!(
                            "CompositionRoot exposes prohibited public-surface role '{}' in field type '{}'",
                            role.variant_name(),
                            signature
                        ),
                    ));
                }
            }
        }

        for trait_impl in trait_impls_for_type(catalogue, entry, name.as_str(), extractor)? {
            let type_parameters = impl_type_parameters(trait_impl);
            if !resolves_to_known_non_execution_trait(
                catalogue,
                trait_impl.trait_ref(),
                &known_non_execution_traits,
                extractor,
                &type_parameters,
            )? && !trait_impl_has_resolved_catalogue_entry(
                trait_impl,
                catalogue,
                &trait_identities,
                extractor,
            )? {
                violations.push(CatalogueLintViolation::new(
                    rule.kind().discriminant_name(),
                    name.as_str(),
                    format!(
                        "CompositionRoot implements unresolved trait '{}'; catalogue method surface is required for pure-DI validation",
                        trait_impl.trait_ref().as_str()
                    ),
                ));
            }
        }

        let all_methods = collect_public_methods(
            catalogue,
            all_catalogues,
            entry,
            name.as_str(),
            &trait_identities,
            extractor,
        )?;
        for method in all_methods {
            let returns_primary_adapter = signature_exposes_role(
                method.returns.as_str(),
                RoleKind::PrimaryAdapter,
                all_catalogues,
                target_layer_id,
            );
            let is_constructor = is_wiring_constructor(method);

            if !is_constructor && !returns_primary_adapter {
                violations.push(CatalogueLintViolation::new(
                    rule.kind().discriminant_name(),
                    name.as_str(),
                    format!(
                        "method '{}' is an execution method; CompositionRoot methods must be zero-argument PrimaryAdapter wiring accessors",
                        method.name.as_str()
                    ),
                ));
            }

            for signature in method_surface_types(method) {
                for role in RoleKind::all().iter().copied() {
                    if signature_exposes_prohibited_role(
                        signature,
                        role,
                        all_catalogues,
                        target_layer_id,
                    ) {
                        violations.push(CatalogueLintViolation::new(
                            rule.kind().discriminant_name(),
                            name.as_str(),
                            format!(
                                "method '{}' exposes prohibited public-surface role '{}' in signature type '{}'",
                                method.name.as_str(),
                                role.variant_name(),
                                signature
                            ),
                        ));
                    }
                }
            }
        }
    }
    Ok(violations)
}
