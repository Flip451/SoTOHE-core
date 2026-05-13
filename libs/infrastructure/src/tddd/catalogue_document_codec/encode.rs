//! Domain → DTO conversions for [`CatalogueDocument`] (encode path).

use domain::tddd::catalogue_v2::composite::{TypeKindV2, TypestateMarker};
use domain::tddd::catalogue_v2::entries::{FunctionEntry, TraitEntry, TypeEntry};
use domain::tddd::catalogue_v2::variants::{FieldDecl, VariantDecl, VariantPayload};
use domain::tddd::catalogue_v2::{
    CatalogueDocument, MethodDeclaration, ParamDeclaration, TraitImplDeclV2,
};

use crate::tddd::spec_ground_codec::{informal_grounds_to_dtos, spec_refs_to_dtos};

use super::dto::{
    CatalogueDocumentDto, FieldDeclDto, FunctionEntryDto, MethodDeclarationDto,
    MethodGenericParamDto, ParamDto, TraitEntryDto, TraitImplDto, TypeEntryDto, TypeKindDto,
    TypestateMarkerDto, VariantDeclDto, VariantPayloadDto,
};

// ---------------------------------------------------------------------------
// Top-level entry point
// ---------------------------------------------------------------------------

pub(super) fn domain_to_dto(doc: &CatalogueDocument) -> CatalogueDocumentDto {
    CatalogueDocumentDto {
        schema_version: doc.schema_version,
        crate_name: doc.crate_name.as_str().to_owned(),
        layer: doc.layer.as_ref().to_owned(),
        types: doc
            .types
            .iter()
            .map(|(k, v)| (k.as_str().to_owned(), type_entry_to_dto(v)))
            .collect(),
        traits: doc
            .traits
            .iter()
            .map(|(k, v)| (k.as_str().to_owned(), trait_entry_to_dto(v)))
            .collect(),
        functions: doc
            .functions
            .iter()
            .map(|(k, v)| (k.to_string(), function_entry_to_dto(v)))
            .collect(),
    }
}

// ---------------------------------------------------------------------------
// Entry converters
// ---------------------------------------------------------------------------

pub(super) fn type_entry_to_dto(entry: &TypeEntry) -> TypeEntryDto {
    TypeEntryDto {
        action: entry.action.to_string(),
        role: entry.role.to_string(),
        kind: type_kind_to_dto(&entry.kind),
        methods: entry.methods.iter().map(method_decl_to_dto).collect(),
        trait_impls: entry.trait_impls.iter().map(trait_impl_to_dto).collect(),
        module_path: entry.module_path.to_string(),
        docs: entry.docs.clone(),
        spec_refs: spec_refs_to_dtos(&entry.spec_refs),
        informal_grounds: informal_grounds_to_dtos(&entry.informal_grounds),
    }
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

pub(super) fn method_decl_to_dto(m: &MethodDeclaration) -> MethodDeclarationDto {
    MethodDeclarationDto {
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
        docs: m.docs.clone(),
    }
}

fn param_decl_to_dto(p: &ParamDeclaration) -> ParamDto {
    ParamDto { name: p.name.as_str().to_owned(), ty: p.ty.as_str().to_owned() }
}

fn trait_impl_to_dto(t: &TraitImplDeclV2) -> TraitImplDto {
    TraitImplDto {
        trait_name: t.trait_name.as_str().to_owned(),
        origin_crate: t.origin_crate.as_str().to_owned(),
        generic_args: t.generic_args().map(str::to_owned),
    }
}

pub(super) fn trait_entry_to_dto(entry: &TraitEntry) -> TraitEntryDto {
    TraitEntryDto {
        action: entry.action.to_string(),
        role: entry.role.to_string(),
        methods: entry.methods.iter().map(method_decl_to_dto).collect(),
        supertrait_bounds: entry.supertrait_bounds.iter().map(|b| b.as_str().to_owned()).collect(),
        module_path: entry.module_path.to_string(),
        docs: entry.docs.clone(),
        spec_refs: spec_refs_to_dtos(&entry.spec_refs),
        informal_grounds: informal_grounds_to_dtos(&entry.informal_grounds),
    }
}

pub(super) fn function_entry_to_dto(entry: &FunctionEntry) -> FunctionEntryDto {
    FunctionEntryDto {
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
        docs: entry.docs.clone(),
        spec_refs: spec_refs_to_dtos(&entry.spec_refs),
        informal_grounds: informal_grounds_to_dtos(&entry.informal_grounds),
    }
}
