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

use crate::plan_ref::{ContentHash, SpecElementId};
use crate::tddd::layer_id::LayerId;

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
}
