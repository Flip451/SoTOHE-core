//! Baseline types for TDDD reverse signal filtering.
//!
//! A `TypeBaseline` captures the TypeGraph structure at `/track:design` time.
//! During signal evaluation, types present in the baseline with unchanged
//! structure are skipped (not flagged as Red), filtering out existing-type noise.

use std::collections::HashMap;

use crate::schema::TypeKind;
use crate::timestamp::Timestamp;

// ---------------------------------------------------------------------------
// TypeBaselineEntry
// ---------------------------------------------------------------------------

/// A single type entry in the baseline snapshot.
///
/// Fields are stored sorted for deterministic comparison and serialization.
#[derive(Debug, Clone)]
pub struct TypeBaselineEntry {
    kind: TypeKind,
    /// Variant names (enums) or field names (structs), sorted.
    members: Vec<String>,
    /// Type names returned by inherent impl methods, sorted.
    method_return_types: Vec<String>,
}

impl TypeBaselineEntry {
    /// Creates a new `TypeBaselineEntry` with members and method_return_types sorted.
    #[must_use]
    pub fn new(
        kind: TypeKind,
        mut members: Vec<String>,
        mut method_return_types: Vec<String>,
    ) -> Self {
        members.sort();
        method_return_types.sort();
        Self { kind, members, method_return_types }
    }

    /// Returns the kind of this type.
    #[must_use]
    pub fn kind(&self) -> &TypeKind {
        &self.kind
    }

    /// Returns the sorted member names.
    #[must_use]
    pub fn members(&self) -> &[String] {
        &self.members
    }

    /// Returns the sorted method return type names.
    #[must_use]
    pub fn method_return_types(&self) -> &[String] {
        &self.method_return_types
    }

    /// Returns `true` if this entry is structurally equal to `other`.
    ///
    /// Compares kind, sorted members, and sorted method_return_types.
    /// Since fields are sorted at construction, this is a direct field comparison.
    #[must_use]
    pub fn structurally_equal(&self, other: &Self) -> bool {
        self.kind == other.kind
            && self.members == other.members
            && self.method_return_types == other.method_return_types
    }
}

// ---------------------------------------------------------------------------
// TraitBaselineEntry
// ---------------------------------------------------------------------------

/// A single trait entry in the baseline snapshot.
///
/// Methods are stored sorted for deterministic comparison and serialization.
#[derive(Debug, Clone)]
pub struct TraitBaselineEntry {
    /// Method names defined by this trait, sorted.
    methods: Vec<String>,
}

impl TraitBaselineEntry {
    /// Creates a new `TraitBaselineEntry` with methods sorted.
    #[must_use]
    pub fn new(mut methods: Vec<String>) -> Self {
        methods.sort();
        Self { methods }
    }

    /// Returns the sorted method names.
    #[must_use]
    pub fn methods(&self) -> &[String] {
        &self.methods
    }

    /// Returns `true` if this entry is structurally equal to `other`.
    ///
    /// Compares sorted method names.
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
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::schema::TypeKind;
    use crate::timestamp::Timestamp;

    // --- TypeBaselineEntry ---

    #[test]
    fn test_type_baseline_entry_sorts_members_on_construction() {
        let entry = TypeBaselineEntry::new(
            TypeKind::Enum,
            vec!["Zebra".into(), "Alpha".into(), "Middle".into()],
            vec![],
        );
        assert_eq!(entry.members(), &["Alpha", "Middle", "Zebra"]);
    }

    #[test]
    fn test_type_baseline_entry_sorts_method_return_types_on_construction() {
        let entry = TypeBaselineEntry::new(
            TypeKind::Struct,
            vec![],
            vec!["Published".into(), "Approved".into()],
        );
        assert_eq!(entry.method_return_types(), &["Approved", "Published"]);
    }

    #[test]
    fn test_type_baseline_entry_structurally_equal_with_same_fields() {
        let a = TypeBaselineEntry::new(
            TypeKind::Enum,
            vec!["B".into(), "A".into()],
            vec!["Y".into(), "X".into()],
        );
        let b = TypeBaselineEntry::new(
            TypeKind::Enum,
            vec!["A".into(), "B".into()],
            vec!["X".into(), "Y".into()],
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
        let a = TypeBaselineEntry::new(TypeKind::Enum, vec!["A".into()], vec![]);
        let b = TypeBaselineEntry::new(TypeKind::Enum, vec!["B".into()], vec![]);
        assert!(!a.structurally_equal(&b));
    }

    #[test]
    fn test_type_baseline_entry_not_equal_with_different_method_return_types() {
        let a = TypeBaselineEntry::new(TypeKind::Struct, vec![], vec!["X".into()]);
        let b = TypeBaselineEntry::new(TypeKind::Struct, vec![], vec!["Y".into()]);
        assert!(!a.structurally_equal(&b));
    }

    #[test]
    fn test_type_baseline_entry_accessors() {
        let entry =
            TypeBaselineEntry::new(TypeKind::Struct, vec!["field".into()], vec!["RetType".into()]);
        assert_eq!(entry.kind(), &TypeKind::Struct);
        assert_eq!(entry.members(), &["field"]);
        assert_eq!(entry.method_return_types(), &["RetType"]);
    }

    // --- TraitBaselineEntry ---

    #[test]
    fn test_trait_baseline_entry_sorts_methods_on_construction() {
        let entry = TraitBaselineEntry::new(vec!["save".into(), "find".into(), "delete".into()]);
        assert_eq!(entry.methods(), &["delete", "find", "save"]);
    }

    #[test]
    fn test_trait_baseline_entry_structurally_equal_with_same_methods() {
        let a = TraitBaselineEntry::new(vec!["save".into(), "find".into()]);
        let b = TraitBaselineEntry::new(vec!["find".into(), "save".into()]);
        assert!(a.structurally_equal(&b));
    }

    #[test]
    fn test_trait_baseline_entry_not_equal_with_different_methods() {
        let a = TraitBaselineEntry::new(vec!["save".into()]);
        let b = TraitBaselineEntry::new(vec!["delete".into()]);
        assert!(!a.structurally_equal(&b));
    }

    // --- TypeBaseline ---

    fn sample_baseline() -> TypeBaseline {
        let mut types = HashMap::new();
        types.insert(
            "TrackId".into(),
            TypeBaselineEntry::new(TypeKind::Struct, vec!["0".into()], vec![]),
        );
        types.insert(
            "TaskStatus".into(),
            TypeBaselineEntry::new(
                TypeKind::Enum,
                vec!["Todo".into(), "InProgress".into(), "Done".into()],
                vec!["TaskStatusKind".into()],
            ),
        );

        let mut traits = HashMap::new();
        traits.insert("TrackReader".into(), TraitBaselineEntry::new(vec!["find".into()]));

        TypeBaseline::new(1, Timestamp::new("2026-04-11T00:01:00Z").unwrap(), types, traits)
    }

    #[test]
    fn test_type_baseline_schema_version() {
        let bl = sample_baseline();
        assert_eq!(bl.schema_version(), 1);
    }

    #[test]
    fn test_type_baseline_captured_at() {
        let bl = sample_baseline();
        assert_eq!(bl.captured_at().as_str(), "2026-04-11T00:01:00Z");
    }

    #[test]
    fn test_type_baseline_get_type_returns_entry() {
        let bl = sample_baseline();
        let entry = bl.get_type("TrackId").unwrap();
        assert_eq!(entry.kind(), &TypeKind::Struct);
        assert_eq!(entry.members(), &["0"]);
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
        assert_eq!(entry.methods(), &["find"]);
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
