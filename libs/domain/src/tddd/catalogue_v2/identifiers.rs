//! Newtype wrappers for catalogue v2 identifier types.
//!
//! All 12 newtypes are implemented here, with validation and `Display` / `FromStr`
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
// Internal helper
// ---------------------------------------------------------------------------

/// Returns `true` if `s` is a syntactically valid Rust identifier fragment:
/// - Non-empty
/// - First character: ASCII alphabetic or underscore
/// - Remaining characters: ASCII alphanumeric or underscore
fn is_valid_rust_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        None => false,
        Some(first) => {
            (first.is_ascii_alphabetic() || first == '_')
                && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        }
    }
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

pub(super) use identifier_newtype;

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
