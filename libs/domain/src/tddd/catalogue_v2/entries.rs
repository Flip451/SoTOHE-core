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

use crate::plan_ref::{InformalGroundRef, SpecRef};
use crate::tddd::catalogue_v2::composite::TypeKindV2;
use crate::tddd::catalogue_v2::identifiers::{ModulePath, TypeRef};
use crate::tddd::catalogue_v2::methods::{
    MethodDeclaration, MethodGenericParam, ParamDeclaration, WherePredicateDecl,
};
// Note: `WherePredicateDecl` is used by `FunctionEntry.where_predicates` and
// `MethodDeclaration.where_predicates`. `TraitEntry` does not currently carry
// where-predicates (ADR `2026-05-13-1153` IN-30 scope: trait-level generic
// parameter declarations are not yet schematized).
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
    /// SoT Chain ② references to spec.json elements.
    /// Empty vec when no spec elements have been linked yet.
    pub spec_refs: Vec<SpecRef>,
    /// Informal ground citations (unpersisted rationale). Non-empty → 🟡 advisory signal.
    /// Empty vec when no informal grounds have been recorded.
    pub informal_grounds: Vec<InformalGroundRef>,
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
    /// Supertrait bounds for this trait (e.g. `[Send, Sync]` for `trait Foo: Send + Sync`).
    ///
    /// Default empty Vec for backward compatibility. When non-empty, the A-codec encodes
    /// these as `GenericBound::TraitBound` entries in `Trait::bounds`, mirroring the
    /// rustdoc C-side representation.
    ///
    /// Using `TypeRef` instead of `String` makes empty-bound entries unrepresentable:
    /// `TypeRef::new` rejects empty strings at construction time, so any stored bound is
    /// guaranteed to be a non-empty type/trait reference string.
    pub supertrait_bounds: Vec<TypeRef>,
    /// Module path within the crate (empty = crate root). Serde default = empty.
    pub module_path: ModulePath,
    /// Optional documentation string.
    pub docs: Option<String>,
    /// SoT Chain ② references to spec.json elements.
    /// Empty vec when no spec elements have been linked yet.
    pub spec_refs: Vec<SpecRef>,
    /// Informal ground citations (unpersisted rationale). Non-empty → 🟡 advisory signal.
    /// Empty vec when no informal grounds have been recorded.
    pub informal_grounds: Vec<InformalGroundRef>,
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
    /// Generic type parameters on this function.
    ///
    /// Populated when the function is declared with APIT (`impl Trait`) or an
    /// explicit generic parameter (`fn f<T: Bound>(...)`). Default empty Vec for
    /// backward compatibility. The A-codec encodes these as `GenericParamDef::Type`
    /// entries in the function's `Generics`, mirroring `MethodDeclaration.generics`.
    ///
    /// (ADR `2026-05-08-0248` D14)
    pub generics: Vec<MethodGenericParam>,
    /// `where`-clause bound predicates on this function's generics.
    ///
    /// Captures `BoundPredicate` entries whose LHS is an arbitrary type
    /// expression — patterns `generics[].bounds` (single-identifier LHS)
    /// cannot represent (e.g. `where Vec<T>: Clone`). Default empty Vec.
    ///
    /// (ADR `2026-05-13-1153-tddd-where-form-generics-normalization` D1, D2)
    pub where_predicates: Vec<WherePredicateDecl>,
    /// Optional documentation string.
    pub docs: Option<String>,
    /// SoT Chain ② references to spec.json elements.
    /// Empty vec when no spec elements have been linked yet.
    pub spec_refs: Vec<SpecRef>,
    /// Informal ground citations (unpersisted rationale). Non-empty → 🟡 advisory signal.
    /// Empty vec when no informal grounds have been recorded.
    pub informal_grounds: Vec<InformalGroundRef>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;
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
            kind: TypeKindV2::PlainStruct {
                fields: vec![],
                has_stripped_fields: false,
                typestate: None,
            },
            methods: vec![],
            trait_impls: vec![],
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
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
            kind: TypeKindV2::PlainStruct {
                fields: fields.clone(),
                has_stripped_fields: false,
                typestate: None,
            },
            methods: vec![],
            trait_impls: vec![],
            module_path: ModulePath::root(),
            docs: Some("A domain entity.".to_string()),
            spec_refs: vec![],
            informal_grounds: vec![],
        };
        match &entry.kind {
            TypeKindV2::PlainStruct { fields: k_fields, has_stripped_fields, typestate } => {
                assert!(!has_stripped_fields);
                assert!(typestate.is_none());
                assert_eq!(k_fields.len(), 1);
            }
            _ => panic!("expected PlainStruct kind"),
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
            spec_refs: vec![],
            informal_grounds: vec![],
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
        let field_ty = TypeRef::new("String").unwrap();
        let entry = TypeEntry {
            action: ItemAction::Add,
            role: DataRole::ValueObject,
            kind: TypeKindV2::TupleStruct { fields: vec![field_ty], has_stripped_fields: false },
            methods: vec![method.clone()],
            trait_impls: vec![],
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
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
            kind: TypeKindV2::PlainStruct {
                fields: vec![],
                has_stripped_fields: false,
                typestate: None,
            },
            methods: vec![],
            trait_impls: vec![],
            module_path: module_path.clone(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
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
                kind: TypeKindV2::PlainStruct {
                    fields: vec![],
                    has_stripped_fields: false,
                    typestate: None,
                },
                methods: vec![],
                trait_impls: vec![],
                module_path: ModulePath::root(),
                docs: None,
                spec_refs: vec![],
                informal_grounds: vec![],
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
            supertrait_bounds: vec![],
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
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
            supertrait_bounds: vec![],
            module_path: ModulePath::root(),
            docs: Some("User repository port.".to_string()),
            spec_refs: vec![],
            informal_grounds: vec![],
        };
        assert_eq!(entry.methods.len(), 1);
        assert_eq!(entry.methods[0], save_method);
        assert_eq!(entry.docs, Some("User repository port.".to_string()));
    }

    #[test]
    fn test_trait_entry_with_supertrait_bounds() {
        let send = TypeRef::new("Send").unwrap();
        let sync = TypeRef::new("Sync").unwrap();
        let entry = TraitEntry {
            action: ItemAction::Add,
            role: ContractRole::SecondaryPort,
            methods: vec![],
            supertrait_bounds: vec![send.clone(), sync.clone()],
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        };
        assert_eq!(entry.supertrait_bounds.len(), 2);
        assert_eq!(entry.supertrait_bounds[0].as_str(), "Send");
        assert_eq!(entry.supertrait_bounds[1].as_str(), "Sync");
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
                supertrait_bounds: vec![],
                module_path: ModulePath::root(),
                docs: None,
                spec_refs: vec![],
                informal_grounds: vec![],
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
            generics: vec![],
            where_predicates: vec![],
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
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
            generics: vec![],
            where_predicates: vec![],
            docs: Some("Register a new user.".to_string()),
            spec_refs: vec![],
            informal_grounds: vec![],
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
                generics: vec![],
                where_predicates: vec![],
                docs: None,
                spec_refs: vec![],
                informal_grounds: vec![],
            };
            assert_eq!(entry.role, role);
        }
    }

    #[test]
    fn test_function_entry_with_generics_stores_them() {
        // ADR 2026-05-08-0248 D14: FunctionEntry carries explicit generic params
        // so the A-codec can mirror rustdoc's `Function.generics`.
        use crate::tddd::catalogue_v2::methods::MethodGenericParam;
        let entry = FunctionEntry {
            action: ItemAction::Add,
            role: FunctionRole::FreeFunction,
            params: vec![],
            returns: TypeRef::new("T").unwrap(),
            is_async: false,
            generics: vec![MethodGenericParam {
                name: ParamName::new("T").unwrap(),
                bounds: vec![TypeRef::new("Clone").unwrap()],
            }],
            where_predicates: vec![],
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        };
        assert_eq!(entry.generics.len(), 1);
        assert_eq!(entry.generics[0].name.as_str(), "T");
        assert_eq!(entry.generics[0].bounds[0].as_str(), "Clone");
    }

    #[test]
    fn test_function_entry_default_generics_is_empty() {
        let entry = FunctionEntry {
            action: ItemAction::Add,
            role: FunctionRole::FreeFunction,
            params: vec![],
            returns: TypeRef::new("()").unwrap(),
            is_async: false,
            generics: vec![],
            where_predicates: vec![],
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        };
        assert!(entry.generics.is_empty());
    }

    #[test]
    fn test_function_entry_generics_distinguishes_otherwise_equal_entries() {
        use crate::tddd::catalogue_v2::methods::MethodGenericParam;
        let base = FunctionEntry {
            action: ItemAction::Add,
            role: FunctionRole::FreeFunction,
            params: vec![],
            returns: TypeRef::new("()").unwrap(),
            is_async: false,
            generics: vec![],
            where_predicates: vec![],
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        };
        let mut with_generic = base.clone();
        with_generic.generics =
            vec![MethodGenericParam { name: ParamName::new("T").unwrap(), bounds: vec![] }];
        assert_ne!(base, with_generic, "generics field participates in equality");
    }

    #[test]
    fn test_function_entry_with_where_predicates_stores_them() {
        // ADR 2026-05-13-1153 D2: FunctionEntry carries explicit where_predicates so
        // catalogue authors can express constraints whose LHS is a type expression
        // (e.g. `where Vec<T>: Bound`) that the inline form cannot represent.
        let entry = FunctionEntry {
            action: ItemAction::Add,
            role: FunctionRole::FreeFunction,
            params: vec![],
            returns: TypeRef::new("()").unwrap(),
            is_async: false,
            generics: vec![],
            where_predicates: vec![WherePredicateDecl {
                type_: TypeRef::new("Vec<T>").unwrap(),
                bounds: vec![TypeRef::new("Send").unwrap()],
            }],
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        };
        assert_eq!(entry.where_predicates.len(), 1);
        assert_eq!(entry.where_predicates[0].type_.as_str(), "Vec<T>");
        assert_eq!(entry.where_predicates[0].bounds[0].as_str(), "Send");
    }

    #[test]
    fn test_function_entry_where_predicates_distinguish_otherwise_equal_entries() {
        let base = FunctionEntry {
            action: ItemAction::Add,
            role: FunctionRole::FreeFunction,
            params: vec![],
            returns: TypeRef::new("()").unwrap(),
            is_async: false,
            generics: vec![],
            where_predicates: vec![],
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        };
        let mut with_where = base.clone();
        with_where.where_predicates = vec![WherePredicateDecl {
            type_: TypeRef::new("T").unwrap(),
            bounds: vec![TypeRef::new("Clone").unwrap()],
        }];
        assert_ne!(base, with_where, "where_predicates field participates in equality");
    }

    // -----------------------------------------------------------------------
    // Grounding fields (T010) — spec_refs and informal_grounds
    // -----------------------------------------------------------------------

    #[test]
    fn test_type_entry_with_non_empty_spec_refs_stores_grounding() {
        use crate::plan_ref::{ContentHash, SpecElementId, SpecRef};
        use std::path::PathBuf;

        let anchor = SpecElementId::try_new("IN-01").unwrap();
        let hash = ContentHash::from_bytes([0u8; 32]);
        let spec_ref = SpecRef::new(PathBuf::from("track/items/x/spec.json"), anchor, hash);

        let entry = TypeEntry {
            action: ItemAction::Add,
            role: DataRole::ValueObject,
            kind: TypeKindV2::PlainStruct {
                fields: vec![],
                has_stripped_fields: false,
                typestate: None,
            },
            methods: vec![],
            trait_impls: vec![],
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![spec_ref.clone()],
            informal_grounds: vec![],
        };
        assert_eq!(entry.spec_refs.len(), 1);
        assert_eq!(entry.spec_refs[0], spec_ref);
        assert!(entry.informal_grounds.is_empty());
    }

    #[test]
    fn test_trait_entry_with_non_empty_informal_grounds_stores_grounding() {
        use crate::plan_ref::{InformalGroundKind, InformalGroundRef, InformalGroundSummary};

        let summary = InformalGroundSummary::try_new("discussed in planning session").unwrap();
        let ground = InformalGroundRef::new(InformalGroundKind::Discussion, summary);

        let entry = TraitEntry {
            action: ItemAction::Add,
            role: ContractRole::SecondaryPort,
            methods: vec![],
            supertrait_bounds: vec![],
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![ground.clone()],
        };
        assert_eq!(entry.informal_grounds.len(), 1);
        assert_eq!(entry.informal_grounds[0], ground);
        assert!(entry.spec_refs.is_empty());
    }

    #[test]
    fn test_function_entry_with_spec_refs_and_informal_grounds_stores_both() {
        use crate::plan_ref::{
            ContentHash, InformalGroundKind, InformalGroundRef, InformalGroundSummary,
            SpecElementId, SpecRef,
        };
        use std::path::PathBuf;

        let anchor = SpecElementId::try_new("AC-02").unwrap();
        let hash = ContentHash::from_bytes([0xabu8; 32]);
        let spec_ref = SpecRef::new(PathBuf::from("track/items/x/spec.json"), anchor, hash);

        let summary = InformalGroundSummary::try_new("user directive from session").unwrap();
        let ground = InformalGroundRef::new(InformalGroundKind::UserDirective, summary);

        let entry = FunctionEntry {
            action: ItemAction::Add,
            role: FunctionRole::FreeFunction,
            params: vec![],
            returns: TypeRef::new("()").unwrap(),
            is_async: false,
            generics: vec![],
            where_predicates: vec![],
            docs: None,
            spec_refs: vec![spec_ref.clone()],
            informal_grounds: vec![ground.clone()],
        };
        assert_eq!(entry.spec_refs.len(), 1);
        assert_eq!(entry.spec_refs[0], spec_ref);
        assert_eq!(entry.informal_grounds.len(), 1);
        assert_eq!(entry.informal_grounds[0], ground);
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
