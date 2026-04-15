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
#[derive(Debug, Clone)]
pub struct SchemaExport {
    crate_name: String,
    types: Vec<TypeInfo>,
    functions: Vec<FunctionInfo>,
    traits: Vec<TraitInfo>,
    impls: Vec<ImplInfo>,
}

impl SchemaExport {
    /// Creates a new schema export.
    pub fn new(
        crate_name: String,
        types: Vec<TypeInfo>,
        functions: Vec<FunctionInfo>,
        traits: Vec<TraitInfo>,
        impls: Vec<ImplInfo>,
    ) -> Self {
        Self { crate_name, types, functions, traits, impls }
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
}

impl FunctionInfo {
    /// Creates a new function info.
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
}

impl ImplInfo {
    /// Creates a new impl info.
    pub fn new(
        target_type: String,
        trait_name: Option<String>,
        methods: Vec<FunctionInfo>,
    ) -> Self {
        Self { target_type, trait_name, methods }
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
#[derive(Debug, Clone)]
pub struct TypeGraph {
    types: HashMap<String, TypeNode>,
    traits: HashMap<String, TraitNode>,
}

impl TypeGraph {
    /// Creates a new `TypeGraph`.
    #[must_use]
    pub fn new(types: HashMap<String, TypeNode>, traits: HashMap<String, TraitNode>) -> Self {
        Self { types, traits }
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
/// T005 (Phase 1 Task 5): the legacy `method_return_types: HashSet<String>`
/// bridge field is removed. `outgoing` is passed in directly from
/// `build_type_graph`, which derives it by filtering self-receiver method
/// return types against the set of typestate-declared type names
/// (ADR 0002 Q4). Callers that need the full set of return type names
/// should walk `methods().iter().flat_map(|m| ...)` directly.
#[derive(Debug, Clone)]
pub struct TypeNode {
    kind: TypeKind,
    /// Variants (for enums) or fields (for structs).
    members: Vec<MemberDeclaration>,
    /// Full L1 signatures of inherent impl methods on this type.
    methods: Vec<MethodDeclaration>,
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
        Self { kind, members, methods, outgoing, module_path: None }
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

/// A public trait in the crate, as indexed for TDDD evaluation.
///
/// T005 (Phase 1 Task 5): the legacy `method_names: Vec<String>` mirror is
/// removed. Callers that need the list of method names should walk
/// `methods().iter().map(|m| m.name())`.
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
#[allow(clippy::unwrap_used)]
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
}
