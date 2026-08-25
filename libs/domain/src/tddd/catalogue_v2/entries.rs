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
    AssocConstName, DocString, ModulePath, RustExpression, TypeName, TypeRef,
};
use crate::tddd::catalogue_v2::methods::{
    MethodDeclaration, MethodGenericParam, ParamDeclaration, WherePredicateDecl,
};
// `MethodGenericParam` and `WherePredicateDecl` are used by `FunctionEntry`,
// `MethodDeclaration`, `InherentImplDeclV2`, and now also `TraitEntry`
// (ADR `2026-05-18-1223` D2 / IN-07).
use crate::tddd::catalogue_v2::roles::{ContractRole, DataRole, FunctionRole, ItemAction};
use crate::tddd::semantic_verify::CatalogueEntryKey;

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
    action: ItemAction,
    /// The DDD / Clean Architecture role of this type. Only `DataRole` is accepted.
    role: DataRole,
    /// The language-level kind (Struct / Enum / TypeAlias) with payload-encoded pattern.
    kind: TypeKindV2,
    /// Inherent methods declared on this type.
    methods: Vec<MethodDeclaration>,
    /// Type-declaration-level generic type parameters (e.g. `[T]` for `struct Foo<T>`).
    ///
    /// Default empty Vec for catalogues that predate this field. Reuses
    /// `MethodGenericParam` (ADR `2026-07-02-1345` D6 / IN-13).
    generics: Vec<MethodGenericParam>,
    /// Type-declaration-level `where`-clause bound predicates
    /// (e.g. `[{ lhs: "T", rhs: ["Clone"] }]` for `struct Foo<T> where T: Clone`).
    ///
    /// Default empty Vec. Reuses `WherePredicateDecl` (ADR `2026-07-02-1345` D6 / IN-13).
    where_predicates: Vec<WherePredicateDecl>,
    /// Module path within the crate (empty = crate root). Serde default = empty.
    module_path: ModulePath,
    /// Optional documentation string.
    docs: Option<DocString>,
    /// SoT Chain ② references to spec.json elements.
    /// Empty vec when no spec elements have been linked yet.
    spec_refs: Vec<SpecRef>,
    /// Informal ground citations (unpersisted rationale). Non-empty → 🟡 advisory signal.
    /// Empty vec when no informal grounds have been recorded.
    informal_grounds: Vec<InformalGroundRef>,
}

impl TypeEntry {
    /// Creates a `TypeEntry` from all fields.
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        action: ItemAction,
        role: DataRole,
        kind: TypeKindV2,
        methods: Vec<MethodDeclaration>,
        generics: Vec<MethodGenericParam>,
        where_predicates: Vec<WherePredicateDecl>,
        module_path: ModulePath,
        docs: Option<DocString>,
        spec_refs: Vec<SpecRef>,
        informal_grounds: Vec<InformalGroundRef>,
    ) -> Self {
        Self {
            action,
            role,
            kind,
            methods,
            generics,
            where_predicates,
            module_path,
            docs,
            spec_refs,
            informal_grounds,
        }
    }

    /// The entry action (Add / Modify / Reference / Delete).
    #[must_use]
    pub fn action(&self) -> ItemAction {
        self.action
    }

    /// The DDD / Clean Architecture role of this type.
    #[must_use]
    pub fn role(&self) -> &DataRole {
        &self.role
    }

    /// The language-level kind (Struct / Enum / TypeAlias).
    #[must_use]
    pub fn kind(&self) -> &TypeKindV2 {
        &self.kind
    }

    /// Inherent methods declared on this type.
    #[must_use]
    pub fn methods(&self) -> &[MethodDeclaration] {
        &self.methods
    }

    /// Type-declaration-level generic type parameters.
    #[must_use]
    pub fn generics(&self) -> &[MethodGenericParam] {
        &self.generics
    }

    /// Type-declaration-level `where`-clause bound predicates.
    #[must_use]
    pub fn where_predicates(&self) -> &[WherePredicateDecl] {
        &self.where_predicates
    }

    /// Module path within the crate (empty = crate root).
    #[must_use]
    pub fn module_path(&self) -> &ModulePath {
        &self.module_path
    }

    /// Optional documentation string.
    #[must_use]
    pub fn docs(&self) -> Option<&DocString> {
        self.docs.as_ref()
    }

    /// SoT Chain ② references to spec.json elements.
    #[must_use]
    pub fn spec_refs(&self) -> &[SpecRef] {
        &self.spec_refs
    }

    /// Informal ground citations (unpersisted rationale).
    #[must_use]
    pub fn informal_grounds(&self) -> &[InformalGroundRef] {
        &self.informal_grounds
    }
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
    action: ItemAction,
    /// The architectural role of this trait. Only `ContractRole` is accepted.
    role: ContractRole,
    /// Methods declared in this trait.
    methods: Vec<MethodDeclaration>,
    /// Associated types declared in this trait (e.g. `type Foo: Bound`).
    ///
    /// Default empty Vec for backward compatibility with all existing catalogues.
    /// When non-empty, the A-codec emits an `ItemEnum::AssocType` item for each entry
    /// so that `Trait.items.len()` matches the C-side (rustdoc) count and the
    /// structural comparison in `build_trait_method_map` finds matching entries.
    assoc_types: Vec<AssocTypeDecl>,
    /// Associated constants declared in this trait (e.g. `const ID: ChainId`).
    ///
    /// Default empty Vec for backward compatibility with all existing catalogues.
    /// When non-empty, the A-codec emits an `ItemEnum::AssocConst` item for each entry
    /// so that `Trait.items.len()` matches the C-side (rustdoc) count.
    assoc_consts: Vec<AssocConstDecl>,
    /// Supertrait bounds for this trait (e.g. `[Send, Sync]` for `trait Foo: Send + Sync`).
    ///
    /// Default empty Vec for backward compatibility. When non-empty, the A-codec encodes
    /// these as `GenericBound::TraitBound` entries in `Trait::bounds`, mirroring the
    /// rustdoc C-side representation.
    ///
    /// Using `TypeRef` instead of `String` makes empty-bound entries unrepresentable:
    /// `TypeRef::new` rejects empty strings at construction time, so any stored bound is
    /// guaranteed to be a non-empty type/trait reference string.
    supertrait_bounds: Vec<TypeRef>,
    /// Trait-level generic type parameters (e.g. `[T]` for `trait Foo<T>`).
    ///
    /// Default empty Vec for backward compatibility with catalogues that predate this field.
    /// Reuses `MethodGenericParam` — no new type needed (ADR `2026-05-18-1223` D2 / IN-07).
    generics: Vec<MethodGenericParam>,
    /// Trait-level `where`-clause bound predicates (e.g. `[{ lhs: "T", rhs: ["Clone"] }]`
    /// for `trait Foo<T> where T: Clone`).
    ///
    /// Default empty Vec for backward compatibility.
    /// Reuses `WherePredicateDecl` — no new type needed (ADR `2026-05-18-1223` D2 / IN-07).
    where_predicates: Vec<WherePredicateDecl>,
    /// Module path within the crate (empty = crate root). Serde default = empty.
    module_path: ModulePath,
    /// Optional documentation string.
    docs: Option<DocString>,
    /// SoT Chain ② references to spec.json elements.
    /// Empty vec when no spec elements have been linked yet.
    spec_refs: Vec<SpecRef>,
    /// Informal ground citations (unpersisted rationale). Non-empty → 🟡 advisory signal.
    /// Empty vec when no informal grounds have been recorded.
    informal_grounds: Vec<InformalGroundRef>,
}

impl TraitEntry {
    /// Creates a `TraitEntry` from all fields.
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        action: ItemAction,
        role: ContractRole,
        methods: Vec<MethodDeclaration>,
        assoc_types: Vec<AssocTypeDecl>,
        assoc_consts: Vec<AssocConstDecl>,
        supertrait_bounds: Vec<TypeRef>,
        generics: Vec<MethodGenericParam>,
        where_predicates: Vec<WherePredicateDecl>,
        module_path: ModulePath,
        docs: Option<DocString>,
        spec_refs: Vec<SpecRef>,
        informal_grounds: Vec<InformalGroundRef>,
    ) -> Self {
        Self {
            action,
            role,
            methods,
            assoc_types,
            assoc_consts,
            supertrait_bounds,
            generics,
            where_predicates,
            module_path,
            docs,
            spec_refs,
            informal_grounds,
        }
    }

    /// The entry action (Add / Modify / Reference / Delete).
    #[must_use]
    pub fn action(&self) -> ItemAction {
        self.action
    }

    /// The architectural role of this trait.
    #[must_use]
    pub fn role(&self) -> &ContractRole {
        &self.role
    }

    /// Methods declared in this trait.
    #[must_use]
    pub fn methods(&self) -> &[MethodDeclaration] {
        &self.methods
    }

    /// Associated types declared in this trait.
    #[must_use]
    pub fn assoc_types(&self) -> &[AssocTypeDecl] {
        &self.assoc_types
    }

    /// Associated constants declared in this trait.
    #[must_use]
    pub fn assoc_consts(&self) -> &[AssocConstDecl] {
        &self.assoc_consts
    }

    /// Supertrait bounds for this trait.
    #[must_use]
    pub fn supertrait_bounds(&self) -> &[TypeRef] {
        &self.supertrait_bounds
    }

    /// Trait-level generic type parameters.
    #[must_use]
    pub fn generics(&self) -> &[MethodGenericParam] {
        &self.generics
    }

    /// Trait-level `where`-clause bound predicates.
    #[must_use]
    pub fn where_predicates(&self) -> &[WherePredicateDecl] {
        &self.where_predicates
    }

    /// Module path within the crate (empty = crate root).
    #[must_use]
    pub fn module_path(&self) -> &ModulePath {
        &self.module_path
    }

    /// Optional documentation string.
    #[must_use]
    pub fn docs(&self) -> Option<&DocString> {
        self.docs.as_ref()
    }

    /// SoT Chain ② references to spec.json elements.
    #[must_use]
    pub fn spec_refs(&self) -> &[SpecRef] {
        &self.spec_refs
    }

    /// Informal ground citations (unpersisted rationale).
    #[must_use]
    pub fn informal_grounds(&self) -> &[InformalGroundRef] {
        &self.informal_grounds
    }
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
    action: ItemAction,
    /// The architectural role of this function. Only `FunctionRole` is accepted.
    role: FunctionRole,
    /// The function parameters.
    params: Vec<ParamDeclaration>,
    /// The return type (generics-inclusive type reference string).
    returns: TypeRef,
    /// Whether this function is `async`.
    is_async: bool,
    /// Generic type parameters on this function.
    ///
    /// Populated when the function is declared with APIT (`impl Trait`) or an
    /// explicit generic parameter (`fn f<T: Bound>(...)`). Default empty Vec for
    /// backward compatibility. The A-codec encodes these as `GenericParamDef::Type`
    /// entries in the function's `Generics`, mirroring `MethodDeclaration.generics`.
    ///
    /// (ADR `2026-05-08-0248` D14)
    generics: Vec<MethodGenericParam>,
    /// `where`-clause bound predicates on this function's generics.
    ///
    /// Captures `BoundPredicate` entries whose LHS is an arbitrary type
    /// expression — patterns `generics[].bounds` (single-identifier LHS)
    /// cannot represent (e.g. `where Vec<T>: Clone`). Default empty Vec.
    ///
    /// (ADR `2026-05-13-1153-tddd-where-form-generics-normalization` D1, D2)
    where_predicates: Vec<WherePredicateDecl>,
    /// Optional documentation string.
    docs: Option<DocString>,
    /// SoT Chain ② references to spec.json elements.
    /// Empty vec when no spec elements have been linked yet.
    spec_refs: Vec<SpecRef>,
    /// Informal ground citations (unpersisted rationale). Non-empty → 🟡 advisory signal.
    /// Empty vec when no informal grounds have been recorded.
    informal_grounds: Vec<InformalGroundRef>,
}

impl FunctionEntry {
    /// Creates a `FunctionEntry` from all fields.
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        action: ItemAction,
        role: FunctionRole,
        params: Vec<ParamDeclaration>,
        returns: TypeRef,
        is_async: bool,
        generics: Vec<MethodGenericParam>,
        where_predicates: Vec<WherePredicateDecl>,
        docs: Option<DocString>,
        spec_refs: Vec<SpecRef>,
        informal_grounds: Vec<InformalGroundRef>,
    ) -> Self {
        Self {
            action,
            role,
            params,
            returns,
            is_async,
            generics,
            where_predicates,
            docs,
            spec_refs,
            informal_grounds,
        }
    }

    /// The entry action (Add / Modify / Reference / Delete).
    #[must_use]
    pub fn action(&self) -> ItemAction {
        self.action
    }

    /// The architectural role of this function.
    #[must_use]
    pub fn role(&self) -> FunctionRole {
        self.role
    }

    /// The function parameters.
    #[must_use]
    pub fn params(&self) -> &[ParamDeclaration] {
        &self.params
    }

    /// The return type.
    #[must_use]
    pub fn returns(&self) -> &TypeRef {
        &self.returns
    }

    /// Whether this function is `async`.
    #[must_use]
    pub fn is_async(&self) -> bool {
        self.is_async
    }

    /// Generic type parameters on this function.
    #[must_use]
    pub fn generics(&self) -> &[MethodGenericParam] {
        &self.generics
    }

    /// `where`-clause bound predicates on this function's generics.
    #[must_use]
    pub fn where_predicates(&self) -> &[WherePredicateDecl] {
        &self.where_predicates
    }

    /// Optional documentation string.
    #[must_use]
    pub fn docs(&self) -> Option<&DocString> {
        self.docs.as_ref()
    }

    /// SoT Chain ② references to spec.json elements.
    #[must_use]
    pub fn spec_refs(&self) -> &[SpecRef] {
        &self.spec_refs
    }

    /// Informal ground citations (unpersisted rationale).
    #[must_use]
    pub fn informal_grounds(&self) -> &[InformalGroundRef] {
        &self.informal_grounds
    }
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
    pub(crate) type_name: CatalogueEntryKey,

    /// Impl-block-level generic type parameters (type parameters only; lifetimes
    /// and const parameters are out of scope per D2 / IN-05).
    ///
    /// Empty Vec when the impl block is not generic (the common case).
    pub(crate) impl_generics: Vec<MethodGenericParam>,

    /// Impl-block-level where-clause predicates applied to `impl_generics`.
    ///
    /// Empty Vec when there are no impl-level where predicates.
    pub(crate) impl_where_predicates: Vec<WherePredicateDecl>,

    /// Method declarations inside this impl block.
    ///
    /// Empty Vec when the impl block contains no methods.
    pub(crate) methods: Vec<MethodDeclaration>,
}

impl InherentImplDeclV2 {
    /// Creates an inherent implementation declaration.
    #[must_use]
    pub fn new(
        type_name: CatalogueEntryKey,
        impl_generics: Vec<MethodGenericParam>,
        impl_where_predicates: Vec<WherePredicateDecl>,
        methods: Vec<MethodDeclaration>,
    ) -> Self {
        Self { type_name, impl_generics, impl_where_predicates, methods }
    }

    /// Returns the catalogue key of the implemented type.
    #[must_use]
    pub fn type_name(&self) -> &CatalogueEntryKey {
        &self.type_name
    }

    /// Returns the implementation-level generic parameters.
    #[must_use]
    pub fn impl_generics(&self) -> &[MethodGenericParam] {
        &self.impl_generics
    }

    /// Returns the implementation-level where predicates.
    #[must_use]
    pub fn impl_where_predicates(&self) -> &[WherePredicateDecl] {
        &self.impl_where_predicates
    }

    /// Returns the methods declared by the implementation.
    #[must_use]
    pub fn methods(&self) -> &[MethodDeclaration] {
        &self.methods
    }
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
        CrateName, DocString, FieldName, MethodName, ModulePath, ParamName, RustExpression,
        TypeName, TypeRef,
    };
    use crate::tddd::catalogue_v2::roles::{NonEmptyVec, SelfReceiver};
    use crate::tddd::catalogue_v2::variants::FieldDecl;
    use crate::tddd::semantic_verify::CatalogueEntryKey;

    // -----------------------------------------------------------------------
    // TypeEntry
    // -----------------------------------------------------------------------

    #[test]
    fn test_type_entry_with_data_role_compiles() {
        // TypeEntry.role: DataRole — assigning ContractRole is a compile-time error.
        let entry = TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::Struct(StructKind::new(
                StructShape::Plain { fields: vec![], has_stripped_fields: false },
                None,
            )),
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        );
        assert_eq!(entry.role(), &DataRole::value_object());
        assert_eq!(entry.action(), ItemAction::Add);
    }

    #[test]
    fn test_type_entry_with_struct_kind_and_fields() {
        let field_name = FieldName::new("email").unwrap();
        let field_ty = TypeRef::new("String").unwrap();
        let fields = vec![FieldDecl::new(field_name, field_ty)];
        let entry = TypeEntry::new(
            ItemAction::Add,
            DataRole::entity().unwrap(),
            TypeKindV2::Struct(StructKind::new(
                StructShape::Plain { fields: fields.clone(), has_stripped_fields: false },
                None,
            )),
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            Some(DocString::new("A domain entity.".to_string())),
            vec![],
            vec![],
        );
        match entry.kind() {
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
        assert_eq!(entry.docs(), Some(&DocString::new("A domain entity.".to_string())));
    }

    #[test]
    fn test_type_entry_with_methods() {
        let method = MethodDeclaration::new(
            MethodName::new("as_str").unwrap(),
            Some(SelfReceiver::SharedRef),
            vec![],
            TypeRef::new("str").unwrap(),
            false,
            false,
            vec![],
            vec![],
            vec![],
            ItemAction::Add,
            None,
        );
        let field_ty = TypeRef::new("String").unwrap();
        let entry = TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::Struct(StructKind::new(
                StructShape::Tuple { fields: vec![field_ty], has_stripped_fields: false },
                None,
            )),
            vec![method.clone()],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        );
        assert_eq!(entry.methods().len(), 1);
        assert_eq!(entry.methods()[0], method);
    }

    #[test]
    fn test_type_entry_with_module_path() {
        let module_path =
            ModulePath::from_segments(vec!["user".to_string(), "domain".to_string()]).unwrap();
        let entry = TypeEntry::new(
            ItemAction::Modify,
            DataRole::aggregate_root().unwrap(),
            TypeKindV2::Struct(StructKind::new(
                StructShape::Plain { fields: vec![], has_stripped_fields: false },
                None,
            )),
            vec![],
            vec![],
            vec![],
            module_path.clone(),
            None,
            vec![],
            vec![],
        );
        assert_eq!(entry.module_path(), &module_path);
        assert_eq!(entry.action(), ItemAction::Modify);
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
            let entry = TypeEntry::new(
                ItemAction::Add,
                role.clone(),
                TypeKindV2::Struct(StructKind::new(
                    StructShape::Plain { fields: vec![], has_stripped_fields: false },
                    None,
                )),
                vec![],
                vec![],
                vec![],
                ModulePath::root(),
                None,
                vec![],
                vec![],
            );
            assert_eq!(entry.role(), &role);
        }
    }

    // -----------------------------------------------------------------------
    // TraitEntry
    // -----------------------------------------------------------------------

    fn trait_entry_fixture() -> TraitEntry {
        TraitEntry::new(
            ItemAction::Add,
            ContractRole::SecondaryPort,
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        )
    }

    #[test]
    fn test_trait_entry_with_contract_role_compiles() {
        // TraitEntry.role: ContractRole — assigning DataRole is a compile-time error.
        let entry = trait_entry_fixture();
        assert_eq!(entry.role(), &ContractRole::SecondaryPort);
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
            false,
            vec![],
            vec![],
            vec![],
            ItemAction::Add,
            None,
        );
        let entry = TraitEntry::new(
            ItemAction::Add,
            ContractRole::SecondaryPort,
            vec![save_method.clone()],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            Some(DocString::new("User repository port.".to_string())),
            vec![],
            vec![],
        );
        assert_eq!(entry.methods().len(), 1);
        assert_eq!(entry.methods()[0], save_method);
        assert_eq!(entry.docs(), Some(&DocString::new("User repository port.".to_string())));
    }

    #[test]
    fn test_trait_entry_with_supertrait_bounds() {
        let send = TypeRef::new("Send").unwrap();
        let sync = TypeRef::new("Sync").unwrap();
        let entry = TraitEntry::new(
            ItemAction::Add,
            ContractRole::SecondaryPort,
            vec![],
            vec![],
            vec![],
            vec![send.clone(), sync.clone()],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        );
        assert_eq!(entry.supertrait_bounds().len(), 2);
        assert_eq!(entry.supertrait_bounds()[0].as_str(), "Send");
        assert_eq!(entry.supertrait_bounds()[1].as_str(), "Sync");
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
            let entry = TraitEntry::new(
                ItemAction::Add,
                role.clone(),
                vec![],
                vec![],
                vec![],
                vec![],
                vec![],
                vec![],
                ModulePath::root(),
                None,
                vec![],
                vec![],
            );
            assert_eq!(entry.role(), &role);
        }
    }

    #[test]
    fn test_trait_entry_new_has_empty_generics_by_default() {
        // AC-07: TraitEntry must carry a generics field defaulting to empty Vec.
        let entry = trait_entry_fixture();
        assert!(entry.generics().is_empty());
    }

    #[test]
    fn test_trait_entry_new_has_empty_where_predicates_by_default() {
        // AC-07: TraitEntry must carry a where_predicates field defaulting to empty Vec.
        let entry = trait_entry_fixture();
        assert!(entry.where_predicates().is_empty());
    }

    #[test]
    fn test_trait_entry_new_has_empty_assoc_items_by_default() {
        let entry = trait_entry_fixture();
        assert!(entry.assoc_types().is_empty());
        assert!(entry.assoc_consts().is_empty());
    }

    #[test]
    fn test_trait_entry_generics_and_where_predicates_for_generic_trait_decl() {
        // AC-07 primary: `trait Foo<T> where T: Clone` can be represented.
        use crate::tddd::catalogue_v2::methods::{BoundOp, WherePredicateDecl};
        let entry = TraitEntry::new(
            ItemAction::Add,
            ContractRole::SecondaryPort,
            vec![],
            vec![],
            vec![],
            vec![],
            vec![MethodGenericParam { name: ParamName::new("T").unwrap(), bounds: vec![] }],
            vec![WherePredicateDecl {
                lhs: TypeRef::new("T").unwrap(),
                rhs: vec![TypeRef::new("Clone").unwrap()],
                operator: BoundOp::Bound,
            }],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        );
        assert_eq!(entry.generics().len(), 1);
        assert_eq!(entry.generics()[0].name.as_str(), "T");
        assert_eq!(entry.where_predicates().len(), 1);
        assert_eq!(entry.where_predicates()[0].lhs.as_str(), "T");
        assert_eq!(entry.where_predicates()[0].rhs[0].as_str(), "Clone");
    }

    #[test]
    fn test_trait_entry_generics_participates_in_equality() {
        // generics field must participate in PartialEq (derive-level guarantee).
        let base = trait_entry_fixture();
        let with_generic = TraitEntry::new(
            ItemAction::Add,
            ContractRole::SecondaryPort,
            vec![],
            vec![],
            vec![],
            vec![],
            vec![MethodGenericParam { name: ParamName::new("T").unwrap(), bounds: vec![] }],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        );
        assert_ne!(base, with_generic, "generics field must participate in equality");
    }

    #[test]
    fn test_trait_entry_where_predicates_participates_in_equality() {
        // where_predicates field must participate in PartialEq.
        use crate::tddd::catalogue_v2::methods::{BoundOp, WherePredicateDecl};
        let base = trait_entry_fixture();
        let with_pred = TraitEntry::new(
            ItemAction::Add,
            ContractRole::SecondaryPort,
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![WherePredicateDecl {
                lhs: TypeRef::new("T").unwrap(),
                rhs: vec![TypeRef::new("Clone").unwrap()],
                operator: BoundOp::Bound,
            }],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        );
        assert_ne!(base, with_pred, "where_predicates field must participate in equality");
    }

    #[test]
    fn test_trait_entry_assoc_items_participate_in_equality() {
        let base = trait_entry_fixture();

        let with_assoc_type = TraitEntry::new(
            ItemAction::Add,
            ContractRole::SecondaryPort,
            vec![],
            vec![AssocTypeDecl {
                name: TypeName::new("Input").unwrap(),
                bounds: vec![TypeRef::new("Send").unwrap()],
                default: Some(TypeRef::new("Vec<u8>").unwrap()),
            }],
            vec![],
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        );
        assert_ne!(base, with_assoc_type, "assoc_types field must participate in equality");

        let with_assoc_const = TraitEntry::new(
            ItemAction::Add,
            ContractRole::SecondaryPort,
            vec![],
            vec![],
            vec![AssocConstDecl {
                name: AssocConstName::new("CHAIN_ID").unwrap(),
                ty: TypeRef::new("ChainId").unwrap(),
                default_value: Some(RustExpression::try_new("DEFAULT_CHAIN_ID").unwrap()),
            }],
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        );
        assert_ne!(base, with_assoc_const, "assoc_consts field must participate in equality");
    }

    // -----------------------------------------------------------------------
    // FunctionEntry
    // -----------------------------------------------------------------------

    #[test]
    fn test_function_entry_with_function_role_compiles() {
        // FunctionEntry.role: FunctionRole — assigning DataRole is a compile-time error.
        let entry = FunctionEntry::new(
            ItemAction::Add,
            FunctionRole::FreeFunction,
            vec![],
            TypeRef::new("()").unwrap(),
            false,
            vec![],
            vec![],
            None,
            vec![],
            vec![],
        );
        assert_eq!(entry.role(), FunctionRole::FreeFunction);
        assert!(!entry.is_async());
    }

    #[test]
    fn test_function_entry_async_with_params_and_returns() {
        let entry = FunctionEntry::new(
            ItemAction::Add,
            FunctionRole::UseCaseFunction,
            vec![ParamDeclaration::new(
                ParamName::new("cmd").unwrap(),
                TypeRef::new("RegisterUserCommand").unwrap(),
            )],
            TypeRef::new("Result<UserId, ApplicationError>").unwrap(),
            true,
            vec![],
            vec![],
            Some(DocString::new("Register a new user.".to_string())),
            vec![],
            vec![],
        );
        assert!(entry.is_async());
        assert_eq!(entry.params().len(), 1);
        assert_eq!(entry.docs(), Some(&DocString::new("Register a new user.".to_string())));
    }

    #[test]
    fn test_function_entry_all_function_roles_are_accepted() {
        let roles = [FunctionRole::FreeFunction, FunctionRole::UseCaseFunction];
        for role in roles {
            let entry = FunctionEntry::new(
                ItemAction::Add,
                role,
                vec![],
                TypeRef::new("()").unwrap(),
                false,
                vec![],
                vec![],
                None,
                vec![],
                vec![],
            );
            assert_eq!(entry.role(), role);
        }
    }

    #[test]
    fn test_function_entry_with_generics_stores_them() {
        // ADR 2026-05-08-0248 D14: FunctionEntry carries explicit generic params
        // so the A-codec can mirror rustdoc's `Function.generics`.
        use crate::tddd::catalogue_v2::methods::MethodGenericParam;
        let entry = FunctionEntry::new(
            ItemAction::Add,
            FunctionRole::FreeFunction,
            vec![],
            TypeRef::new("T").unwrap(),
            false,
            vec![MethodGenericParam {
                name: ParamName::new("T").unwrap(),
                bounds: vec![TypeRef::new("Clone").unwrap()],
            }],
            vec![],
            None,
            vec![],
            vec![],
        );
        assert_eq!(entry.generics().len(), 1);
        assert_eq!(entry.generics()[0].name.as_str(), "T");
        assert_eq!(entry.generics()[0].bounds[0].as_str(), "Clone");
    }

    #[test]
    fn test_function_entry_default_generics_is_empty() {
        let entry = FunctionEntry::new(
            ItemAction::Add,
            FunctionRole::FreeFunction,
            vec![],
            TypeRef::new("()").unwrap(),
            false,
            vec![],
            vec![],
            None,
            vec![],
            vec![],
        );
        assert!(entry.generics().is_empty());
    }

    #[test]
    fn test_function_entry_generics_distinguishes_otherwise_equal_entries() {
        use crate::tddd::catalogue_v2::methods::MethodGenericParam;
        let base = FunctionEntry::new(
            ItemAction::Add,
            FunctionRole::FreeFunction,
            vec![],
            TypeRef::new("()").unwrap(),
            false,
            vec![],
            vec![],
            None,
            vec![],
            vec![],
        );
        let with_generic = FunctionEntry::new(
            ItemAction::Add,
            FunctionRole::FreeFunction,
            vec![],
            TypeRef::new("()").unwrap(),
            false,
            vec![MethodGenericParam { name: ParamName::new("T").unwrap(), bounds: vec![] }],
            vec![],
            None,
            vec![],
            vec![],
        );
        assert_ne!(base, with_generic, "generics field participates in equality");
    }

    #[test]
    fn test_function_entry_with_where_predicates_stores_them() {
        // ADR 2026-05-18-1223 D1: FunctionEntry carries explicit where_predicates so
        // catalogue authors can express constraints whose LHS is a type expression
        // (e.g. `where Vec<T>: Bound`) that the inline form cannot represent.
        use crate::tddd::catalogue_v2::methods::BoundOp;
        let entry = FunctionEntry::new(
            ItemAction::Add,
            FunctionRole::FreeFunction,
            vec![],
            TypeRef::new("()").unwrap(),
            false,
            vec![],
            vec![WherePredicateDecl {
                lhs: TypeRef::new("Vec<T>").unwrap(),
                rhs: vec![TypeRef::new("Send").unwrap()],
                operator: BoundOp::Bound,
            }],
            None,
            vec![],
            vec![],
        );
        assert_eq!(entry.where_predicates().len(), 1);
        assert_eq!(entry.where_predicates()[0].lhs.as_str(), "Vec<T>");
        assert_eq!(entry.where_predicates()[0].rhs[0].as_str(), "Send");
    }

    #[test]
    fn test_function_entry_where_predicates_distinguish_otherwise_equal_entries() {
        use crate::tddd::catalogue_v2::methods::BoundOp;
        let base = FunctionEntry::new(
            ItemAction::Add,
            FunctionRole::FreeFunction,
            vec![],
            TypeRef::new("()").unwrap(),
            false,
            vec![],
            vec![],
            None,
            vec![],
            vec![],
        );
        let with_where = FunctionEntry::new(
            ItemAction::Add,
            FunctionRole::FreeFunction,
            vec![],
            TypeRef::new("()").unwrap(),
            false,
            vec![],
            vec![WherePredicateDecl {
                lhs: TypeRef::new("T").unwrap(),
                rhs: vec![TypeRef::new("Clone").unwrap()],
                operator: BoundOp::Bound,
            }],
            None,
            vec![],
            vec![],
        );
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

        let entry = TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::Struct(StructKind::new(
                StructShape::Plain { fields: vec![], has_stripped_fields: false },
                None,
            )),
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![spec_ref.clone()],
            vec![],
        );
        assert_eq!(entry.spec_refs().len(), 1);
        assert_eq!(entry.spec_refs()[0], spec_ref);
        assert!(entry.informal_grounds().is_empty());
    }

    #[test]
    fn test_trait_entry_with_non_empty_informal_grounds_stores_grounding() {
        use crate::plan_ref::{InformalGroundKind, InformalGroundRef, InformalGroundSummary};

        let summary = InformalGroundSummary::try_new("discussed in planning session").unwrap();
        let ground = InformalGroundRef::new(InformalGroundKind::Discussion, summary);

        let entry = TraitEntry::new(
            ItemAction::Add,
            ContractRole::SecondaryPort,
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![ground.clone()],
        );
        assert_eq!(entry.informal_grounds().len(), 1);
        assert_eq!(entry.informal_grounds()[0], ground);
        assert!(entry.spec_refs().is_empty());
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

        let entry = FunctionEntry::new(
            ItemAction::Add,
            FunctionRole::FreeFunction,
            vec![],
            TypeRef::new("()").unwrap(),
            false,
            vec![],
            vec![],
            None,
            vec![spec_ref.clone()],
            vec![ground.clone()],
        );
        assert_eq!(entry.spec_refs().len(), 1);
        assert_eq!(entry.spec_refs()[0], spec_ref);
        assert_eq!(entry.informal_grounds().len(), 1);
        assert_eq!(entry.informal_grounds()[0], ground);
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
            false,
            vec![],
            vec![],
            vec![],
            ItemAction::Add,
            None,
        );
        let method_b = MethodDeclaration::new(
            MethodName::new("validate").unwrap(),
            Some(SelfReceiver::SharedRef),
            vec![],
            TypeRef::new("Result<(), DomainError>").unwrap(),
            false,
            false,
            vec![],
            vec![],
            vec![],
            ItemAction::Add,
            None,
        );

        let impl_block_a = InherentImplDeclV2 {
            type_name: CatalogueEntryKey::try_new(type_name.as_str().to_owned()).unwrap(),
            impl_generics: vec![],
            impl_where_predicates: vec![],
            methods: vec![method_a.clone()],
        };
        let impl_block_b = InherentImplDeclV2 {
            type_name: CatalogueEntryKey::try_new(type_name.as_str().to_owned()).unwrap(),
            impl_generics: vec![],
            impl_where_predicates: vec![],
            methods: vec![method_b.clone()],
        };

        // Both blocks share the same type_name, representing two inherent impl blocks
        // for `Email` in the source code.
        assert_eq!(
            impl_block_a.type_name,
            CatalogueEntryKey::try_new(type_name.as_str().to_owned()).unwrap()
        );
        assert_eq!(
            impl_block_b.type_name,
            CatalogueEntryKey::try_new(type_name.as_str().to_owned()).unwrap()
        );
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
            type_name: CatalogueEntryKey::try_new(type_name.as_str().to_owned()).unwrap(),
            impl_generics: vec![generic_param],
            impl_where_predicates: vec![where_pred],
            methods: vec![],
        };

        assert_eq!(
            impl_block.type_name,
            CatalogueEntryKey::try_new(type_name.as_str().to_owned()).unwrap()
        );
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
            type_name: CatalogueEntryKey::try_new(type_name.as_str().to_owned()).unwrap(),
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
            type_name: CatalogueEntryKey::try_new(type_name.as_str().to_owned()).unwrap(),
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
        assert!(doc.inherent_impls().is_empty());
    }

    #[test]
    fn test_catalogue_document_inherent_impls_stores_multiple_entries_for_one_type() {
        use crate::tddd::catalogue_v2::document::CatalogueDocument;
        use crate::tddd::layer_id::LayerId;

        let crate_name = CrateName::new("domain").unwrap();
        let layer = LayerId::try_new("domain").unwrap();
        let mut doc = CatalogueDocument::new(3, crate_name, layer);

        let type_name = TypeName::new("Email").unwrap();
        doc.push_inherent_impl(InherentImplDeclV2 {
            type_name: CatalogueEntryKey::try_new(type_name.as_str().to_owned()).unwrap(),
            impl_generics: vec![],
            impl_where_predicates: vec![],
            methods: vec![],
        });
        doc.push_inherent_impl(InherentImplDeclV2 {
            type_name: CatalogueEntryKey::try_new(type_name.as_str().to_owned()).unwrap(),
            impl_generics: vec![],
            impl_where_predicates: vec![],
            methods: vec![],
        });

        assert_eq!(doc.inherent_impls().len(), 2);
        assert_eq!(
            doc.inherent_impls()[0].type_name,
            CatalogueEntryKey::try_new(type_name.as_str().to_owned()).unwrap()
        );
        assert_eq!(
            doc.inherent_impls()[1].type_name,
            CatalogueEntryKey::try_new(type_name.as_str().to_owned()).unwrap()
        );
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
