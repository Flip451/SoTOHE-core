//! Decode helpers for nested declarations, `InherentImplDeclV2`, `FunctionEntry`, and
//! trait-item name uniqueness.
//!
//! Extracted from `decode.rs` to keep that module within the 700-line size budget.

use std::collections::HashSet;

use domain::tddd::catalogue_v2::entries::{
    AssocConstDecl, AssocTypeDecl, FunctionEntry, InherentImplDeclV2,
};
use domain::tddd::catalogue_v2::roles::{FunctionRole, ItemAction};
use domain::tddd::catalogue_v2::{
    BoundOp, DocString, MethodDeclaration, MethodGenericParam, MethodName, ParamDeclaration,
    ParamName, SelfReceiver, TypeName, TypeRef, WherePredicateDecl,
};

use std::str::FromStr;

use crate::tddd::spec_ground_codec::{informal_grounds_from_dtos, spec_refs_from_dtos};

use super::CatalogueDocumentCodecError;
use super::dto::{
    BoundOpDto, FunctionEntryDto, InherentImplDeclDto, MethodDeclarationDto, MethodGenericParamDto,
    ParamDto, WherePredicateDeclDto,
};
use super::validate::{validate_bound_str_with_generics, validate_type_ref_str_with_generics};

pub(super) fn method_generics_from_dtos(
    entry_name: &str,
    dtos: Vec<MethodGenericParamDto>,
) -> Result<Vec<MethodGenericParam>, CatalogueDocumentCodecError> {
    method_generics_from_dtos_with_outer_generics(entry_name, dtos, &[])
}

pub(super) fn method_generics_from_dtos_with_outer_generics(
    entry_name: &str,
    dtos: Vec<MethodGenericParamDto>,
    outer_generic_names: &[&str],
) -> Result<Vec<MethodGenericParam>, CatalogueDocumentCodecError> {
    let err = |reason: String| CatalogueDocumentCodecError::InvalidEntry {
        entry_name: entry_name.to_owned(),
        reason,
    };
    let generic_names = dtos.iter().map(|generic| generic.name.clone()).collect::<Vec<_>>();
    let generic_name_refs = outer_generic_names
        .iter()
        .copied()
        .chain(generic_names.iter().map(String::as_str))
        .collect::<Vec<_>>();
    let generics: Vec<MethodGenericParam> = dtos
        .into_iter()
        .map(|g| {
            // Non-alias entries keep the parent's shape-only name validation
            // (`ParamName`): rustdoc normalizes `r#type` to `type`, so a
            // keyword name is a legitimate pre-existing representation here.
            // The non-keyword restriction applies only in alias validation
            // (`validate_type_alias_generic_names`), per spec OUT-01.
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
                    // TypeParamBound accepts `?Sized`; rustdoc-normalized raw
                    // identifiers are restored only when parsing requires it.
                    validate_bound_str_with_generics(&bound, &generic_name_refs)
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

    let mut seen: HashSet<&str> = outer_generic_names.iter().copied().collect();
    for g in &generics {
        if !seen.insert(g.name.as_str()) {
            return Err(err(format!("duplicate generic param name '{}'", g.name.as_str())));
        }
    }
    Ok(generics)
}

pub(super) fn where_predicates_from_dtos_with_generics(
    entry_name: &str,
    dtos: Vec<WherePredicateDeclDto>,
    generic_names: &[&str],
) -> Result<Vec<WherePredicateDecl>, CatalogueDocumentCodecError> {
    let err = |reason: String| CatalogueDocumentCodecError::InvalidEntry {
        entry_name: entry_name.to_owned(),
        reason,
    };
    dtos.into_iter()
        .map(|w| {
            let lhs = TypeRef::new(w.lhs.clone())
                .map_err(|e| err(format!("invalid where predicate lhs '{}': {e}", w.lhs)))?;
            validate_type_ref_str_with_generics(w.lhs.as_str(), generic_names)
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
                        BoundOp::Bound => validate_bound_str_with_generics(&entry, generic_names)
                            .map_err(|e| {
                            err(format!("invalid where predicate rhs[{idx}]: {e}"))
                        })?,
                        BoundOp::Equal => {
                            validate_type_ref_str_with_generics(&entry, generic_names).map_err(
                                |e| err(format!("invalid where predicate rhs[{idx}]: {e}")),
                            )?
                        }
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

pub(super) fn method_decl_from_dto_with_outer_generics(
    entry_name: &str,
    dto: MethodDeclarationDto,
    outer_generic_names: &[&str],
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

    let generics = method_generics_from_dtos_with_outer_generics(
        entry_name,
        dto.generics,
        outer_generic_names,
    )?;
    let generic_names = outer_generic_names
        .iter()
        .copied()
        .chain(generics.iter().map(|generic| generic.name.as_str()))
        .collect::<Vec<_>>();
    let returns = TypeRef::new(dto.returns.clone())
        .map_err(|e| err(format!("invalid returns type '{}': {e}", dto.returns)))?;
    let where_predicates =
        where_predicates_from_dtos_with_generics(entry_name, dto.where_predicates, &generic_names)?;

    let spec_refs = spec_refs_from_dtos(&dto.spec_refs)
        .map_err(|e| err(format!("invalid {}: {}", e.field, e.reason)))?;
    Ok(MethodDeclaration::new(
        name,
        receiver,
        params,
        returns,
        dto.is_async,
        dto.has_default_impl,
        generics,
        where_predicates,
        spec_refs,
        dto.docs.map(DocString::new),
    ))
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
        let item_name = method.name().as_str();
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
    let generic_names =
        impl_generics.iter().map(|generic| generic.name.as_str()).collect::<Vec<_>>();
    let impl_where_predicates = where_predicates_from_dtos_with_generics(
        type_name_str,
        dto.impl_where_predicates,
        &generic_names,
    )?;

    let methods = dto
        .methods
        .into_iter()
        .map(|m| method_decl_from_dto_with_outer_generics(type_name_str, m, &generic_names))
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
    let generic_names = generics.iter().map(|generic| generic.name.as_str()).collect::<Vec<_>>();
    let where_predicates =
        where_predicates_from_dtos_with_generics(name, dto.where_predicates, &generic_names)?;

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

    Ok(FunctionEntry::new(
        action,
        role,
        params,
        returns,
        dto.is_async,
        generics,
        where_predicates,
        dto.docs.map(DocString::new),
        spec_refs,
        informal_grounds,
    ))
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
            false,
            vec![],
            vec![],
            vec![],
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
