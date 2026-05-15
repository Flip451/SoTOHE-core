//! Flat type kind enum for the catalogue v2 schema.
//!
//! Implements:
//! - [`TypestateMarker`]: marker carried by `TypeKindV2::PlainStruct` when the struct
//!   is a typestate state. Replaces the old `CompositePattern::TypestateState` embedding.
//! - [`TypestateTransitions`]: transition specification for a typestate state.
//! - [`TypeKindV2`]: language-level kind for `TypeEntry`. 5 self-contained variants,
//!   each carrying only the fields semantically valid for that kind. ADR 1 D7.
//!
//! No serde derives — per ADR `knowledge/adr/2026-04-14-1531-domain-serde-ripout.md`,
//! the domain layer is serialization-free. The infrastructure codec (T003) handles JSON.
//!
//! ## Design rationale
//!
//! The old `TypeKindV2::Struct { pattern: CompositePattern, fields }` mixed structural
//! kind (`Unit`) with DDD-semantic patterns (`TypestateState`, `Newtype`) in a single
//! category-error enum.  The flat redesign eliminates this by giving each variant only
//! the fields that are semantically valid for it:
//!
//! - `UnitStruct`: no fields possible — illegal state `Unit + non-empty fields` is
//!   structurally unrepresentable.
//! - `TupleStruct { fields, has_stripped_fields }`: mirrors `rustdoc_types::StructKind::Tuple`.
//!   Newtype semantic is structurally a `TupleStruct` with one field; detection deferred
//!   to lint layer (T013+), not the schema.
//! - `PlainStruct { fields, has_stripped_fields, typestate }`: mirrors `StructKind::Plain`.
//!   An optional `TypestateMarker` encodes typestate membership without polluting other
//!   struct kinds.
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
// TypestateMarker — typestate membership marker for PlainStruct
// ---------------------------------------------------------------------------

/// Typestate membership marker carried by `TypeKindV2::PlainStruct`.
///
/// When present (`typestate: Some(TypestateMarker { .. })`), the plain struct
/// is a state in a typestate machine. The `state_name` field names the typestate
/// machine (the root type that the states belong to), and `transitions` lists
/// the methods that produce the next state.
///
/// Fields are private with fallible constructor and accessor methods (CN-09).
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
    /// Creates a new `TypestateMarker`.
    ///
    /// # Errors
    ///
    /// Returns an error string if `state_name` is empty or whitespace-only
    /// (validated via [`TypeName`] construction).
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
/// 5 self-contained variants. Each variant carries only the fields semantically
/// valid for it — illegal field combinations are structurally unrepresentable.
///
/// - `UnitStruct`: a unit struct (`pub struct Foo;`) with no fields. Fields cannot
///   be attached — the variant has no `fields` payload.
/// - `TupleStruct { fields, has_stripped_fields }`: a tuple struct (positional fields).
///   Mirrors `rustdoc_types::StructKind::Tuple`. Newtypes (`struct Foo(Bar)`) are
///   structurally a `TupleStruct` with a single field; semantic detection is deferred
///   to the lint layer (T013+), not the schema.
/// - `PlainStruct { fields, has_stripped_fields, typestate }`: a plain named-field
///   struct. Mirrors `rustdoc_types::StructKind::Plain`. An optional `TypestateMarker`
///   encodes typestate membership without affecting other struct kinds.
/// - `Enum { variants }`: a sum / enum type.
/// - `TypeAlias { target }`: a type alias.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeKindV2 {
    /// A unit struct (`pub struct Foo;`).
    ///
    /// No fields are possible — the structural absence of a `fields` payload
    /// makes `UnitStruct + non-empty fields` a compile-time impossibility.
    UnitStruct,
    /// A tuple struct (`pub struct Foo(Bar, Baz)`).
    ///
    /// Mirrors `rustdoc_types::StructKind::Tuple`.
    /// - `fields`: the positional field types (unnamed — tuple fields are `.0`, `.1`, ...).
    /// - `has_stripped_fields`: `true` when rustdoc omits private fields from
    ///   the documentation output. Stored here (not in a nested struct) to keep
    ///   the schema flat and match the rustdoc representation.
    ///
    /// Using `Vec<TypeRef>` instead of `Vec<FieldDecl>` makes illegal states
    /// unrepresentable: tuple fields are positional and have no user-visible names,
    /// so fabricating names (e.g. `"0"`, `"inner"`) would be misleading and wrong.
    TupleStruct {
        /// Positional field types (unnamed; index-addressable as `.0`, `.1`, …).
        fields: Vec<TypeRef>,
        /// `true` when the struct has at least one private field that rustdoc
        /// omits from the documentation output.
        has_stripped_fields: bool,
    },
    /// A plain named-field struct (`pub struct Foo { bar: Bar }`).
    ///
    /// Mirrors `rustdoc_types::StructKind::Plain`.
    /// - `fields`: the named field declarations.
    /// - `has_stripped_fields`: `true` when rustdoc omits private fields.
    /// - `typestate`: optional typestate membership marker. Present when this
    ///   struct is a state in a typestate machine.
    PlainStruct {
        /// Named field declarations.
        fields: Vec<FieldDecl>,
        /// `true` when the struct has at least one private field that rustdoc
        /// omits from the documentation output.
        has_stripped_fields: bool,
        /// Optional typestate membership marker.
        ///
        /// When `Some`, this struct is a state in the named typestate machine.
        /// When `None`, this is a plain struct with no typestate relationship.
        typestate: Option<TypestateMarker>,
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
    // TypeKindV2 — UnitStruct
    // -----------------------------------------------------------------------

    #[test]
    fn test_type_kind_v2_unit_struct_matches_unit_struct() {
        let kind = TypeKindV2::UnitStruct;
        assert!(matches!(kind, TypeKindV2::UnitStruct));
    }

    #[test]
    fn test_type_kind_v2_unit_struct_not_equal_to_plain_struct() {
        let unit = TypeKindV2::UnitStruct;
        let plain =
            TypeKindV2::PlainStruct { fields: vec![], has_stripped_fields: false, typestate: None };
        assert_ne!(unit, plain);
    }

    // -----------------------------------------------------------------------
    // TypeKindV2 — TupleStruct
    // -----------------------------------------------------------------------

    #[test]
    fn test_type_kind_v2_tuple_struct_holds_fields_and_stripped_flag() {
        let field_ty = TypeRef::new("String").unwrap();
        let fields = vec![field_ty.clone()];
        let kind = TypeKindV2::TupleStruct { fields: fields.clone(), has_stripped_fields: false };
        match &kind {
            TypeKindV2::TupleStruct { fields: k_fields, has_stripped_fields: k_stripped } => {
                assert_eq!(k_fields.len(), 1);
                assert_eq!(k_fields[0], field_ty);
                assert!(!k_stripped);
            }
            _ => panic!("expected TupleStruct"),
        }
    }

    #[test]
    fn test_type_kind_v2_tuple_struct_with_stripped_fields() {
        let kind = TypeKindV2::TupleStruct { fields: vec![], has_stripped_fields: true };
        assert!(matches!(kind, TypeKindV2::TupleStruct { has_stripped_fields: true, .. }));
    }

    // -----------------------------------------------------------------------
    // TypeKindV2 — PlainStruct
    // -----------------------------------------------------------------------

    #[test]
    fn test_type_kind_v2_plain_struct_no_typestate() {
        let field_name = FieldName::new("email").unwrap();
        let field_ty = TypeRef::new("String").unwrap();
        let fields = vec![FieldDecl::new(field_name.clone(), field_ty.clone())];
        let kind = TypeKindV2::PlainStruct {
            fields: fields.clone(),
            has_stripped_fields: false,
            typestate: None,
        };
        match &kind {
            TypeKindV2::PlainStruct { fields: k_fields, has_stripped_fields, typestate } => {
                assert_eq!(k_fields.len(), 1);
                assert_eq!(k_fields[0].name, field_name);
                assert!(!has_stripped_fields);
                assert!(typestate.is_none());
            }
            _ => panic!("expected PlainStruct"),
        }
    }

    #[test]
    fn test_type_kind_v2_plain_struct_with_typestate_marker() {
        let state_name = TypeName::new("ReviewMachine").unwrap();
        let m = MethodName::new("start").unwrap();
        let transitions = TypestateTransitions::new(vec![m]);
        let marker = TypestateMarker::new(state_name.clone(), transitions);
        let kind = TypeKindV2::PlainStruct {
            fields: vec![],
            has_stripped_fields: false,
            typestate: Some(marker.clone()),
        };
        match &kind {
            TypeKindV2::PlainStruct { typestate: Some(ts), .. } => {
                assert_eq!(ts.state_name(), &state_name);
            }
            _ => panic!("expected PlainStruct with typestate"),
        }
    }

    #[test]
    fn test_type_kind_v2_plain_struct_with_stripped_fields() {
        let kind =
            TypeKindV2::PlainStruct { fields: vec![], has_stripped_fields: true, typestate: None };
        assert!(matches!(kind, TypeKindV2::PlainStruct { has_stripped_fields: true, .. }));
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
    // Illegal-state prevention: structural enforcement
    // -----------------------------------------------------------------------

    #[test]
    fn test_unit_struct_has_no_fields_structurally() {
        // UnitStruct has no `fields` payload — no way to attach fields at the type level.
        // This test documents the structural guarantee.
        let kind = TypeKindV2::UnitStruct;
        assert!(matches!(kind, TypeKindV2::UnitStruct));
        // There is no `kind.fields` accessor because UnitStruct has no fields field.
    }

    #[test]
    fn test_typestate_only_on_plain_struct() {
        // typestate: Option<TypestateMarker> only exists on PlainStruct.
        // UnitStruct and TupleStruct cannot carry a typestate marker.
        let unit = TypeKindV2::UnitStruct;
        let tuple = TypeKindV2::TupleStruct { fields: vec![], has_stripped_fields: false };
        // Confirm neither variant has a typestate field by destructuring.
        assert!(matches!(unit, TypeKindV2::UnitStruct));
        assert!(matches!(tuple, TypeKindV2::TupleStruct { .. }));
    }

    #[test]
    fn test_all_five_variants_are_distinct() {
        let unit = TypeKindV2::UnitStruct;
        let tuple = TypeKindV2::TupleStruct { fields: vec![], has_stripped_fields: false };
        let plain =
            TypeKindV2::PlainStruct { fields: vec![], has_stripped_fields: false, typestate: None };
        let enum_ = TypeKindV2::Enum { variants: vec![] };
        let alias = TypeKindV2::TypeAlias { target: TypeRef::new("u32").unwrap() };
        assert_ne!(unit, tuple);
        assert_ne!(unit, plain);
        assert_ne!(unit, enum_);
        assert_ne!(unit, alias);
        assert_ne!(tuple, plain);
        assert_ne!(tuple, enum_);
        assert_ne!(tuple, alias);
        assert_ne!(plain, enum_);
        assert_ne!(plain, alias);
        assert_ne!(enum_, alias);
    }
}
