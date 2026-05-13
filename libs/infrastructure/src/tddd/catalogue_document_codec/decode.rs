//! DTO → domain conversions for [`CatalogueDocument`].
//!
//! All public-to-module functions convert a DTO type into the corresponding domain type.
//! Validation at this boundary ensures only valid domain values propagate downstream.

use std::collections::HashSet;
use std::str::FromStr;

use domain::tddd::LayerId;
use domain::tddd::catalogue_v2::composite::{TypeKindV2, TypestateMarker, TypestateTransitions};
use domain::tddd::catalogue_v2::entries::{FunctionEntry, TraitEntry, TypeEntry};
use domain::tddd::catalogue_v2::identifiers::{FieldName, VariantName};
use domain::tddd::catalogue_v2::roles::{ContractRole, DataRole};
use domain::tddd::catalogue_v2::variants::{FieldDecl, VariantDecl, VariantPayload};
use domain::tddd::catalogue_v2::{
    CatalogueDocument, CrateName, FunctionPath, FunctionRole, GenericArgsError, ItemAction,
    MethodDeclaration, MethodGenericParam, MethodName, ModulePath, ParamDeclaration, ParamName,
    SelfReceiver, TraitImplDeclV2, TraitName, TypeName, TypeRef,
};

use crate::tddd::spec_ground_codec::{informal_grounds_from_dtos, spec_refs_from_dtos};

use super::CatalogueDocumentCodecError;
use super::dto::{
    CatalogueDocumentDto, FieldDeclDto, FunctionEntryDto, MethodDeclarationDto, ParamDto,
    TraitEntryDto, TraitImplDto, TypeEntryDto, TypeKindDto, TypestateMarkerDto, VariantDeclDto,
    VariantPayloadDto,
};

// ---------------------------------------------------------------------------
// Top-level entry point
// ---------------------------------------------------------------------------

pub(super) fn dto_to_domain(
    dto: CatalogueDocumentDto,
) -> Result<CatalogueDocument, CatalogueDocumentCodecError> {
    let err = |name: &str, reason: String| CatalogueDocumentCodecError::InvalidEntry {
        entry_name: name.to_owned(),
        reason,
    };

    let crate_name = CrateName::new(&dto.crate_name)
        .map_err(|e| err(&dto.crate_name, format!("invalid crate_name: {e}")))?;

    let layer =
        LayerId::try_new(&dto.layer).map_err(|e| err(&dto.layer, format!("invalid layer: {e}")))?;

    let mut doc = CatalogueDocument::new(dto.schema_version, crate_name, layer);

    // Types
    for (type_name_str, entry_dto) in dto.types {
        let type_name = TypeName::new(&type_name_str)
            .map_err(|e| err(&type_name_str, format!("invalid type name: {e}")))?;
        let entry = type_entry_from_dto(&type_name_str, entry_dto)?;
        doc.types.insert(type_name, entry);
    }

    // Traits
    for (trait_name_str, entry_dto) in dto.traits {
        let trait_name = TraitName::new(&trait_name_str)
            .map_err(|e| err(&trait_name_str, format!("invalid trait name: {e}")))?;
        let entry = trait_entry_from_dto(&trait_name_str, entry_dto)?;
        doc.traits.insert(trait_name, entry);
    }

    // Functions
    // D4: all function path keys must start with `<crate_name>::`
    let expected_prefix = format!("{}::", dto.crate_name);
    for (fn_path_str, entry_dto) in dto.functions {
        if !fn_path_str.starts_with(&expected_prefix) {
            return Err(CatalogueDocumentCodecError::CrossCrateFunctionPath {
                key: fn_path_str,
                expected_crate: dto.crate_name.clone(),
            });
        }
        let fn_path = FunctionPath::from_str(&fn_path_str)
            .map_err(|e| err(&fn_path_str, format!("invalid function path: {e}")))?;
        let entry = function_entry_from_dto(&fn_path_str, entry_dto)?;
        doc.functions.insert(fn_path, entry);
    }

    Ok(doc)
}

// ---------------------------------------------------------------------------
// Entry converters
// ---------------------------------------------------------------------------

pub(super) fn type_entry_from_dto(
    name: &str,
    dto: TypeEntryDto,
) -> Result<TypeEntry, CatalogueDocumentCodecError> {
    let err = |reason: String| CatalogueDocumentCodecError::InvalidEntry {
        entry_name: name.to_owned(),
        reason,
    };

    let action = ItemAction::from_str(&dto.action)
        .map_err(|e| err(format!("invalid action '{}': {e}", dto.action)))?;

    let role = DataRole::from_str(&dto.role)
        .map_err(|e| err(format!("invalid data role '{}': {e}", dto.role)))?;

    let kind = type_kind_from_dto(name, dto.kind)?;

    let methods = dto
        .methods
        .into_iter()
        .map(|m| method_decl_from_dto(name, m))
        .collect::<Result<Vec<_>, _>>()?;

    let trait_impls = dto
        .trait_impls
        .into_iter()
        .map(|t| trait_impl_from_dto(name, t))
        .collect::<Result<Vec<_>, _>>()?;

    let module_path = if dto.module_path.is_empty() {
        ModulePath::root()
    } else {
        ModulePath::from_str(&dto.module_path)
            .map_err(|e| err(format!("invalid module_path '{}': {e}", dto.module_path)))?
    };

    let spec_refs = spec_refs_from_dtos(&dto.spec_refs).map_err(|e| {
        CatalogueDocumentCodecError::InvalidEntry {
            entry_name: name.to_owned(),
            reason: format!("{}: {}", e.field, e.reason),
        }
    })?;
    let informal_grounds = informal_grounds_from_dtos(&dto.informal_grounds).map_err(|e| {
        CatalogueDocumentCodecError::InvalidEntry {
            entry_name: name.to_owned(),
            reason: format!("{}: {}", e.field, e.reason),
        }
    })?;

    Ok(TypeEntry {
        action,
        role,
        kind,
        methods,
        trait_impls,
        module_path,
        docs: dto.docs,
        spec_refs,
        informal_grounds,
    })
}

fn type_kind_from_dto(
    name: &str,
    dto: TypeKindDto,
) -> Result<TypeKindV2, CatalogueDocumentCodecError> {
    let err = |reason: String| CatalogueDocumentCodecError::InvalidEntry {
        entry_name: name.to_owned(),
        reason,
    };

    match dto {
        TypeKindDto::UnitStruct => Ok(TypeKindV2::UnitStruct),
        TypeKindDto::TupleStruct { fields, has_stripped_fields } => {
            let fields = fields
                .into_iter()
                .map(|ty| {
                    TypeRef::new(ty.clone())
                        .map_err(|e| err(format!("invalid tuple_struct field type '{ty}': {e}")))
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(TypeKindV2::TupleStruct { fields, has_stripped_fields })
        }
        TypeKindDto::PlainStruct { fields, has_stripped_fields, typestate } => {
            let fields = fields
                .into_iter()
                .map(|f| field_decl_from_dto(name, f))
                .collect::<Result<Vec<_>, _>>()?;
            let typestate = typestate.map(|ts| typestate_marker_from_dto(name, ts)).transpose()?;
            Ok(TypeKindV2::PlainStruct { fields, has_stripped_fields, typestate })
        }
        TypeKindDto::Enum { variants } => {
            let variants = variants
                .into_iter()
                .map(|v| variant_decl_from_dto(name, v))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(TypeKindV2::Enum { variants })
        }
        TypeKindDto::TypeAlias { target } => {
            let target = TypeRef::new(target.clone())
                .map_err(|e| err(format!("invalid type_alias target '{}': {e}", target)))?;
            Ok(TypeKindV2::TypeAlias { target })
        }
    }
}

fn typestate_marker_from_dto(
    name: &str,
    dto: TypestateMarkerDto,
) -> Result<TypestateMarker, CatalogueDocumentCodecError> {
    let err = |reason: String| CatalogueDocumentCodecError::InvalidEntry {
        entry_name: name.to_owned(),
        reason,
    };

    let state_name = TypeName::new(&dto.state_name)
        .map_err(|e| err(format!("invalid typestate state_name '{}': {e}", dto.state_name)))?;
    let transition_methods = dto
        .transition_methods
        .into_iter()
        .map(|m| {
            MethodName::new(&m)
                .map_err(|e| err(format!("invalid transition method name '{}': {e}", m)))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let transitions = TypestateTransitions::new(transition_methods);
    Ok(TypestateMarker::new(state_name, transitions))
}

fn field_decl_from_dto(
    entry_name: &str,
    dto: FieldDeclDto,
) -> Result<FieldDecl, CatalogueDocumentCodecError> {
    let err = |reason: String| CatalogueDocumentCodecError::InvalidEntry {
        entry_name: entry_name.to_owned(),
        reason,
    };

    let name = FieldName::new(&dto.name)
        .map_err(|e| err(format!("invalid field name '{}': {e}", dto.name)))?;
    let ty = TypeRef::new(dto.ty.clone())
        .map_err(|e| err(format!("invalid field type '{}': {e}", dto.ty)))?;
    Ok(FieldDecl::new(name, ty))
}

fn variant_decl_from_dto(
    entry_name: &str,
    dto: VariantDeclDto,
) -> Result<VariantDecl, CatalogueDocumentCodecError> {
    let err = |reason: String| CatalogueDocumentCodecError::InvalidEntry {
        entry_name: entry_name.to_owned(),
        reason,
    };

    let name = VariantName::new(&dto.name)
        .map_err(|e| err(format!("invalid variant name '{}': {e}", dto.name)))?;
    let payload = variant_payload_from_dto(entry_name, dto.payload)?;
    Ok(VariantDecl { name, payload })
}

fn variant_payload_from_dto(
    entry_name: &str,
    dto: VariantPayloadDto,
) -> Result<VariantPayload, CatalogueDocumentCodecError> {
    let err = |reason: String| CatalogueDocumentCodecError::InvalidEntry {
        entry_name: entry_name.to_owned(),
        reason,
    };

    match dto {
        VariantPayloadDto::Unit => Ok(VariantPayload::Unit),
        VariantPayloadDto::Tuple { fields } => {
            let type_refs = fields
                .into_iter()
                .map(|f| {
                    TypeRef::new(f.clone())
                        .map_err(|e| err(format!("invalid tuple field type '{}': {e}", f)))
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(VariantPayload::Tuple(type_refs))
        }
        VariantPayloadDto::Struct { fields } => {
            let field_decls = fields
                .into_iter()
                .map(|f| field_decl_from_dto(entry_name, f))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(VariantPayload::Struct(field_decls))
        }
    }
}

/// Validates that `bound_str` is syntactically well-formed as a Rust type param bound
/// using `syn::parse_str::<syn::TypeParamBound>`.
///
/// Using `TypeParamBound` (not `syn::Type`) accepts the relaxed bound `?Sized` which
/// `syn::Type` would reject. Valid inputs include `"Send"`, `"Into<String>"`, `"?Sized"`.
///
/// Used to validate `MethodGenericParam.bounds[]` and `TraitEntry.supertrait_bounds[]`
/// at the codec boundary so that malformed bound syntax (e.g. `"<T>"`, `"T U"`) is
/// rejected here rather than failing later inside `CatalogueToExtendedCrateCodec`.
/// `TypeRef::new` only rejects empty strings and does not validate syntax; this
/// function provides the stronger structural check.
///
/// # Errors
///
/// Returns an error string with the `syn` parse error message if `bound_str` is
/// not a valid Rust type param bound syntax.
fn validate_bound_str(bound_str: &str) -> Result<(), String> {
    syn::parse_str::<syn::TypeParamBound>(bound_str)
        .map(|_| ())
        .map_err(|e| format!("invalid bound syntax '{}': {e}", bound_str))
}

pub(super) fn method_decl_from_dto(
    entry_name: &str,
    dto: MethodDeclarationDto,
) -> Result<MethodDeclaration, CatalogueDocumentCodecError> {
    let err = |reason: String| CatalogueDocumentCodecError::InvalidEntry {
        entry_name: entry_name.to_owned(),
        reason,
    };

    let name = MethodName::new(&dto.name)
        .map_err(|e| err(format!("invalid method name '{}': {e}", dto.name)))?;

    let receiver = match dto.receiver.as_deref() {
        None | Some("") => None,
        Some(r) => {
            let recv = SelfReceiver::from_str(r)
                .map_err(|e| err(format!("invalid self receiver '{}': {e}", r)))?;
            Some(recv)
        }
    };

    let params = dto
        .params
        .into_iter()
        .map(|p| param_decl_from_dto(entry_name, p))
        .collect::<Result<Vec<_>, _>>()?;

    let returns = TypeRef::new(dto.returns.clone())
        .map_err(|e| err(format!("invalid returns type '{}': {e}", dto.returns)))?;

    let generics: Vec<MethodGenericParam> = dto
        .generics
        .into_iter()
        .map(|g| {
            let name = ParamName::new(&g.name).map_err(|_| {
                if g.name.is_empty() {
                    err("generic param name must not be empty".to_owned())
                } else {
                    err(format!(
                        "generic param name '{}' is not a valid Rust identifier \
                         (must match [a-zA-Z_][a-zA-Z0-9_]*)",
                        g.name
                    ))
                }
            })?;
            let bounds = g
                .bounds
                .into_iter()
                .enumerate()
                .map(|(idx, bound)| {
                    // validate_bound_str uses syn::TypeParamBound which accepts ?Sized.
                    validate_bound_str(&bound)
                        .map_err(|e| err(format!("invalid generic param bound[{idx}]: {e}")))?;
                    TypeRef::new(bound.clone())
                        .map_err(|e| err(format!("invalid bound type ref '{bound}': {e}")))
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok::<MethodGenericParam, CatalogueDocumentCodecError>(MethodGenericParam {
                name,
                bounds,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    // Reject duplicate generic param names — duplicate names produce an impossible Rust
    // signature and must be caught at the codec boundary rather than propagated downstream.
    {
        let mut seen = HashSet::new();
        for g in &generics {
            if !seen.insert(g.name.as_str()) {
                return Err(err(format!("duplicate generic param name '{}'", g.name.as_str())));
            }
        }
    }

    let mut decl = MethodDeclaration::new(name, receiver, params, returns, dto.is_async, dto.docs);
    decl.has_default_impl = dto.has_default_impl;
    decl.generics = generics;
    Ok(decl)
}

fn param_decl_from_dto(
    entry_name: &str,
    dto: ParamDto,
) -> Result<ParamDeclaration, CatalogueDocumentCodecError> {
    let err = |reason: String| CatalogueDocumentCodecError::InvalidEntry {
        entry_name: entry_name.to_owned(),
        reason,
    };

    let name = ParamName::new(&dto.name)
        .map_err(|e| err(format!("invalid param name '{}': {e}", dto.name)))?;
    let ty = TypeRef::new(dto.ty.clone())
        .map_err(|e| err(format!("invalid param type '{}': {e}", dto.ty)))?;
    Ok(ParamDeclaration::new(name, ty))
}

fn trait_impl_from_dto(
    entry_name: &str,
    dto: TraitImplDto,
) -> Result<TraitImplDeclV2, CatalogueDocumentCodecError> {
    let err = |reason: String| CatalogueDocumentCodecError::InvalidEntry {
        entry_name: entry_name.to_owned(),
        reason,
    };

    let trait_name = TraitName::new(&dto.trait_name)
        .map_err(|e| err(format!("invalid trait name '{}': {e}", dto.trait_name)))?;
    let origin_crate = CrateName::new(&dto.origin_crate)
        .map_err(|e| err(format!("invalid origin_crate '{}': {e}", dto.origin_crate)))?;
    match dto.generic_args {
        None => Ok(TraitImplDeclV2::new(trait_name, origin_crate)),
        Some(args) => TraitImplDeclV2::new_with_generic_args(trait_name, origin_crate, args)
            .map_err(|e: GenericArgsError| err(format!("invalid generic_args: {e}"))),
    }
}

pub(super) fn trait_entry_from_dto(
    name: &str,
    dto: TraitEntryDto,
) -> Result<TraitEntry, CatalogueDocumentCodecError> {
    let err = |reason: String| CatalogueDocumentCodecError::InvalidEntry {
        entry_name: name.to_owned(),
        reason,
    };

    let action = ItemAction::from_str(&dto.action)
        .map_err(|e| err(format!("invalid action '{}': {e}", dto.action)))?;

    let role = ContractRole::from_str(&dto.role)
        .map_err(|e| err(format!("invalid contract role '{}': {e}", dto.role)))?;

    let methods = dto
        .methods
        .into_iter()
        .map(|m| method_decl_from_dto(name, m))
        .collect::<Result<Vec<_>, _>>()?;

    let module_path = if dto.module_path.is_empty() {
        ModulePath::root()
    } else {
        ModulePath::from_str(&dto.module_path)
            .map_err(|e| err(format!("invalid module_path '{}': {e}", dto.module_path)))?
    };

    // Validate and convert supertrait_bounds: each must be syntactically well-formed
    // as a Rust type param bound. `validate_bound_str` uses syn::TypeParamBound which
    // accepts `?Sized` in addition to plain trait paths.
    let supertrait_bounds = dto
        .supertrait_bounds
        .into_iter()
        .enumerate()
        .map(|(idx, bound)| {
            validate_bound_str(&bound)
                .map_err(|e| err(format!("invalid supertrait_bounds[{idx}]: {e}")))?;
            TypeRef::new(bound.clone())
                .map_err(|e| err(format!("invalid supertrait_bound type ref '{bound}': {e}")))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let spec_refs = spec_refs_from_dtos(&dto.spec_refs).map_err(|e| {
        CatalogueDocumentCodecError::InvalidEntry {
            entry_name: name.to_owned(),
            reason: format!("{}: {}", e.field, e.reason),
        }
    })?;
    let informal_grounds = informal_grounds_from_dtos(&dto.informal_grounds).map_err(|e| {
        CatalogueDocumentCodecError::InvalidEntry {
            entry_name: name.to_owned(),
            reason: format!("{}: {}", e.field, e.reason),
        }
    })?;

    Ok(TraitEntry {
        action,
        role,
        methods,
        supertrait_bounds,
        module_path,
        docs: dto.docs,
        spec_refs,
        informal_grounds,
    })
}

pub(super) fn function_entry_from_dto(
    name: &str,
    dto: FunctionEntryDto,
) -> Result<FunctionEntry, CatalogueDocumentCodecError> {
    let err = |reason: String| CatalogueDocumentCodecError::InvalidEntry {
        entry_name: name.to_owned(),
        reason,
    };

    let action = ItemAction::from_str(&dto.action)
        .map_err(|e| err(format!("invalid action '{}': {e}", dto.action)))?;

    let role = FunctionRole::from_str(&dto.role)
        .map_err(|e| err(format!("invalid function role '{}': {e}", dto.role)))?;

    let params = dto
        .params
        .into_iter()
        .map(|p| param_decl_from_dto(name, p))
        .collect::<Result<Vec<_>, _>>()?;

    let returns = TypeRef::new(dto.returns.clone())
        .map_err(|e| err(format!("invalid returns type '{}': {e}", dto.returns)))?;

    let spec_refs = spec_refs_from_dtos(&dto.spec_refs).map_err(|e| {
        CatalogueDocumentCodecError::InvalidEntry {
            entry_name: name.to_owned(),
            reason: format!("{}: {}", e.field, e.reason),
        }
    })?;
    let informal_grounds = informal_grounds_from_dtos(&dto.informal_grounds).map_err(|e| {
        CatalogueDocumentCodecError::InvalidEntry {
            entry_name: name.to_owned(),
            reason: format!("{}: {}", e.field, e.reason),
        }
    })?;

    Ok(FunctionEntry {
        action,
        role,
        params,
        returns,
        is_async: dto.is_async,
        docs: dto.docs,
        spec_refs,
        informal_grounds,
    })
}
