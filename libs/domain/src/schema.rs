//! Domain types for the `export-schema` feature (BRIDGE-01).
//!
//! These types represent the public API surface of a Rust crate as extracted
//! from rustdoc JSON. `Serialize` is derived for JSON output.

use std::collections::{HashMap, HashSet};

use serde::Serialize;

/// Top-level export result containing all public API elements of a crate.
#[derive(Debug, Clone, Serialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum TypeKind {
    /// A struct (with named fields, tuple fields, or unit).
    Struct,
    /// An enum with variants.
    Enum,
    /// A type alias.
    TypeAlias,
}

/// Information about a public type (struct, enum, or type alias).
#[derive(Debug, Clone, Serialize)]
pub struct TypeInfo {
    name: String,
    kind: TypeKind,
    docs: Option<String>,
    /// For enums: variant names. For structs: field names. Empty for type aliases.
    members: Vec<String>,
}

impl TypeInfo {
    /// Creates a new type info.
    pub fn new(name: String, kind: TypeKind, docs: Option<String>, members: Vec<String>) -> Self {
        Self { name, kind, docs, members }
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

    /// Returns variant names (enums) or field names (structs).
    pub fn members(&self) -> &[String] {
        &self.members
    }
}

/// Information about a public function or method.
#[derive(Debug, Clone, Serialize)]
pub struct FunctionInfo {
    name: String,
    /// Human-readable signature string (e.g., `fn foo(x: u32) -> bool`).
    signature: String,
    docs: Option<String>,
    /// Type names extracted from the return type (e.g., `["Published"]` for `-> Published`).
    return_type_names: Vec<String>,
    /// `true` if the first parameter is `self`, `&self`, or `&mut self`.
    has_self_receiver: bool,
}

impl FunctionInfo {
    /// Creates a new function info.
    pub fn new(
        name: String,
        signature: String,
        docs: Option<String>,
        return_type_names: Vec<String>,
        has_self_receiver: bool,
    ) -> Self {
        Self { name, signature, docs, return_type_names, has_self_receiver }
    }

    /// Returns the function name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the signature string.
    pub fn signature(&self) -> &str {
        &self.signature
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
}

/// Information about a public trait definition.
#[derive(Debug, Clone, Serialize)]
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
#[derive(Debug, Clone, Serialize)]
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
// CodeProfile — pre-indexed query interface for domain type evaluation
// ---------------------------------------------------------------------------

/// Pre-indexed view of a crate's public API for domain type evaluation.
///
/// Constructed from `SchemaExport` by infrastructure. The domain evaluation
/// layer uses only this type — no raw string parsing needed.
#[derive(Debug, Clone)]
pub struct CodeProfile {
    types: HashMap<String, CodeType>,
    traits: HashMap<String, CodeTrait>,
}

impl CodeProfile {
    /// Creates a new `CodeProfile`.
    #[must_use]
    pub fn new(types: HashMap<String, CodeType>, traits: HashMap<String, CodeTrait>) -> Self {
        Self { types, traits }
    }

    /// Returns `true` if a type with the given name exists in the profile.
    #[must_use]
    pub fn has_type(&self, name: &str) -> bool {
        self.types.contains_key(name)
    }

    /// Returns the `CodeType` for the given name, if present.
    #[must_use]
    pub fn get_type(&self, name: &str) -> Option<&CodeType> {
        self.types.get(name)
    }

    /// Returns the `CodeTrait` for the given name, if present.
    #[must_use]
    pub fn get_trait(&self, name: &str) -> Option<&CodeTrait> {
        self.traits.get(name)
    }
}

/// A public type in the crate.
#[derive(Debug, Clone)]
pub struct CodeType {
    kind: TypeKind,
    /// Variant names (for enums) or field names (for structs).
    members: Vec<String>,
    /// Type names returned by inherent (non-trait) impl methods.
    ///
    /// `Result<T, E>` and `Option<T>` are unwrapped to extract `T`.
    /// Only the last path segment is stored (e.g., `"Published"` not `"crate::Published"`).
    method_return_types: HashSet<String>,
}

impl CodeType {
    /// Creates a new `CodeType`.
    #[must_use]
    pub fn new(kind: TypeKind, members: Vec<String>, method_return_types: HashSet<String>) -> Self {
        Self { kind, members, method_return_types }
    }

    /// Returns the kind of this type.
    #[must_use]
    pub fn kind(&self) -> &TypeKind {
        &self.kind
    }

    /// Returns variant names (enums) or field names (structs).
    #[must_use]
    pub fn members(&self) -> &[String] {
        &self.members
    }

    /// Returns type names returned by inherent impl methods.
    #[must_use]
    pub fn method_return_types(&self) -> &HashSet<String> {
        &self.method_return_types
    }
}

/// A public trait in the crate.
#[derive(Debug, Clone)]
pub struct CodeTrait {
    method_names: Vec<String>,
}

impl CodeTrait {
    /// Creates a new `CodeTrait`.
    #[must_use]
    pub fn new(method_names: Vec<String>) -> Self {
        Self { method_names }
    }

    /// Returns the method names of this trait.
    #[must_use]
    pub fn method_names(&self) -> &[String] {
        &self.method_names
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
            "fn consume(self) -> Done".to_string(),
            None,
            vec!["Done".to_string()],
            true,
        );
        assert!(method.has_self_receiver());

        let assoc_fn = FunctionInfo::new(
            "from_db".to_string(),
            "fn from_db() -> Published".to_string(),
            None,
            vec!["Published".to_string()],
            false,
        );
        assert!(!assoc_fn.has_self_receiver());
    }

    #[test]
    fn schema_export_accessors_return_correct_values() {
        let export = SchemaExport::new(
            "domain".to_string(),
            vec![TypeInfo::new(
                "TrackStatus".to_string(),
                TypeKind::Enum,
                Some("Status of a track".to_string()),
                vec!["Planned".to_string(), "InProgress".to_string()],
            )],
            vec![],
            vec![],
            vec![],
        );

        assert_eq!(export.crate_name(), "domain");
        assert_eq!(export.types().len(), 1);
        assert_eq!(export.types().first().unwrap().name(), "TrackStatus");
        assert_eq!(export.types().first().unwrap().kind(), &TypeKind::Enum);
        assert_eq!(export.types().first().unwrap().members(), &["Planned", "InProgress"]);
    }
}
