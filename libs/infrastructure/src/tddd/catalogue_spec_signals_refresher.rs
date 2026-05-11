//! Per-layer catalogue-spec-signal refresh logic.
//!
//! Moved from the CLI layer so that the CLI composition root never imports
//! `domain::CatalogueSpecSignal`, `domain::CatalogueSpecSignalsDocument`,
//! `domain::ConfidenceSignal`, `domain::ContentHash`, or
//! `domain::evaluate_catalogue_entry_signal` directly (CN-01 / AC-03).
//!
//! Supports both v2 (`TypeCatalogueDocument`) and v3 (`CatalogueDocument`)
//! catalogue formats. For both v2 and v3, per-entry signals are computed from
//! `spec_refs[]` and `informal_grounds[]` via `evaluate_catalogue_entry_signal`.
//! The informal-priority rule (ADR `2026-04-23-0344` §D1.1) applies, with the
//! `action: "reference"` exemption from ADR `2026-05-11-1257` D5:
//! - `action == Reference` + both empty → Blue (baseline-implicit grounding)
//! - `informal_grounds` non-empty → Yellow
//! - `informal_grounds` empty + `spec_refs` non-empty → Blue
//! - both empty (non-reference action) → Red
//!
//! ADR reference: `2026-04-23-0344-catalogue-spec-signal-activation.md`
//! §D2 / §D3.1 / IN-09; `2026-05-11-1257-tddd-v2-catalogue-spec-link-restoration.md` D5.

use std::path::Path;

use domain::tddd::catalogue_v2::roles::ItemAction;
use domain::{
    CatalogueSpecSignal, CatalogueSpecSignalsDocument, ConfidenceSignal, ContentHash, TrackId,
    TypeAction, evaluate_catalogue_entry_signal,
};

use crate::tddd::catalogue_codec;
use crate::tddd::catalogue_document_codec::{CatalogueDocumentCodec, CatalogueDocumentCodecError};
use crate::tddd::fs_catalogue_spec_signals_store::FsCatalogueSpecSignalsStore;
use crate::tddd::type_signals_codec;
use crate::track::symlink_guard::reject_symlinks_below;
use crate::verify::tddd_layers::TdddLayerBinding;
use usecase::catalogue_spec_signals::CatalogueSpecSignalsWriter;

/// Refresh a single layer: read `<layer>-types.json` from the local workspace,
/// compute per-entry signals + the raw-bytes SHA-256, build the document, and
/// persist via the writer port.
///
/// `track_id` is accepted as `&str` so the CLI never imports `domain::TrackId`.
/// Internally it is validated and converted via `TrackId::try_new` before the
/// writer is called.
///
/// # Errors
///
/// Returns a human-readable error string when the catalogue cannot be read,
/// decoded, or written, or when the `track_id` fails internal domain validation.
pub fn refresh_one_layer(
    items_dir: &Path,
    track_dir: &Path,
    track_id: &str,
    binding: &TdddLayerBinding,
    writer: &FsCatalogueSpecSignalsStore,
) -> Result<(), String> {
    let layer_id = binding.layer_id();

    // Security: guard items_dir itself against a directly symlinked root.
    // `reject_symlinks_below` only inspects descendants — a symlinked root would
    // bypass it. Check before canonicalize so the metadata check is consistent
    // with the symlink-guard anchor used by `reject_symlinks_below`.
    match items_dir.symlink_metadata() {
        Ok(meta) if meta.file_type().is_symlink() => {
            return Err(format!(
                "symlink guard: refusing to use symlinked items_dir: {}",
                items_dir.display()
            ));
        }
        Ok(_) => {}
        Err(e) => {
            return Err(format!(
                "symlink guard: cannot stat items_dir '{}': {e}",
                items_dir.display()
            ));
        }
    }

    // Validate track_id early so it can be used in the track_dir consistency check.
    let track_id_domain =
        TrackId::try_new(track_id).map_err(|e| format!("invalid track_id '{track_id}': {e}"))?;

    // Security: verify track_dir is contained within items_dir AND exactly equals
    // `items_dir/<track_id>`. The caller passes both independently; accepting a
    // mismatched pair (e.g. items_dir/track-a as track_dir with track_id="track-b")
    // would read the catalogue from one track and write signals into another.
    // Canonicalize both paths to handle `..` traversal, then verify the exact match.
    let canonical_items = items_dir
        .canonicalize()
        .map_err(|e| format!("cannot canonicalize items_dir '{}': {e}", items_dir.display()))?;
    let canonical_track = track_dir.canonicalize().map_err(|e| {
        format!(
            "cannot canonicalize track_dir '{}': {e} — does the directory exist?",
            track_dir.display()
        )
    })?;
    let expected_track = canonical_items.join(track_id_domain.as_ref());
    if canonical_track != expected_track {
        return Err(format!(
            "track_dir '{}' does not match items_dir/track_id (expected '{}'). \
             Mismatched track_dir and track_id would read from one track and write to another.",
            track_dir.display(),
            expected_track.display(),
        ));
    }

    let catalogue_path = track_dir.join(binding.catalogue_file());

    // Symlink guard on the READ path (fail-closed): reject symlinks at the leaf
    // and every ancestor below `items_dir` (not just `track_dir`). Using
    // `items_dir` as the trusted root ensures that a symlinked `track_dir`
    // (i.e., `items_dir/<track_id>`) is also caught. Mirrors
    // `execute_baseline_capture` which anchors at `items_dir` for the same reason
    // (ADR 2026-04-18-1400 §D7).
    reject_symlinks_below(&catalogue_path, items_dir).map_err(|e| {
        format!(
            "refusing to read catalogue '{}' for layer '{layer_id}': {e}",
            catalogue_path.display()
        )
    })?;

    // Read the local catalogue bytes (not the origin blob).
    let bytes = std::fs::read(&catalogue_path).map_err(|e| {
        format!("cannot read catalogue '{}' for layer '{layer_id}': {e}", catalogue_path.display())
    })?;

    let text = std::str::from_utf8(&bytes).map_err(|e| {
        format!("catalogue '{}' contains non-UTF-8 bytes: {e}", catalogue_path.display())
    })?;

    // Derive the filename stem (e.g. "domain" from "domain-types.json") for
    // CatalogueDocumentCodec::decode, which validates crate_name against it.
    let filename_stem = catalogue_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .strip_suffix("-types.json")
        .map(str::to_owned)
        .unwrap_or_else(|| {
            catalogue_path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_owned()
        });

    // Try v3 codec first (CatalogueDocument); fall back to v2 (TypeCatalogueDocument)
    // ONLY when the catalogue is confirmed to be a pre-v3 format (schema_version ≠ 3).
    // Both formats compute per-entry signals from spec_refs[] and informal_grounds[]
    // via evaluate_catalogue_entry_signal (informal-priority rule).
    let signals: Vec<CatalogueSpecSignal> =
        match CatalogueDocumentCodec::decode(text, &filename_stem) {
            Ok(v3_doc) => {
                // v3: enumerate all entries across the three BTreeMaps, compute
                // per-entry signal from spec_refs / informal_grounds.
                let mut sigs: Vec<CatalogueSpecSignal> = Vec::new();
                for (type_name, entry) in &v3_doc.types {
                    let signal = evaluate_catalogue_entry_signal(
                        entry.action,
                        &entry.spec_refs,
                        &entry.informal_grounds,
                    );
                    sigs.push(CatalogueSpecSignal::new(type_name.as_str(), signal));
                }
                for (trait_name, entry) in &v3_doc.traits {
                    let signal = evaluate_catalogue_entry_signal(
                        entry.action,
                        &entry.spec_refs,
                        &entry.informal_grounds,
                    );
                    sigs.push(CatalogueSpecSignal::new(trait_name.as_str(), signal));
                }
                for (fn_path, entry) in &v3_doc.functions {
                    let signal = evaluate_catalogue_entry_signal(
                        entry.action,
                        &entry.spec_refs,
                        &entry.informal_grounds,
                    );
                    sigs.push(CatalogueSpecSignal::new(fn_path.to_string(), signal));
                }
                sigs
            }
            // UnsupportedSchemaVersion means the file is not v3 (e.g. schema_version=2);
            // fall back to the v2 codec to handle tracks authored before the v3 migration.
            Err(CatalogueDocumentCodecError::UnsupportedSchemaVersion { .. }) => {
                let catalogue = catalogue_codec::decode(text).map_err(|e| {
                    format!("cannot decode catalogue '{}': {e}", catalogue_path.display())
                })?;
                catalogue
                    .entries()
                    .iter()
                    .map(|entry| {
                        let action = type_action_to_item_action(entry.action());
                        let signal = evaluate_catalogue_entry_signal(
                            action,
                            entry.spec_refs(),
                            entry.informal_grounds(),
                        );
                        CatalogueSpecSignal::new(entry.name(), signal)
                    })
                    .collect()
            }
            // All other v3 decode errors (InvalidEntry, CrossCrateFunctionPath,
            // CrateNameMismatch, Json) are propagated directly so that the operator
            // sees the actual validation failure rather than a misleading v2 schema
            // mismatch.  Swallowing these errors (via a catch-all `Err(_)`) would
            // obscure the root cause and make malformed v3 catalogues harder to diagnose.
            Err(e) => {
                return Err(format!("cannot decode catalogue '{}': {e}", catalogue_path.display()));
            }
        };

    // Compute raw-bytes SHA-256 (same canonical-hash helper as merge_gate_adapter).
    let catalogue_hash_hex = type_signals_codec::declaration_hash(&bytes);
    let catalogue_declaration_hash =
        ContentHash::try_from_hex(&catalogue_hash_hex).map_err(|e| {
            format!("internal: catalogue hash for layer '{layer_id}' is not canonical hex: {e}")
        })?;

    // Summary counts for stdout (same pattern as `sotp track type-signals`).
    let (blue, yellow, red) = count_signals(&signals);

    // Build the document and persist atomically.
    let doc = CatalogueSpecSignalsDocument::new(catalogue_declaration_hash, signals);

    writer
        .write_catalogue_spec_signals(&track_id_domain, layer_id, &doc)
        .map_err(|e| format!("cannot write catalogue-spec signals for layer '{layer_id}': {e}"))?;

    println!(
        "[OK] catalogue-spec-signals: layer={layer_id} blue={blue} yellow={yellow} red={red} (total={})",
        blue + yellow + red
    );
    Ok(())
}

/// Convert a v2 [`TypeAction`] to the corresponding v3 [`ItemAction`].
///
/// Used when computing catalogue-spec signals for a v2 `TypeCatalogueEntry` so
/// that `evaluate_catalogue_entry_signal` (which uses the v3 `ItemAction` enum)
/// can apply the D5 reference-action exemption correctly.
fn type_action_to_item_action(action: TypeAction) -> ItemAction {
    match action {
        TypeAction::Add => ItemAction::Add,
        TypeAction::Modify => ItemAction::Modify,
        TypeAction::Reference => ItemAction::Reference,
        TypeAction::Delete => ItemAction::Delete,
    }
}

fn count_signals(signals: &[CatalogueSpecSignal]) -> (usize, usize, usize) {
    let mut blue = 0;
    let mut yellow = 0;
    let mut red = 0;
    for s in signals {
        match s.signal {
            ConfidenceSignal::Blue => blue += 1,
            ConfidenceSignal::Yellow => yellow += 1,
            ConfidenceSignal::Red => red += 1,
            _ => red += 1,
        }
    }
    (blue, yellow, red)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use domain::ConfidenceSignal;

    use crate::tddd::catalogue_document_codec::CatalogueDocumentCodec;

    use super::*;

    /// Build a v3 document with three type entries and verify all three signal colours:
    ///
    ///   - "BlueType": has spec_refs → Blue
    ///   - "YellowType": has informal_grounds → Yellow
    ///   - "RedType": no grounding → Red
    ///
    /// Simulates the refresher's signal computation inline.
    #[test]
    fn test_v3_branch_computes_blue_yellow_red_from_grounding_fields() {
        let json = r#"{
  "schema_version": 3,
  "crate_name": "domain",
  "layer": "domain",
  "types": {
    "BlueType": {
      "action": "add",
      "role": "ValueObject",
      "kind": { "kind": "unit_struct" },
      "spec_refs": [
        { "file": "track/items/x/spec.json", "anchor": "IN-01", "hash": "0000000000000000000000000000000000000000000000000000000000000000" }
      ],
      "informal_grounds": []
    },
    "YellowType": {
      "action": "add",
      "role": "ValueObject",
      "kind": { "kind": "unit_struct" },
      "spec_refs": [],
      "informal_grounds": [
        { "kind": "discussion", "summary": "planning note" }
      ]
    },
    "RedType": {
      "action": "add",
      "role": "ValueObject",
      "kind": { "kind": "unit_struct" },
      "spec_refs": [],
      "informal_grounds": []
    }
  },
  "traits": {},
  "functions": {}
}"#;

        let v3_doc = CatalogueDocumentCodec::decode(json, "domain").unwrap();

        // Replicate the refresher's v3 signal computation inline.
        let mut signals: Vec<CatalogueSpecSignal> = Vec::new();
        for (type_name, entry) in &v3_doc.types {
            let signal = evaluate_catalogue_entry_signal(
                entry.action,
                &entry.spec_refs,
                &entry.informal_grounds,
            );
            signals.push(CatalogueSpecSignal::new(type_name.as_str(), signal));
        }
        for (trait_name, entry) in &v3_doc.traits {
            let signal = evaluate_catalogue_entry_signal(
                entry.action,
                &entry.spec_refs,
                &entry.informal_grounds,
            );
            signals.push(CatalogueSpecSignal::new(trait_name.as_str(), signal));
        }
        for (fn_path, entry) in &v3_doc.functions {
            let signal = evaluate_catalogue_entry_signal(
                entry.action,
                &entry.spec_refs,
                &entry.informal_grounds,
            );
            signals.push(CatalogueSpecSignal::new(fn_path.to_string(), signal));
        }

        let (blue, yellow, red) = count_signals(&signals);
        assert_eq!(blue, 1, "expected 1 Blue signal (BlueType)");
        assert_eq!(yellow, 1, "expected 1 Yellow signal (YellowType)");
        assert_eq!(red, 1, "expected 1 Red signal (RedType)");

        // Verify that BlueType → Blue and YellowType → Yellow specifically.
        let blue_sig = signals.iter().find(|s| s.type_name == "BlueType").unwrap();
        assert_eq!(blue_sig.signal, ConfidenceSignal::Blue);

        let yellow_sig = signals.iter().find(|s| s.type_name == "YellowType").unwrap();
        assert_eq!(yellow_sig.signal, ConfidenceSignal::Yellow);

        let red_sig = signals.iter().find(|s| s.type_name == "RedType").unwrap();
        assert_eq!(red_sig.signal, ConfidenceSignal::Red);
    }
}
