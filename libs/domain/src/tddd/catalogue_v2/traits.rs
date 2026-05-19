//! Trait implementation declaration for the catalogue v2 schema.
//!
//! Implements:
//! - [`TraitImplDeclV2`]: identity-only trait implementation record (`trait_name`,
//!   `origin_crate`, optional `generic_args`, optional `impl_generics`,
//!   optional `impl_where_predicates`).
//!   No `methods` field — trait/impl signature alignment is delegated to the Rust compiler
//!   (ADR 1 D10 / CN-07).
//!
//! No serde derives — per ADR `knowledge/adr/2026-04-14-1531-domain-serde-ripout.md`,
//! the domain layer is serialization-free. The infrastructure codec (T003) handles JSON.

use std::fmt;

use crate::tddd::catalogue_v2::identifiers::{CrateName, TraitName};
use crate::tddd::catalogue_v2::methods::{MethodGenericParam, WherePredicateDecl};

// ---------------------------------------------------------------------------
// GenericArgsError — validation error for generic_args
// ---------------------------------------------------------------------------

/// Validation error for the `generic_args` argument passed to
/// [`TraitImplDeclV2::new_with_generic_args`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GenericArgsError {
    /// The provided string was empty or contained only whitespace.
    #[error("generic_args must not be empty or whitespace-only")]
    Empty,
    /// The provided string was already wrapped in angle brackets (starts with `<`).
    /// Pass only the inner type name — the Display impl adds the brackets automatically.
    #[error(
        "generic_args must not start with `<` — pass the inner type without surrounding \
         angle brackets (e.g. `\"CatalogueLoaderError\"`, not `\"<CatalogueLoaderError>\"`)"
    )]
    StartsWithAngleBracket,
    /// The provided string contains unbalanced or misordered angle brackets.
    /// Angle brackets must be properly nested: each `<` must have a matching `>` that
    /// appears after it, and no `>` may appear before its opening `<`. This ensures
    /// the Display impl wraps the value correctly (e.g. `"Vec<i32>"` produces
    /// `"From<Vec<i32>>"` without any malformed identity keys).
    #[error(
        "generic_args contains unbalanced or misordered angle brackets — brackets must be \
         properly nested (e.g. `\"Vec<i32>\"` is valid, `\"Vec<T><U>\"` and `\">T<\"` are not)"
    )]
    UnbalancedAngleBrackets,
}

// ---------------------------------------------------------------------------
// TraitImplDeclV2 — identity-only trait implementation declaration
// ---------------------------------------------------------------------------

/// Identity-only trait implementation declaration.
///
/// Declares that a type implements a particular trait, identified by `trait_name`
/// and the crate it originates from (`origin_crate`). There is no `methods` field —
/// the methods declared in the trait definition and the implementing type's inherent
/// methods are the source of truth for the Rust compiler; duplicating them in the
/// catalogue would create a maintenance burden without adding value (ADR 1 D10).
///
/// The optional `generic_args` field allows distinguishing multiple impls of the
/// same trait on the same type — for example, two `#[from]` variants in a thiserror
/// enum each generate a distinct `From<X>` impl. When `generic_args` is `Some`,
/// the signal evaluator constructs the identity key as `"TypeName: From<X>"`,
/// matching the C-side rustdoc key exactly. When `None`, the bare trait name is
/// used and the stripped-key fallback in phase2 handles backward-compatible matching.
///
/// Replaces the old `TraitImplDecl` type (which included methods).
///
/// Used in [`crate::tddd::catalogue_v2::entries::TypeEntry::trait_impls`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitImplDeclV2 {
    /// The short name of the implemented trait.
    pub trait_name: TraitName,
    /// The crate that defines the trait.
    pub origin_crate: CrateName,
    /// Generic argument string for the impl, e.g. `"CatalogueLoaderError"` for
    /// `From<CatalogueLoaderError>`. `None` denotes a non-generic impl (`Display`,
    /// `Error`) or a declaration that intentionally elides generics for backward-
    /// compatible stripped-key matching.
    ///
    /// When `Some`, the signal evaluator produces an identity key like
    /// `"TypeName: From<CatalogueLoaderError>"`, disambiguating impls with different
    /// concrete type parameters (e.g. two `#[from]` variants in a thiserror enum).
    ///
    /// Invariants (enforced by [`TraitImplDeclV2::new_with_generic_args`]):
    /// - Must not be empty or whitespace-only.
    /// - Must not start with `<` (pass the raw type string; the Display impl adds brackets).
    generic_args: Option<String>,
    /// Impl-block-level generic type parameters (type parameters only; lifetimes and
    /// const parameters are out of scope per IN-06).
    ///
    /// Allows cataloguing `impl<L, R, W> Trait for Foo<L, R, W>` where the impl block
    /// itself introduces generic parameters. Empty Vec when the trait impl is not generic
    /// at the impl-block level (the common case).
    ///
    /// The infrastructure codec serialises this field with `#[serde(default,
    /// skip_serializing_if = "Vec::is_empty")]` so existing catalogue JSON that lacks the
    /// field loads unchanged (CN-01 backward-compatible design).
    pub impl_generics: Vec<MethodGenericParam>,
    /// Impl-block-level where-clause predicates applied to `impl_generics`.
    ///
    /// Allows cataloguing constraints such as `where L: Send` on an impl block.
    /// Empty Vec when there are no impl-level where predicates.
    ///
    /// The infrastructure codec serialises this field with `#[serde(default,
    /// skip_serializing_if = "Vec::is_empty")]` so existing catalogue JSON that lacks the
    /// field loads unchanged (CN-01 backward-compatible design).
    pub impl_where_predicates: Vec<WherePredicateDecl>,
    /// The type this impl block targets, for cross-crate trait implementations.
    ///
    /// Declares the concrete type path that appears after `for` in the impl header,
    /// e.g. `"ExternalCrate::ExternalType"` for
    /// `impl MyTrait for ExternalCrate::ExternalType`. When `None`, the impl targets
    /// the type declared in the parent `TypeEntry` (the common case for same-crate impls).
    ///
    /// The infrastructure codec serialises this field with `#[serde(default,
    /// skip_serializing_if = "Option::is_none")]` so existing catalogue JSON that lacks the
    /// field loads unchanged with `None` (CN-01 backward-compatible design, IN-13).
    pub for_: Option<String>,
}

impl TraitImplDeclV2 {
    /// Creates a new `TraitImplDeclV2` without generic args.
    ///
    /// `impl_generics` and `impl_where_predicates` default to empty Vec.
    /// `for_` defaults to `None`.
    #[must_use]
    pub fn new(trait_name: TraitName, origin_crate: CrateName) -> Self {
        Self {
            trait_name,
            origin_crate,
            generic_args: None,
            impl_generics: vec![],
            impl_where_predicates: vec![],
            for_: None,
        }
    }

    /// Creates a new `TraitImplDeclV2` with explicit generic args.
    ///
    /// Use this constructor when the catalogue must distinguish multiple impls of
    /// the same trait on the same type (e.g. `From<CatalogueLoaderError>` vs
    /// `From<ContractMapWriterError>` generated by thiserror `#[from]`).
    ///
    /// The `generic_args` string is the raw type argument **without** surrounding
    /// angle brackets. The Display impl wraps it automatically: `"CatalogueLoaderError"`
    /// renders as `From<CatalogueLoaderError>`.
    ///
    /// `impl_generics` and `impl_where_predicates` default to empty Vec.
    ///
    /// # Errors
    ///
    /// Returns [`GenericArgsError::Empty`] when `generic_args` is empty or
    /// whitespace-only after trimming.
    ///
    /// Returns [`GenericArgsError::StartsWithAngleBracket`] when `generic_args`
    /// starts with `<` — this indicates the caller has already wrapped the type
    /// in angle brackets (e.g. `"<T>"` instead of `"T"`).
    ///
    /// Returns [`GenericArgsError::UnbalancedAngleBrackets`] when the number of
    /// `<` characters does not equal the number of `>` characters — an unbalanced
    /// string would produce a broken identity key when wrapped by the Display impl.
    pub fn new_with_generic_args(
        trait_name: TraitName,
        origin_crate: CrateName,
        generic_args: String,
    ) -> Result<Self, GenericArgsError> {
        let trimmed = generic_args.trim();
        if trimmed.is_empty() {
            return Err(GenericArgsError::Empty);
        }
        if trimmed.starts_with('<') {
            return Err(GenericArgsError::StartsWithAngleBracket);
        }
        if !Self::angle_brackets_are_valid(trimmed) {
            return Err(GenericArgsError::UnbalancedAngleBrackets);
        }
        Ok(Self {
            trait_name,
            origin_crate,
            generic_args: Some(trimmed.to_owned()),
            impl_generics: vec![],
            impl_where_predicates: vec![],
            for_: None,
        })
    }

    /// Returns the generic args string, if any.
    ///
    /// The returned string does **not** include surrounding angle brackets —
    /// the Display impl adds them automatically.
    #[must_use]
    pub fn generic_args(&self) -> Option<&str> {
        self.generic_args.as_deref()
    }

    /// Returns `true` when the angle brackets in `s` are properly nested.
    ///
    /// Valid generic arg strings must satisfy all of the following:
    /// - Depth never goes negative (no `>` appears before its matching `<`).
    /// - Depth returns to zero exactly at the end (all opens are closed).
    /// - Depth does not return to zero before the end of the string — a premature
    ///   close (e.g. `"Vec<T><U>"`) would create separate bracket groups, which
    ///   are not valid Rust type expressions and would produce malformed identity keys.
    ///
    /// Note: strings with no brackets at all (e.g. plain type names) are always
    /// valid — depth stays at zero throughout.
    fn angle_brackets_are_valid(s: &str) -> bool {
        let has_any_bracket = s.contains('<') || s.contains('>');
        let mut depth: i32 = 0;
        let chars: Vec<char> = s.chars().collect();
        for (i, &c) in chars.iter().enumerate() {
            match c {
                '<' => depth += 1,
                '>' => {
                    depth -= 1;
                    if depth < 0 {
                        return false;
                    }
                    // Depth returning to zero before the end means separate groups.
                    if has_any_bracket && depth == 0 {
                        // Peek ahead: if there are more `<` or `>` chars remaining,
                        // this is a premature close (e.g. "Vec<T><U>").
                        let has_more_brackets = chars
                            .get(i + 1..)
                            .is_some_and(|rest| rest.iter().any(|&rc| rc == '<' || rc == '>'));
                        if has_more_brackets {
                            return false;
                        }
                    }
                }
                _ => {}
            }
        }
        depth == 0
    }
}

impl fmt::Display for TraitImplDeclV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.generic_args() {
            Some(args) => write!(f, "{}<{}>", self.trait_name.as_str(), args),
            None => write!(f, "{}", self.trait_name.as_str()),
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

    #[test]
    fn test_trait_impl_decl_v2_new_stores_trait_name_and_origin_crate() {
        let trait_name = TraitName::new("UserRepository").unwrap();
        let origin_crate = CrateName::new("domain").unwrap();
        let decl = TraitImplDeclV2::new(trait_name.clone(), origin_crate.clone());
        assert_eq!(decl.trait_name, trait_name);
        assert_eq!(decl.origin_crate, origin_crate);
        assert_eq!(decl.generic_args(), None);
    }

    #[test]
    fn test_trait_impl_decl_v2_for_std_trait() {
        let trait_name = TraitName::new("Display").unwrap();
        let origin_crate = CrateName::new("std").unwrap();
        let decl = TraitImplDeclV2::new(trait_name.clone(), origin_crate.clone());
        assert_eq!(decl.trait_name, trait_name);
        assert_eq!(decl.origin_crate, origin_crate);
        assert_eq!(decl.generic_args(), None);
    }

    #[test]
    fn test_trait_impl_decl_v2_for_cross_crate_trait() {
        let trait_name = TraitName::new("Serialize").unwrap();
        let origin_crate = CrateName::new("serde").unwrap();
        let decl = TraitImplDeclV2::new(trait_name.clone(), origin_crate.clone());
        assert_eq!(decl.trait_name.as_str(), "Serialize");
        assert_eq!(decl.origin_crate.as_str(), "serde");
    }

    #[test]
    fn test_trait_impl_decl_v2_has_generic_args_field() {
        // Struct literal construction is no longer possible (field is private).
        // Use the constructor and verify via the accessor.
        let decl = TraitImplDeclV2::new_with_generic_args(
            TraitName::new("From").unwrap(),
            CrateName::new("core").unwrap(),
            "CatalogueLoaderError".to_string(),
        )
        .unwrap();
        assert_eq!(decl.trait_name.as_str(), "From");
        assert_eq!(decl.origin_crate.as_str(), "core");
        assert_eq!(decl.generic_args(), Some("CatalogueLoaderError"));
    }

    #[test]
    fn test_trait_impl_decl_v2_new_with_generic_args_stores_all_three_fields() {
        let trait_name = TraitName::new("From").unwrap();
        let origin_crate = CrateName::new("core").unwrap();
        let decl = TraitImplDeclV2::new_with_generic_args(
            trait_name.clone(),
            origin_crate.clone(),
            "CatalogueLoaderError".to_string(),
        )
        .unwrap();
        assert_eq!(decl.trait_name, trait_name);
        assert_eq!(decl.origin_crate, origin_crate);
        assert_eq!(decl.generic_args(), Some("CatalogueLoaderError"));
    }

    #[test]
    fn test_trait_impl_decl_v2_equality_by_all_three_fields() {
        let a = TraitImplDeclV2::new_with_generic_args(
            TraitName::new("From").unwrap(),
            CrateName::new("core").unwrap(),
            "CatalogueLoaderError".to_string(),
        )
        .unwrap();
        let b = TraitImplDeclV2::new_with_generic_args(
            TraitName::new("From").unwrap(),
            CrateName::new("core").unwrap(),
            "CatalogueLoaderError".to_string(),
        )
        .unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn test_trait_impl_decl_v2_different_generic_args_are_not_equal() {
        let a = TraitImplDeclV2::new_with_generic_args(
            TraitName::new("From").unwrap(),
            CrateName::new("core").unwrap(),
            "CatalogueLoaderError".to_string(),
        )
        .unwrap();
        let b = TraitImplDeclV2::new_with_generic_args(
            TraitName::new("From").unwrap(),
            CrateName::new("core").unwrap(),
            "ContractMapWriterError".to_string(),
        )
        .unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn test_trait_impl_decl_v2_some_generic_args_not_equal_to_none() {
        let a = TraitImplDeclV2::new_with_generic_args(
            TraitName::new("From").unwrap(),
            CrateName::new("core").unwrap(),
            "CatalogueLoaderError".to_string(),
        )
        .unwrap();
        let b =
            TraitImplDeclV2::new(TraitName::new("From").unwrap(), CrateName::new("core").unwrap());
        assert_ne!(a, b);
    }

    #[test]
    fn test_trait_impl_decl_v2_display_without_generic_args() {
        let decl = TraitImplDeclV2::new(
            TraitName::new("Display").unwrap(),
            CrateName::new("core").unwrap(),
        );
        assert_eq!(decl.to_string(), "Display");
    }

    #[test]
    fn test_trait_impl_decl_v2_display_with_generic_args() {
        let decl = TraitImplDeclV2::new_with_generic_args(
            TraitName::new("From").unwrap(),
            CrateName::new("core").unwrap(),
            "CatalogueLoaderError".to_string(),
        )
        .unwrap();
        assert_eq!(decl.to_string(), "From<CatalogueLoaderError>");
    }

    // -------------------------------------------------------------------
    // Validation: new_with_generic_args rejects illegal values
    // -------------------------------------------------------------------

    #[test]
    fn test_new_with_generic_args_with_empty_string_returns_empty_error() {
        let result = TraitImplDeclV2::new_with_generic_args(
            TraitName::new("From").unwrap(),
            CrateName::new("core").unwrap(),
            String::new(),
        );
        assert_eq!(result, Err(GenericArgsError::Empty));
    }

    #[test]
    fn test_new_with_generic_args_with_whitespace_only_returns_empty_error() {
        let result = TraitImplDeclV2::new_with_generic_args(
            TraitName::new("From").unwrap(),
            CrateName::new("core").unwrap(),
            "   ".to_string(),
        );
        assert_eq!(result, Err(GenericArgsError::Empty));
    }

    #[test]
    fn test_new_with_generic_args_with_bracketed_string_returns_error() {
        // "<T>" starts with `<`, so it is rejected — caller should pass "T" instead.
        let result = TraitImplDeclV2::new_with_generic_args(
            TraitName::new("From").unwrap(),
            CrateName::new("core").unwrap(),
            "<T>".to_string(),
        );
        assert_eq!(result, Err(GenericArgsError::StartsWithAngleBracket));
    }

    #[test]
    fn test_new_with_generic_args_with_leading_angle_bracket_returns_error() {
        let result = TraitImplDeclV2::new_with_generic_args(
            TraitName::new("From").unwrap(),
            CrateName::new("core").unwrap(),
            "<CatalogueLoaderError".to_string(),
        );
        assert_eq!(result, Err(GenericArgsError::StartsWithAngleBracket));
    }

    #[test]
    fn test_new_with_generic_args_with_trailing_angle_bracket_returns_unbalanced_error() {
        // "CatalogueLoaderError>" has one unmatched `>` — would produce broken Display key.
        let result = TraitImplDeclV2::new_with_generic_args(
            TraitName::new("From").unwrap(),
            CrateName::new("core").unwrap(),
            "CatalogueLoaderError>".to_string(),
        );
        assert_eq!(result, Err(GenericArgsError::UnbalancedAngleBrackets));
    }

    #[test]
    fn test_new_with_generic_args_with_double_close_bracket_returns_unbalanced_error() {
        // "Vec<T>>" has one `<` but two `>` — unbalanced.
        let result = TraitImplDeclV2::new_with_generic_args(
            TraitName::new("From").unwrap(),
            CrateName::new("core").unwrap(),
            "Vec<T>>".to_string(),
        );
        assert_eq!(result, Err(GenericArgsError::UnbalancedAngleBrackets));
    }

    #[test]
    fn test_new_with_generic_args_with_plain_type_name_succeeds() {
        let decl = TraitImplDeclV2::new_with_generic_args(
            TraitName::new("From").unwrap(),
            CrateName::new("core").unwrap(),
            "T".to_string(),
        )
        .unwrap();
        assert_eq!(decl.generic_args(), Some("T"));
    }

    #[test]
    fn test_new_with_generic_args_with_nested_generics_succeeds() {
        // Properly nested generics like "Vec<i32>" are valid.
        let decl = TraitImplDeclV2::new_with_generic_args(
            TraitName::new("From").unwrap(),
            CrateName::new("core").unwrap(),
            "Vec<i32>".to_string(),
        )
        .unwrap();
        assert_eq!(decl.generic_args(), Some("Vec<i32>"));
        assert_eq!(decl.to_string(), "From<Vec<i32>>");
    }

    #[test]
    fn test_new_with_generic_args_with_misordered_brackets_returns_unbalanced_error() {
        // ">T<" has balanced count but inverted order — depth goes negative on first `>`.
        let result = TraitImplDeclV2::new_with_generic_args(
            TraitName::new("From").unwrap(),
            CrateName::new("core").unwrap(),
            ">T<".to_string(),
        );
        assert_eq!(result, Err(GenericArgsError::UnbalancedAngleBrackets));
    }

    #[test]
    fn test_new_with_generic_args_with_inverted_brackets_returns_unbalanced_error() {
        // ">U<" — depth goes negative on the first `>`, which is misordered.
        let result = TraitImplDeclV2::new_with_generic_args(
            TraitName::new("From").unwrap(),
            CrateName::new("core").unwrap(),
            ">U<".to_string(),
        );
        assert_eq!(result, Err(GenericArgsError::UnbalancedAngleBrackets));
    }

    #[test]
    fn test_new_with_generic_args_with_two_separate_generic_groups_returns_unbalanced_error() {
        // "Vec<T><U>" closes back to depth 0 before end and then opens again — two separate
        // bracket groups. This would produce "From<Vec<T><U>>" which is not a valid Rust
        // trait impl and would create a malformed identity key.
        let result = TraitImplDeclV2::new_with_generic_args(
            TraitName::new("From").unwrap(),
            CrateName::new("core").unwrap(),
            "Vec<T><U>".to_string(),
        );
        assert_eq!(result, Err(GenericArgsError::UnbalancedAngleBrackets));
    }

    #[test]
    fn test_new_with_generic_args_trims_surrounding_whitespace() {
        let decl = TraitImplDeclV2::new_with_generic_args(
            TraitName::new("From").unwrap(),
            CrateName::new("core").unwrap(),
            "  CatalogueLoaderError  ".to_string(),
        )
        .unwrap();
        assert_eq!(decl.generic_args(), Some("CatalogueLoaderError"));
    }

    #[test]
    fn test_trait_impl_decl_v2_equality_by_both_fields_no_generic_args() {
        let a =
            TraitImplDeclV2::new(TraitName::new("Clone").unwrap(), CrateName::new("core").unwrap());
        let b =
            TraitImplDeclV2::new(TraitName::new("Clone").unwrap(), CrateName::new("core").unwrap());
        assert_eq!(a, b);
    }

    #[test]
    fn test_trait_impl_decl_v2_different_crates_are_not_equal() {
        let a =
            TraitImplDeclV2::new(TraitName::new("Debug").unwrap(), CrateName::new("std").unwrap());
        let b =
            TraitImplDeclV2::new(TraitName::new("Debug").unwrap(), CrateName::new("core").unwrap());
        assert_ne!(a, b);
    }

    #[test]
    fn test_trait_impl_decl_v2_different_trait_names_are_not_equal() {
        let a =
            TraitImplDeclV2::new(TraitName::new("Clone").unwrap(), CrateName::new("core").unwrap());
        let b =
            TraitImplDeclV2::new(TraitName::new("Copy").unwrap(), CrateName::new("core").unwrap());
        assert_ne!(a, b);
    }

    // -------------------------------------------------------------------
    // AC-06: impl_generics + impl_where_predicates fields (IN-06)
    // -------------------------------------------------------------------

    #[test]
    fn test_trait_impl_decl_v2_new_has_empty_impl_generics_by_default() {
        // `impl_generics` defaults to empty Vec (serde backward-compat via `#[serde(default)]`).
        let decl = TraitImplDeclV2::new(
            TraitName::new("MyTrait").unwrap(),
            CrateName::new("domain").unwrap(),
        );
        assert!(
            decl.impl_generics.is_empty(),
            "TraitImplDeclV2::new must initialise impl_generics to empty Vec"
        );
    }

    #[test]
    fn test_trait_impl_decl_v2_new_has_empty_impl_where_predicates_by_default() {
        // `impl_where_predicates` defaults to empty Vec (serde backward-compat via
        // `#[serde(default)]`).
        let decl = TraitImplDeclV2::new(
            TraitName::new("MyTrait").unwrap(),
            CrateName::new("domain").unwrap(),
        );
        assert!(
            decl.impl_where_predicates.is_empty(),
            "TraitImplDeclV2::new must initialise impl_where_predicates to empty Vec"
        );
    }

    #[test]
    fn test_trait_impl_decl_v2_impl_generics_and_where_predicates_for_generic_impl_block() {
        // AC-06: `impl<L, R, W> Trait for Foo<L, R, W> where L: Send` must be representable.
        use crate::tddd::catalogue_v2::identifiers::{ParamName, TypeRef};
        use crate::tddd::catalogue_v2::methods::{BoundOp, MethodGenericParam, WherePredicateDecl};

        let mut decl = TraitImplDeclV2::new(
            TraitName::new("MyTrait").unwrap(),
            CrateName::new("domain").unwrap(),
        );
        // impl_generics: L, R, W (type parameters on the impl block)
        decl.impl_generics = vec![
            MethodGenericParam { name: ParamName::new("L").unwrap(), bounds: vec![] },
            MethodGenericParam { name: ParamName::new("R").unwrap(), bounds: vec![] },
            MethodGenericParam { name: ParamName::new("W").unwrap(), bounds: vec![] },
        ];
        // impl_where_predicates: L: Send
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
        use crate::tddd::catalogue_v2::identifiers::ParamName;
        use crate::tddd::catalogue_v2::methods::MethodGenericParam;

        let base = TraitImplDeclV2::new(
            TraitName::new("Serialize").unwrap(),
            CrateName::new("serde").unwrap(),
        );
        let mut with_generics = base.clone();
        with_generics.impl_generics =
            vec![MethodGenericParam { name: ParamName::new("T").unwrap(), bounds: vec![] }];
        assert_ne!(base, with_generics, "impl_generics participates in TraitImplDeclV2 equality");
    }

    #[test]
    fn test_trait_impl_decl_v2_impl_where_predicates_participates_in_equality() {
        use crate::tddd::catalogue_v2::identifiers::TypeRef;
        use crate::tddd::catalogue_v2::methods::{BoundOp, WherePredicateDecl};

        let base = TraitImplDeclV2::new(
            TraitName::new("Serialize").unwrap(),
            CrateName::new("serde").unwrap(),
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

    // -------------------------------------------------------------------
    // AC-12 / IN-13: for_ field (Option<String>, default None)
    // -------------------------------------------------------------------

    #[test]
    fn test_trait_impl_decl_v2_new_has_none_for_by_default() {
        // `for_` must default to None so catalogues that predate IN-13 stay backward-compat.
        let decl = TraitImplDeclV2::new(
            TraitName::new("Iterator").unwrap(),
            CrateName::new("core").unwrap(),
        );
        assert!(decl.for_.is_none(), "TraitImplDeclV2::new must initialise for_ to None");
    }

    #[test]
    fn test_trait_impl_decl_v2_new_with_generic_args_has_none_for_by_default() {
        // `new_with_generic_args` must also default for_ to None.
        let decl = TraitImplDeclV2::new_with_generic_args(
            TraitName::new("From").unwrap(),
            CrateName::new("core").unwrap(),
            "SomeError".to_string(),
        )
        .unwrap();
        assert!(
            decl.for_.is_none(),
            "TraitImplDeclV2::new_with_generic_args must initialise for_ to None"
        );
    }

    #[test]
    fn test_trait_impl_decl_v2_for_field_can_be_set_to_some() {
        // `for_` can be set after construction (public field).
        let mut decl =
            TraitImplDeclV2::new(TraitName::new("Debug").unwrap(), CrateName::new("core").unwrap());
        decl.for_ = Some("ExternalType".to_string());
        assert_eq!(decl.for_.as_deref(), Some("ExternalType"));
    }

    #[test]
    fn test_trait_impl_decl_v2_for_field_participates_in_equality() {
        // for_ must be part of PartialEq: two decls differing only in for_ are not equal.
        let base =
            TraitImplDeclV2::new(TraitName::new("Clone").unwrap(), CrateName::new("core").unwrap());
        let mut with_for = base.clone();
        with_for.for_ = Some("ForeignType".to_string());
        assert_ne!(base, with_for, "for_ participates in TraitImplDeclV2 equality");
    }
}
