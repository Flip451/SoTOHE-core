//! Verdict vocabulary and hash-frozen verdict caches for the obligation gate.
//!
//! [`ObligationFulfillmentVerdict`] and [`WaiverVerdict`] are the per-edge
//! semantic-review outcomes: a passing verdict structurally carries an
//! [`EvidenceCitation`] so "pass without citation" is impossible, a fail carries
//! a reason (and, for fulfillment, a [`FulfillmentFailCategory`]), and `Pending`
//! is treated as fail at the gate. Verdicts are frozen against a three-component
//! cache key ([`ObligationFulfillmentCacheKey`] / [`WaiverCacheKey`]); when any
//! key component's hash changes the entry is stale and treated as absent, so the
//! only recovery path is re-evaluation (IN-09 / IN-12 / CN-04 / AC-05 / AC-06).

use crate::tddd::test_obligation::hashes::{
    AnchorTextHash, BoundTestsSetHash, DeclarationHash, WaivedReasonHash,
};
use crate::tddd::test_obligation::ids::{
    DiagnosticMessage, TestObligationEdgeId, TestObligationId,
};
use crate::tddd::test_obligation::vocab::FulfillmentFailCategory;
use crate::{EvidenceCitation, TrackId};

/// Outcome of semantically reviewing the tests bound to an obligation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObligationFulfillmentVerdict {
    /// The bound tests fulfill the obligation; `citation` quotes the evidence.
    Fulfilled {
        /// Verbatim quotation from the tests that fulfills the obligation.
        citation: EvidenceCitation,
    },
    /// The bound tests fail to fulfill the obligation.
    Fail {
        /// Which of the three fail categories the failure falls into.
        category: FulfillmentFailCategory,
        /// Human-readable description of the failure.
        reason: DiagnosticMessage,
    },
    /// The reviewer could not confirm fulfillment; treated as fail at the gate.
    Pending,
}

/// Outcome of semantically reviewing a waived obligation edge's reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WaiverVerdict {
    /// The waiver reason holds for the edge; `citation` quotes the evidence.
    Waived {
        /// Verbatim quotation supporting the waiver.
        citation: EvidenceCitation,
    },
    /// The waiver reason does not hold for the edge.
    Fail {
        /// Human-readable description of why the waiver was rejected.
        reason: DiagnosticMessage,
    },
    /// The reviewer could not confirm the waiver; treated as fail at the gate.
    Pending,
}

/// Three-component cache key freezing an obligation-fulfillment verdict.
///
/// Combines the bound-tests-set hash (claim side) with the declaration and
/// anchor-text hashes (evidence side). Any component changing produces a
/// different key, so the frozen verdict is no longer found (IN-09 / CN-04).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObligationFulfillmentCacheKey {
    // Read by the fulfillment cache codec (T012) when it (de)serializes cache
    // keys; no read accessors are declared in this batch's type contract.
    #[allow(dead_code)]
    bound_tests_set_hash: BoundTestsSetHash,
    #[allow(dead_code)]
    declaration_hash: DeclarationHash,
    #[allow(dead_code)]
    anchor_text_hash: AnchorTextHash,
}

impl ObligationFulfillmentCacheKey {
    /// Builds an [`ObligationFulfillmentCacheKey`] from its three hash components.
    #[must_use]
    pub fn new(
        bound_tests_set_hash: BoundTestsSetHash,
        declaration_hash: DeclarationHash,
        anchor_text_hash: AnchorTextHash,
    ) -> Self {
        Self { bound_tests_set_hash, declaration_hash, anchor_text_hash }
    }
}

/// A single frozen obligation-fulfillment verdict entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObligationFulfillmentCacheEntry {
    // Read by the fulfillment cache codec (T012); no read accessors are declared
    // in this batch's type contract.
    #[allow(dead_code)]
    edge_id: TestObligationEdgeId,
    #[allow(dead_code)]
    obligation_id: TestObligationId,
    #[allow(dead_code)]
    key: ObligationFulfillmentCacheKey,
    #[allow(dead_code)]
    verdict: ObligationFulfillmentVerdict,
}

impl ObligationFulfillmentCacheEntry {
    /// Builds an [`ObligationFulfillmentCacheEntry`].
    #[must_use]
    pub fn new(
        edge_id: TestObligationEdgeId,
        obligation_id: TestObligationId,
        key: ObligationFulfillmentCacheKey,
        verdict: ObligationFulfillmentVerdict,
    ) -> Self {
        Self { edge_id, obligation_id, key, verdict }
    }
}

/// Track-scoped collection of frozen obligation-fulfillment verdicts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObligationFulfillmentCacheDocument {
    // Read by the fulfillment cache codec (T012); no read accessor is declared in
    // this batch's type contract.
    #[allow(dead_code)]
    track_id: TrackId,
    entries: Vec<ObligationFulfillmentCacheEntry>,
}

impl ObligationFulfillmentCacheDocument {
    /// Builds an [`ObligationFulfillmentCacheDocument`] for `track_id`.
    #[must_use]
    pub fn new(track_id: TrackId, entries: Vec<ObligationFulfillmentCacheEntry>) -> Self {
        Self { track_id, entries }
    }

    /// Returns the frozen fulfillment verdict entries.
    #[must_use]
    pub fn entries(&self) -> &[ObligationFulfillmentCacheEntry] {
        &self.entries
    }
}

/// Three-component cache key freezing a waiver verdict.
///
/// Combines the waived-reason hash (claim side) with the declaration and
/// anchor-text hashes (evidence side). Any component changing produces a
/// different key, so the frozen verdict is no longer found (IN-09 / CN-04).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaiverCacheKey {
    // Read by the waiver cache codec (T012); no read accessors are declared in
    // this batch's type contract.
    #[allow(dead_code)]
    waived_reason_hash: WaivedReasonHash,
    #[allow(dead_code)]
    declaration_hash: DeclarationHash,
    #[allow(dead_code)]
    anchor_text_hash: AnchorTextHash,
}

impl WaiverCacheKey {
    /// Builds a [`WaiverCacheKey`] from its three hash components.
    #[must_use]
    pub fn new(
        waived_reason_hash: WaivedReasonHash,
        declaration_hash: DeclarationHash,
        anchor_text_hash: AnchorTextHash,
    ) -> Self {
        Self { waived_reason_hash, declaration_hash, anchor_text_hash }
    }
}

/// A single frozen waiver verdict entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaiverCacheEntry {
    // Read by the waiver cache codec (T012); no read accessors are declared in
    // this batch's type contract.
    #[allow(dead_code)]
    edge_id: TestObligationEdgeId,
    #[allow(dead_code)]
    key: WaiverCacheKey,
    #[allow(dead_code)]
    verdict: WaiverVerdict,
}

impl WaiverCacheEntry {
    /// Builds a [`WaiverCacheEntry`].
    #[must_use]
    pub fn new(edge_id: TestObligationEdgeId, key: WaiverCacheKey, verdict: WaiverVerdict) -> Self {
        Self { edge_id, key, verdict }
    }
}

/// Track-scoped collection of frozen waiver verdicts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaiverCacheDocument {
    // Read by the waiver cache codec (T012); no read accessor is declared in this
    // batch's type contract.
    #[allow(dead_code)]
    track_id: TrackId,
    entries: Vec<WaiverCacheEntry>,
}

impl WaiverCacheDocument {
    /// Builds a [`WaiverCacheDocument`] for `track_id`.
    #[must_use]
    pub fn new(track_id: TrackId, entries: Vec<WaiverCacheEntry>) -> Self {
        Self { track_id, entries }
    }

    /// Returns the frozen waiver verdict entries.
    #[must_use]
    pub fn entries(&self) -> &[WaiverCacheEntry] {
        &self.entries
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::ContentHash;
    use crate::tddd::semantic_verify::CatalogueEntryKey;
    use crate::tddd::test_obligation::ids::{TestObligationAnchorId, TestObligationItemIdentifier};
    use crate::tddd::test_obligation::vocab::TestObligationKind;

    fn citation(text: &str) -> EvidenceCitation {
        EvidenceCitation::try_new(text.to_owned()).unwrap()
    }

    fn diagnostic(text: &str) -> DiagnosticMessage {
        DiagnosticMessage::try_new(text.to_owned()).unwrap()
    }

    fn edge_id() -> TestObligationEdgeId {
        TestObligationEdgeId::new(
            CatalogueEntryKey::try_new("domain::User".to_owned()).unwrap(),
            TestObligationAnchorId::try_new("spec.json".to_owned(), "IN-09".to_owned()).unwrap(),
        )
    }

    fn obligation_id() -> TestObligationId {
        TestObligationId::new(
            CatalogueEntryKey::try_new("domain::User".to_owned()).unwrap(),
            TestObligationKind::Result,
            TestObligationItemIdentifier::try_new("entry".to_owned()).unwrap(),
        )
    }

    fn fulfillment_key() -> ObligationFulfillmentCacheKey {
        ObligationFulfillmentCacheKey::new(
            BoundTestsSetHash::new(ContentHash::from_bytes([1u8; 32])),
            DeclarationHash::new(ContentHash::from_bytes([2u8; 32])),
            AnchorTextHash::new(ContentHash::from_bytes([3u8; 32])),
        )
    }

    fn waiver_key() -> WaiverCacheKey {
        WaiverCacheKey::new(
            WaivedReasonHash::new(ContentHash::from_bytes([4u8; 32])),
            DeclarationHash::new(ContentHash::from_bytes([2u8; 32])),
            AnchorTextHash::new(ContentHash::from_bytes([3u8; 32])),
        )
    }

    #[test]
    fn test_fulfillment_verdict_variants() {
        let fulfilled = ObligationFulfillmentVerdict::Fulfilled {
            citation: citation("asserts empty rejected"),
        };
        let fail = ObligationFulfillmentVerdict::Fail {
            category: FulfillmentFailCategory::Contradiction,
            reason: diagnostic("asserts the opposite"),
        };
        assert_ne!(fulfilled, fail);
        assert_ne!(fail, ObligationFulfillmentVerdict::Pending);
    }

    #[test]
    fn test_waiver_verdict_variants() {
        let waived = WaiverVerdict::Waived { citation: citation("covered by integration suite") };
        let fail = WaiverVerdict::Fail { reason: diagnostic("reason does not hold") };
        assert_ne!(waived, fail);
        assert_ne!(fail, WaiverVerdict::Pending);
    }

    #[test]
    fn test_fulfillment_cache_key_changes_with_any_component() {
        let base = fulfillment_key();
        let different_tests = ObligationFulfillmentCacheKey::new(
            BoundTestsSetHash::new(ContentHash::from_bytes([9u8; 32])),
            DeclarationHash::new(ContentHash::from_bytes([2u8; 32])),
            AnchorTextHash::new(ContentHash::from_bytes([3u8; 32])),
        );
        assert_ne!(base, different_tests);
    }

    #[test]
    fn test_fulfillment_cache_document_round_trips() {
        let entry = ObligationFulfillmentCacheEntry::new(
            edge_id(),
            obligation_id(),
            fulfillment_key(),
            ObligationFulfillmentVerdict::Fulfilled { citation: citation("cite") },
        );
        let doc = ObligationFulfillmentCacheDocument::new(
            TrackId::try_new("my-track").unwrap(),
            vec![entry],
        );
        assert_eq!(doc.entries().len(), 1);
    }

    #[test]
    fn test_waiver_cache_document_round_trips() {
        let entry = WaiverCacheEntry::new(
            edge_id(),
            waiver_key(),
            WaiverVerdict::Waived { citation: citation("cite") },
        );
        let doc = WaiverCacheDocument::new(TrackId::try_new("my-track").unwrap(), vec![entry]);
        assert_eq!(doc.entries().len(), 1);
    }

    #[test]
    fn test_empty_caches_are_valid() {
        let fulfillment =
            ObligationFulfillmentCacheDocument::new(TrackId::try_new("t").unwrap(), vec![]);
        let waiver = WaiverCacheDocument::new(TrackId::try_new("t").unwrap(), vec![]);
        assert!(fulfillment.entries().is_empty());
        assert!(waiver.entries().is_empty());
    }
}
