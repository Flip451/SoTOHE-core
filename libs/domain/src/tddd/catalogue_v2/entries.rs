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
use crate::tddd::catalogue_v2::identifiers::{
    AssocConstName, ModulePath, RustExpression, TypeName, TypeRef,
};
use crate::tddd::catalogue_v2::methods::{
    MethodDeclaration, MethodGenericParam, ParamDeclaration, WherePredicateDecl,
};
// `MethodGenericParam` and `WherePredicateDecl` are used by `FunctionEntry`,
// `MethodDeclaration`, `InherentImplDeclV2`, and now also `TraitEntry`
// (ADR `2026-05-18-1223` D2 / IN-07).
use crate::tddd::catalogue_v2::roles::{ContractRole, DataRole, FunctionRole, ItemAction};

// ---------------------------------------------------------------------------
// AssocTypeDecl — associated type declaration in a trait
// ---------------------------------------------------------------------------

/// Declaration of an associated type item in a trait (e.g. `type Foo: Bound = Default`).
///
/// Used in [`TraitEntry::assoc_types`] to declare associated types so that the A-side
/// (catalogue) item count matches the C-side (rustdoc) item count.
///
/// ## Scope notes
///
/// - `bounds`: the trait bounds on the associated type, e.g. `["Send", "Sync"]` for
///   `type Foo: Send + Sync`. Empty when the associated type has no bounds.
/// - `default`: the default type for the associated type, if present.
///
/// No generic-params field is needed for the known GAT traits in this codebase:
/// `type Input<'a>` has only a lifetime parameter, and lifetime params are excluded
/// from the fingerprint comparison in `build_generics_fingerprint_with_combined_canon`
/// (only `GenericParamDefKind::Type` and `Const` are processed there). Therefore the
/// catalogue can declare `type Input` without any generic-params field and still match
/// the C-side's lifetime-excluded fingerprint `assoc_type[]:=`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssocTypeDecl {
    /// The name of the associated type (e.g. `Input` in `type Input`).
    ///
    /// Uses [`TypeName`] to make illegal names unrepresentable at the domain model level:
    /// an associated type name such as `Input` is a type-level identifier and reuses the
    /// same validated newtype as struct/enum names (prefer-type-safe-abstractions).
    pub name: TypeName,
    /// Trait bounds on the associated type (e.g. `["Send"]` for `type Foo: Send`).
    /// Empty Vec when the associated type has no bounds.
    pub bounds: Vec<TypeRef>,
    /// Optional default type for the associated type (e.g. `Some("Vec<u8>")` for
    /// `type Foo = Vec<u8>`). `None` when the associated type has no default.
    pub default: Option<TypeRef>,
}

// ---------------------------------------------------------------------------
// AssocConstDecl — associated constant declaration in a trait
// ---------------------------------------------------------------------------

/// Declaration of an associated constant item in a trait (e.g. `const ID: ChainId`).
///
/// Used in [`TraitEntry::assoc_consts`] to declare associated constants so that the
/// A-side (catalogue) item count matches the C-side (rustdoc) item count.
///
/// ## Field mapping to the signal evaluator's `assoc_const:{ty_str}={val_str}`
///
/// - `ty`: feeds `ty_str` via `format_type_with_canon`.
/// - `default_value`: feeds `val_str` via `apply_canon_to_str`; `None` becomes `""`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssocConstDecl {
    /// The name of the associated constant (e.g. `ID` in `const ID: ChainId`).
    ///
    /// Uses [`AssocConstName`] to make illegal names unrepresentable at the domain model
    /// level: a const name like `ID` has no other existing fitting newtype, so a dedicated
    /// validated newtype is introduced (prefer-type-safe-abstractions).
    pub name: AssocConstName,
    /// The type of the associated constant (e.g. `"ChainId"`).
    pub ty: TypeRef,
    /// Optional default value expression (e.g. `Some("42")` for `const N: usize = 42`).
    /// `None` when the constant has no default (common for trait-required constants).
    pub default_value: Option<RustExpression>,
}

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
    /// Associated types declared in this trait (e.g. `type Foo: Bound`).
    ///
    /// Default empty Vec for backward compatibility with all existing catalogues.
    /// When non-empty, the A-codec emits an `ItemEnum::AssocType` item for each entry
    /// so that `Trait.items.len()` matches the C-side (rustdoc) count and the
    /// structural comparison in `build_trait_method_map` finds matching entries.
    pub assoc_types: Vec<AssocTypeDecl>,
    /// Associated constants declared in this trait (e.g. `const ID: ChainId`).
    ///
    /// Default empty Vec for backward compatibility with all existing catalogues.
    /// When non-empty, the A-codec emits an `ItemEnum::AssocConst` item for each entry
    /// so that `Trait.items.len()` matches the C-side (rustdoc) count.
    pub assoc_consts: Vec<AssocConstDecl>,
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
    /// Trait-level generic type parameters (e.g. `[T]` for `trait Foo<T>`).
    ///
    /// Default empty Vec for backward compatibility with catalogues that predate this field.
    /// Reuses `MethodGenericParam` — no new type needed (ADR `2026-05-18-1223` D2 / IN-07).
    pub generics: Vec<MethodGenericParam>,
    /// Trait-level `where`-clause bound predicates (e.g. `[{ lhs: "T", rhs: ["Clone"] }]`
    /// for `trait Foo<T> where T: Clone`).
    ///
    /// Default empty Vec for backward compatibility.
    /// Reuses `WherePredicateDecl` — no new type needed (ADR `2026-05-18-1223` D2 / IN-07).
    pub where_predicates: Vec<WherePredicateDecl>,
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
// InherentImplDeclV2 — a single inherent impl block for a named type
// ---------------------------------------------------------------------------

/// A single inherent `impl` block for a named type (ADR D2, IN-05 / IN-08).
///
/// One struct may have multiple `impl` blocks in Rust source code. Each is
/// represented as a separate `InherentImplDeclV2` entry in
/// `CatalogueDocument::inherent_impls`. The `type_name` field identifies the
/// target struct; multiple entries sharing the same `type_name` represent
/// multiple impl blocks for that struct.
///
/// ## Scope
///
/// - `impl_generics`: type parameters only (lifetime / const parameters are out of scope).
/// - `impl_where_predicates`: where-clause predicates on the impl-block-level generics.
/// - `methods`: all methods declared in this impl block.
///
/// No serde derives — per ADR `knowledge/adr/2026-04-14-1531-domain-serde-ripout.md`,
/// the domain layer is serialization-free. The infrastructure codec handles JSON.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InherentImplDeclV2 {
    /// The name of the type this impl block belongs to.
    ///
    /// Multiple `InherentImplDeclV2` entries with the same `type_name` represent
    /// multiple inherent impl blocks for that single struct in the source.
    pub type_name: TypeName,

    /// Impl-block-level generic type parameters (type parameters only; lifetimes
    /// and const parameters are out of scope per D2 / IN-05).
    ///
    /// Empty Vec when the impl block is not generic (the common case).
    pub impl_generics: Vec<MethodGenericParam>,

    /// Impl-block-level where-clause predicates applied to `impl_generics`.
    ///
    /// Empty Vec when there are no impl-level where predicates.
    pub impl_where_predicates: Vec<WherePredicateDecl>,

    /// Method declarations inside this impl block.
    ///
    /// Empty Vec when the impl block contains no methods.
    pub methods: Vec<MethodDeclaration>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::tddd::catalogue_v2::composite::{StructKind, StructShape};
    use crate::tddd::catalogue_v2::identifiers::{
        CrateName, FieldName, MethodName, ModulePath, ParamName, RustExpression, TypeName, TypeRef,
    };
    use crate::tddd::catalogue_v2::roles::{NonEmptyVec, SelfReceiver};
    use crate::tddd::catalogue_v2::variants::FieldDecl;

    // -----------------------------------------------------------------------
    // TypeEntry
    // -----------------------------------------------------------------------

    #[test]
    fn test_type_entry_with_data_role_compiles() {
        // TypeEntry.role: DataRole — assigning ContractRole is a compile-time error.
        let entry = TypeEntry {
            action: ItemAction::Add,
            role: DataRole::value_object(),
            kind: TypeKindV2::Struct(StructKind::new(
                StructShape::Plain { fields: vec![], has_stripped_fields: false },
                None,
            )),
            methods: vec![],
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        };
        assert_eq!(entry.role, DataRole::value_object());
        assert_eq!(entry.action, ItemAction::Add);
    }

    #[test]
    fn test_type_entry_with_struct_kind_and_fields() {
        let field_name = FieldName::new("email").unwrap();
        let field_ty = TypeRef::new("String").unwrap();
        let fields = vec![FieldDecl::new(field_name, field_ty)];
        let entry = TypeEntry {
            action: ItemAction::Add,
            role: DataRole::entity().unwrap(),
            kind: TypeKindV2::Struct(StructKind::new(
                StructShape::Plain { fields: fields.clone(), has_stripped_fields: false },
                None,
            )),
            methods: vec![],
            module_path: ModulePath::root(),
            docs: Some("A domain entity.".to_string()),
            spec_refs: vec![],
            informal_grounds: vec![],
        };
        match &entry.kind {
            TypeKindV2::Struct(sk) => match &sk.shape {
                StructShape::Plain { fields: k_fields, has_stripped_fields } => {
                    assert!(!has_stripped_fields);
                    assert!(sk.typestate.is_none());
                    assert_eq!(k_fields.len(), 1);
                }
                _ => panic!("expected Plain shape"),
            },
            _ => panic!("expected Struct kind"),
        }
        assert_eq!(entry.docs, Some("A domain entity.".to_string()));
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
            role: DataRole::value_object(),
            kind: TypeKindV2::Struct(StructKind::new(
                StructShape::Tuple { fields: vec![field_ty], has_stripped_fields: false },
                None,
            )),
            methods: vec![method.clone()],
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
            role: DataRole::aggregate_root().unwrap(),
            kind: TypeKindV2::Struct(StructKind::new(
                StructShape::Plain { fields: vec![], has_stripped_fields: false },
                None,
            )),
            methods: vec![],
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
            DataRole::value_object(),
            DataRole::entity().unwrap(),
            DataRole::aggregate_root().unwrap(),
            DataRole::domain_service(),
            DataRole::Specification,
            DataRole::Factory,
            DataRole::use_case(),
            DataRole::Interactor,
            DataRole::Command,
            DataRole::Query,
            DataRole::Dto,
            DataRole::ErrorType,
            DataRole::SecondaryAdapter,
            DataRole::EventPolicy {
                reacts_to: NonEmptyVec::new(TypeRef::new("OrderPlaced").unwrap(), vec![]),
            }, // (kept multi-line for readability — line exceeds small-heuristics threshold)
            DataRole::DomainEvent,
        ];
        for role in roles {
            let entry = TypeEntry {
                action: ItemAction::Add,
                role: role.clone(),
                kind: TypeKindV2::Struct(StructKind::new(
                    StructShape::Plain { fields: vec![], has_stripped_fields: false },
                    None,
                )),
                methods: vec![],
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

    fn trait_entry_fixture() -> TraitEntry {
        TraitEntry {
            action: ItemAction::Add,
            role: ContractRole::SecondaryPort,
            methods: vec![],
            assoc_types: vec![],
            assoc_consts: vec![],
            supertrait_bounds: vec![],
            generics: vec![],
            where_predicates: vec![],
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        }
    }

    #[test]
    fn test_trait_entry_with_contract_role_compiles() {
        // TraitEntry.role: ContractRole — assigning DataRole is a compile-time error.
        let entry = trait_entry_fixture();
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
        let mut entry = trait_entry_fixture();
        entry.methods = vec![save_method.clone()];
        entry.docs = Some("User repository port.".to_string());
        assert_eq!(entry.methods.len(), 1);
        assert_eq!(entry.methods[0], save_method);
        assert_eq!(entry.docs, Some("User repository port.".to_string()));
    }

    #[test]
    fn test_trait_entry_with_supertrait_bounds() {
        let send = TypeRef::new("Send").unwrap();
        let sync = TypeRef::new("Sync").unwrap();
        let mut entry = trait_entry_fixture();
        entry.supertrait_bounds = vec![send.clone(), sync.clone()];
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
            ContractRole::Repository { aggregate: TypeRef::new("Order").unwrap() },
        ];
        for role in roles {
            let mut entry = trait_entry_fixture();
            entry.role = role.clone();
            assert_eq!(entry.role, role);
        }
    }

    #[test]
    fn test_trait_entry_new_has_empty_generics_by_default() {
        // AC-07: TraitEntry must carry a generics field defaulting to empty Vec.
        let entry = trait_entry_fixture();
        assert!(entry.generics.is_empty());
    }

    #[test]
    fn test_trait_entry_new_has_empty_where_predicates_by_default() {
        // AC-07: TraitEntry must carry a where_predicates field defaulting to empty Vec.
        let entry = trait_entry_fixture();
        assert!(entry.where_predicates.is_empty());
    }

    #[test]
    fn test_trait_entry_new_has_empty_assoc_items_by_default() {
        let entry = trait_entry_fixture();
        assert!(entry.assoc_types.is_empty());
        assert!(entry.assoc_consts.is_empty());
    }

    #[test]
    fn test_trait_entry_generics_and_where_predicates_for_generic_trait_decl() {
        // AC-07 primary: `trait Foo<T> where T: Clone` can be represented.
        use crate::tddd::catalogue_v2::methods::{BoundOp, WherePredicateDecl};
        let mut entry = trait_entry_fixture();
        entry.generics =
            vec![MethodGenericParam { name: ParamName::new("T").unwrap(), bounds: vec![] }];
        entry.where_predicates = vec![WherePredicateDecl {
            lhs: TypeRef::new("T").unwrap(),
            rhs: vec![TypeRef::new("Clone").unwrap()],
            operator: BoundOp::Bound,
        }];
        assert_eq!(entry.generics.len(), 1);
        assert_eq!(entry.generics[0].name.as_str(), "T");
        assert_eq!(entry.where_predicates.len(), 1);
        assert_eq!(entry.where_predicates[0].lhs.as_str(), "T");
        assert_eq!(entry.where_predicates[0].rhs[0].as_str(), "Clone");
    }

    #[test]
    fn test_trait_entry_generics_participates_in_equality() {
        // generics field must participate in PartialEq (derive-level guarantee).
        let base = trait_entry_fixture();
        let mut with_generic = base.clone();
        with_generic.generics =
            vec![MethodGenericParam { name: ParamName::new("T").unwrap(), bounds: vec![] }];
        assert_ne!(base, with_generic, "generics field must participate in equality");
    }

    #[test]
    fn test_trait_entry_where_predicates_participates_in_equality() {
        // where_predicates field must participate in PartialEq.
        use crate::tddd::catalogue_v2::methods::{BoundOp, WherePredicateDecl};
        let base = trait_entry_fixture();
        let mut with_pred = base.clone();
        with_pred.where_predicates = vec![WherePredicateDecl {
            lhs: TypeRef::new("T").unwrap(),
            rhs: vec![TypeRef::new("Clone").unwrap()],
            operator: BoundOp::Bound,
        }];
        assert_ne!(base, with_pred, "where_predicates field must participate in equality");
    }

    #[test]
    fn test_trait_entry_assoc_items_participate_in_equality() {
        let base = trait_entry_fixture();

        let mut with_assoc_type = base.clone();
        with_assoc_type.assoc_types = vec![AssocTypeDecl {
            name: TypeName::new("Input").unwrap(),
            bounds: vec![TypeRef::new("Send").unwrap()],
            default: Some(TypeRef::new("Vec<u8>").unwrap()),
        }];
        assert_ne!(base, with_assoc_type, "assoc_types field must participate in equality");

        let mut with_assoc_const = base.clone();
        with_assoc_const.assoc_consts = vec![AssocConstDecl {
            name: AssocConstName::new("CHAIN_ID").unwrap(),
            ty: TypeRef::new("ChainId").unwrap(),
            default_value: Some(RustExpression::try_new("DEFAULT_CHAIN_ID").unwrap()),
        }];
        assert_ne!(base, with_assoc_const, "assoc_consts field must participate in equality");
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
        // ADR 2026-05-18-1223 D1: FunctionEntry carries explicit where_predicates so
        // catalogue authors can express constraints whose LHS is a type expression
        // (e.g. `where Vec<T>: Bound`) that the inline form cannot represent.
        use crate::tddd::catalogue_v2::methods::BoundOp;
        let entry = FunctionEntry {
            action: ItemAction::Add,
            role: FunctionRole::FreeFunction,
            params: vec![],
            returns: TypeRef::new("()").unwrap(),
            is_async: false,
            generics: vec![],
            where_predicates: vec![WherePredicateDecl {
                lhs: TypeRef::new("Vec<T>").unwrap(),
                rhs: vec![TypeRef::new("Send").unwrap()],
                operator: BoundOp::Bound,
            }],
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        };
        assert_eq!(entry.where_predicates.len(), 1);
        assert_eq!(entry.where_predicates[0].lhs.as_str(), "Vec<T>");
        assert_eq!(entry.where_predicates[0].rhs[0].as_str(), "Send");
    }

    #[test]
    fn test_function_entry_where_predicates_distinguish_otherwise_equal_entries() {
        use crate::tddd::catalogue_v2::methods::BoundOp;
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
            lhs: TypeRef::new("T").unwrap(),
            rhs: vec![TypeRef::new("Clone").unwrap()],
            operator: BoundOp::Bound,
        }];
        assert_ne!(base, with_where, "where_predicates field participates in equality");
    }

    // -----------------------------------------------------------------------
    // Grounding fields (T010) — spec_refs and informal_grounds
    // -----------------------------------------------------------------------

    #[test]
    fn test_type_entry_with_non_empty_spec_refs_stores_grounding() {
        use crate::plan_ref::{SpecElementId, SpecRef};
        use std::path::PathBuf;

        let anchor = SpecElementId::try_new("IN-01").unwrap();
        let spec_ref = SpecRef::new(PathBuf::from("track/items/x/spec.json"), anchor);

        let entry = TypeEntry {
            action: ItemAction::Add,
            role: DataRole::value_object(),
            kind: TypeKindV2::Struct(StructKind::new(
                StructShape::Plain { fields: vec![], has_stripped_fields: false },
                None,
            )),
            methods: vec![],
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

        let mut entry = trait_entry_fixture();
        entry.informal_grounds = vec![ground.clone()];
        assert_eq!(entry.informal_grounds.len(), 1);
        assert_eq!(entry.informal_grounds[0], ground);
        assert!(entry.spec_refs.is_empty());
    }

    #[test]
    fn test_function_entry_with_spec_refs_and_informal_grounds_stores_both() {
        use crate::plan_ref::{
            InformalGroundKind, InformalGroundRef, InformalGroundSummary, SpecElementId, SpecRef,
        };
        use std::path::PathBuf;

        let anchor = SpecElementId::try_new("AC-02").unwrap();
        let spec_ref = SpecRef::new(PathBuf::from("track/items/x/spec.json"), anchor);

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
    // InherentImplDeclV2
    // -----------------------------------------------------------------------

    #[test]
    fn test_inherent_impl_decl_v2_one_struct_multiple_impl_blocks() {
        // Verifies the primary design constraint: 1 struct can have N impl blocks
        // represented as N separate `InherentImplDeclV2` entries in the Vec.
        let type_name = TypeName::new("Email").unwrap();

        let method_a = MethodDeclaration::new(
            MethodName::new("as_str").unwrap(),
            Some(SelfReceiver::SharedRef),
            vec![],
            TypeRef::new("str").unwrap(),
            false,
            None,
        );
        let method_b = MethodDeclaration::new(
            MethodName::new("validate").unwrap(),
            Some(SelfReceiver::SharedRef),
            vec![],
            TypeRef::new("Result<(), DomainError>").unwrap(),
            false,
            None,
        );

        let impl_block_a = InherentImplDeclV2 {
            type_name: type_name.clone(),
            impl_generics: vec![],
            impl_where_predicates: vec![],
            methods: vec![method_a.clone()],
        };
        let impl_block_b = InherentImplDeclV2 {
            type_name: type_name.clone(),
            impl_generics: vec![],
            impl_where_predicates: vec![],
            methods: vec![method_b.clone()],
        };

        // Both blocks share the same type_name, representing two inherent impl blocks
        // for `Email` in the source code.
        assert_eq!(impl_block_a.type_name, type_name);
        assert_eq!(impl_block_b.type_name, type_name);
        assert_eq!(impl_block_a.methods.len(), 1);
        assert_eq!(impl_block_b.methods.len(), 1);
        assert_eq!(impl_block_a.methods[0].name.as_str(), "as_str");
        assert_eq!(impl_block_b.methods[0].name.as_str(), "validate");

        // A Vec of two entries represents the two impl blocks for one struct.
        let inherent_impls = [impl_block_a, impl_block_b];
        assert_eq!(inherent_impls.len(), 2);
        assert_eq!(inherent_impls[0].type_name, inherent_impls[1].type_name);
    }

    #[test]
    fn test_inherent_impl_decl_v2_with_generics_and_where_predicates() {
        use crate::tddd::catalogue_v2::methods::{BoundOp, WherePredicateDecl};

        let type_name = TypeName::new("Container").unwrap();
        let generic_param = MethodGenericParam {
            name: ParamName::new("T").unwrap(),
            bounds: vec![TypeRef::new("Clone").unwrap()],
        };
        let where_pred = WherePredicateDecl {
            lhs: TypeRef::new("Vec<T>").unwrap(),
            rhs: vec![TypeRef::new("Send").unwrap()],
            operator: BoundOp::Bound,
        };
        let impl_block = InherentImplDeclV2 {
            type_name: type_name.clone(),
            impl_generics: vec![generic_param],
            impl_where_predicates: vec![where_pred],
            methods: vec![],
        };

        assert_eq!(impl_block.type_name, type_name);
        assert_eq!(impl_block.impl_generics.len(), 1);
        assert_eq!(impl_block.impl_generics[0].name.as_str(), "T");
        assert_eq!(impl_block.impl_where_predicates.len(), 1);
        assert_eq!(impl_block.impl_where_predicates[0].lhs.as_str(), "Vec<T>");
        assert!(impl_block.methods.is_empty());
    }

    #[test]
    fn test_inherent_impl_decl_v2_default_fields_are_empty_vecs() {
        let type_name = TypeName::new("Foo").unwrap();
        let impl_block = InherentImplDeclV2 {
            type_name: type_name.clone(),
            impl_generics: vec![],
            impl_where_predicates: vec![],
            methods: vec![],
        };
        assert!(impl_block.impl_generics.is_empty());
        assert!(impl_block.impl_where_predicates.is_empty());
        assert!(impl_block.methods.is_empty());
    }

    #[test]
    fn test_inherent_impl_decl_v2_equality_by_all_fields() {
        let type_name = TypeName::new("Foo").unwrap();
        let a = InherentImplDeclV2 {
            type_name: type_name.clone(),
            impl_generics: vec![],
            impl_where_predicates: vec![],
            methods: vec![],
        };
        let b = a.clone();
        assert_eq!(a, b);
    }

    // -----------------------------------------------------------------------
    // CatalogueDocument.inherent_impls
    // -----------------------------------------------------------------------

    #[test]
    fn test_catalogue_document_inherent_impls_defaults_to_empty() {
        use crate::tddd::catalogue_v2::document::CatalogueDocument;
        use crate::tddd::layer_id::LayerId;

        let crate_name = CrateName::new("domain").unwrap();
        let layer = LayerId::try_new("domain").unwrap();
        let doc = CatalogueDocument::new(3, crate_name, layer);
        assert!(doc.inherent_impls.is_empty());
    }

    #[test]
    fn test_catalogue_document_inherent_impls_stores_multiple_entries_for_one_type() {
        use crate::tddd::catalogue_v2::document::CatalogueDocument;
        use crate::tddd::layer_id::LayerId;

        let crate_name = CrateName::new("domain").unwrap();
        let layer = LayerId::try_new("domain").unwrap();
        let mut doc = CatalogueDocument::new(3, crate_name, layer);

        let type_name = TypeName::new("Email").unwrap();
        doc.inherent_impls.push(InherentImplDeclV2 {
            type_name: type_name.clone(),
            impl_generics: vec![],
            impl_where_predicates: vec![],
            methods: vec![],
        });
        doc.inherent_impls.push(InherentImplDeclV2 {
            type_name: type_name.clone(),
            impl_generics: vec![],
            impl_where_predicates: vec![],
            methods: vec![],
        });

        assert_eq!(doc.inherent_impls.len(), 2);
        assert_eq!(doc.inherent_impls[0].type_name, type_name);
        assert_eq!(doc.inherent_impls[1].type_name, type_name);
    }

    // -----------------------------------------------------------------------
    // Role separation — compile-time enforcement documentation
    // -----------------------------------------------------------------------

    #[test]
    fn test_role_type_separation_is_enforced_at_compile_time() {
        // TypeEntry.role is DataRole, TraitEntry.role is ContractRole,
        // FunctionEntry.role is FunctionRole. The following would be compile errors:
        //   let _: TypeEntry = TypeEntry { role: ContractRole::SecondaryPort, .. }; // ERROR
        //   let _: TraitEntry = TraitEntry { role: DataRole::value_object(), .. };     // ERROR
        //
        // We verify at runtime that the types are distinct (they have different Display output).
        let type_role = DataRole::value_object();
        let trait_role = ContractRole::SpecificationPort;
        let fn_role = FunctionRole::FreeFunction;
        assert_ne!(type_role.to_string(), trait_role.to_string());
        assert_ne!(trait_role.to_string(), fn_role.to_string());
        assert_ne!(type_role.to_string(), fn_role.to_string());
    }
}
