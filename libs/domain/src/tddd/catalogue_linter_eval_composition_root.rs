//! Composition-root public-surface checks for `CompositionRootPureDi`.

use std::collections::BTreeMap;

use super::super::helpers::{
    bare_name_in_type_ref, collect_methods_for_type, entry_role_kind, type_entries_for_target,
};
use super::eval_helpers::sig_type_contains_entry;
use super::{CatalogueLintViolation, CatalogueLinterError, CatalogueLinterRule, RoleKind};
use crate::tddd::catalogue_v2::CatalogueDocument;
use crate::tddd::catalogue_v2::composite::{StructShape, TypeKindV2};
use crate::tddd::catalogue_v2::entries::TypeEntry;
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

fn collect_public_methods<'a>(
    catalogue: &'a CatalogueDocument,
    all_catalogues: &'a BTreeMap<LayerId, CatalogueDocument>,
    entry: &'a TypeEntry,
    type_name: &str,
) -> Result<Vec<&'a MethodDeclaration>, CatalogueLinterError> {
    let mut methods = collect_methods_for_type(catalogue, entry, type_name)?;
    for trait_impl in trait_impls_for_type(catalogue, type_name) {
        for trait_catalogue in all_catalogues.values() {
            for (_trait_name, trait_entry) in
                trait_catalogue.traits().iter().filter(|(trait_name, entry)| {
                    entry.action() != ItemAction::Delete
                        && bare_name_in_type_ref(
                            trait_impl.trait_ref().as_str(),
                            trait_name.as_str(),
                        )
                })
            {
                methods.extend(trait_entry.methods().iter());
            }
        }
    }
    Ok(methods)
}

fn trait_impls_for_type<'a>(
    catalogue: &'a CatalogueDocument,
    type_name: &str,
) -> impl Iterator<Item = &'a crate::tddd::catalogue_v2::traits::TraitImplDeclV2> {
    catalogue.trait_impls().iter().filter(move |trait_impl| {
        if trait_impl.action() == ItemAction::Delete {
            return false;
        }
        let self_type = trait_impl.for_type().as_str().replace(char::is_whitespace, "");
        self_type == type_name || self_type.starts_with(&format!("{type_name}<"))
    })
}

fn trait_impl_has_resolved_catalogue_entry(
    trait_impl: &crate::tddd::catalogue_v2::traits::TraitImplDeclV2,
    all_catalogues: &BTreeMap<LayerId, CatalogueDocument>,
) -> bool {
    all_catalogues.values().any(|catalogue| {
        catalogue.traits().iter().any(|(trait_name, entry)| {
            entry.action() != ItemAction::Delete
                && bare_name_in_type_ref(trait_impl.trait_ref().as_str(), trait_name.as_str())
        })
    })
}

fn is_known_non_execution_trait(trait_ref: &str) -> bool {
    let trait_name = trait_ref.split('<').next().unwrap_or(trait_ref).trim();
    matches!(
        trait_name,
        "core::clone::Clone"
            | "std::clone::Clone"
            | "core::cmp::PartialEq"
            | "std::cmp::PartialEq"
            | "core::cmp::Eq"
            | "std::cmp::Eq"
            | "core::cmp::PartialOrd"
            | "std::cmp::PartialOrd"
            | "core::cmp::Ord"
            | "std::cmp::Ord"
            | "core::default::Default"
            | "std::default::Default"
            | "core::fmt::Debug"
            | "std::fmt::Debug"
            | "core::hash::Hash"
            | "std::hash::Hash"
            | "core::convert::From"
            | "std::convert::From"
            | "core::convert::TryFrom"
            | "std::convert::TryFrom"
    )
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
) -> Vec<&'a str> {
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
        TypeKindV2::TypeAlias { target } => vec![target.as_str()],
    };
    types.extend(generic_bound_types(entry.generics(), entry.where_predicates()));
    for inherent_impl in catalogue
        .inherent_impls()
        .iter()
        .filter(|inherent_impl| inherent_impl.type_name.as_str() == type_name)
    {
        types.extend(generic_bound_types(
            inherent_impl.impl_generics.as_slice(),
            inherent_impl.impl_where_predicates.as_slice(),
        ));
    }
    for trait_impl in trait_impls_for_type(catalogue, type_name) {
        types.push(trait_impl.trait_ref().as_str());
        types.extend(generic_bound_types(
            trait_impl.impl_generics(),
            trait_impl.impl_where_predicates(),
        ));
    }
    types
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

pub(super) fn evaluate_composition_root_pure_di(
    rule: &CatalogueLinterRule,
    catalogue: &CatalogueDocument,
    all_catalogues: &BTreeMap<LayerId, CatalogueDocument>,
    target_layer_id: &LayerId,
) -> Result<Vec<CatalogueLintViolation>, CatalogueLinterError> {
    let mut violations = Vec::new();
    for (name, entry) in type_entries_for_target(catalogue, rule.target()) {
        for signature in type_surface_types(catalogue, entry, name.as_str()) {
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

        for trait_impl in trait_impls_for_type(catalogue, name.as_str()) {
            if !is_known_non_execution_trait(trait_impl.trait_ref().as_str())
                && !trait_impl_has_resolved_catalogue_entry(trait_impl, all_catalogues)
            {
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

        let all_methods = collect_public_methods(catalogue, all_catalogues, entry, name.as_str())?;
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
