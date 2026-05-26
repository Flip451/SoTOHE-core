//! Type kind enum for the catalogue v2 schema.
//!
//! Implements:
//! - [`TypestateMarker`]: typestate membership marker, now carried by `StructKind` so that
//!   any struct shape (unit / tuple / plain) can be a typestate state.
//! - [`TypestateTransitions`]: transition specification for a typestate state.
//! - [`StructShape`]: the Rust-level structural form of a struct (Unit / Tuple / Plain).
//! - [`StructKind`]: groups the three struct shapes under a single type with an orthogonal
//!   optional `typestate` marker. ADR `knowledge/adr/2026-05-26-1002-typestate-struct-kind-orthogonal.md` D1.
//! - [`TypeKindV2`]: language-level kind for `TypeEntry`. 3 variants:
//!   `Struct(StructKind)`, `Enum`, `TypeAlias`.
//!
//! No serde derives — per ADR `knowledge/adr/2026-04-14-1531-domain-serde-ripout.md`,
//! the domain layer is serialization-free. The infrastructure codec handles JSON.
//!
//! ## Design rationale
//!
//! The old flat 5-variant design placed `TypestateMarker` only on `PlainStruct`, making it
//! impossible to declare a unit struct or tuple struct as a typestate state.  The new design
//! groups all three struct shapes into `StructKind { shape: StructShape, typestate }`, making
//! typestate membership expressible for any struct shape while keeping shape-specific
//! field constraints structurally unrepresentable (e.g. `StructShape::Unit` has no `fields`
//! payload — attaching fields to a unit struct is a compile-time impossibility).
//!
//! - `Struct(StructKind)`: any struct shape with an orthogonal typestate marker.
//! - `Enum { variants }`: sum type (unchanged).
//! - `TypeAlias { target }`: type alias (unchanged).

use crate::tddd::catalogue_v2::identifiers::{MethodName, TypeName, TypeRef};
use crate::tddd::catalogue_v2::variants::{FieldDecl, VariantDecl};

// ---------------------------------------------------------------------------
// TypestateTransitions — transition specification for a typestate state
// ---------------------------------------------------------------------------

/// Transition specification for a typestate state.
///
/// `transition_methods` holds the method names that produce the next typestate.
/// An empty list is valid (e.g. a terminal typestate state with no outgoing
/// transitions).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypestateTransitions {
    /// Method names that transition to the next typestate state.
    transition_methods: Vec<MethodName>,
}

impl TypestateTransitions {
    /// Creates a new `TypestateTransitions` with the given transition method names.
    #[must_use]
    pub fn new(transition_methods: Vec<MethodName>) -> Self {
        Self { transition_methods }
    }

    /// Returns the transition method names.
    #[must_use]
    pub fn transition_methods(&self) -> &[MethodName] {
        &self.transition_methods
    }
}

impl std::fmt::Display for TypestateTransitions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let methods: Vec<&str> = self.transition_methods.iter().map(|m| m.as_str()).collect();
        write!(f, "[{}]", methods.join(", "))
    }
}

// ---------------------------------------------------------------------------
// TypestateMarker — typestate membership marker carried by StructKind
// ---------------------------------------------------------------------------

/// Typestate membership marker carried by [`StructKind`].
///
/// When present (`typestate: Some(TypestateMarker { .. })`), the struct
/// (of any shape — unit, tuple, or plain) is a state in a typestate machine.
/// The `state_name` field names the typestate machine (the root type that the
/// states belong to), and `transitions` lists the methods that produce the next
/// state.
///
/// Fields are private with constructor and accessor methods.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypestateMarker {
    /// The name of the typestate machine this state belongs to.
    ///
    /// Must be non-empty and contain no leading/trailing whitespace.
    state_name: TypeName,
    /// Transition methods for this typestate state.
    transitions: TypestateTransitions,
}

impl TypestateMarker {
    /// Creates a new `TypestateMarker` from already-validated components.
    pub fn new(state_name: TypeName, transitions: TypestateTransitions) -> Self {
        Self { state_name, transitions }
    }

    /// Returns the typestate machine name (the root type that owns this state).
    #[must_use]
    pub fn state_name(&self) -> &TypeName {
        &self.state_name
    }

    /// Returns the transition specification for this typestate state.
    #[must_use]
    pub fn transitions(&self) -> &TypestateTransitions {
        &self.transitions
    }
}

impl std::fmt::Display for TypestateMarker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "typestate_state(of={}, transitions={})", self.state_name, self.transitions)
    }
}

// ---------------------------------------------------------------------------
// StructShape — Rust-level structural form of a struct
// ---------------------------------------------------------------------------

/// The Rust-level structural form of a struct.
///
/// Each variant carries only the fields valid for that struct form, making
/// illegal combinations structurally unrepresentable:
///
/// - `Unit`: a unit struct (`pub struct Foo;`). No fields are possible —
///   the structural absence of a `fields` payload makes `Unit + non-empty
///   fields` a compile-time impossibility (AC-05).
/// - `Tuple`: a tuple struct (`pub struct Foo(Bar, Baz)`). Uses `Vec<TypeRef>`
///   instead of `Vec<FieldDecl>` because tuple fields are positional and unnamed.
///   Fabricating names would be misleading (AC-06).
/// - `Plain`: a plain named-field struct (`pub struct Foo { bar: Bar }`).
///   Uses `Vec<FieldDecl>` which requires named fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructShape {
    /// A unit struct. No fields.
    Unit,
    /// A tuple struct (positional unnamed fields).
    ///
    /// Mirrors `rustdoc_types::StructKind::Tuple`.
    Tuple {
        /// Positional field types (unnamed; index-addressable as `.0`, `.1`, …).
        fields: Vec<TypeRef>,
        /// `true` when the struct has at least one private field that rustdoc
        /// omits from the documentation output.
        has_stripped_fields: bool,
    },
    /// A plain named-field struct.
    ///
    /// Mirrors `rustdoc_types::StructKind::Plain`.
    Plain {
        /// Named field declarations.
        fields: Vec<FieldDecl>,
        /// `true` when the struct has at least one private field that rustdoc
        /// omits from the documentation output.
        has_stripped_fields: bool,
    },
}

// ---------------------------------------------------------------------------
// StructKind — struct shape + orthogonal typestate marker
// ---------------------------------------------------------------------------

/// Groups the three struct shapes under a single type with an orthogonal
/// optional typestate membership marker.
///
/// `shape` encodes the Rust-level form of the struct. `typestate` is `Some`
/// when this struct is a state in a typestate machine, regardless of shape.
///
/// This design makes typestate membership expressible for any struct shape
/// (unit / tuple / plain) while keeping the structural constraints of each
/// shape intact — a `StructShape::Unit` still has no `fields` payload.
///
/// ADR: `knowledge/adr/2026-05-26-1002-typestate-struct-kind-orthogonal.md` D1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructKind {
    /// The Rust-level structural form of this struct.
    pub shape: StructShape,
    /// Optional typestate membership marker.
    ///
    /// `Some` when this struct is a state in the named typestate machine.
    /// `None` when this is a plain struct with no typestate relationship.
    /// Orthogonal to shape: any struct form can carry a typestate marker.
    pub typestate: Option<TypestateMarker>,
}

impl StructKind {
    /// Creates a new `StructKind` with the given shape and typestate marker.
    #[must_use]
    pub fn new(shape: StructShape, typestate: Option<TypestateMarker>) -> Self {
        Self { shape, typestate }
    }
}

// ---------------------------------------------------------------------------
// TypeKindV2 — language-level kind for TypeEntry
// ---------------------------------------------------------------------------

/// Language-level kind for `TypeEntry`.
///
/// Named `TypeKindV2` to avoid collision with `domain::schema::TypeKind` (which is
/// scheduled for deletion). The `catalogue_v2` module uses short names as
/// BTreeMap keys, so both would appear as `TypeKind` in the catalogue; the V2 suffix
/// distinguishes them at the catalogue level.
///
/// 3 variants. The former flat 5-variant design (UnitStruct / TupleStruct /
/// PlainStruct / Enum / TypeAlias) is replaced by grouping all struct shapes into
/// `Struct(StructKind)` so that typestate membership can be expressed for any
/// struct shape (ADR `2026-05-26-1002-typestate-struct-kind-orthogonal.md` D1).
///
/// - `Struct(StructKind)`: any struct shape with an orthogonal typestate marker.
/// - `Enum { variants }`: a sum / enum type.
/// - `TypeAlias { target }`: a type alias.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeKindV2 {
    /// Any struct shape, with an optional orthogonal typestate membership marker.
    ///
    /// The `StructKind` wrapper groups `StructShape` (Unit / Tuple / Plain) with
    /// `Option<TypestateMarker>` so that typestate membership is expressible
    /// regardless of shape.
    Struct(StructKind),
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
    use crate::tddd::catalogue_v2::variants::{FieldDecl, VariantPayload};

    // -----------------------------------------------------------------------
    // TypestateTransitions
    // -----------------------------------------------------------------------

    #[test]
    fn test_typestate_transitions_new_stores_methods() {
        let m1 = MethodName::new("approve").unwrap();
        let m2 = MethodName::new("reject").unwrap();
        let transitions = TypestateTransitions::new(vec![m1.clone(), m2.clone()]);
        assert_eq!(transitions.transition_methods(), &[m1, m2]);
    }

    #[test]
    fn test_typestate_transitions_empty_is_valid() {
        let transitions = TypestateTransitions::new(vec![]);
        assert!(transitions.transition_methods().is_empty());
    }

    #[test]
    fn test_typestate_transitions_display_formats_methods() {
        let m = MethodName::new("start").unwrap();
        let transitions = TypestateTransitions::new(vec![m]);
        assert_eq!(transitions.to_string(), "[start]");
    }

    // -----------------------------------------------------------------------
    // TypestateMarker
    // -----------------------------------------------------------------------

    #[test]
    fn test_typestate_marker_new_stores_state_name_and_transitions() {
        let state_name = TypeName::new("ReviewState").unwrap();
        let m = MethodName::new("approve").unwrap();
        let transitions = TypestateTransitions::new(vec![m.clone()]);
        let marker = TypestateMarker::new(state_name.clone(), transitions.clone());
        assert_eq!(marker.state_name(), &state_name);
        assert_eq!(marker.transitions().transition_methods(), &[m]);
    }

    #[test]
    fn test_typestate_marker_display_includes_state_name_and_transitions() {
        let state_name = TypeName::new("Fsm").unwrap();
        let transitions = TypestateTransitions::new(vec![]);
        let marker = TypestateMarker::new(state_name, transitions);
        let s = marker.to_string();
        assert!(s.contains("typestate_state(of=Fsm"), "display: {s}");
    }

    // -----------------------------------------------------------------------
    // Helper: build a TypestateMarker for tests
    // -----------------------------------------------------------------------

    fn make_marker(state_name: &str, methods: &[&str]) -> TypestateMarker {
        let name = TypeName::new(state_name).unwrap();
        let ms = methods.iter().map(|m| MethodName::new(*m).unwrap()).collect();
        TypestateMarker::new(name, TypestateTransitions::new(ms))
    }

    // -----------------------------------------------------------------------
    // StructShape — Unit (AC-05: no fields structurally)
    // -----------------------------------------------------------------------

    #[test]
    fn test_struct_shape_unit_has_no_fields_structurally() {
        // StructShape::Unit has no `fields` payload — attaching fields is impossible at the type level.
        let shape = StructShape::Unit;
        assert!(matches!(shape, StructShape::Unit));
        // There is no `shape.fields` accessor because Unit has no fields payload.
    }

    // -----------------------------------------------------------------------
    // StructShape — Tuple (AC-06: no named fields structurally)
    // -----------------------------------------------------------------------

    #[test]
    fn test_struct_shape_tuple_holds_type_refs_not_field_decls() {
        // StructShape::Tuple uses Vec<TypeRef>, not Vec<FieldDecl> — named fields cannot be attached.
        let field_ty = TypeRef::new("Uuid").unwrap();
        let shape =
            StructShape::Tuple { fields: vec![field_ty.clone()], has_stripped_fields: false };
        match &shape {
            StructShape::Tuple { fields, has_stripped_fields } => {
                assert_eq!(fields.len(), 1);
                assert_eq!(fields[0], field_ty);
                assert!(!has_stripped_fields);
            }
            _ => panic!("expected Tuple"),
        }
    }

    #[test]
    fn test_struct_shape_tuple_with_stripped_fields() {
        let shape = StructShape::Tuple { fields: vec![], has_stripped_fields: true };
        assert!(matches!(shape, StructShape::Tuple { has_stripped_fields: true, .. }));
    }

    // -----------------------------------------------------------------------
    // StructShape — Plain
    // -----------------------------------------------------------------------

    #[test]
    fn test_struct_shape_plain_holds_named_field_decls() {
        let field_name = FieldName::new("email").unwrap();
        let field_ty = TypeRef::new("String").unwrap();
        let fields = vec![FieldDecl::new(field_name.clone(), field_ty.clone())];
        let shape = StructShape::Plain { fields: fields.clone(), has_stripped_fields: false };
        match &shape {
            StructShape::Plain { fields: k_fields, has_stripped_fields } => {
                assert_eq!(k_fields.len(), 1);
                assert_eq!(k_fields[0].name, field_name);
                assert!(!has_stripped_fields);
            }
            _ => panic!("expected Plain"),
        }
    }

    // -----------------------------------------------------------------------
    // StructKind — orthogonal typestate placement
    // -----------------------------------------------------------------------

    #[test]
    fn test_struct_kind_unit_with_typestate_marker_succeeds() {
        // AC-01: unit struct can carry a typestate marker
        let marker = make_marker("LockMachine", &["lock", "unlock"]);
        let kind = StructKind::new(StructShape::Unit, Some(marker.clone()));
        assert!(matches!(kind.shape, StructShape::Unit));
        assert_eq!(kind.typestate.as_ref().unwrap().state_name().as_str(), "LockMachine");
    }

    #[test]
    fn test_struct_kind_tuple_with_typestate_marker_succeeds() {
        // AC-02: tuple struct can carry a typestate marker
        let field_ty = TypeRef::new("Uuid").unwrap();
        let marker = make_marker("PendingMachine", &["activate"]);
        let kind = StructKind::new(
            StructShape::Tuple { fields: vec![field_ty], has_stripped_fields: false },
            Some(marker.clone()),
        );
        assert!(matches!(kind.shape, StructShape::Tuple { .. }));
        assert_eq!(kind.typestate.as_ref().unwrap().state_name().as_str(), "PendingMachine");
    }

    #[test]
    fn test_struct_kind_plain_with_typestate_marker_succeeds() {
        // AC-07 regression: plain struct typestate still works
        let field_name = FieldName::new("value").unwrap();
        let field_ty = TypeRef::new("String").unwrap();
        let marker = make_marker("ReviewMachine", &["start"]);
        let kind = StructKind::new(
            StructShape::Plain {
                fields: vec![FieldDecl::new(field_name, field_ty)],
                has_stripped_fields: false,
            },
            Some(marker.clone()),
        );
        assert!(matches!(kind.shape, StructShape::Plain { .. }));
        assert_eq!(kind.typestate.as_ref().unwrap().state_name().as_str(), "ReviewMachine");
    }

    #[test]
    fn test_struct_kind_unit_no_typestate() {
        let kind = StructKind::new(StructShape::Unit, None);
        assert!(matches!(kind.shape, StructShape::Unit));
        assert!(kind.typestate.is_none());
    }

    #[test]
    fn test_struct_kind_equality_same_shape_and_typestate() {
        let k1 = StructKind::new(StructShape::Unit, None);
        let k2 = StructKind::new(StructShape::Unit, None);
        assert_eq!(k1, k2);
    }

    #[test]
    fn test_struct_kind_inequality_different_shape() {
        let k_unit = StructKind::new(StructShape::Unit, None);
        let k_tuple = StructKind::new(
            StructShape::Tuple { fields: vec![], has_stripped_fields: false },
            None,
        );
        assert_ne!(k_unit, k_tuple);
    }

    // -----------------------------------------------------------------------
    // TypeKindV2 — Struct variant
    // -----------------------------------------------------------------------

    #[test]
    fn test_type_kind_v2_struct_unit_matches() {
        let kind = TypeKindV2::Struct(StructKind::new(StructShape::Unit, None));
        assert!(matches!(kind, TypeKindV2::Struct(_)));
    }

    #[test]
    fn test_type_kind_v2_struct_unit_not_equal_to_struct_plain() {
        let unit = TypeKindV2::Struct(StructKind::new(StructShape::Unit, None));
        let plain = TypeKindV2::Struct(StructKind::new(
            StructShape::Plain { fields: vec![], has_stripped_fields: false },
            None,
        ));
        assert_ne!(unit, plain);
    }

    #[test]
    fn test_type_kind_v2_struct_tuple_holds_fields() {
        let field_ty = TypeRef::new("String").unwrap();
        let kind = TypeKindV2::Struct(StructKind::new(
            StructShape::Tuple { fields: vec![field_ty.clone()], has_stripped_fields: false },
            None,
        ));
        match &kind {
            TypeKindV2::Struct(sk) => match &sk.shape {
                StructShape::Tuple { fields, has_stripped_fields } => {
                    assert_eq!(fields.len(), 1);
                    assert_eq!(fields[0], field_ty);
                    assert!(!has_stripped_fields);
                }
                _ => panic!("expected Tuple shape"),
            },
            _ => panic!("expected Struct"),
        }
    }

    #[test]
    fn test_type_kind_v2_struct_plain_no_typestate() {
        let field_name = FieldName::new("email").unwrap();
        let field_ty = TypeRef::new("String").unwrap();
        let kind = TypeKindV2::Struct(StructKind::new(
            StructShape::Plain {
                fields: vec![FieldDecl::new(field_name.clone(), field_ty)],
                has_stripped_fields: false,
            },
            None,
        ));
        match &kind {
            TypeKindV2::Struct(sk) => {
                assert!(sk.typestate.is_none());
                match &sk.shape {
                    StructShape::Plain { fields, .. } => {
                        assert_eq!(fields[0].name, field_name);
                    }
                    _ => panic!("expected Plain shape"),
                }
            }
            _ => panic!("expected Struct"),
        }
    }

    // -----------------------------------------------------------------------
    // TypeKindV2 — Enum
    // -----------------------------------------------------------------------

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
    fn test_type_kind_v2_enum_with_empty_variants_succeeds() {
        let kind = TypeKindV2::Enum { variants: vec![] };
        assert!(matches!(kind, TypeKindV2::Enum { .. }));
    }

    // -----------------------------------------------------------------------
    // TypeKindV2 — TypeAlias
    // -----------------------------------------------------------------------

    #[test]
    fn test_type_kind_v2_type_alias_variant_holds_target() {
        let target = TypeRef::new("Result<User, DomainError>").unwrap();
        let kind = TypeKindV2::TypeAlias { target: target.clone() };
        match &kind {
            TypeKindV2::TypeAlias { target: k_target } => assert_eq!(k_target, &target),
            _ => panic!("expected TypeAlias"),
        }
    }

    // -----------------------------------------------------------------------
    // Illegal-state prevention: structural enforcement (AC-05, AC-06)
    // -----------------------------------------------------------------------

    #[test]
    fn test_unit_shape_has_no_fields_structurally() {
        // AC-05: StructShape::Unit has no `fields` payload.
        let kind = TypeKindV2::Struct(StructKind::new(StructShape::Unit, None));
        assert!(
            matches!(kind, TypeKindV2::Struct(ref sk) if matches!(sk.shape, StructShape::Unit))
        );
        // There is no `sk.shape.fields` accessor because Unit has no fields payload.
    }

    #[test]
    fn test_tuple_shape_uses_type_refs_not_field_decls() {
        // AC-06: StructShape::Tuple uses Vec<TypeRef>, not Vec<FieldDecl>.
        // Named fields cannot be attached to a Tuple shape.
        let kind = TypeKindV2::Struct(StructKind::new(
            StructShape::Tuple { fields: vec![], has_stripped_fields: false },
            None,
        ));
        assert!(
            matches!(kind, TypeKindV2::Struct(ref sk) if matches!(sk.shape, StructShape::Tuple { .. }))
        );
    }

    #[test]
    fn test_all_three_variants_are_distinct() {
        let struct_ = TypeKindV2::Struct(StructKind::new(StructShape::Unit, None));
        let enum_ = TypeKindV2::Enum { variants: vec![] };
        let alias = TypeKindV2::TypeAlias { target: TypeRef::new("u32").unwrap() };
        assert_ne!(struct_, enum_);
        assert_ne!(struct_, alias);
        assert_ne!(enum_, alias);
    }
}
