//! Decode helpers for `InherentImplDeclV2`, `FunctionEntry`, and trait-item name uniqueness.
//!
//! Extracted from `decode.rs` to keep that module within the 700-line size budget.

use std::collections::HashSet;

use domain::tddd::catalogue_v2::entries::{
    AssocConstDecl, AssocTypeDecl, FunctionEntry, InherentImplDeclV2,
};
use domain::tddd::catalogue_v2::roles::{FunctionRole, ItemAction};
use domain::tddd::catalogue_v2::{MethodDeclaration, TypeName, TypeRef};

use std::str::FromStr;

use crate::tddd::spec_ground_codec::{informal_grounds_from_dtos, spec_refs_from_dtos};

use super::CatalogueDocumentCodecError;
use super::decode::{
    method_decl_from_dto, method_generics_from_dtos, param_decl_from_dto,
    where_predicates_from_dtos,
};
use super::dto::{FunctionEntryDto, InherentImplDeclDto};

/// Validate that trait item names are unique within Rust's associated item namespaces.
///
/// Associated types live in the type namespace. Methods and associated consts share
/// the value namespace. A trait may therefore legally contain `type Item; fn Item();`,
/// but may not contain `const Item: ...; fn Item();`.
pub(super) fn validate_trait_item_names(
    entry_name: &str,
    methods: &[MethodDeclaration],
    assoc_types: &[AssocTypeDecl],
    assoc_consts: &[AssocConstDecl],
) -> Result<(), CatalogueDocumentCodecError> {
    let err = |reason: String| CatalogueDocumentCodecError::InvalidEntry {
        entry_name: entry_name.to_owned(),
        reason,
    };
    let mut type_names = HashSet::new();
    for assoc_type in assoc_types {
        let item_name = assoc_type.name.as_str();
        if !type_names.insert(item_name.to_owned()) {
            return Err(err(format!("duplicate trait associated type name '{item_name}'")));
        }
    }

    let mut value_names = HashSet::new();
    for method in methods {
        let item_name = method.name.as_str();
        if !value_names.insert(item_name.to_owned()) {
            return Err(err(format!("duplicate trait value item name '{item_name}'")));
        }
    }
    for assoc_const in assoc_consts {
        let item_name = assoc_const.name.as_str();
        if !value_names.insert(item_name.to_owned()) {
            return Err(err(format!("duplicate trait value item name '{item_name}'")));
        }
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use domain::tddd::catalogue_v2::entries::{AssocConstDecl, AssocTypeDecl};
    use domain::tddd::catalogue_v2::{
        AssocConstName, MethodDeclaration, MethodName, TypeName, TypeRef,
    };

    use super::validate_trait_item_names;

    fn method(name: &str) -> Result<MethodDeclaration, String> {
        Ok(MethodDeclaration::new(
            MethodName::new(name).map_err(|e| e.to_string())?,
            None,
            vec![],
            TypeRef::new("()").map_err(|e| e.to_string())?,
            false,
            None,
        ))
    }

    fn assoc_type(name: &str) -> Result<AssocTypeDecl, String> {
        Ok(AssocTypeDecl {
            name: TypeName::new(name).map_err(|e| e.to_string())?,
            bounds: vec![],
            default: None,
        })
    }

    fn assoc_const(name: &str) -> Result<AssocConstDecl, String> {
        Ok(AssocConstDecl {
            name: AssocConstName::new(name).map_err(|e| e.to_string())?,
            ty: TypeRef::new("usize").map_err(|e| e.to_string())?,
            default_value: None,
        })
    }

    #[test]
    fn test_validate_trait_item_names_type_and_method_same_name_allowed() -> Result<(), String> {
        let methods = vec![method("Item")?];
        let assoc_types = vec![assoc_type("Item")?];
        let result = validate_trait_item_names("T", &methods, &assoc_types, &[]);

        assert!(result.is_ok(), "type and method names occupy distinct namespaces: {result:?}");
        Ok(())
    }

    #[test]
    fn test_validate_trait_item_names_type_and_const_same_name_allowed() -> Result<(), String> {
        let assoc_types = vec![assoc_type("Item")?];
        let assoc_consts = vec![assoc_const("Item")?];
        let result = validate_trait_item_names("T", &[], &assoc_types, &assoc_consts);

        assert!(result.is_ok(), "type and const names occupy distinct namespaces: {result:?}");
        Ok(())
    }

    #[test]
    fn test_validate_trait_item_names_method_and_const_same_name_rejected() -> Result<(), String> {
        let methods = vec![method("Item")?];
        let assoc_consts = vec![assoc_const("Item")?];
        let result = validate_trait_item_names("T", &methods, &[], &assoc_consts);

        assert!(result.is_err(), "method and const names share the value namespace");
        Ok(())
    }
}
