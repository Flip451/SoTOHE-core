//! Newtype wrappers for catalogue v2 identifier types.
//!
//! All 13 newtypes are implemented here, with validation and `Display` / `FromStr`
//! derived or hand-implemented as appropriate.
//!
//! No serde derives are attached — per ADR `knowledge/adr/2026-04-14-1531-domain-serde-ripout.md`,
//! the domain layer is serialization-free. Serde codec lives in the infrastructure
//! layer (catalogue_v2 codec, to be implemented in a later task).
//!
//! Validation rules (ADR 1 D5):
//! - `Identifier`: non-empty, ASCII alphanumeric + underscore, no leading digit.
//! - Newtypes wrapping `Identifier` inherit its validation.
//! - `ModulePath`: `Vec<Identifier>` joined with `::`.
//! - `TypeRef`: free-form type string (generics allowed, `::` allowed).
//! - `FunctionPath`: struct with `crate_name`, `module_path`, `name`.

use std::fmt;
use std::str::FromStr;

#[path = "identifiers_helpers.rs"]
mod helpers;

use helpers::is_valid_rust_identifier;

mod fully_qualified_item_path;

/// Namespace carried by a catalogue item identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CatalogueItemNamespace {
    /// A data type / type alias identity.
    Type,
    /// A trait identity.
    Trait,
}

/// Fully qualified identity for a catalogue type or trait entry.
///
/// A catalogue declaration may omit `module_path` while it is still being
/// resolved. That state is represented by an `Unplaced*` variant rather than
/// by treating the crate root as an implicit placement. The variants also keep
/// the type and trait namespaces separate, so same-named declarations cannot be
/// collapsed accidentally.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FullyQualifiedItemPath {
    /// A type with an explicitly known module path.
    PlacedType { crate_name: CrateName, module_path: ModulePath, name: Identifier },
    /// A type whose module path is not specified by the catalogue declaration.
    UnplacedType { crate_name: CrateName, name: Identifier },
    /// A trait with an explicitly known module path.
    PlacedTrait { crate_name: CrateName, module_path: ModulePath, name: Identifier },
    /// A trait whose module path is not specified by the catalogue declaration.
    UnplacedTrait { crate_name: CrateName, name: Identifier },
}

impl fmt::Debug for FullyQualifiedItemPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = formatter.debug_struct("FullyQualifiedItemPath");
        debug.field("crate_name", self.crate_name());
        if let Some(module_path) = self.module_path() {
            debug.field("module_path", module_path);
        } else {
            debug.field("module_path", &None::<&ModulePath>);
        }
        debug.field("name", self.name());
        debug.field("path", &self.to_string());
        debug.finish()
    }
}

impl fmt::Display for FullyQualifiedItemPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.crate_name(), f)?;
        if let Some(module_path) = self.module_path().filter(|path| !path.is_root()) {
            write!(f, "::{module_path}")?;
        }
        write!(f, "::{}", self.name())
    }
}

// ---------------------------------------------------------------------------
// IdentifierError — shared error type for all identifier newtypes
// ---------------------------------------------------------------------------

/// Error type for `Identifier` and all newtype wrappers around it.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum IdentifierError {
    /// The identifier string was empty.
    #[error("identifier must not be empty")]
    Empty,

    /// The identifier contains characters outside ASCII alphanumeric + underscore,
    /// or starts with a digit.
    #[error(
        "identifier '{0}' is not a valid Rust identifier \
         (must match [a-zA-Z_][a-zA-Z0-9_]*)"
    )]
    InvalidCharacters(String),

    /// A module path segment was invalid.
    #[error("module path segment '{0}' is not a valid Rust identifier")]
    InvalidSegment(String),

    /// The `FunctionPath` string format was invalid (not `crate::module::name` shape).
    #[error(
        "function path '{0}' could not be parsed; expected form '<crate_name>[::<module_segment>...].<function_name>'"
    )]
    InvalidFunctionPath(String),
}

// ---------------------------------------------------------------------------
// Identifier — common base newtype
// ---------------------------------------------------------------------------

/// Common base newtype for Rust identifier validation.
///
/// Invariants: non-empty, ASCII alphanumeric + underscore only, no leading digit.
/// Shared validation base for `TypeName`, `TraitName`, `FieldName`, `MethodName`,
/// `ParamName`, `VariantName`, `CrateName`, and `FunctionName` (ADR 1 D5).
///
/// # Errors
///
/// [`FromStr`] returns `IdentifierError::Empty` for empty input and
/// `IdentifierError::InvalidCharacters` for strings that fail the Rust identifier
/// character rules.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Identifier(String);

impl Identifier {
    /// Creates a new `Identifier`, validating the Rust identifier rules.
    ///
    /// # Errors
    ///
    /// Returns `IdentifierError::Empty` for empty input.
    /// Returns `IdentifierError::InvalidCharacters` if `s` fails identifier rules.
    pub fn new(s: impl Into<String>) -> Result<Self, IdentifierError> {
        let s = s.into();
        if s.is_empty() {
            return Err(IdentifierError::Empty);
        }
        if !is_valid_rust_identifier(&s) {
            return Err(IdentifierError::InvalidCharacters(s));
        }
        Ok(Self(s))
    }

    /// Returns the underlying string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Identifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for Identifier {
    type Err = IdentifierError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

// ---------------------------------------------------------------------------
// Macro: declare a newtype wrapping Identifier
// ---------------------------------------------------------------------------

macro_rules! identifier_newtype {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(Identifier);

        impl $name {
            /// Creates a new instance, validating the Rust identifier rules.
            ///
            /// # Errors
            ///
            /// Returns `IdentifierError::Empty` for empty input.
            /// Returns `IdentifierError::InvalidCharacters` if the string fails identifier rules.
            pub fn new(s: impl Into<String>) -> Result<Self, IdentifierError> {
                Identifier::new(s).map(Self)
            }

            /// Returns the underlying string slice.
            #[must_use]
            pub fn as_str(&self) -> &str {
                self.0.as_str()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(&self.0, f)
            }
        }

        impl FromStr for $name {
            type Err = IdentifierError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Self::new(s)
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }
    };
}

// ---------------------------------------------------------------------------
// Identifier-based newtypes (8 types)
// ---------------------------------------------------------------------------

identifier_newtype!(
    /// Newtype around `Identifier` for type short names.
    ///
    /// Used as the key type in `CatalogueDocument::types` map (ADR 1 D5 / D11).
    TypeName
);

identifier_newtype!(
    /// Newtype around `Identifier` for trait short names.
    ///
    /// Used as the key type in `CatalogueDocument::traits` map (ADR 1 D5 / D11).
    TraitName
);

identifier_newtype!(
    /// Newtype around `Identifier` for struct field names.
    ///
    /// Used in `FieldDecl` (ADR 1 D5).
    FieldName
);

identifier_newtype!(
    /// Newtype around `Identifier` for method names.
    ///
    /// Used in `MethodDeclaration` and `TypestateTransitions::transition_methods`
    /// (ADR 1 D5 / D3).
    MethodName
);

identifier_newtype!(
    /// Newtype around `Identifier` for function / method parameter names.
    ///
    /// Used in `ParamDeclaration` (ADR 1 D5).
    ParamName
);

identifier_newtype!(
    /// Newtype around `Identifier` for enum variant names.
    ///
    /// Used in `VariantDecl` (ADR 1 D5).
    VariantName
);

identifier_newtype!(
    /// Newtype around `Identifier` for crate names.
    ///
    /// Used in `FunctionPath::crate_name` and `TraitImplDeclV2::origin_crate` (ADR 1 D5 / D10).
    CrateName
);

identifier_newtype!(
    /// Newtype around `Identifier` for function names in `FunctionPath` (ADR 1 D5 / D11).
    FunctionName
);

identifier_newtype!(
    /// Validated name for a declared invariant.
    ///
    /// Uses the same non-empty Rust identifier validation as the other catalogue v2
    /// identifier-backed newtypes.
    InvariantName
);

identifier_newtype!(
    /// Validated name for an associated constant in a trait (e.g. `ID` in `const ID: ChainId`).
    ///
    /// Associated constant names follow the same Rust identifier rules as other
    /// catalogue v2 identifier-backed newtypes. Distinct from `TypeName` (associated
    /// type names) and `MethodName` (method names) to make illegal names unrepresentable
    /// at the domain model level (ADR prefer-type-safe-abstractions).
    AssocConstName
);

// ---------------------------------------------------------------------------
// RustExpressionError — error type for RustExpression
// ---------------------------------------------------------------------------

/// Error type returned by [`RustExpression::try_new`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RustExpressionError {
    /// The expression string was empty.
    #[error("Rust expression must not be empty")]
    Empty,
    /// The expression string has leading or trailing whitespace.
    #[error("Rust expression must not have leading or trailing whitespace, got: '{0}'")]
    WhitespaceBoundary(String),
}

// ---------------------------------------------------------------------------
// RustExpression — validated Rust expression string
// ---------------------------------------------------------------------------

/// Newtype wrapping `String` for validated Rust expression strings.
///
/// Used as the type of `AssocConstDecl::default_value` to encode the "Rust
/// expression" contract in the type system per
/// `knowledge/conventions/prefer-type-safe-abstractions.md` § Newtype.
///
/// # Validation (performed in [`RustExpression::try_new`])
///
/// - Non-empty string (rejects `""`).
/// - Rejects leading or trailing ASCII/Unicode whitespace.
///
/// Full Rust syntax parsing is deliberately NOT performed here — only trivial
/// rejections are applied. The infrastructure codec layer applies additional
/// `syn::parse_str::<syn::Expr>` validation at the boundary.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RustExpression(String);

impl RustExpression {
    /// Creates a new `RustExpression`, rejecting empty strings and strings with
    /// leading/trailing whitespace.
    ///
    /// # Errors
    ///
    /// Returns [`RustExpressionError::Empty`] for empty input.
    /// Returns [`RustExpressionError::WhitespaceBoundary`] if the string starts
    /// or ends with whitespace.
    pub fn try_new(s: impl Into<String>) -> Result<Self, RustExpressionError> {
        let s = s.into();
        if s.is_empty() {
            return Err(RustExpressionError::Empty);
        }
        if s != s.trim() {
            return Err(RustExpressionError::WhitespaceBoundary(s));
        }
        Ok(Self(s))
    }

    /// Returns the underlying string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RustExpression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

// ---------------------------------------------------------------------------
// DocString — documentation-text newtype
// ---------------------------------------------------------------------------

/// Newtype wrapping `String` for entry documentation text.
///
/// Wraps the opaque free-text documentation carried by the catalogue entry value
/// objects (`TypeEntry` / `TraitEntry` / `FunctionEntry`). Keeps a raw `String` out
/// of named-field position in those types so they satisfy the catalogue linter's
/// `ForbidPrimitiveInTypes` rule (ADR `2026-07-04-0525-catalogue-v2-entry-lint-conformance` D4).
///
/// Documentation is arbitrary free text, so construction is infallible — no
/// validation is performed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocString(String);

impl DocString {
    /// Creates a new `DocString` from arbitrary documentation text.
    #[must_use]
    pub fn new(text: String) -> Self {
        Self(text)
    }

    /// Returns the underlying documentation text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

// ---------------------------------------------------------------------------
// ModulePath — Vec<Identifier> joined with ::
// ---------------------------------------------------------------------------

/// Newtype wrapping `Vec<Identifier>` for module path segment lists.
///
/// Serializes as a `::` joined string (e.g. `"tddd::catalogue"`).
/// An empty `ModulePath` represents the crate root.
/// `serde default` (when the field is absent in JSON) should decode to empty vec;
/// the codec layer handles this default (ADR 1 D7).
///
/// # Errors
///
/// [`FromStr`] splits on `::` and validates each segment as an `Identifier`.
/// Returns `IdentifierError::Empty` if the input is empty.
/// Returns `IdentifierError::InvalidSegment` for any segment that fails identifier rules.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct ModulePath(Vec<Identifier>);

impl ModulePath {
    /// Creates an empty `ModulePath` representing the crate root.
    #[must_use]
    pub fn root() -> Self {
        Self(Vec::new())
    }

    /// Creates a `ModulePath` from a slice of already-validated `Identifier`s.
    #[must_use]
    pub fn from_identifiers(segments: Vec<Identifier>) -> Self {
        Self(segments)
    }

    /// Creates a `ModulePath` from a slice of string segments, validating each.
    ///
    /// # Errors
    ///
    /// Returns `IdentifierError::InvalidSegment` if any segment fails identifier rules.
    pub fn from_segments<S: Into<String>>(segments: Vec<S>) -> Result<Self, IdentifierError> {
        let mut out = Vec::with_capacity(segments.len());
        for seg in segments {
            let s = seg.into();
            Identifier::new(s.clone()).map_err(|_| IdentifierError::InvalidSegment(s.clone()))?;
            out.push(Identifier(s));
        }
        Ok(Self(out))
    }

    /// Returns the segments as a slice of `Identifier`s.
    #[must_use]
    pub fn segments(&self) -> &[Identifier] {
        &self.0
    }

    /// Returns `true` if the module path has no segments (crate root).
    #[must_use]
    pub fn is_root(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Display for ModulePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut first = true;
        for seg in &self.0 {
            if !first {
                f.write_str("::")?;
            }
            first = false;
            fmt::Display::fmt(seg, f)?;
        }
        Ok(())
    }
}

impl FromStr for ModulePath {
    type Err = IdentifierError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            return Ok(Self::root());
        }
        let segments: Result<Vec<Identifier>, _> = s
            .split("::")
            .map(|seg| {
                Identifier::new(seg).map_err(|_| IdentifierError::InvalidSegment(seg.to_string()))
            })
            .collect();
        Ok(Self(segments?))
    }
}

// ---------------------------------------------------------------------------
// TypeRef — free-form type reference string
// ---------------------------------------------------------------------------

/// Newtype wrapping `String` for generics-inclusive type reference strings.
///
/// Examples: `"Result<Option<User>, DomainError>"`, `"Vec<UserId>"`, `"domain_core::UserId"`.
/// Allows angle brackets, commas, and `::` for crate-prefixed cross-crate references
/// (ADR 2 D11). Distinct from `Identifier` (ADR 1 D5).
///
/// Validation: must be non-empty. Generic parse is deferred to the codec layer (T005).
///
/// # Errors
///
/// [`FromStr`] returns `IdentifierError::Empty` for empty input.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TypeRef(String);

impl TypeRef {
    /// Creates a new `TypeRef`, validating that the string is non-empty.
    ///
    /// # Errors
    ///
    /// Returns `IdentifierError::Empty` for empty input.
    pub fn new(s: impl Into<String>) -> Result<Self, IdentifierError> {
        let s = s.into();
        if s.is_empty() {
            return Err(IdentifierError::Empty);
        }
        Ok(Self(s))
    }

    /// Returns the underlying string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TypeRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for TypeRef {
    type Err = IdentifierError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl AsRef<str> for TypeRef {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

// ---------------------------------------------------------------------------
// FunctionPath — full-path key for FunctionEntry BTreeMap
// ---------------------------------------------------------------------------

/// Full-path key for `FunctionEntry` `BTreeMap`: `crate_name + module_path + name`.
///
/// Cross-workspace functions use crate name prefix directly (no `crate::` prefix;
/// no `::` leading) per ADR 1 D11. Example: `"domain_core::register_user"`.
///
/// `module_path` defaults to empty (crate root) when the function is at crate root level
/// (ADR 1 D7).
///
/// Display format: `<crate_name>[::<module_path>]::<name>`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FunctionPath {
    /// The crate name prefix.
    pub crate_name: CrateName,
    /// The module path segments (empty = crate root).
    pub module_path: ModulePath,
    /// The function's short name.
    pub name: FunctionName,
}

impl FunctionPath {
    /// Creates a new `FunctionPath`.
    #[must_use]
    pub fn new(crate_name: CrateName, module_path: ModulePath, name: FunctionName) -> Self {
        Self { crate_name, module_path, name }
    }

    /// Creates a `FunctionPath` where the function is at crate root.
    #[must_use]
    pub fn at_root(crate_name: CrateName, name: FunctionName) -> Self {
        Self { crate_name, module_path: ModulePath::root(), name }
    }
}

impl fmt::Display for FunctionPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.crate_name, f)?;
        if !self.module_path.is_root() {
            write!(f, "::{}", self.module_path)?;
        }
        write!(f, "::{}", self.name)
    }
}

impl FromStr for FunctionPath {
    type Err = IdentifierError;

    /// Parses a function path of the form `<crate_name>[::<seg>...]<::<function_name>]`.
    ///
    /// The last `::` separated segment is the function name; everything before the last
    /// segment is interpreted as `<crate_name>[::<module_path>]`.
    ///
    /// # Errors
    ///
    /// Returns `IdentifierError::InvalidFunctionPath` if the string has fewer than 2
    /// `::` separated segments (i.e., at minimum `<crate>::<function>`).
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split("::").collect();
        if parts.len() < 2 {
            return Err(IdentifierError::InvalidFunctionPath(s.to_string()));
        }
        let crate_part =
            parts.first().ok_or_else(|| IdentifierError::InvalidFunctionPath(s.to_string()))?;
        let name_part =
            parts.last().ok_or_else(|| IdentifierError::InvalidFunctionPath(s.to_string()))?;

        let crate_name = CrateName::new(*crate_part)
            .map_err(|_| IdentifierError::InvalidFunctionPath(s.to_string()))?;
        let name = FunctionName::new(*name_part)
            .map_err(|_| IdentifierError::InvalidFunctionPath(s.to_string()))?;

        // Middle segments form the module path (everything between first and last).
        // parts.len() >= 2 is guaranteed by the check above, so saturating_sub(1) >= 1.
        let end = parts.len().saturating_sub(1);
        let module_segments: Vec<String> =
            parts.get(1..end).unwrap_or_default().iter().map(|seg| seg.to_string()).collect();
        let module_path = ModulePath::from_segments(module_segments)
            .map_err(|_| IdentifierError::InvalidFunctionPath(s.to_string()))?;

        Ok(Self { crate_name, module_path, name })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "identifiers_tests.rs"]
mod tests;

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod assoc_const_name_tests {
    use super::*;

    #[test]
    fn test_assoc_const_name_with_valid_name_succeeds() {
        let name = AssocConstName::new("CHAIN_ID").unwrap();
        assert_eq!(name.as_str(), "CHAIN_ID");
    }

    #[test]
    fn test_assoc_const_name_with_empty_string_returns_empty_error() {
        assert_eq!(AssocConstName::new(""), Err(IdentifierError::Empty));
    }

    #[test]
    fn test_assoc_const_name_with_leading_digit_returns_invalid_characters_error() {
        let result = AssocConstName::new("1BAD");
        assert!(matches!(result, Err(IdentifierError::InvalidCharacters(_))));
    }

    #[test]
    fn test_assoc_const_name_display_fromstr_roundtrip() {
        let original = AssocConstName::new("MAX_RETRIES").unwrap();
        let displayed = original.to_string();
        let parsed: AssocConstName = displayed.parse().unwrap();
        assert_eq!(original, parsed);
    }
}

#[cfg(test)]
mod doc_string_tests {
    use super::*;

    #[test]
    fn test_doc_string_new_preserves_text() {
        let doc = DocString::new("A domain entity.".to_string());
        assert_eq!(doc.as_str(), "A domain entity.");
    }

    #[test]
    fn test_doc_string_empty_is_allowed() {
        let doc = DocString::new(String::new());
        assert_eq!(doc.as_str(), "");
    }

    #[test]
    fn test_doc_string_equality_by_value() {
        assert_eq!(DocString::new("x".to_string()), DocString::new("x".to_string()));
        assert_ne!(DocString::new("x".to_string()), DocString::new("y".to_string()));
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod rust_expression_tests {
    use super::*;

    #[test]
    fn test_rust_expression_empty_string_is_rejected() {
        assert_eq!(RustExpression::try_new(""), Err(RustExpressionError::Empty));
    }

    #[test]
    fn test_rust_expression_leading_whitespace_is_rejected() {
        let result = RustExpression::try_new(" 42");
        assert!(
            matches!(result, Err(RustExpressionError::WhitespaceBoundary(_))),
            "expected WhitespaceBoundary, got: {result:?}"
        );
    }

    #[test]
    fn test_rust_expression_trailing_whitespace_is_rejected() {
        let result = RustExpression::try_new("42 ");
        assert!(
            matches!(result, Err(RustExpressionError::WhitespaceBoundary(_))),
            "expected WhitespaceBoundary, got: {result:?}"
        );
    }

    #[test]
    fn test_rust_expression_valid_expressions_are_accepted() {
        assert!(RustExpression::try_new("42").is_ok());
        assert!(RustExpression::try_new("DEFAULT_CHAIN_ID").is_ok());
        assert!(RustExpression::try_new("vec![1,2,3]").is_ok());
    }

    #[test]
    fn test_rust_expression_display_returns_underlying_string() {
        let expr = RustExpression::try_new("DEFAULT_CHAIN_ID").unwrap();
        assert_eq!(expr.to_string(), "DEFAULT_CHAIN_ID");
    }
}
