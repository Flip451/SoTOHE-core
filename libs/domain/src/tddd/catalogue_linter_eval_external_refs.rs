//! Identity-aware evaluation of the external-reference method rule.

use std::collections::BTreeMap;

use super::super::helpers::{collect_methods_for_type, field_type_refs, type_entries_for_target};
use super::eval_config::ensure_target_can_produce_type_ref_checks;
use super::identity::{
    CatalogueIdentityContext, TypeRefInspectionContext, build_declared_identities,
    declared_identity_universe, declared_locality_modules, entry_identity, generic_parameter_names,
    resolution_message, resolve_reference_identities, signature_contains_identity,
};
use super::{CatalogueLintViolation, CatalogueLinterError, CatalogueLinterRule, RolePayloadField};
use crate::tddd::catalogue_linter::TypeRefPathExtractorPort;
use crate::tddd::catalogue_v2::CatalogueDocument;
use crate::tddd::catalogue_v2::roles::ItemAction;
use crate::tddd::layer_id::LayerId;

pub(super) fn evaluate_no_external_reference_in_methods<E: TypeRefPathExtractorPort>(
    rule: &CatalogueLinterRule,
    target_field: RolePayloadField,
    catalogue: &CatalogueDocument,
    all_catalogues: &BTreeMap<LayerId, CatalogueDocument>,
    extractor: &E,
) -> Result<Vec<CatalogueLintViolation>, CatalogueLinterError> {
    // Per ADR D6/D11, this rule is defined only for `exclusive_members`.
    // Other DataRole fields do not have external-reference-in-methods
    // semantics in the minimum-core rule set (D19 fail-closed).
    if target_field != RolePayloadField::ExclusiveMembers {
        return Err(CatalogueLinterError::InvalidRuleConfig(super::FreeText::new(format!(
            "NoExternalReferenceInMethods: unsupported target_field '{}'; \
                 only 'exclusive_members' is supported (ADR D6/D11)",
            target_field
        ))));
    }
    ensure_target_can_produce_type_ref_checks(
        rule.kind().discriminant_name(),
        rule.target().target_roles(),
        target_field,
    )?;

    let declared_identities = build_declared_identities(all_catalogues)?;
    let identity_universe = declared_identity_universe(&declared_identities);
    let locality_modules =
        declared_locality_modules(all_catalogues, &declared_identities, catalogue.crate_name());
    let identity_context = CatalogueIdentityContext {
        catalogue_crate: catalogue.crate_name(),
        universe: &identity_universe,
        locality_modules: &locality_modules,
        entries: &declared_identities,
    };
    let mut violations = Vec::new();

    for (agg_name, agg_entry) in type_entries_for_target(catalogue, rule.target()) {
        let type_parameters = generic_parameter_names(agg_entry.generics());
        let inspection = TypeRefInspectionContext {
            type_parameters: &type_parameters,
            lifetime_parameters: &[],
            const_parameters: &[],
        };
        let exclusive_refs = field_type_refs(agg_entry.role(), target_field);
        if exclusive_refs.is_empty() {
            continue;
        }
        let aggregate_identity = entry_identity(catalogue, agg_name, agg_entry.module_path())?;
        let mut inside_identities = vec![aggregate_identity];
        let mut exclusive_identities = Vec::new();
        for type_ref in exclusive_refs {
            match resolve_reference_identities(type_ref, identity_context, extractor, inspection) {
                Ok(identities) => {
                    for identity in identities {
                        if !exclusive_identities.contains(&identity) {
                            exclusive_identities.push(identity.clone());
                        }
                        if !inside_identities.contains(&identity) {
                            inside_identities.push(identity);
                        }
                    }
                }
                Err(error) => violations.push(CatalogueLintViolation::new(
                    rule.kind().discriminant_name(),
                    agg_name.as_str(),
                    resolution_message(type_ref, &error),
                )),
            }
        }
        for type_ref in field_type_refs(agg_entry.role(), RolePayloadField::SharedValueObjects) {
            match resolve_reference_identities(type_ref, identity_context, extractor, inspection) {
                Ok(identities) => {
                    for identity in identities {
                        if !inside_identities.contains(&identity) {
                            inside_identities.push(identity);
                        }
                    }
                }
                Err(error) => violations.push(CatalogueLintViolation::new(
                    rule.kind().discriminant_name(),
                    agg_name.as_str(),
                    resolution_message(type_ref, &error),
                )),
            }
        }
        for (other_name, other_entry) in
            catalogue.types().iter().filter(|(_, e)| e.action() != ItemAction::Delete)
        {
            let other_identity = entry_identity(catalogue, other_name, other_entry.module_path())?;
            if inside_identities.contains(&other_identity) {
                continue;
            }
            let all_methods =
                collect_methods_for_type(catalogue, other_entry, other_name.as_str())?;
            for exclusive_identity in &exclusive_identities {
                let mut comparison = None;
                'method_scan: for method in &all_methods {
                    let mut signature_type_parameters =
                        generic_parameter_names(other_entry.generics());
                    signature_type_parameters.extend(generic_parameter_names(&method.generics));
                    let signature_types = method
                        .params
                        .iter()
                        .map(|parameter| &parameter.ty)
                        .chain(std::iter::once(&method.returns));
                    for signature_type in signature_types {
                        let signature_inspection = TypeRefInspectionContext {
                            type_parameters: &signature_type_parameters,
                            lifetime_parameters: &[],
                            const_parameters: &[],
                        };
                        match signature_contains_identity(
                            signature_type,
                            exclusive_identity,
                            identity_context,
                            extractor,
                            signature_inspection,
                        ) {
                            Ok(true) => {
                                comparison = Some(Ok(true));
                                break 'method_scan;
                            }
                            Ok(false) => {}
                            Err(error) => {
                                comparison = Some(Err((error, signature_type.clone())));
                                break 'method_scan;
                            }
                        }
                    }
                }
                match comparison {
                    Some(Ok(true)) => violations.push(CatalogueLintViolation::new(
                        rule.kind().discriminant_name(),
                        agg_name.as_str(),
                        format!(
                            "exclusive member '{}' is referenced in methods of external entry '{}'",
                            exclusive_identity,
                            other_name.as_str()
                        ),
                    )),
                    Some(Err((error, signature_type))) => {
                        violations.push(CatalogueLintViolation::new(
                            rule.kind().discriminant_name(),
                            agg_name.as_str(),
                            format!(
                                "could not compare method signature type '{}' with exclusive member '{}': {error}",
                                signature_type, exclusive_identity
                            ),
                        ));
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(violations)
}
