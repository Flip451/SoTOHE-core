//! Domain → DTO conversions for [`CatalogueDocument`] (encode path).

use domain::tddd::catalogue_v2::composite::{TypeKindV2, TypestateMarker};
use domain::tddd::catalogue_v2::entries::{
    FunctionEntry, InherentImplDeclV2, TraitEntry, TypeEntry,
};
use domain::tddd::catalogue_v2::variants::{FieldDecl, VariantDecl, VariantPayload};
use domain::tddd::catalogue_v2::{
    BoundOp, CatalogueDocument, MethodDeclaration, ParamDeclaration, TraitImplDeclV2,
    WherePredicateDecl,
};

use crate::tddd::spec_ground_codec::{informal_grounds_to_dtos, spec_refs_to_dtos};

use super::CatalogueDocumentCodecError;
use super::dto::{
    BoundOpDto, CatalogueDocumentDto, FieldDeclDto, FunctionEntryDto, InherentImplDeclDto,
    MethodDeclarationDto, MethodGenericParamDto, ParamDto, TraitEntryDto, TraitImplDto,
    TypeEntryDto, TypeKindDto, TypestateMarkerDto, VariantDeclDto, VariantPayloadDto,
    WherePredicateDeclDto,
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
    Ok(CatalogueDocumentDto {
        schema_version: doc.schema_version,
        crate_name: doc.crate_name.as_str().to_owned(),
        layer: doc.layer.as_ref().to_owned(),
        types,
        traits,
        functions,
        inherent_impls,
    })
}

// ---------------------------------------------------------------------------
// Entry converters
// ---------------------------------------------------------------------------

pub(super) fn type_entry_to_dto(
    entry: &TypeEntry,
) -> Result<TypeEntryDto, CatalogueDocumentCodecError> {
    let methods = entry.methods.iter().map(method_decl_to_dto).collect::<Result<_, _>>()?;
    let trait_impls =
        entry.trait_impls.iter().map(trait_impl_to_dto).collect::<Result<Vec<_>, _>>()?;
    Ok(TypeEntryDto {
        action: entry.action.to_string(),
        role: entry.role.to_string(),
        kind: type_kind_to_dto(&entry.kind),
        methods,
        trait_impls,
        module_path: entry.module_path.to_string(),
        docs: entry.docs.clone(),
        spec_refs: spec_refs_to_dtos(&entry.spec_refs),
        informal_grounds: informal_grounds_to_dtos(&entry.informal_grounds),
    })
}

fn type_kind_to_dto(kind: &TypeKindV2) -> TypeKindDto {
    match kind {
        TypeKindV2::UnitStruct => TypeKindDto::UnitStruct,
        TypeKindV2::TupleStruct { fields, has_stripped_fields } => TypeKindDto::TupleStruct {
            fields: fields.iter().map(|ty| ty.as_str().to_owned()).collect(),
            has_stripped_fields: *has_stripped_fields,
        },
        TypeKindV2::PlainStruct { fields, has_stripped_fields, typestate } => {
            TypeKindDto::PlainStruct {
                fields: fields.iter().map(field_decl_to_dto).collect(),
                has_stripped_fields: *has_stripped_fields,
                typestate: typestate.as_ref().map(typestate_marker_to_dto),
            }
        }
        TypeKindV2::Enum { variants } => {
            TypeKindDto::Enum { variants: variants.iter().map(variant_decl_to_dto).collect() }
        }
        TypeKindV2::TypeAlias { target } => {
            TypeKindDto::TypeAlias { target: target.as_str().to_owned() }
        }
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
        generics: m
            .generics
            .iter()
            .map(|g| MethodGenericParamDto {
                name: g.name.as_str().to_owned(),
                bounds: g.bounds.iter().map(|b| b.as_str().to_owned()).collect(),
            })
            .collect(),
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
/// when `w.rhs.len() != 1` or when `w.lhs` is not an associated-type projection
/// (i.e., does not contain `"::"`). The decoder enforces these same invariants so
/// the encoder mirrors them to prevent round-trip breakage.
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
            // Mirror the decoder's Equal-LHS invariant: the LHS must be an
            // associated-type projection (e.g. `T::Assoc`, `<T as Trait>::Assoc`).
            // A bare type parameter (`"T"`) encodes to JSON that the decoder then
            // rejects, breaking the round-trip guarantee.
            let lhs_str = w.lhs.as_str();
            if !lhs_str.contains("::") {
                return Err(CatalogueDocumentCodecError::InvalidEntry {
                    entry_name: lhs_str.to_owned(),
                    reason: format!(
                        "Equal where-predicate lhs '{}' is not a supported projection form; \
                         expected a path with at least one '::' segment (e.g. `T::Assoc`, \
                         `<T as Trait>::Assoc`) — bare type parameters cannot appear as the \
                         LHS of a `where T = U` equality constraint",
                        lhs_str
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

fn trait_impl_to_dto(t: &TraitImplDeclV2) -> Result<TraitImplDto, CatalogueDocumentCodecError> {
    let impl_where_predicates = t
        .impl_where_predicates
        .iter()
        .map(where_predicate_decl_to_dto)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(TraitImplDto {
        trait_name: t.trait_name.as_str().to_owned(),
        origin_crate: t.origin_crate.as_str().to_owned(),
        generic_args: t.generic_args().map(str::to_owned),
        impl_generics: t
            .impl_generics
            .iter()
            .map(|g| MethodGenericParamDto {
                name: g.name.as_str().to_owned(),
                bounds: g.bounds.iter().map(|b| b.as_str().to_owned()).collect(),
            })
            .collect(),
        impl_where_predicates,
    })
}

pub(super) fn trait_entry_to_dto(
    entry: &TraitEntry,
) -> Result<TraitEntryDto, CatalogueDocumentCodecError> {
    let methods = entry.methods.iter().map(method_decl_to_dto).collect::<Result<_, _>>()?;
    let where_predicates =
        entry.where_predicates.iter().map(where_predicate_decl_to_dto).collect::<Result<_, _>>()?;
    Ok(TraitEntryDto {
        action: entry.action.to_string(),
        role: entry.role.to_string(),
        methods,
        supertrait_bounds: entry.supertrait_bounds.iter().map(|b| b.as_str().to_owned()).collect(),
        generics: entry
            .generics
            .iter()
            .map(|g| MethodGenericParamDto {
                name: g.name.as_str().to_owned(),
                bounds: g.bounds.iter().map(|b| b.as_str().to_owned()).collect(),
            })
            .collect(),
        where_predicates,
        module_path: entry.module_path.to_string(),
        docs: entry.docs.clone(),
        spec_refs: spec_refs_to_dtos(&entry.spec_refs),
        informal_grounds: informal_grounds_to_dtos(&entry.informal_grounds),
    })
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
        impl_generics: decl
            .impl_generics
            .iter()
            .map(|g| MethodGenericParamDto {
                name: g.name.as_str().to_owned(),
                bounds: g.bounds.iter().map(|b| b.as_str().to_owned()).collect(),
            })
            .collect(),
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
        generics: entry
            .generics
            .iter()
            .map(|g| MethodGenericParamDto {
                name: g.name.as_str().to_owned(),
                bounds: g.bounds.iter().map(|b| b.as_str().to_owned()).collect(),
            })
            .collect(),
        where_predicates,
        docs: entry.docs.clone(),
        spec_refs: spec_refs_to_dtos(&entry.spec_refs),
        informal_grounds: informal_grounds_to_dtos(&entry.informal_grounds),
    })
}
