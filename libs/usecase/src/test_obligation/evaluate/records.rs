//! Transactional cache persistence for evaluate (fulfillment + waiver).
//!
//! Binding-record dispatch that used to live here has moved into
//! [`super::plan`]; only the two-step save (fulfillment first, waiver second
//! with a fulfillment rollback on waiver failure) still needs both cache
//! ports and the previous-document snapshot, so it stays as a method on the
//! interactor.

use domain::TrackId;
use domain::tddd::test_obligation::errors::{ObligationEvaluateError, VerifyCacheError};
use domain::tddd::test_obligation::verdict::{
    ObligationFulfillmentCacheDocument, ObligationFulfillmentCacheEntry, WaiverCacheDocument,
    WaiverCacheEntry,
};

use super::EvaluateTestObligationsInteractor;
use crate::test_obligation::diag;

impl EvaluateTestObligationsInteractor {
    /// Persists both verdict cache documents for the evaluated scope.
    ///
    /// If the fulfillment save succeeds and the waiver save then fails, the
    /// fulfillment cache is rewound to its previous document so `results`
    /// cannot observe a half-applied verdict set.
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
}
