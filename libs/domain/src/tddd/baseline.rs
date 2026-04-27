//! Baseline types for TDDD reverse signal filtering.
//!
//! A `TypeBaseline` captures the TypeGraph structure at `/track:design` time.
//! During signal evaluation, types present in the baseline with unchanged
//! structure are skipped (not flagged as Red), filtering out existing-type noise.
//!
//! Baseline schema v2 replaces the flat `Vec<String>` members /
//! method_return_types / methods representation with structured
//! `Vec<MemberDeclaration>` and `Vec<MethodDeclaration>` so that the baseline
//! captures full L1 signatures. Legacy `method_return_types` / `method_names`
//! fields are removed.
//!
//! T005 (S3): extends baseline schema with:
//! - `TraitImplBaselineEntry` — captures `trait_name` + `origin_crate` for
//!   each trait impl on a type.
//! - `TypeBaselineEntry::trait_impls` — `Vec<TraitImplBaselineEntry>` for
//!   origin-crate-aware reverse filtering.
//! - `FunctionBaselineEntry` — captures free function signature at L1 resolution.
//! - `TypeBaseline::functions` — `HashMap<String, FunctionBaselineEntry>` keyed
//!   by fully-qualified name (e.g. `"crate::module::fn_name"`). String key is
//!   used here (vs the in-memory tuple key in `TypeGraph`) so that JSON
//!   serialization in T007 `baseline_codec` can use a plain object key.

use std::collections::HashMap;

use crate::schema::TypeKind;
use crate::tddd::catalogue::{MemberDeclaration, MethodDeclaration, ParamDeclaration};
use crate::timestamp::Timestamp;

// ---------------------------------------------------------------------------
// TraitImplBaselineEntry
// ---------------------------------------------------------------------------

/// A single trait implementation captured in the baseline snapshot.
///
/// T005 (S3): new type. Records `trait_name` + `origin_crate` so that T008's
/// signal evaluator can apply the IN-10 workspace-origin filter against the
/// baseline without re-deriving origin information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitImplBaselineEntry {
    trait_name: String,
    /// The crate that defines this trait (e.g., `"domain"`, `"std"`).
    /// `""` (empty string) when the origin could not be determined.
    origin_crate: String,
}

impl TraitImplBaselineEntry {
    /// Creates a new `TraitImplBaselineEntry`.
    #[must_use]
    pub fn new(trait_name: impl Into<String>, origin_crate: impl Into<String>) -> Self {
        Self { trait_name: trait_name.into(), origin_crate: origin_crate.into() }
    }

    /// Returns the trait name.
    #[must_use]
    pub fn trait_name(&self) -> &str {
        &self.trait_name
    }

    /// Returns the crate that defines this trait.
    #[must_use]
    pub fn origin_crate(&self) -> &str {
        &self.origin_crate
    }
}

// ---------------------------------------------------------------------------
// TypeBaselineEntry
// ---------------------------------------------------------------------------

/// A single type entry in the baseline snapshot.
///
/// Members are sorted by name and methods by method name for deterministic
/// comparison and serialization.
///
/// T005 (S3): adds `trait_impls: Vec<TraitImplBaselineEntry>` for
/// origin-crate-aware reverse filtering in T008.
#[derive(Debug, Clone)]
pub struct TypeBaselineEntry {
    kind: TypeKind,
    /// Variants (for enums) or fields (for structs), sorted by name.
    members: Vec<MemberDeclaration>,
    /// Structured L1 signatures of inherent impl methods, sorted by name.
    methods: Vec<MethodDeclaration>,
    /// Trait implementations on this type, for origin-crate-aware filtering.
    trait_impls: Vec<TraitImplBaselineEntry>,
}

impl TypeBaselineEntry {
    /// Creates a new `TypeBaselineEntry` with members and methods sorted.
    ///
    /// `trait_impls` defaults to empty for backward compatibility.
    #[must_use]
    pub fn new(
        kind: TypeKind,
        mut members: Vec<MemberDeclaration>,
        mut methods: Vec<MethodDeclaration>,
    ) -> Self {
        members.sort_by(|a, b| a.name().cmp(b.name()));
        methods.sort_by(|a, b| a.name().cmp(b.name()));
        Self { kind, members, methods, trait_impls: Vec::new() }
    }

    /// Creates a new `TypeBaselineEntry` with explicit `trait_impls`.
    #[must_use]
    pub fn with_trait_impls(
        kind: TypeKind,
        mut members: Vec<MemberDeclaration>,
        mut methods: Vec<MethodDeclaration>,
        trait_impls: Vec<TraitImplBaselineEntry>,
    ) -> Self {
        members.sort_by(|a, b| a.name().cmp(b.name()));
        methods.sort_by(|a, b| a.name().cmp(b.name()));
        Self { kind, members, methods, trait_impls }
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

    /// Returns the trait implementations on this type.
    #[must_use]
    pub fn trait_impls(&self) -> &[TraitImplBaselineEntry] {
        &self.trait_impls
    }

    /// Returns `true` if this entry is structurally equal to `other`.
    ///
    /// Compares kind, sorted members, and sorted method declarations. Since
    /// both fields are sorted at construction, this is a direct comparison.
    /// `trait_impls` is intentionally excluded from the structural equality
    /// check — trait impls are used for signal filtering, not structural diff.
    #[must_use]
    pub fn structurally_equal(&self, other: &Self) -> bool {
        self.kind == other.kind && self.members == other.members && self.methods == other.methods
    }
}

// ---------------------------------------------------------------------------
// FunctionBaselineEntry
// ---------------------------------------------------------------------------

/// A single free function captured in the baseline snapshot.
///
/// T005 (S3): new type. Mirrors the `FunctionNode` domain type but lives in
/// the baseline layer. Stored in `TypeBaseline::functions` under a
/// fully-qualified string key (e.g., `"crate::module::fn_name"`).
///
/// The string key (vs the `(short_name, module_path)` tuple key used in
/// `TypeGraph`) is chosen so that T007 `baseline_codec` can serialize
/// `TypeBaseline::functions` as a plain JSON object with string keys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionBaselineEntry {
    /// Structured parameter list at L1 resolution.
    params: Vec<ParamDeclaration>,
    /// Return type names (last-segment short names).
    returns: Vec<String>,
    /// Whether the function is declared `async fn`.
    is_async: bool,
    /// Module path for scoped lookup (e.g., `"domain::review"`). `None` if unknown.
    module_path: Option<String>,
}

impl FunctionBaselineEntry {
    /// Creates a new `FunctionBaselineEntry`.
    #[must_use]
    pub fn new(
        params: Vec<ParamDeclaration>,
        returns: Vec<String>,
        is_async: bool,
        module_path: Option<String>,
    ) -> Self {
        Self { params, returns, is_async, module_path }
    }

    /// Returns the structured parameter list (L1 resolution).
    #[must_use]
    pub fn params(&self) -> &[ParamDeclaration] {
        &self.params
    }

    /// Returns the return type names.
    #[must_use]
    pub fn returns(&self) -> &[String] {
        &self.returns
    }

    /// Returns `true` if the function is declared `async fn`.
    #[must_use]
    pub fn is_async(&self) -> bool {
        self.is_async
    }

    /// Returns the module path, if known.
    #[must_use]
    pub fn module_path(&self) -> Option<&str> {
        self.module_path.as_deref()
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
///
/// T005 (S3): adds `functions: HashMap<String, FunctionBaselineEntry>` keyed
/// by fully-qualified name (e.g., `"crate::module::fn_name"`). This mirrors
/// `TypeGraph::functions` but uses a string key for JSON serialization
/// compatibility (T007 `baseline_codec`).
#[derive(Debug, Clone)]
pub struct TypeBaseline {
    schema_version: u32,
    captured_at: Timestamp,
    types: HashMap<String, TypeBaselineEntry>,
    traits: HashMap<String, TraitBaselineEntry>,
    /// Free functions keyed by fully-qualified name string.
    functions: HashMap<String, FunctionBaselineEntry>,
}

impl TypeBaseline {
    /// Creates a new `TypeBaseline` (backward-compatible: `functions` defaults to empty).
    #[must_use]
    pub fn new(
        schema_version: u32,
        captured_at: Timestamp,
        types: HashMap<String, TypeBaselineEntry>,
        traits: HashMap<String, TraitBaselineEntry>,
    ) -> Self {
        Self { schema_version, captured_at, types, traits, functions: HashMap::new() }
    }

    /// Creates a new `TypeBaseline` with an explicit `functions` map.
    #[must_use]
    pub fn with_functions(
        schema_version: u32,
        captured_at: Timestamp,
        types: HashMap<String, TypeBaselineEntry>,
        traits: HashMap<String, TraitBaselineEntry>,
        functions: HashMap<String, FunctionBaselineEntry>,
    ) -> Self {
        Self { schema_version, captured_at, types, traits, functions }
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

    /// Returns the function entries (keyed by fully-qualified name).
    #[must_use]
    pub fn functions(&self) -> &HashMap<String, FunctionBaselineEntry> {
        &self.functions
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

    /// Looks up a function entry by fully-qualified name.
    #[must_use]
    pub fn get_function(&self, fq_name: &str) -> Option<&FunctionBaselineEntry> {
        self.functions.get(fq_name)
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

    /// Returns `true` if the given fully-qualified function name exists in the baseline.
    #[must_use]
    pub fn has_function(&self, fq_name: &str) -> bool {
        self.functions.contains_key(fq_name)
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

    // --- T005 (S3): TraitImplBaselineEntry ---

    #[test]
    fn test_trait_impl_baseline_entry_accessors() {
        let entry = TraitImplBaselineEntry::new("TrackReader", "domain");
        assert_eq!(entry.trait_name(), "TrackReader");
        assert_eq!(entry.origin_crate(), "domain");
    }

    #[test]
    fn test_trait_impl_baseline_entry_empty_origin_crate() {
        let entry = TraitImplBaselineEntry::new("Display", "");
        assert_eq!(entry.origin_crate(), "");
    }

    // --- T005 (S3): TypeBaselineEntry::trait_impls ---

    #[test]
    fn test_type_baseline_entry_new_has_empty_trait_impls() {
        let entry = TypeBaselineEntry::new(TypeKind::Struct, vec![], vec![]);
        assert!(entry.trait_impls().is_empty());
    }

    #[test]
    fn test_type_baseline_entry_with_trait_impls_stores_entries() {
        let impls = vec![
            TraitImplBaselineEntry::new("TrackReader", "domain"),
            TraitImplBaselineEntry::new("Display", "std"),
        ];
        let entry = TypeBaselineEntry::with_trait_impls(TypeKind::Struct, vec![], vec![], impls);
        assert_eq!(entry.trait_impls().len(), 2);
        assert_eq!(entry.trait_impls()[0].trait_name(), "TrackReader");
        assert_eq!(entry.trait_impls()[1].trait_name(), "Display");
    }

    #[test]
    fn test_type_baseline_entry_structurally_equal_ignores_trait_impls() {
        // trait_impls is excluded from structural equality check
        let a = TypeBaselineEntry::with_trait_impls(
            TypeKind::Struct,
            vec![],
            vec![],
            vec![TraitImplBaselineEntry::new("TrackReader", "domain")],
        );
        let b = TypeBaselineEntry::new(TypeKind::Struct, vec![], vec![]);
        assert!(a.structurally_equal(&b));
    }

    #[test]
    fn test_type_baseline_entry_with_trait_impls_sorts_members() {
        let entry = TypeBaselineEntry::with_trait_impls(
            TypeKind::Enum,
            vec![MemberDeclaration::variant("Z"), MemberDeclaration::variant("A")],
            vec![],
            vec![],
        );
        let names: Vec<&str> = entry.members().iter().map(|m| m.name()).collect();
        assert_eq!(names, vec!["A", "Z"]);
    }

    // --- T005 (S3): FunctionBaselineEntry ---

    #[test]
    fn test_function_baseline_entry_accessors() {
        let params = vec![ParamDeclaration::new("id", "TrackId")];
        let returns = vec!["Option<Track>".to_string()];
        let entry =
            FunctionBaselineEntry::new(params, returns, false, Some("domain::track".to_string()));
        assert_eq!(entry.params().len(), 1);
        assert_eq!(entry.params()[0].name(), "id");
        assert_eq!(entry.returns(), &["Option<Track>"]);
        assert!(!entry.is_async());
        assert_eq!(entry.module_path(), Some("domain::track"));
    }

    #[test]
    fn test_function_baseline_entry_async_and_no_module_path() {
        let entry = FunctionBaselineEntry::new(vec![], vec![], true, None);
        assert!(entry.is_async());
        assert!(entry.module_path().is_none());
    }

    // --- T005 (S3): TypeBaseline::functions ---

    #[test]
    fn test_type_baseline_new_has_empty_functions() {
        let bl = TypeBaseline::new(
            2,
            Timestamp::new("2026-04-27T00:00:00Z").unwrap(),
            HashMap::new(),
            HashMap::new(),
        );
        assert!(bl.functions().is_empty());
    }

    #[test]
    fn test_type_baseline_with_functions_stores_entries() {
        let mut functions = HashMap::new();
        let fq = "domain::track::build_baseline".to_string();
        functions.insert(
            fq.clone(),
            FunctionBaselineEntry::new(
                vec![],
                vec!["TypeBaseline".to_string()],
                false,
                Some("domain::track".to_string()),
            ),
        );
        let bl = TypeBaseline::with_functions(
            2,
            Timestamp::new("2026-04-27T00:00:00Z").unwrap(),
            HashMap::new(),
            HashMap::new(),
            functions,
        );
        assert!(bl.has_function(&fq));
        assert!(!bl.has_function("domain::track::nonexistent"));
    }

    #[test]
    fn test_type_baseline_get_function_returns_entry() {
        let mut functions = HashMap::new();
        let fq = "infra::tddd::build_type_graph".to_string();
        let entry = FunctionBaselineEntry::new(
            vec![ParamDeclaration::new("schema", "SchemaExport")],
            vec!["TypeGraph".to_string()],
            false,
            Some("infra::tddd".to_string()),
        );
        functions.insert(fq.clone(), entry);
        let bl = TypeBaseline::with_functions(
            2,
            Timestamp::new("2026-04-27T00:00:00Z").unwrap(),
            HashMap::new(),
            HashMap::new(),
            functions,
        );
        let result = bl.get_function(&fq);
        assert!(result.is_some());
        assert_eq!(result.unwrap().params().len(), 1);
        assert_eq!(result.unwrap().params()[0].name(), "schema");
    }

    #[test]
    fn test_type_baseline_get_function_returns_none_for_missing() {
        let bl = TypeBaseline::new(
            2,
            Timestamp::new("2026-04-27T00:00:00Z").unwrap(),
            HashMap::new(),
            HashMap::new(),
        );
        assert!(bl.get_function("domain::track::nonexistent").is_none());
    }

    #[test]
    fn test_type_baseline_functions_returns_all_entries() {
        let mut functions = HashMap::new();
        functions.insert(
            "crate::foo".to_string(),
            FunctionBaselineEntry::new(vec![], vec![], false, None),
        );
        functions.insert(
            "crate::bar".to_string(),
            FunctionBaselineEntry::new(vec![], vec![], false, None),
        );
        let bl = TypeBaseline::with_functions(
            2,
            Timestamp::new("2026-04-27T00:00:00Z").unwrap(),
            HashMap::new(),
            HashMap::new(),
            functions,
        );
        assert_eq!(bl.functions().len(), 2);
    }
}
