//! Trait implementation declaration for the catalogue v2 schema.
//!
//! Implements:
//! - [`TraitImplDeclV2`]: identity-only trait implementation record (`trait_name`, `origin_crate`).
//!   No `methods` field — trait/impl signature alignment is delegated to the Rust compiler
//!   (ADR 1 D10 / CN-07).
//!
//! No serde derives — per ADR `knowledge/adr/2026-04-14-1531-domain-serde-ripout.md`,
//! the domain layer is serialization-free. The infrastructure codec (T003) handles JSON.

use crate::tddd::catalogue_v2::identifiers::{CrateName, TraitName};

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
/// Replaces the old `TraitImplDecl` type (which included methods).
///
/// Used in [`crate::tddd::catalogue_v2::entries::TypeEntry::trait_impls`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitImplDeclV2 {
    /// The short name of the implemented trait.
    pub trait_name: TraitName,
    /// The crate that defines the trait.
    pub origin_crate: CrateName,
}

impl TraitImplDeclV2 {
    /// Creates a new `TraitImplDeclV2`.
    #[must_use]
    pub fn new(trait_name: TraitName, origin_crate: CrateName) -> Self {
        Self { trait_name, origin_crate }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_trait_impl_decl_v2_new_stores_trait_name_and_origin_crate() {
        let trait_name = TraitName::new("UserRepository").unwrap();
        let origin_crate = CrateName::new("domain").unwrap();
        let decl = TraitImplDeclV2::new(trait_name.clone(), origin_crate.clone());
        assert_eq!(decl.trait_name, trait_name);
        assert_eq!(decl.origin_crate, origin_crate);
    }

    #[test]
    fn test_trait_impl_decl_v2_for_std_trait() {
        let trait_name = TraitName::new("Display").unwrap();
        let origin_crate = CrateName::new("std").unwrap();
        let decl = TraitImplDeclV2::new(trait_name.clone(), origin_crate.clone());
        assert_eq!(decl.trait_name, trait_name);
        assert_eq!(decl.origin_crate, origin_crate);
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
    fn test_trait_impl_decl_v2_has_no_methods_field() {
        // This test documents the identity-only design (ADR 1 D10 / CN-07).
        // The struct compiles fine with only trait_name and origin_crate — no methods field.
        let decl = TraitImplDeclV2 {
            trait_name: TraitName::new("Send").unwrap(),
            origin_crate: CrateName::new("core").unwrap(),
        };
        // Only 2 fields — no methods, no method count.
        let _ = &decl.trait_name;
        let _ = &decl.origin_crate;
    }

    #[test]
    fn test_trait_impl_decl_v2_equality_by_both_fields() {
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
}
