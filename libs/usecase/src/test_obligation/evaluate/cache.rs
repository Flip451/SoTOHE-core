//! Verdict-cache freshness lookups.

use domain::tddd::test_obligation::hashes::VerifierPromptFingerprint;
use domain::tddd::test_obligation::ids::{TestObligationEdgeId, TestObligationId};
use domain::tddd::test_obligation::verdict::{
    ObligationFulfillmentCacheDocument, ObligationFulfillmentCacheKey,
    ObligationFulfillmentVerdict, WaiverCacheDocument, WaiverCacheKey, WaiverVerdict,
};

/// Returns a frozen fulfillment verdict only when the edge, obligation, and
/// complete three-hash cache key and verifier fingerprint still match the
/// current inputs. A cached `Pending` verdict is never replayed: it records an
/// unadjudicated pair (e.g. a transient verifier failure), so treating it as
/// frozen would deadlock the pair — the pair must re-verify instead.
pub(super) fn cached_fulfillment_verdict(
    cache: Option<&ObligationFulfillmentCacheDocument>,
    edge_id: &TestObligationEdgeId,
    obligation_id: &TestObligationId,
    key: &ObligationFulfillmentCacheKey,
    verifier_fingerprint: &VerifierPromptFingerprint,
) -> Option<ObligationFulfillmentVerdict> {
    cache?
        .entries()
        .iter()
        .find(|entry| {
            entry.edge_id() == edge_id
                && entry.obligation_id() == obligation_id
                && entry.key() == key
                && entry.verifier_fingerprint() == Some(verifier_fingerprint)
                && !matches!(entry.verdict(), ObligationFulfillmentVerdict::Pending)
        })
        .map(|entry| entry.verdict().clone())
}

/// Returns a frozen waiver verdict only when the edge and complete three-hash
/// cache key and verifier fingerprint still match the current inputs. A cached
/// `Pending` verdict is never replayed (same re-verify rule as fulfillment).
pub(super) fn cached_waiver_verdict(
    cache: Option<&WaiverCacheDocument>,
    edge_id: &TestObligationEdgeId,
    key: &WaiverCacheKey,
    verifier_fingerprint: &VerifierPromptFingerprint,
) -> Option<WaiverVerdict> {
    cache?
        .entries()
        .iter()
        .find(|entry| {
            entry.edge_id() == edge_id
                && entry.key() == key
                && entry.verifier_fingerprint() == Some(verifier_fingerprint)
                && !matches!(entry.verdict(), WaiverVerdict::Pending)
        })
        .map(|entry| entry.verdict().clone())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use domain::tddd::semantic_verify::CatalogueEntryKey;
    use domain::tddd::test_obligation::hashes::{
        AnchorTextHash, BoundTestsSetHash, DeclarationHash, WaivedReasonHash,
    };
    use domain::tddd::test_obligation::ids::{
        TestObligationAnchorId, TestObligationItemIdentifier,
    };
    use domain::tddd::test_obligation::verdict::{
        ObligationFulfillmentCacheDocument, ObligationFulfillmentCacheEntry, WaiverCacheDocument,
        WaiverCacheEntry,
    };
    use domain::tddd::test_obligation::vocab::TestObligationKind;
    use domain::{ContentHash, EvidenceCitation, TrackId, ValidationError};

    use super::*;

    fn hash(byte: u8) -> ContentHash {
        ContentHash::from_bytes([byte; 32])
    }

    fn edge() -> TestObligationEdgeId {
        TestObligationEdgeId::new(
            CatalogueEntryKey::try_new("Money".to_owned()).unwrap(),
            TestObligationAnchorId::try_new("spec.json".to_owned(), "IN-05".to_owned()).unwrap(),
        )
    }

    fn obligation() -> TestObligationId {
        TestObligationId::new(
            CatalogueEntryKey::try_new("Money".to_owned()).unwrap(),
            TestObligationKind::Boundary,
            TestObligationItemIdentifier::try_new("voluntary:IN-05".to_owned()).unwrap(),
        )
    }

    fn fulfillment_key() -> ObligationFulfillmentCacheKey {
        ObligationFulfillmentCacheKey::new(
            BoundTestsSetHash::new(hash(1)),
            DeclarationHash::new(hash(2)),
            AnchorTextHash::new(hash(3)),
        )
    }

    fn waiver_key() -> WaiverCacheKey {
        WaiverCacheKey::new(
            WaivedReasonHash::new(hash(5)),
            DeclarationHash::new(hash(2)),
            AnchorTextHash::new(hash(3)),
        )
    }

    fn fingerprint() -> VerifierPromptFingerprint {
        VerifierPromptFingerprint::new(hash(7))
    }

    fn track() -> TrackId {
        TrackId::try_new("cache-tests").unwrap()
    }

    fn fulfillment_cache(
        verdict: ObligationFulfillmentVerdict,
    ) -> ObligationFulfillmentCacheDocument {
        ObligationFulfillmentCacheDocument::new(
            track(),
            vec![ObligationFulfillmentCacheEntry::new(
                edge(),
                obligation(),
                fulfillment_key(),
                verdict,
                Some(fingerprint()),
            )],
        )
    }

    /// A frozen `Pending` records an unadjudicated pair; replaying it as a
    /// cache hit would deadlock the pair, so the lookup must miss.
    #[test]
    fn test_cached_fulfillment_pending_verdict_is_not_replayed() {
        let cache = fulfillment_cache(ObligationFulfillmentVerdict::Pending);
        let found = cached_fulfillment_verdict(
            Some(&cache),
            &edge(),
            &obligation(),
            &fulfillment_key(),
            &fingerprint(),
        );
        assert!(found.is_none(), "cached Pending must re-verify, not replay: {found:?}");
    }

    #[test]
    fn test_cached_fulfillment_adjudicated_verdict_is_replayed() -> Result<(), ValidationError> {
        let verdict = ObligationFulfillmentVerdict::Fulfilled {
            citation: EvidenceCitation::try_new("asserts the rejection".to_owned())?,
        };
        let cache = fulfillment_cache(verdict.clone());
        let found = cached_fulfillment_verdict(
            Some(&cache),
            &edge(),
            &obligation(),
            &fulfillment_key(),
            &fingerprint(),
        );
        assert_eq!(found, Some(verdict), "adjudicated verdicts stay frozen");
        Ok(())
    }

    #[test]
    fn test_cached_waiver_pending_verdict_is_not_replayed() {
        let cache = WaiverCacheDocument::new(
            track(),
            vec![WaiverCacheEntry::new(
                edge(),
                waiver_key(),
                WaiverVerdict::Pending,
                Some(fingerprint()),
            )],
        );
        let found = cached_waiver_verdict(Some(&cache), &edge(), &waiver_key(), &fingerprint());
        assert!(found.is_none(), "cached Pending waiver must re-verify: {found:?}");
    }
}
