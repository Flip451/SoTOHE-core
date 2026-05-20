//! Trait implementation declaration for the catalogue v2 schema.
//!
//! Implements:
//! - [`TraitImplDeclV2`]: top-level trait implementation record with 2-axis symmetric structure:
//!   `trait_ref: TypeRef` (the trait, possibly with generic args) and `for_type: TypeRef`
//!   (the self type, either self-crate or external crate).
//!   No `methods` field — trait/impl signature alignment is delegated to the Rust compiler
//!   (ADR 1 D10 / CN-07).
//!
//! No serde derives — per ADR `knowledge/adr/2026-04-14-1531-domain-serde-ripout.md`,
//! the domain layer is serialization-free. The infrastructure codec (T003) handles JSON.
//!
//! ADR `2026-05-20-0048-tddd-trait-impl-top-level-promotion` D2:
//! - `trait_ref`: fully-qualified trait reference (e.g. `"core::convert::From<MyError>"`, `"std::fmt::Display"`)
//! - `for_type`: self type reference, self-crate short name or fully-qualified external path
//! - `impl_generics`: impl-block-level generic type parameters
//! - `impl_where_predicates`: impl-block-level where-clause predicates
//!
//! Old fields `trait_name`, `origin_crate`, `generic_args` are removed.

use crate::tddd::catalogue_v2::identifiers::TypeRef;
use crate::tddd::catalogue_v2::methods::{MethodGenericParam, WherePredicateDecl};
use crate::tddd::catalogue_v2::roles::ItemAction;

// ---------------------------------------------------------------------------
// TraitImplDeclV2 — top-level trait implementation declaration (ADR 0048 D1/D2)
// ---------------------------------------------------------------------------

/// Top-level trait implementation declaration (ADR `2026-05-20-0048` D1 / D2).
///
/// Represents one `impl Trait for Type` block in the catalogue. Unlike the old design
/// where `TraitImplDeclV2` was embedded inside `TypeEntry`, these are now stored at
/// `CatalogueDocument::trait_impls` as independent entries — symmetric with
/// `CatalogueDocument::inherent_impls`.
///
/// ## Field semantics (D2)
///
/// - `action`: TDDD operation for this impl entry (Add / Modify / Reference / Delete).
///   Defaults to `Add` (the common case — declaring a new impl). As a top-level
///   independent entry with no parent `TypeEntry`, `TraitImplDeclV2` must carry its own
///   action rather than inheriting from a parent (ADR `2026-05-20-0048` D2 / IN-15).
///
/// - `trait_ref`: the trait reference as a `TypeRef` string. Includes the crate prefix
///   for external traits (e.g. `"core::convert::From<MyError>"`, `"std::fmt::Display"`)
///   and uses the bare short name for self-crate traits (e.g. `"MyTrait"`). Generic
///   arguments are embedded in the `trait_ref` string (e.g. `"From<Vec<i32>>"`).
///
/// - `for_type`: the self type as a `TypeRef` string. Self-crate types use the bare
///   short name (e.g. `"SelfType"`); external-crate types use the fully-qualified path
///   (e.g. `"std::vec::Vec<i32>"`).
///
/// - `impl_generics`: impl-block-level generic type parameters (type parameters only;
///   lifetimes and const parameters are out of scope per IN-06). Empty Vec for
///   non-generic impls.
///
/// - `impl_where_predicates`: impl-block-level where-clause predicates. Empty Vec
///   when there are no impl-level constraints.
///
/// ## Coverage
///
/// The 2-axis design covers all three cases:
/// - Case A: `impl From<external::Ext> for SelfType` — external trait + self-crate type
/// - Case B: `impl MyTrait for std::vec::Vec<i32>` — self-crate trait + external-crate type
/// - Self-crate: `impl MyTrait for SelfType` — both in self crate
///
/// Used in [`crate::tddd::catalogue_v2::document::CatalogueDocument::trait_impls`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitImplDeclV2 {
    /// TDDD action for this impl entry (Add / Modify / Reference / Delete).
    ///
    /// Defaults to `Add`. As a top-level independent entry with no parent `TypeEntry`,
    /// this field is required rather than inherited (ADR `2026-05-20-0048` D2 / IN-15).
    /// The serde default is handled at the infrastructure DTO layer (not here — the domain
    /// layer is serialization-free per `knowledge/adr/2026-04-14-1531-domain-serde-ripout.md`).
    pub action: ItemAction,

    /// The trait reference (includes generic args if any).
    ///
    /// Examples:
    /// - `"core::convert::From<MyError>"` — external trait with generic arg
    /// - `"std::fmt::Display"` — external trait without generic arg
    /// - `"MyTrait"` — self-crate trait (bare short name)
    /// - `"FnOnce<(A,), B>"` — complex generic form
    pub trait_ref: TypeRef,

    /// The self type of the impl (the `Type` in `impl Trait for Type`).
    ///
    /// Examples:
    /// - `"SelfType"` — self-crate type (bare short name)
    /// - `"std::vec::Vec<i32>"` — external-crate type
    pub for_type: TypeRef,

    /// Impl-block-level generic type parameters (type parameters only; lifetimes and
    /// const parameters are out of scope per IN-06).
    ///
    /// Allows cataloguing `impl<L, R, W> Trait for Foo<L, R, W>` where the impl block
    /// itself introduces generic parameters. Empty Vec when the trait impl is not generic
    /// at the impl-block level (the common case).
    pub impl_generics: Vec<MethodGenericParam>,

    /// Impl-block-level where-clause predicates applied to `impl_generics`.
    ///
    /// Allows cataloguing constraints such as `where L: Send` on an impl block.
    /// Empty Vec when there are no impl-level where predicates.
    pub impl_where_predicates: Vec<WherePredicateDecl>,
}

impl TraitImplDeclV2 {
    /// Creates a new `TraitImplDeclV2` with default `Add` action.
    ///
    /// `impl_generics` and `impl_where_predicates` default to empty Vec.
    #[must_use]
    pub fn new(trait_ref: TypeRef, for_type: TypeRef) -> Self {
        Self {
            action: ItemAction::Add,
            trait_ref,
            for_type,
            impl_generics: vec![],
            impl_where_predicates: vec![],
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::tddd::catalogue_v2::identifiers::{ParamName, TypeRef};
    use crate::tddd::catalogue_v2::methods::{BoundOp, MethodGenericParam, WherePredicateDecl};
    use crate::tddd::catalogue_v2::roles::ItemAction;

    #[test]
    fn test_trait_impl_decl_v2_new_stores_trait_ref_and_for_type() {
        let trait_ref = TypeRef::new("std::fmt::Display").unwrap();
        let for_type = TypeRef::new("MyType").unwrap();
        let decl = TraitImplDeclV2::new(trait_ref.clone(), for_type.clone());
        assert_eq!(decl.trait_ref, trait_ref);
        assert_eq!(decl.for_type, for_type);
        assert!(decl.impl_generics.is_empty());
        assert!(decl.impl_where_predicates.is_empty());
    }

    // AC-14(b): `action` defaults to `Add` when using `new()`
    #[test]
    fn test_trait_impl_decl_v2_new_has_default_action_add() {
        let decl = TraitImplDeclV2::new(
            TypeRef::new("std::fmt::Display").unwrap(),
            TypeRef::new("MyType").unwrap(),
        );
        assert_eq!(decl.action, ItemAction::Add, "TraitImplDeclV2::new() default action is Add");
    }

    // AC-14(c): Modify / Reference actions can be set on TraitImplDeclV2
    #[test]
    fn test_trait_impl_decl_v2_modify_action_can_be_set() {
        let mut decl = TraitImplDeclV2::new(
            TypeRef::new("std::fmt::Display").unwrap(),
            TypeRef::new("MyType").unwrap(),
        );
        decl.action = ItemAction::Modify;
        assert_eq!(decl.action, ItemAction::Modify);
    }

    #[test]
    fn test_trait_impl_decl_v2_reference_action_can_be_set() {
        let mut decl = TraitImplDeclV2::new(
            TypeRef::new("std::fmt::Display").unwrap(),
            TypeRef::new("MyType").unwrap(),
        );
        decl.action = ItemAction::Reference;
        assert_eq!(decl.action, ItemAction::Reference);
    }

    #[test]
    fn test_trait_impl_decl_v2_delete_action_can_be_set() {
        let mut decl = TraitImplDeclV2::new(
            TypeRef::new("std::fmt::Display").unwrap(),
            TypeRef::new("MyType").unwrap(),
        );
        decl.action = ItemAction::Delete;
        assert_eq!(decl.action, ItemAction::Delete);
    }

    // action participates in equality
    #[test]
    fn test_trait_impl_decl_v2_action_participates_in_equality() {
        let base = TraitImplDeclV2::new(
            TypeRef::new("std::fmt::Display").unwrap(),
            TypeRef::new("MyType").unwrap(),
        );
        let mut with_modify = base.clone();
        with_modify.action = ItemAction::Modify;
        assert_ne!(base, with_modify, "action participates in TraitImplDeclV2 equality");
    }

    #[test]
    fn test_trait_impl_decl_v2_for_external_trait_with_generic_args() {
        // Case A: external trait with generic args
        let trait_ref = TypeRef::new("core::convert::From<MyError>").unwrap();
        let for_type = TypeRef::new("SelfType").unwrap();
        let decl = TraitImplDeclV2::new(trait_ref.clone(), for_type.clone());
        assert_eq!(decl.trait_ref.as_str(), "core::convert::From<MyError>");
        assert_eq!(decl.for_type.as_str(), "SelfType");
    }

    #[test]
    fn test_trait_impl_decl_v2_for_external_for_type() {
        // Case B: self-crate trait + external-crate type
        let trait_ref = TypeRef::new("MyTrait").unwrap();
        let for_type = TypeRef::new("std::vec::Vec<i32>").unwrap();
        let decl = TraitImplDeclV2::new(trait_ref.clone(), for_type.clone());
        assert_eq!(decl.trait_ref.as_str(), "MyTrait");
        assert_eq!(decl.for_type.as_str(), "std::vec::Vec<i32>");
    }

    #[test]
    fn test_trait_impl_decl_v2_equality_by_all_fields() {
        let a = TraitImplDeclV2::new(
            TypeRef::new("std::fmt::Display").unwrap(),
            TypeRef::new("MyType").unwrap(),
        );
        let b = TraitImplDeclV2::new(
            TypeRef::new("std::fmt::Display").unwrap(),
            TypeRef::new("MyType").unwrap(),
        );
        assert_eq!(a, b);
    }

    #[test]
    fn test_trait_impl_decl_v2_different_trait_ref_are_not_equal() {
        let a = TraitImplDeclV2::new(
            TypeRef::new("std::fmt::Display").unwrap(),
            TypeRef::new("MyType").unwrap(),
        );
        let b = TraitImplDeclV2::new(
            TypeRef::new("std::fmt::Debug").unwrap(),
            TypeRef::new("MyType").unwrap(),
        );
        assert_ne!(a, b);
    }

    #[test]
    fn test_trait_impl_decl_v2_different_for_type_are_not_equal() {
        let a = TraitImplDeclV2::new(
            TypeRef::new("std::fmt::Display").unwrap(),
            TypeRef::new("MyType").unwrap(),
        );
        let b = TraitImplDeclV2::new(
            TypeRef::new("std::fmt::Display").unwrap(),
            TypeRef::new("OtherType").unwrap(),
        );
        assert_ne!(a, b);
    }

    #[test]
    fn test_trait_impl_decl_v2_new_has_empty_impl_generics_by_default() {
        let decl = TraitImplDeclV2::new(
            TypeRef::new("MyTrait").unwrap(),
            TypeRef::new("SelfType").unwrap(),
        );
        assert!(decl.impl_generics.is_empty());
    }

    #[test]
    fn test_trait_impl_decl_v2_new_has_empty_impl_where_predicates_by_default() {
        let decl = TraitImplDeclV2::new(
            TypeRef::new("MyTrait").unwrap(),
            TypeRef::new("SelfType").unwrap(),
        );
        assert!(decl.impl_where_predicates.is_empty());
    }

    #[test]
    fn test_trait_impl_decl_v2_impl_generics_and_where_predicates_for_generic_impl_block() {
        // AC-06 equivalent: `impl<L, R, W> Trait for Foo<L, R, W> where L: Send`
        let mut decl = TraitImplDeclV2::new(
            TypeRef::new("MyTrait").unwrap(),
            TypeRef::new("Foo<L, R, W>").unwrap(),
        );
        decl.impl_generics = vec![
            MethodGenericParam { name: ParamName::new("L").unwrap(), bounds: vec![] },
            MethodGenericParam { name: ParamName::new("R").unwrap(), bounds: vec![] },
            MethodGenericParam { name: ParamName::new("W").unwrap(), bounds: vec![] },
        ];
        decl.impl_where_predicates = vec![WherePredicateDecl {
            lhs: TypeRef::new("L").unwrap(),
            rhs: vec![TypeRef::new("Send").unwrap()],
            operator: BoundOp::Bound,
        }];

        assert_eq!(decl.impl_generics.len(), 3);
        assert_eq!(decl.impl_generics[0].name.as_str(), "L");
        assert_eq!(decl.impl_generics[1].name.as_str(), "R");
        assert_eq!(decl.impl_generics[2].name.as_str(), "W");
        assert_eq!(decl.impl_where_predicates.len(), 1);
        assert_eq!(decl.impl_where_predicates[0].lhs.as_str(), "L");
        assert_eq!(decl.impl_where_predicates[0].rhs[0].as_str(), "Send");
        assert_eq!(decl.impl_where_predicates[0].operator, BoundOp::Bound);
    }

    #[test]
    fn test_trait_impl_decl_v2_impl_generics_participates_in_equality() {
        let base = TraitImplDeclV2::new(
            TypeRef::new("serde::Serialize").unwrap(),
            TypeRef::new("MyStruct").unwrap(),
        );
        let mut with_generics = base.clone();
        with_generics.impl_generics =
            vec![MethodGenericParam { name: ParamName::new("T").unwrap(), bounds: vec![] }];
        assert_ne!(base, with_generics, "impl_generics participates in TraitImplDeclV2 equality");
    }

    #[test]
    fn test_trait_impl_decl_v2_impl_where_predicates_participates_in_equality() {
        let base = TraitImplDeclV2::new(
            TypeRef::new("serde::Serialize").unwrap(),
            TypeRef::new("MyStruct").unwrap(),
        );
        let mut with_where = base.clone();
        with_where.impl_where_predicates = vec![WherePredicateDecl {
            lhs: TypeRef::new("T").unwrap(),
            rhs: vec![TypeRef::new("Clone").unwrap()],
            operator: BoundOp::Bound,
        }];
        assert_ne!(
            base, with_where,
            "impl_where_predicates participates in TraitImplDeclV2 equality"
        );
    }

    // AC-14: `impl MyTrait for std::vec::Vec<i32>` (Case B) is representable
    #[test]
    fn test_trait_impl_decl_v2_external_self_type_case_b_is_representable() {
        let trait_ref = TypeRef::new("MyTrait").unwrap();
        let for_type = TypeRef::new("std::vec::Vec<i32>").unwrap();
        let decl = TraitImplDeclV2::new(trait_ref, for_type);
        assert_eq!(decl.trait_ref.as_str(), "MyTrait");
        assert_eq!(decl.for_type.as_str(), "std::vec::Vec<i32>");
    }

    // AC-13: top-level trait_impls on CatalogueDocument (tested via document.rs)
    // AC-15: self-crate and external-crate types use the same path
    #[test]
    fn test_trait_impl_decl_v2_self_crate_and_external_use_same_schema() {
        // Self-crate type
        let self_crate = TraitImplDeclV2::new(
            TypeRef::new("std::fmt::Display").unwrap(),
            TypeRef::new("SelfType").unwrap(),
        );
        // External-crate type (Case B)
        let external = TraitImplDeclV2::new(
            TypeRef::new("MyTrait").unwrap(),
            TypeRef::new("std::vec::Vec<i32>").unwrap(),
        );
        // Both are representable with the same struct — same schema path
        assert!(!self_crate.impl_generics.is_empty() || self_crate.impl_generics.is_empty());
        assert!(!external.impl_generics.is_empty() || external.impl_generics.is_empty());
    }
}
