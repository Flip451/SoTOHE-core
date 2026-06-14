//! Role → kind_tag mapping helpers for the type-signal evaluator.
//!
//! These functions map v3 catalogue roles to the `kind_tag` strings written into
//! `<layer>-type-signals.json` and consumed by `check_type_signals`.  The
//! kind_tag values are the historically-established set grandfathered by ADR
//! `2026-04-12-1200-strict-spec-signal-gate-v2`.

use domain::tddd::catalogue_v2::composite::TypeKindV2;
use domain::tddd::catalogue_v2::roles::{ContractRole, DataRole, FunctionRole};

/// Maps a v3 `DataRole` + `TypeKindV2` pair to the kind_tag string written into
/// `<layer>-type-signals.json` and consumed by `check_type_signals`.
///
/// This function is the **authoritative definition** of the kind_tag value scheme
/// for v3 `DataRole` entries. The kind_tag values are the historically-established
/// set grandfathered by ADR `2026-04-12-1200-strict-spec-signal-gate-v2`:
///
/// - `TypeKindV2::Struct(StructKind { typestate: Some(_), .. })` (any shape) → `"typestate"`
/// - `TypeKindV2::Enum { .. }` + `DataRole::ErrorType` → `"error_type"`
/// - `TypeKindV2::Enum { .. }` otherwise → `"enum"`
/// - Struct/TypeAlias with `Entity | AggregateRoot | Specification | ValueObject`
///   → `"value_object"` (these four roles share the same kind_tag)
/// - All other struct/alias roles → their canonical snake_case name:
///   `DomainService` → `"domain_service"`, `Factory` → `"factory"`,
///   `UseCase` → `"use_case"`, `Interactor` → `"interactor"`,
///   `Command` → `"command"`, `Query` → `"query"`, `Dto` → `"dto"`,
///   `ErrorType` (non-enum) → `"error_type"`, `SecondaryAdapter` → `"secondary_adapter"`
pub(crate) fn data_role_kind_tag(role: &DataRole, kind: &TypeKindV2) -> &'static str {
    // Typestate detection: Struct (any shape) with a typestate marker.
    if let TypeKindV2::Struct(sk) = kind {
        if sk.typestate.is_some() {
            return "typestate";
        }
    }
    // Enum kind: role determines error_type vs enum.
    if matches!(kind, TypeKindV2::Enum { .. }) {
        return if matches!(role, DataRole::ErrorType) { "error_type" } else { "enum" };
    }
    // Struct (no typestate) / TypeAlias: role-based mapping.
    match role {
        DataRole::ValueObject { .. }
        | DataRole::Entity { .. }
        | DataRole::AggregateRoot { .. }
        | DataRole::Specification => "value_object",
        DataRole::DomainService { .. } => "domain_service",
        DataRole::Factory => "factory",
        DataRole::UseCase { .. } => "use_case",
        DataRole::Interactor => "interactor",
        DataRole::Command => "command",
        DataRole::Query => "query",
        DataRole::Dto => "dto",
        DataRole::ErrorType => "error_type",
        DataRole::SecondaryAdapter => "secondary_adapter",
        DataRole::EventPolicy { .. } => "event_policy",
    }
}

/// Maps a `ContractRole` (v3 catalogue trait entry role) to the kind_tag string
/// written into `<layer>-type-signals.json` and consumed by `check_type_signals`.
///
/// This function is the **authoritative definition** of the kind_tag value scheme
/// for v3 `ContractRole` entries. The kind_tag values are the historically-established
/// set grandfathered by ADR `2026-04-12-1200-strict-spec-signal-gate-v2`:
///
/// - `SpecificationPort | SecondaryPort` → `"secondary_port"`
///   (both v3 `ContractRole` variants map to the same kind_tag; `SpecificationPort`
///   has no distinct kind_tag at this level)
/// - `ApplicationService` → `"application_service"`
pub(crate) const fn contract_role_kind_tag(role: &ContractRole) -> &'static str {
    match role {
        ContractRole::SpecificationPort
        | ContractRole::SecondaryPort
        | ContractRole::Repository { .. } => "secondary_port",
        ContractRole::ApplicationService => "application_service",
    }
}

/// Maps a `FunctionRole` (v3 catalogue function entry role) to the kind_tag string
/// written into `<layer>-type-signals.json` and consumed by `check_type_signals`.
///
/// This function is the **authoritative definition** of the kind_tag value scheme
/// for v3 `FunctionRole` entries. The kind_tag value is the historically-established
/// value grandfathered by ADR `2026-04-12-1200-strict-spec-signal-gate-v2`:
///
/// - All `FunctionRole` variants (`FreeFunction`, `UseCaseFunction`) → `"free_function"`
///   (both v3 function roles share the same kind_tag)
pub(crate) const fn function_role_kind_tag(_role: FunctionRole) -> &'static str {
    "free_function"
}
