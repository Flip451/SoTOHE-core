//! Status-lane aggregation used by the informational `results` command.

use std::path::PathBuf;

use domain::SpecDocumentLoaderPort;
use domain::TrackId;
use domain::tddd::test_obligation::binding::{TestBindingsDocument, TestLocation};
use domain::tddd::test_obligation::errors::ObligationResultsError;
use domain::tddd::test_obligation::hashes::{
    BoundTestsSetHash, DeclarationHash, VerifierPromptFingerprint, WaivedReasonHash,
};
use domain::tddd::test_obligation::ids::{
    TestObligationBrief, TestObligationEdgeId, TestObligationId, WaivedReason,
};
use domain::tddd::test_obligation::obligations::{ObligationsDocument, TestObligation};
use domain::tddd::test_obligation::ports::TestSourceScannerPort;
use domain::tddd::test_obligation::verdict::{
    FulfillmentCacheLookupError, ObligationFulfillmentCacheDocument, ObligationFulfillmentCacheKey,
    ObligationFulfillmentVerdict, WaiverCacheDocument, WaiverCacheKey, WaiverCacheLookupError,
    WaiverVerdict,
};

use crate::catalogue_document_loader::AttestedCatalogueDocumentLoaderPort;
use crate::pre_review_gate::{ImplPlanReaderPort, TaskContractReaderPort};

use super::check_support::{
    SpecElement, active_cited_edges_from_catalogues, edge_is_derived, fulfillment_tests,
    spec_elements_from_document, synthetic_voluntary_obligation_id, voluntary_tests, waived_reason,
};
use super::freshness::{responsibility_hash, spec_element_hash};
use super::results::TestObligationStatusLaneSummary;
use super::status_lanes::{
    StatusLaneFinding, StatusLaneFindingKind, StatusLaneTarget, TaskStatusAttributor,
    tally_findings, target_for_direct_edge, target_for_obligation, targets_for_scope,
};
use super::{
    LoadedCatalogueDocument, declaration_with_obligation_context, declaration_with_obligation_item,
    diag, find_declaration_text_from_loaded, obligation_declaration_text_from_loaded,
    sha256_content_hash, synthetic_voluntary_obligation_brief,
};

/// Cache rows that cannot be treated as current by the informational results
/// projection. Indices are aligned with the loaded cache documents.
pub(super) struct StatusLaneProjection {
    status_lane_summaries: Vec<TestObligationStatusLaneSummary>,
    invalidated_fulfillment: Vec<usize>,
    invalidated_waiver: Vec<usize>,
}

impl StatusLaneProjection {
    fn without_context(
        fulfillment: Option<&ObligationFulfillmentCacheDocument>,
        waiver: Option<&WaiverCacheDocument>,
    ) -> Self {
        Self {
            status_lane_summaries: Vec::new(),
            invalidated_fulfillment: fulfillment
                .map_or_else(Vec::new, |document| (0..document.entries().len()).collect()),
            invalidated_waiver: waiver
                .map_or_else(Vec::new, |document| (0..document.entries().len()).collect()),
        }
    }

    fn for_caches(
        fulfillment: &ObligationFulfillmentCacheDocument,
        waiver: &WaiverCacheDocument,
    ) -> Self {
        Self {
            status_lane_summaries: Vec::new(),
            invalidated_fulfillment: (0..fulfillment.entries().len()).collect(),
            invalidated_waiver: (0..waiver.entries().len()).collect(),
        }
    }

    fn set_status_lane_summaries(&mut self, summaries: Vec<TestObligationStatusLaneSummary>) {
        self.status_lane_summaries = summaries;
    }

    /// Returns the status summaries for the task-status lanes.
    #[must_use]
    pub(super) fn status_lane_summaries(&self) -> &[TestObligationStatusLaneSummary] {
        &self.status_lane_summaries
    }

    /// Returns fulfillment rows whose stored verdict is not current.
    #[must_use]
    pub(super) fn invalidated_fulfillment(&self) -> &[usize] {
        &self.invalidated_fulfillment
    }

    /// Returns waiver rows whose stored verdict is not current.
    #[must_use]
    pub(super) fn invalidated_waiver(&self) -> &[usize] {
        &self.invalidated_waiver
    }

    fn invalidate_fulfillment_rows(
        &mut self,
        cache: &ObligationFulfillmentCacheDocument,
        edge: &TestObligationEdgeId,
        obligation_id: &TestObligationId,
        current: Option<(&ObligationFulfillmentCacheKey, &VerifierPromptFingerprint)>,
    ) {
        for (index, entry) in cache.entries().iter().enumerate() {
            if entry.edge_id() != edge || entry.obligation_id() != obligation_id {
                continue;
            }
            let is_current = current.is_some_and(|(key, fingerprint)| {
                entry.key() == key && entry.verifier_fingerprint() == Some(fingerprint)
            });
            if is_current {
                self.invalidated_fulfillment.retain(|candidate| *candidate != index);
            } else if !self.invalidated_fulfillment.contains(&index) {
                self.invalidated_fulfillment.push(index);
            }
        }
    }

    fn invalidate_waiver_rows(
        &mut self,
        cache: &WaiverCacheDocument,
        edge: &TestObligationEdgeId,
        obligation_id: &TestObligationId,
        current: Option<(&WaiverCacheKey, &VerifierPromptFingerprint)>,
    ) {
        for (index, entry) in cache.entries().iter().enumerate() {
            if entry.edge_id() != edge || entry.obligation_id() != Some(obligation_id) {
                continue;
            }
            let is_current = current.is_some_and(|(key, fingerprint)| {
                entry.key() == key && entry.verifier_fingerprint() == Some(fingerprint)
            });
            if is_current {
                self.invalidated_waiver.retain(|candidate| *candidate != index);
            } else if !self.invalidated_waiver.contains(&index) {
                self.invalidated_waiver.push(index);
            }
        }
    }
}

/// Computes the informational missing / stale / verdict-absent totals without
/// changing any artifact or gate verdict.
#[allow(clippy::too_many_arguments)]
pub(super) fn collect_status_lane_projection(
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
    catalogue_reader: &dyn AttestedCatalogueDocumentLoaderPort,
    task_contract_reader: &dyn TaskContractReaderPort,
    impl_plan_reader: &dyn ImplPlanReaderPort,
) -> Result<StatusLaneProjection, ObligationResultsError> {
    let (Some(obligations), Some(bindings)) = (obligations, bindings) else {
        return Ok(StatusLaneProjection::without_context(fulfillment, waiver));
    };
    if catalogue_paths.is_empty() {
        return Ok(StatusLaneProjection::without_context(fulfillment, waiver));
    }
    let catalogues = load_catalogues(catalogue_paths, catalogue_reader)?;
    let spec_path = PathBuf::from(format!("track/items/{}/spec.json", track_id.as_ref()));
    let spec = spec_reader
        .load(&spec_path)
        .map_err(|error| malformed(&format!("spec read failed: {error:?}")))?;
    let spec_elements = spec_elements_from_document(&spec);
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
    let mut projection = StatusLaneProjection::for_caches(&fulfillment, &waiver);

    for obligation in obligations.obligations() {
        collect_obligation_findings(
            obligation,
            bindings,
            &catalogues,
            &spec_elements,
            &fulfillment,
            &waiver,
            source_scanner,
            fulfillment_fingerprint,
            waiver_fingerprint,
            &mut projection,
            &mut findings,
        )?;
    }
    for edge in &cited_edges {
        if !edge_is_derived(obligations, edge) {
            collect_direct_edge_findings(
                edge,
                bindings,
                &catalogues,
                &spec_elements,
                &fulfillment,
                &waiver,
                source_scanner,
                fulfillment_fingerprint,
                waiver_fingerprint,
                &mut projection,
                &mut findings,
            )?;
        }
    }

    let summaries = tally_findings(&attributor, &findings)
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
        })?;
    projection.set_status_lane_summaries(summaries);
    Ok(projection)
}

#[allow(clippy::too_many_arguments)]
fn collect_obligation_findings(
    obligation: &TestObligation,
    bindings: &TestBindingsDocument,
    catalogues: &[LoadedCatalogueDocument],
    spec_elements: &[SpecElement],
    fulfillment: &ObligationFulfillmentCacheDocument,
    waiver: &WaiverCacheDocument,
    source_scanner: &dyn TestSourceScannerPort,
    fulfillment_fingerprint: &VerifierPromptFingerprint,
    waiver_fingerprint: &VerifierPromptFingerprint,
    projection: &mut StatusLaneProjection,
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
                obligation.brief(),
                &target,
                spec_elements,
                waiver,
                waiver_fingerprint,
                projection,
                findings,
            );
        } else if let Some(tests) = fulfilled {
            inspect_fulfillment(
                &edge,
                obligation.id(),
                tests,
                declaration_with_obligation_context(
                    &obligation_declaration_text_from_loaded(catalogues, obligation)
                        .unwrap_or_default(),
                    obligation.id(),
                    obligation.brief(),
                ),
                obligation.brief(),
                &target,
                spec_elements,
                fulfillment,
                source_scanner,
                fulfillment_fingerprint,
                projection,
                findings,
            )?;
        } else if let Some(tests) = voluntary_tests(bindings, &edge) {
            inspect_fulfillment(
                &edge,
                obligation.id(),
                tests,
                declaration_with_obligation_context(
                    &obligation_declaration_text_from_loaded(catalogues, obligation)
                        .unwrap_or_default(),
                    obligation.id(),
                    obligation.brief(),
                ),
                obligation.brief(),
                &target,
                spec_elements,
                fulfillment,
                source_scanner,
                fulfillment_fingerprint,
                projection,
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
    spec_elements: &[SpecElement],
    fulfillment: &ObligationFulfillmentCacheDocument,
    waiver: &WaiverCacheDocument,
    source_scanner: &dyn TestSourceScannerPort,
    fulfillment_fingerprint: &VerifierPromptFingerprint,
    waiver_fingerprint: &VerifierPromptFingerprint,
    projection: &mut StatusLaneProjection,
    findings: &mut Vec<StatusLaneFinding>,
) -> Result<(), ObligationResultsError> {
    let target = target_for_direct_edge(catalogues, edge).map_err(|error| {
        malformed(&format!("task attribution target failed: {}", error.as_str()))
    })?;
    let declaration = find_declaration_text_from_loaded(catalogues, edge.entry_key().as_str())
        .unwrap_or_default();
    let synthetic_id = synthetic_voluntary_obligation_id(edge);
    let synthetic_brief = synthetic_voluntary_obligation_brief(edge).map_err(|error| {
        malformed(&format!("invalid voluntary obligation brief for {edge:?}: {error}"))
    })?;
    if let Some(reason) = waived_reason(bindings, edge) {
        inspect_waiver(
            edge,
            &synthetic_id,
            &reason,
            declaration_with_obligation_item(&declaration, synthetic_id.item_identifier().as_str()),
            &synthetic_brief,
            &target,
            spec_elements,
            waiver,
            waiver_fingerprint,
            projection,
            findings,
        );
    } else if let Some(tests) = voluntary_tests(bindings, edge) {
        inspect_fulfillment(
            edge,
            &synthetic_id,
            tests,
            declaration_with_obligation_context(&declaration, &synthetic_id, &synthetic_brief),
            &synthetic_brief,
            &target,
            spec_elements,
            fulfillment,
            source_scanner,
            fulfillment_fingerprint,
            projection,
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
    obligation_brief: &TestObligationBrief,
    target: &StatusLaneTarget,
    spec_elements: &[SpecElement],
    cache: &ObligationFulfillmentCacheDocument,
    source_scanner: &dyn TestSourceScannerPort,
    verifier_fingerprint: &VerifierPromptFingerprint,
    projection: &mut StatusLaneProjection,
    findings: &mut Vec<StatusLaneFinding>,
) -> Result<(), ObligationResultsError> {
    let mut source = String::new();
    for test in tests {
        let Some(body) = source_scanner
            .scan_test_body(test)
            .map_err(|error| malformed(&format!("test source read failed: {error:?}")))?
        else {
            projection.invalidate_fulfillment_rows(cache, edge, obligation_id, None);
            findings.push(missing(target.clone()));
            return Ok(());
        };
        source.push_str(&body);
        source.push('\n');
    }
    let current_bound = sha256_content_hash(source.as_bytes());
    let current_decl = sha256_content_hash(declaration.as_bytes());
    let current_spec_element = spec_element_hash(spec_elements, edge);
    let current_responsibility = responsibility_hash(obligation_id, obligation_brief);
    let current_key = ObligationFulfillmentCacheKey::new(
        BoundTestsSetHash::new(current_bound),
        DeclarationHash::new(current_decl),
        current_spec_element,
        current_responsibility,
    );
    let entry = match cache.lookup_current(edge, obligation_id, &current_key, verifier_fingerprint)
    {
        Ok(entry) => entry,
        Err(FulfillmentCacheLookupError::AmbiguousCurrentEntries { .. }) => {
            projection.invalidate_fulfillment_rows(cache, edge, obligation_id, None);
            findings.push(verdict_absent(target.clone()));
            return Ok(());
        }
    };
    let Some(entry) = entry else {
        projection.invalidate_fulfillment_rows(cache, edge, obligation_id, None);
        if cache.entries().iter().any(|candidate| {
            candidate.edge_id() == edge
                && candidate.obligation_id() == obligation_id
                && candidate.verifier_fingerprint() == Some(verifier_fingerprint)
        }) {
            findings.push(stale(target.clone()));
        } else {
            findings.push(verdict_absent(target.clone()));
        }
        return Ok(());
    };
    projection.invalidate_fulfillment_rows(
        cache,
        edge,
        obligation_id,
        Some((&current_key, verifier_fingerprint)),
    );
    if !matches!(entry.verdict(), ObligationFulfillmentVerdict::Fulfilled { .. }) {
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
    obligation_brief: &TestObligationBrief,
    target: &StatusLaneTarget,
    spec_elements: &[SpecElement],
    cache: &WaiverCacheDocument,
    verifier_fingerprint: &VerifierPromptFingerprint,
    projection: &mut StatusLaneProjection,
    findings: &mut Vec<StatusLaneFinding>,
) {
    let current_reason = sha256_content_hash(reason.as_str().as_bytes());
    let current_decl = sha256_content_hash(declaration.as_bytes());
    let current_spec_element = spec_element_hash(spec_elements, edge);
    let current_responsibility = responsibility_hash(obligation_id, obligation_brief);
    let current_key = WaiverCacheKey::new(
        WaivedReasonHash::new(current_reason),
        DeclarationHash::new(current_decl),
        current_spec_element,
        current_responsibility,
    );
    let entry = match cache.lookup_current(edge, obligation_id, &current_key, verifier_fingerprint)
    {
        Ok(entry) => entry,
        Err(WaiverCacheLookupError::AmbiguousCurrentEntries { .. }) => {
            projection.invalidate_waiver_rows(cache, edge, obligation_id, None);
            findings.push(verdict_absent(target.clone()));
            return;
        }
    };
    let Some(entry) = entry else {
        projection.invalidate_waiver_rows(cache, edge, obligation_id, None);
        if cache.entries().iter().any(|candidate| {
            candidate.edge_id() == edge
                && candidate.obligation_id() == Some(obligation_id)
                && candidate.verifier_fingerprint() == Some(verifier_fingerprint)
        }) {
            findings.push(stale(target.clone()));
        } else {
            findings.push(verdict_absent(target.clone()));
        }
        return;
    };
    projection.invalidate_waiver_rows(
        cache,
        edge,
        obligation_id,
        Some((&current_key, verifier_fingerprint)),
    );
    if !matches!(entry.verdict(), WaiverVerdict::Waived { .. }) {
        findings.push(verdict_absent(target.clone()));
    }
}

fn load_catalogues(
    paths: &[PathBuf],
    reader: &dyn AttestedCatalogueDocumentLoaderPort,
) -> Result<Vec<LoadedCatalogueDocument>, ObligationResultsError> {
    paths
        .iter()
        .map(|path| {
            reader
                .load(path)
                .map(|attested| LoadedCatalogueDocument::new(path, attested.into_document()))
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
