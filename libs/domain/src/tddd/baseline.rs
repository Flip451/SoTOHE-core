//! Baseline types for TDDD reverse signal filtering.
//!
//! A `TypeBaseline` captures the TypeGraph structure at `/track:design` time.
//! During signal evaluation, types present in the baseline with unchanged
//! structure are skipped (not flagged as Red), filtering out existing-type noise.
//!
//! T005 (TDDD-01 Phase 1 Task 5): baseline schema v2 replaces the flat
//! `Vec<String>` members / method_return_types / methods representation with
//! structured `Vec<MemberDeclaration>` and `Vec<MethodDeclaration>` so that
//! the baseline captures full L1 signatures. Legacy `method_return_types` /
//! `method_names` fields are removed.

use std::collections::HashMap;

use crate::schema::TypeKind;
use crate::tddd::catalogue::{MemberDeclaration, MethodDeclaration};
use crate::timestamp::Timestamp;

// ---------------------------------------------------------------------------
// TypeBaselineEntry
// ---------------------------------------------------------------------------

/// A single type entry in the baseline snapshot.
///
/// Members are sorted by name and methods by method name for deterministic
/// comparison and serialization.
#[derive(Debug, Clone)]
pub struct TypeBaselineEntry {
    kind: TypeKind,
    /// Variants (for enums) or fields (for structs), sorted by name.
    members: Vec<MemberDeclaration>,
    /// Structured L1 signatures of inherent impl methods, sorted by name.
    methods: Vec<MethodDeclaration>,
}

impl TypeBaselineEntry {
    /// Creates a new `TypeBaselineEntry` with members and methods sorted.
    #[must_use]
    pub fn new(
        kind: TypeKind,
        mut members: Vec<MemberDeclaration>,
        mut methods: Vec<MethodDeclaration>,
    ) -> Self {
        members.sort_by(|a, b| a.name().cmp(b.name()));
        methods.sort_by(|a, b| a.name().cmp(b.name()));
        Self { kind, members, methods }
    }

    /// Returns the kind of this type.
    #[must_use]
    pub fn kind(&self) -> &TypeKind {
        &self.kind
    }

    /// Returns the sorted members (variants or fields).
    #[must_use]
    pub fn members(&self) -> &[MemberDeclaration] {
        &self.members
    }

    /// Returns the sorted method declarations.
    #[must_use]
    pub fn methods(&self) -> &[MethodDeclaration] {
        &self.methods
    }

    /// Returns `true` if this entry is structurally equal to `other`.
    ///
    /// Compares kind, sorted members, and sorted method declarations. Since
    /// both fields are sorted at construction, this is a direct comparison.
    #[must_use]
    pub fn structurally_equal(&self, other: &Self) -> bool {
        self.kind == other.kind && self.members == other.members && self.methods == other.methods
    }
}

// ---------------------------------------------------------------------------
// TraitBaselineEntry
// ---------------------------------------------------------------------------

/// A single trait entry in the baseline snapshot.
///
/// Methods are sorted by name for deterministic comparison and serialization.
#[derive(Debug, Clone)]
pub struct TraitBaselineEntry {
    /// Structured L1 signatures of trait methods, sorted by name.
    methods: Vec<MethodDeclaration>,
}

impl TraitBaselineEntry {
    /// Creates a new `TraitBaselineEntry` with methods sorted.
    #[must_use]
    pub fn new(mut methods: Vec<MethodDeclaration>) -> Self {
        methods.sort_by(|a, b| a.name().cmp(b.name()));
        Self { methods }
    }

    /// Returns the sorted method declarations.
    #[must_use]
    pub fn methods(&self) -> &[MethodDeclaration] {
        &self.methods
    }

    /// Returns `true` if this entry is structurally equal to `other`.
    #[must_use]
    pub fn structurally_equal(&self, other: &Self) -> bool {
        self.methods == other.methods
    }
}

// ---------------------------------------------------------------------------
// TypeBaseline
// ---------------------------------------------------------------------------

/// Snapshot of the TypeGraph at `/track:design` time.
///
/// Used by `check_consistency` to distinguish existing-unchanged types
/// (skip) from structurally-changed or newly-added types (Red).
#[derive(Debug, Clone)]
pub struct TypeBaseline {
    schema_version: u32,
    captured_at: Timestamp,
    types: HashMap<String, TypeBaselineEntry>,
    traits: HashMap<String, TraitBaselineEntry>,
}

impl TypeBaseline {
    /// Creates a new `TypeBaseline`.
    #[must_use]
    pub fn new(
        schema_version: u32,
        captured_at: Timestamp,
        types: HashMap<String, TypeBaselineEntry>,
        traits: HashMap<String, TraitBaselineEntry>,
    ) -> Self {
        Self { schema_version, captured_at, types, traits }
    }

    /// Returns the schema version.
    #[must_use]
    pub fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns the capture timestamp (ISO 8601).
    #[must_use]
    pub fn captured_at(&self) -> &Timestamp {
        &self.captured_at
    }

    /// Returns the type entries.
    #[must_use]
    pub fn types(&self) -> &HashMap<String, TypeBaselineEntry> {
        &self.types
    }

    /// Returns the trait entries.
    #[must_use]
    pub fn traits(&self) -> &HashMap<String, TraitBaselineEntry> {
        &self.traits
    }

    /// Looks up a type entry by name.
    #[must_use]
    pub fn get_type(&self, name: &str) -> Option<&TypeBaselineEntry> {
        self.types.get(name)
    }

    /// Looks up a trait entry by name.
    #[must_use]
    pub fn get_trait(&self, name: &str) -> Option<&TraitBaselineEntry> {
        self.traits.get(name)
    }

    /// Returns `true` if the given type name exists in the baseline.
    #[must_use]
    pub fn has_type(&self, name: &str) -> bool {
        self.types.contains_key(name)
    }

    /// Returns `true` if the given trait name exists in the baseline.
    #[must_use]
    pub fn has_trait(&self, name: &str) -> bool {
        self.traits.contains_key(name)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::schema::TypeKind;
    use crate::tddd::catalogue::ParamDeclaration;
    use crate::timestamp::Timestamp;

    fn unit_method(name: &str) -> MethodDeclaration {
        MethodDeclaration::new(name, Some("&self".into()), vec![], "()", false)
    }

    // --- TypeBaselineEntry ---

    #[test]
    fn test_type_baseline_entry_sorts_members_on_construction() {
        let entry = TypeBaselineEntry::new(
            TypeKind::Enum,
            vec![
                MemberDeclaration::variant("Zebra"),
                MemberDeclaration::variant("Alpha"),
                MemberDeclaration::variant("Middle"),
            ],
            vec![],
        );
        let names: Vec<&str> = entry.members().iter().map(|m| m.name()).collect();
        assert_eq!(names, vec!["Alpha", "Middle", "Zebra"]);
    }

    #[test]
    fn test_type_baseline_entry_sorts_methods_on_construction() {
        let entry = TypeBaselineEntry::new(
            TypeKind::Struct,
            vec![],
            vec![unit_method("publish"), unit_method("archive")],
        );
        let names: Vec<&str> = entry.methods().iter().map(|m| m.name()).collect();
        assert_eq!(names, vec!["archive", "publish"]);
    }

    #[test]
    fn test_type_baseline_entry_structurally_equal_with_same_fields() {
        let a = TypeBaselineEntry::new(
            TypeKind::Enum,
            vec![MemberDeclaration::variant("B"), MemberDeclaration::variant("A")],
            vec![unit_method("y"), unit_method("x")],
        );
        let b = TypeBaselineEntry::new(
            TypeKind::Enum,
            vec![MemberDeclaration::variant("A"), MemberDeclaration::variant("B")],
            vec![unit_method("x"), unit_method("y")],
        );
        assert!(a.structurally_equal(&b));
    }

    #[test]
    fn test_type_baseline_entry_not_equal_with_different_kind() {
        let a = TypeBaselineEntry::new(TypeKind::Struct, vec![], vec![]);
        let b = TypeBaselineEntry::new(TypeKind::Enum, vec![], vec![]);
        assert!(!a.structurally_equal(&b));
    }

    #[test]
    fn test_type_baseline_entry_not_equal_with_different_members() {
        let a =
            TypeBaselineEntry::new(TypeKind::Enum, vec![MemberDeclaration::variant("A")], vec![]);
        let b =
            TypeBaselineEntry::new(TypeKind::Enum, vec![MemberDeclaration::variant("B")], vec![]);
        assert!(!a.structurally_equal(&b));
    }

    #[test]
    fn test_type_baseline_entry_not_equal_with_different_method_signature() {
        let a = TypeBaselineEntry::new(
            TypeKind::Struct,
            vec![],
            vec![MethodDeclaration::new("find", Some("&self".into()), vec![], "()", false)],
        );
        let b = TypeBaselineEntry::new(
            TypeKind::Struct,
            vec![],
            vec![MethodDeclaration::new(
                "find",
                Some("&self".into()),
                vec![ParamDeclaration::new("id", "UserId")],
                "()",
                false,
            )],
        );
        assert!(!a.structurally_equal(&b));
    }

    #[test]
    fn test_type_baseline_entry_accessors() {
        let entry = TypeBaselineEntry::new(
            TypeKind::Struct,
            vec![MemberDeclaration::field("field", "String")],
            vec![unit_method("get")],
        );
        assert_eq!(entry.kind(), &TypeKind::Struct);
        assert_eq!(entry.members().len(), 1);
        assert_eq!(entry.members()[0].name(), "field");
        assert_eq!(entry.methods().len(), 1);
        assert_eq!(entry.methods()[0].name(), "get");
    }

    // --- TraitBaselineEntry ---

    #[test]
    fn test_trait_baseline_entry_sorts_methods_on_construction() {
        let entry = TraitBaselineEntry::new(vec![
            unit_method("save"),
            unit_method("find"),
            unit_method("delete"),
        ]);
        let names: Vec<&str> = entry.methods().iter().map(|m| m.name()).collect();
        assert_eq!(names, vec!["delete", "find", "save"]);
    }

    #[test]
    fn test_trait_baseline_entry_structurally_equal_with_same_methods() {
        let a = TraitBaselineEntry::new(vec![unit_method("save"), unit_method("find")]);
        let b = TraitBaselineEntry::new(vec![unit_method("find"), unit_method("save")]);
        assert!(a.structurally_equal(&b));
    }

    #[test]
    fn test_trait_baseline_entry_not_equal_with_different_methods() {
        let a = TraitBaselineEntry::new(vec![unit_method("save")]);
        let b = TraitBaselineEntry::new(vec![unit_method("delete")]);
        assert!(!a.structurally_equal(&b));
    }

    // --- TypeBaseline ---

    fn sample_baseline() -> TypeBaseline {
        let mut types = HashMap::new();
        types.insert(
            "TrackId".into(),
            TypeBaselineEntry::new(
                TypeKind::Struct,
                vec![MemberDeclaration::field("0", "u64")],
                vec![],
            ),
        );
        types.insert(
            "TaskStatus".into(),
            TypeBaselineEntry::new(
                TypeKind::Enum,
                vec![
                    MemberDeclaration::variant("Todo"),
                    MemberDeclaration::variant("InProgress"),
                    MemberDeclaration::variant("Done"),
                ],
                vec![MethodDeclaration::new(
                    "kind",
                    Some("&self".into()),
                    vec![],
                    "TaskStatusKind",
                    false,
                )],
            ),
        );

        let mut traits = HashMap::new();
        traits.insert("TrackReader".into(), TraitBaselineEntry::new(vec![unit_method("find")]));

        TypeBaseline::new(2, Timestamp::new("2026-04-13T00:01:00Z").unwrap(), types, traits)
    }

    #[test]
    fn test_type_baseline_schema_version() {
        let bl = sample_baseline();
        assert_eq!(bl.schema_version(), 2);
    }

    #[test]
    fn test_type_baseline_captured_at() {
        let bl = sample_baseline();
        assert_eq!(bl.captured_at().as_str(), "2026-04-13T00:01:00Z");
    }

    #[test]
    fn test_type_baseline_get_type_returns_entry() {
        let bl = sample_baseline();
        let entry = bl.get_type("TrackId").unwrap();
        assert_eq!(entry.kind(), &TypeKind::Struct);
        assert_eq!(entry.members().len(), 1);
        assert_eq!(entry.members()[0].name(), "0");
    }

    #[test]
    fn test_type_baseline_get_type_returns_none_for_missing() {
        let bl = sample_baseline();
        assert!(bl.get_type("NonExistent").is_none());
    }

    #[test]
    fn test_type_baseline_get_trait_returns_entry() {
        let bl = sample_baseline();
        let entry = bl.get_trait("TrackReader").unwrap();
        assert_eq!(entry.methods().len(), 1);
        assert_eq!(entry.methods()[0].name(), "find");
    }

    #[test]
    fn test_type_baseline_get_trait_returns_none_for_missing() {
        let bl = sample_baseline();
        assert!(bl.get_trait("NonExistent").is_none());
    }

    #[test]
    fn test_type_baseline_has_type() {
        let bl = sample_baseline();
        assert!(bl.has_type("TrackId"));
        assert!(!bl.has_type("Missing"));
    }

    #[test]
    fn test_type_baseline_has_trait() {
        let bl = sample_baseline();
        assert!(bl.has_trait("TrackReader"));
        assert!(!bl.has_trait("Missing"));
    }

    #[test]
    fn test_type_baseline_types_returns_all_entries() {
        let bl = sample_baseline();
        assert_eq!(bl.types().len(), 2);
    }

    #[test]
    fn test_type_baseline_traits_returns_all_entries() {
        let bl = sample_baseline();
        assert_eq!(bl.traits().len(), 1);
    }
}
