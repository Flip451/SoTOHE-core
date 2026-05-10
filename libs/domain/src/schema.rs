//! Domain types for the `export-schema` feature (BRIDGE-01).
//!
//! These types represent the public API surface of a Rust crate as extracted
//! from rustdoc JSON. The BRIDGE-01 JSON wire format is owned by
//! `infrastructure::schema_export_codec`.
//!
//! T008: `TypeGraph` / `TypeNode` / `TraitNode` / `TraitImplEntry` /
//! `FunctionNode` are deleted — signal evaluation now operates on
//! `rustdoc_types::Crate` / `ExtendedCrate` directly via `SignalEvaluatorV2`.

use std::collections::HashMap;

use crate::tddd::catalogue::MemberDeclaration;
use crate::tddd::catalogue_v2::methods::ParamDeclaration;

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
/// T004: the historical `signature: String` field has been replaced by
/// structured fields (`params` / `returns` / `receiver` / `is_async`).
#[derive(Debug, Clone)]
pub struct FunctionInfo {
    name: String,
    docs: Option<String>,
    /// Type names extracted from the return type.
    return_type_names: Vec<String>,
    /// `true` if the first parameter is `self`, `&self`, or `&mut self`.
    has_self_receiver: bool,
    /// Structured parameter list (excluding the self receiver).
    params: Vec<ParamDeclaration>,
    /// Return type string at L1 resolution.
    returns: String,
    /// Self-receiver form, or `None` for associated functions.
    receiver: Option<String>,
    /// Whether the function is declared `async fn`.
    is_async: bool,
    /// Module path for free functions.
    module_path: Option<String>,
}

impl FunctionInfo {
    /// Creates a new function info (backward-compatible: `module_path` defaults to `None`).
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
    /// Stable fully-qualified definition path of the trait from rustdoc `paths`.
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
        use crate::tddd::catalogue::MemberDeclaration;
        let export = SchemaExport::new(
            "domain".to_string(),
            vec![TypeInfo::new(
                "TrackStatus".to_string(),
                TypeKind::Enum,
                Some("Status of a track".to_string()),
                vec![
                    MemberDeclaration::unit_variant("Planned"),
                    MemberDeclaration::unit_variant("InProgress"),
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
}
