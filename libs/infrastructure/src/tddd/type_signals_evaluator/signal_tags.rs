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
/// This function is the **single source of truth** for the kind_tag value scheme
/// for v3 `DataRole` entries. The kind_tag values are the historically-established
/// set grandfathered by ADR `2026-04-12-1200-strict-spec-signal-gate-v2`.
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
        DataRole::DomainEvent => "domain_event",
        DataRole::CompositionRoot => "composition_root",
        DataRole::PrimaryAdapter => "primary_adapter",
    }
}

/// Maps a `ContractRole` (v3 catalogue trait entry role) to the kind_tag string
/// written into `<layer>-type-signals.json` and consumed by `check_type_signals`.
///
/// This function is the **single source of truth** for the kind_tag value scheme
/// for v3 `ContractRole` entries. The kind_tag values are the historically-established
/// set grandfathered by ADR `2026-04-12-1200-strict-spec-signal-gate-v2`.
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
/// This function is the **single source of truth** for the kind_tag value scheme
/// for v3 `FunctionRole` entries. The kind_tag values are the historically-established
/// set grandfathered by ADR `2026-04-12-1200-strict-spec-signal-gate-v2`.
pub(crate) const fn function_role_kind_tag(_role: FunctionRole) -> &'static str {
    "free_function"
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use domain::tddd::catalogue_v2::composite::TypeKindV2;
    use domain::tddd::catalogue_v2::roles::DataRole;

    /// Regression test: enum-shaped DomainEvent must keep the generic "enum"
    /// signal tag because the renderer groups non-error enum entries under the
    /// "## Enums" section.
    #[test]
    fn test_data_role_kind_tag_domain_event_enum_returns_enum_tag() {
        let enum_kind = TypeKindV2::Enum { variants: vec![] };
        let tag = data_role_kind_tag(&DataRole::DomainEvent, &enum_kind);
        assert_eq!(tag, "enum");
    }

    /// Baseline: error_type enum still produces "error_type" after the refactor.
    #[test]
    fn test_data_role_kind_tag_error_type_enum_returns_error_type_tag() {
        let enum_kind = TypeKindV2::Enum { variants: vec![] };
        let tag = data_role_kind_tag(&DataRole::ErrorType, &enum_kind);
        assert_eq!(tag, "error_type");
    }

    /// Baseline: unrelated enum role (e.g. Command) still produces generic "enum".
    #[test]
    fn test_data_role_kind_tag_command_enum_returns_enum_tag() {
        let enum_kind = TypeKindV2::Enum { variants: vec![] };
        let tag = data_role_kind_tag(&DataRole::Command, &enum_kind);
        assert_eq!(tag, "enum");
    }
}
