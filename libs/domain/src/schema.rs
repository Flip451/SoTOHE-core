//! Domain types for the `export-schema` feature (BRIDGE-01) and for the
//! TDDD catalogue evaluator (`TypeGraph`).
//!
//! These types represent the public API surface of a Rust crate as extracted
//! from rustdoc JSON. The BRIDGE-01 JSON wire format is owned by
//! `infrastructure::schema_export_codec`; `TypeGraph` / `TypeNode` /
//! `TraitNode` are the pre-indexed query interface used by `tddd::signals`
//! and `tddd::consistency`.
//!
//! T004 (TDDD-01 3c) extends these types with structured signature fields
//! (`params` / `returns` / `receiver` / `is_async`), replaces `TypeInfo::members`
//! with `Vec<MemberDeclaration>`, and adds `TypeNode::methods` / `TraitNode::methods`
//! as `Vec<MethodDeclaration>`. See ADR
//! `knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md` §Phase 1-4
//! and Consequences C1.

use std::collections::{HashMap, HashSet};

use crate::tddd::catalogue::{MemberDeclaration, MethodDeclaration, ParamDeclaration};

/// Top-level export result containing all public API elements of a crate.
///
/// T006 (S4): adds `trait_origins: HashMap<String, String>` mapping a trait's
/// stable fully-qualified definition path to the crate that defines it
/// (e.g., `"domain::ports::TrackReader" → "domain"`).
/// Keyed by def_path rather than short name to avoid collisions when two
/// distinct traits share the same short name (e.g., a local `Display` and
/// `std::fmt::Display`).  Populated by `build_schema_export` from rustdoc
/// JSON `paths`/`external_crates`.
#[derive(Debug, Clone)]
pub struct SchemaExport {
    crate_name: String,
    types: Vec<TypeInfo>,
    functions: Vec<FunctionInfo>,
    traits: Vec<TraitInfo>,
    impls: Vec<ImplInfo>,
    /// Maps trait def_path (stable fully-qualified definition path) → defining crate name.
    /// `""` if the origin could not be determined.
    trait_origins: HashMap<String, String>,
}

impl SchemaExport {
    /// Creates a new schema export (backward-compatible: `trait_origins` defaults to empty).
    pub fn new(
        crate_name: String,
        types: Vec<TypeInfo>,
        functions: Vec<FunctionInfo>,
        traits: Vec<TraitInfo>,
        impls: Vec<ImplInfo>,
    ) -> Self {
        Self { crate_name, types, functions, traits, impls, trait_origins: HashMap::new() }
    }

    /// Creates a new schema export with an explicit `trait_origins` map.
    pub fn with_trait_origins(
        crate_name: String,
        types: Vec<TypeInfo>,
        functions: Vec<FunctionInfo>,
        traits: Vec<TraitInfo>,
        impls: Vec<ImplInfo>,
        trait_origins: HashMap<String, String>,
    ) -> Self {
        Self { crate_name, types, functions, traits, impls, trait_origins }
    }

    /// Returns the crate name.
    pub fn crate_name(&self) -> &str {
        &self.crate_name
    }

    /// Returns the public types.
    pub fn types(&self) -> &[TypeInfo] {
        &self.types
    }

    /// Returns the free functions.
    pub fn functions(&self) -> &[FunctionInfo] {
        &self.functions
    }

    /// Returns the trait definitions.
    pub fn traits(&self) -> &[TraitInfo] {
        &self.traits
    }

    /// Returns the impl blocks.
    pub fn impls(&self) -> &[ImplInfo] {
        &self.impls
    }

    /// Returns the map of trait def_path → defining crate name.
    ///
    /// The key is the stable fully-qualified definition path of the trait as
    /// found in `krate.paths` (e.g., `"std::fmt::Display"`), not the short
    /// name.  Used by `build_type_graph` together with
    /// `ImplInfo::trait_def_path` to populate `TraitImplEntry::origin_crate`
    /// without short-name aliasing.
    pub fn trait_origins(&self) -> &HashMap<String, String> {
        &self.trait_origins
    }
}

/// Kind of a public type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeKind {
    /// A struct (with named fields, tuple fields, or unit).
    Struct,
    /// An enum with variants.
    Enum,
    /// A type alias.
    TypeAlias,
}

/// Information about a public type (struct, enum, or type alias).
#[derive(Debug, Clone)]
pub struct TypeInfo {
    name: String,
    kind: TypeKind,
    docs: Option<String>,
    /// For enums: variants. For structs: fields (name + type string).
    /// Empty for type aliases.
    members: Vec<MemberDeclaration>,
    /// Module path for disambiguation (e.g., `"domain::review"`). `None` if unknown.
    module_path: Option<String>,
}

impl TypeInfo {
    /// Creates a new type info.
    pub fn new(
        name: String,
        kind: TypeKind,
        docs: Option<String>,
        members: Vec<MemberDeclaration>,
    ) -> Self {
        Self { name, kind, docs, members, module_path: None }
    }

    /// Creates a new type info with a module path.
    pub fn with_module_path(
        name: String,
        kind: TypeKind,
        docs: Option<String>,
        members: Vec<MemberDeclaration>,
        module_path: String,
    ) -> Self {
        Self { name, kind, docs, members, module_path: Some(module_path) }
    }

    /// Returns the type name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the type kind.
    pub fn kind(&self) -> &TypeKind {
        &self.kind
    }

    /// Returns the documentation string.
    pub fn docs(&self) -> Option<&str> {
        self.docs.as_deref()
    }

    /// Returns struct fields or enum variants.
    pub fn members(&self) -> &[MemberDeclaration] {
        &self.members
    }

    /// Returns the module path, if known.
    pub fn module_path(&self) -> Option<&str> {
        self.module_path.as_deref()
    }
}

/// Information about a public function or method.
///
/// T004: the historical `signature: String` field has been removed in favor
/// of structured fields (`params` / `returns` / `receiver` / `is_async`).
/// Callers that need a human-readable signature should construct a
/// `MethodDeclaration` from `TypeNode::methods` / `TraitNode::methods` and
/// call `MethodDeclaration::signature_string()`. This is a BRIDGE-01 JSON
/// breaking change (ADR 0002 Consequences C1).
#[derive(Debug, Clone)]
pub struct FunctionInfo {
    name: String,
    docs: Option<String>,
    /// Type names extracted from the return type (e.g., `["Published"]` for
    /// `-> Published`, `["User"]` for `-> Result<User, DomainError>`).
    /// Used by `build_type_graph` to compute typestate `outgoing` edges.
    return_type_names: Vec<String>,
    /// `true` if the first parameter is `self`, `&self`, or `&mut self`.
    has_self_receiver: bool,
    /// Structured parameter list (excluding the self receiver). L1 resolution:
    /// last-segment short names, generics preserved verbatim.
    params: Vec<ParamDeclaration>,
    /// Return type string at L1 resolution. `"()"` when the return type is
    /// the unit type.
    returns: String,
    /// Self-receiver form: `"&self"` / `"&mut self"` / `"self"`, or `None`
    /// for associated functions.
    receiver: Option<String>,
    /// Whether the function is declared `async fn`.
    is_async: bool,
    /// Module path for free functions (e.g., `"domain::review"`). `None` if
    /// not a free function, or if the module path could not be determined.
    ///
    /// T006 (S4): populated by `build_schema_export` using rustdoc `paths`.
    module_path: Option<String>,
}

impl FunctionInfo {
    /// Creates a new function info (backward-compatible: `module_path` defaults to `None`).
    ///
    /// `has_self_receiver` is derived from `receiver` to prevent the illegal state
    /// where `has_self_receiver = true` but `receiver = None` (or vice versa).
    /// Callers that pass an explicit `has_self_receiver` value should ensure it
    /// agrees with `receiver`; the constructor silently corrects disagreements by
    /// trusting `receiver` as the source of truth.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        name: String,
        docs: Option<String>,
        return_type_names: Vec<String>,
        _has_self_receiver: bool,
        params: Vec<ParamDeclaration>,
        returns: String,
        receiver: Option<String>,
        is_async: bool,
    ) -> Self {
        // Derive `has_self_receiver` from `receiver` so the two fields can never
        // disagree. The `_has_self_receiver` parameter is accepted for API
        // compatibility but is intentionally ignored.
        let has_self_receiver = receiver.is_some();
        Self {
            name,
            docs,
            return_type_names,
            has_self_receiver,
            params,
            returns,
            receiver,
            is_async,
            module_path: None,
        }
    }

    /// Creates a new function info with an explicit module path.
    ///
    /// T006 (S4): used by `build_schema_export` for free functions.
    #[allow(clippy::too_many_arguments)]
    pub fn with_module_path(
        name: String,
        docs: Option<String>,
        return_type_names: Vec<String>,
        _has_self_receiver: bool,
        params: Vec<ParamDeclaration>,
        returns: String,
        receiver: Option<String>,
        is_async: bool,
        module_path: Option<String>,
    ) -> Self {
        let has_self_receiver = receiver.is_some();
        Self {
            name,
            docs,
            return_type_names,
            has_self_receiver,
            params,
            returns,
            receiver,
            is_async,
            module_path,
        }
    }

    /// Returns the function name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the documentation string.
    pub fn docs(&self) -> Option<&str> {
        self.docs.as_deref()
    }

    /// Returns the type names extracted from the return type.
    pub fn return_type_names(&self) -> &[String] {
        &self.return_type_names
    }

    /// Returns `true` if the first parameter is `self`, `&self`, or `&mut self`.
    pub fn has_self_receiver(&self) -> bool {
        self.has_self_receiver
    }

    /// Returns the structured parameter list (excluding the self receiver).
    pub fn params(&self) -> &[ParamDeclaration] {
        &self.params
    }

    /// Returns the return type string.
    pub fn returns(&self) -> &str {
        &self.returns
    }

    /// Returns the self-receiver form, or `None` for associated functions.
    pub fn receiver(&self) -> Option<&str> {
        self.receiver.as_deref()
    }

    /// Returns `true` if the function is declared `async fn`.
    pub fn is_async(&self) -> bool {
        self.is_async
    }

    /// Returns the module path for this free function, if known.
    ///
    /// T006 (S4): populated for free functions extracted by `build_schema_export`.
    pub fn module_path(&self) -> Option<&str> {
        self.module_path.as_deref()
    }
}

/// Information about a public trait definition.
#[derive(Debug, Clone)]
pub struct TraitInfo {
    name: String,
    docs: Option<String>,
    /// Required and provided method signatures.
    methods: Vec<FunctionInfo>,
}

impl TraitInfo {
    /// Creates a new trait info.
    pub fn new(name: String, docs: Option<String>, methods: Vec<FunctionInfo>) -> Self {
        Self { name, docs, methods }
    }

    /// Returns the trait name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the documentation string.
    pub fn docs(&self) -> Option<&str> {
        self.docs.as_deref()
    }

    /// Returns the trait methods.
    pub fn methods(&self) -> &[FunctionInfo] {
        &self.methods
    }
}

/// Information about an impl block.
#[derive(Debug, Clone)]
pub struct ImplInfo {
    /// The type being implemented for (e.g., `TrackStatus`).
    target_type: String,
    /// The trait being implemented, if any (e.g., `Display`).
    trait_name: Option<String>,
    /// Methods defined in this impl block.
    methods: Vec<FunctionInfo>,
    /// Stable fully-qualified definition path of the trait from rustdoc `paths`
    /// (e.g., `"std::fmt::Display"`).  `None` for inherent impls or when the
    /// trait id is not present in `krate.paths`.  Used by
    /// `code_profile_builder` to key into `SchemaExport::trait_origins` (which
    /// is keyed by def_path, not by short name) so that two traits with the
    /// same short name are never confused.
    trait_def_path: Option<String>,
}

impl ImplInfo {
    /// Creates a new impl info (backward-compatible: `trait_def_path` defaults to `None`).
    pub fn new(
        target_type: String,
        trait_name: Option<String>,
        methods: Vec<FunctionInfo>,
    ) -> Self {
        Self { target_type, trait_name, methods, trait_def_path: None }
    }

    /// Creates a new impl info with an explicit `trait_def_path`.
    ///
    /// `trait_def_path` should be the fully-qualified definition path of the
    /// trait as found in `krate.paths` (e.g., `"std::fmt::Display"`).
    pub fn with_trait_def_path(
        target_type: String,
        trait_name: Option<String>,
        methods: Vec<FunctionInfo>,
        trait_def_path: Option<String>,
    ) -> Self {
        Self { target_type, trait_name, methods, trait_def_path }
    }

    /// Returns the target type name.
    pub fn target_type(&self) -> &str {
        &self.target_type
    }

    /// Returns the trait name if this is a trait impl.
    pub fn trait_name(&self) -> Option<&str> {
        self.trait_name.as_deref()
    }

    /// Returns the methods.
    pub fn methods(&self) -> &[FunctionInfo] {
        &self.methods
    }

    /// Returns the stable fully-qualified definition path of the trait, if known.
    ///
    /// `None` for inherent impls or when the trait id was not found in
    /// `krate.paths` during schema export.  When present, consumers should
    /// prefer this over `trait_name` for keying into `SchemaExport::trait_origins`.
    pub fn trait_def_path(&self) -> Option<&str> {
        self.trait_def_path.as_deref()
    }
}

/// Errors that can occur during schema export.
#[derive(Debug, thiserror::Error)]
pub enum SchemaExportError {
    /// Nightly Rust toolchain is not installed.
    #[error("nightly toolchain not found: install with `rustup toolchain install nightly`")]
    NightlyNotFound,
    /// `cargo rustdoc` command failed.
    #[error("rustdoc failed: {0}")]
    RustdocFailed(String),
    /// Failed to parse rustdoc JSON output.
    #[error("failed to parse rustdoc JSON: {0}")]
    ParseFailed(String),
    /// The specified crate was not found in the workspace.
    #[error("crate not found: {0}")]
    CrateNotFound(String),
}

/// Port trait for extracting a crate's public API schema.
///
/// Infrastructure implements this using rustdoc JSON generation and parsing.
pub trait SchemaExporter {
    /// Extracts the public API of the given crate.
    ///
    /// # Errors
    /// Returns `SchemaExportError` if nightly is missing, rustdoc fails, or parsing fails.
    fn export(&self, crate_name: &str) -> Result<SchemaExport, SchemaExportError>;
}

// ---------------------------------------------------------------------------
// TypeGraph — pre-indexed query interface for domain type evaluation
// ---------------------------------------------------------------------------

/// Pre-indexed view of a crate's public API for type catalogue evaluation.
///
/// Constructed from `SchemaExport` by infrastructure. The domain evaluation
/// layer uses only this type — no raw string parsing needed.
///
/// T004 (S3): adds `functions: HashMap<(String, Option<String>), FunctionNode>`
/// for in-memory free-function lookup keyed by `(short_name, module_path)`.
/// JSON serialization (T007 `baseline_codec`) converts the tuple key to a
/// fully-qualified string at codec boundaries.
#[derive(Debug, Clone)]
pub struct TypeGraph {
    types: HashMap<String, TypeNode>,
    traits: HashMap<String, TraitNode>,
    /// Free functions indexed by `(short_name, module_path)`.
    functions: HashMap<(String, Option<String>), FunctionNode>,
}

impl TypeGraph {
    /// Creates a new `TypeGraph` (backward-compatible: `functions` defaults to empty).
    #[must_use]
    pub fn new(types: HashMap<String, TypeNode>, traits: HashMap<String, TraitNode>) -> Self {
        Self { types, traits, functions: HashMap::new() }
    }

    /// Creates a new `TypeGraph` with an explicit `functions` map.
    #[must_use]
    pub fn with_functions(
        types: HashMap<String, TypeNode>,
        traits: HashMap<String, TraitNode>,
        functions: HashMap<(String, Option<String>), FunctionNode>,
    ) -> Self {
        Self { types, traits, functions }
    }

    /// Returns `true` if a type with the given name exists in the profile.
    #[must_use]
    pub fn has_type(&self, name: &str) -> bool {
        self.types.contains_key(name)
    }

    /// Returns the `TypeNode` for the given name, if present.
    #[must_use]
    pub fn get_type(&self, name: &str) -> Option<&TypeNode> {
        self.types.get(name)
    }

    /// Returns the `TraitNode` for the given name, if present.
    #[must_use]
    pub fn get_trait(&self, name: &str) -> Option<&TraitNode> {
        self.traits.get(name)
    }

    /// Returns a `TraitImplEntry` for a specific type + trait combination, if present.
    #[must_use]
    pub fn get_impl(&self, type_name: &str, trait_name: &str) -> Option<&TraitImplEntry> {
        self.types.get(type_name)?.trait_impls().iter().find(|i| i.trait_name() == trait_name)
    }

    /// Returns `true` if a function with the given `(short_name, module_path)` key exists.
    #[must_use]
    pub fn has_function(&self, short_name: &str, module_path: Option<&str>) -> bool {
        self.functions.contains_key(&(short_name.to_string(), module_path.map(str::to_string)))
    }

    /// Returns the `FunctionNode` for the given `(short_name, module_path)` key, if present.
    #[must_use]
    pub fn get_function(
        &self,
        short_name: &str,
        module_path: Option<&str>,
    ) -> Option<&FunctionNode> {
        self.functions.get(&(short_name.to_string(), module_path.map(str::to_string)))
    }

    /// Returns the functions map.
    #[must_use]
    pub fn functions(&self) -> &HashMap<(String, Option<String>), FunctionNode> {
        &self.functions
    }

    /// Returns an iterator over all type names in this graph.
    pub fn type_names(&self) -> impl Iterator<Item = &String> {
        self.types.keys()
    }

    /// Returns an iterator over all trait names in this graph.
    pub fn trait_names(&self) -> impl Iterator<Item = &String> {
        self.traits.keys()
    }
}

/// A public type in the crate, as indexed for TDDD evaluation.
///
/// The legacy `method_return_types: HashSet<String>` bridge field is removed.
/// `outgoing` is passed in directly from `build_type_graph`, which derives it
/// by filtering self-receiver method return types against the set of
/// typestate-declared type names (ADR 0002 Q4). Callers that need the full set
/// of return type names should walk `methods().iter().flat_map(|m| ...)` directly.
#[derive(Debug, Clone)]
pub struct TypeNode {
    kind: TypeKind,
    /// Variants (for enums) or fields (for structs).
    members: Vec<MemberDeclaration>,
    /// Full L1 signatures of inherent impl methods on this type.
    methods: Vec<MethodDeclaration>,
    /// Trait implementations on this type (e.g., `impl TraitName for Self`).
    trait_impls: Vec<TraitImplEntry>,
    /// Outgoing typestate transitions.
    outgoing: HashSet<String>,
    /// Module path for disambiguation (e.g., `"domain::review"`). `None` if unknown.
    module_path: Option<String>,
}

impl TypeNode {
    /// Creates a new `TypeNode`.
    ///
    /// `outgoing` is passed in by `build_type_graph` already filtered to the
    /// typestate set — this constructor stores it as-is.
    #[must_use]
    pub fn new(
        kind: TypeKind,
        members: Vec<MemberDeclaration>,
        methods: Vec<MethodDeclaration>,
        outgoing: HashSet<String>,
    ) -> Self {
        Self { kind, members, methods, trait_impls: Vec::new(), outgoing, module_path: None }
    }

    /// Sets the module path for disambiguation.
    pub fn set_module_path(&mut self, path: String) {
        self.module_path = Some(path);
    }

    /// Returns the kind of this type.
    #[must_use]
    pub fn kind(&self) -> &TypeKind {
        &self.kind
    }

    /// Returns struct fields or enum variants.
    #[must_use]
    pub fn members(&self) -> &[MemberDeclaration] {
        &self.members
    }

    /// Returns the structured inherent method declarations.
    #[must_use]
    pub fn methods(&self) -> &[MethodDeclaration] {
        &self.methods
    }

    /// Returns the trait implementations on this type.
    #[must_use]
    pub fn trait_impls(&self) -> &[TraitImplEntry] {
        &self.trait_impls
    }

    /// Sets the trait implementations for this type.
    pub fn set_trait_impls(&mut self, impls: Vec<TraitImplEntry>) {
        self.trait_impls = impls;
    }

    /// Returns the module path, if known.
    #[must_use]
    pub fn module_path(&self) -> Option<&str> {
        self.module_path.as_deref()
    }

    /// Returns outgoing typestate transitions.
    #[must_use]
    pub fn outgoing(&self) -> &HashSet<String> {
        &self.outgoing
    }
}

/// A trait implementation found on a type, as indexed for TDDD evaluation.
///
/// Represents one `impl TraitName for TypeName { ... }` block. The evaluator
/// uses this to verify that a `SecondaryAdapter` entry's declared trait
/// implementations actually exist in the crate profile.
///
/// T004 (S3): adds `origin_crate` to distinguish workspace-owned traits from
/// external/std traits. Signal evaluators (T008 IN-10) filter on this field
/// to avoid false-positive reverse extras for external crate traits.
#[derive(Debug, Clone)]
pub struct TraitImplEntry {
    trait_name: String,
    methods: Vec<MethodDeclaration>,
    /// The crate that defines this trait (e.g., `"domain"`, `"std"`).
    /// `""` (empty string) when the origin could not be determined.
    origin_crate: String,
}

impl TraitImplEntry {
    /// Creates a new `TraitImplEntry`.
    #[must_use]
    pub fn new(trait_name: impl Into<String>, methods: Vec<MethodDeclaration>) -> Self {
        Self { trait_name: trait_name.into(), methods, origin_crate: String::new() }
    }

    /// Creates a new `TraitImplEntry` with an explicit `origin_crate`.
    #[must_use]
    pub fn with_origin_crate(
        trait_name: impl Into<String>,
        methods: Vec<MethodDeclaration>,
        origin_crate: impl Into<String>,
    ) -> Self {
        Self { trait_name: trait_name.into(), methods, origin_crate: origin_crate.into() }
    }

    /// Returns the trait name (last-segment short name).
    #[must_use]
    pub fn trait_name(&self) -> &str {
        &self.trait_name
    }

    /// Returns the methods implemented for this trait.
    #[must_use]
    pub fn methods(&self) -> &[MethodDeclaration] {
        &self.methods
    }

    /// Returns the crate that defines this trait.
    ///
    /// Returns an empty string when the origin is unknown (default for entries
    /// created via `TraitImplEntry::new` before T006 populates the field).
    #[must_use]
    pub fn origin_crate(&self) -> &str {
        &self.origin_crate
    }
}

// ---------------------------------------------------------------------------
// FunctionNode — a public free function in the crate, indexed for evaluation
// ---------------------------------------------------------------------------

/// A public free function in the crate, as indexed for TDDD evaluation.
///
/// T004 (S3): new type. Represents a top-level `pub fn` (not a method).
/// Stored in `TypeGraph::functions` under the key `(short_name, module_path)`.
/// Serialization/deserialization (T007 `baseline_codec`) uses a fully-qualified
/// string key (`"crate::module::fn_name"`) — the in-memory tuple key is
/// converted at codec boundaries.
#[derive(Debug, Clone)]
pub struct FunctionNode {
    /// Structured parameter list at L1 resolution (excluding any self receiver).
    params: Vec<ParamDeclaration>,
    /// Return type names extracted from the return type (last-segment short names).
    returns: Vec<String>,
    /// Whether the function is declared `async fn`.
    is_async: bool,
    /// Module path for scoped lookup (e.g., `"domain::review"`). `None` if unknown.
    module_path: Option<String>,
}

impl FunctionNode {
    /// Creates a new `FunctionNode`.
    #[must_use]
    pub fn new(
        params: Vec<ParamDeclaration>,
        returns: Vec<String>,
        is_async: bool,
        module_path: Option<String>,
    ) -> Self {
        Self { params, returns, is_async, module_path }
    }

    /// Returns the structured parameter list (L1 resolution, no self receiver).
    #[must_use]
    pub fn params(&self) -> &[ParamDeclaration] {
        &self.params
    }

    /// Returns the return type names (last-segment short names).
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

/// A public trait in the crate, as indexed for TDDD evaluation.
///
/// The legacy `method_names: Vec<String>` mirror is removed. Callers that need
/// the list of method names should walk `methods().iter().map(|m| m.name())`.
#[derive(Debug, Clone)]
pub struct TraitNode {
    methods: Vec<MethodDeclaration>,
}

impl TraitNode {
    /// Creates a new `TraitNode` from the structured method list.
    #[must_use]
    pub fn new(methods: Vec<MethodDeclaration>) -> Self {
        Self { methods }
    }

    /// Returns the structured method declarations.
    #[must_use]
    pub fn methods(&self) -> &[MethodDeclaration] {
        &self.methods
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn function_info_has_self_receiver_reflects_constructor_value() {
        let method = FunctionInfo::new(
            "consume".to_string(),
            None,
            vec!["Done".to_string()],
            true,
            vec![],
            "Done".to_string(),
            Some("self".to_string()),
            false,
        );
        assert!(method.has_self_receiver());
        assert_eq!(method.receiver(), Some("self"));
        assert!(method.params().is_empty());

        let assoc_fn = FunctionInfo::new(
            "from_db".to_string(),
            None,
            vec!["Published".to_string()],
            false,
            vec![],
            "Published".to_string(),
            None,
            false,
        );
        assert!(!assoc_fn.has_self_receiver());
        assert_eq!(assoc_fn.receiver(), None);
    }

    #[test]
    fn schema_export_accessors_return_correct_values() {
        let export = SchemaExport::new(
            "domain".to_string(),
            vec![TypeInfo::new(
                "TrackStatus".to_string(),
                TypeKind::Enum,
                Some("Status of a track".to_string()),
                vec![
                    MemberDeclaration::variant("Planned"),
                    MemberDeclaration::variant("InProgress"),
                ],
            )],
            vec![],
            vec![],
            vec![],
        );

        assert_eq!(export.crate_name(), "domain");
        assert_eq!(export.types().len(), 1);
        let track_status = export.types().first().unwrap();
        assert_eq!(track_status.name(), "TrackStatus");
        assert_eq!(track_status.kind(), &TypeKind::Enum);
        let member_names: Vec<&str> =
            track_status.members().iter().map(MemberDeclaration::name).collect();
        assert_eq!(member_names, vec!["Planned", "InProgress"]);
    }

    #[test]
    fn type_info_module_path_none_by_default() {
        let ti = TypeInfo::new("Foo".to_string(), TypeKind::Struct, None, vec![]);
        assert!(ti.module_path().is_none());
    }

    #[test]
    fn type_info_with_module_path_stores_path() {
        let ti = TypeInfo::with_module_path(
            "Error".to_string(),
            TypeKind::Enum,
            None,
            vec![],
            "domain::review".to_string(),
        );
        assert_eq!(ti.module_path(), Some("domain::review"));
    }

    #[test]
    fn type_node_module_path_none_by_default() {
        let node = TypeNode::new(TypeKind::Struct, vec![], vec![], HashSet::new());
        assert!(node.module_path().is_none());
    }

    #[test]
    fn type_node_set_module_path_stores_value() {
        let mut node = TypeNode::new(TypeKind::Struct, vec![], vec![], HashSet::new());
        node.set_module_path("domain::guard".to_string());
        assert_eq!(node.module_path(), Some("domain::guard"));
    }

    // --- T003 TDDD-05: TraitImplEntry + TypeNode::trait_impls + TypeGraph::get_impl ---

    #[test]
    fn test_trait_impl_entry_accessors() {
        let methods = vec![MethodDeclaration::new(
            "find",
            Some("&self".into()),
            vec![ParamDeclaration::new("id", "ReviewId")],
            "Option<Review>",
            false,
        )];
        let entry = TraitImplEntry::new("ReviewReader", methods);
        assert_eq!(entry.trait_name(), "ReviewReader");
        assert_eq!(entry.methods().len(), 1);
        assert_eq!(entry.methods()[0].name(), "find");
    }

    #[test]
    fn test_type_node_trait_impls_default_empty() {
        let node = TypeNode::new(TypeKind::Struct, vec![], vec![], HashSet::new());
        assert!(node.trait_impls().is_empty());
    }

    #[test]
    fn test_type_graph_get_impl_returns_entry() {
        let mut node = TypeNode::new(TypeKind::Struct, vec![], vec![], HashSet::new());
        node.set_trait_impls(vec![TraitImplEntry::new("ReviewReader", vec![])]);
        let mut types = HashMap::new();
        types.insert("FsReviewStore".to_string(), node);
        let graph = TypeGraph::new(types, HashMap::new());
        let entry = graph.get_impl("FsReviewStore", "ReviewReader");
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().trait_name(), "ReviewReader");
    }

    #[test]
    fn test_type_graph_get_impl_returns_none_for_wrong_trait() {
        let mut node = TypeNode::new(TypeKind::Struct, vec![], vec![], HashSet::new());
        node.set_trait_impls(vec![TraitImplEntry::new("ReviewReader", vec![])]);
        let mut types = HashMap::new();
        types.insert("FsReviewStore".to_string(), node);
        let graph = TypeGraph::new(types, HashMap::new());
        assert!(graph.get_impl("FsReviewStore", "ReviewWriter").is_none());
    }

    #[test]
    fn test_type_graph_get_impl_returns_none_for_missing_type() {
        let graph = TypeGraph::new(HashMap::new(), HashMap::new());
        assert!(graph.get_impl("NonExistent", "ReviewReader").is_none());
    }

    #[test]
    fn trait_node_exposes_structured_methods() {
        let methods = vec![
            MethodDeclaration::new(
                "save",
                Some("&self".into()),
                vec![],
                "Result<(), Error>",
                false,
            ),
            MethodDeclaration::new(
                "find",
                Some("&self".into()),
                vec![ParamDeclaration::new("id", "UserId")],
                "Option<User>",
                false,
            ),
        ];
        let node = TraitNode::new(methods);
        let names: Vec<&str> = node.methods().iter().map(MethodDeclaration::name).collect();
        assert_eq!(names, vec!["save", "find"]);
        assert_eq!(node.methods().len(), 2);
    }

    // --- T004 (S3): TraitImplEntry::origin_crate ---

    #[test]
    fn test_trait_impl_entry_new_has_empty_origin_crate() {
        let entry = TraitImplEntry::new("Display", vec![]);
        assert_eq!(entry.origin_crate(), "");
    }

    #[test]
    fn test_trait_impl_entry_with_origin_crate_stores_value() {
        let entry = TraitImplEntry::with_origin_crate("TrackReader", vec![], "domain");
        assert_eq!(entry.trait_name(), "TrackReader");
        assert_eq!(entry.origin_crate(), "domain");
        assert!(entry.methods().is_empty());
    }

    #[test]
    fn test_trait_impl_entry_with_origin_crate_external() {
        let entry = TraitImplEntry::with_origin_crate("Display", vec![], "std");
        assert_eq!(entry.origin_crate(), "std");
    }

    // --- T004 (S3): FunctionNode ---

    #[test]
    fn test_function_node_accessors() {
        let params = vec![ParamDeclaration::new("id", "TrackId")];
        let returns = vec!["Option<Track>".to_string()];
        let node = FunctionNode::new(
            params.clone(),
            returns.clone(),
            false,
            Some("domain::track".to_string()),
        );
        assert_eq!(node.params().len(), 1);
        assert_eq!(node.params()[0].name(), "id");
        assert_eq!(node.returns(), &["Option<Track>"]);
        assert!(!node.is_async());
        assert_eq!(node.module_path(), Some("domain::track"));
    }

    #[test]
    fn test_function_node_async_flag() {
        let node = FunctionNode::new(vec![], vec!["()".to_string()], true, None);
        assert!(node.is_async());
        assert!(node.module_path().is_none());
    }

    // --- T004 (S3): TypeGraph::functions ---

    #[test]
    fn test_type_graph_new_has_empty_functions() {
        let graph = TypeGraph::new(HashMap::new(), HashMap::new());
        assert!(graph.functions().is_empty());
    }

    #[test]
    fn test_type_graph_with_functions_stores_entries() {
        let mut functions = HashMap::new();
        let key = ("build_baseline".to_string(), Some("infra::tddd".to_string()));
        let node = FunctionNode::new(
            vec![],
            vec!["TypeBaseline".to_string()],
            false,
            Some("infra::tddd".to_string()),
        );
        functions.insert(key.clone(), node);

        let graph = TypeGraph::with_functions(HashMap::new(), HashMap::new(), functions);
        assert!(graph.has_function("build_baseline", Some("infra::tddd")));
        assert!(!graph.has_function("build_baseline", None));
        assert!(!graph.has_function("other_fn", Some("infra::tddd")));
    }

    #[test]
    fn test_type_graph_get_function_returns_correct_node() {
        let mut functions = HashMap::new();
        let node = FunctionNode::new(
            vec![ParamDeclaration::new("x", "u32")],
            vec!["String".to_string()],
            false,
            None,
        );
        functions.insert(("render".to_string(), None), node);

        let graph = TypeGraph::with_functions(HashMap::new(), HashMap::new(), functions);
        let result = graph.get_function("render", None);
        assert!(result.is_some());
        assert_eq!(result.unwrap().params().len(), 1);
    }

    #[test]
    fn test_type_graph_get_function_returns_none_for_missing() {
        let graph = TypeGraph::new(HashMap::new(), HashMap::new());
        assert!(graph.get_function("nonexistent", None).is_none());
    }

    #[test]
    fn test_type_graph_has_function_with_no_module_path() {
        let mut functions = HashMap::new();
        functions.insert(
            ("top_level_fn".to_string(), None),
            FunctionNode::new(vec![], vec![], false, None),
        );
        let graph = TypeGraph::with_functions(HashMap::new(), HashMap::new(), functions);
        assert!(graph.has_function("top_level_fn", None));
        assert!(!graph.has_function("top_level_fn", Some("some::module")));
    }
}
