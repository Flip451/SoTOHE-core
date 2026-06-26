//! Results command helpers for [`super::driver_adapter::FsRefVerifyAggregateAdapter`].
//!
//! Extracted from `driver_adapter.rs` to stay under the 700-line module-size cap.

use std::path::Path;

use usecase::ref_verify::RefVerifyDriverError;

/// Load TDDD layer bindings for the `results` command.
///
/// Propagates missing or invalid `architecture-rules.json` as
/// [`RefVerifyDriverError::Wiring`] so Chain2 results fail closed when layer
/// bindings cannot be determined.
pub(crate) fn load_results_tddd_bindings(
    project_root: &Path,
) -> Result<Vec<crate::verify::tddd_layers::TdddLayerBinding>, RefVerifyDriverError> {
    let rules_path = project_root.join("architecture-rules.json");
    match crate::verify::tddd_layers::load_tddd_layers(&rules_path, project_root) {
        Ok(bindings) => Ok(bindings),
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
/// - `Pass`: origins from cache entry; empty reason.
/// - `Fail`: origins from cache entry; reason from cached `Fail { reason }`.
/// - `Pending` (cache miss or `Pending` verdict in cache): origins from the
///   current pair-source `origin_lookup`, falling back to cached origins for
///   `Pending`-in-cache entries when the key is absent from the lookup.
fn extract_verdict_and_origins(
    key: &(domain::ContentHash, domain::ContentHash),
    cache_map: &std::collections::HashMap<
        (domain::ContentHash, domain::ContentHash),
        &domain::tddd::semantic_verify::SemanticVerifyEntry,
    >,
    origin_lookup: &std::collections::HashMap<
        (domain::ContentHash, domain::ContentHash),
        (
            domain::tddd::semantic_verify::VerifyOriginRef,
            domain::tddd::semantic_verify::VerifyOriginRef,
        ),
    >,
) -> (
    domain::tddd::semantic_verify::SemanticVerdict,
    String,
    domain::tddd::semantic_verify::VerifyOriginRef,
    domain::tddd::semantic_verify::VerifyOriginRef,
) {
    use domain::tddd::semantic_verify::SemanticVerdict;

    match cache_map.get(key) {
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
            // Pending in cache → unresolved; prefer origins from pair source.
            SemanticVerdict::Pending => {
                let (co, eo) = origin_lookup
                    .get(key)
                    .cloned()
                    .unwrap_or_else(|| (entry.claim_origin.clone(), entry.evidence_origin.clone()));
                (SemanticVerdict::Pending, "pair not yet verified".to_owned(), co, eo)
            }
        },
        // Cache miss → pending; derive origins from current pair source.
        // The lookup should always succeed because `origin_lookup` is built from
        // the same `current_pairs` that supply the keys.
        None => {
            use domain::tddd::semantic_verify::{AdrDecisionRef, VerifyOriginRef};
            let placeholder =
                || VerifyOriginRef::AdrDecision(AdrDecisionRef::new(String::new(), String::new()));
            let (co, eo) =
                origin_lookup.get(key).cloned().unwrap_or_else(|| (placeholder(), placeholder()));
            (SemanticVerdict::Pending, "pair not yet verified".to_owned(), co, eo)
        }
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
    use domain::tddd::semantic_verify::{SemanticVerdict, SemanticVerifyEntry, VerifyOriginRef};
    use std::collections::HashMap;
    use usecase::ref_verify::{
        RefVerifyCacheScope, RefVerifyChainFilter, RefVerifyDriverError, RefVerifyLaneSummary,
        RefVerifyLayerFilter, RefVerifyPairRecord, RefVerifyResultsOutput, RefVerifyVerdictFilter,
    };

    // Validate layer filter when Chain2 results are requested.
    let include_chain2 = matches!(chain, RefVerifyChainFilter::Chain2 | RefVerifyChainFilter::All);
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

    // Build cache lookup maps keyed by (claim_hash, evidence_hash).
    let chain1_map: HashMap<HashKey, &SemanticVerifyEntry> =
        chain1_cache.iter().map(|e| ((e.claim_hash.clone(), e.evidence_hash.clone()), e)).collect();

    let chain2_maps: HashMap<String, HashMap<HashKey, &SemanticVerifyEntry>> = chain2_caches
        .iter()
        .map(|(layer_id, entries)| {
            let map = entries
                .iter()
                .map(|e| ((e.claim_hash.clone(), e.evidence_hash.clone()), e))
                .collect();
            (layer_id.as_ref().to_owned(), map)
        })
        .collect();

    // Build origin lookup from current pair source for pending-pair origin derivation.
    let mut origin_lookup: HashMap<HashKey, (VerifyOriginRef, VerifyOriginRef)> = HashMap::new();
    for pair in &current_pairs {
        origin_lookup
            .entry((pair.claim_hash.clone(), pair.evidence_hash.clone()))
            .or_insert_with(|| (pair.claim_origin.clone(), pair.evidence_origin.clone()));
    }

    // (e/h) Classify pairs and accumulate per-lane data.
    let mut chain1_lane: Option<LaneAccum> = None;
    // Ordered list of layer strings for deterministic output (insertion order).
    let mut chain2_lane_order: Vec<String> = Vec::new();
    let mut chain2_lane_map: HashMap<String, LaneAccum> = HashMap::new();
    let empty_map: HashMap<HashKey, &SemanticVerifyEntry> = HashMap::new();

    for pair in &current_pairs {
        let key = (pair.claim_hash.clone(), pair.evidence_hash.clone());

        match &pair.cache_scope {
            RefVerifyCacheScope::SpecAdr => {
                let (v, r, co, eo) = extract_verdict_and_origins(&key, &chain1_map, &origin_lookup);
                let lane = chain1_lane.get_or_insert_with(|| LaneAccum {
                    label: "Chain1 (spec\u{2194}ADR)".to_owned(),
                    pass_count: 0,
                    fail_count: 0,
                    pending_count: 0,
                    records: Vec::new(),
                });
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
            RefVerifyCacheScope::CatalogueSpec { layer: layer_id } => {
                let layer_str = layer_id.as_ref().to_owned();
                let layer_cache = chain2_maps.get(&layer_str).unwrap_or(&empty_map);
                let (v, r, co, eo) = extract_verdict_and_origins(&key, layer_cache, &origin_lookup);
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

    // (f/g/h/i) Apply chain, layer, and verdict filters; assemble output.
    let include_chain1 = matches!(chain, RefVerifyChainFilter::Chain1 | RefVerifyChainFilter::All);
    let include_chain2 = matches!(chain, RefVerifyChainFilter::Chain2 | RefVerifyChainFilter::All);

    let layer_matches = |lane_layer: &str| match &layer {
        RefVerifyLayerFilter::All => true,
        RefVerifyLayerFilter::Specific(id) => id.as_ref() == lane_layer,
    };

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
            if !layer_matches(layer_str.as_str()) {
                continue;
            }
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

    #[test]
    fn load_results_tddd_bindings_missing_rules_returns_error() {
        let dir = tempfile::tempdir().unwrap();

        let err = load_results_tddd_bindings(dir.path()).unwrap_err();

        assert!(matches!(err, RefVerifyDriverError::Wiring(_)));
        assert!(err.to_string().contains("cannot load TDDD layer bindings for results"));
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
    fn compute_results_empty_pairs_returns_empty_output() {
        let out = compute_results(
            vec![],
            vec![],
            vec![],
            RefVerifyChainFilter::All,
            RefVerifyLayerFilter::All,
            RefVerifyVerdictFilter::All,
        )
        .unwrap();
        assert!(out.lane_summaries.is_empty());
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
            vec![(layer_id.clone(), vec![pass_cache_entry(0x01, 0x02)])],
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
            vec![(domain_id.clone(), vec![pass_cache_entry(0x01, 0x02)])],
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
            vec![(layer_id.clone(), vec![pass_cache_entry(0x01, 0x02)])],
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
    fn compute_results_chain2_all_with_pre_phase2_state_returns_zero_pair_result() {
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
        assert!(out.lane_summaries.is_empty());
        assert!(out.pair_records.is_empty());
        assert_eq!(out.total_pass, 0);
        assert_eq!(out.total_fail, 0);
        assert_eq!(out.total_pending, 0);
    }

    /// Verifies that a specific valid layer with an absent catalogue is still
    /// accepted as a zero-pair pre-Phase-2 result, not misclassified as an
    /// unknown-layer typo by cache-based validation.
    #[test]
    fn compute_results_chain2_specific_with_pre_phase2_state_returns_zero_pair_result() {
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
        assert!(out.lane_summaries.is_empty());
        assert!(out.pair_records.is_empty());
        assert_eq!(out.total_pass, 0);
        assert_eq!(out.total_fail, 0);
        assert_eq!(out.total_pending, 0);
    }
}
