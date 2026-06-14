//! Per-layer catalogue-spec-signal refresh logic.
//!
//! Moved from the CLI layer so that the CLI composition root never imports
//! `domain::CatalogueSpecSignal`, `domain::CatalogueSpecSignalsDocument`,
//! `domain::ConfidenceSignal`, `domain::ContentHash`, or
//! `domain::evaluate_catalogue_entry_signal` directly (CN-01 / AC-03).
//!
//! Supports v3 (`CatalogueDocument`) catalogue format only. Per-entry signals
//! are computed from `spec_refs[]` and `informal_grounds[]` via
//! `evaluate_catalogue_entry_signal`.
//! The informal-priority rule (ADR `2026-04-23-0344` §D1.1) applies, with the
//! `action: "reference"` exemption from ADR `2026-05-11-1257` D5:
//! - `action == Reference` + both empty → Blue (baseline-implicit grounding)
//! - `informal_grounds` non-empty → Yellow
//! - `informal_grounds` empty + `spec_refs` non-empty → Blue
//! - both empty (non-reference action) → Red
//!
//! Non-v3 catalogues (schema_version ≠ 3) are treated as
//! `CatalogueDocumentCodecError::UnsupportedSchemaVersion` and returned as a
//! fail-closed error (CN-11). No v2 fallback is attempted.
//!
//! ADR reference: `2026-04-23-0344-catalogue-spec-signal-activation.md`
//! §D2 / §D3.1 / IN-09; `2026-05-11-1257-tddd-v2-catalogue-spec-link-restoration.md` D5;
//! `2026-05-08-0248-tddd-catalogue-layer-schema-axis-separation.md` D1 (CN-11).

use std::path::Path;

use domain::{
    CatalogueSpecSignal, CatalogueSpecSignalsDocument, ConfidenceSignal, ContentHash, TrackId,
    evaluate_catalogue_entry_signal,
};

use crate::tddd::catalogue_document_codec::CatalogueDocumentCodec;
use crate::tddd::fs_catalogue_spec_signals_store::FsCatalogueSpecSignalsStore;
use crate::tddd::type_signals_codec;
use crate::track::symlink_guard::reject_symlinks_below;
use crate::verify::plan_artifact_refs::{canonical_json, canonical_json_sha256};
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

    // v3-only: decode via CatalogueDocumentCodec. Any error — including
    // UnsupportedSchemaVersion for non-v3 catalogues — is propagated directly
    // as a fail-closed error (CN-11). No v2 fallback is attempted.
    let v3_doc = CatalogueDocumentCodec::decode(text, &filename_stem)
        .map_err(|e| format!("cannot decode catalogue '{}': {e}", catalogue_path.display()))?;

    // Parse the catalogue bytes as raw JSON to extract per-entry canonical JSON
    // subtrees for `entry_hash` computation (CN-04 / IN-05 / AC-06 of ADR
    // `2026-05-27-1601-sot-chain-semantic-review-gate.md`).
    // The hash is computed over the canonical JSON of each entry's value object
    // within the `types`, `traits`, or `functions` map.
    let raw_json: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| {
        format!(
            "cannot re-parse catalogue '{}' as JSON for entry_hash: {e}",
            catalogue_path.display()
        )
    })?;

    let mut signals: Vec<CatalogueSpecSignal> = Vec::new();
    for (type_name, entry) in &v3_doc.types {
        let signal = evaluate_catalogue_entry_signal(
            entry.action,
            &entry.spec_refs,
            &entry.informal_grounds,
        );
        let entry_hash = catalogue_entry_hash(&raw_json, "types", type_name.as_str())
            .map_err(|e| format!("entry_hash for type '{type_name}': {e}"))?;
        signals.push(CatalogueSpecSignal::new(type_name.as_str(), signal, entry_hash));
    }
    for (trait_name, entry) in &v3_doc.traits {
        let signal = evaluate_catalogue_entry_signal(
            entry.action,
            &entry.spec_refs,
            &entry.informal_grounds,
        );
        let entry_hash = catalogue_entry_hash(&raw_json, "traits", trait_name.as_str())
            .map_err(|e| format!("entry_hash for trait '{trait_name}': {e}"))?;
        signals.push(CatalogueSpecSignal::new(trait_name.as_str(), signal, entry_hash));
    }
    for (fn_path, entry) in &v3_doc.functions {
        let signal = evaluate_catalogue_entry_signal(
            entry.action,
            &entry.spec_refs,
            &entry.informal_grounds,
        );
        let fn_key = fn_path.to_string();
        let entry_hash = catalogue_entry_hash(&raw_json, "functions", &fn_key)
            .map_err(|e| format!("entry_hash for function '{fn_key}': {e}"))?;
        signals.push(CatalogueSpecSignal::new(fn_key, signal, entry_hash));
    }

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

/// Compute the SHA-256 of the canonical JSON for a single catalogue entry.
///
/// Looks up `section[entry_key]` in the raw catalogue `serde_json::Value`
/// and hashes the canonical JSON of that subtree.  Returns an error string
/// (human-readable) when the section or key is absent — which would indicate
/// a mismatch between the decoded `CatalogueDocument` and the raw JSON (should
/// never occur with a well-formed v3 catalogue).
fn catalogue_entry_hash(
    raw: &serde_json::Value,
    section: &str,
    entry_key: &str,
) -> Result<ContentHash, String> {
    let value = raw.get(section).and_then(|s| s.get(entry_key)).ok_or_else(|| {
        format!(
            "internal: catalogue entry '{entry_key}' not found in section '{section}' of raw JSON"
        )
    })?;
    let json_str = canonical_json(value);
    let hex = canonical_json_sha256(&json_str);
    ContentHash::try_from_hex(&hex).map_err(|e| {
        format!("internal: canonical_json_sha256 produced non-hex output for '{entry_key}': {e}")
    })
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
    fn test_v4_catalogue_computes_blue_yellow_red_from_grounding_fields() {
        let json = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {
    "BlueType": {
      "action": "add",
      "role": { "ValueObject": {} },
      "kind": { "kind": "struct", "shape": { "kind": "unit" } },
      "spec_refs": [
        { "file": "track/items/x/spec.json", "anchor": "IN-01" }
      ],
      "informal_grounds": []
    },
    "YellowType": {
      "action": "add",
      "role": { "ValueObject": {} },
      "kind": { "kind": "struct", "shape": { "kind": "unit" } },
      "spec_refs": [],
      "informal_grounds": [
        { "kind": "discussion", "summary": "planning note" }
      ]
    },
    "RedType": {
      "action": "add",
      "role": { "ValueObject": {} },
      "kind": { "kind": "struct", "shape": { "kind": "unit" } },
      "spec_refs": [],
      "informal_grounds": []
    }
  },
  "traits": {},
  "functions": {}
}"#;

        let v3_doc = CatalogueDocumentCodec::decode(json, "domain").unwrap();

        // Replicate the refresher's signal computation inline.
        let raw_json: serde_json::Value = serde_json::from_str(json).unwrap();
        let mut signals: Vec<CatalogueSpecSignal> = Vec::new();
        for (type_name, entry) in &v3_doc.types {
            let signal = evaluate_catalogue_entry_signal(
                entry.action,
                &entry.spec_refs,
                &entry.informal_grounds,
            );
            let entry_hash = catalogue_entry_hash(&raw_json, "types", type_name.as_str()).unwrap();
            signals.push(CatalogueSpecSignal::new(type_name.as_str(), signal, entry_hash));
        }
        for (trait_name, entry) in &v3_doc.traits {
            let signal = evaluate_catalogue_entry_signal(
                entry.action,
                &entry.spec_refs,
                &entry.informal_grounds,
            );
            let entry_hash =
                catalogue_entry_hash(&raw_json, "traits", trait_name.as_str()).unwrap();
            signals.push(CatalogueSpecSignal::new(trait_name.as_str(), signal, entry_hash));
        }
        for (fn_path, entry) in &v3_doc.functions {
            let signal = evaluate_catalogue_entry_signal(
                entry.action,
                &entry.spec_refs,
                &entry.informal_grounds,
            );
            let fn_key = fn_path.to_string();
            let entry_hash = catalogue_entry_hash(&raw_json, "functions", &fn_key).unwrap();
            signals.push(CatalogueSpecSignal::new(fn_key, signal, entry_hash));
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
