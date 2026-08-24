//! Internal helper functions for catalogue linter evaluation.
//!
//! This module is declared by `catalogue_linter.rs` via `#[path]` and is not
//! a public module. All items are `pub(super)` so they are visible to
//! `evaluate_catalogue_lint` in `catalogue_linter_eval.rs`.

use std::collections::BTreeSet;

use super::identity_helpers::root_path_occurrence;
use super::{
    CatalogueLinterError, FreeText, RoleKind, RolePayloadField, RuleTarget,
    TypeRefPathExtractorPort,
};
use crate::tddd::catalogue_v2::CatalogueDocument;
use crate::tddd::catalogue_v2::composite::{StructKind, StructShape};
use crate::tddd::catalogue_v2::entries::{FunctionEntry, TraitEntry, TypeEntry};
use crate::tddd::catalogue_v2::identifiers::{
    CrateName, FullyQualifiedItemPath, FunctionPath, ModulePath, ParamName, TypeRef,
};
use crate::tddd::catalogue_v2::identity_resolution::{
    CatalogueIdentityResolutionError, STANDARD_EXTERNAL_TRAIT_PATHS, is_explicit_external_path,
    normalize_lookup, resolve_catalogue_identity,
};
use crate::tddd::catalogue_v2::methods::MethodDeclaration;
use crate::tddd::catalogue_v2::roles::{ContractRole, DataRole, ItemAction};
use crate::tddd::catalogue_v2::traits::TraitImplDeclV2;
use crate::tddd::semantic_verify::CatalogueEntryKey;

// ---------------------------------------------------------------------------
// Entry filtering helpers
// ---------------------------------------------------------------------------

/// Returns the `RoleKind` for a `TypeEntry`'s `DataRole`.
pub(super) fn entry_role_kind(entry: &TypeEntry) -> RoleKind {
    RoleKind::from_data_role(entry.role())
}

/// Returns `true` when the `target` selector matches the given `RoleKind`.
pub(super) fn target_matches(target: &RuleTarget, role: RoleKind) -> bool {
    target.matches(role)
}

/// Iterates over `(type_name, entry)` pairs in `catalogue.types` where the
/// entry's `DataRole` matches the rule's `RuleTarget`.
///
/// Entries with `action: Delete` or `action: Reference` are excluded so that
/// fail-closed semantics are preserved:
/// - A delete-marked entry is treated as absent and no lint rule is applied
///   against it.
/// - A reference-marked entry cites a pre-existing type without restating its
///   full structure (e.g. `trait_impls` established when the type was
///   originally declared are not repeated in a reference entry). It is
///   opaque to this catalogue's rule evaluations, so no lint rule is applied
///   against it either — otherwise rules such as `TraitImplRequired` would
///   false-positive on every reference entry whose trait impls live outside
///   this catalogue's `trait_impls` list.
pub(super) fn type_entries_for_target<'a>(
    catalogue: &'a CatalogueDocument,
    target: &RuleTarget,
) -> impl Iterator<Item = (&'a CatalogueEntryKey, &'a TypeEntry)> {
    catalogue.types().iter().filter(move |(_name, entry)| {
        entry.action() != ItemAction::Delete
            && entry.action() != ItemAction::Reference
            && target_matches(target, entry_role_kind(entry))
    })
}

/// Iterates over `(trait_name, entry)` pairs in `catalogue.traits` where the
/// entry's `ContractRole` matches the rule's `RuleTarget`.
///
/// Entries with `action: Delete` or `action: Reference` are excluded so that
/// fail-closed semantics are preserved (mirrors `type_entries_for_target`):
/// - A delete-marked entry is treated as absent and no lint rule is applied
///   against it.
/// - A reference-marked entry cites a pre-existing trait without restating
///   its full structure — it is opaque to this catalogue's rule evaluations,
///   so no lint rule is applied against it either. Otherwise the shipped
///   `result_err` default rule would falsely flag a track that only cites an
///   unchanged upstream trait carrying a legacy `Result<_, String>`.
pub(super) fn trait_entries_for_target<'a>(
    catalogue: &'a CatalogueDocument,
    target: &RuleTarget,
) -> impl Iterator<Item = (&'a CatalogueEntryKey, &'a TraitEntry)> {
    catalogue.traits().iter().filter(move |(_name, entry)| {
        entry.action() != ItemAction::Delete
            && entry.action() != ItemAction::Reference
            && target_matches(target, RoleKind::from_contract_role(entry.role()))
    })
}

/// Iterates over `(function_path, entry)` pairs in `catalogue.functions` where
/// the entry's `FunctionRole` matches the rule's `RuleTarget`.
///
/// Entries with `action: Delete` or `action: Reference` are excluded so that
/// fail-closed semantics are preserved (mirrors `type_entries_for_target`):
/// - A delete-marked entry is treated as absent and no lint rule is applied
///   against it.
/// - A reference-marked entry cites a pre-existing function without restating
///   its full structure — it is opaque to this catalogue's rule evaluations,
///   so no lint rule is applied against it either.
pub(super) fn function_entries_for_target<'a>(
    catalogue: &'a CatalogueDocument,
    target: &RuleTarget,
) -> impl Iterator<Item = (&'a FunctionPath, &'a FunctionEntry)> {
    catalogue.functions().iter().filter(move |(_path, entry)| {
        entry.action() != ItemAction::Delete
            && entry.action() != ItemAction::Reference
            && target_matches(target, RoleKind::from_function_role(&entry.role()))
    })
}

/// Aggregates methods from the type entry and matching inherent impl blocks.
/// Duplicate names must have identical declarations; otherwise this fails closed
/// with `InvalidRuleConfig`.
pub(super) fn collect_methods_for_type<'a>(
    catalogue: &'a CatalogueDocument,
    entry: &'a TypeEntry,
    type_name: &str,
) -> Result<Vec<&'a MethodDeclaration>, CatalogueLinterError> {
    let type_identity = canonical_catalogue_identity(catalogue, type_name, entry.module_path())?;
    let type_identities = declared_type_identities(catalogue)?;
    let mut methods = Vec::new();
    let mut seen_names = std::collections::BTreeMap::new();
    let mut source_methods = entry.methods().iter().collect::<Vec<_>>();

    for impl_decl in catalogue.inherent_impls() {
        let Some(impl_identity) = resolve_catalogue_entry_reference(
            catalogue,
            impl_decl.type_name().as_str(),
            &type_identities,
            false,
        )?
        else {
            continue;
        };
        if impl_identity == type_identity {
            source_methods.extend(impl_decl.methods().iter());
        }
    }

    for method in source_methods {
        if let Some(existing) = seen_names.get(method.name.as_str()) {
            if *existing != method {
                return Err(CatalogueLinterError::InvalidRuleConfig(FreeText::new(format!(
                    "method '{}' for type '{}' has inconsistent duplicate declarations \
                     across TypeEntry.methods and inherent_impls; keep one canonical \
                     declaration or make duplicate declarations identical",
                    method.name.as_str(),
                    type_name
                ))));
            }
            continue;
        }
        seen_names.insert(method.name.as_str(), method);
        methods.push(method);
    }

    Ok(methods)
}

/// Converts a catalogue key and its declaration-module fallback into the shared
/// fully qualified identity used by the linter.
fn canonical_catalogue_identity(
    catalogue: &CatalogueDocument,
    raw_key: &str,
    declared_module_path: &ModulePath,
) -> Result<FullyQualifiedItemPath, CatalogueLinterError> {
    let key = CatalogueEntryKey::try_new(raw_key.to_owned()).map_err(|error| {
        CatalogueLinterError::InvalidRuleConfig(FreeText::new(format!(
            "invalid catalogue entry identity '{raw_key}': {error}"
        )))
    })?;
    FullyQualifiedItemPath::from_catalogue_entry_key(
        catalogue.crate_name(),
        &key,
        declared_module_path,
    )
    .map_err(|error| {
        CatalogueLinterError::InvalidRuleConfig(FreeText::new(format!(
            "invalid catalogue entry identity '{raw_key}': {error}"
        )))
    })
}

fn declared_type_identities(
    catalogue: &CatalogueDocument,
) -> Result<BTreeSet<FullyQualifiedItemPath>, CatalogueLinterError> {
    let mut identities = BTreeSet::new();
    for (name, entry) in catalogue.types() {
        if entry.action() == ItemAction::Delete {
            continue;
        }
        identities.insert(canonical_catalogue_identity(
            catalogue,
            name.as_str(),
            entry.module_path(),
        )?);
    }
    Ok(identities)
}

fn resolve_catalogue_entry_reference(
    catalogue: &CatalogueDocument,
    raw_key: &str,
    identities: &BTreeSet<FullyQualifiedItemPath>,
    allow_explicit_external: bool,
) -> Result<Option<FullyQualifiedItemPath>, CatalogueLinterError> {
    let reference = TypeRef::new(raw_key.to_owned()).map_err(|error| {
        CatalogueLinterError::InvalidRuleConfig(FreeText::new(format!(
            "invalid catalogue entry reference '{raw_key}': {error}"
        )))
    })?;
    match resolve_catalogue_identity(&reference, catalogue.crate_name(), identities) {
        Ok(identity) => Ok(Some(identity)),
        Err(CatalogueIdentityResolutionError::UnresolvedIdentifier(location))
            if allow_explicit_external
                && is_explicit_external_path(&location, catalogue.crate_name(), identities) =>
        {
            Ok(None)
        }
        Err(error) => Err(CatalogueLinterError::IdentityResolutionFailed(error)),
    }
}

/// Returns the `TypeRef` for the named field of a `ContractRole`, if applicable.
///
/// Returns `Some(...)` when the field is recognised and the role carries it.
/// Returns `None` when the field is a recognised `RolePayloadField` variant but
/// the given role does not carry it (e.g. `Aggregate` on `ContractRole::SecondaryPort`,
/// or any DataRole-only field such as `Emits`). `RolePayloadField` is a closed
/// enum (D19 fail-closed, enforced by the type system): an unrecognised field
/// name is unrepresentable here, so this function is infallible.
pub(super) fn contract_role_type_ref(
    role: &ContractRole,
    field: RolePayloadField,
) -> Option<&TypeRef> {
    match field {
        RolePayloadField::Aggregate => match role {
            ContractRole::Repository { aggregate } => Some(aggregate),
            _ => None,
        },
        // DataRole-only fields — not carried by any ContractRole variant.
        // Return None so that the entry is skipped without a violation.
        RolePayloadField::ExclusiveMembers
        | RolePayloadField::SharedValueObjects
        | RolePayloadField::Emits
        | RolePayloadField::Handles
        | RolePayloadField::ReactsTo
        | RolePayloadField::Invariants
        | RolePayloadField::Identity => None,
    }
}

// ---------------------------------------------------------------------------
// Struct / method inspection helpers
// ---------------------------------------------------------------------------

/// Returns `true` if the struct shape has any public (non-stripped) fields.
///
/// - `StructShape::Plain { fields, .. }`: public when `!fields.is_empty()`.
/// - `StructShape::Tuple { fields, .. }`: public when `!fields.is_empty()`.
/// - `StructShape::Unit`: never has fields.
///
/// Per D9 / D18: enum variant payload (`TypeKindV2::Enum`) is not checked here.
pub(super) fn struct_has_public_fields(kind: &StructKind) -> bool {
    match &kind.shape {
        StructShape::Plain { fields, .. } => !fields.is_empty(),
        StructShape::Tuple { fields, .. } => !fields.is_empty(),
        StructShape::Unit => false,
    }
}

/// Returns whether a trait implementation for `type_name` resolves to the
/// required fully qualified trait identity.
pub(super) fn has_trait_impl<E: TypeRefPathExtractorPort>(
    catalogue: &CatalogueDocument,
    type_name: &str,
    type_module_path: &ModulePath,
    trait_name_prefix: &str,
    extractor: &E,
) -> Result<bool, CatalogueLinterError> {
    let type_identity = canonical_catalogue_identity(catalogue, type_name, type_module_path)?;
    let type_identities = declared_type_identities(catalogue)?;
    let trait_identities = declared_trait_identities(catalogue)?;
    let external_trait_identities = allowed_external_trait_identities(catalogue, extractor)?;

    for ti in catalogue.trait_impls() {
        // Exclude delete-action impl entries: a deleted impl does not count as present.
        if ti.action() == ItemAction::Delete {
            continue;
        }
        let type_parameters = impl_type_parameters(ti);
        let for_type = root_path_occurrence(ti.for_type(), extractor, &type_parameters)?;
        let Some(for_type_identity) = resolve_catalogue_entry_reference(
            catalogue,
            for_type.as_str(),
            &type_identities,
            true,
        )?
        else {
            continue;
        };
        if for_type_identity == type_identity
            && trait_ref_matches(
                catalogue,
                ti.trait_ref(),
                trait_name_prefix,
                &trait_identities,
                &external_trait_identities,
                extractor,
                &type_parameters,
            )?
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn declared_trait_identities(
    catalogue: &CatalogueDocument,
) -> Result<BTreeSet<FullyQualifiedItemPath>, CatalogueLinterError> {
    let mut identities = BTreeSet::new();
    for (name, entry) in catalogue.traits() {
        if entry.action() == ItemAction::Delete {
            continue;
        }
        identities.insert(canonical_catalogue_identity(
            catalogue,
            name.as_str(),
            entry.module_path(),
        )?);
    }
    Ok(identities)
}

fn trait_ref_matches<E: TypeRefPathExtractorPort>(
    catalogue: &CatalogueDocument,
    actual_ref: &TypeRef,
    required_ref: &str,
    trait_identities: &BTreeSet<FullyQualifiedItemPath>,
    external_trait_identities: &BTreeSet<FullyQualifiedItemPath>,
    extractor: &E,
    type_parameters: &[ParamName],
) -> Result<bool, CatalogueLinterError> {
    let actual_ref = root_path_occurrence(actual_ref, extractor, type_parameters)?;
    let required_ref = TypeRef::new(required_ref.to_owned()).map_err(|error| {
        CatalogueLinterError::InvalidRuleConfig(FreeText::new(format!(
            "invalid required trait reference '{required_ref}': {error}"
        )))
    })?;
    let required_ref = root_path_occurrence(&required_ref, extractor, &[])?;
    let actual_identity = resolve_trait_identity(
        catalogue,
        &actual_ref,
        trait_identities,
        external_trait_identities,
    )?;
    let required_identity = resolve_trait_identity(
        catalogue,
        &required_ref,
        trait_identities,
        external_trait_identities,
    )?;

    match (actual_identity, required_identity) {
        (Some(actual), Some(required)) => Ok(actual == required),
        // Unresolved references are not identities and must not match by their
        // terminal spelling or path suffix.
        (None, None) => Ok(false),
        _ => Ok(false),
    }
}

/// Builds the standard and explicitly declared external trait identity universe.
fn allowed_external_trait_identities<E: TypeRefPathExtractorPort>(
    catalogue: &CatalogueDocument,
    extractor: &E,
) -> Result<BTreeSet<FullyQualifiedItemPath>, CatalogueLinterError> {
    let mut standard_identities = BTreeSet::new();
    for path in STANDARD_EXTERNAL_TRAIT_PATHS.split_whitespace() {
        let key = CatalogueEntryKey::try_new(path.to_owned()).map_err(|error| {
            CatalogueLinterError::InvalidRuleConfig(FreeText::new(format!(
                "invalid allowed external trait path '{path}': {error}"
            )))
        })?;
        let identity = FullyQualifiedItemPath::from_fully_qualified_key(&key).map_err(|error| {
            CatalogueLinterError::InvalidRuleConfig(FreeText::new(format!(
                "invalid allowed external trait path '{path}': {error}"
            )))
        })?;
        standard_identities.insert(identity);
    }

    let mut identities = standard_identities.clone();

    for trait_impl in catalogue.trait_impls() {
        if trait_impl.action() == ItemAction::Delete {
            continue;
        }
        let type_parameters = impl_type_parameters(trait_impl);
        let path = root_path_occurrence(trait_impl.trait_ref(), extractor, &type_parameters)?;
        let path = normalize_lookup(path.as_str(), catalogue.crate_name());
        if !path.contains("::") {
            continue;
        }
        let path = TypeRef::new(path).map_err(|error| {
            CatalogueLinterError::InvalidRuleConfig(FreeText::new(format!(
                "invalid external trait implementation path: {error}"
            )))
        })?;
        let identity =
            resolve_external_trait_identity(&path, catalogue.crate_name(), &standard_identities)?;
        if identity.crate_name() != catalogue.crate_name() {
            identities.insert(identity);
        }
    }
    Ok(identities)
}

fn resolve_external_trait_identity(
    path: &TypeRef,
    catalogue_crate: &CrateName,
    standard_identities: &BTreeSet<FullyQualifiedItemPath>,
) -> Result<FullyQualifiedItemPath, CatalogueLinterError> {
    match resolve_catalogue_identity(path, catalogue_crate, standard_identities) {
        Ok(identity) => Ok(identity),
        Err(CatalogueIdentityResolutionError::UnresolvedIdentifier(_)) => {
            let key = CatalogueEntryKey::try_new(path.as_str().to_owned()).map_err(|error| {
                CatalogueLinterError::InvalidRuleConfig(FreeText::new(format!(
                    "invalid external trait implementation path '{}': {error}",
                    path.as_str()
                )))
            })?;
            FullyQualifiedItemPath::from_fully_qualified_key(&key).map_err(|error| {
                CatalogueLinterError::InvalidRuleConfig(FreeText::new(format!(
                    "invalid external trait implementation path '{}': {error}",
                    path.as_str()
                )))
            })
        }
        Err(error) => Err(CatalogueLinterError::IdentityResolutionFailed(error)),
    }
}

fn resolve_trait_identity(
    catalogue: &CatalogueDocument,
    reference: &TypeRef,
    trait_identities: &BTreeSet<FullyQualifiedItemPath>,
    external_trait_identities: &BTreeSet<FullyQualifiedItemPath>,
) -> Result<Option<FullyQualifiedItemPath>, CatalogueLinterError> {
    match resolve_catalogue_identity(reference, catalogue.crate_name(), trait_identities) {
        Ok(identity) => Ok(Some(identity)),
        Err(CatalogueIdentityResolutionError::UnresolvedIdentifier(_)) => {
            match resolve_catalogue_identity(
                reference,
                catalogue.crate_name(),
                external_trait_identities,
            ) {
                Ok(identity) => Ok(Some(identity)),
                Err(CatalogueIdentityResolutionError::UnresolvedIdentifier(_)) => Ok(None),
                Err(error) => Err(CatalogueLinterError::IdentityResolutionFailed(error)),
            }
        }
        Err(error) => Err(CatalogueLinterError::IdentityResolutionFailed(error)),
    }
}

fn impl_type_parameters(impl_decl: &TraitImplDeclV2) -> Vec<ParamName> {
    impl_decl.impl_generics().iter().map(|generic| generic.name.clone()).collect()
}

/// Returns whether `bare_name` is a delimiter-bounded component of `sig_type`.
pub(super) fn bare_name_in_type_ref(sig_type: &str, bare_name: &str) -> bool {
    identifier_name_in_str(sig_type, bare_name, |_| true)
}

pub(super) fn identifier_name_in_str(
    source: &str,
    name: &str,
    accept_start: impl Fn(&str) -> bool,
) -> bool {
    let mut rest = source;
    while let Some(pos) = rest.find(name) {
        let before = &rest[..pos];
        let before_ok = before.chars().next_back().is_none_or(|c| !c.is_alphanumeric() && c != '_');
        let after_pos = pos + name.len();
        let after_ok = after_pos == rest.len()
            || rest[after_pos..].chars().next().is_some_and(|c| !c.is_alphanumeric() && c != '_');
        if before_ok && after_ok && accept_start(before) {
            return true;
        }
        if after_pos >= rest.len() {
            break;
        }
        rest = &rest[after_pos..];
    }
    false
}

// ---------------------------------------------------------------------------
// DataRole field accessor helpers
// ---------------------------------------------------------------------------

/// Returns the identity accessor method name for roles that carry one.
pub(super) fn identity_accessor_name(role: &DataRole) -> Option<&str> {
    match role {
        DataRole::Entity { identity, .. } => Some(identity.method_name().as_str()),
        DataRole::AggregateRoot { identity, .. } => Some(identity.method_name().as_str()),
        _ => None,
    }
}

/// Returns the invariants slice for roles that carry one.
pub(super) fn invariants_for_role(
    role: &DataRole,
) -> &[crate::tddd::catalogue_v2::roles::InvariantDecl] {
    match role {
        DataRole::ValueObject { invariants } => invariants.as_slice(),
        DataRole::Entity { invariants, .. } => invariants.as_slice(),
        DataRole::AggregateRoot { invariants, .. } => invariants.as_slice(),
        _ => &[],
    }
}

/// Validates that `field` is a `DataRole` field (as opposed to a
/// `ContractRole`-only field such as `Aggregate`, or the accessor-only
/// `Identity` field).
///
/// This must be called before any loop over type entries so that a
/// wrong-category `target_field` is rejected even when the catalogue contains
/// no matching entries for the rule's `RuleTarget` (D19 fail-closed).
/// `RolePayloadField` is a closed enum, so a totally unrecognised field name
/// is unrepresentable here (rejected earlier, at the usecase config-parsing
/// boundary); this validation covers the remaining runtime-checkable failure
/// mode — a syntactically valid field that is the wrong category for this use.
///
/// # Errors
///
/// Returns [`CatalogueLinterError::InvalidRuleConfig`] when `field` is
/// `Identity` or `Aggregate` (not `DataRole` fields).
pub(super) fn validate_data_role_field(
    field: RolePayloadField,
) -> Result<(), CatalogueLinterError> {
    match field {
        RolePayloadField::Invariants
        | RolePayloadField::ExclusiveMembers
        | RolePayloadField::SharedValueObjects
        | RolePayloadField::Emits
        | RolePayloadField::Handles
        | RolePayloadField::ReactsTo => Ok(()),
        RolePayloadField::Identity | RolePayloadField::Aggregate => {
            Err(CatalogueLinterError::InvalidRuleConfig(FreeText::new(format!(
                "target_field '{field}' is not a recognised DataRole field name; \
                 valid DataRole fields are: exclusive_members, shared_value_objects, emits, handles, \
                 reacts_to, invariants"
            ))))
        }
    }
}

/// Validates that `field` is a `ContractRole` field (currently only
/// `Aggregate`).
///
/// This must be called before any loop over trait entries so that a
/// wrong-category `target_field` is rejected even when the catalogue contains
/// no matching entries for the rule's `RuleTarget` (D19 fail-closed).
/// `RolePayloadField` is a closed enum, so a totally unrecognised field name
/// is unrepresentable here (rejected earlier, at the usecase config-parsing
/// boundary); this validation covers the remaining runtime-checkable failure
/// mode — a syntactically valid field that is not a `ContractRole` field.
///
/// # Errors
///
/// Returns [`CatalogueLinterError::InvalidRuleConfig`] when `field` is not
/// `Aggregate`.
pub(super) fn validate_contract_role_field(
    field: RolePayloadField,
) -> Result<(), CatalogueLinterError> {
    match field {
        RolePayloadField::Aggregate => Ok(()),
        RolePayloadField::Invariants
        | RolePayloadField::Identity
        | RolePayloadField::ExclusiveMembers
        | RolePayloadField::SharedValueObjects
        | RolePayloadField::Emits
        | RolePayloadField::Handles
        | RolePayloadField::ReactsTo => {
            Err(CatalogueLinterError::InvalidRuleConfig(FreeText::new(format!(
                "unknown target_field '{field}' for ContractRole: not a recognised ContractRole field name; \
             valid names are: aggregate"
            ))))
        }
    }
}

/// Returns `true` when the named field Vec for the given role is empty (or the
/// role does not carry that field).
///
/// For `Invariants`, delegates to [`invariants_for_role`] because invariants
/// use `InvariantDecl` rather than `TypeRef` and are not visible through
/// [`field_type_refs`]. `RolePayloadField` is a closed enum, so an unrecognised
/// field name is unrepresentable here; this function is infallible.
pub(super) fn field_vec_is_empty(role: &DataRole, field: RolePayloadField) -> bool {
    if field == RolePayloadField::Invariants {
        return invariants_for_role(role).is_empty();
    }
    field_type_refs(role, field).is_empty()
}

/// Returns the `TypeRef` slice for a named field on a `DataRole`.
///
/// Returns an empty slice when the field is valid but the given role does not
/// carry that field (e.g. `Emits` on `DataRole::Entity`), or when the field is
/// not `TypeRef`-backed at all (`Invariants`) or is `ContractRole`-only
/// (`Aggregate`, `Identity`). `RolePayloadField` is a closed enum, so an
/// unrecognised field name is unrepresentable here; this function is
/// infallible.
pub(super) fn field_type_refs(
    role: &DataRole,
    field: RolePayloadField,
) -> &[crate::tddd::catalogue_v2::identifiers::TypeRef] {
    match field {
        // `invariants` uses `InvariantDecl`, not `TypeRef`; callers that need
        // invariants should use `invariants_for_role` directly.
        RolePayloadField::Invariants => &[],
        RolePayloadField::ExclusiveMembers => {
            if let DataRole::AggregateRoot { exclusive_members, .. } = role {
                exclusive_members.as_slice()
            } else {
                &[]
            }
        }
        RolePayloadField::SharedValueObjects => {
            if let DataRole::AggregateRoot { shared_value_objects, .. } = role {
                shared_value_objects.as_slice()
            } else {
                &[]
            }
        }
        RolePayloadField::Emits => match role {
            DataRole::AggregateRoot { emits, .. } | DataRole::DomainService { emits } => {
                emits.as_slice()
            }
            _ => &[],
        },
        RolePayloadField::Handles => {
            if let DataRole::UseCase { handles } = role {
                handles.as_slice()
            } else {
                &[]
            }
        }
        RolePayloadField::ReactsTo => {
            if let DataRole::EventPolicy { reacts_to } = role {
                reacts_to.as_slice()
            } else {
                &[]
            }
        }
        // ContractRole-only fields — no DataRole variant carries these.
        RolePayloadField::Aggregate | RolePayloadField::Identity => &[],
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::tddd::LayerId;
    use crate::tddd::catalogue_linter::{ExtractedTypeRefPath, TypeRefPathExtractionError};
    use crate::tddd::catalogue_v2::composite::{StructKind, StructShape, TypeKindV2};
    use crate::tddd::catalogue_v2::entries::{InherentImplDeclV2, TraitEntry, TypeEntry};
    use crate::tddd::catalogue_v2::identifiers::{CrateName, MethodName, ModulePath, ParamName};
    use crate::tddd::catalogue_v2::methods::{MethodDeclaration, MethodGenericParam};
    use crate::tddd::catalogue_v2::roles::ContractRole;
    use crate::tddd::catalogue_v2::traits::TraitImplDeclV2;

    struct TestTypeRefExtractor;

    impl TypeRefPathExtractorPort for TestTypeRefExtractor {
        fn extract(
            &self,
            type_ref: &TypeRef,
            type_parameters: &[ParamName],
            _lifetime_parameters: &[crate::tddd::catalogue_v2::identifiers::ParamName],
            _const_parameters: &[crate::tddd::catalogue_v2::identifiers::ParamName],
        ) -> Result<Vec<ExtractedTypeRefPath>, TypeRefPathExtractionError> {
            if type_parameters.iter().any(|parameter| parameter.as_str() == type_ref.as_str()) {
                return Ok(vec![ExtractedTypeRefPath::TypeParameter(
                    ParamName::new(type_ref.as_str().to_owned())
                        .expect("test extractor emits a valid parameter name"),
                )]);
            }
            let path = match type_ref.as_str() {
                "PartialEq garbage" => {
                    return Err(TypeRefPathExtractionError::UnsupportedSyntax {
                        location: type_ref.clone(),
                    });
                }
                "core :: cmp :: PartialEq" => "core::cmp::PartialEq",
                "PartialEq<Self>" => "PartialEq",
                // Mirror the syntax-only adapter's occurrence contract: a
                // reference or tuple contributes its nested path, while a
                // generic path contributes its root path.
                "&Thing" | "(Thing,)" => "Thing",
                "&Required" => "Required",
                "Thing<T>"
                | "Thing<T> + Other<U>"
                | "Thing<{ b'{' as usize }>"
                | "Thing<{ b'>' as usize }>"
                | "Thing<{ '>' as usize }>"
                | "Thing<{ \">\" }>"
                | "Thing<{ b\">\" }>"
                | "Thing<{ 1 < 2 }>"
                | "Thing<{ b'\\x3c' as usize }>"
                | "Thing<'static, Other<'static>>"
                | "Thing<{ f(r#\"a\"<\"b\"#) }>"
                | "Thing<fn() -> Other>"
                | "Thing<T /* < */> + Other<U /* > */>"
                | "Thing /* path comment */ <T>"
                | "r#Thing<T>" => "Thing",
                "Vec<Thing>" => "Vec",
                "Thing<T> + Send" | "Thing " => "Thing",
                value => value,
            };
            Ok(vec![ExtractedTypeRefPath::Path(
                TypeRef::new(path.to_owned()).expect("test extractor emits a non-empty path"),
            )])
        }
    }

    const EXTRACTOR: TestTypeRefExtractor = TestTypeRefExtractor;

    fn catalogue_with_type_and_required_trait(
        type_key: &str,
        type_module_path: ModulePath,
    ) -> CatalogueDocument {
        let mut catalogue = CatalogueDocument::new(
            5,
            CrateName::new("domain").expect("valid crate name"),
            LayerId::try_new("domain").expect("valid layer"),
        );
        catalogue.insert_type(
            CatalogueEntryKey::try_new(type_key.to_owned()).expect("valid type key"),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::value_object(),
                TypeKindV2::Struct(StructKind::new(StructShape::Unit, None)),
                vec![],
                vec![],
                vec![],
                type_module_path,
                None,
                vec![],
                vec![],
            ),
        );
        catalogue.insert_trait(
            CatalogueEntryKey::try_new("Required".to_owned()).expect("valid trait key"),
            TraitEntry::new(
                ItemAction::Add,
                ContractRole::SpecificationPort,
                vec![],
                vec![],
                vec![],
                vec![],
                vec![],
                vec![],
                ModulePath::root(),
                None,
                vec![],
                vec![],
            ),
        );
        catalogue
    }

    #[test]
    fn test_collect_methods_resolves_qualified_type_key_and_short_impl_owner() {
        let mut catalogue = CatalogueDocument::new(
            5,
            CrateName::new("domain").expect("valid crate name"),
            LayerId::try_new("domain").expect("valid layer"),
        );
        let type_key =
            CatalogueEntryKey::try_new("a::Thing".to_owned()).expect("valid qualified type key");
        let entry = TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::Struct(StructKind::new(StructShape::Unit, None)),
            vec![],
            vec![],
            vec![],
            ModulePath::from_segments(vec!["a".to_owned()]).expect("valid module path"),
            None,
            vec![],
            vec![],
        );
        catalogue.insert_type(type_key.clone(), entry);
        catalogue.push_inherent_impl(InherentImplDeclV2::new(
            CatalogueEntryKey::try_new("Thing".to_owned()).expect("valid short type key"),
            vec![],
            vec![],
            vec![MethodDeclaration::associated_function(
                MethodName::new("from_impl").expect("valid method name"),
                vec![],
                TypeRef::new("()").expect("valid return type"),
            )],
        ));

        let entry = catalogue.types().get(&type_key).expect("type entry is present");
        let methods = collect_methods_for_type(&catalogue, entry, type_key.as_str())
            .expect("equivalent catalogue identities must match");

        assert_eq!(methods.len(), 1);
        assert_eq!(methods.first().expect("one method is present").name().as_str(), "from_impl");
    }

    #[test]
    fn test_collect_methods_rejects_unresolved_catalogue_qualified_impl_owner() {
        let mut catalogue = CatalogueDocument::new(
            5,
            CrateName::new("domain").expect("valid crate name"),
            LayerId::try_new("domain").expect("valid layer"),
        );
        let type_key =
            CatalogueEntryKey::try_new("a::Thing".to_owned()).expect("valid qualified type key");
        catalogue.insert_type(
            type_key.clone(),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::value_object(),
                TypeKindV2::Struct(StructKind::new(StructShape::Unit, None)),
                vec![],
                vec![],
                vec![],
                ModulePath::from_segments(vec!["a".to_owned()]).expect("valid module path"),
                None,
                vec![],
                vec![],
            ),
        );
        catalogue.push_inherent_impl(InherentImplDeclV2::new(
            CatalogueEntryKey::try_new("crate::missing::Thing".to_owned())
                .expect("valid unresolved catalogue path"),
            vec![],
            vec![],
            vec![MethodDeclaration::associated_function(
                MethodName::new("must_not_be_skipped").expect("valid method name"),
                vec![],
                TypeRef::new("()").expect("valid return type"),
            )],
        ));

        let entry = catalogue.types().get(&type_key).expect("type entry is present");
        let error = collect_methods_for_type(&catalogue, entry, type_key.as_str())
            .expect_err("an unresolved in-catalogue owner must fail closed");
        assert!(matches!(
            error,
            CatalogueLinterError::IdentityResolutionFailed(
                CatalogueIdentityResolutionError::UnresolvedIdentifier(reference)
            ) if reference.as_str() == "crate::missing::Thing"
        ));
    }

    #[test]
    fn test_collect_methods_rejects_external_impl_owner() {
        let mut catalogue = CatalogueDocument::new(
            5,
            CrateName::new("domain").expect("valid crate name"),
            LayerId::try_new("domain").expect("valid layer"),
        );
        let type_key = CatalogueEntryKey::try_new("Thing".to_owned()).expect("valid type key");
        catalogue.insert_type(
            type_key.clone(),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::value_object(),
                TypeKindV2::Struct(StructKind::new(StructShape::Unit, None)),
                vec![],
                vec![],
                vec![],
                ModulePath::root(),
                None,
                vec![],
                vec![],
            ),
        );
        catalogue.push_inherent_impl(InherentImplDeclV2::new(
            CatalogueEntryKey::try_new("external_crate::Thing".to_owned())
                .expect("valid external path"),
            vec![],
            vec![],
            vec![],
        ));

        let entry = catalogue.types().get(&type_key).expect("type entry is present");
        let error = collect_methods_for_type(&catalogue, entry, type_key.as_str())
            .expect_err("inherent impls cannot target external types");
        assert!(matches!(
            error,
            CatalogueLinterError::IdentityResolutionFailed(
                CatalogueIdentityResolutionError::UnresolvedIdentifier(reference)
            ) if reference.as_str() == "external_crate::Thing"
        ));
    }

    #[test]
    fn test_has_trait_impl_resolves_qualified_type_key_and_short_impl_owner() {
        let mut catalogue = CatalogueDocument::new(
            5,
            CrateName::new("domain").expect("valid crate name"),
            LayerId::try_new("domain").expect("valid layer"),
        );
        catalogue.insert_type(
            CatalogueEntryKey::try_new("a::Thing".to_owned()).expect("valid type key"),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::value_object(),
                TypeKindV2::Struct(StructKind::new(StructShape::Unit, None)),
                vec![],
                vec![],
                vec![],
                ModulePath::from_segments(vec!["a".to_owned()]).expect("valid module path"),
                None,
                vec![],
                vec![],
            ),
        );
        catalogue.insert_trait(
            CatalogueEntryKey::try_new("a::Required".to_owned()).expect("valid trait key"),
            TraitEntry::new(
                ItemAction::Add,
                ContractRole::SpecificationPort,
                vec![],
                vec![],
                vec![],
                vec![],
                vec![],
                vec![],
                ModulePath::from_segments(vec!["a".to_owned()]).expect("valid module path"),
                None,
                vec![],
                vec![],
            ),
        );
        catalogue.push_trait_impl(TraitImplDeclV2::new(
            TypeRef::new("Required").expect("valid trait ref"),
            TypeRef::new("Thing").expect("valid for type"),
        ));

        assert!(
            has_trait_impl(
                &catalogue,
                "a::Thing",
                &ModulePath::from_segments(vec!["a".to_owned()]).expect("valid module path"),
                "Required",
                &EXTRACTOR,
            )
            .expect("trait implementation identity must resolve")
        );
    }

    #[test]
    fn test_has_trait_impl_rejects_reference_for_type_as_root_identity() {
        let mut catalogue = catalogue_with_type_and_required_trait("Thing", ModulePath::root());
        catalogue.push_trait_impl(TraitImplDeclV2::new(
            TypeRef::new("Required").expect("valid trait ref"),
            TypeRef::new("&Thing").expect("valid reference type"),
        ));

        let result =
            has_trait_impl(&catalogue, "Thing", &ModulePath::root(), "Required", &EXTRACTOR);

        assert!(matches!(
            result,
            Err(CatalogueLinterError::IdentityResolutionFailed(
                CatalogueIdentityResolutionError::ClassificationFailed { location }
            )) if location.as_str() == "&Thing"
        ));
    }

    #[test]
    fn test_has_trait_impl_rejects_singleton_tuple_for_type_as_root_identity() {
        let mut catalogue = catalogue_with_type_and_required_trait("Thing", ModulePath::root());
        catalogue.push_trait_impl(TraitImplDeclV2::new(
            TypeRef::new("Required").expect("valid trait ref"),
            TypeRef::new("(Thing,)").expect("valid tuple type"),
        ));

        let result =
            has_trait_impl(&catalogue, "Thing", &ModulePath::root(), "Required", &EXTRACTOR);

        assert!(matches!(
            result,
            Err(CatalogueLinterError::IdentityResolutionFailed(
                CatalogueIdentityResolutionError::ClassificationFailed { location }
            )) if location.as_str() == "(Thing,)"
        ));
    }

    #[test]
    fn test_has_trait_impl_rejects_reference_trait_as_required_identity() {
        let mut catalogue = catalogue_with_type_and_required_trait("Thing", ModulePath::root());
        catalogue.push_trait_impl(TraitImplDeclV2::new(
            TypeRef::new("&Required").expect("valid reference trait ref"),
            TypeRef::new("Thing").expect("valid for type"),
        ));

        let result =
            has_trait_impl(&catalogue, "Thing", &ModulePath::root(), "Required", &EXTRACTOR);

        assert!(matches!(
            result,
            Err(CatalogueLinterError::IdentityResolutionFailed(
                CatalogueIdentityResolutionError::ClassificationFailed { location }
            )) if location.as_str() == "&Required"
        ));
    }

    #[test]
    fn test_has_trait_impl_accepts_bare_generic_and_qualified_type_root_shapes() {
        let mut bare = catalogue_with_type_and_required_trait("Thing", ModulePath::root());
        bare.push_trait_impl(TraitImplDeclV2::new(
            TypeRef::new("Required").expect("valid trait ref"),
            TypeRef::new("Thing").expect("valid bare for type"),
        ));
        assert!(
            has_trait_impl(&bare, "Thing", &ModulePath::root(), "Required", &EXTRACTOR,)
                .expect("bare root path should resolve")
        );

        let mut generic = catalogue_with_type_and_required_trait("Thing", ModulePath::root());
        generic.push_trait_impl(TraitImplDeclV2::from_parts(
            ItemAction::Add,
            TypeRef::new("Required").expect("valid trait ref"),
            TypeRef::new("Thing<T>").expect("valid generic for type"),
            vec![MethodGenericParam {
                name: ParamName::new("T").expect("valid generic parameter"),
                bounds: vec![],
            }],
            vec![],
        ));
        assert!(
            has_trait_impl(&generic, "Thing", &ModulePath::root(), "Required", &EXTRACTOR,)
                .expect("generic root path should resolve")
        );

        let module_path =
            ModulePath::from_segments(vec!["module".to_owned()]).expect("valid module path");
        let mut qualified =
            catalogue_with_type_and_required_trait("module::Thing", module_path.clone());
        qualified.push_trait_impl(TraitImplDeclV2::new(
            TypeRef::new("Required").expect("valid trait ref"),
            TypeRef::new("domain::module::Thing").expect("valid fully qualified for type"),
        ));
        assert!(
            has_trait_impl(&qualified, "module::Thing", &module_path, "Required", &EXTRACTOR,)
                .expect("fully qualified root path should resolve")
        );

        let mut nested = catalogue_with_type_and_required_trait("Vec", ModulePath::root());
        nested.push_trait_impl(TraitImplDeclV2::new(
            TypeRef::new("Required").expect("valid trait ref"),
            TypeRef::new("Vec<Thing>").expect("valid nested generic for type"),
        ));
        assert!(
            has_trait_impl(&nested, "Vec", &ModulePath::root(), "Required", &EXTRACTOR,)
                .expect("the outer path must remain the nominal root of nested generics")
        );
    }

    #[test]
    fn test_has_trait_impl_accepts_const_block_with_literal_brace_as_root_shape() {
        let mut catalogue = catalogue_with_type_and_required_trait("Thing", ModulePath::root());
        catalogue.push_trait_impl(TraitImplDeclV2::new(
            TypeRef::new("Required").expect("valid trait ref"),
            TypeRef::new("Thing<{ b'{' as usize }>").expect("valid const-generic for type"),
        ));

        assert!(
            has_trait_impl(&catalogue, "Thing", &ModulePath::root(), "Required", &EXTRACTOR,)
                .expect("a brace inside a parsed const block must not invalidate the root path")
        );
    }

    #[test]
    fn test_has_trait_impl_accepts_const_block_with_literal_angle_as_root_shape() {
        let mut catalogue = catalogue_with_type_and_required_trait("Thing", ModulePath::root());
        catalogue.push_trait_impl(TraitImplDeclV2::new(
            TypeRef::new("Required").expect("valid trait ref"),
            TypeRef::new("Thing<{ b'>' as usize }>").expect("valid const-generic for type"),
        ));

        assert!(
            has_trait_impl(&catalogue, "Thing", &ModulePath::root(), "Required", &EXTRACTOR,)
                .expect("an angle inside a parsed const literal must not affect root validation")
        );
    }

    #[test]
    fn test_has_trait_impl_accepts_all_angle_containing_literal_shapes() {
        for type_ref in ["Thing<{ '>' as usize }>", "Thing<{ \">\" }>", "Thing<{ b\">\" }>"] {
            let mut catalogue = catalogue_with_type_and_required_trait("Thing", ModulePath::root());
            catalogue.push_trait_impl(TraitImplDeclV2::new(
                TypeRef::new("Required").expect("valid trait ref"),
                TypeRef::new(type_ref).expect("valid literal-containing const-generic type"),
            ));

            assert!(
                has_trait_impl(&catalogue, "Thing", &ModulePath::root(), "Required", &EXTRACTOR,)
                    .expect("literal contents must not affect generic-depth validation"),
                "expected {type_ref} to retain its nominal root"
            );
        }
    }

    #[test]
    fn test_has_trait_impl_accepts_lexically_ambiguous_generic_contents() {
        for type_ref in [
            "Thing<{ 1 < 2 }>",
            "Thing<{ b'\\x3c' as usize }>",
            "Thing<'static, Other<'static>>",
            "Thing<{ f(r#\"a\"<\"b\"#) }>",
            "Thing<fn() -> Other>",
            "Thing /* path comment */ <T>",
        ] {
            let mut catalogue = catalogue_with_type_and_required_trait("Thing", ModulePath::root());
            catalogue.push_trait_impl(TraitImplDeclV2::new(
                TypeRef::new("Required").expect("valid trait ref"),
                TypeRef::new(type_ref).expect("valid generic type"),
            ));

            assert!(
                has_trait_impl(&catalogue, "Thing", &ModulePath::root(), "Required", &EXTRACTOR,)
                    .expect("parser-accepted generic contents must retain their nominal root"),
                "expected {type_ref} to retain its nominal root"
            );
        }
    }

    #[test]
    fn test_has_trait_impl_rejects_compound_root_when_comments_hide_delimiters() {
        let mut catalogue = catalogue_with_type_and_required_trait("Thing", ModulePath::root());
        catalogue.push_trait_impl(TraitImplDeclV2::new(
            TypeRef::new("Required").expect("valid trait ref"),
            TypeRef::new("Thing<T /* < */> + Other<U /* > */>")
                .expect("valid commented compound trait-object type"),
        ));

        let error =
            has_trait_impl(&catalogue, "Thing", &ModulePath::root(), "Required", &EXTRACTOR)
                .expect_err("comments must not hide a compound root from fail-closed validation");
        assert!(matches!(
            error,
            CatalogueLinterError::IdentityResolutionFailed(
                CatalogueIdentityResolutionError::ClassificationFailed { location }
            ) if location.as_str() == "Thing<T /* < */> + Other<U /* > */>"
        ));
    }

    #[test]
    fn test_has_trait_impl_accepts_raw_identifier_generic_as_root_shape() {
        let mut catalogue = catalogue_with_type_and_required_trait("Thing", ModulePath::root());
        catalogue.push_trait_impl(TraitImplDeclV2::from_parts(
            ItemAction::Add,
            TypeRef::new("Required").expect("valid trait ref"),
            TypeRef::new("r#Thing<T>").expect("valid raw-identifier generic for type"),
            vec![MethodGenericParam {
                name: ParamName::new("T").expect("valid generic parameter"),
                bounds: vec![],
            }],
            vec![],
        ));

        assert!(
            has_trait_impl(&catalogue, "Thing", &ModulePath::root(), "Required", &EXTRACTOR,)
                .expect("the adapter's raw-identifier canonicalization must be accepted")
        );
    }

    #[test]
    fn test_has_trait_impl_rejects_trait_bound_after_generic_root_shape() {
        let mut catalogue = catalogue_with_type_and_required_trait("Thing", ModulePath::root());
        catalogue.push_trait_impl(TraitImplDeclV2::from_parts(
            ItemAction::Add,
            TypeRef::new("Required").expect("valid trait ref"),
            TypeRef::new("Thing<T> + Send").expect("valid trait-object type"),
            vec![MethodGenericParam {
                name: ParamName::new("T").expect("valid generic parameter"),
                bounds: vec![],
            }],
            vec![],
        ));

        let error =
            has_trait_impl(&catalogue, "Thing", &ModulePath::root(), "Required", &EXTRACTOR)
                .expect_err("a trait bound must not be treated as a nominal root path");
        assert!(matches!(
            error,
            CatalogueLinterError::IdentityResolutionFailed(
                CatalogueIdentityResolutionError::ClassificationFailed { location }
            ) if location.as_str() == "Thing<T> + Send"
        ));
    }

    #[test]
    fn test_has_trait_impl_rejects_compound_trait_object_root_shape() {
        let mut catalogue = catalogue_with_type_and_required_trait("Thing", ModulePath::root());
        catalogue.push_trait_impl(TraitImplDeclV2::from_parts(
            ItemAction::Add,
            TypeRef::new("Required").expect("valid trait ref"),
            TypeRef::new("Thing<T> + Other<U>").expect("valid compound trait-object type"),
            vec![MethodGenericParam {
                name: ParamName::new("T").expect("valid generic parameter"),
                bounds: vec![],
            }],
            vec![],
        ));

        let error =
            has_trait_impl(&catalogue, "Thing", &ModulePath::root(), "Required", &EXTRACTOR)
                .expect_err("a compound trait-object form must not satisfy a nominal root impl");
        assert!(matches!(
            error,
            CatalogueLinterError::IdentityResolutionFailed(
                CatalogueIdentityResolutionError::ClassificationFailed { location }
            ) if location.as_str() == "Thing<T> + Other<U>"
        ));
    }

    #[test]
    fn test_has_trait_impl_accepts_trailing_whitespace_after_root_path() {
        let mut catalogue = catalogue_with_type_and_required_trait("Thing", ModulePath::root());
        catalogue.push_trait_impl(TraitImplDeclV2::new(
            TypeRef::new("Required").expect("valid trait ref"),
            TypeRef::new("Thing ").expect("valid path with trailing whitespace"),
        ));

        assert!(
            has_trait_impl(&catalogue, "Thing", &ModulePath::root(), "Required", &EXTRACTOR,)
                .expect("trailing whitespace is not part of the root identity")
        );
    }

    #[test]
    fn test_has_trait_impl_rejects_unresolved_external_trait_with_matching_short_name() {
        let mut catalogue = CatalogueDocument::new(
            5,
            CrateName::new("domain").expect("valid crate name"),
            LayerId::try_new("domain").expect("valid layer"),
        );
        catalogue.insert_type(
            CatalogueEntryKey::try_new("a::Thing".to_owned()).expect("valid type key"),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::value_object(),
                TypeKindV2::Struct(StructKind::new(StructShape::Unit, None)),
                vec![],
                vec![],
                vec![],
                ModulePath::from_segments(vec!["a".to_owned()]).expect("valid module path"),
                None,
                vec![],
                vec![],
            ),
        );
        catalogue.push_trait_impl(TraitImplDeclV2::new(
            TypeRef::new("evil::PartialEq").expect("valid external trait ref"),
            TypeRef::new("Thing").expect("valid for type"),
        ));

        let error = has_trait_impl(
            &catalogue,
            "a::Thing",
            &ModulePath::from_segments(vec!["a".to_owned()]).expect("valid module path"),
            "PartialEq",
            &EXTRACTOR,
        )
        .expect_err("colliding external trait names must fail closed as ambiguous");

        let CatalogueLinterError::IdentityResolutionFailed(
            CatalogueIdentityResolutionError::AmbiguousIdentifier(_, candidates),
        ) = error
        else {
            panic!("expected an ambiguity error with both candidate identities");
        };
        let candidate_paths =
            candidates.as_slice().iter().map(ToString::to_string).collect::<Vec<_>>();
        assert_eq!(candidate_paths, ["core::cmp::PartialEq", "evil::PartialEq"]);
    }

    #[test]
    fn test_has_trait_impl_accepts_canonical_external_standard_traits() {
        let mut catalogue = CatalogueDocument::new(
            5,
            CrateName::new("domain").expect("valid crate name"),
            LayerId::try_new("domain").expect("valid layer"),
        );
        catalogue.insert_type(
            CatalogueEntryKey::try_new("Thing".to_owned()).expect("valid type key"),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::value_object(),
                TypeKindV2::Struct(StructKind::new(StructShape::Unit, None)),
                vec![],
                vec![],
                vec![],
                ModulePath::root(),
                None,
                vec![],
                vec![],
            ),
        );
        catalogue.push_trait_impl(TraitImplDeclV2::new(
            TypeRef::new("core::cmp::PartialEq").expect("valid canonical trait ref"),
            TypeRef::new("Thing").expect("valid for type"),
        ));

        let present =
            has_trait_impl(&catalogue, "Thing", &ModulePath::root(), "PartialEq", &EXTRACTOR)
                .expect("canonical external trait should resolve");

        assert!(present);

        catalogue.push_trait_impl(TraitImplDeclV2::new(
            TypeRef::new("core::fmt::Debug").expect("valid canonical debug trait ref"),
            TypeRef::new("Thing").expect("valid for type"),
        ));
        let debug_present =
            has_trait_impl(&catalogue, "Thing", &ModulePath::root(), "Debug", &EXTRACTOR)
                .expect("canonical debug trait should resolve");

        assert!(debug_present);

        catalogue.push_trait_impl(TraitImplDeclV2::new(
            TypeRef::new("RangeBounds").expect("valid bare external trait ref"),
            TypeRef::new("Thing").expect("valid for type"),
        ));
        let range_bounds_present =
            has_trait_impl(&catalogue, "Thing", &ModulePath::root(), "RangeBounds", &EXTRACTOR)
                .expect("bare standard external trait should resolve");

        assert!(range_bounds_present);
    }

    #[test]
    fn test_has_trait_impl_resolves_std_trait_to_canonical_core_identity() {
        let mut catalogue = CatalogueDocument::new(
            5,
            CrateName::new("domain").expect("valid crate name"),
            LayerId::try_new("domain").expect("valid layer"),
        );
        catalogue.insert_type(
            CatalogueEntryKey::try_new("Thing".to_owned()).expect("valid type key"),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::value_object(),
                TypeKindV2::Struct(StructKind::new(StructShape::Unit, None)),
                vec![],
                vec![],
                vec![],
                ModulePath::root(),
                None,
                vec![],
                vec![],
            ),
        );
        catalogue.push_trait_impl(TraitImplDeclV2::new(
            TypeRef::new("std::cmp::PartialEq").expect("valid std trait ref"),
            TypeRef::new("Thing").expect("valid for type"),
        ));

        let present =
            has_trait_impl(&catalogue, "Thing", &ModulePath::root(), "PartialEq", &EXTRACTOR)
                .expect("std re-export must resolve to the canonical core identity");

        assert!(present);
    }

    #[test]
    fn test_has_trait_impl_resolves_absolute_external_trait_path() {
        let mut catalogue = CatalogueDocument::new(
            5,
            CrateName::new("domain").expect("valid crate name"),
            LayerId::try_new("domain").expect("valid layer"),
        );
        catalogue.insert_type(
            CatalogueEntryKey::try_new("Thing".to_owned()).expect("valid type key"),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::value_object(),
                TypeKindV2::Struct(StructKind::new(StructShape::Unit, None)),
                vec![],
                vec![],
                vec![],
                ModulePath::root(),
                None,
                vec![],
                vec![],
            ),
        );
        catalogue.push_trait_impl(TraitImplDeclV2::new(
            TypeRef::new("::external_crate::Trait").expect("valid absolute trait ref"),
            TypeRef::new("Thing").expect("valid for type"),
        ));

        let present = has_trait_impl(&catalogue, "Thing", &ModulePath::root(), "Trait", &EXTRACTOR)
            .expect("absolute external trait path should resolve");

        assert!(present);
    }

    #[test]
    fn test_allowed_external_trait_identities_does_not_classify_crate_alias_as_external() {
        let mut catalogue = CatalogueDocument::new(
            5,
            CrateName::new("domain").expect("valid crate name"),
            LayerId::try_new("domain").expect("valid layer"),
        );
        catalogue.insert_type(
            CatalogueEntryKey::try_new("Thing".to_owned()).expect("valid type key"),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::value_object(),
                TypeKindV2::Struct(StructKind::new(StructShape::Unit, None)),
                vec![],
                vec![],
                vec![],
                ModulePath::root(),
                None,
                vec![],
                vec![],
            ),
        );
        catalogue.insert_trait(
            CatalogueEntryKey::try_new("SomeTrait".to_owned()).expect("valid trait key"),
            TraitEntry::new(
                ItemAction::Add,
                ContractRole::SpecificationPort,
                vec![],
                vec![],
                vec![],
                vec![],
                vec![],
                vec![],
                ModulePath::root(),
                None,
                vec![],
                vec![],
            ),
        );
        catalogue.push_trait_impl(TraitImplDeclV2::new(
            TypeRef::new("crate::SomeTrait").expect("valid crate-relative trait ref"),
            TypeRef::new("Thing").expect("valid for type"),
        ));

        let external_identities = allowed_external_trait_identities(&catalogue, &EXTRACTOR)
            .expect("crate-relative trait path should not be invalid configuration");
        assert!(
            !external_identities.iter().any(|identity| identity.to_string() == "crate::SomeTrait")
        );

        assert!(
            has_trait_impl(&catalogue, "Thing", &ModulePath::root(), "SomeTrait", &EXTRACTOR)
                .expect("crate-relative trait path should resolve through the local universe")
        );
    }

    #[test]
    fn test_has_trait_impl_rejects_malformed_trait_reference() {
        let mut catalogue = CatalogueDocument::new(
            5,
            CrateName::new("domain").expect("valid crate name"),
            LayerId::try_new("domain").expect("valid layer"),
        );
        catalogue.insert_type(
            CatalogueEntryKey::try_new("Thing".to_owned()).expect("valid type key"),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::value_object(),
                TypeKindV2::Struct(StructKind::new(StructShape::Unit, None)),
                vec![],
                vec![],
                vec![],
                ModulePath::root(),
                None,
                vec![],
                vec![],
            ),
        );
        catalogue.push_trait_impl(TraitImplDeclV2::new(
            TypeRef::new("PartialEq garbage").expect("non-empty malformed reference"),
            TypeRef::new("Thing").expect("valid for type"),
        ));

        let error =
            has_trait_impl(&catalogue, "Thing", &ModulePath::root(), "PartialEq", &EXTRACTOR)
                .expect_err("malformed trait syntax must fail closed");
        assert!(matches!(error, CatalogueLinterError::PathExtractionFailed(_)));
    }

    #[test]
    fn test_has_trait_impl_does_not_resolve_impl_generic_as_catalogue_type() {
        let mut catalogue = CatalogueDocument::new(
            5,
            CrateName::new("domain").expect("valid crate name"),
            LayerId::try_new("domain").expect("valid layer"),
        );
        catalogue.insert_type(
            CatalogueEntryKey::try_new("T".to_owned()).expect("valid type key"),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::value_object(),
                TypeKindV2::Struct(StructKind::new(StructShape::Unit, None)),
                vec![],
                vec![],
                vec![],
                ModulePath::root(),
                None,
                vec![],
                vec![],
            ),
        );
        let generic = MethodGenericParam {
            name: ParamName::new("T").expect("valid parameter name"),
            bounds: vec![],
        };
        catalogue.push_trait_impl(TraitImplDeclV2::from_parts(
            ItemAction::Add,
            TypeRef::new("PartialEq").expect("valid trait ref"),
            TypeRef::new("T").expect("valid generic self type"),
            vec![generic],
            vec![],
        ));

        let error = has_trait_impl(&catalogue, "T", &ModulePath::root(), "PartialEq", &EXTRACTOR)
            .expect_err("impl generic labels must not resolve as catalogue identities");
        assert!(matches!(
            error,
            CatalogueLinterError::IdentityResolutionFailed(
                CatalogueIdentityResolutionError::ClassificationFailed { location }
            ) if location.as_str() == "T"
        ));
    }
}
