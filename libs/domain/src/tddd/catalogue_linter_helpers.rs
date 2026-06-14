//! Internal helper functions for catalogue linter evaluation.
//!
//! This module is declared by `catalogue_linter.rs` via `#[path]` and is not
//! a public module. All items are `pub(super)` so they are visible to
//! `evaluate_catalogue_lint` in `catalogue_linter_eval.rs` and to
//! `ddd_strict_preset` in `catalogue_linter_preset.rs`.

use super::{RoleKind, RuleTarget};
use crate::tddd::catalogue_v2::CatalogueDocument;
use crate::tddd::catalogue_v2::composite::{StructKind, StructShape};
use crate::tddd::catalogue_v2::entries::{TraitEntry, TypeEntry};
use crate::tddd::catalogue_v2::identifiers::{TraitName, TypeName, TypeRef};
use crate::tddd::catalogue_v2::roles::{ContractRole, DataRole};

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
pub(super) fn type_entries_for_target<'a>(
    catalogue: &'a CatalogueDocument,
    target: &RuleTarget,
) -> impl Iterator<Item = (&'a TypeName, &'a TypeEntry)> {
    catalogue
        .types
        .iter()
        .filter(move |(_name, entry)| target_matches(target, entry_role_kind(entry)))
}

/// Iterates over `(trait_name, entry)` pairs in `catalogue.traits` where the
/// entry's `ContractRole` matches the rule's `RuleTarget`.
pub(super) fn trait_entries_for_target<'a>(
    catalogue: &'a CatalogueDocument,
    target: &RuleTarget,
) -> impl Iterator<Item = (&'a TraitName, &'a TraitEntry)> {
    catalogue.traits.iter().filter(move |(_name, entry)| {
        target_matches(target, RoleKind::from_contract_role(&entry.role))
    })
}

/// Returns the `TypeRef` for the named field of a `ContractRole`, if applicable.
///
/// Currently supports `"aggregate"` on `ContractRole::Repository`.
pub(super) fn contract_role_type_ref<'a>(
    role: &'a ContractRole,
    field: &str,
) -> Option<&'a TypeRef> {
    match (field, role) {
        ("aggregate", ContractRole::Repository { aggregate }) => Some(aggregate),
        _ => None,
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

/// Returns `true` when the named field Vec for the given role is empty (or the
/// role does not carry that field).
///
/// For `"invariants"`, delegates to [`invariants_for_role`] because invariants
/// use `InvariantDecl` rather than `TypeRef` and are not visible through
/// [`field_type_refs`].
pub(super) fn field_vec_is_empty(role: &DataRole, field: &str) -> bool {
    if field == "invariants" {
        return invariants_for_role(role).is_empty();
    }
    field_type_refs(role, field).is_empty()
}

/// Returns the `Vec<TypeRef>` for a named field on a `DataRole`, or an empty
/// slice when the role doesn't carry that field.
pub(super) fn field_type_refs<'a>(
    role: &'a DataRole,
    field: &str,
) -> &'a [crate::tddd::catalogue_v2::identifiers::TypeRef] {
    match (field, role) {
        ("invariants", _) => &[], // invariants use InvariantDecl, not TypeRef
        ("exclusive_members", DataRole::AggregateRoot { exclusive_members, .. }) => {
            exclusive_members.as_slice()
        }
        ("shared_value_objects", DataRole::AggregateRoot { shared_value_objects, .. }) => {
            shared_value_objects.as_slice()
        }
        ("emits", DataRole::AggregateRoot { emits, .. }) => emits.as_slice(),
        ("emits", DataRole::DomainService { emits }) => emits.as_slice(),
        ("handles", DataRole::UseCase { handles }) => handles.as_slice(),
        ("reacts_to", DataRole::EventPolicy { reacts_to }) => reacts_to.as_slice(),
        ("aggregate", _) => &[], // ContractRole field, not DataRole — handled via trait entries
        _ => &[],
    }
}
