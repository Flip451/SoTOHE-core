//! Status-lane aggregation used by the informational `results` command.

use std::path::PathBuf;

use domain::SpecDocumentLoaderPort;
use domain::TrackId;
use domain::tddd::catalogue_v2::catalogue_impl_signals_ports::CatalogueDocumentLoaderPort;
use domain::tddd::test_obligation::binding::{TestBindingsDocument, TestLocation};
use domain::tddd::test_obligation::errors::ObligationResultsError;
use domain::tddd::test_obligation::hashes::VerifierPromptFingerprint;
use domain::tddd::test_obligation::ids::{TestObligationEdgeId, TestObligationId, WaivedReason};
use domain::tddd::test_obligation::obligations::{ObligationsDocument, TestObligation};
use domain::tddd::test_obligation::ports::TestSourceScannerPort;
use domain::tddd::test_obligation::verdict::{
    ObligationFulfillmentCacheDocument, ObligationFulfillmentVerdict, WaiverCacheDocument,
    WaiverVerdict,
};

use crate::pre_review_gate::{ImplPlanReaderPort, TaskContractReaderPort};

use super::check_support::{
    active_cited_edges_from_catalogues, anchor_text, anchor_texts, edge_is_derived,
    fulfillment_tests, spec_elements_from_document, synthetic_voluntary_obligation_id,
    voluntary_tests, waived_reason,
};
use super::results::TestObligationStatusLaneSummary;
use super::status_lanes::{
    StatusLaneFinding, StatusLaneFindingKind, StatusLaneTarget, TaskStatusAttributor,
    tally_findings, target_for_direct_edge, target_for_obligation, targets_for_scope,
};
use super::{
    LoadedCatalogueDocument, declaration_with_obligation_item, diag,
    find_declaration_text_from_loaded, obligation_declaration_text_from_loaded,
    sha256_content_hash,
};

/// Computes the informational missing / stale / verdict-absent totals without
/// changing any artifact or gate verdict.
#[allow(clippy::too_many_arguments)]
pub(super) fn collect_status_lane_summaries(
    track_id: &TrackId,
    catalogue_paths: &[PathBuf],
    obligations: Option<&ObligationsDocument>,
    bindings: Option<&TestBindingsDocument>,
    fulfillment: Option<&ObligationFulfillmentCacheDocument>,
    waiver: Option<&WaiverCacheDocument>,
    source_scanner: &dyn TestSourceScannerPort,
    fulfillment_fingerprint: &VerifierPromptFingerprint,
    waiver_fingerprint: &VerifierPromptFingerprint,
    spec_reader: &dyn SpecDocumentLoaderPort,
    catalogue_reader: &dyn CatalogueDocumentLoaderPort,
    task_contract_reader: &dyn TaskContractReaderPort,
    impl_plan_reader: &dyn ImplPlanReaderPort,
) -> Result<Vec<TestObligationStatusLaneSummary>, ObligationResultsError> {
    let (Some(obligations), Some(bindings)) = (obligations, bindings) else {
        return Ok(Vec::new());
    };
    if catalogue_paths.is_empty() {
        return Ok(Vec::new());
    }
    let catalogues = load_catalogues(catalogue_paths, catalogue_reader)?;
    let spec_path = PathBuf::from(format!("track/items/{}/spec.json", track_id.as_ref()));
    let spec = spec_reader
        .load(&spec_path)
        .map_err(|error| malformed(&format!("spec read failed: {error:?}")))?;
    let spec_texts = anchor_texts(&spec_elements_from_document(&spec));
    let cited_edges = active_cited_edges_from_catalogues(&catalogues)
        .map_err(|error| malformed(&format!("catalogue edge read failed: {error:?}")))?;
    let targets = targets_for_scope(obligations, &cited_edges, &catalogues).map_err(|error| {
        malformed(&format!("task attribution target failed: {}", error.as_str()))
    })?;
    let attributor =
        TaskStatusAttributor::load(task_contract_reader, impl_plan_reader, track_id, &targets)
            .map_err(|error| malformed(&format!("task attribution failed: {}", error.as_str())))?;
    let fulfillment = fulfillment
        .cloned()
        .unwrap_or_else(|| ObligationFulfillmentCacheDocument::new(track_id.clone(), Vec::new()));
    let waiver =
        waiver.cloned().unwrap_or_else(|| WaiverCacheDocument::new(track_id.clone(), Vec::new()));
    let mut findings = Vec::new();

    for obligation in obligations.obligations() {
        collect_obligation_findings(
            obligation,
            bindings,
            &catalogues,
            &spec_texts,
            &fulfillment,
            &waiver,
            source_scanner,
            fulfillment_fingerprint,
            waiver_fingerprint,
            &mut findings,
        )?;
    }
    for edge in &cited_edges {
        if !edge_is_derived(obligations, edge) {
            collect_direct_edge_findings(
                edge,
                bindings,
                &catalogues,
                &spec_texts,
                &fulfillment,
                &waiver,
                source_scanner,
                fulfillment_fingerprint,
                waiver_fingerprint,
                &mut findings,
            )?;
        }
    }

    tally_findings(&attributor, &findings)
        .map_err(|error| malformed(&format!("task attribution failed: {}", error.as_str())))
        .map(|tallies| {
            tallies
                .into_iter()
                .map(|tally| {
                    TestObligationStatusLaneSummary::new(
                        tally.task_status(),
                        tally.missing_count(),
                        tally.stale_count(),
                        tally.verdict_absent_count(),
                    )
                })
                .collect()
        })
}

#[allow(clippy::too_many_arguments)]
fn collect_obligation_findings(
    obligation: &TestObligation,
    bindings: &TestBindingsDocument,
    catalogues: &[LoadedCatalogueDocument],
    spec_texts: &[(String, String)],
    fulfillment: &ObligationFulfillmentCacheDocument,
    waiver: &WaiverCacheDocument,
    source_scanner: &dyn TestSourceScannerPort,
    fulfillment_fingerprint: &VerifierPromptFingerprint,
    waiver_fingerprint: &VerifierPromptFingerprint,
    findings: &mut Vec<StatusLaneFinding>,
) -> Result<(), ObligationResultsError> {
    let target = target_for_obligation(catalogues, obligation).map_err(|error| {
        malformed(&format!("task attribution target failed: {}", error.as_str()))
    })?;
    let edges = obligation
        .spec_refs()
        .iter()
        .map(|anchor| {
            TestObligationEdgeId::new(obligation.id().entry_key().clone(), anchor.clone())
        })
        .collect::<Vec<_>>();
    let fulfilled = fulfillment_tests(bindings, obligation.id());
    if edges.is_empty() {
        // D1: owning no anchors is not a fulfillment gap. A fulfillment
        // record is a structural orphan handled by `check`, not `missing`.
        return Ok(());
    }
    let any_voluntary = edges.iter().any(|edge| voluntary_tests(bindings, edge).is_some());
    let any_waived = edges.iter().any(|edge| waived_reason(bindings, edge).is_some());
    if fulfilled.is_none() && !any_voluntary && !any_waived {
        findings.push(missing(target));
        return Ok(());
    }
    for edge in edges {
        if let Some(reason) = waived_reason(bindings, &edge) {
            inspect_waiver(
                &edge,
                obligation.id(),
                &reason,
                declaration_with_obligation_item(
                    &obligation_declaration_text_from_loaded(catalogues, obligation)
                        .unwrap_or_default(),
                    obligation.id().item_identifier().as_str(),
                ),
                &target,
                spec_texts,
                waiver,
                waiver_fingerprint,
                findings,
            );
        } else if let Some(tests) = fulfilled {
            inspect_fulfillment(
                &edge,
                obligation.id(),
                tests,
                declaration_with_obligation_item(
                    &obligation_declaration_text_from_loaded(catalogues, obligation)
                        .unwrap_or_default(),
                    obligation.id().item_identifier().as_str(),
                ),
                &target,
                spec_texts,
                fulfillment,
                source_scanner,
                fulfillment_fingerprint,
                findings,
            )?;
        } else if let Some(tests) = voluntary_tests(bindings, &edge) {
            inspect_fulfillment(
                &edge,
                obligation.id(),
                tests,
                declaration_with_obligation_item(
                    &obligation_declaration_text_from_loaded(catalogues, obligation)
                        .unwrap_or_default(),
                    obligation.id().item_identifier().as_str(),
                ),
                &target,
                spec_texts,
                fulfillment,
                source_scanner,
                fulfillment_fingerprint,
                findings,
            )?;
        } else {
            findings.push(missing(target.clone()));
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn collect_direct_edge_findings(
    edge: &TestObligationEdgeId,
    bindings: &TestBindingsDocument,
    catalogues: &[LoadedCatalogueDocument],
    spec_texts: &[(String, String)],
    fulfillment: &ObligationFulfillmentCacheDocument,
    waiver: &WaiverCacheDocument,
    source_scanner: &dyn TestSourceScannerPort,
    fulfillment_fingerprint: &VerifierPromptFingerprint,
    waiver_fingerprint: &VerifierPromptFingerprint,
    findings: &mut Vec<StatusLaneFinding>,
) -> Result<(), ObligationResultsError> {
    let target = target_for_direct_edge(catalogues, edge).map_err(|error| {
        malformed(&format!("task attribution target failed: {}", error.as_str()))
    })?;
    let declaration = find_declaration_text_from_loaded(catalogues, edge.entry_key().as_str())
        .unwrap_or_default();
    let synthetic_id = synthetic_voluntary_obligation_id(edge);
    if let Some(reason) = waived_reason(bindings, edge) {
        inspect_waiver(
            edge,
            &synthetic_id,
            &reason,
            declaration_with_obligation_item(&declaration, synthetic_id.item_identifier().as_str()),
            &target,
            spec_texts,
            waiver,
            waiver_fingerprint,
            findings,
        );
    } else if let Some(tests) = voluntary_tests(bindings, edge) {
        inspect_fulfillment(
            edge,
            &synthetic_id,
            tests,
            declaration_with_obligation_item(&declaration, synthetic_id.item_identifier().as_str()),
            &target,
            spec_texts,
            fulfillment,
            source_scanner,
            fulfillment_fingerprint,
            findings,
        )?;
    } else {
        findings.push(missing(target));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn inspect_fulfillment(
    edge: &TestObligationEdgeId,
    obligation_id: &TestObligationId,
    tests: &[TestLocation],
    declaration: String,
    target: &StatusLaneTarget,
    spec_texts: &[(String, String)],
    cache: &ObligationFulfillmentCacheDocument,
    source_scanner: &dyn TestSourceScannerPort,
    verifier_fingerprint: &VerifierPromptFingerprint,
    findings: &mut Vec<StatusLaneFinding>,
) -> Result<(), ObligationResultsError> {
    let mut source = String::new();
    for test in tests {
        let Some(body) = source_scanner
            .scan_test_body(test)
            .map_err(|error| malformed(&format!("test source read failed: {error:?}")))?
        else {
            findings.push(missing(target.clone()));
            return Ok(());
        };
        source.push_str(&body);
        source.push('\n');
    }
    let Some(entry) = cache
        .entries()
        .iter()
        .find(|entry| entry.edge_id() == edge && entry.obligation_id() == obligation_id)
    else {
        findings.push(verdict_absent(target.clone()));
        return Ok(());
    };
    if entry.verifier_fingerprint() != Some(verifier_fingerprint) {
        findings.push(verdict_absent(target.clone()));
        return Ok(());
    }
    let current_bound = sha256_content_hash(source.as_bytes());
    let current_decl = sha256_content_hash(declaration.as_bytes());
    let current_anchor = sha256_content_hash(anchor_text(spec_texts, edge.anchor_id()).as_bytes());
    let key = entry.key();
    if key.bound_tests_set_hash().as_hash() != &current_bound
        || key.declaration_hash().as_hash() != &current_decl
        || key.anchor_text_hash().as_hash() != &current_anchor
    {
        findings.push(stale(target.clone()));
    } else if !matches!(entry.verdict(), ObligationFulfillmentVerdict::Fulfilled { .. }) {
        findings.push(verdict_absent(target.clone()));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn inspect_waiver(
    edge: &TestObligationEdgeId,
    obligation_id: &TestObligationId,
    reason: &WaivedReason,
    declaration: String,
    target: &StatusLaneTarget,
    spec_texts: &[(String, String)],
    cache: &WaiverCacheDocument,
    verifier_fingerprint: &VerifierPromptFingerprint,
    findings: &mut Vec<StatusLaneFinding>,
) {
    let Some(entry) = cache
        .entries()
        .iter()
        .find(|entry| entry.edge_id() == edge && entry.obligation_id() == Some(obligation_id))
    else {
        findings.push(verdict_absent(target.clone()));
        return;
    };
    if entry.verifier_fingerprint() != Some(verifier_fingerprint) {
        findings.push(verdict_absent(target.clone()));
        return;
    }
    let current_reason = sha256_content_hash(reason.as_str().as_bytes());
    let current_decl = sha256_content_hash(declaration.as_bytes());
    let current_anchor = sha256_content_hash(anchor_text(spec_texts, edge.anchor_id()).as_bytes());
    let key = entry.key();
    if key.waived_reason_hash().as_hash() != &current_reason
        || key.declaration_hash().as_hash() != &current_decl
        || key.anchor_text_hash().as_hash() != &current_anchor
    {
        findings.push(stale(target.clone()));
    } else if !matches!(entry.verdict(), WaiverVerdict::Waived { .. }) {
        findings.push(verdict_absent(target.clone()));
    }
}

fn load_catalogues(
    paths: &[PathBuf],
    reader: &dyn CatalogueDocumentLoaderPort,
) -> Result<Vec<LoadedCatalogueDocument>, ObligationResultsError> {
    paths
        .iter()
        .map(|path| {
            reader
                .load(path)
                .map(|document| LoadedCatalogueDocument::new(path, document))
                .map_err(|error| malformed(&format!("catalogue read failed: {error:?}")))
        })
        .collect()
}

fn missing(target: StatusLaneTarget) -> StatusLaneFinding {
    StatusLaneFinding::new(target, StatusLaneFindingKind::Missing)
}

fn stale(target: StatusLaneTarget) -> StatusLaneFinding {
    StatusLaneFinding::new(target, StatusLaneFindingKind::Stale)
}

fn verdict_absent(target: StatusLaneTarget) -> StatusLaneFinding {
    StatusLaneFinding::new(target, StatusLaneFindingKind::VerdictAbsent)
}

fn malformed(message: &str) -> ObligationResultsError {
    ObligationResultsError::MalformedArtifact(diag(message))
}
