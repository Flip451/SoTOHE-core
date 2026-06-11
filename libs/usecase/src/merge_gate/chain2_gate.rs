//! Chain ② (catalogue-spec integrity + signal gate) per-layer check.
//!
//! Extracted from `merge_gate` to keep that module under the 700-line
//! production-code limit. Contains the per-layer body of the Chain ② loop
//! in [`super::check_strict_merge_gate`].

use std::collections::{BTreeMap, HashMap};

use domain::tddd::catalogue_v2::CatalogueDocument;
use domain::verify::{VerifyFinding, VerifyOutcome};
use domain::{CatalogueSpecSignalsDocument, ConfidenceSignal, ContentHash, SpecElementId};

use super::{BlobFetchResult, TrackBlobReader};
use crate::catalogue_traversal::iter_catalogue_entries;

/// Evaluates Chain ② (catalogue-spec integrity + signal gate) for a single
/// opted-in layer.
///
/// Returns a [`VerifyOutcome`] that may be empty (all checks passed) or carry
/// one or more findings. The caller is responsible for merging the result into
/// its accumulated outcome and for the opted-in / enabled-layer filtering that
/// precedes this call.
///
/// # Parameters
/// - `reader`: port implementation, used to fetch the signals document and the
///   catalogue document for `layer_id`.
/// - `branch`: the PR branch ref (e.g. `"track/foo-2026-06-07"`).
/// - `track_id`: the track slug derived from `branch` (e.g. `"foo-2026-06-07"`).
/// - `layer_id`: the TDDD layer being evaluated (e.g. `"domain"`, `"usecase"`).
/// - `spec_element_hashes`: the spec-element anchor → hash map produced by
///   [`crate::catalogue_spec_refs::SpecElementHashReader::read_spec_element_hashes`].
pub(super) fn check_chain2_for_layer<R: TrackBlobReader>(
    reader: &R,
    branch: &str,
    track_id: &str,
    layer_id: &str,
    spec_element_hashes: &BTreeMap<SpecElementId, ContentHash>,
) -> VerifyOutcome {
    let mut outcome = VerifyOutcome::pass();

    // Step 1: read signals file.
    //
    // For an opted-in layer the signals file MUST exist on the branch —
    // treating `NotFound` as silent skip would let a PR bypass Chain ②
    // by deleting `<layer>-catalogue-spec-signals.json` while leaving the
    // opt-in flag set. Fail-closed with a remediation hint.
    let signals_doc: CatalogueSpecSignalsDocument =
        match reader.read_catalogue_spec_signals_document(branch, track_id, layer_id) {
            BlobFetchResult::Found(doc) => doc,
            BlobFetchResult::NotFound => {
                return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                    "opted-in layer '{layer_id}' is missing \
                 <layer>-catalogue-spec-signals.json on origin/{branch}. Run \
                 `sotp track catalogue-spec-signals` and commit the generated file \
                 so the merge gate can evaluate Chain ②."
                ))]);
            }
            BlobFetchResult::FetchError(msg) => {
                return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                    "failed to read catalogue-spec signals for layer '{layer_id}' \
                 on origin/{branch}: {msg}"
                ))]);
            }
        };

    // Step 2: read catalogue document + hash.
    //
    // Opted-in layers are also `tddd.enabled` (the set is a strict subset),
    // so a missing catalogue on an opted-in layer is an integrity violation,
    // not a benign opt-out. Fail-closed.
    //
    // Returns `(doc, raw_bytes_sha256_hex, entry_hashes)`.
    let (catalogue, catalogue_hash_hex, entry_hashes): (
        CatalogueDocument,
        String,
        HashMap<String, ContentHash>,
    ) = match reader.read_catalogue_for_spec_ref_check(branch, track_id, layer_id) {
        BlobFetchResult::Found(triple) => triple,
        BlobFetchResult::NotFound => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "opted-in layer '{layer_id}' is missing its catalogue file \
                 on origin/{branch}. A layer cannot opt in to Chain ② without the \
                 `<layer>-types.json` catalogue the signals are computed from."
            ))]);
        }
        BlobFetchResult::FetchError(msg) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "failed to read catalogue hash for layer '{layer_id}' \
                 on origin/{branch}: {msg}"
            ))]);
        }
    };
    let catalogue_hash = match ContentHash::try_from_hex(&catalogue_hash_hex) {
        Ok(h) => h,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "catalogue hash for layer '{layer_id}' is not canonical hex: {e}"
            ))]);
        }
    };

    // Step 3: integrity binary gate (dangling / stale).
    //
    // Iterates types + traits + functions BTreeMaps and checks each entry's
    // `spec_refs` for dangling anchors. StaleSignals check
    // uses `signals_doc.catalogue_declaration_hash`.
    let catalogue_file = format!("{layer_id}-types.json");
    let mut integrity_errors: Vec<VerifyFinding> = Vec::new();

    for (type_name, entry) in &catalogue.types {
        check_spec_refs_for_entry(
            layer_id,
            type_name.as_str(),
            &entry.spec_refs,
            spec_element_hashes,
            &mut integrity_errors,
        );
    }
    for (trait_name, entry) in &catalogue.traits {
        check_spec_refs_for_entry(
            layer_id,
            trait_name.as_str(),
            &entry.spec_refs,
            spec_element_hashes,
            &mut integrity_errors,
        );
    }
    for (fn_path, entry) in &catalogue.functions {
        check_spec_refs_for_entry(
            layer_id,
            &fn_path.to_string(),
            &entry.spec_refs,
            spec_element_hashes,
            &mut integrity_errors,
        );
    }

    // StaleSignals: compare signals_doc.catalogue_declaration_hash to current hash.
    if signals_doc.catalogue_declaration_hash != catalogue_hash {
        integrity_errors.push(VerifyFinding::error(format!(
            "catalogue-spec integrity violation on layer '{layer_id}': \
             StaleSignals {{ declared: {:?}, actual: {:?} }}",
            signals_doc.catalogue_declaration_hash, catalogue_hash
        )));
    }

    if !integrity_errors.is_empty() {
        outcome.merge(VerifyOutcome::from_findings(integrity_errors));
        return outcome;
    }

    // Step 4: signal gate — strict=true (merge gate blocks Yellow).
    //
    // Coverage check: total entry count must equal signals count, and
    // positional names must match (fail-closed against trimmed signals files).
    let total_entries = catalogue.types.len() + catalogue.traits.len() + catalogue.functions.len();
    let signals = &signals_doc.signals;
    if total_entries != signals.len() {
        outcome.merge(VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
            "{catalogue_file}: catalogue-spec signals coverage mismatch — catalogue has \
             {total_entries} entry/entries, signals document has {} signal(s). \
             Regenerate the signals file with `sotp track catalogue-spec-signals` so \
             every catalogue entry is covered.",
            signals.len()
        ))]));
        return outcome;
    }

    // Positional name match: types → traits → functions, BTreeMap iteration order.
    let catalogue_names: Vec<String> = catalogue
        .types
        .keys()
        .map(|k| k.as_str().to_owned())
        .chain(catalogue.traits.keys().map(|k| k.as_str().to_owned()))
        .chain(catalogue.functions.keys().map(|k| k.to_string()))
        .collect();
    if let Some((i, cat_name, sig)) = catalogue_names
        .iter()
        .zip(signals.iter())
        .enumerate()
        .find(|(_, (cat_name, sig))| cat_name.as_str() != sig.type_name.as_str())
        .map(|(i, (cat_name, sig))| (i, cat_name, sig))
    {
        outcome.merge(VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
            "{catalogue_file}: catalogue-spec signals positional mismatch at index {i} \
             (catalogue entry '{cat_name}' vs signal '{}'). Regenerate the signals file.",
            sig.type_name
        ))]));
        return outcome;
    }

    if signals.is_empty() {
        // Empty on both sides: pass (empty layer).
        return outcome;
    }

    // Per-entry hash freshness check (AC-06 / IN-05 / D4 of ADR
    // `2026-05-27-1601-sot-chain-semantic-review-gate.md`).
    //
    // Each signal carries `entry_hash` — the SHA-256 of the catalogue
    // entry's canonical JSON subtree at signal-generation time.
    // `entry_hashes` holds the same hashes freshly computed from the
    // current catalogue bytes.
    //
    // A mismatch means the signals file was not regenerated after the
    // catalogue changed, so a semantic Chain ② cache key would reference
    // the wrong entry content. The gate blocks until the signals file is
    // regenerated (`sotp track catalogue-spec-signals`).
    let mut entry_hash_errors: Vec<VerifyFinding> = Vec::new();
    for (entry, signal) in iter_catalogue_entries(&catalogue).zip(signals.iter()) {
        match entry_hashes.get(entry.section_key.as_str()) {
            None => {
                entry_hash_errors.push(VerifyFinding::error(format!(
                    "{catalogue_file}: per-entry hash missing for '{entry_key}' \
                     (section_key '{section_key}') — the catalogue adapter did not \
                     supply a hash for this entry. Regenerate the signals file.",
                    entry_key = entry.key,
                    section_key = entry.section_key,
                )));
            }
            Some(current_hash) if current_hash != signal.entry_hash() => {
                entry_hash_errors.push(VerifyFinding::error(format!(
                    "{catalogue_file}: per-entry hash mismatch for '{entry_key}' — \
                     signals file records {{declared: {declared:?}}} but current catalogue \
                     entry has {{actual: {actual:?}}}. Regenerate the signals file with \
                     `sotp track catalogue-spec-signals`.",
                    entry_key = entry.key,
                    declared = signal.entry_hash(),
                    actual = current_hash,
                )));
            }
            Some(_) => {} // hash matches — no finding
        }
    }
    if !entry_hash_errors.is_empty() {
        outcome.merge(VerifyOutcome::from_findings(entry_hash_errors));
        return outcome;
    }

    // Confidence signal gate: Red and Yellow both block in strict merge mode.
    let reds: Vec<&str> = signals
        .iter()
        .filter(|s| s.signal == ConfidenceSignal::Red)
        .map(|s| s.type_name.as_str())
        .collect();
    if !reds.is_empty() {
        outcome.merge(VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
            "{catalogue_file}: {} catalogue entry/entries have Red catalogue-spec signal \
             (missing both spec_refs[] and informal_grounds[] — every entry must carry \
             at least one grounding ref): {}",
            reds.len(),
            reds.join(", ")
        ))]));
        return outcome;
    }

    let yellows: Vec<&str> = signals
        .iter()
        .filter(|s| s.signal == ConfidenceSignal::Yellow)
        .map(|s| s.type_name.as_str())
        .collect();
    if !yellows.is_empty() {
        outcome.merge(VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
            "{catalogue_file}: {} catalogue entry/entries have Yellow catalogue-spec signal \
             — merge gate will block these until upgraded to Blue. Upgrade by promoting \
             informal_grounds[] to spec_refs[] entries with file + anchor, \
             then regenerate catalogue-spec signals: {}",
            yellows.len(),
            yellows.join(", ")
        ))]));
    }

    outcome
}

/// Checks all `spec_refs` for a single catalogue entry, pushing any findings
/// into `errors`.
fn check_spec_refs_for_entry(
    layer_id: &str,
    entry_name: &str,
    spec_refs: &[domain::SpecRef],
    spec_element_hashes: &BTreeMap<SpecElementId, ContentHash>,
    errors: &mut Vec<VerifyFinding>,
) {
    for (ref_index, spec_ref) in spec_refs.iter().enumerate() {
        if !spec_element_hashes.contains_key(&spec_ref.anchor) {
            errors.push(VerifyFinding::error(format!(
                "catalogue-spec integrity violation on layer '{layer_id}': \
                 DanglingAnchor {{ catalogue_entry: {:?}, ref_index: {ref_index}, \
                 spec_file: {:?}, anchor: {:?} }}",
                entry_name, spec_ref.file, spec_ref.anchor
            )));
        }
    }
}
