//! Transactional cache persistence for evaluate (fulfillment + waiver).
//!
//! Binding-record dispatch that used to live here has moved into
//! [`super::plan`]; only the two-step save and its compensation still need
//! both cache ports, so they stay as a method on the interactor.

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
    /// The fulfillment cache is committed last because its `ReevaluationRequired`
    /// load state is the fail-closed gate. If that final save fails, the waiver
    /// cache is restored to its prior document before the error is returned.
    pub(super) fn save_caches(
        &self,
        track_id: &TrackId,
        fulfillment_entries: Vec<ObligationFulfillmentCacheEntry>,
        waiver_entries: Vec<WaiverCacheEntry>,
    ) -> Result<(), ObligationEvaluateError> {
        let previous_waiver =
            self.waiver_cache.load(track_id).map_err(ObligationEvaluateError::CachePersistence)?;
        let fulfillment_document =
            ObligationFulfillmentCacheDocument::new(track_id.clone(), fulfillment_entries);
        let waiver_document = WaiverCacheDocument::new(track_id.clone(), waiver_entries);

        self.waiver_cache
            .save(&waiver_document)
            .map_err(|m| ObligationEvaluateError::CachePersistence(VerifyCacheError::Io(m)))?;
        if let Err(message) = self.fulfillment_cache.save(&fulfillment_document) {
            let rollback_document = previous_waiver
                .unwrap_or_else(|| WaiverCacheDocument::new(track_id.clone(), Vec::new()));
            if let Err(rollback) = self.waiver_cache.save(&rollback_document) {
                return Err(ObligationEvaluateError::CachePersistence(VerifyCacheError::Io(diag(
                    &format!(
                        "failed to save fulfillment cache: {}; rollback of waiver cache failed: {}",
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
