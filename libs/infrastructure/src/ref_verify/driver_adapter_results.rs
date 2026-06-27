//! Results command helpers for [`super::driver_adapter::FsRefVerifyAggregateAdapter`].
//!
//! Extracted from `driver_adapter.rs` to stay under the 700-line module-size cap.

use std::path::Path;

use usecase::ref_verify::RefVerifyDriverError;

/// Load TDDD layer bindings for the `results` command.
///
/// Returns an empty binding set when `architecture-rules.json` is not present
/// (pre-TDDD repository or zero-layer state), matching the behaviour of
/// [`super::RefVerifyScopeResolver`].  Any other I/O error (corrupt file,
/// permission denied) or parse error propagates as
/// [`RefVerifyDriverError::Wiring`] so those cases remain fail-closed.
pub(crate) fn load_results_tddd_bindings(
    project_root: &Path,
) -> Result<Vec<crate::verify::tddd_layers::TdddLayerBinding>, RefVerifyDriverError> {
    use crate::verify::tddd_layers::LoadTdddLayersError;
    let rules_path = project_root.join("architecture-rules.json");
    match crate::verify::tddd_layers::load_tddd_layers(&rules_path, project_root) {
        Ok(bindings) => Ok(bindings),
        // Absent rules file → pre-TDDD repository; treat as zero bindings,
        // consistent with RefVerifyScopeResolver::load_bindings_or_empty.
        Err(LoadTdddLayersError::Io { source, .. })
            if source.kind() == std::io::ErrorKind::NotFound =>
        {
            Ok(Vec::new())
        }
        Err(e) => Err(RefVerifyDriverError::Wiring(format!(
            "cannot load TDDD layer bindings for results: {e}"
        ))),
    }
}

pub(crate) fn resolve_results_chain2_target_layers(
    bindings: &[crate::verify::tddd_layers::TdddLayerBinding],
    layer: &usecase::ref_verify::RefVerifyLayerFilter,
) -> Result<Vec<domain::tddd::LayerId>, RefVerifyDriverError> {
    use domain::tddd::LayerId;
    use usecase::ref_verify::RefVerifyLayerFilter;

    match layer {
        RefVerifyLayerFilter::Specific(layer_id) => {
            if !bindings.iter().any(|b| b.layer_id() == layer_id.as_ref()) {
                let valid: Vec<&str> = bindings.iter().map(|b| b.layer_id()).collect();
                let valid_list = if valid.is_empty() {
                    "(none - no TDDD layers configured)".to_owned()
                } else {
                    valid.join(", ")
                };
                return Err(RefVerifyDriverError::Wiring(format!(
                    "unknown layer '{}' for --layer filter; valid TDDD layers: {valid_list}",
                    layer_id.as_ref()
                )));
            }
            Ok(vec![layer_id.clone()])
        }
        RefVerifyLayerFilter::All => bindings
            .iter()
            .map(|binding| {
                LayerId::try_new(binding.layer_id().to_owned()).map_err(|e| {
                    RefVerifyDriverError::Wiring(format!(
                        "invalid TDDD layer '{}' for results: {e}",
                        binding.layer_id()
                    ))
                })
            })
            .collect(),
    }
}

/// Classify one pair against a cache lookup map and derive its origin references.
///
/// The cache map holds `Vec<&SemanticVerifyEntry>` per `(claim_hash, evidence_hash)` key
/// to handle duplicate catalogue entries that share the same hash pair but have different
/// `claim_origin` / `evidence_origin` (e.g. two catalogue entries with identical canonical
/// JSON but distinct entry keys).  When multiple entries share a key, the Vec is searched
/// for an entry whose origins exactly match `current_claim_origin` and
/// `current_evidence_origin`; only that entry's verdict is used.  If no exact origin match
/// is found, the pair is treated as a cache miss (pending).
///
/// - `Pass`: origins from the origin-matched cache entry; empty reason.
/// - `Fail`: origins from the origin-matched cache entry; reason from cached `Fail { reason }`.
/// - `Pending` (no origin-matched entry or `Pending` verdict in matched entry): origins from
///   the `current_claim_origin` / `current_evidence_origin` parameters — always the calling
///   pair's own origins, so pairs in different layers that share identical
///   `(claim_hash, evidence_hash)` values each carry the correct per-layer origin reference.
fn extract_verdict_and_origins(
    key: &(domain::ContentHash, domain::ContentHash),
    cache_map: &std::collections::HashMap<
        (domain::ContentHash, domain::ContentHash),
        Vec<&domain::tddd::semantic_verify::SemanticVerifyEntry>,
    >,
    current_claim_origin: &domain::tddd::semantic_verify::VerifyOriginRef,
    current_evidence_origin: &domain::tddd::semantic_verify::VerifyOriginRef,
) -> (
    domain::tddd::semantic_verify::SemanticVerdict,
    String,
    domain::tddd::semantic_verify::VerifyOriginRef,
    domain::tddd::semantic_verify::VerifyOriginRef,
) {
    use domain::tddd::semantic_verify::SemanticVerdict;

    // Find the cache entry whose origins exactly match the current pair.
    // When multiple entries share the same hash pair (duplicate catalogue entries with
    // identical canonical JSON but distinct entry_keys), only the entry whose
    // claim_origin + evidence_origin match is selected; others are ignored for this pair.
    let matching_entry = cache_map.get(key).and_then(|entries| {
        entries
            .iter()
            .find(|e| {
                &e.claim_origin == current_claim_origin
                    && &e.evidence_origin == current_evidence_origin
            })
            .copied()
    });

    match matching_entry {
        Some(entry) => match &entry.verdict {
            SemanticVerdict::Pass { .. } => (
                entry.verdict.clone(),
                String::new(),
                entry.claim_origin.clone(),
                entry.evidence_origin.clone(),
            ),
            SemanticVerdict::Fail { reason } => (
                entry.verdict.clone(),
                reason.clone(),
                entry.claim_origin.clone(),
                entry.evidence_origin.clone(),
            ),
            // Pending in cache → unresolved; use current pair's origin (per-layer correct).
            SemanticVerdict::Pending => (
                SemanticVerdict::Pending,
                "pair not yet verified".to_owned(),
                current_claim_origin.clone(),
                current_evidence_origin.clone(),
            ),
        },
        // Cache miss (no entry for key, or no origin-matched entry) → pending;
        // use current pair's origin (per-layer correct).
        None => (
            SemanticVerdict::Pending,
            "pair not yet verified".to_owned(),
            current_claim_origin.clone(),
            current_evidence_origin.clone(),
        ),
    }
}

/// Core classification and assembly logic for `ref-verify results`.
///
/// Separated from the I/O-bound adapter method so it can be unit-tested without
/// filesystem access. All parameters are pre-loaded; no I/O or LLM calls are
/// made inside this function.
///
/// # Errors
///
/// Returns [`usecase::ref_verify::RefVerifyDriverError::Wiring`] when the
/// chain filter includes Chain2 and a `Specific` layer filter names a layer
/// that is not present in the resolved TDDD bindings (`chain2_caches`).
#[allow(clippy::too_many_lines)]
pub(crate) fn compute_results(
    chain1_cache: Vec<domain::tddd::semantic_verify::SemanticVerifyEntry>,
    chain2_caches: Vec<(
        domain::tddd::LayerId,
        Vec<domain::tddd::semantic_verify::SemanticVerifyEntry>,
    )>,
    current_pairs: Vec<usecase::ref_verify::RefVerifyPair>,
    chain: usecase::ref_verify::RefVerifyChainFilter,
    layer: usecase::ref_verify::RefVerifyLayerFilter,
    verdict: usecase::ref_verify::RefVerifyVerdictFilter,
) -> Result<usecase::ref_verify::RefVerifyResultsOutput, usecase::ref_verify::RefVerifyDriverError>
{
    use domain::ContentHash;
    use domain::tddd::semantic_verify::{SemanticVerdict, SemanticVerifyEntry};
    use std::collections::HashMap;
    use usecase::ref_verify::{
        RefVerifyCacheScope, RefVerifyChainFilter, RefVerifyDriverError, RefVerifyLaneSummary,
        RefVerifyLayerFilter, RefVerifyPairRecord, RefVerifyResultsOutput, RefVerifyVerdictFilter,
    };

    // Determine which chains are included before validation and pre-initialization.
    let include_chain1 = matches!(chain, RefVerifyChainFilter::Chain1 | RefVerifyChainFilter::All);
    let include_chain2 = matches!(chain, RefVerifyChainFilter::Chain2 | RefVerifyChainFilter::All);

    // Validate layer filter when Chain2 results are requested.
    if include_chain2 {
        if let RefVerifyLayerFilter::Specific(layer_id) = &layer {
            let valid: Vec<&str> = chain2_caches.iter().map(|(id, _)| id.as_ref()).collect();
            if !valid.contains(&layer_id.as_ref()) {
                let valid_list = if valid.is_empty() {
                    "(none — no TDDD layers configured)".to_owned()
                } else {
                    valid.join(", ")
                };
                return Err(RefVerifyDriverError::Wiring(format!(
                    "unknown layer '{}' for --layer filter; \
                     valid TDDD layers: {valid_list}",
                    layer_id.as_ref(),
                )));
            }
        }
    }

    type HashKey = (ContentHash, ContentHash);

    // Per-lane accumulator: collects counts and records during pair classification.
    struct LaneAccum {
        label: String,
        pass_count: usize,
        fail_count: usize,
        pending_count: usize,
        records: Vec<RefVerifyPairRecord>,
    }

    // Build cache lookup maps keyed by (claim_hash, evidence_hash), collecting all
    // entries per key into a Vec.  When multiple cache entries share the same hash
    // pair (e.g. duplicate catalogue entries with identical canonical JSON but
    // distinct entry keys), all are retained so that extract_verdict_and_origins can
    // select the entry whose origins exactly match each current pair.
    let chain1_map: HashMap<HashKey, Vec<&SemanticVerifyEntry>> =
        chain1_cache.iter().fold(HashMap::new(), |mut acc, e| {
            acc.entry((e.claim_hash.clone(), e.evidence_hash.clone())).or_default().push(e);
            acc
        });

    let chain2_maps: HashMap<String, HashMap<HashKey, Vec<&SemanticVerifyEntry>>> = chain2_caches
        .iter()
        .map(|(layer_id, entries)| {
            let map: HashMap<HashKey, Vec<&SemanticVerifyEntry>> =
                entries.iter().fold(HashMap::new(), |mut acc, e| {
                    acc.entry((e.claim_hash.clone(), e.evidence_hash.clone())).or_default().push(e);
                    acc
                });
            (layer_id.as_ref().to_owned(), map)
        })
        .collect();

    // Layer filter closure — defined here so it can be used during pre-initialization.
    let layer_matches = |lane_layer: &str| match &layer {
        RefVerifyLayerFilter::All => true,
        RefVerifyLayerFilter::Specific(id) => id.as_ref() == lane_layer,
    };

    // Pre-initialize lanes from the resolved chain/layer set so that every resolved
    // lane appears in the summary even when it has zero pairs (AC-02 / AC-07 / T004).
    //
    // Chain1 lane: present whenever include_chain1 is true.
    let mut chain1_lane: Option<LaneAccum> = if include_chain1 {
        Some(LaneAccum {
            label: "Chain1 (spec\u{2194}ADR)".to_owned(),
            pass_count: 0,
            fail_count: 0,
            pending_count: 0,
            records: Vec::new(),
        })
    } else {
        None
    };

    // Chain2 lanes: one per layer in chain2_caches that passes the layer filter.
    // Insertion order is preserved for deterministic output.
    let mut chain2_lane_order: Vec<String> = Vec::new();
    let mut chain2_lane_map: HashMap<String, LaneAccum> = HashMap::new();
    if include_chain2 {
        for (layer_id, _) in &chain2_caches {
            let layer_str = layer_id.as_ref().to_owned();
            if !layer_matches(&layer_str) {
                continue;
            }
            if !chain2_lane_map.contains_key(&layer_str) {
                chain2_lane_order.push(layer_str.clone());
                chain2_lane_map.insert(
                    layer_str.clone(),
                    LaneAccum {
                        label: format!("Chain2:{layer_str}"),
                        pass_count: 0,
                        fail_count: 0,
                        pending_count: 0,
                        records: Vec::new(),
                    },
                );
            }
        }
    }

    let empty_map: HashMap<HashKey, Vec<&SemanticVerifyEntry>> = HashMap::new();

    // Classify pairs and increment the pre-initialized lane counts.
    // Pairs whose lane is absent (excluded by chain or layer filter) are skipped.
    for pair in &current_pairs {
        let key = (pair.claim_hash.clone(), pair.evidence_hash.clone());

        match &pair.cache_scope {
            RefVerifyCacheScope::SpecAdr => {
                let (v, r, co, eo) = extract_verdict_and_origins(
                    &key,
                    &chain1_map,
                    &pair.claim_origin,
                    &pair.evidence_origin,
                );
                if let Some(lane) = chain1_lane.as_mut() {
                    match &v {
                        SemanticVerdict::Pass { .. } => lane.pass_count += 1,
                        SemanticVerdict::Fail { .. } => lane.fail_count += 1,
                        SemanticVerdict::Pending => lane.pending_count += 1,
                    }
                    lane.records.push(RefVerifyPairRecord {
                        chain_scope: RefVerifyCacheScope::SpecAdr,
                        chain_layer: "Chain1".to_owned(),
                        claim_hash: pair.claim_hash.clone(),
                        evidence_hash: pair.evidence_hash.clone(),
                        verdict: v,
                        reason: r,
                        claim_origin: co,
                        evidence_origin: eo,
                    });
                }
            }
            RefVerifyCacheScope::CatalogueSpec { layer: layer_id } => {
                let layer_str = layer_id.as_ref().to_owned();
                let layer_cache = chain2_maps.get(&layer_str).unwrap_or(&empty_map);
                let (v, r, co, eo) = extract_verdict_and_origins(
                    &key,
                    layer_cache,
                    &pair.claim_origin,
                    &pair.evidence_origin,
                );
                if let Some(lane) = chain2_lane_map.get_mut(&layer_str) {
                    match &v {
                        SemanticVerdict::Pass { .. } => lane.pass_count += 1,
                        SemanticVerdict::Fail { .. } => lane.fail_count += 1,
                        SemanticVerdict::Pending => lane.pending_count += 1,
                    }
                    lane.records.push(RefVerifyPairRecord {
                        chain_scope: RefVerifyCacheScope::CatalogueSpec { layer: layer_id.clone() },
                        chain_layer: format!("Chain2:{layer_str}"),
                        claim_hash: pair.claim_hash.clone(),
                        evidence_hash: pair.evidence_hash.clone(),
                        verdict: v,
                        reason: r,
                        claim_origin: co,
                        evidence_origin: eo,
                    });
                }
            }
        }
    }

    // Assemble output from pre-initialized lanes; apply verdict filter to pair_records.
    // include_chain1 / include_chain2 were computed at the top of the function.

    let verdict_matches = |v: &SemanticVerdict| match &verdict {
        RefVerifyVerdictFilter::FailPending => {
            matches!(v, SemanticVerdict::Fail { .. } | SemanticVerdict::Pending)
        }
        RefVerifyVerdictFilter::Pass => matches!(v, SemanticVerdict::Pass { .. }),
        RefVerifyVerdictFilter::Fail => matches!(v, SemanticVerdict::Fail { .. }),
        RefVerifyVerdictFilter::Pending => matches!(v, SemanticVerdict::Pending),
        RefVerifyVerdictFilter::All => true,
    };

    let mut lane_summaries: Vec<RefVerifyLaneSummary> = Vec::new();
    let mut pair_records: Vec<RefVerifyPairRecord> = Vec::new();
    let mut total_pass = 0usize;
    let mut total_fail = 0usize;
    let mut total_pending = 0usize;

    if include_chain1 {
        if let Some(lane) = chain1_lane {
            lane_summaries.push(RefVerifyLaneSummary {
                label: lane.label,
                pass_count: lane.pass_count,
                fail_count: lane.fail_count,
                pending_count: lane.pending_count,
            });
            total_pass += lane.pass_count;
            total_fail += lane.fail_count;
            total_pending += lane.pending_count;
            for record in lane.records {
                if verdict_matches(&record.verdict) {
                    pair_records.push(record);
                }
            }
        }
    }

    if include_chain2 {
        for layer_str in &chain2_lane_order {
            // chain2_lane_order was populated only for layers that passed the layer
            // filter during pre-initialization; no additional filtering is needed here.
            if let Some(lane) = chain2_lane_map.remove(layer_str.as_str()) {
                lane_summaries.push(RefVerifyLaneSummary {
                    label: lane.label,
                    pass_count: lane.pass_count,
                    fail_count: lane.fail_count,
                    pending_count: lane.pending_count,
                });
                total_pass += lane.pass_count;
                total_fail += lane.fail_count;
                total_pending += lane.pending_count;
                for record in lane.records {
                    if verdict_matches(&record.verdict) {
                        pair_records.push(record);
                    }
                }
            }
        }
    }

    Ok(RefVerifyResultsOutput {
        lane_summaries,
        pair_records,
        total_pass,
        total_fail,
        total_pending,
    })
}

// ── Chain1-only scope resolution and pre-flight validation helpers ────────────
//
// Extracted from `driver_adapter.rs` to stay under the 700-line module-size cap.
// These helpers are called from `FsRefVerifyAggregateAdapter::results` in that
// module but live here because they share the same "results-path validation"
// concern as the functions above.

/// Returns `Err(Wiring(...))` when both `present` and `absent` are non-empty
/// (partial catalogue set — fail-closed semantics).
///
/// # Semantics
/// - All-absent (`present` empty): pre-Phase-2 valid → `Ok(())`.
/// - Partial (`present` non-empty AND `absent` non-empty): missing layers → `Err(Wiring)`.
/// - All-present (`absent` empty): normal operation → `Ok(())`.
pub(crate) fn check_partial_catalogue_set(
    present: &[domain::tddd::LayerId],
    absent: &[domain::tddd::LayerId],
) -> Result<(), RefVerifyDriverError> {
    if !present.is_empty() && !absent.is_empty() {
        let missing_list = absent.iter().map(|id| id.as_ref()).collect::<Vec<_>>().join(", ");
        Err(RefVerifyDriverError::Wiring(format!(
            "partial Chain2 catalogue set: catalogue absent for layer(s) [{missing_list}]; \
             run the ref-verify pipeline to generate missing catalogues or check Phase-2 status",
        )))
    } else {
        Ok(())
    }
}

pub(crate) fn inspect_chain2_catalogue_set(
    project_root: &Path,
    track_id: &str,
    bindings: &[crate::verify::tddd_layers::TdddLayerBinding],
) -> Result<(Vec<domain::tddd::LayerId>, Vec<domain::tddd::LayerId>), RefVerifyDriverError> {
    let mut present = Vec::new();
    let mut absent = Vec::new();

    for binding in bindings {
        let layer_id =
            domain::tddd::LayerId::try_new(binding.layer_id().to_owned()).map_err(|e| {
                RefVerifyDriverError::Wiring(format!(
                    "invalid TDDD layer '{}' for results: {e}",
                    binding.layer_id()
                ))
            })?;
        let catalogue_path =
            project_root.join("track").join("items").join(track_id).join(binding.catalogue_file());
        let catalogue_exists =
            crate::track::symlink_guard::reject_symlinks_below(&catalogue_path, project_root)
                .map_err(|e| {
                    RefVerifyDriverError::Wiring(format!(
                        "cannot inspect catalogue '{}': {e}",
                        catalogue_path.display()
                    ))
                })?;
        if catalogue_exists {
            present.push(layer_id);
        } else {
            absent.push(layer_id);
        }
    }

    Ok((present, absent))
}

/// Validate that the track directory `<project_root>/track/items/<track_id>/`
/// exists before any catalogue or cache classification runs.
///
/// A typo in `track_id` would otherwise cause every declared catalogue to be
/// classified as absent, which the "all-absent = pre-Phase-2 valid" branch
/// silently accepts as a zero-pair result.  This function fails closed with a
/// typed [`RefVerifyDriverError::Wiring`] error in that case.
///
/// The validation is chain-filter-agnostic: even a Chain1-only results request
/// must not silently succeed with empty output when the track directory does not
/// exist.
pub(crate) fn check_track_dir_exists(
    canonical_root: &Path,
    track_id: &str,
) -> Result<(), RefVerifyDriverError> {
    let track_dir = canonical_root.join("track").join("items").join(track_id);
    match crate::track::symlink_guard::reject_symlinks_below(&track_dir, canonical_root) {
        Ok(true) => {
            let metadata = track_dir.symlink_metadata().map_err(|e| {
                RefVerifyDriverError::Wiring(format!(
                    "cannot stat track directory '{}': {e}",
                    track_dir.display()
                ))
            })?;
            if metadata.is_dir() {
                Ok(())
            } else {
                Err(RefVerifyDriverError::Wiring(format!(
                    "track path '{}' exists but is not a directory — track_id '{}' is invalid",
                    track_dir.display(),
                    track_id,
                )))
            }
        }
        Ok(false) => Err(RefVerifyDriverError::Wiring(format!(
            "track directory not found: '{}' — track_id '{}' does not exist under track/items",
            track_dir.display(),
            track_id,
        ))),
        Err(e) => Err(RefVerifyDriverError::Wiring(format!(
            "cannot inspect track directory '{}': {e}",
            track_dir.display()
        ))),
    }
}

/// Load TDDD bindings for Chain1-only consistency checks.
///
/// Missing `architecture-rules.json` is the pre-TDDD/zero-layer state and
/// contributes no declared Chain2 catalogues. Malformed rules and other I/O
/// errors still fail closed because declared catalogue state cannot be trusted.
fn load_chain1_scope_tddd_bindings(
    project_root: &Path,
) -> Result<Vec<crate::verify::tddd_layers::TdddLayerBinding>, RefVerifyDriverError> {
    use crate::verify::tddd_layers::LoadTdddLayersError;

    let rules_path = project_root.join("architecture-rules.json");
    match crate::verify::tddd_layers::load_tddd_layers(&rules_path, project_root) {
        Ok(bindings) => Ok(bindings),
        Err(LoadTdddLayersError::Io { source, .. })
            if source.kind() == std::io::ErrorKind::NotFound =>
        {
            Ok(Vec::new())
        }
        Err(e) => Err(RefVerifyDriverError::Wiring(format!(
            "cannot load TDDD layer bindings for Chain1 scope: {e}"
        ))),
    }
}

fn inspect_present_chain2_catalogues_for_chain1_scope(
    project_root: &Path,
    track_id: &str,
    bindings: &[crate::verify::tddd_layers::TdddLayerBinding],
) -> Result<Vec<String>, RefVerifyDriverError> {
    let track_dir = project_root.join("track").join("items").join(track_id);
    let mut present = Vec::new();

    for binding in bindings {
        let catalogue_path = track_dir.join(binding.catalogue_file());
        let exists =
            crate::track::symlink_guard::reject_symlinks_below(&catalogue_path, project_root)
                .map_err(|e| {
                    RefVerifyDriverError::Wiring(format!(
                        "cannot inspect catalogue path '{}': {e}",
                        catalogue_path.display()
                    ))
                })?;
        if exists {
            present.push(binding.catalogue_file().to_owned());
        }
    }

    Ok(present)
}

/// Resolve the scope for a Chain1-only results query without invoking
/// [`RefVerifyScopeResolver`].
///
/// [`RefVerifyScopeResolver::resolve`] enforces Chain2 catalogue consistency
/// (IN-05: partial catalogue set is rejected).  For a Chain1-only request that
/// consistency check is irrelevant and would incorrectly block users from
/// inspecting Chain1 (spec vs ADR) failures while Phase-2 artefacts are still
/// incomplete.
///
/// This function performs the only path check that Chain1 pair enumeration
/// needs before the pair source runs: it verifies that the `spec.json` path can
/// be inspected without crossing a symlink.  A missing `spec.json` remains a
/// legal Phase-0 zero-pair state only when no declared Chain2 catalogue exists.
pub(crate) fn resolve_chain1_only_scope(
    canonical_root: &Path,
    track_id: &str,
) -> Result<usecase::ref_verify::RefVerifyScope, RefVerifyDriverError> {
    use usecase::ref_verify::RefVerifyScope;

    let spec_path = canonical_root.join("track").join("items").join(track_id).join("spec.json");
    let spec_exists =
        crate::track::symlink_guard::reject_symlinks_below(&spec_path, canonical_root).map_err(
            |e| {
                RefVerifyDriverError::Wiring(format!(
                    "cannot inspect spec.json at '{}': {e}",
                    spec_path.display()
                ))
            },
        )?;
    let bindings = load_chain1_scope_tddd_bindings(canonical_root)?;
    let present_catalogues =
        inspect_present_chain2_catalogues_for_chain1_scope(canonical_root, track_id, &bindings)?;
    if !spec_exists && !present_catalogues.is_empty() {
        return Err(RefVerifyDriverError::Wiring(format!(
            "ref-verify results --chain 1: spec.json not found while TDDD catalogue(s) exist for \
             track '{}': {}",
            track_id,
            present_catalogues.join(", ")
        )));
    }
    Ok(RefVerifyScope::Chain1)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    use domain::ContentHash;
    use domain::plan_ref::SpecElementId;
    use domain::tddd::LayerId;
    use domain::tddd::semantic_verify::{
        AdrDecisionRef, CatalogueEntryKey, CatalogueEntryRef, CatalogueSectionKey,
        EvidenceCitation, SemanticVerdict, SemanticVerifyEntry, SpecElementRef, SpecSectionKind,
        VerifyOriginRef,
    };
    use usecase::ref_verify::{
        RefVerifyCacheScope, RefVerifyChainFilter, RefVerifyLayerFilter, RefVerifyPair,
        RefVerifyVerdictFilter,
    };

    fn test_hash(byte: u8) -> ContentHash {
        ContentHash::from_bytes([byte; 32])
    }

    fn spec_origin(byte: u8) -> VerifyOriginRef {
        VerifyOriginRef::SpecElement(SpecElementRef::new(
            SpecSectionKind::Goal,
            SpecElementId::try_new(format!("GO-{byte:02}")).unwrap(),
            format!("spec-{byte:02}"),
        ))
    }

    fn adr_origin(byte: u8) -> VerifyOriginRef {
        VerifyOriginRef::AdrDecision(AdrDecisionRef::new(
            "test.md".to_owned(),
            format!("D{byte:02x}"),
        ))
    }

    fn catalogue_origin(byte: u8, layer: &str) -> VerifyOriginRef {
        VerifyOriginRef::CatalogueEntry(CatalogueEntryRef::new(
            format!("{layer}-types.json"),
            CatalogueSectionKey::Types,
            CatalogueEntryKey::try_new(format!("Type{byte:02}")).unwrap(),
        ))
    }

    fn chain1_pair(claim: u8, evidence: u8) -> RefVerifyPair {
        RefVerifyPair {
            claim: format!("claim-{claim}"),
            evidence: format!("evidence-{evidence}"),
            claim_hash: test_hash(claim),
            evidence_hash: test_hash(evidence),
            cache_scope: RefVerifyCacheScope::SpecAdr,
            known_bad: false,
            claim_origin: spec_origin(claim),
            evidence_origin: adr_origin(evidence),
        }
    }

    fn chain2_pair(claim: u8, evidence: u8, layer: &str) -> RefVerifyPair {
        let layer_id = LayerId::try_new(layer.to_owned()).unwrap();
        RefVerifyPair {
            claim: format!("claim-{claim}"),
            evidence: format!("evidence-{evidence}"),
            claim_hash: test_hash(claim),
            evidence_hash: test_hash(evidence),
            cache_scope: RefVerifyCacheScope::CatalogueSpec { layer: layer_id },
            known_bad: false,
            claim_origin: catalogue_origin(claim, layer),
            evidence_origin: spec_origin(evidence),
        }
    }

    fn pass_cache_entry(claim: u8, evidence: u8) -> SemanticVerifyEntry {
        SemanticVerifyEntry::new(
            test_hash(claim),
            test_hash(evidence),
            SemanticVerdict::Pass { citation: EvidenceCitation::try_new("ok".to_owned()).unwrap() },
            spec_origin(claim),
            adr_origin(evidence),
        )
    }

    fn fail_cache_entry(claim: u8, evidence: u8, reason: &str) -> SemanticVerifyEntry {
        SemanticVerifyEntry::new(
            test_hash(claim),
            test_hash(evidence),
            SemanticVerdict::Fail { reason: reason.to_owned() },
            spec_origin(claim),
            adr_origin(evidence),
        )
    }

    /// Chain-2 pass cache entry: claim_origin is CatalogueEntry, evidence_origin is
    /// SpecElement — matching the origin shape produced by `chain2_pair`.
    fn chain2_pass_cache_entry(claim: u8, evidence: u8, layer: &str) -> SemanticVerifyEntry {
        SemanticVerifyEntry::new(
            test_hash(claim),
            test_hash(evidence),
            SemanticVerdict::Pass { citation: EvidenceCitation::try_new("ok".to_owned()).unwrap() },
            catalogue_origin(claim, layer),
            spec_origin(evidence),
        )
    }

    #[test]
    fn load_results_tddd_bindings_missing_rules_returns_empty() {
        // Absent architecture-rules.json → pre-TDDD repository state.
        // The helper must return Ok(vec![]) rather than an error so that
        // `sotp ref-verify results --chain all` can still show Chain1 output.
        let dir = tempfile::tempdir().unwrap();

        let bindings = load_results_tddd_bindings(dir.path()).unwrap();

        assert!(bindings.is_empty(), "absent rules file should yield zero bindings");
    }

    #[test]
    fn load_results_tddd_bindings_malformed_rules_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("architecture-rules.json"), "{not json").unwrap();

        let err = load_results_tddd_bindings(dir.path()).unwrap_err();

        assert!(matches!(err, RefVerifyDriverError::Wiring(_)));
        assert!(err.to_string().contains("cannot load TDDD layer bindings for results"));
    }

    #[test]
    fn resolve_results_chain2_target_layers_all_invalid_layer_returns_wiring_error() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("architecture-rules.json"),
            r#"{
              "layers": [
                {
                  "crate": "1domain",
                  "tddd": {
                    "enabled": true,
                    "catalogue_file": "1domain-types.json"
                  }
                }
              ]
            }"#,
        )
        .unwrap();
        let bindings = load_results_tddd_bindings(dir.path()).unwrap();

        let err = resolve_results_chain2_target_layers(&bindings, &RefVerifyLayerFilter::All)
            .unwrap_err();

        assert!(
            matches!(err, RefVerifyDriverError::Wiring(ref msg)
                if msg.contains("invalid TDDD layer '1domain'")),
            "expected invalid layer Wiring error, got: {err:?}"
        );
    }

    fn domain_chain2_caches()
    -> Vec<(LayerId, Vec<domain::tddd::semantic_verify::SemanticVerifyEntry>)> {
        vec![(LayerId::try_new("domain".to_owned()).unwrap(), vec![])]
    }

    fn domain_usecase_chain2_caches()
    -> Vec<(LayerId, Vec<domain::tddd::semantic_verify::SemanticVerifyEntry>)> {
        vec![
            (LayerId::try_new("domain".to_owned()).unwrap(), vec![]),
            (LayerId::try_new("usecase".to_owned()).unwrap(), vec![]),
        ]
    }

    #[test]
    fn compute_results_empty_pairs_returns_chain1_zero_lane() {
        // chain=All with no resolved Chain2 layers (chain2_caches empty):
        // Chain1 lane is always pre-initialized when include_chain1=true, so it
        // appears in lane_summaries with zero counts even when there are no pairs.
        let out = compute_results(
            vec![],
            vec![],
            vec![],
            RefVerifyChainFilter::All,
            RefVerifyLayerFilter::All,
            RefVerifyVerdictFilter::All,
        )
        .unwrap();
        assert_eq!(out.lane_summaries.len(), 1, "Chain1 lane must appear even with zero pairs");
        assert_eq!(out.lane_summaries[0].label, "Chain1 (spec\u{2194}ADR)");
        assert_eq!(out.lane_summaries[0].pass_count, 0);
        assert_eq!(out.lane_summaries[0].fail_count, 0);
        assert_eq!(out.lane_summaries[0].pending_count, 0);
        assert!(out.pair_records.is_empty());
        assert_eq!(out.total_pass, 0);
        assert_eq!(out.total_fail, 0);
        assert_eq!(out.total_pending, 0);
    }

    #[test]
    fn compute_results_no_cache_returns_all_pending() {
        let out = compute_results(
            vec![],
            vec![],
            vec![chain1_pair(0x01, 0x02)],
            RefVerifyChainFilter::All,
            RefVerifyLayerFilter::All,
            RefVerifyVerdictFilter::All,
        )
        .unwrap();
        assert_eq!(out.lane_summaries.len(), 1);
        assert_eq!(out.lane_summaries[0].pending_count, 1);
        assert_eq!(out.lane_summaries[0].pass_count, 0);
        assert_eq!(out.lane_summaries[0].fail_count, 0);
        assert_eq!(out.total_pending, 1);
        assert_eq!(out.pair_records.len(), 1);
        assert!(matches!(out.pair_records[0].verdict, SemanticVerdict::Pending));
    }

    #[test]
    fn compute_results_pass_cache_entry_returns_pass_lane_summary() {
        let out = compute_results(
            vec![pass_cache_entry(0x01, 0x02)],
            vec![],
            vec![chain1_pair(0x01, 0x02)],
            RefVerifyChainFilter::All,
            RefVerifyLayerFilter::All,
            RefVerifyVerdictFilter::All,
        )
        .unwrap();
        assert_eq!(out.total_pass, 1);
        assert_eq!(out.total_fail, 0);
        assert_eq!(out.total_pending, 0);
        assert_eq!(out.lane_summaries[0].pass_count, 1);
        assert_eq!(out.pair_records.len(), 1);
        assert!(matches!(out.pair_records[0].verdict, SemanticVerdict::Pass { .. }));
    }

    #[test]
    fn compute_results_layer_filter_narrows_chain2_output() {
        let layer_id = LayerId::try_new("domain".to_owned()).unwrap();
        let out = compute_results(
            vec![],
            domain_usecase_chain2_caches(),
            vec![chain2_pair(0x01, 0x02, "domain"), chain2_pair(0x03, 0x04, "usecase")],
            RefVerifyChainFilter::Chain2,
            RefVerifyLayerFilter::Specific(layer_id),
            RefVerifyVerdictFilter::All,
        )
        .unwrap();
        assert_eq!(out.lane_summaries.len(), 1, "only domain lane should be included");
        assert_eq!(out.lane_summaries[0].label, "Chain2:domain");
        assert_eq!(out.total_pending, 1);
        assert_eq!(out.pair_records.len(), 1);
        assert_eq!(out.pair_records[0].chain_layer, "Chain2:domain");
    }

    #[test]
    fn compute_results_chain1_filter_excludes_chain2() {
        let out = compute_results(
            vec![],
            vec![],
            vec![chain1_pair(0x01, 0x02), chain2_pair(0x03, 0x04, "domain")],
            RefVerifyChainFilter::Chain1,
            RefVerifyLayerFilter::All,
            RefVerifyVerdictFilter::All,
        )
        .unwrap();
        assert_eq!(out.lane_summaries.len(), 1);
        assert_eq!(out.lane_summaries[0].label, "Chain1 (spec\u{2194}ADR)");
        assert_eq!(out.total_pending, 1);
        assert_eq!(out.pair_records.len(), 1);
    }

    #[test]
    fn compute_results_chain2_filter_excludes_chain1() {
        let out = compute_results(
            vec![],
            domain_chain2_caches(),
            vec![chain1_pair(0x01, 0x02), chain2_pair(0x03, 0x04, "domain")],
            RefVerifyChainFilter::Chain2,
            RefVerifyLayerFilter::All,
            RefVerifyVerdictFilter::All,
        )
        .unwrap();
        assert_eq!(out.lane_summaries.len(), 1);
        assert!(out.lane_summaries[0].label.starts_with("Chain2:"));
        assert_eq!(out.total_pending, 1);
        assert_eq!(out.pair_records.len(), 1);
    }

    #[test]
    fn compute_results_verdict_filter_fail_pending_excludes_pass() {
        let out = compute_results(
            vec![pass_cache_entry(0x01, 0x02)],
            vec![],
            vec![chain1_pair(0x01, 0x02), chain1_pair(0x03, 0x04)],
            RefVerifyChainFilter::All,
            RefVerifyLayerFilter::All,
            RefVerifyVerdictFilter::FailPending,
        )
        .unwrap();
        // Lane summary covers all verdicts.
        assert_eq!(out.lane_summaries[0].pass_count, 1);
        assert_eq!(out.lane_summaries[0].pending_count, 1);
        assert_eq!(out.total_pass, 1);
        assert_eq!(out.total_pending, 1);
        // pair_records only includes the pending pair.
        assert_eq!(out.pair_records.len(), 1);
        assert!(matches!(out.pair_records[0].verdict, SemanticVerdict::Pending));
    }

    #[test]
    fn compute_results_verdict_filter_pass_includes_only_pass() {
        let out = compute_results(
            vec![pass_cache_entry(0x01, 0x02)],
            vec![],
            vec![chain1_pair(0x01, 0x02), chain1_pair(0x03, 0x04)],
            RefVerifyChainFilter::All,
            RefVerifyLayerFilter::All,
            RefVerifyVerdictFilter::Pass,
        )
        .unwrap();
        // Lane summary still shows all counts.
        assert_eq!(out.lane_summaries[0].pass_count, 1);
        assert_eq!(out.lane_summaries[0].pending_count, 1);
        // pair_records only includes the pass pair.
        assert_eq!(out.pair_records.len(), 1);
        assert!(matches!(out.pair_records[0].verdict, SemanticVerdict::Pass { .. }));
    }

    #[test]
    fn compute_results_verdict_filter_fail_includes_only_fail() {
        let out = compute_results(
            vec![fail_cache_entry(0x01, 0x02, "mismatch")],
            vec![],
            vec![chain1_pair(0x01, 0x02), chain1_pair(0x03, 0x04)],
            RefVerifyChainFilter::All,
            RefVerifyLayerFilter::All,
            RefVerifyVerdictFilter::Fail,
        )
        .unwrap();
        assert_eq!(out.total_fail, 1);
        assert_eq!(out.total_pending, 1);
        assert_eq!(out.pair_records.len(), 1);
        assert!(matches!(out.pair_records[0].verdict, SemanticVerdict::Fail { .. }));
        assert_eq!(out.pair_records[0].reason, "mismatch");
    }

    #[test]
    fn compute_results_verdict_filter_pending_includes_only_pending() {
        let out = compute_results(
            vec![pass_cache_entry(0x01, 0x02)],
            vec![],
            vec![chain1_pair(0x01, 0x02), chain1_pair(0x03, 0x04)],
            RefVerifyChainFilter::All,
            RefVerifyLayerFilter::All,
            RefVerifyVerdictFilter::Pending,
        )
        .unwrap();
        assert_eq!(out.pair_records.len(), 1);
        assert!(matches!(out.pair_records[0].verdict, SemanticVerdict::Pending));
    }

    #[test]
    fn compute_results_verdict_filter_all_returns_all_records() {
        let out = compute_results(
            vec![pass_cache_entry(0x01, 0x02), fail_cache_entry(0x03, 0x04, "fail reason")],
            vec![],
            vec![chain1_pair(0x01, 0x02), chain1_pair(0x03, 0x04), chain1_pair(0x05, 0x06)],
            RefVerifyChainFilter::All,
            RefVerifyLayerFilter::All,
            RefVerifyVerdictFilter::All,
        )
        .unwrap();
        assert_eq!(out.pair_records.len(), 3);
        assert_eq!(out.total_pass, 1);
        assert_eq!(out.total_fail, 1);
        assert_eq!(out.total_pending, 1);
    }

    #[test]
    fn compute_results_lane_summary_totals_unaffected_by_verdict_filter() {
        // verdict_filter = Pass but lane summary still shows all pending counts.
        let out = compute_results(
            vec![],
            vec![],
            vec![chain1_pair(0x01, 0x02), chain1_pair(0x03, 0x04)],
            RefVerifyChainFilter::All,
            RefVerifyLayerFilter::All,
            RefVerifyVerdictFilter::Pass,
        )
        .unwrap();
        assert_eq!(out.lane_summaries[0].pending_count, 2);
        assert_eq!(out.total_pending, 2);
        assert!(out.pair_records.is_empty(), "no pass pairs → records empty");
    }

    // ── layer filter validation ───────────────────────────────────────────────

    #[test]
    fn compute_results_specific_unknown_layer_with_chain2_returns_wiring_error() {
        // Typo "domian" is not in chain2_caches → Wiring error with Chain2 filter.
        let bad_layer = LayerId::try_new("domian".to_owned()).unwrap();
        let err = compute_results(
            vec![],
            domain_chain2_caches(),
            vec![chain2_pair(0x01, 0x02, "domain")],
            RefVerifyChainFilter::Chain2,
            RefVerifyLayerFilter::Specific(bad_layer),
            RefVerifyVerdictFilter::All,
        )
        .unwrap_err();
        assert!(
            matches!(err, usecase::ref_verify::RefVerifyDriverError::Wiring(ref msg)
                if msg.contains("domian") && msg.contains("domain")),
            "expected Wiring error naming the bad layer and valid set, got: {err:?}"
        );
    }

    #[test]
    fn compute_results_specific_unknown_layer_with_all_chain_returns_wiring_error() {
        // Typo "domian" is not in chain2_caches → Wiring error with All filter.
        let bad_layer = LayerId::try_new("domian".to_owned()).unwrap();
        let err = compute_results(
            vec![pass_cache_entry(0x01, 0x02)],
            domain_chain2_caches(),
            vec![chain1_pair(0x01, 0x02), chain2_pair(0x03, 0x04, "domain")],
            RefVerifyChainFilter::All,
            RefVerifyLayerFilter::Specific(bad_layer),
            RefVerifyVerdictFilter::All,
        )
        .unwrap_err();
        assert!(
            matches!(err, usecase::ref_verify::RefVerifyDriverError::Wiring(ref msg)
                if msg.contains("domian")),
            "expected Wiring error for unknown layer under All chain filter, got: {err:?}"
        );
    }

    #[test]
    fn compute_results_specific_valid_layer_with_chain2_succeeds() {
        // A valid layer name in chain2_caches passes validation.
        let valid_layer = LayerId::try_new("domain".to_owned()).unwrap();
        let out = compute_results(
            vec![],
            domain_chain2_caches(),
            vec![chain2_pair(0x01, 0x02, "domain")],
            RefVerifyChainFilter::Chain2,
            RefVerifyLayerFilter::Specific(valid_layer),
            RefVerifyVerdictFilter::All,
        )
        .unwrap();
        assert_eq!(out.total_pending, 1);
    }

    #[test]
    fn compute_results_specific_layer_with_chain1_filter_is_noop() {
        // Chain1 filter: --layer is ignored (no chain2 involved).
        // Even a nonexistent layer name must not produce an error.
        let unknown_layer = LayerId::try_new("domian".to_owned()).unwrap();
        let out = compute_results(
            vec![pass_cache_entry(0x01, 0x02)],
            domain_chain2_caches(),
            vec![chain1_pair(0x01, 0x02)],
            RefVerifyChainFilter::Chain1,
            RefVerifyLayerFilter::Specific(unknown_layer),
            RefVerifyVerdictFilter::All,
        )
        .unwrap();
        // Chain1 lane is included; the bad layer filter has no effect on Chain1.
        assert_eq!(out.total_pass, 1);
        assert_eq!(out.lane_summaries.len(), 1);
        assert_eq!(out.lane_summaries[0].label, "Chain1 (spec\u{2194}ADR)");
    }

    // ── chain=Chain1 short-circuit (P1 finding: Chain2 load skipped for Chain1-only) ─────

    #[test]
    fn compute_results_chain1_only_skips_chain2_bindings() {
        // Structural stand-in for "Chain2 binding load failure":
        // chain2_caches is deliberately empty, simulating the short-circuit in
        // driver_adapter.rs where load_results_tddd_bindings + Chain2 cache loading are
        // skipped entirely when chain=Chain1.  Chain1 data is healthy.
        // Assert: succeeds and returns only the Chain1 lane with correct verdicts.
        let out = compute_results(
            vec![pass_cache_entry(0x01, 0x02)],
            vec![], // empty: simulates skipped Chain2 loading
            vec![chain1_pair(0x01, 0x02)],
            RefVerifyChainFilter::Chain1,
            RefVerifyLayerFilter::All,
            RefVerifyVerdictFilter::All,
        )
        .unwrap();
        assert_eq!(out.lane_summaries.len(), 1, "only Chain1 lane should be present");
        assert_eq!(out.lane_summaries[0].label, "Chain1 (spec\u{2194}ADR)");
        assert_eq!(out.total_pass, 1);
        assert_eq!(out.total_fail, 0);
        assert_eq!(out.total_pending, 0);
        assert_eq!(out.pair_records.len(), 1);
        assert!(matches!(out.pair_records[0].verdict, SemanticVerdict::Pass { .. }));
    }

    // ── F1 / F2 / F3 unit tests ───────────────────────────────────────────────

    /// F1: chain=Chain2 with an absent Chain1 cache succeeds.
    /// Simulates the F1 fix: `load_entries(SpecAdr)` is skipped when chain=Chain2.
    /// An empty `chain1_cache` (as the adapter passes) must not degrade Chain2 output.
    #[test]
    fn compute_results_chain2_absent_chain1_cache_succeeds() {
        let layer_id = LayerId::try_new("domain".to_owned()).unwrap();
        let out = compute_results(
            vec![], // chain1 cache absent — not loaded for chain=Chain2 (F1 fix)
            vec![(layer_id.clone(), vec![chain2_pass_cache_entry(0x01, 0x02, "domain")])],
            vec![chain2_pair(0x01, 0x02, "domain")],
            RefVerifyChainFilter::Chain2,
            RefVerifyLayerFilter::All,
            RefVerifyVerdictFilter::All,
        )
        .unwrap();
        assert_eq!(out.total_pass, 1);
        assert_eq!(out.lane_summaries.len(), 1, "only Chain2 lane expected");
        assert!(out.lane_summaries[0].label.starts_with("Chain2:"));
        assert!(
            !out.lane_summaries.iter().any(|s| s.label.contains("Chain1")),
            "no Chain1 lane should appear"
        );
    }

    /// F2: chain=Chain2 + layer=Specific(domain) + only domain cache loaded succeeds.
    /// Simulates the F2 fix: when layer=Specific(domain), only domain's cache is loaded;
    /// a corrupt or absent usecase cache must not fail a domain-only results query.
    #[test]
    fn compute_results_chain2_specific_layer_absent_other_layer_cache_succeeds() {
        let domain_id = LayerId::try_new("domain".to_owned()).unwrap();
        // chain2_caches has only domain (simulating F2 fix: usecase cache not loaded).
        let out = compute_results(
            vec![],
            vec![(domain_id.clone(), vec![chain2_pass_cache_entry(0x01, 0x02, "domain")])],
            vec![chain2_pair(0x01, 0x02, "domain")],
            RefVerifyChainFilter::Chain2,
            RefVerifyLayerFilter::Specific(domain_id),
            RefVerifyVerdictFilter::All,
        )
        .unwrap();
        assert_eq!(out.total_pass, 1);
        assert_eq!(out.lane_summaries.len(), 1, "only domain lane expected");
        assert_eq!(out.lane_summaries[0].label, "Chain2:domain");
    }

    /// F3: chain=Chain2 + only Chain2 pairs from narrowed pair source → correct results.
    /// Simulates the F3 fix: pair source uses `Chain2 { layer }` scope, so `current_pairs`
    /// contains no Chain1 pairs even when Chain1 artifacts exist on disk.
    #[test]
    fn compute_results_chain2_with_chain2_only_pairs_excludes_chain1_output() {
        let layer_id = LayerId::try_new("domain".to_owned()).unwrap();
        // current_pairs contains only Chain2 pairs (F3: pair source used Chain2 scope).
        let out = compute_results(
            vec![], // chain1 cache empty (F1: not loaded)
            vec![(layer_id.clone(), vec![chain2_pass_cache_entry(0x01, 0x02, "domain")])],
            vec![chain2_pair(0x01, 0x02, "domain")], // only Chain2 pairs (F3 invariant)
            RefVerifyChainFilter::Chain2,
            RefVerifyLayerFilter::All,
            RefVerifyVerdictFilter::All,
        )
        .unwrap();
        assert_eq!(out.total_pass, 1);
        assert_eq!(out.total_fail, 0);
        assert_eq!(out.total_pending, 0);
        assert_eq!(out.lane_summaries.len(), 1);
        assert!(out.lane_summaries[0].label.starts_with("Chain2:"));
        assert_eq!(out.pair_records.len(), 1);
    }

    #[test]
    fn compute_results_chain1_only_layer_specified_is_noop() {
        // chain=Chain1 + Specific(unknown layer) + empty chain2_caches (simulating the
        // short-circuit where Chain2 loading was skipped): no validation error is raised
        // (layer validation is Chain2-only), and Chain1 output is unaffected.
        let unknown_layer = LayerId::try_new("nonexistent-layer".to_owned()).unwrap();
        let out = compute_results(
            vec![pass_cache_entry(0x01, 0x02)],
            vec![], // empty: simulates skipped Chain2 loading
            vec![chain1_pair(0x01, 0x02)],
            RefVerifyChainFilter::Chain1,
            RefVerifyLayerFilter::Specific(unknown_layer),
            RefVerifyVerdictFilter::All,
        )
        .unwrap();
        assert_eq!(out.total_pass, 1);
        assert_eq!(out.lane_summaries.len(), 1);
        assert_eq!(out.lane_summaries[0].label, "Chain1 (spec\u{2194}ADR)");
        assert_eq!(out.pair_records.len(), 1);
    }

    /// Verifies that `compute_results` returns success with zero output when
    /// pre-Phase-2 state causes the adapter to produce empty caches and pairs
    /// for Chain2 (absent-catalogue layers were skipped by the F1 fix).
    #[test]
    fn compute_results_chain2_all_with_pre_phase2_state_returns_zero_count_lane() {
        // Pre-Phase-2: catalogue absent → zero cache entries → zero pairs.
        // The domain layer is still in chain2_caches (the caller records it so that
        // compute_results can distinguish pre-Phase-2 from an unknown-layer typo), and
        // must appear in lane_summaries with zero counts (AC-02 / AC-07).
        let layer_id = LayerId::try_new("domain".to_owned()).unwrap();
        let out = compute_results(
            vec![],                   // no chain1 cache
            vec![(layer_id, vec![])], // absent catalogue contributes zero cache entries
            vec![],                   // no pairs enumerated for absent catalogues
            RefVerifyChainFilter::Chain2,
            RefVerifyLayerFilter::All,
            RefVerifyVerdictFilter::All,
        )
        .unwrap();
        assert_eq!(
            out.lane_summaries.len(),
            1,
            "domain lane must appear even when catalogue is absent"
        );
        assert_eq!(out.lane_summaries[0].label, "Chain2:domain");
        assert_eq!(out.lane_summaries[0].pass_count, 0);
        assert_eq!(out.lane_summaries[0].fail_count, 0);
        assert_eq!(out.lane_summaries[0].pending_count, 0);
        assert!(out.pair_records.is_empty());
        assert_eq!(out.total_pass, 0);
        assert_eq!(out.total_fail, 0);
        assert_eq!(out.total_pending, 0);
    }

    /// Verifies that a specific valid layer with an absent catalogue is still
    /// accepted as a pre-Phase-2 zero-pair state and produces a zero-count lane,
    /// not misclassified as an unknown-layer typo.
    #[test]
    fn compute_results_chain2_specific_with_pre_phase2_state_returns_zero_count_lane() {
        // Pre-Phase-2 with layer=Specific: catalogue absent → zero pairs, but the
        // single requested lane must still appear in lane_summaries with zero counts.
        let layer_id = LayerId::try_new("domain".to_owned()).unwrap();
        let out = compute_results(
            vec![],
            vec![(layer_id.clone(), vec![])],
            vec![],
            RefVerifyChainFilter::Chain2,
            RefVerifyLayerFilter::Specific(layer_id),
            RefVerifyVerdictFilter::All,
        )
        .unwrap();
        assert_eq!(out.lane_summaries.len(), 1, "domain lane must appear even with zero pairs");
        assert_eq!(out.lane_summaries[0].label, "Chain2:domain");
        assert_eq!(out.lane_summaries[0].pass_count, 0);
        assert_eq!(out.lane_summaries[0].fail_count, 0);
        assert_eq!(out.lane_summaries[0].pending_count, 0);
        assert!(out.pair_records.is_empty());
        assert_eq!(out.total_pass, 0);
        assert_eq!(out.total_fail, 0);
        assert_eq!(out.total_pending, 0);
    }

    // ── zero-pair lane invariant (AC-02 / AC-07 / T004) ─────────────────────────

    /// Chain2 layer whose catalogue is present but yields zero current_pairs must
    /// still appear in lane_summaries with zero counts (catalogue has no spec_refs
    /// this cycle, but the lane is declared and must be shown).
    #[test]
    fn compute_results_chain2_layer_with_zero_pairs_keeps_lane_in_summary() {
        let layer_id = LayerId::try_new("domain".to_owned()).unwrap();
        let out = compute_results(
            vec![],
            vec![(layer_id, vec![pass_cache_entry(0x01, 0x02)])], // catalogue present with cache
            vec![],                                               // no current_pairs this cycle
            RefVerifyChainFilter::Chain2,
            RefVerifyLayerFilter::All,
            RefVerifyVerdictFilter::All,
        )
        .unwrap();
        assert_eq!(
            out.lane_summaries.len(),
            1,
            "domain lane must appear even with zero current_pairs"
        );
        assert_eq!(out.lane_summaries[0].label, "Chain2:domain");
        assert_eq!(out.lane_summaries[0].pass_count, 0);
        assert_eq!(out.lane_summaries[0].fail_count, 0);
        assert_eq!(out.lane_summaries[0].pending_count, 0);
        assert!(out.pair_records.is_empty());
        assert_eq!(out.total_pass, 0);
        assert_eq!(out.total_fail, 0);
        assert_eq!(out.total_pending, 0);
    }

    /// chain=All with no resolved Chain2 layers and no current_pairs: the Chain1
    /// lane must appear with zero counts because include_chain1 is true.
    #[test]
    fn compute_results_chain_all_chain1_zero_pairs_keeps_lane_in_summary() {
        let out = compute_results(
            vec![pass_cache_entry(0x01, 0x02)], // chain1 cache has an entry
            vec![],                             // no Chain2 layers
            vec![],                             // zero current_pairs
            RefVerifyChainFilter::All,
            RefVerifyLayerFilter::All,
            RefVerifyVerdictFilter::All,
        )
        .unwrap();
        assert_eq!(
            out.lane_summaries.len(),
            1,
            "Chain1 lane must appear even with zero current_pairs"
        );
        assert_eq!(out.lane_summaries[0].label, "Chain1 (spec\u{2194}ADR)");
        assert_eq!(out.lane_summaries[0].pass_count, 0);
        assert_eq!(out.lane_summaries[0].fail_count, 0);
        assert_eq!(out.lane_summaries[0].pending_count, 0);
        assert!(out.pair_records.is_empty());
        assert_eq!(out.total_pass, 0);
        assert_eq!(out.total_fail, 0);
        assert_eq!(out.total_pending, 0);
    }

    /// --chain 2 --layer domain with zero domain-layer pairs: exactly one lane for
    /// the selected layer must appear in lane_summaries with zero counts.
    #[test]
    fn compute_results_chain2_specific_zero_pairs_keeps_single_lane() {
        let domain_id = LayerId::try_new("domain".to_owned()).unwrap();
        let out = compute_results(
            vec![],
            vec![
                (LayerId::try_new("domain".to_owned()).unwrap(), vec![]),
                (LayerId::try_new("usecase".to_owned()).unwrap(), vec![]),
            ],
            vec![], // zero pairs for domain layer
            RefVerifyChainFilter::Chain2,
            RefVerifyLayerFilter::Specific(domain_id),
            RefVerifyVerdictFilter::All,
        )
        .unwrap();
        assert_eq!(out.lane_summaries.len(), 1, "exactly one lane for the specific layer");
        assert_eq!(out.lane_summaries[0].label, "Chain2:domain");
        assert_eq!(out.lane_summaries[0].pass_count, 0);
        assert_eq!(out.lane_summaries[0].fail_count, 0);
        assert_eq!(out.lane_summaries[0].pending_count, 0);
        assert!(out.pair_records.is_empty());
        assert_eq!(out.total_pass, 0);
        assert_eq!(out.total_fail, 0);
        assert_eq!(out.total_pending, 0);
    }

    // ── pre-TDDD / absent architecture-rules.json regression tests ──────────────

    /// Verifies that `compute_results` with `chain=All` succeeds when
    /// `architecture-rules.json` is absent (pre-TDDD repository).
    ///
    /// In this state `load_results_tddd_bindings` returns `Ok(vec![])` (the
    /// NotFound fix), so `chain2_caches` is empty and no Chain2 pairs are
    /// enumerated.  The command must show a Chain1 lane (from spec ↔ ADR pairs)
    /// and produce no Chain2 lanes.
    #[test]
    fn compute_results_chain_all_with_absent_architecture_rules_succeeds_with_empty_chain2() {
        // Simulate: load_results_tddd_bindings returned Ok(vec![]) because
        // architecture-rules.json was not found.
        // There is one Chain1 pair from spec.json ↔ an ADR decision.
        let out = compute_results(
            vec![],                        // no Chain1 cache (not yet verified)
            vec![],                        // chain2_caches: empty (absent rules)
            vec![chain1_pair(0x01, 0x02)], // one Chain1 pair
            RefVerifyChainFilter::All,
            RefVerifyLayerFilter::All,
            RefVerifyVerdictFilter::FailPending,
        )
        .unwrap();

        // Chain1 lane is present.
        assert_eq!(out.lane_summaries.len(), 1, "only Chain1 lane expected");
        assert_eq!(out.lane_summaries[0].label, "Chain1 (spec\u{2194}ADR)");

        // No Chain2 lanes.
        assert!(
            !out.lane_summaries.iter().any(|s| s.label.starts_with("Chain2:")),
            "no Chain2 lane should appear when architecture-rules.json is absent"
        );

        // The single pair is pending (cache is empty).
        assert_eq!(out.total_pending, 1);
        assert_eq!(out.total_pass, 0);
        assert_eq!(out.total_fail, 0);

        // FailPending verdict filter includes pending pairs.
        assert_eq!(out.pair_records.len(), 1);
        assert!(matches!(out.pair_records[0].verdict, SemanticVerdict::Pending));
    }

    // ── F1 regression: per-layer origin preserved for same-hash pairs ────────────

    /// Regression test for F1: when two Chain2 layers have pairs with identical
    /// `(claim_hash, evidence_hash)` (e.g. the same spec element referenced from
    /// both layers) but different `claim_origin` values (each pointing to their own
    /// catalogue file), both pending records must carry their own layer-correct
    /// origin — not the first layer's origin.
    ///
    /// Before the fix, `origin_lookup` was keyed only by hash, so the second
    /// layer's pending record silently received the first layer's `claim_origin`.
    #[test]
    fn compute_results_pending_origin_preserved_per_layer_with_same_hash() {
        let domain_id = LayerId::try_new("domain".to_owned()).unwrap();
        let usecase_id = LayerId::try_new("usecase".to_owned()).unwrap();

        // Both pairs share the same hashes — simulating catalogue JSON that produces
        // the same content hash in two layers for the same spec element.
        let shared_claim_hash = test_hash(0x01);
        let shared_evidence_hash = test_hash(0x02);

        let domain_pair = RefVerifyPair {
            claim: "claim-domain".to_owned(),
            evidence: "evidence-shared".to_owned(),
            claim_hash: shared_claim_hash.clone(),
            evidence_hash: shared_evidence_hash.clone(),
            cache_scope: RefVerifyCacheScope::CatalogueSpec { layer: domain_id.clone() },
            known_bad: false,
            claim_origin: catalogue_origin(0x01, "domain"),
            evidence_origin: spec_origin(0x02),
        };
        let usecase_pair = RefVerifyPair {
            claim: "claim-usecase".to_owned(),
            evidence: "evidence-shared".to_owned(),
            claim_hash: shared_claim_hash,
            evidence_hash: shared_evidence_hash,
            cache_scope: RefVerifyCacheScope::CatalogueSpec { layer: usecase_id.clone() },
            known_bad: false,
            claim_origin: catalogue_origin(0x01, "usecase"),
            evidence_origin: spec_origin(0x02),
        };

        // No cache entries for either layer → both pairs are cache-miss → Pending.
        let out = compute_results(
            vec![],
            vec![(domain_id, vec![]), (usecase_id, vec![])],
            vec![domain_pair, usecase_pair],
            RefVerifyChainFilter::Chain2,
            RefVerifyLayerFilter::All,
            RefVerifyVerdictFilter::All,
        )
        .unwrap();

        assert_eq!(out.pair_records.len(), 2, "both pairs should be present");
        assert_eq!(out.total_pending, 2, "both pairs should be Pending (cache miss)");

        let domain_record =
            out.pair_records.iter().find(|r| r.chain_layer == "Chain2:domain").unwrap();
        let usecase_record =
            out.pair_records.iter().find(|r| r.chain_layer == "Chain2:usecase").unwrap();

        // Each pending record must carry its own layer's catalogue origin.
        assert_eq!(
            domain_record.claim_origin,
            catalogue_origin(0x01, "domain"),
            "domain pending record must carry domain catalogue origin"
        );
        assert_eq!(
            usecase_record.claim_origin,
            catalogue_origin(0x01, "usecase"),
            "usecase pending record must carry usecase catalogue origin (not domain's)"
        );
    }

    /// Verifies that `compute_results` with `chain=Chain2` also succeeds when
    /// `architecture-rules.json` is absent (pre-TDDD repository).
    ///
    /// In this state `chain2_caches` is empty and no pairs are enumerated, so
    /// the result is an empty output — not an error.
    #[test]
    fn compute_results_chain2_with_absent_architecture_rules_succeeds_with_empty_chain2() {
        // Simulate: load_results_tddd_bindings returned Ok(vec![]) and
        // chain2_layer_ids / chain2_caches are both empty.
        let out = compute_results(
            vec![], // chain1 cache not loaded (chain=Chain2 short-circuit)
            vec![], // chain2_caches: empty (absent architecture-rules.json)
            vec![], // no pairs enumerated (no TDDD layers)
            RefVerifyChainFilter::Chain2,
            RefVerifyLayerFilter::All,
            RefVerifyVerdictFilter::FailPending,
        )
        .unwrap();

        assert!(out.lane_summaries.is_empty(), "no lanes when architecture-rules.json is absent");
        assert!(out.pair_records.is_empty());
        assert_eq!(out.total_pass, 0);
        assert_eq!(out.total_fail, 0);
        assert_eq!(out.total_pending, 0);
    }

    // ── Helpers shared by scope-resolution and pre-flight tests ──────────────

    fn write_tddd_rules(project_root: &std::path::Path) {
        std::fs::write(
            project_root.join("architecture-rules.json"),
            r#"{
              "layers": [
                {
                  "crate": "domain",
                  "tddd": {
                    "enabled": true,
                    "catalogue_file": "domain-types.json"
                  }
                },
                {
                  "crate": "usecase",
                  "tddd": {
                    "enabled": true,
                    "catalogue_file": "usecase-types.json"
                  }
                }
              ]
            }"#,
        )
        .unwrap();
    }

    // ── F2: partial-set catalogue detection ───────────────────────────────────

    /// 2 layers declared, one catalogue absent / one present → Wiring error.
    #[test]
    fn check_partial_catalogue_set_partial_returns_wiring_error() {
        let domain_id = LayerId::try_new("domain".to_owned()).unwrap();
        let usecase_id = LayerId::try_new("usecase".to_owned()).unwrap();
        // Simulate: domain present, usecase absent.
        let present = [domain_id];
        let absent = [usecase_id];
        let err = check_partial_catalogue_set(&present, &absent).unwrap_err();
        assert!(
            matches!(err, RefVerifyDriverError::Wiring(ref msg) if msg.contains("usecase")),
            "expected Wiring error naming the absent layer, got: {err:?}"
        );
    }

    /// All target catalogues absent → pre-Phase-2 valid (success, no error).
    #[test]
    fn check_partial_catalogue_set_all_absent_succeeds() {
        let domain_id = LayerId::try_new("domain".to_owned()).unwrap();
        let usecase_id = LayerId::try_new("usecase".to_owned()).unwrap();
        let present: [LayerId; 0] = [];
        let absent = [domain_id, usecase_id];
        check_partial_catalogue_set(&present, &absent).unwrap();
    }

    /// Single declared target catalogue present → success.
    #[test]
    fn check_partial_catalogue_set_all_present_succeeds() {
        let domain_id = LayerId::try_new("domain".to_owned()).unwrap();
        let present = [domain_id];
        let absent: [LayerId; 0] = [];
        check_partial_catalogue_set(&present, &absent).unwrap();
    }

    /// Specific-layer query must still fail closed when the full declared
    /// Chain2 catalogue set is partial.
    #[test]
    fn inspect_chain2_catalogue_set_partial_returns_wiring_error() {
        let dir = tempfile::tempdir().unwrap();
        let track_id = "test-track";
        let track_dir = dir.path().join("track").join("items").join(track_id);
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(track_dir.join("usecase-types.json"), "{}").unwrap();
        let bindings = crate::verify::tddd_layers::parse_tddd_layers(
            r#"{
              "layers": [
                {
                  "crate": "domain",
                  "tddd": {
                    "enabled": true,
                    "catalogue_file": "domain-types.json"
                  }
                },
                {
                  "crate": "usecase",
                  "tddd": {
                    "enabled": true,
                    "catalogue_file": "usecase-types.json"
                  }
                }
              ]
            }"#,
        )
        .unwrap();

        let (present, absent) =
            inspect_chain2_catalogue_set(dir.path(), track_id, &bindings).unwrap();
        let present_layers: Vec<&str> = present.iter().map(|id| id.as_ref()).collect();
        let absent_layers: Vec<&str> = absent.iter().map(|id| id.as_ref()).collect();
        assert_eq!(present_layers, vec!["usecase"]);
        assert_eq!(absent_layers, vec!["domain"]);

        let err = check_partial_catalogue_set(&present, &absent).unwrap_err();
        assert!(
            matches!(err, RefVerifyDriverError::Wiring(ref msg) if msg.contains("domain")),
            "expected Wiring error naming the absent target layer, got: {err:?}"
        );
    }

    // ── F1: Chain1-only scope resolution bypasses Chain2 consistency ─────────

    /// F1: `chain=Chain1` against a partial Phase-2 catalogue set must succeed.
    ///
    /// `resolve_chain1_only_scope` must return `Chain1` when spec.json is present,
    /// regardless of whether Chain2 catalogues are partially (or fully) absent.
    #[test]
    fn resolve_chain1_only_scope_partial_chain2_catalogue_succeeds() {
        let dir = tempfile::tempdir().unwrap();
        let track_id = "T001";
        let track_dir = dir.path().join("track").join("items").join(track_id);
        write_tddd_rules(dir.path());
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(track_dir.join("spec.json"), "{}").unwrap();
        // Partial Chain2: only one catalogue present — full resolver would reject this.
        std::fs::write(track_dir.join("domain-types.json"), "{}").unwrap();
        // usecase-types.json intentionally absent.

        let scope = resolve_chain1_only_scope(dir.path(), track_id).unwrap();
        assert!(
            matches!(scope, usecase::ref_verify::RefVerifyScope::Chain1),
            "Chain1-only path must return Chain1 scope despite partial Chain2 set; got: {scope:?}"
        );
    }

    /// F1: absent spec.json is Phase-0 zero-pair state, so Chain1 scope still resolves.
    #[test]
    fn resolve_chain1_only_scope_absent_spec_resolves_chain1() {
        let dir = tempfile::tempdir().unwrap();
        let track_id = "T001";
        let track_dir = dir.path().join("track").join("items").join(track_id);
        write_tddd_rules(dir.path());
        std::fs::create_dir_all(&track_dir).unwrap();
        // No spec.json.

        let scope = resolve_chain1_only_scope(dir.path(), track_id).unwrap();
        assert!(
            matches!(scope, usecase::ref_verify::RefVerifyScope::Chain1),
            "Chain1-only path must preserve Phase-0 zero-pair state when spec.json is absent; got: {scope:?}"
        );
    }

    /// F1: absent spec.json is invalid when any declared Chain2 catalogue exists.
    #[test]
    fn resolve_chain1_only_scope_absent_spec_with_catalogue_returns_wiring_error() {
        let dir = tempfile::tempdir().unwrap();
        let track_id = "T001";
        let track_dir = dir.path().join("track").join("items").join(track_id);
        write_tddd_rules(dir.path());
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(track_dir.join("domain-types.json"), "{}").unwrap();
        // No spec.json.

        let err = resolve_chain1_only_scope(dir.path(), track_id).unwrap_err();
        assert!(
            matches!(&err, RefVerifyDriverError::Wiring(msg)
                if msg.contains("spec.json not found while TDDD catalogue")),
            "expected Wiring error for IN-06 violation, got: {err:?}"
        );
    }

    /// F1: missing architecture-rules.json is pre-TDDD zero-layer state.
    #[test]
    fn resolve_chain1_only_scope_missing_rules_resolves_chain1() {
        let dir = tempfile::tempdir().unwrap();
        let track_id = "T001";
        let track_dir = dir.path().join("track").join("items").join(track_id);
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(track_dir.join("spec.json"), "{}").unwrap();
        // No architecture-rules.json.

        let scope = resolve_chain1_only_scope(dir.path(), track_id).unwrap();
        assert!(
            matches!(scope, usecase::ref_verify::RefVerifyScope::Chain1),
            "Chain1-only path must preserve pre-TDDD zero-layer state when rules are absent; got: {scope:?}"
        );
    }

    /// F1: Chain1-only scope resolution still fails closed on malformed TDDD config.
    #[test]
    fn resolve_chain1_only_scope_malformed_rules_returns_wiring_error() {
        let dir = tempfile::tempdir().unwrap();
        let track_id = "T001";
        let track_dir = dir.path().join("track").join("items").join(track_id);
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(track_dir.join("spec.json"), "{}").unwrap();
        std::fs::write(dir.path().join("architecture-rules.json"), "not json").unwrap();

        let err = resolve_chain1_only_scope(dir.path(), track_id).unwrap_err();
        assert!(
            matches!(&err, RefVerifyDriverError::Wiring(msg)
                if msg.contains("cannot load TDDD layer bindings for Chain1 scope")
                    && msg.contains("not valid JSON")),
            "expected Wiring error for malformed TDDD config, got: {err:?}"
        );
    }

    // ── F2: track directory pre-check ─────────────────────────────────────────

    /// F2: An invalid track_id (no track directory) must return a typed Wiring
    /// error, not a silent zero-pair result.
    #[test]
    fn check_track_dir_exists_missing_directory_returns_wiring_error() {
        let dir = tempfile::tempdir().unwrap();
        // No track/items/nonexistent-track/ directory created.
        let err = check_track_dir_exists(dir.path(), "nonexistent-track").unwrap_err();
        assert!(
            matches!(&err, RefVerifyDriverError::Wiring(msg) if msg.contains("track directory not found")),
            "expected Wiring error for missing track directory, got: {err:?}"
        );
    }

    /// F2 counterpart: a valid track directory returns Ok(()).
    #[test]
    fn check_track_dir_exists_valid_directory_returns_ok() {
        let dir = tempfile::tempdir().unwrap();
        let track_id = "T001";
        let track_dir = dir.path().join("track").join("items").join(track_id);
        std::fs::create_dir_all(&track_dir).unwrap();

        check_track_dir_exists(dir.path(), track_id).unwrap();
    }

    /// F2: a regular file at the track slot is not a valid track directory.
    #[test]
    fn check_track_dir_exists_regular_file_returns_wiring_error() {
        let dir = tempfile::tempdir().unwrap();
        let track_id = "T001";
        let items_dir = dir.path().join("track").join("items");
        std::fs::create_dir_all(&items_dir).unwrap();
        std::fs::write(items_dir.join(track_id), "{}").unwrap();

        let err = check_track_dir_exists(dir.path(), track_id).unwrap_err();
        assert!(
            matches!(&err, RefVerifyDriverError::Wiring(msg) if msg.contains("not a directory")),
            "expected Wiring error for non-directory track path, got: {err:?}"
        );
    }

    /// F2: a symlink at the track slot must be rejected before results classification.
    #[cfg(unix)]
    #[test]
    fn check_track_dir_exists_symlink_returns_wiring_error() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let track_id = "T001";
        let items_dir = dir.path().join("track").join("items");
        std::fs::create_dir_all(&items_dir).unwrap();
        std::os::unix::fs::symlink(outside.path(), items_dir.join(track_id)).unwrap();

        let err = check_track_dir_exists(dir.path(), track_id).unwrap_err();
        assert!(
            matches!(&err, RefVerifyDriverError::Wiring(msg)
                if msg.contains("cannot inspect track directory")
                    && msg.contains("refusing to follow symlink")),
            "expected Wiring error for symlinked track path, got: {err:?}"
        );
    }

    // ── duplicate hash, distinct origin (P1 finding — round 22) ─────────────────

    /// Two Chain-2 catalogue entries in the same layer share identical canonical JSON
    /// (same `claim_hash` + `evidence_hash`) but have different `claim_origin` values
    /// (distinct `entry_key` in the same catalogue section).
    ///
    /// Previously, `chain2_map` was built as `HashMap<HashKey, &SemanticVerifyEntry>`
    /// (first-wins or last-wins), so one of the two cache entries was silently dropped.
    /// After the fix both are retained in a `Vec`, and each current pair is matched by
    /// origin, so each pair sees its own cached verdict rather than collapsing to one.
    ///
    /// Invariant: `pair_a` (TypeA, cached as Pass) → `Pass`; `pair_b` (TypeB, cached
    /// as Fail) → `Fail { reason }`.  No cross-contamination of verdicts or origins.
    #[test]
    fn compute_results_duplicate_hash_distinct_origin_keeps_per_origin_verdict() {
        let layer = "domain";
        let layer_id = LayerId::try_new(layer.to_owned()).unwrap();

        // Both entries share (claim_hash=0x01, evidence_hash=0x02) but have different
        // claim_origin: CatalogueEntry with entry_key "TypeA" vs "TypeB".
        let entry_key_a = CatalogueEntryKey::try_new("TypeA".to_owned()).unwrap();
        let entry_key_b = CatalogueEntryKey::try_new("TypeB".to_owned()).unwrap();
        let shared_evidence_origin = spec_origin(0x02);

        let origin_a = VerifyOriginRef::CatalogueEntry(CatalogueEntryRef::new(
            format!("{layer}-types.json"),
            CatalogueSectionKey::Types,
            entry_key_a,
        ));
        let origin_b = VerifyOriginRef::CatalogueEntry(CatalogueEntryRef::new(
            format!("{layer}-types.json"),
            CatalogueSectionKey::Types,
            entry_key_b,
        ));

        // Cache entry A: Pass verdict
        let cache_entry_a = SemanticVerifyEntry::new(
            test_hash(0x01),
            test_hash(0x02),
            SemanticVerdict::Pass {
                citation: EvidenceCitation::try_new("spec says TypeA is covered".to_owned())
                    .unwrap(),
            },
            origin_a.clone(),
            shared_evidence_origin.clone(),
        );
        // Cache entry B: Fail verdict — shares the same hash pair as entry A
        let cache_entry_b = SemanticVerifyEntry::new(
            test_hash(0x01),
            test_hash(0x02),
            SemanticVerdict::Fail { reason: "mismatch for TypeB".to_owned() },
            origin_b.clone(),
            shared_evidence_origin.clone(),
        );

        // Two current pairs with the same hashes but each matching a different cached origin.
        let pair_a = RefVerifyPair {
            claim: "claim-TypeA".to_owned(),
            evidence: "evidence-02".to_owned(),
            claim_hash: test_hash(0x01),
            evidence_hash: test_hash(0x02),
            cache_scope: RefVerifyCacheScope::CatalogueSpec { layer: layer_id.clone() },
            known_bad: false,
            claim_origin: origin_a.clone(),
            evidence_origin: shared_evidence_origin.clone(),
        };
        let pair_b = RefVerifyPair {
            claim: "claim-TypeB".to_owned(),
            evidence: "evidence-02".to_owned(),
            claim_hash: test_hash(0x01),
            evidence_hash: test_hash(0x02),
            cache_scope: RefVerifyCacheScope::CatalogueSpec { layer: layer_id.clone() },
            known_bad: false,
            claim_origin: origin_b.clone(),
            evidence_origin: shared_evidence_origin.clone(),
        };

        let out = compute_results(
            vec![],
            vec![(layer_id, vec![cache_entry_a, cache_entry_b])],
            vec![pair_a, pair_b],
            RefVerifyChainFilter::Chain2,
            RefVerifyLayerFilter::All,
            RefVerifyVerdictFilter::All,
        )
        .unwrap();

        // Both pairs are classified without collapse.
        assert_eq!(out.total_pass, 1, "pair_a (TypeA) should be Pass");
        assert_eq!(out.total_fail, 1, "pair_b (TypeB) should be Fail");
        assert_eq!(out.total_pending, 0, "no pair should fall through to Pending");
        assert_eq!(out.pair_records.len(), 2, "both records must be present");

        // pair_a → Pass with origin_a
        let rec_a = out.pair_records.iter().find(|r| r.claim_origin == origin_a).unwrap();
        assert!(
            matches!(rec_a.verdict, SemanticVerdict::Pass { .. }),
            "TypeA record should carry Pass verdict, got {:?}",
            rec_a.verdict
        );

        // pair_b → Fail with origin_b and the correct reason
        let rec_b = out.pair_records.iter().find(|r| r.claim_origin == origin_b).unwrap();
        assert!(
            matches!(rec_b.verdict, SemanticVerdict::Fail { .. }),
            "TypeB record should carry Fail verdict, got {:?}",
            rec_b.verdict
        );
        assert_eq!(rec_b.reason, "mismatch for TypeB", "Fail reason must not be contaminated");
    }
}
