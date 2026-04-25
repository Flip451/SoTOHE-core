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

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::ConfidenceSignal;
use crate::plan_ref::{ContentHash, InformalGroundRef, SpecElementId, SpecRef};
use crate::tddd::catalogue::TypeCatalogueDocument;
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

/// Check catalogue-spec reference integrity for a single layer's catalogue.
///
/// Implements the binary gate defined in ADR
/// `2026-04-23-0344-catalogue-spec-signal-activation.md` §D1.5 / §D3.6. The
/// function emits one [`SpecRefFinding`] per violation detected across the
/// three categories:
///
/// 1. **DanglingAnchor** — a [`SpecRef::anchor`] that does not exist in the
///    spec element universe.
/// 2. **HashMismatch** — a [`SpecRef::hash`] that differs from the canonical
///    SHA-256 of the anchor's spec element subtree.
/// 3. **StaleSignals** — the `catalogue_declaration_hash` stored in
///    `<layer>-catalogue-spec-signals.json` no longer matches the hash of the
///    current `<layer>-types.json` (reported once at layer level when both
///    `current_catalogue_hash` and `signals_opt` are provided).
///
/// # Parameters
///
/// * `layer` — the layer whose catalogue is being checked; used to tag every
///   [`SpecRefFinding`] so the CLI can group findings per layer.
/// * `catalogue` — the [`TypeCatalogueDocument`] to validate.
/// * `spec_element_hashes` — pre-computed canonical SHA-256 per
///   [`SpecElementId`]. The map's key set also serves as the anchor universe
///   for the dangling check. Absence of a key ⇒ [`SpecRefFindingKind::DanglingAnchor`].
///   The usecase layer builds this map from the spec.json file bytes using
///   the infrastructure's canonical-JSON SHA-256 helper so the hash format
///   matches the existing `sotp verify plan-artifact-refs` pipeline.
/// * `current_catalogue_hash` — SHA-256 of the current `<layer>-types.json`
///   bytes. Required for the stale check; pass `None` to skip staleness
///   (e.g. `--skip-stale=true` in `sotp verify catalogue-spec-refs`).
/// * `signals_opt` — the persisted signals document (if present). Required
///   for the stale check; pass `None` when the signals file has not been
///   generated yet.
///
/// # Returns
///
/// `Vec<SpecRefFinding>` containing every violation encountered. An empty
/// vector denotes full integrity. Findings are emitted in catalogue-entry
/// declaration order, then by `ref_index` within each entry, with the
/// optional `StaleSignals` finding appended last.
///
/// # Determinism
///
/// Pure function. Deterministic across runs with identical inputs. No I/O,
/// no panics, no unwrap outside `#[cfg(test)]`.
#[must_use]
pub fn check_catalogue_spec_ref_integrity(
    layer: &LayerId,
    catalogue: &TypeCatalogueDocument,
    spec_element_hashes: &BTreeMap<SpecElementId, ContentHash>,
    current_catalogue_hash: Option<&ContentHash>,
    signals_opt: Option<&CatalogueSpecSignalsDocument>,
) -> Vec<SpecRefFinding> {
    let mut findings = Vec::new();

    for entry in catalogue.entries() {
        for (ref_index, spec_ref) in entry.spec_refs().iter().enumerate() {
            match spec_element_hashes.get(&spec_ref.anchor) {
                None => {
                    findings.push(SpecRefFinding::new(
                        layer.clone(),
                        SpecRefFindingKind::DanglingAnchor {
                            catalogue_entry: entry.name().to_owned(),
                            ref_index,
                            spec_file: spec_ref.file.clone(),
                            anchor: spec_ref.anchor.clone(),
                        },
                    ));
                }
                Some(actual_hash) if actual_hash != &spec_ref.hash => {
                    findings.push(SpecRefFinding::new(
                        layer.clone(),
                        SpecRefFindingKind::HashMismatch {
                            catalogue_entry: entry.name().to_owned(),
                            ref_index,
                            spec_file: spec_ref.file.clone(),
                            anchor: spec_ref.anchor.clone(),
                            declared: spec_ref.hash.clone(),
                            actual: actual_hash.clone(),
                        },
                    ));
                }
                Some(_) => {
                    // anchor present, hash match — no finding.
                }
            }
        }
    }

    if let (Some(current), Some(signals)) = (current_catalogue_hash, signals_opt) {
        if &signals.catalogue_declaration_hash != current {
            findings.push(SpecRefFinding::new(
                layer.clone(),
                SpecRefFindingKind::StaleSignals {
                    declared_catalogue_hash: signals.catalogue_declaration_hash.clone(),
                    actual_catalogue_hash: current.clone(),
                },
            ));
        }
    }

    findings
}

/// Evaluates catalogue-spec signal gate rules against a
/// [`CatalogueSpecSignalsDocument`].
///
/// Symmetric with `check_spec_doc_signals` (Phase 1, in
/// `libs/domain/src/spec.rs`) and `check_type_signals` (SoT Chain ③, in
/// `libs/domain/src/tddd/consistency.rs`): shared 3-signal gate shape so
/// the merge-gate assembly can treat every SoT Chain layer uniformly.
///
/// # Rules
///
/// - **Coverage mismatch** (signals length ≠ catalogue entries length, or
///   positional `type_name` mismatch at any index) →
///   [`crate::verify::VerifyFinding::error`]. This catches the fail-open path where a
///   tampered `<layer>-catalogue-spec-signals.json` keeps a valid
///   `catalogue_declaration_hash` but omits or renames entries so Yellow /
///   Red signals are silently dropped.
/// - any [`ConfidenceSignal::Red`] entry → `VerifyFinding::error`
///   (always blocks, regardless of `strict`)
/// - any [`ConfidenceSignal::Yellow`] entry:
///   - `strict = true` → `VerifyFinding::error` (merge gate blocks)
///   - `strict = false` → `VerifyFinding::warning` (CI interim mode visualises)
/// - all [`ConfidenceSignal::Blue`] (or empty on both sides) → `VerifyOutcome::pass`
///
/// Coverage check runs first so a mis-covered signals file fails the gate
/// even when every listed signal is Blue. Red then takes precedence over
/// Yellow: a document with both Red and Yellow entries reports the Red
/// finding only. This matches the precedent set by the Phase 1 / SoT
/// Chain ③ sibling gates where Red is the terminal state and Yellow is
/// only inspected when Red is absent.
///
/// # Parameters
///
/// * `catalogue` — the authoritative type catalogue document for this
///   layer. Used for coverage validation: `signals.signals[i]` must cover
///   `catalogue.entries()[i]` (positional match on `type_name`).
/// * `signals` — the persisted signals document for one layer
///   (`<layer>-catalogue-spec-signals.json`).
/// * `strict` — `true` for the merge gate (Yellow blocks), `false` for CI
///   interim (Yellow visualises as warning).
/// * `catalogue_file` — human-readable layer identifier used in error
///   messages (e.g. `"domain-types.json"`). Callers pass the source file
///   name so the finding is self-describing without needing external
///   context.
///
/// # Reference
///
/// - ADR `2026-04-23-0344-catalogue-spec-signal-activation.md` §D4 (strict / interim behaviour)
/// - Sibling: `check_spec_doc_signals` in `libs/domain/src/spec.rs` (Phase 1 spec signal gate)
/// - Sibling: `check_type_signals` in `libs/domain/src/tddd/consistency.rs` (SoT Chain ③ type signal gate)
#[must_use]
pub fn check_catalogue_spec_signals(
    catalogue: &super::catalogue::TypeCatalogueDocument,
    signals: &CatalogueSpecSignalsDocument,
    strict: bool,
    catalogue_file: &str,
) -> crate::verify::VerifyOutcome {
    use crate::verify::{VerifyFinding, VerifyOutcome};

    // Coverage validation (fail-closed): a tampered signals file with a
    // matching `catalogue_declaration_hash` could still omit entries and
    // silently drop Yellow / Red signals. Enforce length equality + positional
    // `type_name` match so the gate cannot be bypassed by trimming the
    // signals array.
    let catalogue_entries = catalogue.entries();
    if catalogue_entries.len() != signals.signals.len() {
        return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
            "{catalogue_file}: catalogue-spec signals coverage mismatch — catalogue has {} \
             entry/entries, signals document has {} signal(s). Regenerate the signals file \
             with `sotp track catalogue-spec-signals` so every catalogue entry is covered.",
            catalogue_entries.len(),
            signals.signals.len()
        ))]);
    }
    if let Some((i, entry, sig)) = catalogue_entries
        .iter()
        .zip(signals.signals.iter())
        .enumerate()
        .find(|(_, (entry, sig))| entry.name() != sig.type_name)
        .map(|(i, (entry, sig))| (i, entry, sig))
    {
        return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
            "{catalogue_file}: catalogue-spec signals positional mismatch at index {i} \
             (catalogue entry '{}' vs signal '{}'). Regenerate the signals file.",
            entry.name(),
            sig.type_name
        ))]);
    }

    // Empty on both sides: nothing to gate on (layer with no catalogue
    // entries). Coverage check above already rejected the asymmetric cases.
    if signals.signals.is_empty() {
        return VerifyOutcome::pass();
    }

    let reds: Vec<&str> = signals
        .signals
        .iter()
        .filter(|s| s.signal == ConfidenceSignal::Red)
        .map(|s| s.type_name.as_str())
        .collect();
    if !reds.is_empty() {
        return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
            "{catalogue_file}: {} catalogue entry/entries have Red catalogue-spec signal (missing both spec_refs[] and informal_grounds[] — every entry must carry at least one grounding ref): {}",
            reds.len(),
            reds.join(", ")
        ))]);
    }

    let yellows: Vec<&str> = signals
        .signals
        .iter()
        .filter(|s| s.signal == ConfidenceSignal::Yellow)
        .map(|s| s.type_name.as_str())
        .collect();
    if !yellows.is_empty() {
        let message = format!(
            "{catalogue_file}: {} catalogue entry/entries have Yellow catalogue-spec signal — merge gate will block these until upgraded to Blue. Upgrade by promoting informal_grounds[] to spec_refs[] with anchor + canonical SHA-256 hash: {}",
            yellows.len(),
            yellows.join(", ")
        );
        if strict {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(message)]);
        }
        return VerifyOutcome::from_findings(vec![VerifyFinding::warning(message)]);
    }

    VerifyOutcome::pass()
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

    // ---------------------------------------------------------------------------
    // check_catalogue_spec_ref_integrity (T004, IN-02)
    // ---------------------------------------------------------------------------

    use crate::tddd::catalogue::{
        TypeAction, TypeCatalogueDocument, TypeCatalogueEntry, TypeDefinitionKind,
    };

    /// Build a catalogue entry with explicit spec_refs (no informal_grounds).
    fn entry_with_refs(name: &str, spec_refs: Vec<SpecRef>) -> TypeCatalogueEntry {
        TypeCatalogueEntry::with_refs(
            name,
            "test entry",
            TypeDefinitionKind::ValueObject,
            TypeAction::Add,
            true,
            spec_refs,
            Vec::new(),
        )
        .unwrap()
    }

    /// Build a catalogue document from a list of entries.
    fn catalogue(entries: Vec<TypeCatalogueEntry>) -> TypeCatalogueDocument {
        TypeCatalogueDocument::new(1, entries)
    }

    /// Build a SpecRef with anchor + explicit hash (no file path customisation).
    fn spec_ref_with_hash(anchor_id: &str, hash_byte: u8) -> SpecRef {
        SpecRef::new("track/items/x/spec.json", anchor(anchor_id), hash(hash_byte))
    }

    fn signals_with_hash(catalogue_hash: ContentHash) -> CatalogueSpecSignalsDocument {
        CatalogueSpecSignalsDocument::new(catalogue_hash, Vec::new())
    }

    #[test]
    fn integrity_check_returns_empty_when_all_refs_valid() {
        let mut hashes = BTreeMap::new();
        hashes.insert(anchor("IN-01"), hash(0xaa));
        hashes.insert(anchor("IN-02"), hash(0xbb));

        let cat = catalogue(vec![entry_with_refs(
            "Foo",
            vec![spec_ref_with_hash("IN-01", 0xaa), spec_ref_with_hash("IN-02", 0xbb)],
        )]);

        let findings = check_catalogue_spec_ref_integrity(&layer(), &cat, &hashes, None, None);
        assert!(findings.is_empty());
    }

    #[test]
    fn integrity_check_reports_dangling_anchor_when_anchor_missing() {
        let hashes = BTreeMap::new(); // empty — every anchor is dangling
        let cat = catalogue(vec![entry_with_refs("Foo", vec![spec_ref_with_hash("IN-99", 0x00)])]);

        let findings = check_catalogue_spec_ref_integrity(&layer(), &cat, &hashes, None, None);
        assert_eq!(findings.len(), 1);
        match &findings[0].kind {
            SpecRefFindingKind::DanglingAnchor {
                anchor: a, catalogue_entry, ref_index, ..
            } => {
                assert_eq!(a, &anchor("IN-99"));
                assert_eq!(catalogue_entry, "Foo");
                assert_eq!(*ref_index, 0);
            }
            other => panic!("expected DanglingAnchor, got {other:?}"),
        }
    }

    #[test]
    fn integrity_check_reports_hash_mismatch_when_hashes_differ() {
        let mut hashes = BTreeMap::new();
        hashes.insert(anchor("IN-01"), hash(0xaa));

        let cat = catalogue(vec![entry_with_refs(
            "Foo",
            vec![spec_ref_with_hash("IN-01", 0xbb)], // declared 0xbb, actual 0xaa
        )]);

        let findings = check_catalogue_spec_ref_integrity(&layer(), &cat, &hashes, None, None);
        assert_eq!(findings.len(), 1);
        match &findings[0].kind {
            SpecRefFindingKind::HashMismatch { declared, actual, anchor: a, .. } => {
                assert_eq!(a, &anchor("IN-01"));
                assert_eq!(declared, &hash(0xbb));
                assert_eq!(actual, &hash(0xaa));
            }
            other => panic!("expected HashMismatch, got {other:?}"),
        }
    }

    #[test]
    fn integrity_check_reports_stale_signals_when_catalogue_hashes_differ() {
        let hashes = BTreeMap::new();
        let cat = catalogue(vec![]);
        let current = hash(0x33);
        let signals = signals_with_hash(hash(0x44));

        let findings = check_catalogue_spec_ref_integrity(
            &layer(),
            &cat,
            &hashes,
            Some(&current),
            Some(&signals),
        );
        assert_eq!(findings.len(), 1);
        match &findings[0].kind {
            SpecRefFindingKind::StaleSignals { declared_catalogue_hash, actual_catalogue_hash } => {
                assert_eq!(declared_catalogue_hash, &hash(0x44));
                assert_eq!(actual_catalogue_hash, &hash(0x33));
            }
            other => panic!("expected StaleSignals, got {other:?}"),
        }
    }

    #[test]
    fn integrity_check_skips_stale_when_either_current_or_signals_is_none() {
        let hashes = BTreeMap::new();
        let cat = catalogue(vec![]);
        let current = hash(0x33);
        let signals = signals_with_hash(hash(0x44));

        // current_catalogue_hash = None → skip
        let findings =
            check_catalogue_spec_ref_integrity(&layer(), &cat, &hashes, None, Some(&signals));
        assert!(findings.is_empty());

        // signals_opt = None → skip
        let findings =
            check_catalogue_spec_ref_integrity(&layer(), &cat, &hashes, Some(&current), None);
        assert!(findings.is_empty());
    }

    #[test]
    fn integrity_check_skips_stale_when_catalogue_hashes_match() {
        let hashes = BTreeMap::new();
        let cat = catalogue(vec![]);
        let same_hash = hash(0x55);
        let signals = signals_with_hash(same_hash.clone());

        let findings = check_catalogue_spec_ref_integrity(
            &layer(),
            &cat,
            &hashes,
            Some(&same_hash),
            Some(&signals),
        );
        assert!(findings.is_empty());
    }

    #[test]
    fn integrity_check_preserves_catalogue_entry_and_ref_index_order() {
        let hashes = BTreeMap::new(); // all anchors dangling so we get a finding per ref
        let cat = catalogue(vec![
            entry_with_refs(
                "First",
                vec![spec_ref_with_hash("IN-01", 0x00), spec_ref_with_hash("IN-02", 0x00)],
            ),
            entry_with_refs("Second", vec![spec_ref_with_hash("IN-03", 0x00)]),
        ]);

        let findings = check_catalogue_spec_ref_integrity(&layer(), &cat, &hashes, None, None);
        assert_eq!(findings.len(), 3);

        // Ordering: entries in declaration order, refs in ref_index order.
        let ids: Vec<&str> = findings
            .iter()
            .map(|f| match &f.kind {
                SpecRefFindingKind::DanglingAnchor { anchor, .. } => anchor.as_ref(),
                _ => "",
            })
            .collect();
        assert_eq!(ids, vec!["IN-01", "IN-02", "IN-03"]);

        let entries: Vec<&str> = findings
            .iter()
            .map(|f| match &f.kind {
                SpecRefFindingKind::DanglingAnchor { catalogue_entry, .. } => {
                    catalogue_entry.as_str()
                }
                _ => "",
            })
            .collect();
        assert_eq!(entries, vec!["First", "First", "Second"]);
    }

    #[test]
    fn integrity_check_tags_findings_with_supplied_layer() {
        let hashes = BTreeMap::new();
        let cat = catalogue(vec![entry_with_refs("X", vec![spec_ref_with_hash("IN-01", 0x00)])]);
        let custom_layer = LayerId::try_new("usecase").unwrap();

        let findings = check_catalogue_spec_ref_integrity(&custom_layer, &cat, &hashes, None, None);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].layer, custom_layer);
    }

    // ---------------------------------------------------------------------------
    // check_catalogue_spec_signals (T005, IN-03)
    // ---------------------------------------------------------------------------

    fn signals_doc(entries: Vec<CatalogueSpecSignal>) -> CatalogueSpecSignalsDocument {
        CatalogueSpecSignalsDocument::new(hash(0x00), entries)
    }

    /// Builds a `TypeCatalogueDocument` whose entry names (positional) match
    /// the `signals.signals` `type_name`s. Used by the signal-gate tests so
    /// the coverage check passes and the Red / Yellow logic under test is
    /// the only thing exercised.
    fn catalogue_matching(
        signals: &CatalogueSpecSignalsDocument,
    ) -> super::super::catalogue::TypeCatalogueDocument {
        use super::super::catalogue::{
            TypeCatalogueDocument, TypeCatalogueEntry, TypeDefinitionKind,
        };
        use crate::TypeAction;

        let entries = signals
            .signals
            .iter()
            .map(|s| {
                TypeCatalogueEntry::new(
                    s.type_name.clone(),
                    "generated fixture",
                    TypeDefinitionKind::ValueObject,
                    TypeAction::Add,
                    true,
                )
                .expect("test fixture: signal type_name is non-empty")
            })
            .collect();
        TypeCatalogueDocument::new(1, entries)
    }

    #[test]
    fn check_signals_passes_when_document_is_empty() {
        let doc = signals_doc(vec![]);
        let catalogue = catalogue_matching(&doc);
        for &strict in &[true, false] {
            let outcome =
                check_catalogue_spec_signals(&catalogue, &doc, strict, "domain-types.json");
            assert!(outcome.is_ok());
            assert!(outcome.findings().is_empty());
        }
    }

    #[test]
    fn check_signals_passes_when_all_blue() {
        let doc = signals_doc(vec![
            CatalogueSpecSignal::new("T1", ConfidenceSignal::Blue),
            CatalogueSpecSignal::new("T2", ConfidenceSignal::Blue),
        ]);
        let catalogue = catalogue_matching(&doc);
        for &strict in &[true, false] {
            let outcome =
                check_catalogue_spec_signals(&catalogue, &doc, strict, "domain-types.json");
            assert!(outcome.is_ok());
            assert!(outcome.findings().is_empty());
        }
    }

    #[test]
    fn check_signals_errors_when_red_regardless_of_strict() {
        let doc = signals_doc(vec![
            CatalogueSpecSignal::new("Ok", ConfidenceSignal::Blue),
            CatalogueSpecSignal::new("Bad", ConfidenceSignal::Red),
        ]);
        let catalogue = catalogue_matching(&doc);

        for &strict in &[true, false] {
            let outcome =
                check_catalogue_spec_signals(&catalogue, &doc, strict, "domain-types.json");
            assert_eq!(outcome.findings().len(), 1);
            assert!(outcome.has_errors());
            let msg = outcome.findings()[0].message();
            assert!(msg.contains("Red"));
            assert!(msg.contains("Bad"));
            assert!(msg.contains("domain-types.json"));
        }
    }

    #[test]
    fn check_signals_yellow_strict_true_returns_error() {
        let doc = signals_doc(vec![CatalogueSpecSignal::new("Pending", ConfidenceSignal::Yellow)]);
        let catalogue = catalogue_matching(&doc);
        let outcome = check_catalogue_spec_signals(&catalogue, &doc, true, "domain-types.json");
        assert_eq!(outcome.findings().len(), 1);
        assert!(outcome.has_errors());
        let msg = outcome.findings()[0].message();
        assert!(msg.contains("Yellow"));
        assert!(msg.contains("Pending"));
    }

    #[test]
    fn check_signals_yellow_strict_false_returns_warning() {
        use crate::verify::Severity;

        let doc = signals_doc(vec![CatalogueSpecSignal::new("Pending", ConfidenceSignal::Yellow)]);
        let catalogue = catalogue_matching(&doc);
        let outcome = check_catalogue_spec_signals(&catalogue, &doc, false, "domain-types.json");
        assert_eq!(outcome.findings().len(), 1);
        assert!(outcome.is_ok(), "warning should not set has_errors");
        assert_eq!(outcome.findings()[0].severity(), Severity::Warning);
        let msg = outcome.findings()[0].message();
        assert!(msg.contains("Yellow"));
        assert!(msg.contains("Pending"));
    }

    #[test]
    fn check_signals_red_takes_precedence_over_yellow() {
        let doc = signals_doc(vec![
            CatalogueSpecSignal::new("TY", ConfidenceSignal::Yellow),
            CatalogueSpecSignal::new("TR", ConfidenceSignal::Red),
        ]);
        let catalogue = catalogue_matching(&doc);
        for &strict in &[true, false] {
            let outcome =
                check_catalogue_spec_signals(&catalogue, &doc, strict, "domain-types.json");
            assert_eq!(outcome.findings().len(), 1);
            let msg = outcome.findings()[0].message();
            assert!(msg.contains("Red"));
            assert!(msg.contains("TR"));
            // Yellow entry is NOT mentioned in the Red-only finding.
            assert!(!msg.contains("TY"));
        }
    }

    #[test]
    fn check_signals_message_includes_catalogue_file() {
        let doc = signals_doc(vec![CatalogueSpecSignal::new("T", ConfidenceSignal::Yellow)]);
        let catalogue = catalogue_matching(&doc);
        let outcome = check_catalogue_spec_signals(&catalogue, &doc, true, "usecase-types.json");
        let msg = outcome.findings()[0].message();
        assert!(msg.starts_with("usecase-types.json:"));
    }

    // Coverage validation tests (PR #111 fail-open fix).

    #[test]
    fn check_signals_errors_when_signals_shorter_than_catalogue() {
        // Tampered signals file: catalogue has 2 entries, signals doc omits 1.
        // A valid `catalogue_declaration_hash` would otherwise let this pass —
        // the length-equality check rejects it.
        use super::super::catalogue::{
            TypeCatalogueDocument, TypeCatalogueEntry, TypeDefinitionKind,
        };
        use crate::TypeAction;

        let catalogue = TypeCatalogueDocument::new(
            1,
            vec![
                TypeCatalogueEntry::new(
                    "A",
                    "desc",
                    TypeDefinitionKind::ValueObject,
                    TypeAction::Add,
                    true,
                )
                .unwrap(),
                TypeCatalogueEntry::new(
                    "B",
                    "desc",
                    TypeDefinitionKind::ValueObject,
                    TypeAction::Add,
                    true,
                )
                .unwrap(),
            ],
        );
        let doc = signals_doc(vec![CatalogueSpecSignal::new("A", ConfidenceSignal::Blue)]);

        let outcome = check_catalogue_spec_signals(&catalogue, &doc, true, "domain-types.json");
        assert!(outcome.has_errors(), "coverage mismatch must block");
        let msg = outcome.findings()[0].message();
        assert!(
            msg.contains("coverage mismatch"),
            "expected coverage mismatch message, got: {msg}"
        );
        assert!(msg.contains("2"));
        assert!(msg.contains("1"));
    }

    #[test]
    fn check_signals_errors_when_catalogue_empty_but_signals_present() {
        use super::super::catalogue::TypeCatalogueDocument;

        let catalogue = TypeCatalogueDocument::new(1, vec![]);
        let doc = signals_doc(vec![CatalogueSpecSignal::new("Ghost", ConfidenceSignal::Blue)]);

        let outcome = check_catalogue_spec_signals(&catalogue, &doc, true, "domain-types.json");
        assert!(outcome.has_errors(), "stray signal without catalogue entry must block");
        assert!(outcome.findings()[0].message().contains("coverage mismatch"));
    }

    #[test]
    fn check_signals_errors_when_type_name_mismatched_positionally() {
        // Same length but positional name mismatch: signals[0] refers to a
        // name that is not catalogue.entries[0].name. This defeats a subtler
        // tampering that keeps the length correct but swaps in a Blue row for
        // an entry that should be Yellow / Red.
        use super::super::catalogue::{
            TypeCatalogueDocument, TypeCatalogueEntry, TypeDefinitionKind,
        };
        use crate::TypeAction;

        let catalogue = TypeCatalogueDocument::new(
            1,
            vec![
                TypeCatalogueEntry::new(
                    "Real",
                    "desc",
                    TypeDefinitionKind::ValueObject,
                    TypeAction::Add,
                    true,
                )
                .unwrap(),
            ],
        );
        let doc = signals_doc(vec![CatalogueSpecSignal::new("Fake", ConfidenceSignal::Blue)]);

        let outcome = check_catalogue_spec_signals(&catalogue, &doc, true, "domain-types.json");
        assert!(outcome.has_errors(), "positional name mismatch must block");
        let msg = outcome.findings()[0].message();
        assert!(msg.contains("positional mismatch"));
        assert!(msg.contains("Real"));
        assert!(msg.contains("Fake"));
    }
}
