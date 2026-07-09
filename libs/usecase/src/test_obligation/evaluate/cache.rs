//! Three-hash verdict-cache freshness lookups.

use domain::tddd::test_obligation::ids::{TestObligationEdgeId, TestObligationId};
use domain::tddd::test_obligation::verdict::{
    ObligationFulfillmentCacheDocument, ObligationFulfillmentCacheKey,
    ObligationFulfillmentVerdict, WaiverCacheDocument, WaiverCacheKey, WaiverVerdict,
};

/// Returns a frozen fulfillment verdict only when the edge, obligation, and
/// complete three-hash cache key still match the current inputs.
pub(super) fn cached_fulfillment_verdict(
    cache: Option<&ObligationFulfillmentCacheDocument>,
    edge_id: &TestObligationEdgeId,
    obligation_id: &TestObligationId,
    key: &ObligationFulfillmentCacheKey,
) -> Option<ObligationFulfillmentVerdict> {
    cache?
        .entries()
        .iter()
        .find(|entry| {
            entry.edge_id() == edge_id
                && entry.obligation_id() == obligation_id
                && entry.key() == key
        })
        .map(|entry| entry.verdict().clone())
}

/// Returns a frozen waiver verdict only when the edge and complete three-hash
/// cache key still match the current inputs.
pub(super) fn cached_waiver_verdict(
    cache: Option<&WaiverCacheDocument>,
    edge_id: &TestObligationEdgeId,
    key: &WaiverCacheKey,
) -> Option<WaiverVerdict> {
    cache?
        .entries()
        .iter()
        .find(|entry| entry.edge_id() == edge_id && entry.key() == key)
        .map(|entry| entry.verdict().clone())
}
