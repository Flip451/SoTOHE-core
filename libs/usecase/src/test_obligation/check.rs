//! `bin/sotp test-obligation check` — pure-read totality + drift gate.
//!
//! [`CheckTestObligationsInteractor`] deterministically verifies (IN-08): the
//! decision-table config loads and validates (fail-closed on malformed /
//! role-incomplete `.harness/config/test-obligation-rules.json` so `check`
//! cannot silently pass on stale obligations / bindings / caches — IN-08),
//! every derived obligation is bound and its tests exist (`missing` /
//! `orphaned` existence drift — IN-13), every ref edge resolves to a fulfilled
//! / waived verdict (totality — CN-02), and every resolving verdict is fresh
//! against the current three-hash key and verifier-prompt fingerprint
//! (`spec_changed` / `decl_changed` / `test_changed` / `reason_changed` freshness
//! drift — CN-04 / AC-04). A mismatched or absent fingerprint is treated as a
//! missing verdict. Scope is
//! resolved by artifact existence (IN-14 / AC-10): both absent passes with zero
//! pairs, a half-materialised scope is fail-closed. Uncited `AC` / `CN` spec
//! elements are surfaced as findings (IN-16 / AC-13). The gate never recomputes
//! freshness from source alone — recovery is `evaluate`'s job (CN-04).

// `ObligationCheckError` carries unboxed non-empty payloads (`NonEmptyDrifts` /
// `NonEmptyEdgeIds`) per the catalogue contract, which makes the `Err` variant
// large. Boxing would diverge from the declared type shape, so the size is
// accepted here rather than boxed.
#![allow(clippy::result_large_err)]

use std::path::PathBuf;
use std::sync::Arc;

use domain::ContentHash;
use domain::TrackId;
use domain::tddd::catalogue_v2::catalogue_impl_signals_ports::CatalogueDocumentLoaderPort;
use domain::tddd::test_obligation::binding::{
    TestBindingRecord, TestBindingsDocument, TestLocation,
};
use domain::tddd::test_obligation::drift::{NonEmptyDrifts, TestObligationDrift};
use domain::tddd::test_obligation::errors::{ObligationCheckError, TestSourceScanError};
use domain::tddd::test_obligation::hashes::VerifierPromptFingerprint;
use domain::tddd::test_obligation::ids::{
    NonEmptyEdgeIds, TestObligationEdgeId, TestObligationId, WaivedReason,
};
use domain::tddd::test_obligation::obligations::{ObligationsDocument, TestObligation};
use domain::tddd::test_obligation::ports::{
    ObligationFulfillmentCachePort, ObligationsArtifactPort, TestBindingsArtifactPort,
    TestObligationRulesLoaderPort, TestSourceScannerPort, WaiverCachePort,
};
use domain::tddd::test_obligation::projection::RoleObligationItemsProjector;
use domain::tddd::test_obligation::scope::UncitedSpecElementFinding;
use domain::tddd::test_obligation::verdict::{
    ObligationFulfillmentCacheDocument, ObligationFulfillmentVerdict, WaiverCacheDocument,
    WaiverVerdict,
};

use domain::SpecDocumentLoaderPort;

use super::check_support::{
    GateState, SpecElement, active_cited_edges_from_catalogues, anchor_text, anchor_texts,
    compute_uncited_from, edge_is_derived, edge_is_known, fulfillment_tests,
    spec_elements_from_document, synthetic_edge, synthetic_voluntary_obligation_id,
    voluntary_tests, waived_reason,
};
use super::derive::derive_obligations_document;
use super::{
    LoadedCatalogueDocument, TestObligationCatalogueCommandInput, diag,
    find_declaration_text_from_loaded, obligation_declaration_text_from_loaded,
    sha256_content_hash,
};

/// Command input for [`CheckTestObligationsApplicationService`] (IN-08).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckTestObligationsCommand {
    input: TestObligationCatalogueCommandInput,
}

impl CheckTestObligationsCommand {
    /// Builds a [`CheckTestObligationsCommand`].
    #[must_use]
    pub fn new(input: TestObligationCatalogueCommandInput) -> Self {
        Self { input }
    }
}

/// Structured output of a passing `check` (IN-08 / AC-04 / AC-10 / AC-13).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckTestObligationsOutcome {
    resolved_edges: Vec<TestObligationEdgeId>,
    uncited_findings: Vec<UncitedSpecElementFinding>,
}

impl CheckTestObligationsOutcome {
    /// Builds a verified-scope outcome (both artifacts present, all edges fresh).
    #[must_use]
    pub fn new_verified_scope(
        resolved_edges: Vec<TestObligationEdgeId>,
        uncited_findings: Vec<UncitedSpecElementFinding>,
    ) -> Self {
        Self { resolved_edges, uncited_findings }
    }

    /// Builds an empty-scope outcome (both artifacts absent — zero pairs).
    #[must_use]
    pub fn new_empty_scope(uncited_findings: Vec<UncitedSpecElementFinding>) -> Self {
        Self { resolved_edges: Vec::new(), uncited_findings }
    }

    /// Returns the edges resolved by a fresh fulfilled / waived verdict.
    #[must_use]
    pub fn resolved_edges(&self) -> &[TestObligationEdgeId] {
        &self.resolved_edges
    }

    /// Returns the uncited `AC` / `CN` spec-element findings.
    #[must_use]
    pub fn uncited_findings(&self) -> &[UncitedSpecElementFinding] {
        &self.uncited_findings
    }
}

/// Primary port for `bin/sotp test-obligation check` (IN-08 / AC-04 / AC-10).
pub trait CheckTestObligationsApplicationService {
    /// Runs the pure-read totality + drift gate.
    ///
    /// # Errors
    ///
    /// Returns [`ObligationCheckError`] when the branch is inactive, the scope is
    /// half-materialised, a drift is detected, an edge is unresolved, a verdict
    /// is stale, or an artifact / cache / source cannot be read.
    fn execute(
        &self,
        cmd: &CheckTestObligationsCommand,
    ) -> Result<CheckTestObligationsOutcome, ObligationCheckError>;
}

/// Interactor implementing [`CheckTestObligationsApplicationService`] (IN-08).
pub struct CheckTestObligationsInteractor {
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
        }
    }

    /// Loads the catalogues named in the command.
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

    /// Loads the parsed spec elements for the uncited-finding computation.
    fn spec_elements(&self, track_id: &TrackId) -> Result<Vec<SpecElement>, ObligationCheckError> {
        let spec_path = PathBuf::from(format!("track/items/{}/spec.json", track_id.as_ref()));
        let spec = self.spec_reader.load(&spec_path).map_err(ObligationCheckError::SpecLoad)?;
        Ok(spec_elements_from_document(&spec))
    }

    /// Hashes the current bound-test bodies for freshness comparison.
    fn current_bound_hash(
        &self,
        tests: &[TestLocation],
    ) -> Result<ContentHash, ObligationCheckError> {
        let mut source = String::new();
        for location in tests {
            let body = self
                .source_scanner
                .scan_test_body(location)
                .map_err(ObligationCheckError::SourceScan)?
                .ok_or_else(|| {
                    ObligationCheckError::SourceScan(TestSourceScanError::Io(diag(
                        "bound test source not found",
                    )))
                })?;
            source.push_str(&body);
            source.push('\n');
        }
        Ok(sha256_content_hash(source.as_bytes()))
    }

    /// Verifies that every bound test location still resolves in the worktree.
    fn bound_test_sources_exist(
        &self,
        tests: &[TestLocation],
    ) -> Result<bool, ObligationCheckError> {
        for location in tests {
            if self
                .source_scanner
                .scan_test_body(location)
                .map_err(ObligationCheckError::SourceScan)?
                .is_none()
            {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Classifies an unavailable fulfillment verdict after checking test existence.
    fn classify_unavailable_fulfillment_verdict(
        &self,
        edge: &TestObligationEdgeId,
        obligation_id: &TestObligationId,
        tests: &[TestLocation],
        gate: &mut GateState,
    ) -> Result<(), ObligationCheckError> {
        if self.bound_test_sources_exist(tests)? {
            gate.stale.push(edge.clone());
        } else {
            gate.drifts.push(TestObligationDrift::missing_obligation(
                obligation_id.clone(),
                diag("bound test source not found"),
            ));
        }
        Ok(())
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

        // Existence-based scope resolution (IN-14 / AC-10).
        let (obligations, bindings) = match (obligations, bindings) {
            (None, None) => return Ok(CheckTestObligationsOutcome::new_empty_scope(Vec::new())),
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

        // `try_new` yields the error only for a non-empty set, so each drift /
        // unresolved / stale gate fires exactly when it has findings (fail-closed).
        if let Ok(drifts) = NonEmptyDrifts::try_new(gate.drifts) {
            return Err(ObligationCheckError::DriftsDetected { drifts });
        }
        if let Ok(edges) = NonEmptyEdgeIds::try_new(gate.unresolved) {
            return Err(ObligationCheckError::UnresolvedEdges { edges });
        }
        if let Ok(edges) = NonEmptyEdgeIds::try_new(gate.stale) {
            return Err(ObligationCheckError::StaleVerdicts { edges });
        }
        Ok(CheckTestObligationsOutcome::new_verified_scope(gate.resolved, uncited))
    }
}

impl CheckTestObligationsInteractor {
    /// Flags bindings whose obligation / edge is no longer derived
    /// (`orphaned` existence drift — CN-11).
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
                        gate.drifts.push(TestObligationDrift::orphaned_edge(
                            synthetic_edge(obligation_id),
                            diag("binding references an obligation that is no longer derived"),
                        ));
                    }
                }
                TestBindingRecord::Waiver { edge_id, .. } => {
                    if !edge_is_known(obligations, cited_edges, edge_id) {
                        gate.drifts.push(TestObligationDrift::orphaned_edge(
                            edge_id.clone(),
                            diag("waiver references an edge that is no longer derived"),
                        ));
                    }
                }
                TestBindingRecord::VoluntaryBinding { edge_id, .. } => {
                    if !edge_is_known(obligations, cited_edges, edge_id) {
                        gate.drifts.push(TestObligationDrift::orphaned_edge(
                            edge_id.clone(),
                            diag("voluntary binding references an edge that is no longer cited"),
                        ));
                    }
                }
            }
        }
    }

    /// Resolves every derived edge to fresh / drifted / unresolved / stale.
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
                gate.drifts.push(TestObligationDrift::missing_obligation(
                    obligation.id().clone(),
                    diag("obligation has no fulfillment or waiver binding"),
                ));
                continue;
            }

            for edge in edges {
                if let Some(reason) = waived_reason(bindings, &edge) {
                    self.resolve_waiver_edge(
                        &edge, obligation, &reason, catalogues, spec_texts, waiver, gate,
                    );
                } else if let Some(tests) = fulfilled {
                    self.resolve_fulfillment_edge(
                        &edge,
                        obligation,
                        tests,
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
                        catalogues,
                        spec_texts,
                        fulfillment,
                        gate,
                    )?;
                } else {
                    gate.unresolved.push(edge);
                }
            }
        }
        for edge in cited_edges {
            if edge_is_derived(obligations, edge) {
                continue;
            }
            if let Some(reason) = waived_reason(bindings, edge) {
                self.resolve_direct_waiver_edge(
                    edge, &reason, catalogues, spec_texts, waiver, gate,
                );
            } else if let Some(tests) = voluntary_tests(bindings, edge) {
                self.resolve_direct_fulfillment_edge(
                    edge,
                    tests,
                    catalogues,
                    spec_texts,
                    fulfillment,
                    gate,
                )?;
            } else {
                gate.unresolved.push(edge.clone());
            }
        }
        Ok(())
    }

    /// Resolves a single fulfillment edge against the frozen verdict cache.
    #[allow(clippy::too_many_arguments)]
    fn resolve_fulfillment_edge(
        &self,
        edge: &TestObligationEdgeId,
        obligation: &TestObligation,
        tests: &[TestLocation],
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
            spec_texts,
            fulfillment,
            gate,
        )
    }

    /// Resolves a direct voluntary-binding edge against the frozen verdict cache.
    #[allow(clippy::too_many_arguments)]
    fn resolve_direct_fulfillment_edge(
        &self,
        edge: &TestObligationEdgeId,
        tests: &[TestLocation],
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
            spec_texts,
            fulfillment,
            gate,
        )
    }

    /// Resolves a fulfillment edge from its already-selected declaration text.
    #[allow(clippy::too_many_arguments)]
    fn resolve_fulfillment_cache_entry(
        &self,
        edge: &TestObligationEdgeId,
        obligation_id: &TestObligationId,
        declaration: &str,
        tests: &[TestLocation],
        spec_texts: &[(String, String)],
        fulfillment: &ObligationFulfillmentCacheDocument,
        gate: &mut GateState,
    ) -> Result<(), ObligationCheckError> {
        let Some(entry) = fulfillment
            .entries()
            .iter()
            .find(|e| e.edge_id() == edge && e.obligation_id() == obligation_id)
        else {
            return self.classify_unavailable_fulfillment_verdict(edge, obligation_id, tests, gate);
        };
        if entry.verifier_fingerprint() != Some(&self.fulfillment_verifier_fingerprint) {
            return self.classify_unavailable_fulfillment_verdict(edge, obligation_id, tests, gate);
        }
        let current_bound = self.current_bound_hash(tests)?;
        let current_decl = sha256_content_hash(declaration.as_bytes());
        let current_anchor =
            sha256_content_hash(anchor_text(spec_texts, edge.anchor_id()).as_bytes());
        let key = entry.key();
        if key.bound_tests_set_hash().as_hash() != &current_bound {
            gate.drifts.push(TestObligationDrift::test_changed_edge(
                edge.clone(),
                diag("bound test bodies changed since the verdict was frozen"),
            ));
        } else if key.declaration_hash().as_hash() != &current_decl {
            gate.drifts.push(TestObligationDrift::decl_changed_edge(
                edge.clone(),
                diag("entry declaration changed since the verdict was frozen"),
            ));
        } else if key.anchor_text_hash().as_hash() != &current_anchor {
            gate.drifts.push(TestObligationDrift::spec_changed_edge(
                edge.clone(),
                diag("anchor text changed since the verdict was frozen"),
            ));
        } else if matches!(entry.verdict(), ObligationFulfillmentVerdict::Fulfilled { .. }) {
            gate.resolved.push(edge.clone());
        } else {
            gate.stale.push(edge.clone());
        }
        Ok(())
    }

    /// Resolves a single waiver edge against the frozen verdict cache.
    #[allow(clippy::too_many_arguments)]
    fn resolve_waiver_edge(
        &self,
        edge: &TestObligationEdgeId,
        obligation: &TestObligation,
        reason: &WaivedReason,
        catalogues: &[LoadedCatalogueDocument],
        spec_texts: &[(String, String)],
        waiver: &WaiverCacheDocument,
        gate: &mut GateState,
    ) {
        let declaration =
            obligation_declaration_text_from_loaded(catalogues, obligation).unwrap_or_default();
        self.resolve_waiver_cache_entry(edge, reason, &declaration, spec_texts, waiver, gate);
    }

    /// Resolves a direct waived edge that has no derived obligation.
    #[allow(clippy::too_many_arguments)]
    fn resolve_direct_waiver_edge(
        &self,
        edge: &TestObligationEdgeId,
        reason: &WaivedReason,
        catalogues: &[LoadedCatalogueDocument],
        spec_texts: &[(String, String)],
        waiver: &WaiverCacheDocument,
        gate: &mut GateState,
    ) {
        let declaration = find_declaration_text_from_loaded(catalogues, edge.entry_key().as_str())
            .unwrap_or_default();
        self.resolve_waiver_cache_entry(edge, reason, &declaration, spec_texts, waiver, gate);
    }

    /// Resolves a waiver edge from its already-selected declaration text.
    fn resolve_waiver_cache_entry(
        &self,
        edge: &TestObligationEdgeId,
        reason: &WaivedReason,
        declaration: &str,
        spec_texts: &[(String, String)],
        waiver: &WaiverCacheDocument,
        gate: &mut GateState,
    ) {
        let Some(entry) = waiver.entries().iter().find(|e| e.edge_id() == edge) else {
            gate.stale.push(edge.clone());
            return;
        };
        if entry.verifier_fingerprint() != Some(&self.waiver_verifier_fingerprint) {
            gate.stale.push(edge.clone());
            return;
        }
        let current_reason = sha256_content_hash(reason.as_str().as_bytes());
        let current_decl = sha256_content_hash(declaration.as_bytes());
        let current_anchor =
            sha256_content_hash(anchor_text(spec_texts, edge.anchor_id()).as_bytes());
        let key = entry.key();
        if key.waived_reason_hash().as_hash() != &current_reason {
            gate.drifts.push(TestObligationDrift::reason_changed_edge(
                edge.clone(),
                diag("waived reason changed since the verdict was frozen"),
            ));
        } else if key.declaration_hash().as_hash() != &current_decl {
            gate.drifts.push(TestObligationDrift::decl_changed_edge(
                edge.clone(),
                diag("entry declaration changed since the verdict was frozen"),
            ));
        } else if key.anchor_text_hash().as_hash() != &current_anchor {
            gate.drifts.push(TestObligationDrift::spec_changed_edge(
                edge.clone(),
                diag("anchor text changed since the verdict was frozen"),
            ));
        } else if matches!(entry.verdict(), WaiverVerdict::Waived { .. }) {
            gate.resolved.push(edge.clone());
        } else {
            gate.stale.push(edge.clone());
        }
    }
}

#[cfg(test)]
#[path = "check_tests.rs"]
mod tests;
