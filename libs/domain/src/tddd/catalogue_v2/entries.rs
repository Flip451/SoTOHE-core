//! Catalogue entry types for the catalogue v2 schema.
//!
//! Implements the three entry types that populate the `BTreeMap`s in `CatalogueDocument`:
//! - [`TypeEntry`]: entry in `CatalogueDocument::types`. Carries `DataRole` (not `ContractRole`).
//! - [`TraitEntry`]: entry in `CatalogueDocument::traits`. Carries `ContractRole`.
//! - [`FunctionEntry`]: entry in `CatalogueDocument::functions`. Carries `FunctionRole`.
//!
//! The Role × Entry type constraint (ADR 1 D2) is enforced at the Rust type system level:
//! `TypeEntry.role: DataRole` means a `ContractRole` value cannot be stored there without
//! a compile error. No runtime guard is needed.
//!
//! No serde derives — per ADR `knowledge/adr/2026-04-14-1531-domain-serde-ripout.md`,
//! the domain layer is serialization-free. The infrastructure codec (T003) handles JSON.

use crate::tddd::catalogue_v2::composite::TypeKindV2;
use crate::tddd::catalogue_v2::identifiers::{ModulePath, TypeRef};
use crate::tddd::catalogue_v2::methods::{MethodDeclaration, ParamDeclaration};
use crate::tddd::catalogue_v2::roles::{ContractRole, DataRole, FunctionRole, ItemAction};
use crate::tddd::catalogue_v2::traits::TraitImplDeclV2;

// ---------------------------------------------------------------------------
// TypeEntry — entry in CatalogueDocument::types
// ---------------------------------------------------------------------------

/// Entry in `CatalogueDocument::types` BTreeMap (ADR 1 D7).
///
/// Holds all data about a type (struct / enum / type alias) declared in the catalogue.
/// The `role: DataRole` field ensures that only `DataRole` values can be attached to a
/// type entry — assigning a `ContractRole` is a compile-time error (ADR 1 D2).
///
/// `module_path` defaults to empty (crate root) when not specified in JSON (ADR 1 D7).
/// The infrastructure codec (T003) handles the `serde default` for this field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeEntry {
    /// The action for this entry (Add / Modify / Reference / Delete). Default: `Add`.
    pub action: ItemAction,
    /// The DDD / Clean Architecture role of this type. Only `DataRole` is accepted.
    pub role: DataRole,
    /// The language-level kind (Struct / Enum / TypeAlias) with payload-encoded pattern.
    pub kind: TypeKindV2,
    /// Inherent methods declared on this type.
    pub methods: Vec<MethodDeclaration>,
    /// Trait implementations declared for this type.
    pub trait_impls: Vec<TraitImplDeclV2>,
    /// Module path within the crate (empty = crate root). Serde default = empty.
    pub module_path: ModulePath,
    /// Optional documentation string.
    pub docs: Option<String>,
}

// ---------------------------------------------------------------------------
// TraitEntry — entry in CatalogueDocument::traits
// ---------------------------------------------------------------------------

/// Entry in `CatalogueDocument::traits` BTreeMap (ADR 1 D7).
///
/// Holds all data about a trait declared in the catalogue. The `role: ContractRole`
/// field ensures that only `ContractRole` values can be attached to a trait entry —
/// assigning a `DataRole` is a compile-time error (ADR 1 D2).
///
/// `module_path` defaults to empty (crate root) when not specified in JSON (ADR 1 D7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitEntry {
    /// The action for this entry (Add / Modify / Reference / Delete). Default: `Add`.
    pub action: ItemAction,
    /// The architectural role of this trait. Only `ContractRole` is accepted.
    pub role: ContractRole,
    /// Methods declared in this trait.
    pub methods: Vec<MethodDeclaration>,
    /// Module path within the crate (empty = crate root). Serde default = empty.
    pub module_path: ModulePath,
    /// Optional documentation string.
    pub docs: Option<String>,
}

// ---------------------------------------------------------------------------
// FunctionEntry — entry in CatalogueDocument::functions
// ---------------------------------------------------------------------------

/// Entry in `CatalogueDocument::functions` BTreeMap (ADR 1 D7).
///
/// Holds all data about a free function declared in the catalogue. The
/// `role: FunctionRole` field ensures that only `FunctionRole` values can be attached
/// to a function entry (ADR 1 D2).
///
/// Note: `returns` uses `TypeRef` which allows empty-ish values; the value `"()"` is
/// used for functions returning the unit type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionEntry {
    /// The action for this entry (Add / Modify / Reference / Delete). Default: `Add`.
    pub action: ItemAction,
    /// The architectural role of this function. Only `FunctionRole` is accepted.
    pub role: FunctionRole,
    /// The function parameters.
    pub params: Vec<ParamDeclaration>,
    /// The return type (generics-inclusive type reference string).
    pub returns: TypeRef,
    /// Whether this function is `async`.
    pub is_async: bool,
    /// Optional documentation string.
    pub docs: Option<String>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::tddd::catalogue_v2::composite::CompositePattern;
    use crate::tddd::catalogue_v2::identifiers::{
        CrateName, FieldName, MethodName, ModulePath, ParamName, TraitName, TypeRef,
    };
    use crate::tddd::catalogue_v2::roles::SelfReceiver;
    use crate::tddd::catalogue_v2::variants::FieldDecl;

    // -----------------------------------------------------------------------
    // TypeEntry
    // -----------------------------------------------------------------------

    #[test]
    fn test_type_entry_with_data_role_compiles() {
        // TypeEntry.role: DataRole — assigning ContractRole is a compile-time error.
        let entry = TypeEntry {
            action: ItemAction::Add,
            role: DataRole::ValueObject,
            kind: TypeKindV2::Struct { pattern: CompositePattern::Plain, fields: vec![] },
            methods: vec![],
            trait_impls: vec![],
            module_path: ModulePath::root(),
            docs: None,
        };
        assert_eq!(entry.role, DataRole::ValueObject);
        assert_eq!(entry.action, ItemAction::Add);
    }

    #[test]
    fn test_type_entry_with_struct_kind_and_fields() {
        let field_name = FieldName::new("email").unwrap();
        let field_ty = TypeRef::new("String").unwrap();
        let fields = vec![FieldDecl::new(field_name, field_ty)];
        let entry = TypeEntry {
            action: ItemAction::Add,
            role: DataRole::Entity,
            kind: TypeKindV2::Struct { pattern: CompositePattern::Plain, fields: fields.clone() },
            methods: vec![],
            trait_impls: vec![],
            module_path: ModulePath::root(),
            docs: Some("A domain entity.".to_string()),
        };
        match &entry.kind {
            TypeKindV2::Struct { pattern, fields: k_fields } => {
                assert_eq!(*pattern, CompositePattern::Plain);
                assert_eq!(k_fields.len(), 1);
            }
            _ => panic!("expected Struct kind"),
        }
        assert_eq!(entry.docs, Some("A domain entity.".to_string()));
    }

    #[test]
    fn test_type_entry_with_trait_impls() {
        let trait_impl = TraitImplDeclV2::new(
            TraitName::new("Display").unwrap(),
            CrateName::new("std").unwrap(),
        );
        let entry = TypeEntry {
            action: ItemAction::Add,
            role: DataRole::ValueObject,
            kind: TypeKindV2::Enum { variants: vec![] },
            methods: vec![],
            trait_impls: vec![trait_impl.clone()],
            module_path: ModulePath::root(),
            docs: None,
        };
        assert_eq!(entry.trait_impls.len(), 1);
        assert_eq!(entry.trait_impls[0], trait_impl);
    }

    #[test]
    fn test_type_entry_with_methods() {
        let method = MethodDeclaration::new(
            MethodName::new("as_str").unwrap(),
            Some(SelfReceiver::SharedRef),
            vec![],
            TypeRef::new("str").unwrap(),
            false,
            None,
        );
        let entry = TypeEntry {
            action: ItemAction::Add,
            role: DataRole::ValueObject,
            kind: TypeKindV2::Struct {
                pattern: CompositePattern::Newtype { inner: TypeRef::new("String").unwrap() },
                fields: vec![],
            },
            methods: vec![method.clone()],
            trait_impls: vec![],
            module_path: ModulePath::root(),
            docs: None,
        };
        assert_eq!(entry.methods.len(), 1);
        assert_eq!(entry.methods[0], method);
    }

    #[test]
    fn test_type_entry_with_module_path() {
        let module_path =
            ModulePath::from_segments(vec!["user".to_string(), "domain".to_string()]).unwrap();
        let entry = TypeEntry {
            action: ItemAction::Modify,
            role: DataRole::AggregateRoot,
            kind: TypeKindV2::Struct { pattern: CompositePattern::Plain, fields: vec![] },
            methods: vec![],
            trait_impls: vec![],
            module_path: module_path.clone(),
            docs: None,
        };
        assert_eq!(entry.module_path, module_path);
        assert_eq!(entry.action, ItemAction::Modify);
    }

    #[test]
    fn test_type_entry_all_data_roles_are_accepted() {
        // Verify that all DataRole values can be used — no runtime rejection.
        let roles = [
            DataRole::ValueObject,
            DataRole::Entity,
            DataRole::AggregateRoot,
            DataRole::DomainService,
            DataRole::Specification,
            DataRole::Factory,
            DataRole::UseCase,
            DataRole::Interactor,
            DataRole::Command,
            DataRole::Query,
            DataRole::Dto,
            DataRole::ErrorType,
            DataRole::SecondaryAdapter,
        ];
        for role in roles {
            let entry = TypeEntry {
                action: ItemAction::Add,
                role,
                kind: TypeKindV2::Struct { pattern: CompositePattern::Plain, fields: vec![] },
                methods: vec![],
                trait_impls: vec![],
                module_path: ModulePath::root(),
                docs: None,
            };
            assert_eq!(entry.role, role);
        }
    }

    // -----------------------------------------------------------------------
    // TraitEntry
    // -----------------------------------------------------------------------

    #[test]
    fn test_trait_entry_with_contract_role_compiles() {
        // TraitEntry.role: ContractRole — assigning DataRole is a compile-time error.
        let entry = TraitEntry {
            action: ItemAction::Add,
            role: ContractRole::SecondaryPort,
            methods: vec![],
            module_path: ModulePath::root(),
            docs: None,
        };
        assert_eq!(entry.role, ContractRole::SecondaryPort);
    }

    #[test]
    fn test_trait_entry_with_methods() {
        let save_method = MethodDeclaration::new(
            MethodName::new("save").unwrap(),
            Some(SelfReceiver::SharedRef),
            vec![ParamDeclaration::new(
                ParamName::new("user").unwrap(),
                TypeRef::new("User").unwrap(),
            )],
            TypeRef::new("Result<(), DomainError>").unwrap(),
            false,
            None,
        );
        let entry = TraitEntry {
            action: ItemAction::Add,
            role: ContractRole::SecondaryPort,
            methods: vec![save_method.clone()],
            module_path: ModulePath::root(),
            docs: Some("User repository port.".to_string()),
        };
        assert_eq!(entry.methods.len(), 1);
        assert_eq!(entry.methods[0], save_method);
        assert_eq!(entry.docs, Some("User repository port.".to_string()));
    }

    #[test]
    fn test_trait_entry_all_contract_roles_are_accepted() {
        let roles = [
            ContractRole::SpecificationPort,
            ContractRole::ApplicationService,
            ContractRole::SecondaryPort,
        ];
        for role in roles {
            let entry = TraitEntry {
                action: ItemAction::Add,
                role,
                methods: vec![],
                module_path: ModulePath::root(),
                docs: None,
            };
            assert_eq!(entry.role, role);
        }
    }

    // -----------------------------------------------------------------------
    // FunctionEntry
    // -----------------------------------------------------------------------

    #[test]
    fn test_function_entry_with_function_role_compiles() {
        // FunctionEntry.role: FunctionRole — assigning DataRole is a compile-time error.
        let entry = FunctionEntry {
            action: ItemAction::Add,
            role: FunctionRole::FreeFunction,
            params: vec![],
            returns: TypeRef::new("()").unwrap(),
            is_async: false,
            docs: None,
        };
        assert_eq!(entry.role, FunctionRole::FreeFunction);
        assert!(!entry.is_async);
    }

    #[test]
    fn test_function_entry_async_with_params_and_returns() {
        let entry = FunctionEntry {
            action: ItemAction::Add,
            role: FunctionRole::UseCaseFunction,
            params: vec![ParamDeclaration::new(
                ParamName::new("cmd").unwrap(),
                TypeRef::new("RegisterUserCommand").unwrap(),
            )],
            returns: TypeRef::new("Result<UserId, ApplicationError>").unwrap(),
            is_async: true,
            docs: Some("Register a new user.".to_string()),
        };
        assert!(entry.is_async);
        assert_eq!(entry.params.len(), 1);
        assert_eq!(entry.docs, Some("Register a new user.".to_string()));
    }

    #[test]
    fn test_function_entry_all_function_roles_are_accepted() {
        let roles = [FunctionRole::FreeFunction, FunctionRole::UseCaseFunction];
        for role in roles {
            let entry = FunctionEntry {
                action: ItemAction::Add,
                role,
                params: vec![],
                returns: TypeRef::new("()").unwrap(),
                is_async: false,
                docs: None,
            };
            assert_eq!(entry.role, role);
        }
    }

    // -----------------------------------------------------------------------
    // Role separation — compile-time enforcement documentation
    // -----------------------------------------------------------------------

    #[test]
    fn test_role_type_separation_is_enforced_at_compile_time() {
        // TypeEntry.role is DataRole, TraitEntry.role is ContractRole,
        // FunctionEntry.role is FunctionRole. The following would be compile errors:
        //   let _: TypeEntry = TypeEntry { role: ContractRole::SecondaryPort, .. }; // ERROR
        //   let _: TraitEntry = TraitEntry { role: DataRole::ValueObject, .. };     // ERROR
        //
        // We verify at runtime that the types are distinct (they have different Display output).
        let type_role = DataRole::ValueObject;
        let trait_role = ContractRole::SpecificationPort;
        let fn_role = FunctionRole::FreeFunction;
        assert_ne!(type_role.to_string(), trait_role.to_string());
        assert_ne!(trait_role.to_string(), fn_role.to_string());
        assert_ne!(type_role.to_string(), fn_role.to_string());
    }
}
