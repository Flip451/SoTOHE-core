//! Role enums and action/receiver/layer enums for the catalogue v2 schema.
//!
//! Implements all 6 enums from the TDDD v2 domain-types.json:
//! - `DataRole` (13 values) — for `TypeEntry`
//! - `ContractRole` (3 values) — for `TraitEntry`
//! - `FunctionRole` (2 values) — for `FunctionEntry`
//! - `ItemAction` (4 values) — per-entry action
//! - `SelfReceiver` (3 values) — method self-receiver form
//! - `Layer` (3 values) — architectural layer
//!
//! All enums derive `strum::Display` and `strum::EnumString` for string
//! round-trips, consistent with the domain serde-free policy
//! (ADR `knowledge/adr/2026-04-14-1531-domain-serde-ripout.md`).
//!
//! No serde derives — the infrastructure codec layer handles JSON serialization.

// Re-export strum traits to make them available to callers.
pub use strum::EnumString;
pub use strum::IntoStaticStr;

// ---------------------------------------------------------------------------
// DataRole — 13 values for TypeEntry
// ---------------------------------------------------------------------------

/// Role enum for `TypeEntry` (struct / enum / type alias).
///
/// Declares the DDD / Clean Architecture role of a data type. `TypeEntry` only
/// accepts `DataRole`; attaching `ContractRole` or `FunctionRole` to a `TypeEntry`
/// is a parse-time type error (ADR 1 D2).
///
/// 13 values covering domain layer through infrastructure layer roles:
/// `ValueObject`, `Entity`, `AggregateRoot`, `DomainService`, `Specification`,
/// `Factory`, `UseCase`, `Interactor`, `Command`, `Query`, `Dto`,
/// `ErrorType`, `SecondaryAdapter`.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, strum::Display, strum::EnumString, strum::IntoStaticStr,
)]
pub enum DataRole {
    /// A value object — immutable, equality by value (DDD).
    ValueObject,
    /// An entity — mutable, equality by identity (DDD).
    Entity,
    /// An aggregate root — consistency boundary (DDD).
    AggregateRoot,
    /// A domain service — stateless domain logic without direct entity ownership (DDD).
    DomainService,
    /// A specification / predicate object (DDD Specification pattern).
    Specification,
    /// A factory — responsible for constructing complex domain objects (DDD).
    Factory,
    /// A use case — orchestrates domain entities to fulfil a user story (Clean Architecture).
    UseCase,
    /// An interactor — concrete implementation of a use case (Clean Architecture variant).
    Interactor,
    /// A command — represents a write intention (CQRS).
    Command,
    /// A query — represents a read intention (CQRS).
    Query,
    /// A data transfer object — carries data across boundaries.
    Dto,
    /// An error type — domain or application error.
    ErrorType,
    /// A secondary adapter — infrastructure implementation of a domain port.
    SecondaryAdapter,
}

// ---------------------------------------------------------------------------
// ContractRole — 3 values for TraitEntry
// ---------------------------------------------------------------------------

/// Role enum for `TraitEntry`.
///
/// Declares the architectural role of a trait (contract / port). Attaching
/// `ContractRole` to a `TypeEntry` is a parse-time type error (ADR 1 D2).
///
/// 3 values: `SpecificationPort`, `ApplicationService`, `SecondaryPort`.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, strum::Display, strum::EnumString, strum::IntoStaticStr,
)]
pub enum ContractRole {
    /// A specification port — driven port in hexagonal architecture (domain boundary).
    SpecificationPort,
    /// An application service — use case interface (Clean Architecture).
    ApplicationService,
    /// A secondary port — driving port for infrastructure adapters (domain boundary).
    SecondaryPort,
}

// ---------------------------------------------------------------------------
// FunctionRole — 2 values for FunctionEntry
// ---------------------------------------------------------------------------

/// Role enum for `FunctionEntry`.
///
/// Declares the architectural role of a free function (ADR 1 D2).
///
/// 2 values: `FreeFunction`, `UseCaseFunction`.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, strum::Display, strum::EnumString, strum::IntoStaticStr,
)]
pub enum FunctionRole {
    /// A free function without a specific use-case responsibility.
    FreeFunction,
    /// A function implementing a use-case entrypoint.
    UseCaseFunction,
}

// ---------------------------------------------------------------------------
// ItemAction — 4 values per catalogue entry
// ---------------------------------------------------------------------------

/// Action for each catalogue entry.
///
/// Inherits semantics from TDDD-03 (ADR 1 D4). Serde default when the `action`
/// field is absent in JSON should decode to `Add` — the codec layer handles this.
///
/// The `Display` / `FromStr` format uses lowercase snake_case (`"add"`, `"modify"`,
/// `"reference"`, `"delete"`) to match the TDDD-03 JSON catalogue wire format, which
/// has been using lowercase since the first release (see ADR `2026-04-11-0003`).
///
/// 4 values: `Add`, `Modify`, `Reference`, `Delete`.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    strum::Display,
    strum::EnumString,
    strum::IntoStaticStr,
    Default,
)]
#[strum(serialize_all = "snake_case")]
pub enum ItemAction {
    /// Add a new type / trait / function (default action when omitted).
    #[default]
    Add,
    /// Modify an existing type / trait / function.
    Modify,
    /// Reference an existing type / trait / function for documentation / intent.
    Reference,
    /// Intentionally delete a type / trait / function.
    Delete,
}

// ---------------------------------------------------------------------------
// SelfReceiver — 3 values for method self-receiver form
// ---------------------------------------------------------------------------

/// Enum for `MethodDeclaration` receiver (ADR 1 D8).
///
/// `Option<SelfReceiver>` is used in `MethodDeclaration`:
/// - `None` means an associated function (no `self` receiver).
/// - `Some(SelfReceiver::Owned)` means `self`.
/// - `Some(SelfReceiver::SharedRef)` means `&self`.
/// - `Some(SelfReceiver::ExclusiveRef)` means `&mut self`.
///
/// The `Display` / `FromStr` format matches the Rust receiver token syntax used in
/// existing catalogue JSON (`"self"`, `"&self"`, `"&mut self"`), consistent with how
/// V1 `MethodDeclaration.receiver: Option<String>` encoded receivers and how ADR 1 D8
/// documents each variant.
///
/// 3 values: `Owned`, `SharedRef`, `ExclusiveRef`.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, strum::Display, strum::EnumString, strum::IntoStaticStr,
)]
pub enum SelfReceiver {
    /// Consumes `self` (value receiver). Display: `"self"`.
    #[strum(serialize = "self")]
    Owned,
    /// Borrows `self` immutably. Display: `"&self"`.
    #[strum(serialize = "&self")]
    SharedRef,
    /// Borrows `self` mutably. Display: `"&mut self"`.
    #[strum(serialize = "&mut self")]
    ExclusiveRef,
}

// ---------------------------------------------------------------------------
// Layer — 3 values for CatalogueDocument::layer
// ---------------------------------------------------------------------------

/// Enum for `CatalogueDocument::layer`.
///
/// Declares the architectural layer of the catalogue file (ADR 1 D1 / D6).
/// One catalogue document corresponds to one crate in one layer.
///
/// The `Display` / `FromStr` format uses lowercase (`"domain"`, `"usecase"`,
/// `"infrastructure"`) matching the three lowercase layer axis values defined in
/// ADR 1 D1.
///
/// 3 values: `Domain`, `Usecase`, `Infrastructure`.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, strum::Display, strum::EnumString, strum::IntoStaticStr,
)]
#[strum(serialize_all = "snake_case")]
pub enum Layer {
    /// The domain layer — pure business logic, no I/O.
    Domain,
    /// The usecase / application layer — orchestration of domain logic.
    Usecase,
    /// The infrastructure layer — I/O adapters, codecs, persistence.
    Infrastructure,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    // Manual variant arrays replace strum::EnumIter to avoid generating public
    // <EnumName>Iter structs that trigger the catalogue declaration check.

    const ALL_DATA_ROLES: &[DataRole] = &[
        DataRole::ValueObject,
        DataRole::Entity,
        DataRole::AggregateRoot,
        DataRole::DomainService,
        DataRole::Specification,
        DataRole::Factory,
        DataRole::UseCase,
        DataRole::Interactor,
        DataRole::Command,
        DataRole::Query,
        DataRole::Dto,
        DataRole::ErrorType,
        DataRole::SecondaryAdapter,
    ];

    const ALL_CONTRACT_ROLES: &[ContractRole] = &[
        ContractRole::SpecificationPort,
        ContractRole::ApplicationService,
        ContractRole::SecondaryPort,
    ];

    const ALL_FUNCTION_ROLES: &[FunctionRole] =
        &[FunctionRole::FreeFunction, FunctionRole::UseCaseFunction];

    const ALL_ITEM_ACTIONS: &[ItemAction] =
        &[ItemAction::Add, ItemAction::Modify, ItemAction::Reference, ItemAction::Delete];

    const ALL_SELF_RECEIVERS: &[SelfReceiver] =
        &[SelfReceiver::Owned, SelfReceiver::SharedRef, SelfReceiver::ExclusiveRef];

    const ALL_LAYERS: &[Layer] = &[Layer::Domain, Layer::Usecase, Layer::Infrastructure];

    // -----------------------------------------------------------------------
    // DataRole — 13 values
    // -----------------------------------------------------------------------

    #[test]
    fn test_data_role_has_13_variants() {
        assert_eq!(ALL_DATA_ROLES.len(), 13);
    }

    #[test]
    fn test_data_role_display_fromstr_roundtrip_all_variants() {
        for role in ALL_DATA_ROLES {
            let s = role.to_string();
            let parsed: DataRole = s.parse().unwrap();
            assert_eq!(*role, parsed, "roundtrip failed for DataRole::{role:?}");
        }
    }

    #[test]
    fn test_data_role_value_object_display() {
        assert_eq!(DataRole::ValueObject.to_string(), "ValueObject");
    }

    #[test]
    fn test_data_role_secondary_adapter_display() {
        assert_eq!(DataRole::SecondaryAdapter.to_string(), "SecondaryAdapter");
    }

    #[test]
    fn test_data_role_fromstr_with_invalid_value_returns_error() {
        let result = "SpecificationPort".parse::<DataRole>();
        assert!(result.is_err(), "ContractRole variant should not parse as DataRole");
    }

    // -----------------------------------------------------------------------
    // ContractRole — 3 values
    // -----------------------------------------------------------------------

    #[test]
    fn test_contract_role_has_3_variants() {
        assert_eq!(ALL_CONTRACT_ROLES.len(), 3);
    }

    #[test]
    fn test_contract_role_display_fromstr_roundtrip_all_variants() {
        for role in ALL_CONTRACT_ROLES {
            let s = role.to_string();
            let parsed: ContractRole = s.parse().unwrap();
            assert_eq!(*role, parsed, "roundtrip failed for ContractRole::{role:?}");
        }
    }

    #[test]
    fn test_contract_role_specification_port_display() {
        assert_eq!(ContractRole::SpecificationPort.to_string(), "SpecificationPort");
    }

    #[test]
    fn test_contract_role_fromstr_with_invalid_value_returns_error() {
        let result = "ValueObject".parse::<ContractRole>();
        assert!(result.is_err(), "DataRole variant should not parse as ContractRole");
    }

    // -----------------------------------------------------------------------
    // FunctionRole — 2 values
    // -----------------------------------------------------------------------

    #[test]
    fn test_function_role_has_2_variants() {
        assert_eq!(ALL_FUNCTION_ROLES.len(), 2);
    }

    #[test]
    fn test_function_role_display_fromstr_roundtrip_all_variants() {
        for role in ALL_FUNCTION_ROLES {
            let s = role.to_string();
            let parsed: FunctionRole = s.parse().unwrap();
            assert_eq!(*role, parsed, "roundtrip failed for FunctionRole::{role:?}");
        }
    }

    #[test]
    fn test_function_role_free_function_display() {
        assert_eq!(FunctionRole::FreeFunction.to_string(), "FreeFunction");
    }

    // -----------------------------------------------------------------------
    // ItemAction — 4 values
    // -----------------------------------------------------------------------

    #[test]
    fn test_item_action_has_4_variants() {
        assert_eq!(ALL_ITEM_ACTIONS.len(), 4);
    }

    #[test]
    fn test_item_action_display_fromstr_roundtrip_all_variants() {
        for action in ALL_ITEM_ACTIONS {
            let s = action.to_string();
            let parsed: ItemAction = s.parse().unwrap();
            assert_eq!(*action, parsed, "roundtrip failed for ItemAction::{action:?}");
        }
    }

    #[test]
    fn test_item_action_default_is_add() {
        assert_eq!(ItemAction::default(), ItemAction::Add);
    }

    #[test]
    fn test_item_action_display_uses_lowercase() {
        assert_eq!(ItemAction::Add.to_string(), "add");
        assert_eq!(ItemAction::Modify.to_string(), "modify");
        assert_eq!(ItemAction::Reference.to_string(), "reference");
        assert_eq!(ItemAction::Delete.to_string(), "delete");
    }

    #[test]
    fn test_item_action_fromstr_with_lowercase_succeeds() {
        assert_eq!("add".parse::<ItemAction>().unwrap(), ItemAction::Add);
        assert_eq!("modify".parse::<ItemAction>().unwrap(), ItemAction::Modify);
        assert_eq!("reference".parse::<ItemAction>().unwrap(), ItemAction::Reference);
        assert_eq!("delete".parse::<ItemAction>().unwrap(), ItemAction::Delete);
    }

    #[test]
    fn test_item_action_fromstr_with_invalid_value_returns_error() {
        let result = "Create".parse::<ItemAction>();
        assert!(result.is_err());
    }

    #[test]
    fn test_item_action_fromstr_with_pascal_case_returns_error() {
        // PascalCase should NOT parse — only lowercase is valid.
        assert!("Add".parse::<ItemAction>().is_err());
    }

    // -----------------------------------------------------------------------
    // SelfReceiver — 3 values
    // -----------------------------------------------------------------------

    #[test]
    fn test_self_receiver_has_3_variants() {
        assert_eq!(ALL_SELF_RECEIVERS.len(), 3);
    }

    #[test]
    fn test_self_receiver_display_fromstr_roundtrip_all_variants() {
        for receiver in ALL_SELF_RECEIVERS {
            let s = receiver.to_string();
            let parsed: SelfReceiver = s.parse().unwrap();
            assert_eq!(*receiver, parsed, "roundtrip failed for SelfReceiver::{receiver:?}");
        }
    }

    #[test]
    fn test_self_receiver_owned_display() {
        // SelfReceiver::Owned displays as the Rust receiver token "self".
        assert_eq!(SelfReceiver::Owned.to_string(), "self");
    }

    #[test]
    fn test_self_receiver_shared_ref_display() {
        // SelfReceiver::SharedRef displays as the Rust receiver token "&self".
        assert_eq!(SelfReceiver::SharedRef.to_string(), "&self");
    }

    #[test]
    fn test_self_receiver_exclusive_ref_display() {
        // SelfReceiver::ExclusiveRef displays as the Rust receiver token "&mut self".
        assert_eq!(SelfReceiver::ExclusiveRef.to_string(), "&mut self");
    }

    #[test]
    fn test_self_receiver_fromstr_with_receiver_tokens_succeeds() {
        // The receiver token strings used in catalogue JSON must parse correctly.
        assert_eq!("self".parse::<SelfReceiver>().unwrap(), SelfReceiver::Owned);
        assert_eq!("&self".parse::<SelfReceiver>().unwrap(), SelfReceiver::SharedRef);
        assert_eq!("&mut self".parse::<SelfReceiver>().unwrap(), SelfReceiver::ExclusiveRef);
    }

    // -----------------------------------------------------------------------
    // Layer — 3 values
    // -----------------------------------------------------------------------

    #[test]
    fn test_layer_has_3_variants() {
        assert_eq!(ALL_LAYERS.len(), 3);
    }

    #[test]
    fn test_layer_display_fromstr_roundtrip_all_variants() {
        for layer in ALL_LAYERS {
            let s = layer.to_string();
            let parsed: Layer = s.parse().unwrap();
            assert_eq!(*layer, parsed, "roundtrip failed for Layer::{layer:?}");
        }
    }

    #[test]
    fn test_layer_domain_display() {
        // Layer::Domain displays as the lowercase layer axis value "domain" per ADR 1 D1.
        assert_eq!(Layer::Domain.to_string(), "domain");
    }

    #[test]
    fn test_layer_fromstr_with_lowercase_succeeds() {
        // The lowercase layer axis values from ADR 1 D1 must parse correctly.
        assert_eq!("domain".parse::<Layer>().unwrap(), Layer::Domain);
        assert_eq!("usecase".parse::<Layer>().unwrap(), Layer::Usecase);
        assert_eq!("infrastructure".parse::<Layer>().unwrap(), Layer::Infrastructure);
    }

    #[test]
    fn test_layer_fromstr_with_pascal_case_returns_error() {
        // PascalCase should NOT parse — only lowercase is valid.
        let result = "Domain".parse::<Layer>();
        assert!(result.is_err(), "PascalCase 'Domain' should not parse as Layer::Domain");
    }

    // -----------------------------------------------------------------------
    // Role type separation tests
    // -----------------------------------------------------------------------

    /// Verifies that DataRole, ContractRole, and FunctionRole are distinct types.
    /// Passing a DataRole where a ContractRole is expected causes a compile error.
    #[test]
    fn test_role_types_are_distinct() {
        let data_role = DataRole::ValueObject;
        let contract_role = ContractRole::SpecificationPort;
        let function_role = FunctionRole::FreeFunction;
        // The following would be compile errors:
        // let _: ContractRole = data_role; // compile error
        // Verify distinct type by asserting string inequality
        assert_ne!(data_role.to_string(), contract_role.to_string());
        assert_ne!(contract_role.to_string(), function_role.to_string());
    }
}
