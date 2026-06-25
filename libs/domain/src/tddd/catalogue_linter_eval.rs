//! `evaluate_catalogue_lint` — pure free-function entry point (D17 / T014).
//!
//! This module is declared by `catalogue_linter.rs` via `#[path]` and is not
//! a public module. The `evaluate_catalogue_lint` function is re-exported from
//! the parent module.
//!
//! Cross-layer type-role resolution helpers live in the sibling submodule
//! `eval_helpers` (file `catalogue_linter_eval_helpers.rs`).

use std::collections::BTreeMap;

use super::helpers::{
    bare_name_in_type_ref, collect_methods_for_type, contract_role_type_ref, entry_role_kind,
    field_type_refs, field_vec_is_empty, function_entries_for_target, has_trait_impl,
    identity_accessor_name, invariants_for_role, struct_has_public_fields,
    trait_entries_for_target, type_entries_for_target, validate_contract_role_field,
    validate_data_role_field,
};
use super::{
    CatalogueLintViolation, CatalogueLinterError, CatalogueLinterRule, CatalogueLinterRuleKind,
    RoleKind,
};
use crate::tddd::catalogue_v2::CatalogueDocument;
use crate::tddd::catalogue_v2::composite::TypeKindV2;
use crate::tddd::catalogue_v2::roles::{InvariantPredicate, ItemAction, SelfReceiver};
use crate::tddd::layer_id::LayerId;

// Cross-layer lookup helpers (strip_ref_sigils, resolve_type_role,
// sig_type_contains_entry, find_in_catalogue, etc.)
#[path = "catalogue_linter_eval_helpers.rs"]
mod eval_helpers;

use eval_helpers::{resolve_type_role, sig_type_contains_entry};

fn ensure_target_can_produce_type_ref_checks(
    rule_kind: &str,
    target_roles: &[RoleKind],
    target_field: &str,
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
        return Err(CatalogueLinterError::InvalidRuleConfig(format!(
            "{}: target_field '{}' is not carried by role '{}' in target_roles [{}]; \
             every target role must carry the field to avoid silent skips",
            rule_kind,
            target_field,
            bad_role.variant_name(),
            role_names
        )));
    }
    Ok(())
}

fn ensure_target_can_produce_data_role_field_checks(
    rule_kind: &str,
    target_roles: &[RoleKind],
    target_field: &str,
) -> Result<(), CatalogueLinterError> {
    let effective_roles =
        if target_roles.is_empty() { RoleKind::DATA_ROLES.as_slice() } else { target_roles };
    // Every target role must carry the field.  A role that does not carry the
    // field will always see an empty vec in `field_vec_is_empty`, causing
    // FieldNonEmpty to fire as a false positive or FieldEmpty to silently pass
    // for every entry (D19 fail-closed).
    if let Some(bad_role) =
        effective_roles.iter().find(|role| !role.carries_data_role_field(target_field))
    {
        let role_names = if target_roles.is_empty() {
            "all DataRole roles".to_owned()
        } else {
            effective_roles.iter().map(|role| role.variant_name()).collect::<Vec<_>>().join(", ")
        };
        return Err(CatalogueLinterError::InvalidRuleConfig(format!(
            "{}: target_field '{}' is not carried by role '{}' in target_roles [{}]; \
             every target role must carry the field to avoid false positives",
            rule_kind,
            target_field,
            bad_role.variant_name(),
            role_names
        )));
    }
    Ok(())
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
pub fn evaluate_catalogue_lint(
    rules: &[CatalogueLinterRule],
    all_catalogues: &BTreeMap<LayerId, CatalogueDocument>,
    target_layer_id: &LayerId,
) -> Result<Vec<CatalogueLintViolation>, CatalogueLinterError> {
    let catalogue = all_catalogues.get(target_layer_id).ok_or_else(|| {
        CatalogueLinterError::UnknownLayer { layer_id: target_layer_id.as_ref().to_owned() }
    })?;

    let mut violations: Vec<CatalogueLintViolation> = Vec::new();

    for rule in rules {
        match rule.kind() {
            CatalogueLinterRuleKind::FieldEmpty { target_field } => {
                validate_data_role_field(target_field.as_str())?;
                ensure_target_can_produce_data_role_field_checks(
                    rule.kind().discriminant_name(),
                    rule.target().target_roles(),
                    target_field.as_str(),
                )?;
                for (name, entry) in type_entries_for_target(catalogue, rule.target()) {
                    if !field_vec_is_empty(&entry.role, target_field.as_str())? {
                        violations.push(CatalogueLintViolation::new(
                            rule.kind().discriminant_name(),
                            name.as_str(),
                            format!("field '{target_field}' must be empty but contains elements"),
                        ));
                    }
                }
            }

            CatalogueLinterRuleKind::FieldNonEmpty { target_field } => {
                validate_data_role_field(target_field.as_str())?;
                ensure_target_can_produce_data_role_field_checks(
                    rule.kind().discriminant_name(),
                    rule.target().target_roles(),
                    target_field.as_str(),
                )?;
                for (name, entry) in type_entries_for_target(catalogue, rule.target()) {
                    if field_vec_is_empty(&entry.role, target_field.as_str())? {
                        violations.push(CatalogueLintViolation::new(
                            rule.kind().discriminant_name(),
                            name.as_str(),
                            format!("field '{target_field}' must not be empty"),
                        ));
                    }
                }
            }

            CatalogueLinterRuleKind::KindLayerConstraint { permitted_layers } => {
                let doc_layer = &catalogue.layer;
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
                let field_str = target_field.as_str();
                if field_str == "invariants" {
                    return Err(CatalogueLinterError::InvalidRuleConfig(
                        "ReferencedRoleConstraint: unsupported target_field 'invariants'; \
                         invariants are predicate declarations, not TypeRef role references; \
                         valid target_field values are: exclusive_members, shared_value_objects, \
                         emits, handles, reacts_to, aggregate"
                            .to_owned(),
                    ));
                }
                if validate_data_role_field(field_str).is_err()
                    && validate_contract_role_field(field_str).is_err()
                {
                    // Propagate the DataRole error as the primary diagnostic.
                    validate_data_role_field(field_str)?;
                }

                ensure_target_can_produce_type_ref_checks(
                    rule.kind().discriminant_name(),
                    rule.target().target_roles(),
                    field_str,
                )?;
                for (name, entry) in type_entries_for_target(catalogue, rule.target()) {
                    for type_ref in field_type_refs(&entry.role, target_field.as_str())? {
                        let ref_str = type_ref.as_str();
                        if resolve_type_role(ref_str, all_catalogues, target_layer_id)
                            != Some(*expected_role)
                        {
                            violations.push(CatalogueLintViolation::new(
                                rule.kind().discriminant_name(),
                                name.as_str(),
                                format!(
                                    "type '{}' referenced in field '{}' does not declare role '{}'",
                                    ref_str,
                                    target_field,
                                    expected_role.variant_name()
                                ),
                            ));
                        }
                    }
                }

                for (name, entry) in trait_entries_for_target(catalogue, rule.target()) {
                    if let Some(type_ref) =
                        contract_role_type_ref(&entry.role, target_field.as_str())?
                    {
                        let ref_str = type_ref.as_str();
                        if resolve_type_role(ref_str, all_catalogues, target_layer_id)
                            != Some(*expected_role)
                        {
                            violations.push(CatalogueLintViolation::new(
                                rule.kind().discriminant_name(),
                                name.as_str(),
                                format!(
                                    "type '{}' referenced in field '{}' does not declare role '{}'",
                                    ref_str,
                                    target_field,
                                    expected_role.variant_name()
                                ),
                            ));
                        }
                    }
                }
            }

            CatalogueLinterRuleKind::TraitImplRequired { required_traits } => {
                for (name, _entry) in type_entries_for_target(catalogue, rule.target()) {
                    for trait_name in required_traits.as_slice() {
                        if !has_trait_impl(catalogue, name.as_str(), trait_name.as_str()) {
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
                                for (tn, e) in
                                    cat.types.iter().filter(|(_, e)| e.action != ItemAction::Delete)
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
                                    .traits
                                    .iter()
                                    .filter(|(_, e)| e.action != ItemAction::Delete)
                                {
                                    let role = RoleKind::from_contract_role(&e.role);
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

            CatalogueLinterRuleKind::MethodReferenceSignature { target_field } => {
                if target_field.as_str() != "invariants" {
                    return Err(CatalogueLinterError::InvalidRuleConfig(format!(
                        "MethodReferenceSignature: unsupported target_field '{}'; only 'invariants' is supported",
                        target_field
                    )));
                }
                for (name, entry) in type_entries_for_target(catalogue, rule.target()) {
                    let all_methods = collect_methods_for_type(catalogue, entry, name.as_str())?;
                    for inv in invariants_for_role(&entry.role) {
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
                if target_field.as_str() != "identity" {
                    return Err(CatalogueLinterError::InvalidRuleConfig(format!(
                        "AccessorSignatureRequired: unsupported target_field '{}'; only 'identity' is supported",
                        target_field
                    )));
                }
                for (name, entry) in type_entries_for_target(catalogue, rule.target()) {
                    let getter_name = match identity_accessor_name(&entry.role) {
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
                if target_field.as_str() != "exclusive_members" {
                    return Err(CatalogueLinterError::InvalidRuleConfig(format!(
                        "FieldElementUniqueAcrossEntries: unsupported target_field '{}'; \
                         only 'exclusive_members' is supported (ADR D6/D11)",
                        target_field
                    )));
                }
                ensure_target_can_produce_type_ref_checks(
                    rule.kind().discriminant_name(),
                    rule.target().target_roles(),
                    target_field.as_str(),
                )?;
                // Key by the tail segment of the TypeRef so that bare `OrderLine` and
                // path-qualified `domain::OrderLine` are treated as the same type and the
                // D11 exclusive-member uniqueness check cannot be bypassed by mixing forms.
                let mut seen: std::collections::HashMap<String, String> =
                    std::collections::HashMap::new();
                for (name, entry) in type_entries_for_target(catalogue, rule.target()) {
                    for type_ref in field_type_refs(&entry.role, target_field.as_str())? {
                        let ref_str = type_ref.as_str();
                        let canonical = ref_str.split("::").last().unwrap_or(ref_str).to_owned();
                        if let Some(prev_entry) = seen.get(&canonical) {
                            if prev_entry.as_str() != name.as_str() {
                                violations.push(CatalogueLintViolation::new(
                                    rule.kind().discriminant_name(),
                                    name.as_str(),
                                    format!(
                                        "type '{}' in field '{}' already belongs to entry '{}'",
                                        ref_str, target_field, prev_entry
                                    ),
                                ));
                            }
                        } else {
                            seen.insert(canonical, name.as_str().to_owned());
                        }
                    }
                }
            }

            CatalogueLinterRuleKind::NoExternalReferenceInMethods { target_field } => {
                // Per ADR D6/D11, this rule is defined only for `exclusive_members`.
                // Other DataRole fields do not have external-reference-in-methods
                // semantics in the minimum-core rule set (D19 fail-closed).
                if target_field.as_str() != "exclusive_members" {
                    return Err(CatalogueLinterError::InvalidRuleConfig(format!(
                        "NoExternalReferenceInMethods: unsupported target_field '{}'; \
                         only 'exclusive_members' is supported (ADR D6/D11)",
                        target_field
                    )));
                }
                ensure_target_can_produce_type_ref_checks(
                    rule.kind().discriminant_name(),
                    rule.target().target_roles(),
                    target_field.as_str(),
                )?;
                let mut agg_exclusive: Vec<(String, Vec<String>)> = Vec::new();
                for (name, entry) in type_entries_for_target(catalogue, rule.target()) {
                    let refs = field_type_refs(&entry.role, target_field.as_str())?
                        .iter()
                        .map(|r| r.as_str().to_owned())
                        .collect();
                    agg_exclusive.push((name.as_str().to_owned(), refs));
                }
                for (agg_name, exclusive_refs) in &agg_exclusive {
                    if exclusive_refs.is_empty() {
                        continue;
                    }
                    // Build a set of bare-name tails for the boundary (aggregate + its
                    // exclusive members + its shared_value_objects).
                    let inside_bare: std::collections::HashSet<String> = {
                        let mut set = std::collections::HashSet::new();
                        // The aggregate itself.
                        let agg_tail = agg_name.split("::").last().unwrap_or(agg_name).to_owned();
                        set.insert(agg_tail);
                        // Exclusive members.
                        for r in exclusive_refs {
                            let tail = r.split("::").last().unwrap_or(r.as_str()).to_owned();
                            set.insert(tail);
                        }
                        // Shared value objects of this aggregate.
                        // Exclude delete-action aggregates: a deleted aggregate's
                        // shared_value_objects no longer define the boundary set.
                        if let Some((_name, entry)) = catalogue.types.iter().find(|(n, e)| {
                            n.as_str() == agg_name.as_str() && e.action != ItemAction::Delete
                        }) {
                            // "shared_value_objects" is a recognised field name — always succeeds.
                            for r in field_type_refs(&entry.role, "shared_value_objects")? {
                                let tail =
                                    r.as_str().split("::").last().unwrap_or(r.as_str()).to_owned();
                                set.insert(tail);
                            }
                        }
                        set
                    };
                    // Exclude delete-action entries: a deleted type cannot have methods
                    // that violate the exclusivity boundary.
                    for (other_name, other_entry) in
                        catalogue.types.iter().filter(|(_, e)| e.action != ItemAction::Delete)
                    {
                        let other_bare =
                            other_name.as_str().split("::").last().unwrap_or(other_name.as_str());
                        if inside_bare.contains(other_bare) {
                            continue;
                        }
                        let all_methods =
                            collect_methods_for_type(catalogue, other_entry, other_name.as_str())?;
                        for exclusive_type in exclusive_refs {
                            // Use the bare tail of the exclusive member for delimiter-boundary
                            // matching so that Vec<OrderLine>, Option<OrderLine>, &OrderLine,
                            // and path-qualified forms are all detected.
                            let bare = exclusive_type
                                .split("::")
                                .last()
                                .unwrap_or(exclusive_type.as_str());
                            let found_in_methods = all_methods.iter().any(|m| {
                                m.params.iter().any(|p| bare_name_in_type_ref(p.ty.as_str(), bare))
                                    || bare_name_in_type_ref(m.returns.as_str(), bare)
                            });
                            if found_in_methods {
                                violations.push(CatalogueLintViolation::new(
                                    rule.kind().discriminant_name(),
                                    agg_name.as_str(),
                                    format!(
                                        "exclusive member '{}' is referenced in methods of external entry '{}'",
                                        exclusive_type,
                                        other_name.as_str()
                                    ),
                                ));
                            }
                        }
                    }
                }
            }

            CatalogueLinterRuleKind::NoPublicField => {
                for (name, entry) in type_entries_for_target(catalogue, rule.target()) {
                    if let TypeKindV2::Struct(struct_kind) = &entry.kind {
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
                        let receiver_str =
                            method.receiver.map(|r| r.to_string()).unwrap_or_default();
                        if receiver_str.as_str() == forbidden_receiver.as_str() {
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
        }
    }

    Ok(violations)
}
