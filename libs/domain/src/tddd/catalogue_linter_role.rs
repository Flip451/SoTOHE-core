//! `RoleKind` — payload-free role discriminant (D15 / D17).
//!
//! This module is declared by `catalogue_linter.rs` via `#[path]` so that
//! `RoleKind` and its inherent methods can live in a separate file while
//! remaining part of the `catalogue_linter` module (re-exported via
//! `pub use role::RoleKind;`). Mirrors the same split already used for
//! `catalogue_linter_helpers.rs` / `catalogue_linter_eval.rs` /
//! `catalogue_linter_eval_primitives.rs`.

use super::RolePayloadField;
use crate::tddd::catalogue_v2::roles::{ContractRole, DataRole, FunctionRole};

// ---------------------------------------------------------------------------
// RoleKind — payload-free role discriminant
// ---------------------------------------------------------------------------

/// Payload-free discriminant that covers every `DataRole`, `ContractRole`, and
/// `FunctionRole` variant (D15 / D17).
///
/// Used in [`super::RuleTarget`] and in rule kind payloads such as
/// [`super::CatalogueLinterRuleKind::NoRoleInMethodSignature`] where the rule must
/// reference a role across all role enums (e.g. `RoleKind::Repository` is a
/// `ContractRole` variant; `RoleKind::FreeFunction` is a `FunctionRole` variant).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RoleKind {
    // --- DataRole variants (17) ---
    /// `DataRole::ValueObject`
    ValueObject,
    /// `DataRole::Entity`
    Entity,
    /// `DataRole::AggregateRoot`
    AggregateRoot,
    /// `DataRole::DomainService`
    DomainService,
    /// `DataRole::Specification`
    Specification,
    /// `DataRole::Factory`
    Factory,
    /// `DataRole::UseCase`
    UseCase,
    /// `DataRole::Interactor`
    Interactor,
    /// `DataRole::Command`
    Command,
    /// `DataRole::Query`
    Query,
    /// `DataRole::Dto`
    Dto,
    /// `DataRole::ErrorType`
    ErrorType,
    /// `DataRole::SecondaryAdapter`
    SecondaryAdapter,
    /// `DataRole::EventPolicy`
    EventPolicy,
    /// `DataRole::DomainEvent`
    DomainEvent,
    /// `DataRole::CompositionRoot`
    CompositionRoot,
    /// `DataRole::PrimaryAdapter`
    PrimaryAdapter,
    // --- ContractRole variants (4) ---
    /// `ContractRole::SpecificationPort`
    SpecificationPort,
    /// `ContractRole::ApplicationService`
    ApplicationService,
    /// `ContractRole::SecondaryPort`
    SecondaryPort,
    /// `ContractRole::Repository`
    Repository,
    // --- FunctionRole variants (2) ---
    /// `FunctionRole::FreeFunction`
    FreeFunction,
    /// `FunctionRole::UseCaseFunction`
    UseCaseFunction,
}

impl RoleKind {
    /// Every role discriminant that a rule target can name.
    pub(crate) const ALL: [Self; 23] = [
        Self::ValueObject,
        Self::Entity,
        Self::AggregateRoot,
        Self::DomainService,
        Self::Specification,
        Self::Factory,
        Self::UseCase,
        Self::Interactor,
        Self::Command,
        Self::Query,
        Self::Dto,
        Self::ErrorType,
        Self::SecondaryAdapter,
        Self::EventPolicy,
        Self::DomainEvent,
        Self::CompositionRoot,
        Self::PrimaryAdapter,
        Self::SpecificationPort,
        Self::ApplicationService,
        Self::SecondaryPort,
        Self::Repository,
        Self::FreeFunction,
        Self::UseCaseFunction,
    ];

    /// All `DataRole` discriminants that a type-entry field rule can scan.
    pub(crate) const DATA_ROLES: [Self; 17] = [
        Self::ValueObject,
        Self::Entity,
        Self::AggregateRoot,
        Self::DomainService,
        Self::Specification,
        Self::Factory,
        Self::UseCase,
        Self::Interactor,
        Self::Command,
        Self::Query,
        Self::Dto,
        Self::ErrorType,
        Self::SecondaryAdapter,
        Self::EventPolicy,
        Self::DomainEvent,
        Self::CompositionRoot,
        Self::PrimaryAdapter,
    ];

    /// All `FunctionRole` discriminants.
    pub(crate) const FUNCTION_ROLES: [Self; 2] = [Self::FreeFunction, Self::UseCaseFunction];

    /// Returns the payload-free discriminant for a `DataRole`.
    #[must_use]
    pub fn from_data_role(role: &DataRole) -> Self {
        match role {
            DataRole::ValueObject { .. } => Self::ValueObject,
            DataRole::Entity { .. } => Self::Entity,
            DataRole::AggregateRoot { .. } => Self::AggregateRoot,
            DataRole::DomainService { .. } => Self::DomainService,
            DataRole::Specification => Self::Specification,
            DataRole::Factory => Self::Factory,
            DataRole::UseCase { .. } => Self::UseCase,
            DataRole::Interactor => Self::Interactor,
            DataRole::Command => Self::Command,
            DataRole::Query => Self::Query,
            DataRole::Dto => Self::Dto,
            DataRole::ErrorType => Self::ErrorType,
            DataRole::SecondaryAdapter => Self::SecondaryAdapter,
            DataRole::EventPolicy { .. } => Self::EventPolicy,
            DataRole::DomainEvent => Self::DomainEvent,
            DataRole::CompositionRoot => Self::CompositionRoot,
            DataRole::PrimaryAdapter => Self::PrimaryAdapter,
        }
    }

    /// Returns the payload-free discriminant for a `ContractRole`.
    #[must_use]
    pub fn from_contract_role(role: &ContractRole) -> Self {
        match role {
            ContractRole::SpecificationPort => Self::SpecificationPort,
            ContractRole::ApplicationService => Self::ApplicationService,
            ContractRole::SecondaryPort => Self::SecondaryPort,
            ContractRole::Repository { .. } => Self::Repository,
        }
    }

    /// Returns the payload-free discriminant for a `FunctionRole`.
    #[must_use]
    pub fn from_function_role(role: &FunctionRole) -> Self {
        match role {
            FunctionRole::FreeFunction => Self::FreeFunction,
            FunctionRole::UseCaseFunction => Self::UseCaseFunction,
        }
    }

    /// Returns a stable display name for this discriminant.
    #[must_use]
    pub fn variant_name(self) -> &'static str {
        match self {
            Self::ValueObject => "ValueObject",
            Self::Entity => "Entity",
            Self::AggregateRoot => "AggregateRoot",
            Self::DomainService => "DomainService",
            Self::Specification => "Specification",
            Self::Factory => "Factory",
            Self::UseCase => "UseCase",
            Self::Interactor => "Interactor",
            Self::Command => "Command",
            Self::Query => "Query",
            Self::Dto => "Dto",
            Self::ErrorType => "ErrorType",
            Self::SecondaryAdapter => "SecondaryAdapter",
            Self::EventPolicy => "EventPolicy",
            Self::DomainEvent => "DomainEvent",
            Self::CompositionRoot => "CompositionRoot",
            Self::PrimaryAdapter => "PrimaryAdapter",
            Self::SpecificationPort => "SpecificationPort",
            Self::ApplicationService => "ApplicationService",
            Self::SecondaryPort => "SecondaryPort",
            Self::Repository => "Repository",
            Self::FreeFunction => "FreeFunction",
            Self::UseCaseFunction => "UseCaseFunction",
        }
    }

    /// Returns `true` when this discriminant carries the named `TypeRef` field.
    ///
    /// Used by the `ReferencedRoleConstraint` pre-check to reject
    /// `target_role × target_field` combinations that cannot produce any role
    /// reference checks (D19 fail-closed).
    ///
    /// `pub(crate)` — internal helper; not part of the public API surface.
    #[must_use]
    pub(crate) fn carries_type_ref_field(self, field: RolePayloadField) -> bool {
        match field {
            RolePayloadField::ExclusiveMembers | RolePayloadField::SharedValueObjects => {
                matches!(self, Self::AggregateRoot)
            }
            RolePayloadField::Emits => matches!(self, Self::AggregateRoot | Self::DomainService),
            RolePayloadField::Handles => matches!(self, Self::UseCase),
            RolePayloadField::ReactsTo => matches!(self, Self::EventPolicy),
            RolePayloadField::Aggregate => matches!(self, Self::Repository),
            RolePayloadField::Invariants | RolePayloadField::Identity => false,
        }
    }

    /// Returns `true` when this discriminant carries the named `DataRole` field.
    ///
    /// Covers both `TypeRef` fields (delegating to [`carries_type_ref_field`])
    /// and `InvariantDecl` fields (`"invariants"`).
    ///
    /// Used by `FieldEmpty` / `FieldNonEmpty` pre-checks to reject
    /// `target_role × target_field` combinations where any target role does not
    /// carry the field (D19 fail-closed).  Both rules iterate type entries and
    /// inspect a field on each entry's `DataRole`; a role that never carries
    /// the field would silently treat every entry as having an empty vec,
    /// producing false positives.
    ///
    /// `pub(crate)` — internal helper; not part of the public API surface.
    #[must_use]
    pub(crate) fn carries_data_role_field(self, field: RolePayloadField) -> bool {
        match field {
            RolePayloadField::Invariants => {
                matches!(self, Self::ValueObject | Self::Entity | Self::AggregateRoot)
            }
            other => self.carries_type_ref_field(other),
        }
    }
}
