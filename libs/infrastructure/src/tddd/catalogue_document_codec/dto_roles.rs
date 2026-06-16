//! Role DTOs used by the catalogue JSON codec.

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) enum DataRoleDto {
    ValueObject {
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        invariants: Vec<InvariantDeclDto>,
    },
    Entity {
        identity: IdentityAccessorDto,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        invariants: Vec<InvariantDeclDto>,
    },
    AggregateRoot {
        identity: IdentityAccessorDto,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        invariants: Vec<InvariantDeclDto>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        exclusive_members: Vec<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        shared_value_objects: Vec<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        emits: Vec<String>,
    },
    DomainService {
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        emits: Vec<String>,
    },
    Specification {},
    Factory {},
    UseCase {
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        handles: Vec<String>,
    },
    Interactor {},
    Command {},
    Query {},
    Dto {},
    ErrorType {},
    SecondaryAdapter {},
    EventPolicy {
        reacts_to: Vec<String>,
    },
    DomainEvent {},
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) enum ContractRoleDto {
    SpecificationPort {},
    ApplicationService {},
    SecondaryPort {},
    Repository { aggregate: String },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct IdentityAccessorDto {
    pub(super) method_name: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct InvariantDeclDto {
    pub(super) name: String,
    pub(super) predicate: InvariantPredicateDto,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) enum InvariantPredicateDto {
    SelfMethod(String),
}
