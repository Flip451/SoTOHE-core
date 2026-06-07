//! DTO → domain conversions for [`CatalogueDocument`].
//!
//! All public-to-module functions convert a DTO type into the corresponding domain type.
//! Validation at this boundary ensures only valid domain values propagate downstream.

use std::collections::HashSet;
use std::str::FromStr;

use domain::tddd::LayerId;
use domain::tddd::catalogue_v2::composite::{
    StructKind, StructShape, TypeKindV2, TypestateMarker, TypestateTransitions,
};
use domain::tddd::catalogue_v2::entries::{
    FunctionEntry, InherentImplDeclV2, TraitEntry, TypeEntry,
};
use domain::tddd::catalogue_v2::identifiers::{FieldName, VariantName};
use domain::tddd::catalogue_v2::roles::{ContractRole, DataRole};
use domain::tddd::catalogue_v2::variants::{FieldDecl, VariantDecl, VariantPayload};
use domain::tddd::catalogue_v2::{
    BoundOp, CatalogueDocument, CrateName, FunctionPath, FunctionRole, ItemAction,
    MethodDeclaration, MethodGenericParam, MethodName, ModulePath, ParamDeclaration, ParamName,
    SelfReceiver, TraitImplDeclV2, TraitName, TypeName, TypeRef, WherePredicateDecl,
};

use crate::tddd::spec_ground_codec::{informal_grounds_from_dtos, spec_refs_from_dtos};

use super::CatalogueDocumentCodecError;
use super::validate::validate_bound_str;
// Re-export validate_type_ref_str and validate_trait_ref_is_path so that encode.rs
// can continue to reference them as `super::decode::validate_*`.
use super::dto::{
    BoundOpDto, CatalogueDocumentDto, FieldDeclDto, FunctionEntryDto, InherentImplDeclDto,
    MethodDeclarationDto, MethodGenericParamDto, ParamDto, StructShapeDto, TraitEntryDto,
    TraitImplDto, TypeEntryDto, TypeKindDto, TypestateMarkerDto, VariantDeclDto, VariantPayloadDto,
    WherePredicateDeclDto,
};
pub(super) use super::validate::{validate_trait_ref_is_path, validate_type_ref_str};

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

    // InherentImpls
    for impl_dto in dto.inherent_impls {
        let impl_decl = inherent_impl_from_dto(impl_dto)?;
        doc.inherent_impls.push(impl_decl);
    }

    // TraitImpls (top-level, ADR `2026-05-20-0048` D1)
    for ti_dto in dto.trait_impls {
        let ti = trait_impl_from_dto(ti_dto)?;
        doc.trait_impls.push(ti);
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

/// Convert a Vec of `MethodGenericParamDto` (shared by Method and Function entries)
/// into validated `MethodGenericParam` values, rejecting duplicate names.
fn method_generics_from_dtos(
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

/// Convert a Vec of `WherePredicateDeclDto` (shared by Method, Function, and Trait entries)
/// into validated `WherePredicateDecl` values, rejecting empty `lhs` or `rhs` entries.
///
/// A `WherePredicateDeclDto` with an empty `rhs` vector and `BoundOp::Bound` operator is
/// rejected because `where T:` (no bound after the colon) is syntactically invalid in Rust
/// and would produce a `WherePredicate::BoundPredicate { bounds: vec![] }` in the extended
/// crate representation.  For `BoundOp::Equal` predicates an empty `rhs` is also rejected.
fn where_predicates_from_dtos(
    entry_name: &str,
    dtos: Vec<WherePredicateDeclDto>,
) -> Result<Vec<WherePredicateDecl>, CatalogueDocumentCodecError> {
    let err = |reason: String| CatalogueDocumentCodecError::InvalidEntry {
        entry_name: entry_name.to_owned(),
        reason,
    };
    dtos.into_iter()
        .map(|w| {
            // Validate non-empty first (TypeRef::new check), then validate Rust type syntax.
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
                    // `Equal` predicates (`where T::Assoc = U`) require exactly one RHS entry.
                    // Multiple RHS entries would be invalid Rust syntax for an equality constraint.
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
                    // `Bound` RHS entries are trait bounds (e.g. `Clone`, `Iterator<Item = u32>`);
                    // use `validate_bound_str` (parses `syn::TypeParamBound`).
                    // `Equal` RHS entries are type expressions (e.g. `u32`, `Vec<T>`);
                    // use `validate_type_ref_str` (parses `syn::Type`) because
                    // `validate_bound_str` rejects plain types like `u32`.
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

/// Converts a top-level `TraitImplDto` into a `TraitImplDeclV2` (ADR `2026-05-20-0048` D2).
///
/// Decodes `action`, validates `trait_ref` as a Rust path (not an arbitrary type),
/// validates both `trait_ref` and `for_type` as non-empty `TypeRef` values and as
/// valid Rust type expressions via `syn::parse_str`. Round-trips the `action` field
/// through `ItemAction::from_str` (mirrors `type_entry_from_dto`).
fn trait_impl_from_dto(dto: TraitImplDto) -> Result<TraitImplDeclV2, CatalogueDocumentCodecError> {
    let err = |reason: String| CatalogueDocumentCodecError::InvalidEntry {
        entry_name: dto.trait_ref.clone(),
        reason,
    };

    // Decode action (mirrors TypeEntryDto / TraitEntryDto action handling).
    let action = ItemAction::from_str(&dto.action)
        .map_err(|e| err(format!("invalid action '{}': {e}", dto.action)))?;

    // Validate and construct trait_ref
    let trait_ref = TypeRef::new(dto.trait_ref.clone())
        .map_err(|e| err(format!("invalid trait_ref '{}': {e}", dto.trait_ref)))?;
    // Validate as a general type expression first (non-empty, parseable Rust type).
    validate_type_ref_str(trait_ref.as_str())
        .map_err(|e| err(format!("invalid trait_ref syntax: {e}")))?;
    // Then tighten: trait_ref must be a path type, not a reference / slice / tuple.
    // This mirrors the invariant enforced by `resolve_trait_ref_for_top_level` in the
    // downstream codec, surfacing invalid forms early at the DTO boundary.
    validate_trait_ref_is_path(trait_ref.as_str())
        .map_err(|e| err(format!("invalid trait_ref (must be a path): {e}")))?;

    // Validate and construct for_type
    let for_type = TypeRef::new(dto.for_type.clone())
        .map_err(|e| err(format!("invalid for_type '{}': {e}", dto.for_type)))?;
    validate_type_ref_str(for_type.as_str())
        .map_err(|e| err(format!("invalid for_type syntax: {e}")))?;

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

    let generics = method_generics_from_dtos(name, dto.generics)?;
    let where_predicates = where_predicates_from_dtos(name, dto.where_predicates)?;

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
        generics,
        where_predicates,
        module_path,
        docs: dto.docs,
        spec_refs,
        informal_grounds,
    })
}

pub(super) fn inherent_impl_from_dto(
    dto: InherentImplDeclDto,
) -> Result<InherentImplDeclV2, CatalogueDocumentCodecError> {
    let err = |name: &str, reason: String| CatalogueDocumentCodecError::InvalidEntry {
        entry_name: name.to_owned(),
        reason,
    };

    // Keep a str reference alive for the error context closures below.
    let type_name_str = dto.type_name.as_str();

    let type_name = TypeName::new(type_name_str)
        .map_err(|e| err(type_name_str, format!("invalid type_name: {e}")))?;

    let impl_generics = method_generics_from_dtos(type_name_str, dto.impl_generics)?;
    let impl_where_predicates =
        where_predicates_from_dtos(type_name_str, dto.impl_where_predicates)?;

    let methods = dto
        .methods
        .into_iter()
        .map(|m| method_decl_from_dto(type_name_str, m))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(InherentImplDeclV2 { type_name, impl_generics, impl_where_predicates, methods })
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

    let generics = method_generics_from_dtos(name, dto.generics)?;
    let where_predicates = where_predicates_from_dtos(name, dto.where_predicates)?;

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
        generics,
        where_predicates,
        docs: dto.docs,
        spec_refs,
        informal_grounds,
    })
}
