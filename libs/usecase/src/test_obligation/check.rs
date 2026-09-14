//! Pure-read totality and drift gate for `bin/sotp test-obligation check`.

// The catalogue contract requires unboxed non-empty error payloads.
#![allow(clippy::result_large_err)]

use std::sync::Arc;

use domain::TaskStatusKind;
use domain::tddd::test_obligation::binding::{
    TestBindingRecord, TestBindingsDocument, TestLocation,
};
use domain::tddd::test_obligation::drift::{NonEmptyDrifts, TestObligationDrift};
use domain::tddd::test_obligation::errors::ObligationCheckError;
use domain::tddd::test_obligation::hashes::{
    BoundTestsSetHash, DeclarationHash, VerifierPromptFingerprint,
};
use domain::tddd::test_obligation::ids::{
    NonEmptyEdgeIds, TestObligationBrief, TestObligationEdgeId, TestObligationId,
};
use domain::tddd::test_obligation::obligations::{ObligationsDocument, TestObligation};
use domain::tddd::test_obligation::ports::{
    ObligationsArtifactPort, TestBindingsArtifactPort, TestObligationRulesLoaderPort,
    TestSourceScannerPort, WaiverCachePort,
};
use domain::tddd::test_obligation::projection::RoleObligationItemsProjector;
use domain::tddd::test_obligation::verdict::{
    ObligationFulfillmentCacheDocument, ObligationFulfillmentCacheKey,
    ObligationFulfillmentVerdict, WaiverCacheDocument,
};

use domain::SpecDocumentLoaderPort;

use crate::catalogue_document_loader::AttestedCatalogueDocumentLoaderPort;
use crate::pre_review_gate::{ImplPlanReaderPort, TaskContractReaderPort};

pub use super::check_contract::{
    CheckTestObligationsApplicationService, CheckTestObligationsCommand,
    CheckTestObligationsOutcome,
};
mod input;
mod validation;
#[path = "check_waiver.rs"]
mod waiver;

use super::check_support::{
    GateState, SpecElement, active_cited_edges_from_catalogues, compute_uncited_from,
    edge_is_derived, edge_is_known, fulfillment_tests, synthetic_edge,
    synthetic_voluntary_obligation_id, voluntary_tests, waived_reason,
};
use super::derive::derive_obligations_document;
use super::freshness::{responsibility_hash, spec_element_hash};
use super::results::TestObligationStatusLaneSummary;
use super::status_lanes::{
    StatusLaneFindingKind, StatusLaneTarget, TaskStatusAttributor, tally_findings,
    target_for_direct_edge, target_for_obligation, targets_for_scope,
};
use super::{
    LoadedCatalogueDocument, declaration_with_obligation_context, diag,
    find_declaration_text_from_loaded, obligation_declaration_text_from_loaded,
    sha256_content_hash, synthetic_voluntary_obligation_brief,
};

use super::ports::ObligationFulfillmentCachePort;
use input::{has_catalogue, load_catalogues, spec_elements};
use validation::validate_voluntary_bindings;

/// Interactor implementing [`CheckTestObligationsApplicationService`] (IN-08).
pub struct CheckTestObligationsInteractor {
    rules_loader: Arc<dyn TestObligationRulesLoaderPort + Send + Sync>,
    obligations_port: Arc<dyn ObligationsArtifactPort + Send + Sync>,
    bindings_port: Arc<dyn TestBindingsArtifactPort + Send + Sync>,
    pub(super) source_scanner: Arc<dyn TestSourceScannerPort + Send + Sync>,
    fulfillment_cache: Arc<dyn ObligationFulfillmentCachePort + Send + Sync>,
    waiver_cache: Arc<dyn WaiverCachePort + Send + Sync>,
    fulfillment_verifier_fingerprint: VerifierPromptFingerprint,
    waiver_verifier_fingerprint: VerifierPromptFingerprint,
    spec_reader: Arc<dyn SpecDocumentLoaderPort + Send + Sync>,
    catalogue_reader: Arc<dyn AttestedCatalogueDocumentLoaderPort + Send + Sync>,
    task_contract_reader: Arc<dyn TaskContractReaderPort>,
    impl_plan_reader: Arc<dyn ImplPlanReaderPort>,
}

impl CheckTestObligationsInteractor {
    /// Builds a [`CheckTestObligationsInteractor`] from its injected ports.
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        rules_loader: Arc<dyn TestObligationRulesLoaderPort + Send + Sync>,
        obligations_port: Arc<dyn ObligationsArtifactPort + Send + Sync>,
        bindings_port: Arc<dyn TestBindingsArtifactPort + Send + Sync>,
        source_scanner: Arc<dyn TestSourceScannerPort + Send + Sync>,
        fulfillment_cache: Arc<dyn ObligationFulfillmentCachePort + Send + Sync>,
        waiver_cache: Arc<dyn WaiverCachePort + Send + Sync>,
        fulfillment_verifier_fingerprint: VerifierPromptFingerprint,
        waiver_verifier_fingerprint: VerifierPromptFingerprint,
        spec_reader: Arc<dyn SpecDocumentLoaderPort + Send + Sync>,
        catalogue_reader: Arc<dyn AttestedCatalogueDocumentLoaderPort + Send + Sync>,
        task_contract_reader: Arc<dyn TaskContractReaderPort>,
        impl_plan_reader: Arc<dyn ImplPlanReaderPort>,
    ) -> Self {
        Self {
            rules_loader,
            obligations_port,
            bindings_port,
            source_scanner,
            fulfillment_cache,
            waiver_cache,
            fulfillment_verifier_fingerprint,
            waiver_verifier_fingerprint,
            spec_reader,
            catalogue_reader,
            task_contract_reader,
            impl_plan_reader,
        }
    }
}

impl CheckTestObligationsApplicationService for CheckTestObligationsInteractor {
    fn execute(
        &self,
        cmd: &CheckTestObligationsCommand,
    ) -> Result<CheckTestObligationsOutcome, ObligationCheckError> {
        // `check` is pure-read (IN-08) and carries no active-branch guard: it may
        // run on any branch (e.g. CI on a detached HEAD), unlike the write-side
        // `derive` / `evaluate` commands.

        // Fail-closed rules-load gate (IN-08): the decision-table config must
        // load and validate before any downstream stage runs, so a malformed or
        // role-incomplete `.harness/config/test-obligation-rules.json` cannot let
        // the gate silently pass on stale obligations / bindings / caches.
        let rules_document = self.rules_loader.load().map_err(ObligationCheckError::RulesLoad)?;

        let obligations = self
            .obligations_port
            .load(cmd.input.track_id())
            .map_err(ObligationCheckError::ArtifactCodec)?;
        let bindings = self
            .bindings_port
            .load(cmd.input.track_id())
            .map_err(ObligationCheckError::ArtifactCodec)?;

        // Catalogue-bearing tracks require enrollment; catalogue-free tracks are empty.
        let (obligations, bindings) = match (obligations, bindings) {
            (None, None) if !has_catalogue(self.catalogue_reader.as_ref(), cmd)? => {
                return Ok(CheckTestObligationsOutcome::new_empty_scope(Vec::new()));
            }
            (None, None) => return Err(ObligationCheckError::ObligationsAbsent),
            (Some(_), None) => return Err(ObligationCheckError::BindingsAbsent),
            (None, Some(_)) => return Err(ObligationCheckError::ObligationsAbsent),
            (Some(obligations), Some(bindings)) => (obligations, bindings),
        };

        validate_voluntary_bindings(&obligations, &bindings)
            .map_err(ObligationCheckError::BindingConsistency)?;

        let catalogues = load_catalogues(self.catalogue_reader.as_ref(), cmd)?;
        let elements = spec_elements(self.spec_reader.as_ref(), cmd.input.track_id())?;
        let derivation_catalogues = catalogues
            .iter()
            .map(|catalogue| (catalogue.read_path().to_path_buf(), catalogue.document().clone()))
            .collect::<Vec<_>>();
        let expected = derive_obligations_document(
            cmd.input.track_id().clone(),
            &rules_document,
            &derivation_catalogues,
            &RoleObligationItemsProjector::new(),
        )
        .map_err(ObligationCheckError::InvalidCatalogueState)?;
        if let Some(detail) = obligations.staleness_against(&expected) {
            return Err(ObligationCheckError::StaleObligationsArtifact { detail });
        }
        let uncited = compute_uncited_from(&catalogues, &elements);
        let cited_edges = active_cited_edges_from_catalogues(&catalogues)?;
        let spec_elements = elements;

        let fulfillment = self
            .fulfillment_cache
            .load(cmd.input.track_id())
            .map_err(ObligationCheckError::CacheIo)?
            .ok_or(ObligationCheckError::FulfillmentCacheRequiresEvaluation)?;
        let waiver = self
            .waiver_cache
            .load(cmd.input.track_id())
            .map_err(ObligationCheckError::CacheIo)?
            .unwrap_or_else(|| WaiverCacheDocument::new(cmd.input.track_id().clone(), Vec::new()));

        let mut gate = GateState::default();
        self.detect_orphaned(&obligations, &bindings, &cited_edges, &mut gate);
        self.resolve_edges(
            &obligations,
            &bindings,
            &cited_edges,
            &catalogues,
            &spec_elements,
            &fulfillment,
            &waiver,
            &mut gate,
        )?;

        let targets = targets_for_scope(&obligations, &cited_edges, &catalogues)
            .map_err(ObligationCheckError::TaskAttribution)?;
        let attributor = TaskStatusAttributor::load(
            self.task_contract_reader.as_ref(),
            self.impl_plan_reader.as_ref(),
            cmd.input.track_id(),
            &targets,
        )
        .map_err(ObligationCheckError::TaskAttribution)?;
        let status_lane_summaries = tally_findings(&attributor, &gate.status_findings())
            .map_err(ObligationCheckError::TaskAttribution)?
            .into_iter()
            .map(|tally| {
                TestObligationStatusLaneSummary::new(
                    tally.task_status(),
                    tally.missing_count(),
                    tally.stale_count(),
                    tally.verdict_absent_count(),
                )
            })
            .collect();

        // Detection above remains status-independent. Only this final
        // interpretation defers todo-attributed unresolved findings.
        let mut blocking_drifts = gate.structural_drifts;
        for (drift, finding) in gate.status_drifts {
            if attributor
                .status_for(finding.target())
                .map_err(ObligationCheckError::TaskAttribution)?
                != TaskStatusKind::Todo
            {
                blocking_drifts.push(drift);
            }
        }
        if let Ok(drifts) = NonEmptyDrifts::try_new(blocking_drifts) {
            return Err(ObligationCheckError::DriftsDetected { drifts });
        }
        let blocking_unresolved = gate
            .unresolved
            .into_iter()
            .map(|(edge, finding)| {
                attributor
                    .status_for(finding.target())
                    .map(|status| (edge, status))
                    .map_err(ObligationCheckError::TaskAttribution)
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .filter_map(|(edge, status)| (status != TaskStatusKind::Todo).then_some(edge))
            .collect::<Vec<_>>();
        if let Ok(edges) = NonEmptyEdgeIds::try_new(blocking_unresolved) {
            return Err(ObligationCheckError::UnresolvedEdges { edges });
        }
        let blocking_verdict_absent = gate
            .verdict_absent
            .into_iter()
            .map(|(edge, finding)| {
                attributor
                    .status_for(finding.target())
                    .map(|status| (edge, status))
                    .map_err(ObligationCheckError::TaskAttribution)
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .filter_map(|(edge, status)| (status != TaskStatusKind::Todo).then_some(edge))
            .collect::<Vec<_>>();
        if let Ok(edges) = NonEmptyEdgeIds::try_new(blocking_verdict_absent) {
            return Err(ObligationCheckError::StaleVerdicts { edges });
        }
        Ok(CheckTestObligationsOutcome::new_verified_scope(
            gate.resolved,
            uncited,
            status_lane_summaries,
        ))
    }
}

impl CheckTestObligationsInteractor {
    fn detect_orphaned(
        &self,
        obligations: &ObligationsDocument,
        bindings: &TestBindingsDocument,
        cited_edges: &[TestObligationEdgeId],
        gate: &mut GateState,
    ) {
        for record in bindings.records() {
            match record {
                TestBindingRecord::Fulfillment { obligation_id, .. } => {
                    if !obligations.obligations().iter().any(|o| o.id() == obligation_id) {
                        gate.structural_drift(TestObligationDrift::orphaned_edge(
                            synthetic_edge(obligation_id),
                            diag("binding references an obligation that is no longer derived"),
                        ));
                    }
                }
                TestBindingRecord::Waiver { edge_id, .. } => {
                    if !edge_is_known(obligations, cited_edges, edge_id) {
                        gate.structural_drift(TestObligationDrift::orphaned_edge(
                            edge_id.clone(),
                            diag("waiver references an edge that is no longer derived"),
                        ));
                    }
                }
                TestBindingRecord::VoluntaryBinding { edge_id, .. } => {
                    if !edge_is_known(obligations, cited_edges, edge_id) {
                        gate.structural_drift(TestObligationDrift::orphaned_edge(
                            edge_id.clone(),
                            diag("voluntary binding references an edge that is no longer cited"),
                        ));
                    }
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn resolve_edges(
        &self,
        obligations: &ObligationsDocument,
        bindings: &TestBindingsDocument,
        cited_edges: &[TestObligationEdgeId],
        catalogues: &[LoadedCatalogueDocument],
        spec_elements: &[SpecElement],
        fulfillment: &ObligationFulfillmentCacheDocument,
        waiver: &WaiverCacheDocument,
        gate: &mut GateState,
    ) -> Result<(), ObligationCheckError> {
        for obligation in obligations.obligations() {
            let target = target_for_obligation(catalogues, obligation)
                .map_err(ObligationCheckError::TaskAttribution)?;
            let edges: Vec<TestObligationEdgeId> = obligation
                .spec_refs()
                .iter()
                .map(|anchor| {
                    TestObligationEdgeId::new(obligation.id().entry_key().clone(), anchor.clone())
                })
                .collect();
            let fulfilled = fulfillment_tests(bindings, obligation.id());
            if edges.is_empty() {
                // D1: an obligation that owns no anchors is not a fulfillment
                // target. A fulfillment record would pass the presence check
                // while evaluate plans zero verifier actions.
                if fulfilled.is_some() {
                    gate.structural_drift(TestObligationDrift::orphaned_edge(
                        synthetic_edge(obligation.id()),
                        diag(
                            "fulfillment binds an obligation that owns no anchors; omit the record or use voluntary/waiver on a cited edge",
                        ),
                    ));
                }
                continue;
            }
            let any_voluntary = edges.iter().any(|edge| voluntary_tests(bindings, edge).is_some());
            let any_waived = edges.iter().any(|edge| waived_reason(bindings, edge).is_some());

            if fulfilled.is_none() && !any_voluntary && !any_waived {
                gate.status_drift(
                    TestObligationDrift::missing_obligation(
                        obligation.id().clone(),
                        diag("obligation has no fulfillment or waiver binding"),
                    ),
                    target.clone(),
                    StatusLaneFindingKind::Missing,
                );
                continue;
            }

            for edge in edges {
                if let Some(reason) = waived_reason(bindings, &edge) {
                    self.resolve_waiver_edge(
                        &edge,
                        obligation,
                        &reason,
                        &target,
                        catalogues,
                        spec_elements,
                        waiver,
                        gate,
                    )?;
                } else if let Some(tests) = fulfilled {
                    self.resolve_fulfillment_edge(
                        &edge,
                        obligation,
                        tests,
                        &target,
                        catalogues,
                        spec_elements,
                        fulfillment,
                        gate,
                    )?;
                } else if let Some(tests) = voluntary_tests(bindings, &edge) {
                    self.resolve_fulfillment_edge(
                        &edge,
                        obligation,
                        tests,
                        &target,
                        catalogues,
                        spec_elements,
                        fulfillment,
                        gate,
                    )?;
                } else {
                    gate.unresolved(edge, target.clone());
                }
            }
        }
        for edge in cited_edges {
            if edge_is_derived(obligations, edge) {
                continue;
            }
            let target = target_for_direct_edge(catalogues, edge)
                .map_err(ObligationCheckError::TaskAttribution)?;
            if let Some(reason) = waived_reason(bindings, edge) {
                self.resolve_direct_waiver_edge(
                    edge,
                    &reason,
                    &target,
                    catalogues,
                    spec_elements,
                    waiver,
                    gate,
                )?;
            } else if let Some(tests) = voluntary_tests(bindings, edge) {
                self.resolve_direct_fulfillment_edge(
                    edge,
                    tests,
                    &target,
                    catalogues,
                    spec_elements,
                    fulfillment,
                    gate,
                )?;
            } else {
                gate.unresolved(edge.clone(), target);
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn resolve_fulfillment_edge(
        &self,
        edge: &TestObligationEdgeId,
        obligation: &TestObligation,
        tests: &[TestLocation],
        target: &StatusLaneTarget,
        catalogues: &[LoadedCatalogueDocument],
        spec_elements: &[SpecElement],
        fulfillment: &ObligationFulfillmentCacheDocument,
        gate: &mut GateState,
    ) -> Result<(), ObligationCheckError> {
        let declaration = declaration_with_obligation_context(
            &obligation_declaration_text_from_loaded(catalogues, obligation).unwrap_or_default(),
            obligation.id(),
            obligation.brief(),
        );
        self.resolve_fulfillment_cache_entry(
            edge,
            obligation.id(),
            &declaration,
            obligation.brief(),
            tests,
            target,
            spec_elements,
            fulfillment,
            gate,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn resolve_direct_fulfillment_edge(
        &self,
        edge: &TestObligationEdgeId,
        tests: &[TestLocation],
        target: &StatusLaneTarget,
        catalogues: &[LoadedCatalogueDocument],
        spec_elements: &[SpecElement],
        fulfillment: &ObligationFulfillmentCacheDocument,
        gate: &mut GateState,
    ) -> Result<(), ObligationCheckError> {
        let obligation_id = synthetic_voluntary_obligation_id(edge);
        let obligation_brief = synthetic_voluntary_obligation_brief(edge).map_err(|error| {
            ObligationCheckError::InvalidCatalogueState(diag(&format!(
                "invalid voluntary obligation brief for {edge:?}: {error}"
            )))
        })?;
        let declaration = declaration_with_obligation_context(
            &find_declaration_text_from_loaded(catalogues, edge.entry_key().as_str())
                .unwrap_or_default(),
            &obligation_id,
            &obligation_brief,
        );
        self.resolve_fulfillment_cache_entry(
            edge,
            &obligation_id,
            &declaration,
            &obligation_brief,
            tests,
            target,
            spec_elements,
            fulfillment,
            gate,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn resolve_fulfillment_cache_entry(
        &self,
        edge: &TestObligationEdgeId,
        obligation_id: &TestObligationId,
        declaration: &str,
        obligation_brief: &TestObligationBrief,
        tests: &[TestLocation],
        target: &StatusLaneTarget,
        spec_elements: &[SpecElement],
        fulfillment: &ObligationFulfillmentCacheDocument,
        gate: &mut GateState,
    ) -> Result<(), ObligationCheckError> {
        let Some(current_bound) = self.current_bound_hash(tests)? else {
            return self.classify_unavailable_fulfillment_verdict(
                edge,
                obligation_id,
                tests,
                target,
                gate,
            );
        };
        let current_key = ObligationFulfillmentCacheKey::new(
            BoundTestsSetHash::new(current_bound),
            DeclarationHash::new(sha256_content_hash(declaration.as_bytes())),
            spec_element_hash(spec_elements, edge),
            responsibility_hash(obligation_id, obligation_brief),
        );
        let entry = fulfillment
            .lookup_current(
                edge,
                obligation_id,
                &current_key,
                &self.fulfillment_verifier_fingerprint,
            )
            .map_err(ObligationCheckError::FulfillmentCacheLookup)?;
        let Some(entry) = entry else {
            let mut bound_changed = false;
            let mut declaration_changed = false;
            let mut spec_element_changed = false;
            let mut responsibility_changed = false;
            for candidate in fulfillment.entries().iter().filter(|candidate| {
                candidate.edge_id() == edge
                    && candidate.obligation_id() == obligation_id
                    && candidate.verifier_fingerprint()
                        == Some(&self.fulfillment_verifier_fingerprint)
            }) {
                bound_changed |=
                    candidate.key().bound_tests_set_hash() != current_key.bound_tests_set_hash();
                declaration_changed |=
                    candidate.key().declaration_hash() != current_key.declaration_hash();
                spec_element_changed |=
                    candidate.key().spec_element_hash() != current_key.spec_element_hash();
                responsibility_changed |=
                    candidate.key().responsibility_hash() != current_key.responsibility_hash();
            }
            let mut record_stale = |changed, drift| {
                if changed {
                    gate.status_drift(drift, target.clone(), StatusLaneFindingKind::Stale);
                }
            };
            record_stale(
                bound_changed,
                TestObligationDrift::test_changed_edge(
                    edge.clone(),
                    diag("bound test bodies changed since the verdict was frozen"),
                ),
            );
            record_stale(
                declaration_changed,
                TestObligationDrift::decl_changed_edge(
                    edge.clone(),
                    diag("entry declaration changed since the verdict was frozen"),
                ),
            );
            record_stale(
                spec_element_changed,
                TestObligationDrift::spec_changed_edge(
                    edge.clone(),
                    diag("specification element changed since the verdict was frozen"),
                ),
            );
            record_stale(
                responsibility_changed,
                TestObligationDrift::decl_changed_edge(
                    edge.clone(),
                    diag("obligation responsibility changed since the verdict was frozen"),
                ),
            );
            if !bound_changed
                && !declaration_changed
                && !spec_element_changed
                && !responsibility_changed
            {
                gate.verdict_absent(edge.clone(), target.clone());
            }
            return Ok(());
        };
        if matches!(entry.verdict(), ObligationFulfillmentVerdict::Fulfilled { .. }) {
            gate.resolved.push(edge.clone());
        } else {
            gate.verdict_absent(edge.clone(), target.clone());
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "check_tests.rs"]
mod tests;
