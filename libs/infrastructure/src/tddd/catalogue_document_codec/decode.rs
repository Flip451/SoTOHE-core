//! DTO → domain conversions for [`CatalogueDocument`].
use domain::tddd::LayerId;
use domain::tddd::catalogue_v2::composite::{
    StructKind, StructShape, TypeKindV2, TypestateMarker, TypestateTransitions,
};
use domain::tddd::catalogue_v2::document::CatalogueDocument;
use domain::tddd::catalogue_v2::entries::{TraitEntry, TypeEntry};
use domain::tddd::catalogue_v2::identifiers::{DocString, FieldName, VariantName};
use domain::tddd::catalogue_v2::variants::{FieldDecl, VariantDecl, VariantPayload};
use domain::tddd::catalogue_v2::{
    CatalogueEntryKey, CrateName, DeletionRecord, FullyQualifiedItemPath, FunctionPath, ItemAction,
    MethodGenericParam, MethodName, ModulePath, TraitImplDeclV2, TypeName, TypeRef,
};
use std::str::FromStr;

use super::decode_assoc::{assoc_const_decl_from_dto, assoc_type_decl_from_dto};
use super::decode_impls::{
    function_entry_from_dto, inherent_impl_from_dto, method_decl_from_dto_with_outer_generics,
    method_generics_from_dtos, validate_trait_item_names, where_predicates_from_dtos_with_generics,
};
use super::decode_roles::{contract_role_from_dto, data_role_from_dto};
use super::dto::{
    CatalogueDocumentDto, FieldDeclDto, StructShapeDto, TraitEntryDto, TraitImplDto, TypeEntryDto,
    TypeKindDto, TypestateMarkerDto, VariantDeclDto, VariantPayloadDto,
};
use super::dto_slots::{EntrySlotDto, TombstoneDto};
use super::validate::{
    validate_bound_str_with_generics, validate_type_alias_generic_name_strs,
    validate_type_alias_generic_names, validate_type_alias_relaxed_bounds,
    validate_type_alias_target, validate_type_alias_where_predicates,
};
use super::{CatalogueDocumentCodecError, EXPLICIT_ROOT_MODULE_PATH};
use crate::tddd::spec_ground_codec::{informal_grounds_from_dtos, spec_refs_from_dtos};
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
    let mut doc = CatalogueDocument::new(dto.schema_version, crate_name.clone(), layer);
    // Delete slots become deletion records rather than live entries.
    for (type_name_str, slot) in dto.types {
        let type_name = CatalogueEntryKey::try_new(type_name_str.clone())
            .map_err(|e| err(&type_name_str, format!("invalid catalogue entry key: {e}")))?;
        match slot {
            EntrySlotDto::Tombstone(tombstone) => {
                let (spec_refs, informal_grounds) =
                    tombstone_grounding(&type_name_str, &tombstone)?;
                let name = tombstone_entry_key(&crate_name, &type_name, &tombstone)?;
                doc.push_deletion(DeletionRecord::Type { name, spec_refs, informal_grounds });
            }
            EntrySlotDto::Live(entry_dto) => {
                let entry = type_entry_from_dto(&type_name_str, entry_dto)?;
                doc.insert_type(type_name, entry);
            }
        }
    }
    // Traits
    for (trait_name_str, slot) in dto.traits {
        let trait_name = CatalogueEntryKey::try_new(trait_name_str.clone())
            .map_err(|e| err(&trait_name_str, format!("invalid catalogue entry key: {e}")))?;
        match slot {
            EntrySlotDto::Tombstone(tombstone) => {
                let (spec_refs, informal_grounds) =
                    tombstone_grounding(&trait_name_str, &tombstone)?;
                let name = tombstone_entry_key(&crate_name, &trait_name, &tombstone)?;
                doc.push_deletion(DeletionRecord::Trait { name, spec_refs, informal_grounds });
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

/// Keeps the module context of a type/trait tombstone in its identity-only
/// domain record. Type and trait deletion records have no separate
/// `module_path` field, so a bare legacy key must be promoted to the local
/// module-qualified notation before the DTO context is discarded. A key that
/// already contains a path is retained as written, matching live-entry key
/// handling and preserving explicitly qualified spellings.
fn tombstone_entry_key(
    crate_name: &CrateName,
    entry_key: &CatalogueEntryKey,
    tombstone: &TombstoneDto,
) -> Result<CatalogueEntryKey, CatalogueDocumentCodecError> {
    let entry_name = entry_key.as_str();
    // Route the tombstone through the same marker-aware decoder as live
    // entries: the explicit root marker written by `encode_module_path` must
    // decode as the crate root, and a legacy empty value keeps its root
    // meaning for tombstones (they carry no separate placement state).
    let module_path = decode_module_path(&tombstone.module_path)
        .map_err(|error| CatalogueDocumentCodecError::InvalidEntry {
            entry_name: entry_name.to_owned(),
            reason: format!("invalid module_path '{}': {error}", tombstone.module_path),
        })?
        .unwrap_or_else(ModulePath::root);
    let identity =
        FullyQualifiedItemPath::from_catalogue_entry_key(crate_name, entry_key, &module_path)
            .map_err(|error| CatalogueDocumentCodecError::InvalidEntry {
                entry_name: entry_name.to_owned(),
                reason: format!("invalid catalogue entry identity: {error}"),
            })?;
    if !tombstone.module_path.is_empty() && identity.module_path() != Some(&module_path) {
        return Err(CatalogueDocumentCodecError::InvalidEntry {
            entry_name: entry_name.to_owned(),
            reason: format!(
                "tombstone key '{entry_name}' identifies module_path '{}', but tombstone \
                 module_path is '{}'",
                identity.module_path().map_or_else(|| "<unplaced>".to_owned(), ToString::to_string),
                tombstone.module_path
            ),
        });
    }
    let effective_name = if module_path.is_root() || entry_name.contains("::") {
        entry_name.to_owned()
    } else {
        format!("{module_path}::{entry_name}")
    };
    CatalogueEntryKey::try_new(effective_name.clone()).map_err(|error| {
        CatalogueDocumentCodecError::InvalidEntry {
            entry_name: entry_name.to_owned(),
            reason: format!("invalid catalogue entry key: {error}"),
        }
    })
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
    if matches!(&kind, TypeKindV2::TypeAlias { .. }) {
        validate_type_alias_generic_name_strs(name, dto.generics.iter().map(|g| g.name.as_str()))?;
    }
    let generics = method_generics_from_dtos(name, dto.generics)?;
    if matches!(&kind, TypeKindV2::TypeAlias { .. }) {
        validate_type_alias_generic_names(name, &generics)?;
    }
    if matches!(&kind, TypeKindV2::TypeAlias { generics: alias_generics, .. }
        if !alias_generics.is_empty() && !generics.is_empty())
    {
        return Err(err(
            "type alias generic declarations must not appear in both the entry and kind payload"
                .to_owned(),
        ));
    }
    let generic_names = match &kind {
        TypeKindV2::TypeAlias { generics: alias_generics, .. } if !alias_generics.is_empty() => {
            alias_generics.iter().map(|generic| generic.name.as_str()).collect::<Vec<_>>()
        }
        _ => generics.iter().map(|generic| generic.name.as_str()).collect::<Vec<_>>(),
    };
    if let TypeKindV2::TypeAlias { target, .. } = &kind {
        validate_type_alias_target(name, target.as_str(), &generic_names)?;
    }
    let methods = dto
        .methods
        .into_iter()
        .map(|m| method_decl_from_dto_with_outer_generics(name, m, &generic_names))
        .collect::<Result<Vec<_>, _>>()?;
    let where_predicates =
        where_predicates_from_dtos_with_generics(name, dto.where_predicates, &generic_names)?;
    if let TypeKindV2::TypeAlias { generics: alias_generics, .. } = &kind {
        validate_type_alias_where_predicates(name, &where_predicates, &generic_names)?;
        let effective_generics: &[MethodGenericParam] =
            if alias_generics.is_empty() { &generics } else { alias_generics };
        validate_type_alias_relaxed_bounds(name, effective_generics, &where_predicates)?;
    }

    let module_path = decode_module_path(&dto.module_path)
        .map_err(|e| err(format!("invalid module_path '{}': {e}", dto.module_path)))?;
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
        TypeKindDto::TypeAlias { target, generics } => {
            let target = TypeRef::new(target.clone())
                .map_err(|e| err(format!("invalid type_alias target '{}': {e}", target)))?;
            validate_type_alias_generic_name_strs(name, generics.iter().map(|g| g.name.as_str()))?;
            let generics = method_generics_from_dtos(name, generics)?;
            validate_type_alias_generic_names(name, &generics)?;
            Ok(TypeKindV2::TypeAlias { target, generics })
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
    let generic_names =
        impl_generics.iter().map(|generic| generic.name.as_str()).collect::<Vec<_>>();
    let impl_where_predicates = where_predicates_from_dtos_with_generics(
        &entry_name,
        dto.impl_where_predicates,
        &generic_names,
    )?;

    Ok(TraitImplDeclV2::from_parts(
        action,
        trait_ref,
        for_type,
        impl_generics,
        impl_where_predicates,
    ))
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

    let module_path = decode_module_path(&dto.module_path)
        .map_err(|e| err(format!("invalid module_path '{}': {e}", dto.module_path)))?;
    let generics = method_generics_from_dtos(name, dto.generics)?;
    let generic_names = generics.iter().map(|generic| generic.name.as_str()).collect::<Vec<_>>();
    let methods = dto
        .methods
        .into_iter()
        .map(|m| method_decl_from_dto_with_outer_generics(name, m, &generic_names))
        .collect::<Result<Vec<_>, _>>()?;
    let supertrait_bounds = dto
        .supertrait_bounds
        .into_iter()
        .enumerate()
        .map(|(idx, bound)| {
            validate_bound_str_with_generics(&bound, &generic_names)
                .map_err(|e| err(format!("invalid supertrait_bounds[{idx}]: {e}")))?;
            TypeRef::new(bound.clone())
                .map_err(|e| err(format!("invalid supertrait_bound type ref '{bound}': {e}")))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let where_predicates =
        where_predicates_from_dtos_with_generics(name, dto.where_predicates, &generic_names)?;

    let assoc_types = dto
        .assoc_types
        .into_iter()
        .enumerate()
        .map(|(idx, d)| assoc_type_decl_from_dto(name, idx, d, &generic_names))
        .collect::<Result<Vec<_>, _>>()?;
    let assoc_consts = dto
        .assoc_consts
        .into_iter()
        .enumerate()
        .map(|(idx, d)| assoc_const_decl_from_dto(name, idx, d, &generic_names))
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

fn decode_module_path(
    module_path: &str,
) -> Result<Option<ModulePath>, domain::tddd::catalogue_v2::identifiers::IdentifierError> {
    if module_path.is_empty() {
        Ok(None)
    } else if module_path == EXPLICIT_ROOT_MODULE_PATH {
        Ok(Some(ModulePath::root()))
    } else {
        ModulePath::from_str(module_path).map(Some)
    }
}
