//! Usecase-owned secondary ports for fulfillment-cache recovery.

use domain::TrackId;
use domain::tddd::test_obligation::binding::NonEmptyTestLocations;
use domain::tddd::test_obligation::errors::{TestSourceScanError, VerifyCacheError};
use domain::tddd::test_obligation::ids::DiagnosticMessage;
use domain::tddd::test_obligation::verdict::ObligationFulfillmentCacheDocument;

use crate::test_obligation::bound_tests::ResolvedBoundTests;

/// Resolves locations to evidence derived from the exact scanned source.
pub trait ResolvedBoundTestsResolverPort: Send + Sync {
    /// Resolves the locations and derives source-coupled bound-test evidence.
    ///
    /// # Errors
    ///
    /// Returns a scan error when a referenced test cannot be read or found.
    fn resolve(
        &self,
        locations: NonEmptyTestLocations,
    ) -> Result<ResolvedBoundTests, TestSourceScanError>;
}

/// Loads and persists the obligation-fulfillment verdict cache for a track.
pub trait ObligationFulfillmentCachePort: Send + Sync {
    /// Loads the fulfillment cache for `track_id`, if it exists.
    ///
    /// # Errors
    ///
    /// Returns a typed cache error when the cache cannot be read or is malformed.
    fn load(
        &self,
        track_id: &TrackId,
    ) -> Result<Option<ObligationFulfillmentCacheDocument>, VerifyCacheError>;

    /// Persists an obligation-fulfillment cache document.
    ///
    /// # Errors
    ///
    /// Returns a diagnostic when the cache cannot be written.
    fn save(&self, doc: &ObligationFulfillmentCacheDocument) -> Result<(), DiagnosticMessage>;
}
