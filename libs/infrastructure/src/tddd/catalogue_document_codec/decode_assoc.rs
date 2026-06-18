//! Decode helpers for associated type and associated const catalogue entries.
//!
//! Extracted from `decode.rs` to keep that module within the 700-line size budget.

use domain::tddd::catalogue_v2::entries::{AssocConstDecl, AssocTypeDecl};
use domain::tddd::catalogue_v2::{AssocConstName, TypeName, TypeRef};

use super::CatalogueDocumentCodecError;
use super::dto::{AssocConstDeclDto, AssocTypeDeclDto};
use super::validate::{validate_bound_str, validate_type_ref_str};

pub(super) fn assoc_type_decl_from_dto(
    trait_name: &str,
    idx: usize,
    dto: AssocTypeDeclDto,
) -> Result<AssocTypeDecl, CatalogueDocumentCodecError> {
    let err = |reason: String| CatalogueDocumentCodecError::InvalidEntry {
        entry_name: trait_name.to_owned(),
        reason,
    };
    // Validate and construct TypeName (reuses the type-level identifier newtype;
    // an associated type name like `Input` is a type identifier — no new type needed).
    let name = TypeName::new(&dto.name)
        .map_err(|e| err(format!("assoc_types[{idx}].name '{}' is invalid: {e}", dto.name)))?;
    let bounds = dto
        .bounds
        .into_iter()
        .enumerate()
        .map(|(bidx, b)| {
            validate_bound_str(&b)
                .map_err(|e| err(format!("assoc_types[{idx}].bounds[{bidx}]: {e}")))?;
            TypeRef::new(b.clone())
                .map_err(|e| err(format!("assoc_types[{idx}].bounds[{bidx}] type ref '{b}': {e}")))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let default = dto
        .default
        .map(|d| {
            validate_type_ref_str(&d)
                .map_err(|e| err(format!("assoc_types[{idx}].default: {e}")))?;
            TypeRef::new(d.clone())
                .map_err(|e| err(format!("assoc_types[{idx}].default type ref '{d}': {e}")))
        })
        .transpose()?;
    Ok(AssocTypeDecl { name, bounds, default })
}

pub(super) fn assoc_const_decl_from_dto(
    trait_name: &str,
    idx: usize,
    dto: AssocConstDeclDto,
) -> Result<AssocConstDecl, CatalogueDocumentCodecError> {
    let err = |reason: String| CatalogueDocumentCodecError::InvalidEntry {
        entry_name: trait_name.to_owned(),
        reason,
    };
    // Validate and construct AssocConstName (dedicated newtype for const identifiers).
    let name = AssocConstName::new(&dto.name)
        .map_err(|e| err(format!("assoc_consts[{idx}].name '{}' is invalid: {e}", dto.name)))?;
    if dto.ty.is_empty() {
        return Err(err(format!("assoc_consts[{idx}].ty is empty")));
    }
    validate_type_ref_str(&dto.ty).map_err(|e| err(format!("assoc_consts[{idx}].ty: {e}")))?;
    let ty = TypeRef::new(dto.ty.clone())
        .map_err(|e| err(format!("assoc_consts[{idx}].ty type ref '{}': {e}", dto.ty)))?;
    let default_value = dto
        .default_value
        .map(|value| {
            if value.trim().is_empty() {
                return Err(err(format!("assoc_consts[{idx}].default_value is empty")));
            }
            syn::parse_str::<syn::Expr>(&value).map_err(|e| {
                err(format!(
                    "assoc_consts[{idx}].default_value '{value}' is not a valid Rust expression: {e}"
                ))
            })?;
            Ok(value)
        })
        .transpose()?;
    Ok(AssocConstDecl { name, ty, default_value })
}
