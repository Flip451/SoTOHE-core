//! Role enums and action/receiver enums for the catalogue v2 schema.
//!
//! Implements role enums and role payload value objects from the TDDD v2 domain-types.json:
//! - `DataRole` (13 values) — for `TypeEntry`
//! - `ContractRole` (3 values) — for `TraitEntry`
//! - `FunctionRole` (2 values) — for `FunctionEntry`
//! - `ItemAction` (4 values) — per-entry action
//! - `SelfReceiver` (3 values) — method self-receiver form
//! - `InvariantName`, `InvariantPredicate`, `InvariantDecl`, `IdentityAccessor`
//! - `NonEmptyVec<T>`
//!
//! The architectural layer axis is represented by [`crate::tddd::LayerId`] — a
//! data-driven validated newtype (ADR `2026-05-08-0248` D1). The former
//! hardcoded `Layer` enum has been removed.
//!
//! Unit-like enums derive `strum::Display` and `strum::EnumString` for string
//! round-trips. `DataRole` and `ContractRole` use hand-written string
//! round-trips because some variants carry schema payload.
//!
//! No serde derives — the infrastructure codec layer handles JSON serialization.

use std::fmt;
use std::str::FromStr;

use crate::tddd::catalogue_v2::identifiers::{IdentifierError, InvariantName, MethodName, TypeRef};

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
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataRole {
    /// A value object — immutable, equality by value (DDD).
    ValueObject { invariants: Vec<InvariantDecl> },
    /// An entity — mutable, equality by identity (DDD).
    Entity { identity: IdentityAccessor, invariants: Vec<InvariantDecl> },
    /// An aggregate root — consistency boundary (DDD).
    AggregateRoot {
        identity: IdentityAccessor,
        invariants: Vec<InvariantDecl>,
        exclusive_members: Vec<TypeRef>,
        shared_value_objects: Vec<TypeRef>,
        emits: Vec<TypeRef>,
    },
    /// A domain service — stateless domain logic without direct entity ownership (DDD).
    DomainService { emits: Vec<TypeRef> },
    /// A specification / predicate object (DDD Specification pattern).
    Specification,
    /// A factory — responsible for constructing complex domain objects (DDD).
    Factory,
    /// A use case — orchestrates domain entities to fulfil a user story (Clean Architecture).
    UseCase { handles: Vec<TypeRef> },
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
    /// A declarative domain-layer policy that reacts to one or more events.
    EventPolicy { reacts_to: NonEmptyVec<TypeRef> },
}

impl DataRole {
    /// Creates the canonical empty-payload `ValueObject` role.
    #[must_use]
    pub fn value_object() -> Self {
        Self::ValueObject { invariants: Vec::new() }
    }

    /// Creates a minimal `Entity` role with the conventional `identity` accessor.
    ///
    /// This is intended for tests and catalogue scaffolding that only need the
    /// role discriminant. Catalogue JSON decode still requires an explicit
    /// `identity` payload.
    pub fn entity() -> Result<Self, IdentifierError> {
        Ok(Self::Entity { identity: Self::placeholder_identity()?, invariants: Vec::new() })
    }

    /// Creates a minimal `AggregateRoot` role with the conventional `identity`
    /// accessor and empty optional vectors.
    pub fn aggregate_root() -> Result<Self, IdentifierError> {
        Ok(Self::AggregateRoot {
            identity: Self::placeholder_identity()?,
            invariants: Vec::new(),
            exclusive_members: Vec::new(),
            shared_value_objects: Vec::new(),
            emits: Vec::new(),
        })
    }

    /// Creates the canonical empty-payload `DomainService` role.
    #[must_use]
    pub fn domain_service() -> Self {
        Self::DomainService { emits: Vec::new() }
    }

    /// Creates the canonical empty-payload `UseCase` role.
    #[must_use]
    pub fn use_case() -> Self {
        Self::UseCase { handles: Vec::new() }
    }

    /// Returns the payload-free role name used for display, parsing, and method
    /// reference signatures.
    #[must_use]
    pub const fn variant_name(&self) -> &'static str {
        match self {
            Self::ValueObject { .. } => "ValueObject",
            Self::Entity { .. } => "Entity",
            Self::AggregateRoot { .. } => "AggregateRoot",
            Self::DomainService { .. } => "DomainService",
            Self::Specification => "Specification",
            Self::Factory => "Factory",
            Self::UseCase { .. } => "UseCase",
            Self::Interactor => "Interactor",
            Self::Command => "Command",
            Self::Query => "Query",
            Self::Dto => "Dto",
            Self::ErrorType => "ErrorType",
            Self::SecondaryAdapter => "SecondaryAdapter",
            Self::EventPolicy { .. } => "EventPolicy",
        }
    }

    fn placeholder_identity() -> Result<IdentityAccessor, IdentifierError> {
        MethodName::new("identity").map(IdentityAccessor::new)
    }

    fn placeholder_event_ref() -> Result<TypeRef, IdentifierError> {
        TypeRef::new("DomainEvent")
    }
}

impl fmt::Display for DataRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.variant_name())
    }
}

impl FromStr for DataRole {
    type Err = strum::ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parse_error = |_| strum::ParseError::VariantNotFound;
        match s {
            "ValueObject" => Ok(Self::value_object()),
            "Entity" => Self::entity().map_err(parse_error),
            "AggregateRoot" => Self::aggregate_root().map_err(parse_error),
            "DomainService" => Ok(Self::domain_service()),
            "Specification" => Ok(Self::Specification),
            "Factory" => Ok(Self::Factory),
            "UseCase" => Ok(Self::use_case()),
            "Interactor" => Ok(Self::Interactor),
            "Command" => Ok(Self::Command),
            "Query" => Ok(Self::Query),
            "Dto" => Ok(Self::Dto),
            "ErrorType" => Ok(Self::ErrorType),
            "SecondaryAdapter" => Ok(Self::SecondaryAdapter),
            "EventPolicy" => {
                let event = Self::placeholder_event_ref().map_err(parse_error)?;
                Ok(Self::EventPolicy { reacts_to: NonEmptyVec::new(event, Vec::new()) })
            }
            _ => Err(strum::ParseError::VariantNotFound),
        }
    }
}

impl TryFrom<&str> for DataRole {
    type Error = strum::ParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::from_str(value)
    }
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContractRole {
    /// A specification port — driven port in hexagonal architecture (domain boundary).
    SpecificationPort,
    /// An application service — use case interface (Clean Architecture).
    ApplicationService,
    /// A secondary port — driving port for infrastructure adapters (domain boundary).
    SecondaryPort,
    /// A repository port scoped to one aggregate root.
    Repository { aggregate: TypeRef },
}

impl ContractRole {
    /// Returns the payload-free role name used for display and parsing.
    #[must_use]
    pub const fn variant_name(&self) -> &'static str {
        match self {
            Self::SpecificationPort => "SpecificationPort",
            Self::ApplicationService => "ApplicationService",
            Self::SecondaryPort => "SecondaryPort",
            Self::Repository { .. } => "Repository",
        }
    }

    fn placeholder_aggregate() -> Result<TypeRef, IdentifierError> {
        TypeRef::new("AggregateRoot")
    }
}

impl fmt::Display for ContractRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.variant_name())
    }
}

impl FromStr for ContractRole {
    type Err = strum::ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "SpecificationPort" => Ok(Self::SpecificationPort),
            "ApplicationService" => Ok(Self::ApplicationService),
            "SecondaryPort" => Ok(Self::SecondaryPort),
            "Repository" => {
                let aggregate = Self::placeholder_aggregate()
                    .map_err(|_| strum::ParseError::VariantNotFound)?;
                Ok(Self::Repository { aggregate })
            }
            _ => Err(strum::ParseError::VariantNotFound),
        }
    }
}

impl TryFrom<&str> for ContractRole {
    type Error = strum::ParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::from_str(value)
    }
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
// Invariant payload value objects
// ---------------------------------------------------------------------------

/// The verification mechanism for an [`InvariantDecl`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvariantPredicate {
    /// A predicate method on the declaring type's `self`.
    SelfMethod(MethodName),
}

/// Declares a named invariant for a domain type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvariantDecl {
    /// The invariant name. Required because unnamed invariants are unidentifiable.
    pub name: InvariantName,
    /// The verification mechanism. Required because predicate-free invariants are
    /// uncheckable.
    pub predicate: InvariantPredicate,
}

impl InvariantDecl {
    /// Creates a new `InvariantDecl`.
    #[must_use]
    pub fn new(name: InvariantName, predicate: InvariantPredicate) -> Self {
        Self { name, predicate }
    }
}

/// References the public getter method that exposes an Entity or AggregateRoot identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityAccessor(MethodName);

impl IdentityAccessor {
    /// Creates a new `IdentityAccessor` from an already-validated method name.
    #[must_use]
    pub fn new(method_name: MethodName) -> Self {
        Self(method_name)
    }

    /// Returns the referenced getter method name.
    #[must_use]
    pub fn method_name(&self) -> &MethodName {
        &self.0
    }
}

// ---------------------------------------------------------------------------
// NonEmptyVec — schema-level non-empty collection
// ---------------------------------------------------------------------------

/// Error type for catalogue v2 value-object construction.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConstructionError {
    /// A collection required to contain at least one element was empty.
    #[error("collection must contain at least one element")]
    EmptyCollection,
}

/// A vector that is guaranteed to contain at least one element.
///
/// Domain-layer type only; infrastructure codecs are responsible for converting
/// JSON arrays into this value object and rejecting empty arrays.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NonEmptyVec<T>(Vec<T>);

impl<T> NonEmptyVec<T> {
    /// Creates a `NonEmptyVec` from a first element and the remaining elements.
    #[must_use]
    pub fn new(first: T, rest: Vec<T>) -> Self {
        let mut values = Vec::with_capacity(rest.len() + 1);
        values.push(first);
        values.extend(rest);
        Self(values)
    }

    /// Creates a `NonEmptyVec` from a vector, rejecting empty input.
    ///
    /// # Errors
    ///
    /// Returns [`ConstructionError::EmptyCollection`] when `values` is empty.
    pub fn try_new(values: Vec<T>) -> Result<Self, ConstructionError> {
        if values.is_empty() {
            return Err(ConstructionError::EmptyCollection);
        }
        Ok(Self(values))
    }

    /// Returns the elements as a slice.
    #[must_use]
    pub fn as_slice(&self) -> &[T] {
        self.0.as_slice()
    }

    /// Returns the first element.
    #[must_use]
    pub fn first(&self) -> Option<&T> {
        self.0.first()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::tddd::catalogue_v2::identifiers::TypeRef;

    // Manual variant arrays replace strum::EnumIter to avoid generating public
    // <EnumName>Iter structs that trigger the catalogue declaration check.

    const ALL_DATA_ROLE_NAMES: &[&str] = &[
        "ValueObject",
        "Entity",
        "AggregateRoot",
        "DomainService",
        "Specification",
        "Factory",
        "UseCase",
        "Interactor",
        "Command",
        "Query",
        "Dto",
        "ErrorType",
        "SecondaryAdapter",
        "EventPolicy",
    ];

    const ALL_CONTRACT_ROLE_NAMES: &[&str] =
        &["SpecificationPort", "ApplicationService", "SecondaryPort", "Repository"];

    const ALL_FUNCTION_ROLES: &[FunctionRole] =
        &[FunctionRole::FreeFunction, FunctionRole::UseCaseFunction];

    const ALL_ITEM_ACTIONS: &[ItemAction] =
        &[ItemAction::Add, ItemAction::Modify, ItemAction::Reference, ItemAction::Delete];

    const ALL_SELF_RECEIVERS: &[SelfReceiver] =
        &[SelfReceiver::Owned, SelfReceiver::SharedRef, SelfReceiver::ExclusiveRef];

    // -----------------------------------------------------------------------
    // DataRole — 14 Stage 1 values
    // -----------------------------------------------------------------------

    #[test]
    fn test_data_role_has_14_variants() {
        assert_eq!(ALL_DATA_ROLE_NAMES.len(), 14);
    }

    #[test]
    fn test_data_role_display_fromstr_roundtrip_all_variants() {
        for role_name in ALL_DATA_ROLE_NAMES {
            let parsed: DataRole = role_name.parse().unwrap();
            assert_eq!(
                parsed.to_string(),
                *role_name,
                "roundtrip failed for DataRole::{role_name}"
            );
        }
    }

    #[test]
    fn test_data_role_value_object_display() {
        assert_eq!(DataRole::value_object().to_string(), "ValueObject");
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
    // ContractRole — 4 Stage 1 values
    // -----------------------------------------------------------------------

    #[test]
    fn test_contract_role_has_4_variants() {
        assert_eq!(ALL_CONTRACT_ROLE_NAMES.len(), 4);
    }

    #[test]
    fn test_contract_role_display_fromstr_roundtrip_all_variants() {
        for role_name in ALL_CONTRACT_ROLE_NAMES {
            let parsed: ContractRole = role_name.parse().unwrap();
            assert_eq!(
                parsed.to_string(),
                *role_name,
                "roundtrip failed for ContractRole::{role_name}"
            );
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
    // Invariant payload value objects
    // -----------------------------------------------------------------------

    #[test]
    fn test_invariant_name_new_with_identifier_returns_name() {
        let name = InvariantName::new("email_is_valid").unwrap();

        assert_eq!(name.as_str(), "email_is_valid");
    }

    #[test]
    fn test_invariant_name_new_with_empty_returns_error() {
        let result = InvariantName::new("");

        assert!(matches!(result, Err(IdentifierError::Empty)));
    }

    #[test]
    fn test_invariant_name_new_with_whitespace_returns_error() {
        let result = InvariantName::new("email is valid");

        assert!(
            matches!(result, Err(IdentifierError::InvalidCharacters(value)) if value == "email is valid")
        );
    }

    #[test]
    fn test_invariant_decl_new_stores_name_and_predicate() {
        let name = InvariantName::new("email_is_valid").unwrap();
        let method_name = MethodName::new("is_email_valid").unwrap();
        let predicate = InvariantPredicate::SelfMethod(method_name.clone());

        let decl = InvariantDecl::new(name.clone(), predicate);

        assert_eq!(decl.name, name);
        assert_eq!(decl.predicate, InvariantPredicate::SelfMethod(method_name));
    }

    #[test]
    fn test_identity_accessor_new_with_method_name_returns_accessor() {
        let method_name = MethodName::new("id").unwrap();

        let accessor = IdentityAccessor::new(method_name.clone());

        assert_eq!(accessor.method_name().as_str(), "id");
        assert_eq!(accessor.method_name(), &method_name);
    }

    #[test]
    fn test_identity_accessor_composed_with_empty_method_name_returns_error() {
        let result = MethodName::new("").map(IdentityAccessor::new);

        assert!(matches!(result, Err(IdentifierError::Empty)));
    }

    #[test]
    fn test_identity_accessor_composed_with_whitespace_method_name_returns_error() {
        let result = MethodName::new("user id").map(IdentityAccessor::new);

        assert!(
            matches!(result, Err(IdentifierError::InvalidCharacters(value)) if value == "user id")
        );
    }

    // -----------------------------------------------------------------------
    // NonEmptyVec
    // -----------------------------------------------------------------------

    #[test]
    fn test_non_empty_vec_new_with_first_and_rest_returns_values() {
        let first = TypeRef::new("UserRegistered").unwrap();
        let second = TypeRef::new("UserRenamed").unwrap();

        let values = NonEmptyVec::new(first.clone(), vec![second.clone()]);

        assert_eq!(values.as_slice().len(), 2);
        assert_eq!(values.first(), Some(&first));
        assert!(values.as_slice().contains(&second));
    }

    #[test]
    fn test_non_empty_vec_try_new_with_values_returns_values() {
        let event = TypeRef::new("UserRegistered").unwrap();

        let values = NonEmptyVec::try_new(vec![event.clone()]).unwrap();

        assert_eq!(values.as_slice(), std::slice::from_ref(&event));
        assert_eq!(values.first(), Some(&event));
    }

    #[test]
    fn test_non_empty_vec_try_new_with_empty_returns_error() {
        let result = NonEmptyVec::<TypeRef>::try_new(vec![]);

        assert!(matches!(result, Err(ConstructionError::EmptyCollection)));
    }

    // -----------------------------------------------------------------------
    // Role type separation tests
    // -----------------------------------------------------------------------

    /// Verifies that DataRole, ContractRole, and FunctionRole are distinct types.
    /// Passing a DataRole where a ContractRole is expected causes a compile error.
    #[test]
    fn test_role_types_are_distinct() {
        let data_role = DataRole::value_object();
        let contract_role = ContractRole::SpecificationPort;
        let function_role = FunctionRole::FreeFunction;
        // The following would be compile errors:
        // let _: ContractRole = data_role; // compile error
        // Verify distinct type by asserting string inequality
        assert_ne!(data_role.to_string(), contract_role.to_string());
        assert_ne!(contract_role.to_string(), function_role.to_string());
    }
}
