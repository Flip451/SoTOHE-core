//! Variant and field declaration types for the catalogue v2 schema.
//!
//! Implements:
//! - [`FieldDecl`]: named struct field declaration (`name: FieldName`, `ty: TypeRef`). ADR 1 D7.
//! - [`VariantPayload`]: enum variant payload (Unit / Tuple / Struct). ADR 1 D12.
//! - [`VariantDecl`]: enum variant declaration (`name: VariantName`, `payload: VariantPayload`).
//!   ADR 1 D12.
//!
//! No serde derives — per ADR `knowledge/adr/2026-04-14-1531-domain-serde-ripout.md`,
//! the domain layer is serialization-free. The infrastructure codec (T003) handles JSON
//! including the `serde default = Unit` for the `VariantDecl::payload` field.

use crate::tddd::catalogue_v2::identifiers::{FieldName, TypeRef, VariantName};

// ---------------------------------------------------------------------------
// FieldDecl — named struct field declaration
// ---------------------------------------------------------------------------

/// Named struct field declaration.
///
/// Used in [`crate::tddd::catalogue_v2::composite::TypeKindV2::Struct::fields`]
/// and [`VariantPayload::Struct`] (ADR 1 D7).
///
/// Both fields use newtype wrappers from the `identifiers` module to enforce
/// the distinction between field names and type references at compile time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldDecl {
    /// The field name.
    pub name: FieldName,
    /// The field type (generics-inclusive type reference string).
    pub ty: TypeRef,
}

impl FieldDecl {
    /// Creates a new `FieldDecl`.
    #[must_use]
    pub fn new(name: FieldName, ty: TypeRef) -> Self {
        Self { name, ty }
    }
}

// ---------------------------------------------------------------------------
// VariantPayload — enum variant payload encoding
// ---------------------------------------------------------------------------

/// Payload for [`VariantDecl`].
///
/// Supersedes `EnumVariantDeclaration::payload_types: Vec<String>` from the old
/// sibling ADR 2026-05-02-0316, by encoding the semantic distinction between
/// unit / tuple / struct variants at the schema structure level (ADR 1 D12).
///
/// The infrastructure codec (T003) implements `serde default = Unit` so that
/// omitting the `payload` field in JSON is decoded as `VariantPayload::Unit`.
///
/// 3 variants:
/// - `Unit`: unit variant — no payload (e.g. `None`, `Add`).
/// - `Tuple(Vec<TypeRef>)`: tuple variant — positional unnamed fields.
/// - `Struct(Vec<FieldDecl>)`: struct variant — named fields.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum VariantPayload {
    /// Unit variant — no payload (e.g. `None`, `Ok` without a value).
    ///
    /// This is the default when the `payload` field is absent in JSON
    /// (handled by the infrastructure codec's `serde default`).
    #[default]
    Unit,
    /// Tuple variant — unnamed positional fields (e.g. `Some(T)`, `Ok(T)`).
    Tuple(Vec<TypeRef>),
    /// Struct variant — named fields (e.g. `Struct { field: Type, ... }`).
    Struct(Vec<FieldDecl>),
}

// ---------------------------------------------------------------------------
// VariantDecl — enum variant declaration
// ---------------------------------------------------------------------------

/// Enum variant declaration.
///
/// Replaces `EnumVariantDeclaration` (ADR 1 D12). Encodes the variant name
/// and its payload (Unit / Tuple / Struct) so that the semantic difference is
/// captured at parse time by schema structure.
///
/// The `payload` field defaults to `VariantPayload::Unit` when absent in the
/// JSON representation (handled by the infrastructure codec's `serde default`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VariantDecl {
    /// The variant name.
    pub name: VariantName,
    /// The variant payload (defaults to `Unit` when omitted in JSON).
    pub payload: VariantPayload,
}

impl VariantDecl {
    /// Creates a new `VariantDecl` with a `Unit` payload (the most common case).
    #[must_use]
    pub fn unit(name: VariantName) -> Self {
        Self { name, payload: VariantPayload::Unit }
    }

    /// Creates a new `VariantDecl` with a `Tuple` payload.
    #[must_use]
    pub fn tuple(name: VariantName, fields: Vec<TypeRef>) -> Self {
        Self { name, payload: VariantPayload::Tuple(fields) }
    }

    /// Creates a new `VariantDecl` with a `Struct` payload.
    #[must_use]
    pub fn struct_variant(name: VariantName, fields: Vec<FieldDecl>) -> Self {
        Self { name, payload: VariantPayload::Struct(fields) }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // FieldDecl
    // -----------------------------------------------------------------------

    #[test]
    fn test_field_decl_new_stores_name_and_ty() {
        let name = FieldName::new("email").unwrap();
        let ty = TypeRef::new("String").unwrap();
        let decl = FieldDecl::new(name.clone(), ty.clone());
        assert_eq!(decl.name, name);
        assert_eq!(decl.ty, ty);
    }

    #[test]
    fn test_field_decl_with_generic_type_ref_succeeds() {
        let name = FieldName::new("items").unwrap();
        let ty = TypeRef::new("Vec<OrderItem>").unwrap();
        let decl = FieldDecl { name, ty };
        assert_eq!(decl.ty.as_str(), "Vec<OrderItem>");
    }

    #[test]
    fn test_field_decl_type_contexts_are_distinct_at_compile_time() {
        // Attempting to pass a FieldName where a TypeRef is expected (or vice versa)
        // would be a compile error. This test documents the invariant with runtime checks.
        let name = FieldName::new("count").unwrap();
        let ty = TypeRef::new("u32").unwrap();
        // name and ty are different types — passing them to wrong argument position
        // is a compile error. We verify the types hold their values correctly.
        assert_eq!(name.as_str(), "count");
        assert_eq!(ty.as_str(), "u32");
    }

    #[test]
    fn test_field_decl_equality_by_both_name_and_ty() {
        let name = FieldName::new("id").unwrap();
        let ty = TypeRef::new("UserId").unwrap();
        let a = FieldDecl::new(name.clone(), ty.clone());
        let b = FieldDecl::new(name, ty);
        assert_eq!(a, b);
    }

    // -----------------------------------------------------------------------
    // VariantPayload
    // -----------------------------------------------------------------------

    #[test]
    fn test_variant_payload_default_is_unit() {
        assert_eq!(VariantPayload::default(), VariantPayload::Unit);
    }

    #[test]
    fn test_variant_payload_unit_has_no_data() {
        let payload = VariantPayload::Unit;
        assert!(matches!(payload, VariantPayload::Unit));
    }

    #[test]
    fn test_variant_payload_tuple_with_single_type_ref() {
        let ty = TypeRef::new("String").unwrap();
        let payload = VariantPayload::Tuple(vec![ty.clone()]);
        match &payload {
            VariantPayload::Tuple(fields) => {
                assert_eq!(fields.len(), 1);
                assert_eq!(fields[0], ty);
            }
            _ => panic!("expected Tuple"),
        }
    }

    #[test]
    fn test_variant_payload_tuple_with_multiple_type_refs() {
        let t1 = TypeRef::new("String").unwrap();
        let t2 = TypeRef::new("u32").unwrap();
        let payload = VariantPayload::Tuple(vec![t1.clone(), t2.clone()]);
        match &payload {
            VariantPayload::Tuple(fields) => {
                assert_eq!(fields.len(), 2);
                assert_eq!(fields[0], t1);
                assert_eq!(fields[1], t2);
            }
            _ => panic!("expected Tuple"),
        }
    }

    #[test]
    fn test_variant_payload_tuple_with_generic_type_ref() {
        let ty = TypeRef::new("Result<User, DomainError>").unwrap();
        let payload = VariantPayload::Tuple(vec![ty]);
        assert!(matches!(payload, VariantPayload::Tuple(_)));
    }

    #[test]
    fn test_variant_payload_struct_with_named_fields() {
        let name = FieldName::new("code").unwrap();
        let ty = TypeRef::new("String").unwrap();
        let field = FieldDecl::new(name.clone(), ty.clone());
        let payload = VariantPayload::Struct(vec![field]);
        match &payload {
            VariantPayload::Struct(fields) => {
                assert_eq!(fields.len(), 1);
                assert_eq!(fields[0].name, name);
                assert_eq!(fields[0].ty, ty);
            }
            _ => panic!("expected Struct"),
        }
    }

    #[test]
    fn test_variant_payload_struct_with_multiple_fields() {
        let f1 = FieldDecl::new(FieldName::new("id").unwrap(), TypeRef::new("u32").unwrap());
        let f2 = FieldDecl::new(FieldName::new("name").unwrap(), TypeRef::new("String").unwrap());
        let payload = VariantPayload::Struct(vec![f1, f2]);
        match &payload {
            VariantPayload::Struct(fields) => assert_eq!(fields.len(), 2),
            _ => panic!("expected Struct"),
        }
    }

    #[test]
    fn test_variant_payload_equality_unit_equals_unit() {
        assert_eq!(VariantPayload::Unit, VariantPayload::Unit);
    }

    #[test]
    fn test_variant_payload_equality_unit_not_equal_to_empty_tuple() {
        // VariantPayload::Unit and VariantPayload::Tuple(vec![]) are distinct variants.
        assert_ne!(VariantPayload::Unit, VariantPayload::Tuple(vec![]));
    }

    // -----------------------------------------------------------------------
    // VariantDecl
    // -----------------------------------------------------------------------

    #[test]
    fn test_variant_decl_unit_constructor_sets_unit_payload() {
        let name = VariantName::new("None").unwrap();
        let decl = VariantDecl::unit(name.clone());
        assert_eq!(decl.name, name);
        assert_eq!(decl.payload, VariantPayload::Unit);
    }

    #[test]
    fn test_variant_decl_tuple_constructor_sets_tuple_payload() {
        let name = VariantName::new("Some").unwrap();
        let ty = TypeRef::new("UserId").unwrap();
        let decl = VariantDecl::tuple(name.clone(), vec![ty.clone()]);
        assert_eq!(decl.name, name);
        match &decl.payload {
            VariantPayload::Tuple(fields) => {
                assert_eq!(fields.len(), 1);
                assert_eq!(fields[0], ty);
            }
            _ => panic!("expected Tuple payload"),
        }
    }

    #[test]
    fn test_variant_decl_struct_variant_constructor_sets_struct_payload() {
        let name = VariantName::new("InternalError").unwrap();
        let field =
            FieldDecl::new(FieldName::new("message").unwrap(), TypeRef::new("String").unwrap());
        let decl = VariantDecl::struct_variant(name.clone(), vec![field.clone()]);
        assert_eq!(decl.name, name);
        match &decl.payload {
            VariantPayload::Struct(fields) => {
                assert_eq!(fields.len(), 1);
                assert_eq!(fields[0], field);
            }
            _ => panic!("expected Struct payload"),
        }
    }

    #[test]
    fn test_variant_decl_payload_omission_defaults_to_unit_via_default() {
        // The domain type itself uses Default::default() for VariantPayload — this matches
        // what the infrastructure codec will produce when the JSON `payload` field is absent.
        let name = VariantName::new("Add").unwrap();
        let decl = VariantDecl { name: name.clone(), payload: VariantPayload::default() };
        assert_eq!(decl.payload, VariantPayload::Unit);
    }

    #[test]
    fn test_variant_decl_equality_by_name_and_payload() {
        let name = VariantName::new("Ok").unwrap();
        let ty = TypeRef::new("User").unwrap();
        let a = VariantDecl::tuple(name.clone(), vec![ty.clone()]);
        let b = VariantDecl::tuple(name, vec![ty]);
        assert_eq!(a, b);
    }
}
