//! DTO → domain conversions for [`CatalogueDocument`].

use std::collections::HashSet;
use std::str::FromStr;

use domain::tddd::LayerId;
use domain::tddd::catalogue_v2::composite::{
    StructKind, StructShape, TypeKindV2, TypestateMarker, TypestateTransitions,
};
use domain::tddd::catalogue_v2::entries::{TraitEntry, TypeEntry};
use domain::tddd::catalogue_v2::identifiers::{DocString, FieldName, VariantName};
use domain::tddd::catalogue_v2::variants::{FieldDecl, VariantDecl, VariantPayload};
use domain::tddd::catalogue_v2::{
    BoundOp, CatalogueDocument, CrateName, DeletionRecord, FunctionPath, ItemAction,
    MethodDeclaration, MethodGenericParam, MethodName, ModulePath, ParamDeclaration, ParamName,
    SelfReceiver, TraitImplDeclV2, TraitName, TypeName, TypeRef, WherePredicateDecl,
};

use crate::tddd::spec_ground_codec::{informal_grounds_from_dtos, spec_refs_from_dtos};

use super::CatalogueDocumentCodecError;
use super::decode_assoc::{assoc_const_decl_from_dto, assoc_type_decl_from_dto};
use super::decode_impls::{
    function_entry_from_dto, inherent_impl_from_dto, validate_trait_item_names,
};
use super::decode_roles::{contract_role_from_dto, data_role_from_dto};
use super::dto::{
    BoundOpDto, CatalogueDocumentDto, FieldDeclDto, MethodDeclarationDto, MethodGenericParamDto,
    ParamDto, StructShapeDto, TraitEntryDto, TraitImplDto, TypeEntryDto, TypeKindDto,
    TypestateMarkerDto, VariantDeclDto, VariantPayloadDto, WherePredicateDeclDto,
};
use super::dto_slots::{EntrySlotDto, TombstoneDto};
use super::validate::{validate_bound_str, validate_type_ref_str};

// Keep the path-only validator referenced for callers outside top-level impl slots.
const _: fn(&str) -> Result<(), String> = super::validate::validate_trait_ref_is_path;

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

    // Delete slots become deletion records rather than live entries.
    for (type_name_str, slot) in dto.types {
        let type_name = TypeName::new(&type_name_str)
            .map_err(|e| err(&type_name_str, format!("invalid type name: {e}")))?;
        match slot {
            EntrySlotDto::Tombstone(tombstone) => {
                let module_path = tombstone_module_path(&type_name_str, &tombstone)?;
                let (spec_refs, informal_grounds) =
                    tombstone_grounding(&type_name_str, &tombstone)?;
                doc.push_deletion(DeletionRecord::Type {
                    name: type_name,
                    module_path,
                    spec_refs,
                    informal_grounds,
                });
            }
            EntrySlotDto::Live(entry_dto) => {
                let entry = type_entry_from_dto(&type_name_str, entry_dto)?;
                doc.insert_type(type_name, entry);
            }
        }
    }

    // Traits
    for (trait_name_str, slot) in dto.traits {
        let trait_name = TraitName::new(&trait_name_str)
            .map_err(|e| err(&trait_name_str, format!("invalid trait name: {e}")))?;
        match slot {
            EntrySlotDto::Tombstone(tombstone) => {
                let module_path = tombstone_module_path(&trait_name_str, &tombstone)?;
                let (spec_refs, informal_grounds) =
                    tombstone_grounding(&trait_name_str, &tombstone)?;
                doc.push_deletion(DeletionRecord::Trait {
                    name: trait_name,
                    module_path,
                    spec_refs,
                    informal_grounds,
                });
            }
            EntrySlotDto::Live(entry_dto) => {
                let entry = trait_entry_from_dto(&trait_name_str, entry_dto)?;
                doc.insert_trait(trait_name, entry);
            }
        }
    }

    // Functions
    // D4: all function path keys must start with `<crate_name>::`
    let expected_prefix = format!("{}::", dto.crate_name);
    for (fn_path_str, slot) in dto.functions {
        if !fn_path_str.starts_with(&expected_prefix) {
            return Err(CatalogueDocumentCodecError::CrossCrateFunctionPath {
                key: fn_path_str,
                expected_crate: dto.crate_name.clone(),
            });
        }
        let fn_path = FunctionPath::from_str(&fn_path_str)
            .map_err(|e| err(&fn_path_str, format!("invalid function path: {e}")))?;
        match slot {
            // A function's module is embedded in its path key, so the tombstone
            // body carries no module_path of its own. Reject non-empty values
            // instead of silently dropping them on re-encode.
            EntrySlotDto::Tombstone(tombstone) => {
                if !tombstone.module_path.is_empty() {
                    return Err(CatalogueDocumentCodecError::InvalidEntry {
                        entry_name: fn_path_str,
                        reason: "function delete tombstone must not carry module_path; \
                                 the function path map key is the full identity"
                            .to_owned(),
                    });
                }
                let (spec_refs, informal_grounds) = tombstone_grounding(&fn_path_str, &tombstone)?;
                doc.push_deletion(DeletionRecord::Function {
                    path: fn_path,
                    spec_refs,
                    informal_grounds,
                });
            }
            EntrySlotDto::Live(entry_dto) => {
                let entry = function_entry_from_dto(&fn_path_str, entry_dto)?;
                doc.insert_function(fn_path, entry);
            }
        }
    }

    // InherentImpls
    for impl_dto in dto.inherent_impls {
        let impl_decl = inherent_impl_from_dto(impl_dto)?;
        doc.push_inherent_impl(impl_decl);
    }

    for ti_dto in dto.trait_impls {
        let ti = trait_impl_from_dto(ti_dto)?;
        doc.push_trait_impl(ti);
    }

    Ok(doc)
}

fn tombstone_module_path(
    entry_name: &str,
    tombstone: &TombstoneDto,
) -> Result<ModulePath, CatalogueDocumentCodecError> {
    if tombstone.module_path.is_empty() {
        Ok(ModulePath::root())
    } else {
        ModulePath::from_str(&tombstone.module_path).map_err(|e| {
            CatalogueDocumentCodecError::InvalidEntry {
                entry_name: entry_name.to_owned(),
                reason: format!("invalid delete module_path '{}': {e}", tombstone.module_path),
            }
        })
    }
}

fn tombstone_grounding(
    entry_name: &str,
    tombstone: &TombstoneDto,
) -> Result<(Vec<domain::SpecRef>, Vec<domain::InformalGroundRef>), CatalogueDocumentCodecError> {
    let spec_refs = spec_refs_from_dtos(&tombstone.spec_refs).map_err(|e| {
        CatalogueDocumentCodecError::InvalidEntry {
            entry_name: entry_name.to_owned(),
            reason: format!("{}: {}", e.field, e.reason),
        }
    })?;
    let informal_grounds =
        informal_grounds_from_dtos(&tombstone.informal_grounds).map_err(|e| {
            CatalogueDocumentCodecError::InvalidEntry {
                entry_name: entry_name.to_owned(),
                reason: format!("{}: {}", e.field, e.reason),
            }
        })?;
    Ok((spec_refs, informal_grounds))
}

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

    let role = data_role_from_dto(name, dto.role)?;

    let kind = type_kind_from_dto(name, dto.kind)?;

    let methods = dto
        .methods
        .into_iter()
        .map(|m| method_decl_from_dto(name, m))
        .collect::<Result<Vec<_>, _>>()?;

    let generics = method_generics_from_dtos(name, dto.generics)?;
    let where_predicates = where_predicates_from_dtos(name, dto.where_predicates)?;

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

    Ok(TypeEntry::new(
        action,
        role,
        kind,
        methods,
        generics,
        where_predicates,
        module_path,
        dto.docs.map(DocString::new),
        spec_refs,
        informal_grounds,
    ))
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
        TypeKindDto::Struct { shape, typestate } => {
            let shape = struct_shape_from_dto(name, shape)?;
            let typestate = typestate.map(|ts| typestate_marker_from_dto(name, ts)).transpose()?;
            Ok(TypeKindV2::Struct(StructKind::new(shape, typestate)))
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

fn struct_shape_from_dto(
    name: &str,
    dto: StructShapeDto,
) -> Result<StructShape, CatalogueDocumentCodecError> {
    let err = |reason: String| CatalogueDocumentCodecError::InvalidEntry {
        entry_name: name.to_owned(),
        reason,
    };

    match dto {
        StructShapeDto::Unit => Ok(StructShape::Unit),
        StructShapeDto::Tuple { fields, has_stripped_fields } => {
            let fields = fields
                .into_iter()
                .map(|ty| {
                    TypeRef::new(ty.clone())
                        .map_err(|e| err(format!("invalid tuple field type '{ty}': {e}")))
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(StructShape::Tuple { fields, has_stripped_fields })
        }
        StructShapeDto::Plain { fields, has_stripped_fields } => {
            let fields = fields
                .into_iter()
                .map(|f| field_decl_from_dto(name, f))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(StructShape::Plain { fields, has_stripped_fields })
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

pub(super) fn method_generics_from_dtos(
    entry_name: &str,
    dtos: Vec<MethodGenericParamDto>,
) -> Result<Vec<MethodGenericParam>, CatalogueDocumentCodecError> {
    let err = |reason: String| CatalogueDocumentCodecError::InvalidEntry {
        entry_name: entry_name.to_owned(),
        reason,
    };
    let generics: Vec<MethodGenericParam> = dtos
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

    let mut seen = HashSet::new();
    for g in &generics {
        if !seen.insert(g.name.as_str()) {
            return Err(err(format!("duplicate generic param name '{}'", g.name.as_str())));
        }
    }
    Ok(generics)
}

pub(super) fn where_predicates_from_dtos(
    entry_name: &str,
    dtos: Vec<WherePredicateDeclDto>,
) -> Result<Vec<WherePredicateDecl>, CatalogueDocumentCodecError> {
    let err = |reason: String| CatalogueDocumentCodecError::InvalidEntry {
        entry_name: entry_name.to_owned(),
        reason,
    };
    dtos.into_iter()
        .map(|w| {
            let lhs = TypeRef::new(w.lhs.clone())
                .map_err(|e| err(format!("invalid where predicate lhs '{}': {e}", w.lhs)))?;
            validate_type_ref_str(w.lhs.as_str())
                .map_err(|e| err(format!("invalid where predicate lhs syntax: {e}")))?;
            if w.rhs.is_empty() {
                return Err(err(format!(
                    "where predicate for '{}' has no rhs bounds (expected at least one bound; \
                     `where T:` or `where T =` without rhs is invalid)",
                    w.lhs
                )));
            }
            let operator = match w.operator {
                BoundOpDto::Bound => BoundOp::Bound,
                BoundOpDto::Equal => {
                    // Equality constraints accept a single RHS type.
                    if w.rhs.len() != 1 {
                        return Err(err(format!(
                            "where predicate for '{}' with operator Equal must have exactly one \
                             rhs entry (got {}); `where T::Assoc = U` accepts a single RHS only",
                            w.lhs,
                            w.rhs.len()
                        )));
                    }
                    BoundOp::Equal
                }
            };
            let rhs = w
                .rhs
                .into_iter()
                .enumerate()
                .map(|(idx, entry)| {
                    match operator {
                        BoundOp::Bound => validate_bound_str(&entry)
                            .map_err(|e| err(format!("invalid where predicate rhs[{idx}]: {e}")))?,
                        BoundOp::Equal => validate_type_ref_str(&entry)
                            .map_err(|e| err(format!("invalid where predicate rhs[{idx}]: {e}")))?,
                    }
                    TypeRef::new(entry.clone())
                        .map_err(|e| err(format!("invalid rhs type ref '{entry}': {e}")))
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok::<WherePredicateDecl, CatalogueDocumentCodecError>(WherePredicateDecl {
                lhs,
                rhs,
                operator,
            })
        })
        .collect()
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

    let generics = method_generics_from_dtos(entry_name, dto.generics)?;
    let where_predicates = where_predicates_from_dtos(entry_name, dto.where_predicates)?;

    let mut decl = MethodDeclaration::new(name, receiver, params, returns, dto.is_async, dto.docs);
    decl.has_default_impl = dto.has_default_impl;
    decl.generics = generics;
    decl.where_predicates = where_predicates;
    Ok(decl)
}

pub(super) fn param_decl_from_dto(
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

fn trait_impl_from_dto(dto: TraitImplDto) -> Result<TraitImplDeclV2, CatalogueDocumentCodecError> {
    let err = |reason: String| CatalogueDocumentCodecError::InvalidEntry {
        entry_name: dto.trait_ref.clone(),
        reason,
    };

    let action = ItemAction::from_str(&dto.action)
        .map_err(|e| err(format!("invalid action '{}': {e}", dto.action)))?;

    let trait_ref = TypeRef::new(dto.trait_ref.clone())
        .map_err(|e| err(format!("invalid trait_ref '{}': {e}", dto.trait_ref)))?;
    let for_type = TypeRef::new(dto.for_type.clone())
        .map_err(|e| err(format!("invalid for_type '{}': {e}", dto.for_type)))?;

    let entry_name = dto.trait_ref.clone();
    let impl_generics = method_generics_from_dtos(&entry_name, dto.impl_generics)?;
    let impl_where_predicates = where_predicates_from_dtos(&entry_name, dto.impl_where_predicates)?;

    let mut decl = TraitImplDeclV2::new(trait_ref, for_type);
    decl.action = action;
    decl.impl_generics = impl_generics;
    decl.impl_where_predicates = impl_where_predicates;
    Ok(decl)
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

    let role = contract_role_from_dto(name, dto.role)?;

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

    let generics = method_generics_from_dtos(name, dto.generics)?;
    let where_predicates = where_predicates_from_dtos(name, dto.where_predicates)?;

    let assoc_types = dto
        .assoc_types
        .into_iter()
        .enumerate()
        .map(|(idx, d)| assoc_type_decl_from_dto(name, idx, d))
        .collect::<Result<Vec<_>, _>>()?;
    let assoc_consts = dto
        .assoc_consts
        .into_iter()
        .enumerate()
        .map(|(idx, d)| assoc_const_decl_from_dto(name, idx, d))
        .collect::<Result<Vec<_>, _>>()?;
    validate_trait_item_names(name, &methods, &assoc_types, &assoc_consts)?;

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

    Ok(TraitEntry::new(
        action,
        role,
        methods,
        assoc_types,
        assoc_consts,
        supertrait_bounds,
        generics,
        where_predicates,
        module_path,
        dto.docs.map(DocString::new),
        spec_refs,
        informal_grounds,
    ))
}

// validate_trait_item_names, inherent_impl_from_dto, and function_entry_from_dto are in
// decode_impls.rs (extracted to keep this module under the 700-line size budget).
