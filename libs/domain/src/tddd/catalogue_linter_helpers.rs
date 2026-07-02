//! Internal helper functions for catalogue linter evaluation.
//!
//! This module is declared by `catalogue_linter.rs` via `#[path]` and is not
//! a public module. All items are `pub(super)` so they are visible to
//! `evaluate_catalogue_lint` in `catalogue_linter_eval.rs`.

use super::{CatalogueLinterError, FreeText, RoleKind, RolePayloadField, RuleTarget};
use crate::tddd::catalogue_v2::CatalogueDocument;
use crate::tddd::catalogue_v2::composite::{StructKind, StructShape};
use crate::tddd::catalogue_v2::entries::{FunctionEntry, TraitEntry, TypeEntry};
use crate::tddd::catalogue_v2::identifiers::{FunctionPath, TraitName, TypeName, TypeRef};
use crate::tddd::catalogue_v2::methods::MethodDeclaration;
use crate::tddd::catalogue_v2::roles::{ContractRole, DataRole, ItemAction};

// ---------------------------------------------------------------------------
// Entry filtering helpers
// ---------------------------------------------------------------------------

/// Returns the `RoleKind` for a `TypeEntry`'s `DataRole`.
pub(super) fn entry_role_kind(entry: &TypeEntry) -> RoleKind {
    RoleKind::from_data_role(&entry.role)
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
) -> impl Iterator<Item = (&'a TypeName, &'a TypeEntry)> {
    catalogue.types.iter().filter(move |(_name, entry)| {
        entry.action != ItemAction::Delete
            && entry.action != ItemAction::Reference
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
) -> impl Iterator<Item = (&'a TraitName, &'a TraitEntry)> {
    catalogue.traits.iter().filter(move |(_name, entry)| {
        entry.action != ItemAction::Delete
            && entry.action != ItemAction::Reference
            && target_matches(target, RoleKind::from_contract_role(&entry.role))
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
    catalogue.functions.iter().filter(move |(_path, entry)| {
        entry.action != ItemAction::Delete
            && entry.action != ItemAction::Reference
            && target_matches(target, RoleKind::from_function_role(&entry.role))
    })
}

/// Aggregates all `MethodDeclaration` items for `type_name` from both the
/// `TypeEntry.methods` slice and any matching `CatalogueDocument::inherent_impls`
/// blocks.
///
/// Method names are the logical identity for inherent methods because Rust does
/// not allow overloads by signature. If both catalogue sources contain the same
/// method name with identical declarations, later duplicates are ignored. If the
/// duplicate declarations differ, this fails closed with `InvalidRuleConfig`
/// instead of letting a stale source hide or fabricate a method-level violation.
///
/// The caller is responsible for ensuring the `TypeEntry` itself has not been
/// filtered out by `action: Delete` before calling this function (e.g. via
/// `type_entries_for_target`). `InherentImplDeclV2` has no `action` field; all
/// inherent impl blocks matching `type_name` are included.
///
/// Returns a `Vec` so the caller can iterate freely without lifetime entanglement
/// across two separate slices.
pub(super) fn collect_methods_for_type<'a>(
    catalogue: &'a CatalogueDocument,
    entry: &'a TypeEntry,
    type_name: &str,
) -> Result<Vec<&'a MethodDeclaration>, CatalogueLinterError> {
    let mut methods = Vec::new();
    let mut seen_names = std::collections::BTreeMap::new();

    for method in entry.methods.iter().chain(
        catalogue
            .inherent_impls
            .iter()
            .filter(|impl_| impl_.type_name.as_str() == type_name)
            .flat_map(|impl_| impl_.methods.iter()),
    ) {
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

/// Returns `true` when the catalogue document has a `trait_impls` entry
/// where `for_type.as_str()` matches `type_name` and `trait_ref.as_str()`
/// matches `trait_name_prefix`.
///
/// `for_type` matching rules (any of):
/// - Exact match: `"Foo"` matches type key `"Foo"`.
/// - Generic self type: `"Foo<T>"` or `"Foo<L, R>"` matches bare key `"Foo"` via
///   the `<` suffix — `TraitImplDeclV2.for_type` may carry type parameters while
///   the catalogue `TypeEntry` key is always the bare type name.
///
/// `trait_ref` matching rules (all checked):
/// - Exact match: `"PartialEq"` matches `"PartialEq"`.
/// - Generic suffix: `"PartialEq<Self>"` matches prefix `"PartialEq"`.
/// - Space suffix: `"PartialEq "` matches prefix `"PartialEq"`.
/// - Path-qualified: `"core::cmp::PartialEq"` matches prefix `"PartialEq"` via
///   the `::TraitName` path tail (also handles generics on path-qualified forms).
pub(super) fn has_trait_impl(
    catalogue: &CatalogueDocument,
    type_name: &str,
    trait_name_prefix: &str,
) -> bool {
    let path_suffix = format!("::{trait_name_prefix}");
    catalogue.trait_impls.iter().any(|ti| {
        // Exclude delete-action impl entries: a deleted impl does not count as present.
        if ti.action == ItemAction::Delete {
            return false;
        }
        let for_type = ti.for_type.as_str();
        let trait_ref = ti.trait_ref.as_str();
        // Match `for_type` either exactly or as a generic self type (e.g. "Foo<T>").
        let for_type_matches = for_type == type_name
            || for_type.starts_with(&format!("{type_name}<"))
            || for_type.starts_with(&format!("{type_name} "));
        for_type_matches
            && (trait_ref == trait_name_prefix
                || trait_ref.starts_with(&format!("{trait_name_prefix}<"))
                || trait_ref.starts_with(&format!("{trait_name_prefix} "))
                || trait_ref.ends_with(&path_suffix)
                || trait_ref.contains(&format!("{path_suffix}<"))
                || trait_ref.contains(&format!("{path_suffix} ")))
    })
}

/// Returns `true` when the bare type name `bare_name` appears as a standalone
/// type component inside `sig_type`.
///
/// Matches:
/// - Exact match: `"OrderRepo" == "OrderRepo"`
/// - Wrapped in generics / references: `"Vec<OrderRepo>"`, `"Option<OrderRepo>"`,
///   `"&OrderRepo"`, `"&mut OrderRepo"`, `"(OrderRepo, X)"`, etc.
/// - Path-qualified: `"ports::OrderRepo"`, `"crate::OrderRepo"` — `::` counts as a
///   boundary so the bare name tail is matched correctly.
///
/// The check uses delimiter-boundary scanning: `bare_name` must be preceded by a
/// type-separator character (`<`, `,`, ` `, `(`, `[`, `&`, `*`, `:`, `+`) or be at the
/// start of the string, and followed by a type-separator character (`>`, `,`, ` `, `)`,
/// `]`, `<`, `;`, `:`, `+`) or be at the end of the string.
///
/// `[` / `]` covers slice and array signatures (`&[OrderRepo]`, `[OrderLine; 4]`).
/// `;` covers array length separators (`[T; N]`).
/// `+` covers trait-object / impl-trait bounds written without spaces
/// (`&dyn OrderRepository+Send`).
pub(super) fn bare_name_in_type_ref(sig_type: &str, bare_name: &str) -> bool {
    if sig_type == bare_name {
        return true;
    }
    // `:` covers `::` path separators: the char immediately before the name will be `:`.
    // `[`/`]` covers slice and array type expressions.
    // `;` covers array length separator (`[T; N]`).
    // `+` covers trait-object / impl-trait bounds (`&dyn Trait+Send`, `impl Trait+Sync`).
    let start_chars: &[char] = &['<', ',', ' ', '(', '[', '&', '*', ':', '+'];
    let end_chars: &[char] = &['>', ',', ' ', ')', ']', '<', ';', ':', '+'];
    let mut rest = sig_type;
    while let Some(pos) = rest.find(bare_name) {
        let before_ok =
            pos == 0 || rest[..pos].chars().next_back().is_some_and(|c| start_chars.contains(&c));
        let after_pos = pos + bare_name.len();
        let after_ok = after_pos == rest.len()
            || rest[after_pos..].chars().next().is_some_and(|c| end_chars.contains(&c));
        if before_ok && after_ok {
            return true;
        }
        // Advance past this occurrence to avoid infinite loop.
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
