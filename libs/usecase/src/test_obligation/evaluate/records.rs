//! Binding-record dispatch and cache persistence for evaluate.

use domain::tddd::test_obligation::binding::TestBindingRecord;
use domain::tddd::test_obligation::errors::{ObligationEvaluateError, VerifyCacheError};
use domain::tddd::test_obligation::hashes::DeclarationHash;
use domain::tddd::test_obligation::ids::TestObligationEdgeId;
use domain::tddd::test_obligation::obligations::ObligationsDocument;
use domain::tddd::test_obligation::verdict::{
    ObligationFulfillmentCacheDocument, ObligationFulfillmentCacheEntry, WaiverCacheDocument,
    WaiverCacheEntry,
};
use domain::{SpecDocument, TrackId};

use super::edges::{
    find_obligation, find_obligation_for_edge, resolve_anchor_text, synthetic_obligation_id,
};
use super::verify::{
    record_pending_edge, record_pending_obligation_edges, record_pending_obligation_id,
};
use super::{EvaluateTestObligationsInteractor, Tally};
use crate::test_obligation::{
    LoadedCatalogueDocument, diag, find_declaration_text_from_loaded,
    obligation_declaration_text_from_loaded,
};

impl EvaluateTestObligationsInteractor {
    /// Persists both verdict cache documents for the evaluated scope.
    pub(super) fn save_caches(
        &self,
        track_id: &TrackId,
        fulfillment_entries: Vec<ObligationFulfillmentCacheEntry>,
        waiver_entries: Vec<WaiverCacheEntry>,
    ) -> Result<(), ObligationEvaluateError> {
        let previous_fulfillment = self
            .fulfillment_cache
            .load(track_id)
            .map_err(ObligationEvaluateError::CachePersistence)?;
        let fulfillment_document =
            ObligationFulfillmentCacheDocument::new(track_id.clone(), fulfillment_entries);
        let waiver_document = WaiverCacheDocument::new(track_id.clone(), waiver_entries);

        self.fulfillment_cache
            .save(&fulfillment_document)
            .map_err(|m| ObligationEvaluateError::CachePersistence(VerifyCacheError::Io(m)))?;
        if let Err(message) = self.waiver_cache.save(&waiver_document) {
            let rollback_document = previous_fulfillment.unwrap_or_else(|| {
                ObligationFulfillmentCacheDocument::new(track_id.clone(), Vec::new())
            });
            if let Err(rollback) = self.fulfillment_cache.save(&rollback_document) {
                return Err(ObligationEvaluateError::CachePersistence(VerifyCacheError::Io(diag(
                    &format!(
                        "failed to save waiver cache: {}; rollback of fulfillment cache failed: {}",
                        message.as_str(),
                        rollback.as_str()
                    ),
                ))));
            }
            return Err(ObligationEvaluateError::CachePersistence(VerifyCacheError::Io(message)));
        }
        Ok(())
    }

    /// Dispatches a single binding record to its lane evaluator.
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn evaluate_record(
        &self,
        record: &TestBindingRecord,
        obligations: &ObligationsDocument,
        catalogues: &[LoadedCatalogueDocument],
        spec: &SpecDocument,
        existing_fulfillment_cache: Option<&ObligationFulfillmentCacheDocument>,
        existing_waiver_cache: Option<&WaiverCacheDocument>,
        tally: &mut Tally,
        fulfillment_entries: &mut Vec<ObligationFulfillmentCacheEntry>,
        waiver_entries: &mut Vec<WaiverCacheEntry>,
    ) -> Result<(), ObligationEvaluateError> {
        match record {
            TestBindingRecord::Fulfillment { obligation_id, tests } => {
                let Some(obligation) = find_obligation(obligations, obligation_id) else {
                    record_pending_obligation_id(obligation_id, tally);
                    return Ok(());
                };
                let Some(declaration) =
                    obligation_declaration_text_from_loaded(catalogues, obligation)
                else {
                    record_pending_obligation_edges(obligation, tally);
                    return Ok(());
                };
                let declaration_hash =
                    DeclarationHash::new(self.hasher.sha256(declaration.as_bytes()));
                for anchor in obligation.spec_refs() {
                    let edge_id = TestObligationEdgeId::new(
                        obligation.id().entry_key().clone(),
                        anchor.clone(),
                    );
                    let Some(anchor_text) = resolve_anchor_text(spec, anchor.element_id()) else {
                        record_pending_edge(edge_id, tally);
                        continue;
                    };
                    self.evaluate_fulfillment(
                        edge_id,
                        obligation.id().clone(),
                        &declaration,
                        &anchor_text,
                        declaration_hash.clone(),
                        tests.as_slice(),
                        existing_fulfillment_cache,
                        tally,
                        fulfillment_entries,
                    )
                    .await?;
                }
                Ok(())
            }
            TestBindingRecord::VoluntaryBinding { edge_id, tests } => {
                if let Some(obligation) = find_obligation_for_edge(obligations, edge_id) {
                    let Some(declaration) =
                        obligation_declaration_text_from_loaded(catalogues, obligation)
                    else {
                        record_pending_edge(edge_id.clone(), tally);
                        return Ok(());
                    };
                    let Some(anchor_text) =
                        resolve_anchor_text(spec, edge_id.anchor_id().element_id())
                    else {
                        record_pending_edge(edge_id.clone(), tally);
                        return Ok(());
                    };
                    let declaration_hash =
                        DeclarationHash::new(self.hasher.sha256(declaration.as_bytes()));
                    return self
                        .evaluate_fulfillment(
                            edge_id.clone(),
                            obligation.id().clone(),
                            &declaration,
                            &anchor_text,
                            declaration_hash,
                            tests.as_slice(),
                            existing_fulfillment_cache,
                            tally,
                            fulfillment_entries,
                        )
                        .await;
                };

                let Some((declaration, anchor_text, declaration_hash)) =
                    self.resolve_edge(edge_id, catalogues, spec)
                else {
                    record_pending_edge(edge_id.clone(), tally);
                    return Ok(());
                };
                self.evaluate_fulfillment(
                    edge_id.clone(),
                    synthetic_obligation_id(edge_id),
                    &declaration,
                    &anchor_text,
                    declaration_hash,
                    tests.as_slice(),
                    existing_fulfillment_cache,
                    tally,
                    fulfillment_entries,
                )
                .await
            }
            TestBindingRecord::Waiver { edge_id, reason } => {
                let Some((declaration, anchor_text, declaration_hash)) =
                    self.resolve_waiver_edge(edge_id, obligations, catalogues, spec)
                else {
                    record_pending_edge(edge_id.clone(), tally);
                    return Ok(());
                };
                self.evaluate_waiver(
                    edge_id.clone(),
                    reason,
                    &declaration,
                    &anchor_text,
                    declaration_hash,
                    existing_waiver_cache,
                    tally,
                    waiver_entries,
                )
                .await
            }
        }
    }

    /// Resolves the verification triple for a waiver edge, preferring the
    /// derived obligation declaration when the edge still belongs to one.
    fn resolve_waiver_edge(
        &self,
        edge_id: &TestObligationEdgeId,
        obligations: &ObligationsDocument,
        catalogues: &[LoadedCatalogueDocument],
        spec: &SpecDocument,
    ) -> Option<(String, String, DeclarationHash)> {
        if let Some(obligation) = find_obligation_for_edge(obligations, edge_id) {
            let declaration = obligation_declaration_text_from_loaded(catalogues, obligation)?;
            let anchor_text = resolve_anchor_text(spec, edge_id.anchor_id().element_id())?;
            let declaration_hash = DeclarationHash::new(self.hasher.sha256(declaration.as_bytes()));
            return Some((declaration, anchor_text, declaration_hash));
        }
        self.resolve_edge(edge_id, catalogues, spec)
    }

    /// Resolves the `(declaration, anchor_text, declaration_hash)` triple for an
    /// edge that has no derived obligation (voluntary / waiver lanes).
    fn resolve_edge(
        &self,
        edge_id: &TestObligationEdgeId,
        catalogues: &[LoadedCatalogueDocument],
        spec: &SpecDocument,
    ) -> Option<(String, String, DeclarationHash)> {
        let declaration =
            find_declaration_text_from_loaded(catalogues, edge_id.entry_key().as_str())?;
        let anchor_text = resolve_anchor_text(spec, edge_id.anchor_id().element_id())?;
        let declaration_hash = DeclarationHash::new(self.hasher.sha256(declaration.as_bytes()));
        Some((declaration, anchor_text, declaration_hash))
    }
}
