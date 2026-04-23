//! Catalogue ↔ spec integrity finding types for SoT Chain ②.
//!
//! `SpecRefFinding` is the structured diagnostic record emitted by
//! `check_catalogue_spec_ref_integrity` (authored in a later task). It carries
//! the layer, and a [`SpecRefFindingKind`] discriminant with per-variant payload.
//!
//! Entry-locator fields (`catalogue_entry`, `ref_index`, `spec_file`) are embedded
//! inside the per-entry variants (`DanglingAnchor`, `HashMismatch`) of
//! [`SpecRefFindingKind`] so that the impossible state of a `StaleSignals`
//! (layer-level) finding carrying per-entry metadata is structurally excluded.
//!
//! I/O-free: this module only defines plain data carriers. The pure evaluation
//! functions and CLI formatting live in later tasks of the
//! `catalogue-spec-signal-activation-2026-04-23` track (ADR
//! `2026-04-23-0344-catalogue-spec-signal-activation.md` §D1.5 / §D3.7).

use std::path::PathBuf;

use crate::ConfidenceSignal;
use crate::plan_ref::{ContentHash, InformalGroundRef, SpecElementId, SpecRef};
use crate::tddd::layer_id::LayerId;

/// Schema version of `<layer>-catalogue-spec-signals.json` — pinned to `1`
/// per ADR `2026-04-23-0344-catalogue-spec-signal-activation.md` §D2.2.
/// Bump requires an accompanying codec migration and a schema-version gate
/// (see the parent ADR §D1.4 for the standard workflow).
pub const CATALOGUE_SPEC_SIGNALS_SCHEMA_VERSION: u32 = 1;

/// Discriminant for the three catalogue-spec integrity violations recognised
/// by ADR 2026-04-23-0344 §D1.5.
///
/// Entry-locator fields are embedded in the per-entry variants so that
/// `StaleSignals` (a layer-level finding) structurally cannot carry
/// per-entry metadata, and `DanglingAnchor` / `HashMismatch` cannot
/// be constructed without a valid entry locator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecRefFindingKind {
    /// A `SpecRef.anchor` refers to an element that is absent from the
    /// target `spec.json`. Carries the failing anchor plus the entry
    /// locator so the CLI layer can report it verbatim.
    DanglingAnchor {
        /// Name of the catalogue entry that owns this spec-ref.
        catalogue_entry: String,
        /// Zero-based index of the failing `SpecRef` within the entry's
        /// `spec_refs` list.
        ref_index: usize,
        /// Path to the `spec.json` file that was checked.
        spec_file: PathBuf,
        /// The anchor that was not found in the spec.
        anchor: SpecElementId,
    },

    /// A `SpecRef.hash` differs from the canonical SHA-256 digest of the
    /// spec element subtree. Carries the anchor plus the declared and
    /// actually-observed hashes for diff-style output, along with the
    /// entry locator.
    HashMismatch {
        /// Name of the catalogue entry that owns this spec-ref.
        catalogue_entry: String,
        /// Zero-based index of the failing `SpecRef` within the entry's
        /// `spec_refs` list.
        ref_index: usize,
        /// Path to the `spec.json` file that was checked.
        spec_file: PathBuf,
        /// The anchor whose hash was checked.
        anchor: SpecElementId,
        /// The hash value stored in the `SpecRef` declaration.
        declared: ContentHash,
        /// The hash value computed from the current spec element.
        actual: ContentHash,
    },

    /// The `catalogue_declaration_hash` stored in a
    /// `<layer>-catalogue-spec-signals.json` document no longer matches the
    /// SHA-256 of the current `<layer>-types.json` file. Reported at layer
    /// granularity — no anchor or catalogue entry is attached. Carries both
    /// the declared and actual hashes.
    StaleSignals { declared_catalogue_hash: ContentHash, actual_catalogue_hash: ContentHash },
}

/// Structured finding produced by the `check_catalogue_spec_ref_integrity`
/// pure function.
///
/// The CLI layer aggregates a `Vec<SpecRefFinding>` and formats human-readable
/// stderr output. Entry-locator data (`catalogue_entry`, `ref_index`,
/// `spec_file`) is embedded inside the per-entry variants of
/// [`SpecRefFindingKind`] rather than as top-level `Option` fields, so the
/// type structurally prevents the impossible combination of a `StaleSignals`
/// finding (layer-level) carrying per-entry metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecRefFinding {
    pub layer: LayerId,
    pub kind: SpecRefFindingKind,
}

impl SpecRefFinding {
    /// Construct a finding with a layer and a kind variant.
    ///
    /// # Errors
    /// This constructor is infallible; it always returns `Self`.
    pub fn new(layer: LayerId, kind: SpecRefFindingKind) -> Self {
        Self { layer, kind }
    }
}

/// Per-entry signal record stored in `<layer>-catalogue-spec-signals.json`.
///
/// Each record pairs a catalogue entry name with the
/// [`ConfidenceSignal`] computed by the informal-priority rule (ADR
/// `2026-04-23-0344-catalogue-spec-signal-activation.md` §D1.1). A catalogue
/// entry with multiple `spec_refs[]` collapses to a single signal — the
/// per-ref hash validation is reported separately as a [`SpecRefFinding`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogueSpecSignal {
    /// Name of the catalogue entry (matches `TypeCatalogueEntry.name`).
    pub type_name: String,
    /// Per-entry signal computed by the informal-priority rule.
    pub signal: ConfidenceSignal,
}

impl CatalogueSpecSignal {
    /// Construct a new per-entry signal record.
    pub fn new(type_name: impl Into<String>, signal: ConfidenceSignal) -> Self {
        Self { type_name: type_name.into(), signal }
    }
}

/// Aggregate root for `<layer>-catalogue-spec-signals.json`.
///
/// Holds:
///
/// * `schema_version` — pinned to [`CATALOGUE_SPEC_SIGNALS_SCHEMA_VERSION`]
///   (= 1); see ADR §D2.2 for the bump protocol.
/// * `catalogue_declaration_hash` — SHA-256 of the input `<layer>-types.json`
///   canonical bytes. Used by `sotp verify catalogue-spec-refs` (T009) for
///   stale detection (§D2.2 / §D2.3). Declared as [`ContentHash`] so the
///   hash representation cannot drift from other SoT Chain hashes.
/// * `signals` — one [`CatalogueSpecSignal`] per catalogue entry, in
///   catalogue-declared order.
///
/// **Deterministic output**: no `generated_at` timestamp or other
/// wall-clock-derived field. With identical input `<layer>-types.json`, the
/// document round-trips byte-identical — a prerequisite for CN-06 / §D2.2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogueSpecSignalsDocument {
    /// Schema version; always [`CATALOGUE_SPEC_SIGNALS_SCHEMA_VERSION`] (= 1).
    /// Private to enforce the pinning invariant — use [`Self::schema_version`]
    /// to read and [`Self::new`] to construct.
    schema_version: u32,
    /// SHA-256 of the canonical `<layer>-types.json` bytes used to compute
    /// the signals.
    pub catalogue_declaration_hash: ContentHash,
    /// Per-entry signal records, in catalogue-declared order.
    pub signals: Vec<CatalogueSpecSignal>,
}

impl CatalogueSpecSignalsDocument {
    /// Construct a document with `schema_version` pre-filled to
    /// [`CATALOGUE_SPEC_SIGNALS_SCHEMA_VERSION`].
    pub fn new(catalogue_declaration_hash: ContentHash, signals: Vec<CatalogueSpecSignal>) -> Self {
        Self {
            schema_version: CATALOGUE_SPEC_SIGNALS_SCHEMA_VERSION,
            catalogue_declaration_hash,
            signals,
        }
    }

    /// Returns the schema version (always [`CATALOGUE_SPEC_SIGNALS_SCHEMA_VERSION`]).
    pub fn schema_version(&self) -> u32 {
        self.schema_version
    }
}

/// Evaluates the catalogue-spec confidence signal for a single catalogue entry.
///
/// Implements the **informal-priority rule** (ADR
/// `2026-04-23-0344-catalogue-spec-signal-activation.md` §D1.1). The rule is
/// the per-layer analogue of Phase 1's
/// [`evaluate_requirement_signal`](crate::evaluate_requirement_signal):
///
/// - `informal_grounds[]` non-empty → 🟡 Yellow (unpersisted ground; takes
///   priority regardless of `spec_refs[]` because any remaining informal ground
///   still requires promotion to a formal `SpecRef` before merge)
/// - `informal_grounds[]` empty + `spec_refs[]` non-empty → 🔵 Blue (formal
///   spec grounding with no pending promotion)
/// - both empty → 🔴 Red
///
/// Per-`SpecRef` integrity (dangling `anchor`, hash drift, stale signals) is
/// outside this signal's scope and is reported via [`SpecRefFinding`] by the
/// binary gate (`check_catalogue_spec_ref_integrity`, authored in T004). This
/// function is pure and I/O-free.
#[must_use]
pub fn evaluate_catalogue_entry_signal(
    spec_refs: &[SpecRef],
    informal_grounds: &[InformalGroundRef],
) -> ConfidenceSignal {
    if !informal_grounds.is_empty() {
        return ConfidenceSignal::Yellow;
    }
    if !spec_refs.is_empty() {
        return ConfidenceSignal::Blue;
    }
    ConfidenceSignal::Red
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn layer() -> LayerId {
        LayerId::try_new("domain").unwrap()
    }

    fn anchor(id: &str) -> SpecElementId {
        SpecElementId::try_new(id).unwrap()
    }

    fn hash(byte: u8) -> ContentHash {
        ContentHash::from_bytes([byte; 32])
    }

    #[test]
    fn dangling_anchor_variant_constructs_with_anchor_and_locator() {
        let kind = SpecRefFindingKind::DanglingAnchor {
            catalogue_entry: "Foo".to_string(),
            ref_index: 0,
            spec_file: PathBuf::from("track/items/x/spec.json"),
            anchor: anchor("IN-99"),
        };
        let finding = SpecRefFinding::new(layer(), kind.clone());

        assert_eq!(finding.layer, layer());
        assert_eq!(finding.kind, kind);
        match &finding.kind {
            SpecRefFindingKind::DanglingAnchor {
                catalogue_entry,
                ref_index,
                spec_file,
                anchor: got_anchor,
            } => {
                assert_eq!(catalogue_entry, "Foo");
                assert_eq!(*ref_index, 0);
                assert_eq!(spec_file, &PathBuf::from("track/items/x/spec.json"));
                assert_eq!(got_anchor, &anchor("IN-99"));
            }
            other => panic!("expected DanglingAnchor, got {other:?}"),
        }
    }

    #[test]
    fn hash_mismatch_variant_carries_declared_and_actual_with_locator() {
        let kind = SpecRefFindingKind::HashMismatch {
            catalogue_entry: "Bar".to_string(),
            ref_index: 2,
            spec_file: PathBuf::from("track/items/y/spec.json"),
            anchor: anchor("IN-01"),
            declared: hash(0xaa),
            actual: hash(0xbb),
        };
        let finding = SpecRefFinding::new(layer(), kind.clone());

        match &finding.kind {
            SpecRefFindingKind::HashMismatch { anchor: got, declared, actual, .. } => {
                assert_eq!(got, &anchor("IN-01"));
                assert_ne!(declared, actual);
            }
            other => panic!("expected HashMismatch, got {other:?}"),
        }
        assert_eq!(finding.kind, kind);
    }

    #[test]
    fn stale_signals_variant_is_layer_level_without_entry_locator() {
        let kind = SpecRefFindingKind::StaleSignals {
            declared_catalogue_hash: hash(0x11),
            actual_catalogue_hash: hash(0x22),
        };
        let finding = SpecRefFinding::new(layer(), kind.clone());

        // StaleSignals carries no entry locator — structurally enforced by the enum.
        assert_eq!(finding.kind, kind);
        match &finding.kind {
            SpecRefFindingKind::StaleSignals { declared_catalogue_hash, actual_catalogue_hash } => {
                assert_ne!(declared_catalogue_hash, actual_catalogue_hash);
            }
            other => panic!("expected StaleSignals, got {other:?}"),
        }
    }

    #[test]
    fn findings_are_equal_when_all_fields_match() {
        let kind = SpecRefFindingKind::DanglingAnchor {
            catalogue_entry: "E".to_string(),
            ref_index: 1,
            spec_file: PathBuf::from("spec.json"),
            anchor: anchor("AC-03"),
        };
        let a = SpecRefFinding::new(layer(), kind.clone());
        let b = SpecRefFinding::new(layer(), kind);
        assert_eq!(a, b);
    }

    #[test]
    fn findings_differ_when_kind_differs() {
        let a = SpecRefFinding::new(
            layer(),
            SpecRefFindingKind::DanglingAnchor {
                catalogue_entry: "X".to_string(),
                ref_index: 0,
                spec_file: PathBuf::from("spec.json"),
                anchor: anchor("IN-01"),
            },
        );
        let b = SpecRefFinding::new(
            layer(),
            SpecRefFindingKind::DanglingAnchor {
                catalogue_entry: "X".to_string(),
                ref_index: 0,
                spec_file: PathBuf::from("spec.json"),
                anchor: anchor("IN-02"),
            },
        );
        assert_ne!(a, b);
    }

    #[test]
    fn catalogue_spec_signal_stores_name_and_signal() {
        let s = CatalogueSpecSignal::new("SpecRefFinding", ConfidenceSignal::Blue);
        assert_eq!(s.type_name, "SpecRefFinding");
        assert_eq!(s.signal, ConfidenceSignal::Blue);
    }

    #[test]
    fn catalogue_spec_signal_equality_compares_all_fields() {
        let a = CatalogueSpecSignal::new("Foo", ConfidenceSignal::Yellow);
        let b = CatalogueSpecSignal::new("Foo", ConfidenceSignal::Yellow);
        let c = CatalogueSpecSignal::new("Foo", ConfidenceSignal::Red);
        let d = CatalogueSpecSignal::new("Bar", ConfidenceSignal::Yellow);
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_ne!(a, d);
    }

    #[test]
    fn document_new_pins_schema_version_to_one() {
        let doc = CatalogueSpecSignalsDocument::new(
            hash(0xcd),
            vec![CatalogueSpecSignal::new("T", ConfidenceSignal::Blue)],
        );
        assert_eq!(doc.schema_version(), CATALOGUE_SPEC_SIGNALS_SCHEMA_VERSION);
        assert_eq!(doc.schema_version(), 1);
    }

    #[test]
    fn document_preserves_catalogue_declaration_hash() {
        let h = hash(0xa1);
        let doc = CatalogueSpecSignalsDocument::new(h.clone(), vec![]);
        assert_eq!(doc.catalogue_declaration_hash, h);
    }

    #[test]
    fn document_preserves_signals_in_order() {
        let signals = vec![
            CatalogueSpecSignal::new("First", ConfidenceSignal::Blue),
            CatalogueSpecSignal::new("Second", ConfidenceSignal::Yellow),
            CatalogueSpecSignal::new("Third", ConfidenceSignal::Red),
        ];
        let doc = CatalogueSpecSignalsDocument::new(hash(0x00), signals.clone());
        assert_eq!(doc.signals, signals);
        assert_eq!(doc.signals.len(), 3);
    }

    #[test]
    fn document_equality_detects_any_field_drift() {
        let signals = vec![CatalogueSpecSignal::new("T", ConfidenceSignal::Blue)];
        let a = CatalogueSpecSignalsDocument::new(hash(0x01), signals.clone());
        let b = CatalogueSpecSignalsDocument::new(hash(0x01), signals.clone());
        let c = CatalogueSpecSignalsDocument::new(hash(0x02), signals.clone());
        let d = CatalogueSpecSignalsDocument::new(hash(0x01), vec![]);
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_ne!(a, d);
    }

    #[test]
    fn document_round_trips_byte_identical_via_clone() {
        // Determinism precondition (CN-06 / §D2.2): with identical input the
        // document value is the same. Encoding round-trip lives in the
        // infrastructure codec test once that task lands.
        let doc = CatalogueSpecSignalsDocument::new(
            hash(0x77),
            vec![
                CatalogueSpecSignal::new("Alpha", ConfidenceSignal::Blue),
                CatalogueSpecSignal::new("Beta", ConfidenceSignal::Yellow),
            ],
        );
        let clone = doc.clone();
        assert_eq!(doc, clone);
    }

    // ---------------------------------------------------------------------------
    // evaluate_catalogue_entry_signal (T003, IN-01)
    // ---------------------------------------------------------------------------

    use crate::plan_ref::{InformalGroundKind, InformalGroundRef, InformalGroundSummary, SpecRef};

    fn informal(kind: InformalGroundKind, summary: &str) -> InformalGroundRef {
        InformalGroundRef::new(kind, InformalGroundSummary::try_new(summary).unwrap())
    }

    fn spec_ref(anchor_id: &str) -> SpecRef {
        SpecRef::new("track/items/x/spec.json", anchor(anchor_id), hash(0x00))
    }

    #[test]
    fn evaluate_catalogue_entry_signal_returns_red_when_both_refs_empty() {
        let signal = evaluate_catalogue_entry_signal(&[], &[]);
        assert_eq!(signal, ConfidenceSignal::Red);
    }

    #[test]
    fn evaluate_catalogue_entry_signal_returns_blue_when_only_spec_refs_present() {
        let refs = vec![spec_ref("IN-01")];
        let signal = evaluate_catalogue_entry_signal(&refs, &[]);
        assert_eq!(signal, ConfidenceSignal::Blue);
    }

    #[test]
    fn evaluate_catalogue_entry_signal_returns_yellow_when_informal_grounds_present() {
        let grounds = vec![informal(InformalGroundKind::UserDirective, "pending promotion")];
        let signal = evaluate_catalogue_entry_signal(&[], &grounds);
        assert_eq!(signal, ConfidenceSignal::Yellow);
    }

    #[test]
    fn evaluate_catalogue_entry_signal_yellow_takes_priority_over_spec_refs() {
        let refs = vec![spec_ref("IN-01"), spec_ref("IN-02")];
        let grounds = vec![informal(InformalGroundKind::Discussion, "still iterating")];
        let signal = evaluate_catalogue_entry_signal(&refs, &grounds);
        // Informal-priority rule: any informal ground → Yellow regardless of
        // spec_refs count. Promotion to Blue requires clearing informal_grounds.
        assert_eq!(signal, ConfidenceSignal::Yellow);
    }
}
