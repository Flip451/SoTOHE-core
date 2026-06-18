//! Domain → DTO conversions for [`CatalogueDocument`] (encode path).

use domain::tddd::catalogue_v2::composite::{StructShape, TypeKindV2, TypestateMarker};
use domain::tddd::catalogue_v2::entries::{
    AssocConstDecl, AssocTypeDecl, FunctionEntry, InherentImplDeclV2, TraitEntry, TypeEntry,
};
use domain::tddd::catalogue_v2::roles::{ContractRole, DataRole, InvariantPredicate};
use domain::tddd::catalogue_v2::variants::{FieldDecl, VariantDecl, VariantPayload};
use domain::tddd::catalogue_v2::{
    BoundOp, CatalogueDocument, InvariantDecl, MethodDeclaration, MethodGenericParam,
    ParamDeclaration, TraitImplDeclV2, WherePredicateDecl,
};

use crate::tddd::spec_ground_codec::{informal_grounds_to_dtos, spec_refs_to_dtos};

use super::CatalogueDocumentCodecError;
use super::SCHEMA_VERSION;
use super::dto::{
    AssocConstDeclDto, AssocTypeDeclDto, BoundOpDto, CatalogueDocumentDto, FieldDeclDto,
    FunctionEntryDto, InherentImplDeclDto, MethodDeclarationDto, MethodGenericParamDto, ParamDto,
    StructShapeDto, TraitEntryDto, TraitImplDto, TypeEntryDto, TypeKindDto, TypestateMarkerDto,
    VariantDeclDto, VariantPayloadDto, WherePredicateDeclDto,
};
use super::dto_roles::{
    ContractRoleDto, DataRoleDto, IdentityAccessorDto, InvariantDeclDto, InvariantPredicateDto,
};

// ---------------------------------------------------------------------------
// Top-level entry point
// ---------------------------------------------------------------------------

pub(super) fn domain_to_dto(
    doc: &CatalogueDocument,
) -> Result<CatalogueDocumentDto, CatalogueDocumentCodecError> {
    let types = doc
        .types
        .iter()
        .map(|(k, v)| type_entry_to_dto(v).map(|dto| (k.as_str().to_owned(), dto)))
        .collect::<Result<_, _>>()?;
    let traits = doc
        .traits
        .iter()
        .map(|(k, v)| trait_entry_to_dto(v).map(|dto| (k.as_str().to_owned(), dto)))
        .collect::<Result<_, _>>()?;
    let functions = doc
        .functions
        .iter()
        .map(|(k, v)| function_entry_to_dto(v).map(|dto| (k.to_string(), dto)))
        .collect::<Result<_, _>>()?;
    let inherent_impls =
        doc.inherent_impls.iter().map(inherent_impl_to_dto).collect::<Result<Vec<_>, _>>()?;
    let trait_impls =
        doc.trait_impls.iter().map(trait_impl_to_dto).collect::<Result<Vec<_>, _>>()?;
    Ok(CatalogueDocumentDto {
        schema_version: SCHEMA_VERSION,
        crate_name: doc.crate_name.as_str().to_owned(),
        layer: doc.layer.as_ref().to_owned(),
        types,
        traits,
        functions,
        inherent_impls,
        trait_impls,
    })
}

// ---------------------------------------------------------------------------
// Entry converters
// ---------------------------------------------------------------------------

pub(super) fn type_entry_to_dto(
    entry: &TypeEntry,
) -> Result<TypeEntryDto, CatalogueDocumentCodecError> {
    let methods = entry.methods.iter().map(method_decl_to_dto).collect::<Result<_, _>>()?;
    Ok(TypeEntryDto {
        action: entry.action.to_string(),
        role: data_role_to_dto(&entry.role),
        kind: type_kind_to_dto(&entry.kind),
        methods,
        module_path: entry.module_path.to_string(),
        docs: entry.docs.clone(),
        spec_refs: spec_refs_to_dtos(&entry.spec_refs),
        informal_grounds: informal_grounds_to_dtos(&entry.informal_grounds),
    })
}

fn data_role_to_dto(role: &DataRole) -> DataRoleDto {
    match role {
        DataRole::ValueObject { invariants } => {
            DataRoleDto::ValueObject { invariants: invariants_to_dtos(invariants) }
        }
        DataRole::Entity { identity, invariants } => DataRoleDto::Entity {
            identity: IdentityAccessorDto {
                method_name: identity.method_name().as_str().to_owned(),
            },
            invariants: invariants_to_dtos(invariants),
        },
        DataRole::AggregateRoot {
            identity,
            invariants,
            exclusive_members,
            shared_value_objects,
            emits,
        } => DataRoleDto::AggregateRoot {
            identity: IdentityAccessorDto {
                method_name: identity.method_name().as_str().to_owned(),
            },
            invariants: invariants_to_dtos(invariants),
            exclusive_members: exclusive_members.iter().map(|r| r.as_str().to_owned()).collect(),
            shared_value_objects: shared_value_objects
                .iter()
                .map(|r| r.as_str().to_owned())
                .collect(),
            emits: emits.iter().map(|r| r.as_str().to_owned()).collect(),
        },
        DataRole::DomainService { emits } => DataRoleDto::DomainService {
            emits: emits.iter().map(|r| r.as_str().to_owned()).collect(),
        },
        DataRole::Specification => DataRoleDto::Specification {},
        DataRole::Factory => DataRoleDto::Factory {},
        DataRole::UseCase { handles } => DataRoleDto::UseCase {
            handles: handles.iter().map(|r| r.as_str().to_owned()).collect(),
        },
        DataRole::Interactor => DataRoleDto::Interactor {},
        DataRole::Command => DataRoleDto::Command {},
        DataRole::Query => DataRoleDto::Query {},
        DataRole::Dto => DataRoleDto::Dto {},
        DataRole::ErrorType => DataRoleDto::ErrorType {},
        DataRole::SecondaryAdapter => DataRoleDto::SecondaryAdapter {},
        DataRole::EventPolicy { reacts_to } => DataRoleDto::EventPolicy {
            reacts_to: reacts_to.as_slice().iter().map(|r| r.as_str().to_owned()).collect(),
        },
        DataRole::DomainEvent => DataRoleDto::DomainEvent {},
    }
}

fn invariants_to_dtos(invariants: &[InvariantDecl]) -> Vec<InvariantDeclDto> {
    invariants
        .iter()
        .map(|decl| InvariantDeclDto {
            name: decl.name.as_str().to_owned(),
            predicate: match &decl.predicate {
                InvariantPredicate::SelfMethod(method) => {
                    InvariantPredicateDto::SelfMethod(method.as_str().to_owned())
                }
            },
        })
        .collect()
}

fn type_kind_to_dto(kind: &TypeKindV2) -> TypeKindDto {
    match kind {
        TypeKindV2::Struct(struct_kind) => TypeKindDto::Struct {
            shape: struct_shape_to_dto(&struct_kind.shape),
            typestate: struct_kind.typestate.as_ref().map(typestate_marker_to_dto),
        },
        TypeKindV2::Enum { variants } => {
            TypeKindDto::Enum { variants: variants.iter().map(variant_decl_to_dto).collect() }
        }
        TypeKindV2::TypeAlias { target } => {
            TypeKindDto::TypeAlias { target: target.as_str().to_owned() }
        }
    }
}

fn struct_shape_to_dto(shape: &StructShape) -> StructShapeDto {
    match shape {
        StructShape::Unit => StructShapeDto::Unit,
        StructShape::Tuple { fields, has_stripped_fields } => StructShapeDto::Tuple {
            fields: fields.iter().map(|ty| ty.as_str().to_owned()).collect(),
            has_stripped_fields: *has_stripped_fields,
        },
        StructShape::Plain { fields, has_stripped_fields } => StructShapeDto::Plain {
            fields: fields.iter().map(field_decl_to_dto).collect(),
            has_stripped_fields: *has_stripped_fields,
        },
    }
}

fn typestate_marker_to_dto(marker: &TypestateMarker) -> TypestateMarkerDto {
    TypestateMarkerDto {
        state_name: marker.state_name().as_str().to_owned(),
        transition_methods: marker
            .transitions()
            .transition_methods()
            .iter()
            .map(|m| m.as_str().to_owned())
            .collect(),
    }
}

fn field_decl_to_dto(f: &FieldDecl) -> FieldDeclDto {
    FieldDeclDto { name: f.name.as_str().to_owned(), ty: f.ty.as_str().to_owned() }
}

fn variant_decl_to_dto(v: &VariantDecl) -> VariantDeclDto {
    VariantDeclDto { name: v.name.as_str().to_owned(), payload: variant_payload_to_dto(&v.payload) }
}

fn variant_payload_to_dto(payload: &VariantPayload) -> VariantPayloadDto {
    match payload {
        VariantPayload::Unit => VariantPayloadDto::Unit,
        VariantPayload::Tuple(fields) => VariantPayloadDto::Tuple {
            fields: fields.iter().map(|f| f.as_str().to_owned()).collect(),
        },
        VariantPayload::Struct(fields) => {
            VariantPayloadDto::Struct { fields: fields.iter().map(field_decl_to_dto).collect() }
        }
    }
}

pub(super) fn method_decl_to_dto(
    m: &MethodDeclaration,
) -> Result<MethodDeclarationDto, CatalogueDocumentCodecError> {
    let where_predicates =
        m.where_predicates.iter().map(where_predicate_decl_to_dto).collect::<Result<_, _>>()?;
    Ok(MethodDeclarationDto {
        name: m.name.as_str().to_owned(),
        receiver: m.receiver.map(|r| r.to_string()),
        params: m.params.iter().map(param_decl_to_dto).collect(),
        returns: m.returns.as_str().to_owned(),
        is_async: m.is_async,
        has_default_impl: m.has_default_impl,
        generics: method_generic_params_to_dtos(&m.generics),
        where_predicates,
        docs: m.docs.clone(),
    })
}

/// Encodes a single [`WherePredicateDecl`] to its DTO form.
///
/// # Errors
///
/// Returns `CatalogueDocumentCodecError::InvalidEntry` when `w.rhs` is
/// empty. An empty rhs list cannot round-trip through the codec because the
/// decoder rejects predicates with no RHS (a bare `where T:` or `where T =`
/// without a right-hand side is invalid and is rejected by
/// `where_predicates_from_dtos`).
///
/// Returns `CatalogueDocumentCodecError::InvalidEntry` for `Equal` predicates
/// when `w.rhs.len() != 1`.
fn where_predicate_decl_to_dto(
    w: &WherePredicateDecl,
) -> Result<WherePredicateDeclDto, CatalogueDocumentCodecError> {
    if w.rhs.is_empty() {
        return Err(CatalogueDocumentCodecError::InvalidEntry {
            entry_name: w.lhs.as_str().to_owned(),
            reason: "where predicate has no rhs — empty rhs cannot round-trip through \
                     the catalogue JSON codec (decoder rejects bare `where T:` predicates)"
                .to_owned(),
        });
    }
    let operator = match w.operator {
        BoundOp::Bound => BoundOpDto::Bound,
        BoundOp::Equal => {
            // `Equal` predicates (`where T::Assoc = U`) must have exactly one RHS.
            // Multiple RHS entries are syntactically invalid and would silently drop
            // entries after the first in the extended-crate codec.
            if w.rhs.len() != 1 {
                return Err(CatalogueDocumentCodecError::InvalidEntry {
                    entry_name: w.lhs.as_str().to_owned(),
                    reason: format!(
                        "where predicate with operator Equal must have exactly one rhs entry \
                         (got {}); `where T::Assoc = U` accepts a single RHS only",
                        w.rhs.len()
                    ),
                });
            }
            BoundOpDto::Equal
        }
    };
    Ok(WherePredicateDeclDto {
        lhs: w.lhs.as_str().to_owned(),
        rhs: w.rhs.iter().map(|b| b.as_str().to_owned()).collect(),
        operator,
    })
}

fn param_decl_to_dto(p: &ParamDeclaration) -> ParamDto {
    ParamDto { name: p.name.as_str().to_owned(), ty: p.ty.as_str().to_owned() }
}

fn method_generic_param_to_dto(g: &MethodGenericParam) -> MethodGenericParamDto {
    MethodGenericParamDto {
        name: g.name.as_str().to_owned(),
        bounds: g.bounds.iter().map(|b| b.as_str().to_owned()).collect(),
    }
}

fn method_generic_params_to_dtos(generics: &[MethodGenericParam]) -> Vec<MethodGenericParamDto> {
    generics.iter().map(method_generic_param_to_dto).collect()
}

/// Encodes a top-level `TraitImplDeclV2` to its DTO form (ADR `2026-05-20-0048` D2).
///
/// Emits `action`, `trait_ref`, and `for_type` fields. Validates `trait_ref` as a
/// Rust path expression (not a reference, slice, or tuple) and validates both
/// `trait_ref` and `for_type` as syn-parseable type expressions to guarantee
/// encode/decode round-trip consistency for in-memory `TraitImplDeclV2` values
/// that were not originally constructed via the JSON decoder.
fn trait_impl_to_dto(t: &TraitImplDeclV2) -> Result<TraitImplDto, CatalogueDocumentCodecError> {
    let err = |reason: String| CatalogueDocumentCodecError::InvalidEntry {
        entry_name: t.trait_ref.as_str().to_owned(),
        reason,
    };
    super::decode::validate_type_ref_str(t.trait_ref.as_str())
        .map_err(|e| err(format!("invalid trait_ref syntax: {e}")))?;
    // Mirror the decode-path path-type constraint: trait_ref must be a path, not a reference etc.
    super::decode::validate_trait_ref_is_path(t.trait_ref.as_str())
        .map_err(|e| err(format!("invalid trait_ref (must be a path): {e}")))?;
    super::decode::validate_type_ref_str(t.for_type.as_str())
        .map_err(|e| err(format!("invalid for_type syntax: {e}")))?;
    let impl_where_predicates = t
        .impl_where_predicates
        .iter()
        .map(where_predicate_decl_to_dto)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(TraitImplDto {
        action: t.action.to_string(),
        trait_ref: t.trait_ref.as_str().to_owned(),
        for_type: t.for_type.as_str().to_owned(),
        impl_generics: method_generic_params_to_dtos(&t.impl_generics),
        impl_where_predicates,
    })
}

pub(super) fn trait_entry_to_dto(
    entry: &TraitEntry,
) -> Result<TraitEntryDto, CatalogueDocumentCodecError> {
    let methods = entry.methods.iter().map(method_decl_to_dto).collect::<Result<_, _>>()?;
    let where_predicates =
        entry.where_predicates.iter().map(where_predicate_decl_to_dto).collect::<Result<_, _>>()?;
    let assoc_types = entry.assoc_types.iter().map(assoc_type_decl_to_dto).collect();
    let assoc_consts = entry.assoc_consts.iter().map(assoc_const_decl_to_dto).collect();
    Ok(TraitEntryDto {
        action: entry.action.to_string(),
        role: contract_role_to_dto(&entry.role),
        methods,
        assoc_types,
        assoc_consts,
        supertrait_bounds: entry.supertrait_bounds.iter().map(|b| b.as_str().to_owned()).collect(),
        generics: method_generic_params_to_dtos(&entry.generics),
        where_predicates,
        module_path: entry.module_path.to_string(),
        docs: entry.docs.clone(),
        spec_refs: spec_refs_to_dtos(&entry.spec_refs),
        informal_grounds: informal_grounds_to_dtos(&entry.informal_grounds),
    })
}

fn assoc_type_decl_to_dto(decl: &AssocTypeDecl) -> AssocTypeDeclDto {
    AssocTypeDeclDto {
        name: decl.name.as_str().to_owned(),
        bounds: decl.bounds.iter().map(|b| b.as_str().to_owned()).collect(),
        default: decl.default.as_ref().map(|d| d.as_str().to_owned()),
    }
}

fn assoc_const_decl_to_dto(decl: &AssocConstDecl) -> AssocConstDeclDto {
    AssocConstDeclDto {
        name: decl.name.as_str().to_owned(),
        ty: decl.ty.as_str().to_owned(),
        default_value: decl.default_value.clone(),
    }
}

fn contract_role_to_dto(role: &ContractRole) -> ContractRoleDto {
    match role {
        ContractRole::SpecificationPort => ContractRoleDto::SpecificationPort {},
        ContractRole::ApplicationService => ContractRoleDto::ApplicationService {},
        ContractRole::SecondaryPort => ContractRoleDto::SecondaryPort {},
        ContractRole::Repository { aggregate } => {
            ContractRoleDto::Repository { aggregate: aggregate.as_str().to_owned() }
        }
    }
}

pub(super) fn inherent_impl_to_dto(
    decl: &InherentImplDeclV2,
) -> Result<InherentImplDeclDto, CatalogueDocumentCodecError> {
    let impl_where_predicates = decl
        .impl_where_predicates
        .iter()
        .map(where_predicate_decl_to_dto)
        .collect::<Result<Vec<_>, _>>()?;
    let methods = decl.methods.iter().map(method_decl_to_dto).collect::<Result<Vec<_>, _>>()?;
    Ok(InherentImplDeclDto {
        type_name: decl.type_name.as_str().to_owned(),
        impl_generics: method_generic_params_to_dtos(&decl.impl_generics),
        impl_where_predicates,
        methods,
    })
}

pub(super) fn function_entry_to_dto(
    entry: &FunctionEntry,
) -> Result<FunctionEntryDto, CatalogueDocumentCodecError> {
    let where_predicates =
        entry.where_predicates.iter().map(where_predicate_decl_to_dto).collect::<Result<_, _>>()?;
    Ok(FunctionEntryDto {
        action: entry.action.to_string(),
        role: entry.role.to_string(),
        params: entry.params.iter().map(param_decl_to_dto).collect(),
        returns: entry.returns.as_str().to_owned(),
        is_async: entry.is_async,
        generics: method_generic_params_to_dtos(&entry.generics),
        where_predicates,
        docs: entry.docs.clone(),
        spec_refs: spec_refs_to_dtos(&entry.spec_refs),
        informal_grounds: informal_grounds_to_dtos(&entry.informal_grounds),
    })
}
