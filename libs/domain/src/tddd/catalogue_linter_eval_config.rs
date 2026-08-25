//! Configuration and target validation helpers for catalogue lint evaluation.

use std::collections::BTreeMap;

use super::{CatalogueLinterError, FreeText, RoleKind, RolePayloadField};
use crate::tddd::catalogue_v2::CatalogueDocument;
use crate::tddd::catalogue_v2::roles::NonEmptyVec;
use crate::tddd::layer_id::LayerId;

pub(super) fn ensure_target_can_produce_type_ref_checks(
    rule_kind: &str,
    target_roles: &[RoleKind],
    target_field: RolePayloadField,
) -> Result<(), CatalogueLinterError> {
    let effective_roles =
        if target_roles.is_empty() { RoleKind::ALL.as_slice() } else { target_roles };
    if let Some(bad_role) =
        effective_roles.iter().find(|role| !role.carries_type_ref_field(target_field))
    {
        let role_names = if target_roles.is_empty() {
            "all roles".to_owned()
        } else {
            effective_roles.iter().map(|role| role.variant_name()).collect::<Vec<_>>().join(", ")
        };
        return Err(CatalogueLinterError::InvalidRuleConfig(FreeText::new(format!(
            "{}: target_field '{}' is not carried by role '{}' in target_roles [{}]; \
             every target role must carry the field to avoid silent skips",
            rule_kind,
            target_field,
            bad_role.variant_name(),
            role_names
        ))));
    }
    Ok(())
}

pub(super) fn ensure_target_can_produce_data_role_field_checks(
    rule_kind: &str,
    target_roles: &[RoleKind],
    target_field: RolePayloadField,
) -> Result<(), CatalogueLinterError> {
    let effective_roles =
        if target_roles.is_empty() { RoleKind::DATA_ROLES.as_slice() } else { target_roles };
    if let Some(bad_role) =
        effective_roles.iter().find(|role| !role.carries_data_role_field(target_field))
    {
        let role_names = if target_roles.is_empty() {
            "all DataRole roles".to_owned()
        } else {
            effective_roles.iter().map(|role| role.variant_name()).collect::<Vec<_>>().join(", ")
        };
        return Err(CatalogueLinterError::InvalidRuleConfig(FreeText::new(format!(
            "{}: target_field '{}' is not carried by role '{}' in target_roles [{}]; \
             every target role must carry the field to avoid false positives",
            rule_kind,
            target_field,
            bad_role.variant_name(),
            role_names
        ))));
    }
    Ok(())
}

pub(super) fn ensure_layers_exist(
    layers: &NonEmptyVec<LayerId>,
    all_catalogues: &BTreeMap<LayerId, CatalogueDocument>,
) -> Result<(), CatalogueLinterError> {
    for layer_id in layers.as_slice() {
        if !all_catalogues.contains_key(layer_id) {
            return Err(CatalogueLinterError::UnknownLayer { layer_id: layer_id.clone() });
        }
    }
    Ok(())
}
