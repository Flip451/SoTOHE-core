//! Domain ValueObject inbound-reference evaluation.

use std::collections::BTreeMap;

use super::super::helpers::collect_methods_for_type;
use super::eval_helpers::sig_type_contains_entry;
use super::{CatalogueLintViolation, CatalogueLinterError, CatalogueLinterRule, RoleKind};
use crate::tddd::catalogue_v2::CatalogueDocument;
use crate::tddd::catalogue_v2::composite::{StructShape, TypeKindV2};
use crate::tddd::catalogue_v2::entries::TypeEntry;
use crate::tddd::catalogue_v2::methods::MethodDeclaration;
use crate::tddd::catalogue_v2::roles::{DataRole, ItemAction};
use crate::tddd::catalogue_v2::variants::VariantPayload;
use crate::tddd::layer_id::LayerId;

fn signature_references_type<'a>(
    mut type_refs: impl Iterator<Item = &'a str>,
    referenced_name: &str,
    catalogue: &CatalogueDocument,
    all_catalogues: &BTreeMap<LayerId, CatalogueDocument>,
) -> bool {
    type_refs.any(|type_ref| {
        sig_type_contains_entry(
            type_ref,
            referenced_name,
            catalogue.layer(),
            catalogue.layer(),
            all_catalogues,
        )
    })
}

fn methods_reference_type<'a>(
    mut methods: impl Iterator<Item = &'a MethodDeclaration>,
    referenced_name: &str,
    catalogue: &CatalogueDocument,
    all_catalogues: &BTreeMap<LayerId, CatalogueDocument>,
) -> bool {
    methods.any(|method| {
        signature_references_type(
            method
                .params
                .iter()
                .map(|param| param.ty.as_str())
                .chain(std::iter::once(method.returns.as_str())),
            referenced_name,
            catalogue,
            all_catalogues,
        )
    })
}

fn type_entry_references_type(
    entry: &TypeEntry,
    entry_name: &str,
    referenced_name: &str,
    catalogue: &CatalogueDocument,
    all_catalogues: &BTreeMap<LayerId, CatalogueDocument>,
) -> Result<bool, CatalogueLinterError> {
    let structural_reference = match entry.kind() {
        TypeKindV2::Struct(struct_kind) => match &struct_kind.shape {
            StructShape::Unit => false,
            StructShape::Tuple { fields, .. } => signature_references_type(
                fields.iter().map(|field| field.as_str()),
                referenced_name,
                catalogue,
                all_catalogues,
            ),
            StructShape::Plain { fields, .. } => signature_references_type(
                fields.iter().map(|field| field.ty.as_str()),
                referenced_name,
                catalogue,
                all_catalogues,
            ),
        },
        TypeKindV2::Enum { variants } => variants.iter().any(|variant| match &variant.payload {
            VariantPayload::Unit => false,
            VariantPayload::Tuple(fields) => signature_references_type(
                fields.iter().map(|field| field.as_str()),
                referenced_name,
                catalogue,
                all_catalogues,
            ),
            VariantPayload::Struct(fields) => signature_references_type(
                fields.iter().map(|field| field.ty.as_str()),
                referenced_name,
                catalogue,
                all_catalogues,
            ),
        }),
        TypeKindV2::TypeAlias { target } => signature_references_type(
            std::iter::once(target.as_str()),
            referenced_name,
            catalogue,
            all_catalogues,
        ),
    };
    if structural_reference {
        return Ok(true);
    }
    Ok(methods_reference_type(
        collect_methods_for_type(catalogue, entry, entry_name)?.into_iter(),
        referenced_name,
        catalogue,
        all_catalogues,
    ))
}

/// Evaluates the domain-only ValueObject inbound-reference rule.
pub(super) fn evaluate_domain_value_object_inbound_references(
    rule: &CatalogueLinterRule,
    catalogue: &CatalogueDocument,
    all_catalogues: &BTreeMap<LayerId, CatalogueDocument>,
) -> Result<Vec<CatalogueLintViolation>, CatalogueLinterError> {
    if catalogue.layer().as_ref() != "domain" {
        return Ok(vec![]);
    }

    let mut violations = Vec::new();
    for (value_object_name, _) in catalogue.types().iter().filter(|(_, entry)| {
        entry.action() != ItemAction::Delete
            && matches!(entry.role(), DataRole::ValueObject { .. })
            && rule.target().matches(RoleKind::ValueObject)
    }) {
        let referenced_by_type = catalogue
            .types()
            .iter()
            .filter(|(other_name, other_entry)| {
                other_name != &value_object_name && other_entry.action() != ItemAction::Delete
            })
            .map(|(other_name, other_entry)| {
                type_entry_references_type(
                    other_entry,
                    other_name.as_str(),
                    value_object_name.as_str(),
                    catalogue,
                    all_catalogues,
                )
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .any(|references| references);

        let referenced_by_trait =
            catalogue.traits().values().filter(|entry| entry.action() != ItemAction::Delete).any(
                |entry| {
                    methods_reference_type(
                        entry.methods().iter(),
                        value_object_name.as_str(),
                        catalogue,
                        all_catalogues,
                    )
                },
            );

        let referenced_by_function = catalogue
            .functions()
            .values()
            .filter(|entry| entry.action() != ItemAction::Delete)
            .any(|entry| {
                signature_references_type(
                    entry
                        .params()
                        .iter()
                        .map(|param| param.ty.as_str())
                        .chain(std::iter::once(entry.returns().as_str())),
                    value_object_name.as_str(),
                    catalogue,
                    all_catalogues,
                )
            });

        if !(referenced_by_type || referenced_by_trait || referenced_by_function) {
            violations.push(CatalogueLintViolation::new(
                rule.kind().discriminant_name(),
                value_object_name.as_str(),
                "domain ValueObject must have an inbound reference from a different domain entry signature",
            ));
        }
    }
    Ok(violations)
}
