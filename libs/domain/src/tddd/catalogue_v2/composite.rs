//! Composite pattern and type kind enums for the catalogue v2 schema.
//!
//! Implements:
//! - [`CompositePattern`]: pattern encoding for `TypeKindV2::Struct` (Plain / TypestateState /
//!   Newtype). ADR 1 D3.
//! - [`TypeKindV2`]: language-level kind for `TypeEntry`. 3 payload-encoded variants:
//!   Struct / Enum / TypeAlias. ADR 1 D7.
//!
//! No serde derives — per ADR `knowledge/adr/2026-04-14-1531-domain-serde-ripout.md`,
//! the domain layer is serialization-free. The infrastructure codec (T003) handles JSON.

use crate::tddd::catalogue_v2::identifiers::{MethodName, TypeName, TypeRef};
use crate::tddd::catalogue_v2::variants::{FieldDecl, VariantDecl};

// ---------------------------------------------------------------------------
// CompositePattern — pattern for TypeKindV2::Struct
// ---------------------------------------------------------------------------

/// Pattern for `TypeKindV2::Struct` (ADR 1 D3 / D7).
///
/// Encodes the implementation pattern of a struct-kind type at the schema level.
/// Only `TypeKindV2::Struct` carries a `CompositePattern`; `Enum` and `TypeAlias`
/// cannot declare a pattern (enforced by schema structure, not runtime validation).
///
/// 3 variants:
/// - `Plain`: a plain struct with no special pattern.
/// - `TypestateState { of, transition_methods }`: a typestate state struct belonging
///   to typestate machine `of`, with `transition_methods` listing method names that
///   produce the next typestate.
/// - `Newtype { inner }`: a newtype wrapper around an inner type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompositePattern {
    /// A plain struct with no special pattern.
    Plain,
    /// A typestate state struct.
    ///
    /// - `of`: the `TypeName` of the typestate machine this state belongs to.
    /// - `transition_methods`: method names that produce the next typestate (ADR 1 D3).
    TypestateState {
        /// The typestate machine this state belongs to.
        of: TypeName,
        /// Method names that transition to the next typestate state.
        transition_methods: Vec<MethodName>,
    },
    /// A newtype wrapper around an inner type.
    ///
    /// - `inner`: the wrapped type reference (generics-inclusive).
    Newtype {
        /// The wrapped inner type.
        inner: TypeRef,
    },
}

// ---------------------------------------------------------------------------
// TypeKindV2 — language-level kind for TypeEntry
// ---------------------------------------------------------------------------

/// Language-level kind for `TypeEntry`.
///
/// Named `TypeKindV2` to avoid collision with `domain::schema::TypeKind` (which is
/// scheduled for deletion in T008). The `catalogue_v2` module uses short names as
/// BTreeMap keys, so both would appear as `TypeKind` in the catalogue; the V2 suffix
/// distinguishes them at the catalogue level (ADR 1 D7; see `domain-types.json`
/// `TypeKindV2.informal_grounds`).
///
/// 3 payload-encoded variants. The Pattern × Kind constraint is encoded at schema
/// structure level: only `Struct` carries a `CompositePattern` (ADR 1 D3).
///
/// - `Struct { pattern, fields }`: a composite type. `pattern` encodes the
///   implementation pattern; `fields` contains the named fields.
/// - `Enum { variants }`: a sum type with a list of variants.
/// - `TypeAlias { target }`: a type alias for `target`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeKindV2 {
    /// A composite struct type.
    Struct {
        /// The implementation pattern (Plain / TypestateState / Newtype).
        pattern: CompositePattern,
        /// Named struct fields.
        fields: Vec<FieldDecl>,
    },
    /// A sum / enum type.
    Enum {
        /// The enum variants.
        variants: Vec<VariantDecl>,
    },
    /// A type alias.
    TypeAlias {
        /// The target type this alias refers to.
        target: TypeRef,
    },
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::tddd::catalogue_v2::identifiers::{FieldName, VariantName};
    use crate::tddd::catalogue_v2::variants::VariantPayload;

    // -----------------------------------------------------------------------
    // CompositePattern
    // -----------------------------------------------------------------------

    #[test]
    fn test_composite_pattern_plain_struct_roundtrip() {
        let pattern = CompositePattern::Plain;
        assert!(matches!(pattern, CompositePattern::Plain));
    }

    #[test]
    fn test_composite_pattern_typestate_state_holds_of_and_transition_methods() {
        let of = TypeName::new("ReviewState").unwrap();
        let m1 = MethodName::new("approve").unwrap();
        let m2 = MethodName::new("reject").unwrap();
        let pattern = CompositePattern::TypestateState {
            of: of.clone(),
            transition_methods: vec![m1.clone(), m2.clone()],
        };
        match &pattern {
            CompositePattern::TypestateState { of: p_of, transition_methods } => {
                assert_eq!(p_of, &of);
                assert_eq!(transition_methods, &[m1, m2]);
            }
            _ => panic!("expected TypestateState"),
        }
    }

    #[test]
    fn test_composite_pattern_newtype_holds_inner_type_ref() {
        let inner = TypeRef::new("String").unwrap();
        let pattern = CompositePattern::Newtype { inner: inner.clone() };
        match &pattern {
            CompositePattern::Newtype { inner: p_inner } => assert_eq!(p_inner, &inner),
            _ => panic!("expected Newtype"),
        }
    }

    #[test]
    fn test_composite_pattern_newtype_with_generic_inner_succeeds() {
        let inner = TypeRef::new("Vec<UserId>").unwrap();
        let pattern = CompositePattern::Newtype { inner };
        assert!(matches!(pattern, CompositePattern::Newtype { .. }));
    }

    // -----------------------------------------------------------------------
    // TypeKindV2
    // -----------------------------------------------------------------------

    #[test]
    fn test_type_kind_v2_struct_variant_holds_pattern_and_fields() {
        let pattern = CompositePattern::Plain;
        let field_name = FieldName::new("email").unwrap();
        let field_ty = TypeRef::new("String").unwrap();
        let fields = vec![FieldDecl { name: field_name.clone(), ty: field_ty.clone() }];
        let kind = TypeKindV2::Struct { pattern: CompositePattern::Plain, fields: fields.clone() };
        match &kind {
            TypeKindV2::Struct { pattern: k_pattern, fields: k_fields } => {
                assert_eq!(k_pattern, &pattern);
                assert_eq!(k_fields.len(), 1);
                assert_eq!(k_fields[0].name, field_name);
                assert_eq!(k_fields[0].ty, field_ty);
            }
            _ => panic!("expected Struct"),
        }
    }

    #[test]
    fn test_type_kind_v2_enum_variant_holds_variants() {
        let variant_name = VariantName::new("Add").unwrap();
        let variants =
            vec![VariantDecl { name: variant_name.clone(), payload: VariantPayload::Unit }];
        let kind = TypeKindV2::Enum { variants: variants.clone() };
        match &kind {
            TypeKindV2::Enum { variants: k_variants } => {
                assert_eq!(k_variants.len(), 1);
                assert_eq!(k_variants[0].name, variant_name);
            }
            _ => panic!("expected Enum"),
        }
    }

    #[test]
    fn test_type_kind_v2_type_alias_variant_holds_target() {
        let target = TypeRef::new("Result<User, DomainError>").unwrap();
        let kind = TypeKindV2::TypeAlias { target: target.clone() };
        match &kind {
            TypeKindV2::TypeAlias { target: k_target } => assert_eq!(k_target, &target),
            _ => panic!("expected TypeAlias"),
        }
    }

    #[test]
    fn test_type_kind_v2_struct_with_typestate_pattern_succeeds() {
        let of = TypeName::new("ReviewMachine").unwrap();
        let m = MethodName::new("start").unwrap();
        let pattern = CompositePattern::TypestateState { of, transition_methods: vec![m] };
        let kind = TypeKindV2::Struct { pattern, fields: vec![] };
        assert!(matches!(kind, TypeKindV2::Struct { .. }));
    }

    #[test]
    fn test_type_kind_v2_enum_with_empty_variants_succeeds() {
        // An enum with no declared variants is structurally valid (empty schema declaration).
        let kind = TypeKindV2::Enum { variants: vec![] };
        assert!(matches!(kind, TypeKindV2::Enum { .. }));
    }

    #[test]
    fn test_type_kind_v2_struct_only_variant_carries_pattern() {
        // Compile-time enforcement: TypeKindV2::Enum and TypeKindV2::TypeAlias do NOT have
        // a `pattern` field — the enum-first design makes Pattern × Kind constraint structural.
        // This test documents that only Struct carries pattern by confirming match arms.
        let struct_kind = TypeKindV2::Struct { pattern: CompositePattern::Plain, fields: vec![] };
        let enum_kind = TypeKindV2::Enum { variants: vec![] };
        let alias_kind = TypeKindV2::TypeAlias { target: TypeRef::new("u32").unwrap() };

        assert!(matches!(struct_kind, TypeKindV2::Struct { pattern: CompositePattern::Plain, .. }));
        assert!(matches!(enum_kind, TypeKindV2::Enum { .. }));
        assert!(matches!(alias_kind, TypeKindV2::TypeAlias { .. }));
    }
}
