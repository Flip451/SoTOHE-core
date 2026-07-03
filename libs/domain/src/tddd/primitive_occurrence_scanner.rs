//! Primitive-occurrence scanning port (ADR `2026-07-01-0004` D1/D2/D3).
//!
//! Defines the domain-owned vocabulary for detecting occurrences of
//! caller-specified "primitive" type names (e.g. `String`, `i32`) inside a
//! single catalogue [`TypeRef`]'s parsed type-tree, and the
//! [`PrimitiveOccurrenceScanner`] secondary port that performs the scan.
//! syn-based parsing itself is confined to an infrastructure adapter
//! (`SynPrimitiveOccurrenceScanner`); this module has no dependency on `syn`.

use std::collections::{BTreeMap, BTreeSet};

use crate::tddd::catalogue_v2::identifiers::{Identifier, IdentifierError, TypeRef};
use crate::tddd::catalogue_v2::roles::NonEmptyVec;

// ---------------------------------------------------------------------------
// PrimitiveOccurrencePosition — 7-variant call-site / detection-site taxonomy
// ---------------------------------------------------------------------------

/// Position taxonomy for primitive-occurrence detection inside a single
/// catalogue [`TypeRef`]'s parsed type-tree (ADR `2026-07-01-0004` D1/D2).
///
/// `NamedField` / `VariantField` / `Param` / `Return` / `Bound` /
/// `TypeAliasTarget` are the catalogue-structural call sites a `TypeRef` can be
/// scanned from; the caller (`evaluate_catalogue_lint`) already knows which
/// one applies before invoking the scanner, since it is iterating the
/// corresponding catalogue slot, and passes it as
/// [`PrimitiveOccurrenceScanner::scan`]'s `position` argument.
///
/// `Param` / `Return` / `Bound` are reused, not caller-exclusive: the scan
/// additionally applies these same three labels to occurrences found by
/// recursively descending into a nested callable-type signature's own
/// parameter / return slots, or into a nested generic-bound clause, discovered
/// anywhere inside the scanned `TypeRef`'s tree independent of the outer
/// `position` the caller supplied -- e.g. a `String` occurring as the
/// parameter type of a `Box<dyn Fn(String) -> _>` field is labelled `Param`
/// even though the field itself is scanned with `position = NamedField`.
///
/// `ResultErr` is the one position that is exclusively scan-intrinsic and
/// never caller-suppliable: the second generic argument of a
/// 2-generic-argument `Result` path segment found anywhere in the tree (at any
/// nesting depth), reclassified as `ResultErr` regardless of the outer
/// `position` or of nested `Param`/`Return`/`Bound` detection. Passing
/// `ResultErr` as the `position` argument to
/// [`PrimitiveOccurrenceScanner::scan`] is a caller contract violation
/// reported via [`PrimitiveOccurrenceScanError::InvalidSitePosition`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PrimitiveOccurrencePosition {
    /// A named struct field's declared type.
    NamedField,
    /// An enum variant field's declared type.
    VariantField,
    /// A method or function parameter's declared type.
    Param,
    /// A method or function return type.
    Return,
    /// A generic bound clause.
    Bound,
    /// A `type_alias`'s target type.
    TypeAliasTarget,
    /// The second (`Err`) type argument of a two-argument `Result` path
    /// segment. Exclusively scan-intrinsic; never caller-suppliable as
    /// `scan`'s `position` argument.
    ResultErr,
}

// ---------------------------------------------------------------------------
// PrimitiveName — validated bare Rust identifier
// ---------------------------------------------------------------------------

/// Validated bare Rust identifier naming one forbidden/candidate primitive
/// type (e.g. `"String"`, `"i32"`, `"u8"`) for primitive-occurrence scanning
/// (ADR `2026-07-01-0004` D1/D2).
///
/// Deliberately a newtype rather than an enum because the forbidden-primitive
/// name set itself remains open-ended and user-configurable by design (ADR
/// D1). Deliberately not a reuse of [`TypeRef`] (too permissive -- allows
/// generics and `::` path qualifiers that could never match a single
/// `syn::Ident` during a scan) or of the `catalogue_v2::identifiers`
/// `TypeName` / `FieldName` newtypes (both scoped to catalogue-authoring-surface
/// concepts unrelated to "a candidate identifier searched for during a syn AST
/// walk"). Wraps [`Identifier`] directly since both share the same validation
/// shape (non-empty, ASCII alphanumeric + underscore, no leading digit).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PrimitiveName(Identifier);

impl PrimitiveName {
    /// Constructs a `PrimitiveName` from any string-like input, validating it
    /// is a syntactically valid Rust identifier (non-empty, ASCII
    /// alphanumeric + underscore, no leading digit) -- the same shape a
    /// `syn::Ident` token itself requires, since `PrimitiveName` values are
    /// compared against parsed `syn::Ident` nodes by exact equality during a
    /// scan.
    ///
    /// # Errors
    ///
    /// Returns `IdentifierError::Empty` for empty input.
    /// Returns `IdentifierError::InvalidCharacters` if `s` fails identifier rules.
    pub fn new(s: impl Into<String>) -> Result<Self, IdentifierError> {
        Identifier::new(s).map(Self)
    }

    /// Returns the underlying string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

// ---------------------------------------------------------------------------
// PrimitiveOccurrenceReport — outcome of scanning one TypeRef
// ---------------------------------------------------------------------------

/// Outcome of scanning one [`TypeRef`]'s parsed type-tree for a requested set
/// of forbidden-primitive names (ADR `2026-07-01-0004` D2).
///
/// Occurrences are grouped by [`PrimitiveOccurrencePosition`]: each key is one
/// position the scan is able to distinguish (the caller-supplied top-level
/// site for occurrences found at the top of the tree, or one of
/// `Param` / `Return` / `Bound` / `ResultErr` for occurrences found via
/// recursive descent), and each value is the subset of requested primitive
/// names found at that position. `evaluate_catalogue_lint` checks, for each
/// position in a `ForbidPrimitiveInTypes` rule's `positions` set, whether
/// [`Self::by_position`] holds a non-empty entry for that position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrimitiveOccurrenceReport {
    occurrences: BTreeMap<PrimitiveOccurrencePosition, BTreeSet<PrimitiveName>>,
}

impl PrimitiveOccurrenceReport {
    /// Constructs a report from the per-position primitive-name occurrence
    /// sets found during a scan.
    #[must_use]
    pub fn new(
        occurrences: BTreeMap<PrimitiveOccurrencePosition, BTreeSet<PrimitiveName>>,
    ) -> Self {
        Self { occurrences }
    }

    /// Returns the full per-position map of requested primitive names found
    /// during the scan, keyed by the [`PrimitiveOccurrencePosition`] each was
    /// found at.
    #[must_use]
    pub fn by_position(&self) -> &BTreeMap<PrimitiveOccurrencePosition, BTreeSet<PrimitiveName>> {
        &self.occurrences
    }
}

// ---------------------------------------------------------------------------
// PrimitiveOccurrenceScanError — scan failure
// ---------------------------------------------------------------------------

/// Failure produced by a [`PrimitiveOccurrenceScanner`] adapter (ADR
/// `2026-07-01-0004` D3).
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum PrimitiveOccurrenceScanError {
    /// The catalogue `TypeRef` string could not be parsed by the adapter's
    /// underlying parser (e.g. `syn`). Carries the offending `TypeRef` -- a
    /// domain value object, not a raw `String` -- so this error type does not
    /// itself introduce a primitive-obsession occurrence for the very rule it
    /// exists to support.
    #[error("failed to parse type_ref '{type_ref}' during primitive-occurrence scan")]
    ParseFailure {
        /// The `TypeRef` that failed to parse.
        type_ref: TypeRef,
    },

    /// `scan` was called with `position` set to
    /// [`PrimitiveOccurrencePosition::ResultErr`], which is not a valid
    /// catalogue-structural call site -- `ResultErr` is exclusively
    /// scan-intrinsic (see [`PrimitiveOccurrencePosition`]'s docs).
    #[error(
        "invalid scan site position {position:?}: ResultErr is scan-intrinsic \
         and cannot be passed as a call-site position"
    )]
    InvalidSitePosition {
        /// The invalid `position` argument the caller supplied.
        position: PrimitiveOccurrencePosition,
    },
}

// ---------------------------------------------------------------------------
// PrimitiveOccurrenceScanner — secondary port
// ---------------------------------------------------------------------------

/// Secondary port judging whether a single [`TypeRef`]'s parsed type-tree
/// contains any of a requested set of forbidden primitive names, reporting
/// each found occurrence's [`PrimitiveOccurrencePosition`] (ADR
/// `2026-07-01-0004` D3).
///
/// Domain owns this port's semantics only; the syn-based implementation is
/// confined to an infrastructure adapter -- domain itself has no dependency
/// on `syn`.
pub trait PrimitiveOccurrenceScanner: Send + Sync {
    /// Scans a single `type_ref`'s parsed type-tree for occurrences of any
    /// primitive name in `primitives`, returning a report keyed by
    /// [`PrimitiveOccurrencePosition`].
    ///
    /// `position` is the caller-known catalogue-structural site (`NamedField`
    /// / `VariantField` / `Param` / `Return` / `Bound` / `TypeAliasTarget`)
    /// this `type_ref` was extracted from, and labels top-level occurrences
    /// not otherwise classified by a structurally-nested position; passing
    /// [`PrimitiveOccurrencePosition::ResultErr`] as `position` is invalid.
    ///
    /// Occurrences discovered by recursing into a nested callable-type
    /// signature's own parameter / return slots or a nested generic-bound
    /// clause are labelled `Param` / `Return` / `Bound` respectively
    /// regardless of `position`, and occurrences in the second generic
    /// argument of a `Result` path segment are labelled `ResultErr`
    /// regardless of `position` or nesting depth.
    ///
    /// # Errors
    ///
    /// Returns [`PrimitiveOccurrenceScanError::ParseFailure`] if `type_ref`
    /// cannot be parsed.
    ///
    /// Returns [`PrimitiveOccurrenceScanError::InvalidSitePosition`] if
    /// `position` is [`PrimitiveOccurrencePosition::ResultErr`].
    fn scan(
        &self,
        type_ref: TypeRef,
        primitives: NonEmptyVec<PrimitiveName>,
        position: PrimitiveOccurrencePosition,
    ) -> Result<PrimitiveOccurrenceReport, PrimitiveOccurrenceScanError>;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_primitive_name_new_accepts_valid_identifier() {
        let name = PrimitiveName::new("String").unwrap();
        assert_eq!(name.as_str(), "String");
    }

    #[test]
    fn test_primitive_name_new_rejects_empty() {
        let result = PrimitiveName::new("");
        assert!(matches!(result, Err(IdentifierError::Empty)));
    }

    #[test]
    fn test_primitive_name_new_rejects_invalid_characters() {
        let result = PrimitiveName::new("Vec<String>");
        assert!(matches!(result, Err(IdentifierError::InvalidCharacters(_))));
    }

    #[test]
    fn test_primitive_occurrence_report_by_position_returns_constructed_map() {
        let mut occurrences = BTreeMap::new();
        let mut names = BTreeSet::new();
        names.insert(PrimitiveName::new("String").unwrap());
        occurrences.insert(PrimitiveOccurrencePosition::NamedField, names);
        let report = PrimitiveOccurrenceReport::new(occurrences.clone());
        assert_eq!(report.by_position(), &occurrences);
    }

    #[test]
    fn test_primitive_occurrence_report_by_position_empty_for_unscanned_position() {
        let report = PrimitiveOccurrenceReport::new(BTreeMap::new());
        assert!(report.by_position().get(&PrimitiveOccurrencePosition::Return).is_none());
    }

    #[test]
    fn test_primitive_occurrence_scan_error_parse_failure_display() {
        let err = PrimitiveOccurrenceScanError::ParseFailure {
            type_ref: TypeRef::new("Vec<!!!>").unwrap(),
        };
        assert!(err.to_string().contains("Vec<!!!>"));
    }

    #[test]
    fn test_primitive_occurrence_scan_error_invalid_site_position_display() {
        let err = PrimitiveOccurrenceScanError::InvalidSitePosition {
            position: PrimitiveOccurrencePosition::ResultErr,
        };
        assert!(err.to_string().contains("ResultErr"));
    }

    #[test]
    fn test_primitive_occurrence_position_ord_is_stable_for_btreemap_keys() {
        // PrimitiveOccurrenceReport stores positions as BTreeMap keys; Ord must
        // be well-defined (derived) so insertion order does not matter.
        let mut occurrences = BTreeMap::new();
        occurrences.insert(PrimitiveOccurrencePosition::ResultErr, BTreeSet::new());
        occurrences.insert(PrimitiveOccurrencePosition::NamedField, BTreeSet::new());
        let report = PrimitiveOccurrenceReport::new(occurrences);
        let keys: Vec<_> = report.by_position().keys().collect();
        assert_eq!(keys.len(), 2);
    }
}
