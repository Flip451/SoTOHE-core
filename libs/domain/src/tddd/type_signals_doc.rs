//! Evaluation-result document for the per-layer TDDD type-signal file.
//!
//! Holds the pure in-memory representation of `<layer>-type-signals.json`
//! (schema_version 1). This module is part of the declaration/evaluation split
//! introduced by ADR `knowledge/adr/2026-04-18-1400-tddd-ci-gate-and-signals-separation.md`
//! §D1, which separates authored type declarations (`<layer>-types.json`) from
//! generated evaluation results (`<layer>-type-signals.json`).
//!
//! `TypeSignalsDocument` is a pure value type: it has no state transitions and
//! no variant-dependent data, so it is modelled as a struct with private fields
//! and accessor methods (see `.claude/rules/04-coding-principles.md`).
//!
//! `TypeSignalsLoadResult` captures the outcome of loading a signal document
//! relative to the current declaration file: `Current` / `Stale { .. }` /
//! `Missing`. The three states carry structurally different data (Stale needs
//! the expected hash to report; Missing carries no data), so the type uses the
//! enum-first pattern.

use crate::Timestamp;
use crate::tddd::catalogue::TypeSignal;

/// Fixed schema version for `<layer>-type-signals.json`.
///
/// The evaluation-result file is a new schema introduced by ADR 2026-04-18-1400.
/// Any future incompatible format change must bump this version.
pub const TYPE_SIGNALS_SCHEMA_VERSION: u32 = 1;

/// In-memory representation of `<layer>-type-signals.json` (schema_version 1).
///
/// Records the output of a single `sotp track type-signals` run for one layer:
/// the per-type confidence signals, the generation timestamp, and a SHA-256
/// fingerprint of the declaration file bytes at evaluation time. The
/// fingerprint enables `verify_from_spec_json` to detect stale evaluation
/// results (declaration file changed after the signals were recorded).
///
/// Pure value type — no state transitions, no variant-dependent data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeSignalsDocument {
    schema_version: u32,
    generated_at: Timestamp,
    declaration_hash: String,
    signals: Vec<TypeSignal>,
}

impl TypeSignalsDocument {
    /// Creates a new `TypeSignalsDocument` with `schema_version = 1`.
    #[must_use]
    pub fn new(
        generated_at: Timestamp,
        declaration_hash: impl Into<String>,
        signals: Vec<TypeSignal>,
    ) -> Self {
        Self {
            schema_version: TYPE_SIGNALS_SCHEMA_VERSION,
            generated_at,
            declaration_hash: declaration_hash.into(),
            signals,
        }
    }

    /// Creates a `TypeSignalsDocument` with an explicit `schema_version`.
    ///
    /// Use this only in the infrastructure codec when decoding: production
    /// code paths should call `new` which pins the version.
    #[must_use]
    pub fn with_schema_version(
        schema_version: u32,
        generated_at: Timestamp,
        declaration_hash: impl Into<String>,
        signals: Vec<TypeSignal>,
    ) -> Self {
        Self { schema_version, generated_at, declaration_hash: declaration_hash.into(), signals }
    }

    /// Returns the schema version recorded in the document.
    #[must_use]
    pub fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns the generation timestamp (ISO 8601 UTC).
    #[must_use]
    pub fn generated_at(&self) -> &Timestamp {
        &self.generated_at
    }

    /// Returns the SHA-256 hex digest of the declaration file bytes at
    /// evaluation time.
    #[must_use]
    pub fn declaration_hash(&self) -> &str {
        &self.declaration_hash
    }

    /// Returns the per-type evaluation signals.
    #[must_use]
    pub fn signals(&self) -> &[TypeSignal] {
        &self.signals
    }
}

/// Outcome of loading a `<layer>-type-signals.json` relative to the current
/// declaration file.
///
/// Enum-first: the three states carry structurally distinct data. `Current`
/// wraps the loaded document, `Stale` additionally records the expected hash
/// so callers can report both the recorded value and the current value, and
/// `Missing` carries no data (signal file absent).
///
/// Per ADR 2026-04-18-1400 §D5, both `Stale` and `Missing` are fail-closed
/// errors on CI and merge gate paths (symmetric between routes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeSignalsLoadResult {
    /// Signal file exists and its `declaration_hash` matches the current
    /// declaration file hash.
    Current(TypeSignalsDocument),
    /// Signal file exists but its `declaration_hash` does not match the
    /// current declaration file hash.
    ///
    /// `doc` is the loaded document (its recorded hash is accessible via
    /// `doc.declaration_hash()`), and `expected_hash` is the hash of the
    /// current declaration file bytes.
    Stale { doc: TypeSignalsDocument, expected_hash: String },
    /// Signal file is absent at the expected path.
    Missing,
}

impl TypeSignalsLoadResult {
    /// Returns the loaded document for the `Current` variant, or `None`
    /// otherwise.
    #[must_use]
    pub fn as_current(&self) -> Option<&TypeSignalsDocument> {
        match self {
            Self::Current(doc) => Some(doc),
            _ => None,
        }
    }

    /// Returns `true` when the variant is `Current`.
    #[must_use]
    pub fn is_current(&self) -> bool {
        matches!(self, Self::Current(_))
    }

    /// Returns `true` when the variant is `Stale`.
    #[must_use]
    pub fn is_stale(&self) -> bool {
        matches!(self, Self::Stale { .. })
    }

    /// Returns `true` when the variant is `Missing`.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        matches!(self, Self::Missing)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::ConfidenceSignal;

    fn ts(raw: &str) -> Timestamp {
        Timestamp::new(raw).unwrap()
    }

    fn signal_blue(name: &str, kind: &str) -> TypeSignal {
        TypeSignal::new(name, kind, ConfidenceSignal::Blue, true, vec![], vec![], vec![])
    }

    fn sample_doc() -> TypeSignalsDocument {
        TypeSignalsDocument::new(
            ts("2026-04-18T12:00:00Z"),
            "abc123",
            vec![signal_blue("Foo", "value_object")],
        )
    }

    // --- TypeSignalsDocument ---

    #[test]
    fn test_new_pins_schema_version_to_one() {
        let doc = sample_doc();
        assert_eq!(doc.schema_version(), TYPE_SIGNALS_SCHEMA_VERSION);
        assert_eq!(doc.schema_version(), 1);
    }

    #[test]
    fn test_new_stores_generated_at() {
        let doc = sample_doc();
        assert_eq!(doc.generated_at().as_str(), "2026-04-18T12:00:00Z");
    }

    #[test]
    fn test_new_stores_declaration_hash() {
        let doc = sample_doc();
        assert_eq!(doc.declaration_hash(), "abc123");
    }

    #[test]
    fn test_new_accepts_string_declaration_hash() {
        let doc =
            TypeSignalsDocument::new(ts("2026-04-18T12:00:00Z"), String::from("deadbeef"), vec![]);
        assert_eq!(doc.declaration_hash(), "deadbeef");
    }

    #[test]
    fn test_new_preserves_signals_in_order() {
        let doc = TypeSignalsDocument::new(
            ts("2026-04-18T12:00:00Z"),
            "h",
            vec![signal_blue("A", "value_object"), signal_blue("B", "enum")],
        );
        assert_eq!(doc.signals().len(), 2);
        assert_eq!(doc.signals()[0].type_name(), "A");
        assert_eq!(doc.signals()[1].type_name(), "B");
    }

    #[test]
    fn test_with_schema_version_allows_explicit_override_for_codec() {
        let doc =
            TypeSignalsDocument::with_schema_version(42, ts("2026-04-18T12:00:00Z"), "h", vec![]);
        assert_eq!(doc.schema_version(), 42);
    }

    #[test]
    fn test_documents_with_same_fields_are_equal() {
        assert_eq!(sample_doc(), sample_doc());
    }

    #[test]
    fn test_documents_differ_on_hash() {
        let a = TypeSignalsDocument::new(ts("2026-04-18T12:00:00Z"), "a", vec![]);
        let b = TypeSignalsDocument::new(ts("2026-04-18T12:00:00Z"), "b", vec![]);
        assert_ne!(a, b);
    }

    // --- TypeSignalsLoadResult ---

    #[test]
    fn test_current_variant_carries_document() {
        let result = TypeSignalsLoadResult::Current(sample_doc());
        assert!(result.is_current());
        assert!(!result.is_stale());
        assert!(!result.is_missing());
        assert_eq!(result.as_current(), Some(&sample_doc()));
    }

    #[test]
    fn test_stale_variant_carries_doc_and_expected_hash() {
        let result =
            TypeSignalsLoadResult::Stale { doc: sample_doc(), expected_hash: "new_hash".into() };
        assert!(result.is_stale());
        assert!(!result.is_current());
        assert!(!result.is_missing());
        assert!(result.as_current().is_none());
        if let TypeSignalsLoadResult::Stale { doc, expected_hash } = &result {
            assert_eq!(doc.declaration_hash(), "abc123");
            assert_eq!(expected_hash, "new_hash");
        }
    }

    #[test]
    fn test_missing_variant_has_no_data() {
        let result = TypeSignalsLoadResult::Missing;
        assert!(result.is_missing());
        assert!(!result.is_current());
        assert!(!result.is_stale());
        assert!(result.as_current().is_none());
    }

    #[test]
    fn test_equality_respects_variant_data() {
        let a = TypeSignalsLoadResult::Current(sample_doc());
        let b = TypeSignalsLoadResult::Current(sample_doc());
        assert_eq!(a, b);

        let stale_a = TypeSignalsLoadResult::Stale { doc: sample_doc(), expected_hash: "x".into() };
        let stale_b = TypeSignalsLoadResult::Stale { doc: sample_doc(), expected_hash: "y".into() };
        assert_ne!(stale_a, stale_b);

        assert_eq!(TypeSignalsLoadResult::Missing, TypeSignalsLoadResult::Missing);
    }
}
