//! Pure-read totality and drift gate for `bin/sotp test-obligation check`.

// The catalogue contract requires unboxed non-empty error payloads.
#![allow(clippy::result_large_err)]

use std::path::PathBuf;
use std::sync::Arc;

use domain::TaskStatusKind;
use domain::TrackId;
use domain::tddd::catalogue_v2::catalogue_impl_signals_ports::{
    CatalogueDocumentLoaderError, CatalogueDocumentLoaderPort,
};
use domain::tddd::test_obligation::binding::{
    TestBindingRecord, TestBindingsDocument, TestLocation,
};
use domain::tddd::test_obligation::drift::{NonEmptyDrifts, TestObligationDrift};
use domain::tddd::test_obligation::errors::ObligationCheckError;
use domain::tddd::test_obligation::hashes::{
    AnchorTextHash, BoundTestsSetHash, DeclarationHash, VerifierPromptFingerprint,
};
use domain::tddd::test_obligation::ids::{
    NonEmptyEdgeIds, TestObligationEdgeId, TestObligationId, WaivedReason,
};
use domain::tddd::test_obligation::obligations::{ObligationsDocument, TestObligation};
use domain::tddd::test_obligation::ports::{
    ObligationFulfillmentCachePort, ObligationsArtifactPort, TestBindingsArtifactPort,
    TestObligationRulesLoaderPort, TestSourceScannerPort, WaiverCachePort,
};
use domain::tddd::test_obligation::projection::RoleObligationItemsProjector;
use domain::tddd::test_obligation::verdict::{
    ObligationFulfillmentCacheDocument, ObligationFulfillmentCacheKey,
    ObligationFulfillmentVerdict, WaiverCacheDocument, WaiverVerdict,
};

use domain::SpecDocumentLoaderPort;

use crate::pre_review_gate::{ImplPlanReaderPort, TaskContractReaderPort};

pub use super::check_contract::{
    CheckTestObligationsApplicationService, CheckTestObligationsCommand,
    CheckTestObligationsOutcome,
};
use super::check_support::{
    GateState, SpecElement, active_cited_edges_from_catalogues, anchor_text, anchor_texts,
    compute_uncited_from, edge_is_derived, edge_is_known, fulfillment_tests,
    spec_elements_from_document, synthetic_edge, synthetic_voluntary_obligation_id,
    voluntary_tests, waived_reason,
};
use super::derive::derive_obligations_document;
use super::results::TestObligationStatusLaneSummary;
use super::status_lanes::{
    StatusLaneFindingKind, StatusLaneTarget, TaskStatusAttributor, tally_findings,
    target_for_direct_edge, target_for_obligation, targets_for_scope,
};
use super::{
    LoadedCatalogueDocument, diag, find_declaration_text_from_loaded,
    obligation_declaration_text_from_loaded, sha256_content_hash,
};

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
    catalogue_reader: Arc<dyn CatalogueDocumentLoaderPort + Send + Sync>,
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
        catalogue_reader: Arc<dyn CatalogueDocumentLoaderPort + Send + Sync>,
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

    fn load_catalogues(
        &self,
        cmd: &CheckTestObligationsCommand,
    ) -> Result<Vec<LoadedCatalogueDocument>, ObligationCheckError> {
        let mut catalogues = Vec::with_capacity(cmd.input.catalogue_paths().len());
        for path in cmd.input.catalogue_paths() {
            let doc =
                self.catalogue_reader.load(path).map_err(ObligationCheckError::CatalogueLoad)?;
            catalogues.push(LoadedCatalogueDocument::new(path, doc));
        }
        Ok(catalogues)
    }

    fn has_catalogue(
        &self,
        cmd: &CheckTestObligationsCommand,
    ) -> Result<bool, ObligationCheckError> {
        cmd.input.catalogue_paths().iter().try_fold(false, |exists, path| {
            match self.catalogue_reader.load(path) {
                Ok(_) => Ok(true),
                Err(CatalogueDocumentLoaderError::NotFound { .. }) => Ok(exists),
                Err(error) => Err(ObligationCheckError::CatalogueLoad(error)),
            }
        })
    }

    fn spec_elements(&self, track_id: &TrackId) -> Result<Vec<SpecElement>, ObligationCheckError> {
        let spec_path = PathBuf::from(format!("track/items/{}/spec.json", track_id.as_ref()));
        let spec = self.spec_reader.load(&spec_path).map_err(ObligationCheckError::SpecLoad)?;
        Ok(spec_elements_from_document(&spec))
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
            (None, None) if !self.has_catalogue(cmd)? => {
                return Ok(CheckTestObligationsOutcome::new_empty_scope(Vec::new()));
            }
            (None, None) => return Err(ObligationCheckError::ObligationsAbsent),
            (Some(_), None) => return Err(ObligationCheckError::BindingsAbsent),
            (None, Some(_)) => return Err(ObligationCheckError::ObligationsAbsent),
            (Some(obligations), Some(bindings)) => (obligations, bindings),
        };

        let catalogues = self.load_catalogues(cmd)?;
        let elements = self.spec_elements(cmd.input.track_id())?;
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
        let spec_texts = anchor_texts(&elements);

        let fulfillment = self
            .fulfillment_cache
            .load(cmd.input.track_id())
            .map_err(ObligationCheckError::CacheIo)?
            .unwrap_or_else(|| {
                ObligationFulfillmentCacheDocument::new(cmd.input.track_id().clone(), Vec::new())
            });
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
            &spec_texts,
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
        spec_texts: &[(String, String)],
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
                        &edge, obligation, &reason, &target, catalogues, spec_texts, waiver, gate,
                    );
                } else if let Some(tests) = fulfilled {
                    self.resolve_fulfillment_edge(
                        &edge,
                        obligation,
                        tests,
                        &target,
                        catalogues,
                        spec_texts,
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
                        spec_texts,
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
                    edge, &reason, &target, catalogues, spec_texts, waiver, gate,
                );
            } else if let Some(tests) = voluntary_tests(bindings, edge) {
                self.resolve_direct_fulfillment_edge(
                    edge,
                    tests,
                    &target,
                    catalogues,
                    spec_texts,
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
        spec_texts: &[(String, String)],
        fulfillment: &ObligationFulfillmentCacheDocument,
        gate: &mut GateState,
    ) -> Result<(), ObligationCheckError> {
        let declaration =
            obligation_declaration_text_from_loaded(catalogues, obligation).unwrap_or_default();
        self.resolve_fulfillment_cache_entry(
            edge,
            obligation.id(),
            &declaration,
            tests,
            target,
            spec_texts,
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
        spec_texts: &[(String, String)],
        fulfillment: &ObligationFulfillmentCacheDocument,
        gate: &mut GateState,
    ) -> Result<(), ObligationCheckError> {
        let obligation_id = synthetic_voluntary_obligation_id(edge);
        let declaration = find_declaration_text_from_loaded(catalogues, edge.entry_key().as_str())
            .unwrap_or_default();
        self.resolve_fulfillment_cache_entry(
            edge,
            &obligation_id,
            &declaration,
            tests,
            target,
            spec_texts,
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
        tests: &[TestLocation],
        target: &StatusLaneTarget,
        spec_texts: &[(String, String)],
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
            AnchorTextHash::new(sha256_content_hash(
                anchor_text(spec_texts, edge.anchor_id()).as_bytes(),
            )),
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
            let mut anchor_changed = false;
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
                anchor_changed |=
                    candidate.key().anchor_text_hash() != current_key.anchor_text_hash();
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
                anchor_changed,
                TestObligationDrift::spec_changed_edge(
                    edge.clone(),
                    diag("anchor text changed since the verdict was frozen"),
                ),
            );
            if !bound_changed && !declaration_changed && !anchor_changed {
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

    #[allow(clippy::too_many_arguments)]
    fn resolve_waiver_edge(
        &self,
        edge: &TestObligationEdgeId,
        obligation: &TestObligation,
        reason: &WaivedReason,
        target: &StatusLaneTarget,
        catalogues: &[LoadedCatalogueDocument],
        spec_texts: &[(String, String)],
        waiver: &WaiverCacheDocument,
        gate: &mut GateState,
    ) {
        let declaration =
            obligation_declaration_text_from_loaded(catalogues, obligation).unwrap_or_default();
        self.resolve_waiver_cache_entry(
            edge,
            obligation.id(),
            reason,
            &declaration,
            target,
            spec_texts,
            waiver,
            gate,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn resolve_direct_waiver_edge(
        &self,
        edge: &TestObligationEdgeId,
        reason: &WaivedReason,
        target: &StatusLaneTarget,
        catalogues: &[LoadedCatalogueDocument],
        spec_texts: &[(String, String)],
        waiver: &WaiverCacheDocument,
        gate: &mut GateState,
    ) {
        let declaration = find_declaration_text_from_loaded(catalogues, edge.entry_key().as_str())
            .unwrap_or_default();
        self.resolve_waiver_cache_entry(
            edge,
            &synthetic_voluntary_obligation_id(edge),
            reason,
            &declaration,
            target,
            spec_texts,
            waiver,
            gate,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn resolve_waiver_cache_entry(
        &self,
        edge: &TestObligationEdgeId,
        obligation_id: &TestObligationId,
        reason: &WaivedReason,
        declaration: &str,
        target: &StatusLaneTarget,
        spec_texts: &[(String, String)],
        waiver: &WaiverCacheDocument,
        gate: &mut GateState,
    ) {
        let Some(entry) = waiver
            .entries()
            .iter()
            .find(|entry| entry.edge_id() == edge && entry.obligation_id() == Some(obligation_id))
        else {
            gate.verdict_absent(edge.clone(), target.clone());
            return;
        };
        if entry.verifier_fingerprint() != Some(&self.waiver_verifier_fingerprint) {
            gate.verdict_absent(edge.clone(), target.clone());
            return;
        }
        let current_reason = sha256_content_hash(reason.as_str().as_bytes());
        let current_decl = sha256_content_hash(declaration.as_bytes());
        let current_anchor =
            sha256_content_hash(anchor_text(spec_texts, edge.anchor_id()).as_bytes());
        let key = entry.key();
        if key.waived_reason_hash().as_hash() != &current_reason {
            gate.status_drift(
                TestObligationDrift::reason_changed_edge(
                    edge.clone(),
                    diag("waived reason changed since the verdict was frozen"),
                ),
                target.clone(),
                StatusLaneFindingKind::Stale,
            );
        } else if key.declaration_hash().as_hash() != &current_decl {
            gate.status_drift(
                TestObligationDrift::decl_changed_edge(
                    edge.clone(),
                    diag("entry declaration changed since the verdict was frozen"),
                ),
                target.clone(),
                StatusLaneFindingKind::Stale,
            );
        } else if key.anchor_text_hash().as_hash() != &current_anchor {
            gate.status_drift(
                TestObligationDrift::spec_changed_edge(
                    edge.clone(),
                    diag("anchor text changed since the verdict was frozen"),
                ),
                target.clone(),
                StatusLaneFindingKind::Stale,
            );
        } else if matches!(entry.verdict(), WaiverVerdict::Waived { .. }) {
            gate.resolved.push(edge.clone());
        } else {
            gate.verdict_absent(edge.clone(), target.clone());
        }
    }
}

#[cfg(test)]
#[path = "check_tests.rs"]
mod tests;
