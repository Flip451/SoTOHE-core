//! Role DTO decoders for [`CatalogueDocument`] entries.

use domain::tddd::catalogue_v2::roles::{ContractRole, DataRole};
use domain::tddd::catalogue_v2::{
    IdentityAccessor, InvariantDecl, InvariantName, InvariantPredicate, MethodName, NonEmptyVec,
    TypeRef,
};

use super::CatalogueDocumentCodecError;
use super::dto_roles::{
    ContractRoleDto, DataRoleDto, IdentityAccessorDto, InvariantDeclDto, InvariantPredicateDto,
};

pub(super) fn data_role_from_dto(
    name: &str,
    dto: DataRoleDto,
) -> Result<DataRole, CatalogueDocumentCodecError> {
    match dto {
        DataRoleDto::ValueObject { invariants } => {
            Ok(DataRole::ValueObject { invariants: invariants_from_dtos(name, invariants)? })
        }
        DataRoleDto::Entity { identity, invariants } => Ok(DataRole::Entity {
            identity: identity_accessor_from_dto(name, identity)?,
            invariants: invariants_from_dtos(name, invariants)?,
        }),
        DataRoleDto::AggregateRoot {
            identity,
            invariants,
            exclusive_members,
            shared_value_objects,
            emits,
        } => Ok(DataRole::AggregateRoot {
            identity: identity_accessor_from_dto(name, identity)?,
            invariants: invariants_from_dtos(name, invariants)?,
            exclusive_members: type_refs_from_strings(
                name,
                "exclusive_members",
                exclusive_members,
            )?,
            shared_value_objects: type_refs_from_strings(
                name,
                "shared_value_objects",
                shared_value_objects,
            )?,
            emits: type_refs_from_strings(name, "emits", emits)?,
        }),
        DataRoleDto::DomainService { emits } => {
            Ok(DataRole::DomainService { emits: type_refs_from_strings(name, "emits", emits)? })
        }
        DataRoleDto::Specification {} => Ok(DataRole::Specification),
        DataRoleDto::Factory {} => Ok(DataRole::Factory),
        DataRoleDto::UseCase { handles } => {
            Ok(DataRole::UseCase { handles: type_refs_from_strings(name, "handles", handles)? })
        }
        DataRoleDto::Interactor {} => Ok(DataRole::Interactor),
        DataRoleDto::Command {} => Ok(DataRole::Command),
        DataRoleDto::Query {} => Ok(DataRole::Query),
        DataRoleDto::Dto {} => Ok(DataRole::Dto),
        DataRoleDto::ErrorType {} => Ok(DataRole::ErrorType),
        DataRoleDto::SecondaryAdapter {} => Ok(DataRole::SecondaryAdapter),
        DataRoleDto::EventPolicy { reacts_to } => {
            let refs = type_refs_from_strings(name, "reacts_to", reacts_to)?;
            let reacts_to = NonEmptyVec::try_new(refs).map_err(|e| {
                CatalogueDocumentCodecError::InvalidEntry {
                    entry_name: name.to_owned(),
                    reason: format!("invalid EventPolicy.reacts_to: {e}"),
                }
            })?;
            Ok(DataRole::EventPolicy { reacts_to })
        }
    }
}

pub(super) fn contract_role_from_dto(
    name: &str,
    dto: ContractRoleDto,
) -> Result<ContractRole, CatalogueDocumentCodecError> {
    match dto {
        ContractRoleDto::SpecificationPort {} => Ok(ContractRole::SpecificationPort),
        ContractRoleDto::ApplicationService {} => Ok(ContractRole::ApplicationService),
        ContractRoleDto::SecondaryPort {} => Ok(ContractRole::SecondaryPort),
        ContractRoleDto::Repository { aggregate } => {
            let aggregate = TypeRef::new(aggregate.clone()).map_err(|e| {
                CatalogueDocumentCodecError::InvalidEntry {
                    entry_name: name.to_owned(),
                    reason: format!("invalid Repository.aggregate '{aggregate}': {e}"),
                }
            })?;
            Ok(ContractRole::Repository { aggregate })
        }
    }
}

fn identity_accessor_from_dto(
    name: &str,
    dto: IdentityAccessorDto,
) -> Result<IdentityAccessor, CatalogueDocumentCodecError> {
    let method_name = MethodName::new(&dto.method_name).map_err(|e| {
        CatalogueDocumentCodecError::InvalidEntry {
            entry_name: name.to_owned(),
            reason: format!("invalid identity.method_name '{}': {e}", dto.method_name),
        }
    })?;
    Ok(IdentityAccessor::new(method_name))
}

fn invariants_from_dtos(
    name: &str,
    dtos: Vec<InvariantDeclDto>,
) -> Result<Vec<InvariantDecl>, CatalogueDocumentCodecError> {
    dtos.into_iter().map(|dto| invariant_from_dto(name, dto)).collect()
}

fn invariant_from_dto(
    name: &str,
    dto: InvariantDeclDto,
) -> Result<InvariantDecl, CatalogueDocumentCodecError> {
    let invariant_name =
        InvariantName::new(&dto.name).map_err(|e| CatalogueDocumentCodecError::InvalidEntry {
            entry_name: name.to_owned(),
            reason: format!("invalid invariant name '{}': {e}", dto.name),
        })?;
    let predicate = match dto.predicate {
        InvariantPredicateDto::SelfMethod(method) => {
            let method = MethodName::new(&method).map_err(|e| {
                CatalogueDocumentCodecError::InvalidEntry {
                    entry_name: name.to_owned(),
                    reason: format!("invalid invariant predicate method '{method}': {e}"),
                }
            })?;
            InvariantPredicate::SelfMethod(method)
        }
    };
    Ok(InvariantDecl::new(invariant_name, predicate))
}

fn type_refs_from_strings(
    name: &str,
    field: &str,
    values: Vec<String>,
) -> Result<Vec<TypeRef>, CatalogueDocumentCodecError> {
    values
        .into_iter()
        .enumerate()
        .map(|(idx, value)| {
            TypeRef::new(value.clone()).map_err(|e| CatalogueDocumentCodecError::InvalidEntry {
                entry_name: name.to_owned(),
                reason: format!("invalid {field}[{idx}] '{value}': {e}"),
            })
        })
        .collect()
}
