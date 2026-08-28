//! `evaluate_catalogue_lint` — pure free-function entry point (D17 / T014).
//!
//! This module is declared by `catalogue_linter.rs` via `#[path]` and is not
//! a public module. The `evaluate_catalogue_lint` function is re-exported from
//! the parent module.
//!
//! Cross-layer type-role resolution helpers live in the sibling submodule
//! `eval_helpers` (file `catalogue_linter_eval_helpers.rs`).

use std::collections::{BTreeMap, BTreeSet};

use super::eval_layer_signature;
use super::eval_primitives;
use super::helpers::{
    collect_methods_for_type, contract_role_type_ref, entry_role_kind, field_type_refs,
    field_vec_is_empty, function_entries_for_target, has_trait_impl, identity_accessor_name,
    invariants_for_role, struct_has_public_fields, trait_entries_for_target,
    type_entries_for_target, validate_contract_role_field, validate_data_role_field,
};
use super::{
    CatalogueLintViolation, CatalogueLinterError, CatalogueLinterRule, CatalogueLinterRuleKind,
    FreeText, RoleKind, RolePayloadField, TypeRefPathExtractorPort,
};
use crate::tddd::catalogue_v2::CatalogueDocument;
use crate::tddd::catalogue_v2::composite::TypeKindV2;
use crate::tddd::catalogue_v2::identifiers::{
    CatalogueItemNamespace, CrateName, FullyQualifiedItemPath, ParamName, TypeRef,
};
use crate::tddd::catalogue_v2::roles::{InvariantPredicate, ItemAction, SelfReceiver};
use crate::tddd::layer_id::LayerId;
use crate::tddd::primitive_occurrence_scanner::PrimitiveOccurrenceScanner;

// Cross-layer lookup helpers used by the rules that retain name-based
// attribution semantics. The three identity-sensitive rules below use the
// shared domain resolver directly.
#[path = "catalogue_linter_eval_helpers.rs"]
pub(super) mod eval_helpers;

use eval_helpers::sig_type_contains_entry;

#[path = "catalogue_linter_eval_composition_root.rs"]
mod eval_composition_root;

#[path = "catalogue_linter_eval_external_refs.rs"]
mod eval_external_refs;

#[path = "catalogue_linter_identity.rs"]
mod identity;

#[path = "catalogue_linter_eval_config.rs"]
mod eval_config;

use eval_config::{
    ensure_layers_exist, ensure_target_can_produce_data_role_field_checks,
    ensure_target_can_produce_type_ref_checks,
};
use identity::{
    CatalogueIdentityContext, TypeRefInspectionContext, build_declared_identities,
    declared_identity_universe, generic_parameter_names, resolution_message,
    resolve_reference_identities, role_constraint_failure,
};

/// Owned identity context reused while the entrypoint inspects every TypeRef
/// in one catalogue. The resolution itself remains delegated to the existing
/// T013 identity adapter below; this wrapper only keeps its derived declaration
/// universe alive across the preflight traversal.
pub(super) struct CatalogueTypeRefIdentityContext {
    catalogue_crate: CrateName,
    universe: BTreeSet<FullyQualifiedItemPath>,
}

pub(super) fn build_type_ref_identity_context(
    all_catalogues: &BTreeMap<LayerId, CatalogueDocument>,
    catalogue_crate: &CrateName,
) -> Result<CatalogueTypeRefIdentityContext, CatalogueLinterError> {
    let entries = build_declared_identities(all_catalogues)?;
    Ok(CatalogueTypeRefIdentityContext {
        catalogue_crate: catalogue_crate.clone(),
        universe: declared_identity_universe(&entries),
    })
}

pub(super) fn inspect_type_ref<E: TypeRefPathExtractorPort>(
    context: &CatalogueTypeRefIdentityContext,
    type_ref: &TypeRef,
    type_parameters: &[ParamName],
    lifetime_parameters: &[ParamName],
    const_parameters: &[ParamName],
    namespace: CatalogueItemNamespace,
    extractor: &E,
) -> Result<(), CatalogueLinterError> {
    let identity_context = CatalogueIdentityContext {
        catalogue_crate: &context.catalogue_crate,
        universe: &context.universe,
        entries: &[],
        namespace,
    };
    let inspection =
        TypeRefInspectionContext { type_parameters, lifetime_parameters, const_parameters };
    resolve_reference_identities(type_ref, identity_context, extractor, inspection).map(|_| ())
}

/// Evaluate `rules` against the catalogue identified by `target_layer_id`
/// within `all_catalogues`.
///
/// Returns the full list of violations found. An empty `Vec` means no rules
/// fired.
///
/// Rules that resolve type roles (`ReferencedRoleConstraint`,
/// `NoRoleInMethodSignature`) perform a **cross-layer lookup** across all
/// entries in `all_catalogues` so that a `UseCase.handles: ["domain::OrderPlaced"]`
/// reference is correctly resolved even when `OrderPlaced` is declared in the
/// `domain` catalogue rather than the `usecase` catalogue.
///
/// `NoExternalReferenceInMethods` and all other rules remain single-layer
/// (they check intra-catalogue structure only).
///
/// This is the pure domain-layer entry point (D17): no I/O, no trait object,
/// no infrastructure dependency.
///
/// # Errors
///
/// Returns [`CatalogueLinterError::UnknownLayer`] when `target_layer_id` is
/// not present in `all_catalogues`.
///
/// Returns [`CatalogueLinterError::InvalidRuleConfig`] if the provided rule
/// configuration is internally inconsistent and prevents execution.
///
/// Returns [`CatalogueLinterError::ScanFailed`] when a `ForbidPrimitiveInTypes`
/// rule's underlying [`PrimitiveOccurrenceScanner::scan`] call fails (e.g. a
/// catalogue `TypeRef` string that does not parse as valid Rust syntax).
pub fn evaluate_catalogue_lint<S: PrimitiveOccurrenceScanner, E: TypeRefPathExtractorPort>(
    rules: &[CatalogueLinterRule],
    all_catalogues: &BTreeMap<LayerId, CatalogueDocument>,
    target_layer_id: &LayerId,
    scanner: &S,
    extractor: &E,
) -> Result<Vec<CatalogueLintViolation>, CatalogueLinterError> {
    let catalogue = all_catalogues
        .get(target_layer_id)
        .ok_or_else(|| CatalogueLinterError::UnknownLayer { layer_id: target_layer_id.clone() })?;

    let mut violations: Vec<CatalogueLintViolation> = Vec::new();

    for rule in rules {
        match rule.kind() {
            CatalogueLinterRuleKind::FieldEmpty { target_field } => {
                validate_data_role_field(*target_field)?;
                ensure_target_can_produce_data_role_field_checks(
                    rule.kind().discriminant_name(),
                    rule.target().target_roles(),
                    *target_field,
                )?;
                for (name, entry) in type_entries_for_target(catalogue, rule.target()) {
                    if !field_vec_is_empty(entry.role(), *target_field) {
                        violations.push(CatalogueLintViolation::new(
                            rule.kind().discriminant_name(),
                            name.as_str(),
                            format!("field '{target_field}' must be empty but contains elements"),
                        ));
                    }
                }
            }

            CatalogueLinterRuleKind::FieldNonEmpty { target_field } => {
                validate_data_role_field(*target_field)?;
                ensure_target_can_produce_data_role_field_checks(
                    rule.kind().discriminant_name(),
                    rule.target().target_roles(),
                    *target_field,
                )?;
                for (name, entry) in type_entries_for_target(catalogue, rule.target()) {
                    if field_vec_is_empty(entry.role(), *target_field) {
                        violations.push(CatalogueLintViolation::new(
                            rule.kind().discriminant_name(),
                            name.as_str(),
                            format!("field '{target_field}' must not be empty"),
                        ));
                    }
                }
            }

            CatalogueLinterRuleKind::KindLayerConstraint { permitted_layers } => {
                let doc_layer = catalogue.layer();
                if !permitted_layers.as_slice().contains(doc_layer) {
                    for (name, _entry) in type_entries_for_target(catalogue, rule.target()) {
                        violations.push(CatalogueLintViolation::new(
                            rule.kind().discriminant_name(),
                            name.as_str(),
                            format!(
                                "entry is declared in layer '{}' which is not in permitted layers",
                                doc_layer.as_ref()
                            ),
                        ));
                    }
                    for (name, _entry) in trait_entries_for_target(catalogue, rule.target()) {
                        violations.push(CatalogueLintViolation::new(
                            rule.kind().discriminant_name(),
                            name.as_str(),
                            format!(
                                "entry is declared in layer '{}' which is not in permitted layers",
                                doc_layer.as_ref()
                            ),
                        ));
                    }
                    for (path, _entry) in function_entries_for_target(catalogue, rule.target()) {
                        violations.push(CatalogueLintViolation::new(
                            rule.kind().discriminant_name(),
                            path.to_string(),
                            format!(
                                "entry is declared in layer '{}' which is not in permitted layers",
                                doc_layer.as_ref()
                            ),
                        ));
                    }
                }
            }

            CatalogueLinterRuleKind::ReferencedRoleConstraint { target_field, expected_role } => {
                // Validate the target_field eagerly so that an unknown field name is
                // rejected even when the catalogue has no matching entries (D19 fail-closed).
                // A field may belong to DataRole only (e.g. "emits") or ContractRole only
                // (e.g. "aggregate").  Reject only names that are unrecognised in BOTH
                // contexts; role-specific validation happens per entry in the loops below.
                let field = *target_field;
                if field == RolePayloadField::Invariants {
                    return Err(CatalogueLinterError::InvalidRuleConfig(FreeText::new(
                        "ReferencedRoleConstraint: unsupported target_field 'invariants'; \
                         invariants are predicate declarations, not TypeRef role references; \
                         valid target_field values are: exclusive_members, shared_value_objects, \
                         emits, handles, reacts_to, aggregate",
                    )));
                }
                if validate_data_role_field(field).is_err()
                    && validate_contract_role_field(field).is_err()
                {
                    // Propagate the DataRole error as the primary diagnostic.
                    validate_data_role_field(field)?;
                }

                ensure_target_can_produce_type_ref_checks(
                    rule.kind().discriminant_name(),
                    rule.target().target_roles(),
                    field,
                )?;
                let declared_identities = build_declared_identities(all_catalogues)?;
                let identity_universe = declared_identity_universe(&declared_identities);
                let identity_context = CatalogueIdentityContext {
                    catalogue_crate: catalogue.crate_name(),
                    universe: &identity_universe,
                    entries: &declared_identities,
                    namespace: CatalogueItemNamespace::Type,
                };
                for (name, entry) in type_entries_for_target(catalogue, rule.target()) {
                    let type_parameters = generic_parameter_names(entry.generics());
                    let inspection = TypeRefInspectionContext {
                        type_parameters: &type_parameters,
                        lifetime_parameters: &[],
                        const_parameters: &[],
                    };
                    for type_ref in field_type_refs(entry.role(), field) {
                        if let Some(message) = role_constraint_failure(
                            type_ref,
                            *expected_role,
                            identity_context,
                            field,
                            extractor,
                            inspection,
                        ) {
                            violations.push(CatalogueLintViolation::new(
                                rule.kind().discriminant_name(),
                                name.as_str(),
                                message,
                            ));
                        }
                    }
                }

                for (name, entry) in trait_entries_for_target(catalogue, rule.target()) {
                    let type_parameters = generic_parameter_names(entry.generics());
                    let inspection = TypeRefInspectionContext {
                        type_parameters: &type_parameters,
                        lifetime_parameters: &[],
                        const_parameters: &[],
                    };
                    if let Some(type_ref) = contract_role_type_ref(entry.role(), field) {
                        if let Some(message) = role_constraint_failure(
                            type_ref,
                            *expected_role,
                            identity_context,
                            field,
                            extractor,
                            inspection,
                        ) {
                            violations.push(CatalogueLintViolation::new(
                                rule.kind().discriminant_name(),
                                name.as_str(),
                                message,
                            ));
                        }
                    }
                }
            }

            CatalogueLinterRuleKind::TraitImplRequired { required_traits } => {
                for (name, entry) in type_entries_for_target(catalogue, rule.target()) {
                    for trait_name in required_traits.as_slice() {
                        if !has_trait_impl(
                            catalogue,
                            name.as_str(),
                            entry.module_path(),
                            trait_name.as_str(),
                            extractor,
                        )? {
                            violations.push(CatalogueLintViolation::new(
                                rule.kind().discriminant_name(),
                                name.as_str(),
                                format!(
                                    "required trait impl '{}' is missing from trait_impls",
                                    trait_name
                                ),
                            ));
                        }
                    }
                }
            }

            CatalogueLinterRuleKind::NoRoleInMethodSignature { forbidden_roles } => {
                for (name, entry) in type_entries_for_target(catalogue, rule.target()) {
                    let all_methods = collect_methods_for_type(catalogue, entry, name.as_str())?;
                    for method in all_methods {
                        let sig_types: Vec<&str> = method
                            .params
                            .iter()
                            .map(|p| p.ty.as_str())
                            .chain(std::iter::once(method.returns.as_str()))
                            .collect();
                        // For each signature slot, search across all catalogues using
                        // `sig_type_contains_entry`, which correctly handles both
                        // plain and generic-wrapped refs (e.g. `Vec<domain::OrderPlaced>`)
                        // while avoiding false positives from explicit layer qualifiers.
                        'sig_slot: for type_ref_str in sig_types {
                            for (cat_layer_id, cat) in all_catalogues {
                                // Check type entries, excluding delete-action entries.
                                for (tn, e) in cat
                                    .types()
                                    .iter()
                                    .filter(|(_, e)| e.action() != ItemAction::Delete)
                                {
                                    let role = entry_role_kind(e);
                                    if forbidden_roles.as_slice().contains(&role)
                                        && sig_type_contains_entry(
                                            type_ref_str,
                                            tn.as_str(),
                                            cat_layer_id,
                                            target_layer_id,
                                            all_catalogues,
                                        )
                                    {
                                        violations.push(CatalogueLintViolation::new(
                                            rule.kind().discriminant_name(),
                                            name.as_str(),
                                            format!(
                                                "method '{}' signature contains type '{}' with forbidden role '{}'",
                                                method.name.as_str(),
                                                type_ref_str,
                                                role.variant_name()
                                            ),
                                        ));
                                        // One violation per (method, sig_type) slot is enough.
                                        continue 'sig_slot;
                                    }
                                }
                                // Check trait entries (ContractRole), excluding delete-action entries.
                                for (tn, e) in cat
                                    .traits()
                                    .iter()
                                    .filter(|(_, e)| e.action() != ItemAction::Delete)
                                {
                                    let role = RoleKind::from_contract_role(e.role());
                                    if forbidden_roles.as_slice().contains(&role)
                                        && sig_type_contains_entry(
                                            type_ref_str,
                                            tn.as_str(),
                                            cat_layer_id,
                                            target_layer_id,
                                            all_catalogues,
                                        )
                                    {
                                        violations.push(CatalogueLintViolation::new(
                                            rule.kind().discriminant_name(),
                                            name.as_str(),
                                            format!(
                                                "method '{}' signature contains type '{}' with forbidden role '{}'",
                                                method.name.as_str(),
                                                type_ref_str,
                                                role.variant_name()
                                            ),
                                        ));
                                        continue 'sig_slot;
                                    }
                                }
                            }
                        }
                    }
                }
            }

            CatalogueLinterRuleKind::NoLayerInMethodSignature { forbidden_layers } => {
                ensure_layers_exist(forbidden_layers, all_catalogues)?;
                violations.extend(eval_layer_signature::evaluate(
                    rule.kind().discriminant_name(),
                    rule.target(),
                    forbidden_layers,
                    catalogue,
                    all_catalogues,
                    target_layer_id,
                )?);
            }

            CatalogueLinterRuleKind::MethodReferenceSignature { target_field } => {
                if *target_field != RolePayloadField::Invariants {
                    return Err(CatalogueLinterError::InvalidRuleConfig(FreeText::new(format!(
                        "MethodReferenceSignature: unsupported target_field '{}'; only 'invariants' is supported",
                        target_field
                    ))));
                }
                for (name, entry) in type_entries_for_target(catalogue, rule.target()) {
                    let all_methods = collect_methods_for_type(catalogue, entry, name.as_str())?;
                    for inv in invariants_for_role(entry.role()) {
                        let InvariantPredicate::SelfMethod(method_name) = &inv.predicate;
                        let mname = method_name.as_str();
                        match all_methods.iter().find(|m| m.name.as_str() == mname) {
                            None => {
                                violations.push(CatalogueLintViolation::new(
                                    rule.kind().discriminant_name(),
                                    name.as_str(),
                                    format!(
                                        "invariant predicate method '{}' not found in public methods",
                                        mname
                                    ),
                                ));
                            }
                            Some(m) => {
                                if m.receiver != Some(SelfReceiver::SharedRef)
                                    || !m.params.is_empty()
                                    || m.returns.as_str() != "bool"
                                {
                                    violations.push(CatalogueLintViolation::new(
                                        rule.kind().discriminant_name(),
                                        name.as_str(),
                                        format!(
                                            "invariant method '{}' must have signature (&self) -> bool",
                                            mname
                                        ),
                                    ));
                                }
                            }
                        }
                    }
                }
            }

            CatalogueLinterRuleKind::AccessorSignatureRequired { target_field } => {
                if *target_field != RolePayloadField::Identity {
                    return Err(CatalogueLinterError::InvalidRuleConfig(FreeText::new(format!(
                        "AccessorSignatureRequired: unsupported target_field '{}'; only 'identity' is supported",
                        target_field
                    ))));
                }
                for (name, entry) in type_entries_for_target(catalogue, rule.target()) {
                    let getter_name = match identity_accessor_name(entry.role()) {
                        Some(g) => g,
                        None => continue,
                    };
                    let all_methods = collect_methods_for_type(catalogue, entry, name.as_str())?;
                    match all_methods.iter().find(|m| m.name.as_str() == getter_name) {
                        None => {
                            violations.push(CatalogueLintViolation::new(
                                rule.kind().discriminant_name(),
                                name.as_str(),
                                format!(
                                    "identity getter '{}' not found in public methods",
                                    getter_name
                                ),
                            ));
                        }
                        Some(m) => {
                            if m.receiver != Some(SelfReceiver::SharedRef)
                                || !m.params.is_empty()
                                || m.returns.as_str() == "()"
                            {
                                violations.push(CatalogueLintViolation::new(
                                    rule.kind().discriminant_name(),
                                    name.as_str(),
                                    format!(
                                        "identity getter '{}' must have signature (&self) -> NonUnit",
                                        getter_name
                                    ),
                                ));
                            }
                        }
                    }
                }
            }

            CatalogueLinterRuleKind::FieldElementUniqueAcrossEntries { target_field } => {
                // Per ADR D6/D11, this rule is defined only for `exclusive_members`.
                // Other DataRole fields (emits, handles, reacts_to, shared_value_objects,
                // invariants) do not have cross-entry uniqueness semantics in the
                // minimum-core rule set (D19 fail-closed).
                if *target_field != RolePayloadField::ExclusiveMembers {
                    return Err(CatalogueLinterError::InvalidRuleConfig(FreeText::new(format!(
                        "FieldElementUniqueAcrossEntries: unsupported target_field '{}'; \
                         only 'exclusive_members' is supported (ADR D6/D11)",
                        target_field
                    ))));
                }
                ensure_target_can_produce_type_ref_checks(
                    rule.kind().discriminant_name(),
                    rule.target().target_roles(),
                    *target_field,
                )?;
                let declared_identities = build_declared_identities(all_catalogues)?;
                let identity_universe = declared_identity_universe(&declared_identities);
                let identity_context = CatalogueIdentityContext {
                    catalogue_crate: catalogue.crate_name(),
                    universe: &identity_universe,
                    entries: &declared_identities,
                    namespace: CatalogueItemNamespace::Type,
                };
                let mut seen: BTreeMap<FullyQualifiedItemPath, String> = BTreeMap::new();
                for (name, entry) in type_entries_for_target(catalogue, rule.target()) {
                    let type_parameters = generic_parameter_names(entry.generics());
                    let inspection = TypeRefInspectionContext {
                        type_parameters: &type_parameters,
                        lifetime_parameters: &[],
                        const_parameters: &[],
                    };
                    for type_ref in field_type_refs(entry.role(), *target_field) {
                        match resolve_reference_identities(
                            type_ref,
                            identity_context,
                            extractor,
                            inspection,
                        ) {
                            Ok(identities) => {
                                for identity in identities {
                                    if let Some(prev_entry) = seen.get(&identity) {
                                        if prev_entry.as_str() != name.as_str() {
                                            violations.push(CatalogueLintViolation::new(
                                                rule.kind().discriminant_name(),
                                                name.as_str(),
                                                format!(
                                                    "type '{}' in field '{}' already belongs to entry '{}'",
                                                    identity, target_field, prev_entry
                                                ),
                                            ));
                                        }
                                    } else {
                                        seen.insert(identity, name.as_str().to_owned());
                                    }
                                }
                            }
                            Err(error) => violations.push(CatalogueLintViolation::new(
                                rule.kind().discriminant_name(),
                                name.as_str(),
                                resolution_message(type_ref, &error),
                            )),
                        }
                    }
                }
            }

            CatalogueLinterRuleKind::NoExternalReferenceInMethods { target_field } => {
                violations.extend(eval_external_refs::evaluate_no_external_reference_in_methods(
                    rule,
                    *target_field,
                    catalogue,
                    all_catalogues,
                    extractor,
                )?);
            }

            CatalogueLinterRuleKind::NoPublicField => {
                for (name, entry) in type_entries_for_target(catalogue, rule.target()) {
                    if let TypeKindV2::Struct(struct_kind) = entry.kind() {
                        if struct_has_public_fields(struct_kind) {
                            violations.push(CatalogueLintViolation::new(
                                rule.kind().discriminant_name(),
                                name.as_str(),
                                "struct has public fields; use private fields with accessor methods instead",
                            ));
                        }
                    }
                }
            }

            CatalogueLinterRuleKind::ForbiddenMethodReceiver { forbidden_receiver } => {
                for (name, entry) in type_entries_for_target(catalogue, rule.target()) {
                    let all_methods = collect_methods_for_type(catalogue, entry, name.as_str())?;
                    for method in all_methods {
                        if method.receiver == Some(*forbidden_receiver) {
                            violations.push(CatalogueLintViolation::new(
                                rule.kind().discriminant_name(),
                                name.as_str(),
                                format!(
                                    "method '{}' uses forbidden receiver '{}'",
                                    method.name.as_str(),
                                    forbidden_receiver
                                ),
                            ));
                        }
                    }
                }
            }

            CatalogueLinterRuleKind::ForbidPrimitiveInTypes { primitives, layers, positions } => {
                ensure_layers_exist(layers, all_catalogues)?;
                let found = eval_primitives::evaluate_forbid_primitive_in_types(
                    rule,
                    catalogue,
                    target_layer_id,
                    primitives,
                    layers,
                    positions,
                    scanner,
                )?;
                violations.extend(found);
            }

            CatalogueLinterRuleKind::CompositionRootPureDi => {
                violations.extend(eval_composition_root::evaluate_composition_root_pure_di(
                    rule,
                    catalogue,
                    all_catalogues,
                    target_layer_id,
                    extractor,
                )?)
            }
        }
    }

    Ok(violations)
}
