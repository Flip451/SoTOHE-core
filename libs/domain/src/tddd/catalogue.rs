//! Type catalogue — declared type entries for the per-track TDDD catalogue.
//!
//! This module owns the core building-block types shared across the TDDD
//! catalogue framework: `TypeSignal`, `MemberDeclaration`, `MethodDeclaration`,
//! `ParamDeclaration`, and `EnumVariantDeclaration`.
//!
//! Historical note (T001): this file used to hold all three responsibilities in
//! a single 2088-line module under the name `DomainType*`. The split and the
//! rename were performed together in the TDDD-01 track (see ADR
//! `knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md` D3 and DM-06 in
//! `knowledge/strategy/TODO.md`).
//!
//! ## T008 partial — V1 type migration (MethodDeclaration / ParamDeclaration)
//!
//! The V1 `MethodDeclaration` and `ParamDeclaration` (plain `String` fields) were
//! deleted and replaced by `catalogue_v2::methods` equivalents (newtype fields:
//! `MethodName`, `ParamName`, `TypeRef`, `SelfReceiver`).  All workspace call sites
//! were migrated to the V2 newtype API in the same change; the workspace compiles
//! cleanly with no String-argument callers remaining.  The `pub use` below
//! re-exports the V2 types at the old `catalogue::*` path so that import paths stay
//! stable across the migration (not a backward-compatibility shim).

use crate::ConfidenceSignal;
use crate::spec::SpecValidationError;
// Re-exports that preserve the `catalogue::MethodDeclaration` / `catalogue::ParamDeclaration`
// module paths after V1 types were deleted and replaced by V2 (catalogue_v2::methods).
// All call sites have been migrated to the V2 newtype API (MethodName / ParamName / TypeRef /
// SelfReceiver); the `pub use` keeps import paths stable across the migration.
// These types are NOT source-compatible with the old V1 constructor signatures.
pub use crate::tddd::catalogue_v2::methods::{MethodDeclaration, ParamDeclaration};

// ---------------------------------------------------------------------------
// Identifier validation helpers (module-private)
// ---------------------------------------------------------------------------

/// Returns `true` if `s` is a syntactically valid Rust enum-variant identifier.
///
/// Rules applied:
/// - Matches `[a-zA-Z_][a-zA-Z0-9_]*` (ASCII-only subset of the Rust identifier grammar).
/// - Rejects the bare wildcard `"_"` which is a placeholder in Rust and cannot serve as a
///   meaningful enum-variant name.
///
/// Keyword names are accepted because rustdoc strips the `r#` prefix from raw identifiers
/// (e.g. `r#type` is exported as `"type"`), so rejecting keywords would create false
/// contract mismatches against valid Rust enums that use raw identifiers as variant names.
///
/// Rust also permits XID_Continue Unicode characters in identifiers, but catalogue entries
/// always use ASCII-only L1 names so ASCII-only checking is the correct invariant here.
fn is_valid_rust_identifier(s: &str) -> bool {
    if s == "_" {
        return false;
    }
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
// EnumVariantDeclaration — enum variant with optional payload types
// ---------------------------------------------------------------------------

/// An enum variant declaration capturing the variant name and its payload
/// type strings at L1 resolution.
///
/// `payload_types` holds the complete type strings (generic arguments included)
/// for each field in the variant payload:
/// - Tuple variant `Foo(Bar, Baz)` → `payload_types: ["Bar", "Baz"]`
/// - Struct variant `Foo { x: Bar }` → `payload_types: ["Bar"]`
/// - Unit variant `Foo` → `payload_types: []`
///
/// Type strings use last-segment short names; module paths containing `::`
/// are rejected by codec validation (L1 invariant, CN-03).
///
/// # Field visibility and invariant contract
///
/// `name` and `payload_types` are declared `pub` so that they appear in the
/// rustdoc-derived schema export, allowing the TDDD L2 member-visibility check
/// to verify that these fields match the catalogue declaration
/// (`expected_members: ["name", "payload_types"]`). Without `pub` visibility,
/// the fields are invisible to rustdoc and the L2 check reports a yellow signal.
///
/// **Accepted encapsulation tradeoff**: direct field assignment (e.g.
/// `evd.name = "invalid::name".to_string()`) bypasses the validation enforced
/// by [`try_new`](EnumVariantDeclaration::try_new). This is the same category of
/// known bypass as the infallible [`new`](EnumVariantDeclaration::new) constructor
/// documented below. All application and codec paths use `try_new` or `new` for
/// construction; callers that require validated invariants must go through one of
/// those constructors and must not mutate fields directly afterwards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumVariantDeclaration {
    pub name: String,
    pub payload_types: Vec<String>,
}

impl EnumVariantDeclaration {
    /// Creates a new `EnumVariantDeclaration`, validating that `name` is a
    /// syntactically valid Rust identifier and that each entry in `payload_types`
    /// follows the L1 short-name convention (no `::` module path separators).
    ///
    /// A valid variant name matches `[a-zA-Z_][a-zA-Z0-9_]*` — no spaces,
    /// no path separators (`::`) and no other punctuation that cannot appear
    /// in a real Rust enum variant.
    ///
    /// Prefer this constructor in application / test code to enforce the domain
    /// invariants that a variant name must identify a real Rust variant and that
    /// type strings must use L1 last-segment short names (CN-03).
    ///
    /// # Errors
    ///
    /// Returns `SpecValidationError::EmptyVariantName` if `name` is empty or
    /// contains only whitespace.
    ///
    /// Returns `SpecValidationError::InvalidVariantName` if `name` is non-empty
    /// but fails the Rust-identifier character rules.
    ///
    /// Returns `SpecValidationError::InvalidPayloadType` if any `payload_types` entry
    /// is empty, contains only whitespace, or contains `::` (module path separator,
    /// violates CN-03 L1 invariant).
    pub fn try_new(
        name: impl Into<String>,
        payload_types: Vec<String>,
    ) -> Result<Self, SpecValidationError> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(SpecValidationError::EmptyVariantName);
        }
        if !is_valid_rust_identifier(&name) {
            return Err(SpecValidationError::InvalidVariantName(name));
        }
        for pt in &payload_types {
            // Reject empty, whitespace-only, strings with leading/trailing whitespace, and
            // strings with '::' module path separators (CN-03 L1 invariant).
            if pt.trim().is_empty() || pt.as_str() != pt.trim() || pt.contains("::") {
                return Err(SpecValidationError::InvalidPayloadType(pt.clone()));
            }
        }
        Ok(Self { name, payload_types })
    }

    /// Creates a new `EnumVariantDeclaration` without name validation.
    ///
    /// **Prefer `try_new` in new call sites.** This infallible variant exists
    /// for codec and render paths that have already validated the name upstream
    /// (e.g. `catalogue_document_codec` rejects empty names at the JSON boundary).
    /// Passing an empty or whitespace-only name produces a value that violates
    /// the domain invariant.
    #[must_use]
    pub fn new(name: impl Into<String>, payload_types: Vec<String>) -> Self {
        Self { name: name.into(), payload_types }
    }

    /// Returns the variant name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the payload type strings (empty for unit variants).
    #[must_use]
    pub fn payload_types(&self) -> &[String] {
        &self.payload_types
    }
}

// ---------------------------------------------------------------------------
// MemberDeclaration — composite type member (enum variant or struct field)
// ---------------------------------------------------------------------------

/// A member of a composite type: either an enum variant (name + payload types)
/// or a struct field (name + type string).
///
/// **Enum-first design** (see `knowledge/conventions/prefer-type-safe-abstractions.md` § Enum-first):
/// the two states carry structurally distinct data — a variant has a name and
/// payload types while a field has a name and a type string. A
/// `struct { name, ty: Option<String> }` shape would allow the illegal
/// `Field { ty: None }` state; the enum shape prevents it at compile time.
///
/// Type strings (on `Field` and in `Variant.payload_types`) follow the same L1
/// convention as `MethodDeclaration`: last-segment short names, generics preserved
/// verbatim. Module paths containing `::` are rejected by codec validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemberDeclaration {
    /// An enum variant: name + payload type strings at L1 resolution.
    /// Unit variants carry an empty `payload_types` vec.
    Variant(EnumVariantDeclaration),
    /// A struct field with its type string.
    Field { name: String, ty: String },
}

impl MemberDeclaration {
    /// Creates a new enum-variant member with the given payload types.
    #[must_use]
    pub fn variant(name: impl Into<String>, payload_types: Vec<String>) -> Self {
        Self::Variant(EnumVariantDeclaration::new(name, payload_types))
    }

    /// Creates a new unit enum-variant member (no payload).
    #[must_use]
    pub fn unit_variant(name: impl Into<String>) -> Self {
        Self::Variant(EnumVariantDeclaration::new(name, Vec::new()))
    }

    /// Creates a new struct-field member.
    #[must_use]
    pub fn field(name: impl Into<String>, ty: impl Into<String>) -> Self {
        Self::Field { name: name.into(), ty: ty.into() }
    }

    /// Returns the member name regardless of kind.
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::Variant(evd) => evd.name(),
            Self::Field { name, .. } => name,
        }
    }

    /// Returns the field type, or `None` for enum variants.
    #[must_use]
    pub fn ty(&self) -> Option<&str> {
        match self {
            Self::Variant(_) => None,
            Self::Field { ty, .. } => Some(ty),
        }
    }
}

// ---------------------------------------------------------------------------
// TypeSignal
// ---------------------------------------------------------------------------

/// Per-type signal evaluation result: the confidence signal for one catalogue
/// entry, derived from the 3-way diff (catalogue declaration / merged baseline /
/// live scanned code). Element type of [`crate::tddd::type_signals_doc::TypeSignalsDocument`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeSignal {
    type_name: String,
    /// Canonical kind tag (e.g. `"typestate"`, `"enum"`, `"value_object"`, …).
    kind_tag: String,
    signal: ConfidenceSignal,
    /// Whether the type was found in the scanned code.
    found_type: bool,
    /// Items (variants / methods / transitions) found in the scanned code.
    found_items: Vec<String>,
    /// Expected items that were not found.
    missing_items: Vec<String>,
    /// Items found in code that were not listed in the entry.
    extra_items: Vec<String>,
}

impl TypeSignal {
    /// Creates a new `TypeSignal`.
    #[must_use]
    pub fn new(
        type_name: impl Into<String>,
        kind_tag: impl Into<String>,
        signal: ConfidenceSignal,
        found_type: bool,
        found_items: Vec<String>,
        missing_items: Vec<String>,
        extra_items: Vec<String>,
    ) -> Self {
        Self {
            type_name: type_name.into(),
            kind_tag: kind_tag.into(),
            signal,
            found_type,
            found_items,
            missing_items,
            extra_items,
        }
    }

    /// Returns the type name.
    #[must_use]
    pub fn type_name(&self) -> &str {
        &self.type_name
    }

    /// Returns the canonical kind tag string.
    #[must_use]
    pub fn kind_tag(&self) -> &str {
        &self.kind_tag
    }

    /// Returns the confidence signal computed from the scan result.
    #[must_use]
    pub fn signal(&self) -> ConfidenceSignal {
        self.signal
    }

    /// Returns the confidence signal as a lowercase string (`"blue"`, `"yellow"`, or `"red"`).
    ///
    /// Callers that only need to branch on the signal level (e.g. CLI display
    /// code under CN-01) should prefer this over `signal()` so they do not need
    /// to import `domain::ConfidenceSignal` directly.
    #[must_use]
    pub fn signal_as_str(&self) -> &'static str {
        match self.signal {
            ConfidenceSignal::Blue => "blue",
            ConfidenceSignal::Yellow => "yellow",
            ConfidenceSignal::Red => "red",
        }
    }

    /// Returns `true` if the type was found during the code scan.
    #[must_use]
    pub fn found_type(&self) -> bool {
        self.found_type
    }

    /// Returns the list of items that were found in the scanned code.
    #[must_use]
    pub fn found_items(&self) -> &[String] {
        &self.found_items
    }

    /// Returns the list of expected items not found in the scanned code.
    #[must_use]
    pub fn missing_items(&self) -> &[String] {
        &self.missing_items
    }

    /// Returns the list of items found in code but not declared in the entry.
    #[must_use]
    pub fn extra_items(&self) -> &[String] {
        &self.extra_items
    }
}

// ---------------------------------------------------------------------------
// Tests — type definitions
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use super::*;

    // --- EnumVariantDeclaration ---

    #[test]
    fn test_enum_variant_declaration_try_new_with_valid_name_succeeds() {
        let evd = EnumVariantDeclaration::try_new("Active", vec![]).unwrap();
        assert_eq!(evd.name(), "Active");
        assert!(evd.payload_types().is_empty());
    }

    #[test]
    fn test_enum_variant_declaration_try_new_with_payload_types_succeeds() {
        let evd =
            EnumVariantDeclaration::try_new("Wrap", vec!["String".into(), "u32".into()]).unwrap();
        assert_eq!(evd.name(), "Wrap");
        assert_eq!(evd.payload_types(), &["String", "u32"]);
    }

    #[test]
    fn test_enum_variant_declaration_try_new_with_empty_name_returns_error() {
        let result = EnumVariantDeclaration::try_new("", vec![]);
        assert!(matches!(result, Err(SpecValidationError::EmptyVariantName)));
    }

    #[test]
    fn test_enum_variant_declaration_try_new_with_whitespace_name_returns_error() {
        let result = EnumVariantDeclaration::try_new("   ", vec![]);
        assert!(matches!(result, Err(SpecValidationError::EmptyVariantName)));
    }

    #[test]
    fn test_enum_variant_declaration_try_new_with_space_in_name_returns_invalid_error() {
        let result = EnumVariantDeclaration::try_new("Foo Bar", vec![]);
        assert!(matches!(result, Err(SpecValidationError::InvalidVariantName(_))));
    }

    #[test]
    fn test_enum_variant_declaration_try_new_with_path_separator_returns_invalid_error() {
        let result = EnumVariantDeclaration::try_new("Foo::Bar", vec![]);
        assert!(matches!(result, Err(SpecValidationError::InvalidVariantName(_))));
    }

    #[test]
    fn test_enum_variant_declaration_try_new_with_underscore_prefix_succeeds() {
        let evd = EnumVariantDeclaration::try_new("_Private", vec![]).unwrap();
        assert_eq!(evd.name(), "_Private");
    }

    #[test]
    fn test_enum_variant_declaration_try_new_with_bare_underscore_returns_invalid_error() {
        let result = EnumVariantDeclaration::try_new("_", vec![]);
        assert!(matches!(result, Err(SpecValidationError::InvalidVariantName(_))));
    }

    /// rustdoc strips `r#` from raw-identifier variant names (e.g. `r#type` → `"type"`),
    /// so keyword strings must be accepted as valid variant names. Otherwise valid
    /// Rust enums using raw identifiers would create false contract mismatches.
    #[test]
    fn test_enum_variant_declaration_try_new_with_rust_keyword_succeeds() {
        for kw in ["fn", "type", "union", "match", "where"] {
            let evd = EnumVariantDeclaration::try_new(kw, vec![]).unwrap_or_else(|e| {
                panic!("keyword '{kw}' must be accepted (raw-identifier): {e}")
            });
            assert_eq!(evd.name(), kw);
        }
    }

    #[test]
    fn test_enum_variant_declaration_try_new_with_module_path_payload_type_returns_error() {
        let result = EnumVariantDeclaration::try_new("Wrap", vec!["domain::UserId".into()]);
        assert!(matches!(result, Err(SpecValidationError::InvalidPayloadType(_))));
    }

    #[test]
    fn test_enum_variant_declaration_try_new_with_valid_payload_type_succeeds() {
        let evd = EnumVariantDeclaration::try_new("Wrap", vec!["UserId".into()]).unwrap();
        assert_eq!(evd.payload_types(), &["UserId"]);
    }

    #[test]
    fn test_enum_variant_declaration_try_new_with_empty_payload_type_returns_error() {
        let result = EnumVariantDeclaration::try_new("Wrap", vec!["".into()]);
        assert!(matches!(result, Err(SpecValidationError::InvalidPayloadType(_))));
    }

    #[test]
    fn test_enum_variant_declaration_try_new_with_whitespace_payload_type_returns_error() {
        let result = EnumVariantDeclaration::try_new("Wrap", vec!["  ".into()]);
        assert!(matches!(result, Err(SpecValidationError::InvalidPayloadType(_))));
    }

    #[test]
    fn test_enum_variant_declaration_try_new_with_leading_whitespace_payload_type_returns_error() {
        let result = EnumVariantDeclaration::try_new("Wrap", vec![" UserId".into()]);
        assert!(matches!(result, Err(SpecValidationError::InvalidPayloadType(_))));
    }

    #[test]
    fn test_enum_variant_declaration_try_new_with_trailing_whitespace_payload_type_returns_error() {
        let result = EnumVariantDeclaration::try_new("Wrap", vec!["UserId ".into()]);
        assert!(matches!(result, Err(SpecValidationError::InvalidPayloadType(_))));
    }

    #[test]
    fn test_enum_variant_declaration_try_new_with_generic_payload_type_succeeds() {
        // Generic payload types like "Vec<String>" or "Result<User, DomainError>" are valid.
        let evd = EnumVariantDeclaration::try_new("Wrap", vec!["Result<User, DomainError>".into()])
            .unwrap();
        assert_eq!(evd.payload_types(), &["Result<User, DomainError>"]);
    }

    #[test]
    fn test_enum_variant_declaration_try_new_with_alphanumeric_name_succeeds() {
        let evd = EnumVariantDeclaration::try_new("State1", vec![]).unwrap();
        assert_eq!(evd.name(), "State1");
    }

    #[test]
    fn test_member_declaration_variant_constructor_with_payload_types() {
        let m = MemberDeclaration::variant("Wrap", vec!["i64".into()]);
        assert_eq!(m.name(), "Wrap");
        assert!(m.ty().is_none());
        if let MemberDeclaration::Variant(evd) = &m {
            assert_eq!(evd.payload_types(), &["i64"]);
        } else {
            panic!("expected Variant");
        }
    }

    // --- TypeSignal ---

    #[test]
    fn test_type_signal_accessors() {
        let signal = TypeSignal::new(
            "TrackStatus",
            "enum",
            ConfidenceSignal::Yellow,
            true,
            vec!["Active".into()],
            vec!["Done".into()],
            vec!["Legacy".into()],
        );
        assert_eq!(signal.type_name(), "TrackStatus");
        assert_eq!(signal.kind_tag(), "enum");
        assert_eq!(signal.signal(), ConfidenceSignal::Yellow);
        assert!(signal.found_type());
        assert_eq!(signal.found_items(), &["Active"]);
        assert_eq!(signal.missing_items(), &["Done"]);
        assert_eq!(signal.extra_items(), &["Legacy"]);
    }
}
