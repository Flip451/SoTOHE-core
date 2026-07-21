//! Verdict vocabulary and hash-frozen verdict caches for the obligation gate.
//!
//! [`ObligationFulfillmentVerdict`] and [`WaiverVerdict`] are the per-edge
//! semantic-review outcomes: a passing verdict structurally carries an
//! [`EvidenceCitation`] so "pass without citation" is impossible, a fail carries
//! a reason (and, for fulfillment, a [`FulfillmentFailCategory`]), and `Pending`
//! is treated as fail at the gate. Verdicts are frozen against a three-component
//! cache key ([`ObligationFulfillmentCacheKey`] / [`WaiverCacheKey`]) plus the
//! verifier-prompt fingerprint that makes the recorded verdict valid. When any
//! key component or the prompt fingerprint changes, the entry is stale and
//! treated as absent, so the only recovery path is re-evaluation (IN-09 / IN-12 /
//! CN-04 / AC-05 / AC-06).

use crate::tddd::test_obligation::hashes::{
    AnchorTextHash, BoundTestsSetHash, DeclarationHash, VerifierPromptFingerprint, WaivedReasonHash,
};
use crate::tddd::test_obligation::ids::{
    DiagnosticMessage, TestObligationEdgeId, TestObligationId,
};
use crate::tddd::test_obligation::vocab::FulfillmentFailCategory;
use crate::{EvidenceCitation, TrackId, ValidationError};

/// Validated known-bad calibration-probe detection rate as a `0..=100` percentage.
///
/// Replaces a raw `NonZeroU8`, which wrongly rejected a legitimate `0%` detection
/// rate; this newtype admits the full `0..=100` range and rejects only `> 100`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectionRatePercent {
    value: u8,
}

impl DetectionRatePercent {
    /// Validates and wraps `value` as a [`DetectionRatePercent`].
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::InvalidDetectionRate`] when `value` exceeds 100.
    pub fn try_new(value: u8) -> Result<Self, ValidationError> {
        if value > 100 {
            return Err(ValidationError::InvalidDetectionRate(value));
        }
        Ok(Self { value })
    }

    /// Returns the inner percentage value (`0..=100`).
    #[must_use]
    pub fn value(&self) -> u8 {
        self.value
    }
}

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
    bound_tests_set_hash: BoundTestsSetHash,
    declaration_hash: DeclarationHash,
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

    /// Returns the bound-tests-set hash (claim side).
    #[must_use]
    pub fn bound_tests_set_hash(&self) -> &BoundTestsSetHash {
        &self.bound_tests_set_hash
    }

    /// Returns the entry-declaration hash (evidence side).
    #[must_use]
    pub fn declaration_hash(&self) -> &DeclarationHash {
        &self.declaration_hash
    }

    /// Returns the anchor-text hash (evidence side).
    #[must_use]
    pub fn anchor_text_hash(&self) -> &AnchorTextHash {
        &self.anchor_text_hash
    }
}

/// A single frozen obligation-fulfillment verdict entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObligationFulfillmentCacheEntry {
    edge_id: TestObligationEdgeId,
    obligation_id: TestObligationId,
    key: ObligationFulfillmentCacheKey,
    verdict: ObligationFulfillmentVerdict,
    verifier_fingerprint: Option<VerifierPromptFingerprint>,
}

impl ObligationFulfillmentCacheEntry {
    /// Builds an [`ObligationFulfillmentCacheEntry`].
    #[must_use]
    pub fn new(
        edge_id: TestObligationEdgeId,
        obligation_id: TestObligationId,
        key: ObligationFulfillmentCacheKey,
        verdict: ObligationFulfillmentVerdict,
        verifier_fingerprint: Option<VerifierPromptFingerprint>,
    ) -> Self {
        Self { edge_id, obligation_id, key, verdict, verifier_fingerprint }
    }

    /// Returns the obligation edge this verdict is frozen against.
    #[must_use]
    pub fn edge_id(&self) -> &TestObligationEdgeId {
        &self.edge_id
    }

    /// Returns the obligation this verdict concerns.
    #[must_use]
    pub fn obligation_id(&self) -> &TestObligationId {
        &self.obligation_id
    }

    /// Returns the three-component cache key freezing this verdict.
    #[must_use]
    pub fn key(&self) -> &ObligationFulfillmentCacheKey {
        &self.key
    }

    /// Returns the frozen fulfillment verdict.
    #[must_use]
    pub fn verdict(&self) -> &ObligationFulfillmentVerdict {
        &self.verdict
    }

    /// Returns the verifier-prompt fingerprint that produced this verdict.
    ///
    /// `None` denotes a legacy cache entry and is fail-closed by readers.
    #[must_use]
    pub fn verifier_fingerprint(&self) -> Option<&VerifierPromptFingerprint> {
        self.verifier_fingerprint.as_ref()
    }
}

/// Track-scoped collection of frozen obligation-fulfillment verdicts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObligationFulfillmentCacheDocument {
    track_id: TrackId,
    entries: Vec<ObligationFulfillmentCacheEntry>,
}

impl ObligationFulfillmentCacheDocument {
    /// Builds an [`ObligationFulfillmentCacheDocument`] for `track_id`.
    #[must_use]
    pub fn new(track_id: TrackId, entries: Vec<ObligationFulfillmentCacheEntry>) -> Self {
        Self { track_id, entries }
    }

    /// Returns the track this cache was frozen for.
    #[must_use]
    pub fn track_id(&self) -> &TrackId {
        &self.track_id
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
    waived_reason_hash: WaivedReasonHash,
    declaration_hash: DeclarationHash,
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

    /// Returns the waived-reason hash (claim side).
    #[must_use]
    pub fn waived_reason_hash(&self) -> &WaivedReasonHash {
        &self.waived_reason_hash
    }

    /// Returns the entry-declaration hash (evidence side).
    #[must_use]
    pub fn declaration_hash(&self) -> &DeclarationHash {
        &self.declaration_hash
    }

    /// Returns the anchor-text hash (evidence side).
    #[must_use]
    pub fn anchor_text_hash(&self) -> &AnchorTextHash {
        &self.anchor_text_hash
    }
}

/// A single frozen waiver verdict entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaiverCacheEntry {
    edge_id: TestObligationEdgeId,
    obligation_id: Option<TestObligationId>,
    key: WaiverCacheKey,
    verdict: WaiverVerdict,
    verifier_fingerprint: Option<VerifierPromptFingerprint>,
}

impl WaiverCacheEntry {
    /// Builds a [`WaiverCacheEntry`].
    #[must_use]
    pub fn new(
        edge_id: TestObligationEdgeId,
        obligation_id: Option<TestObligationId>,
        key: WaiverCacheKey,
        verdict: WaiverVerdict,
        verifier_fingerprint: Option<VerifierPromptFingerprint>,
    ) -> Self {
        Self { edge_id, obligation_id, key, verdict, verifier_fingerprint }
    }

    /// Returns the obligation edge this waiver verdict is frozen against.
    #[must_use]
    pub fn edge_id(&self) -> &TestObligationEdgeId {
        &self.edge_id
    }

    /// Returns the obligation that owns this edge adjudication.
    ///
    /// `None` denotes a legacy cache entry. Readers fail closed and require it
    /// to be re-evaluated before it can satisfy the gate.
    #[must_use]
    pub fn obligation_id(&self) -> Option<&TestObligationId> {
        self.obligation_id.as_ref()
    }

    /// Returns the three-component cache key freezing this verdict.
    #[must_use]
    pub fn key(&self) -> &WaiverCacheKey {
        &self.key
    }

    /// Returns the frozen waiver verdict.
    #[must_use]
    pub fn verdict(&self) -> &WaiverVerdict {
        &self.verdict
    }

    /// Returns the verifier-prompt fingerprint that produced this verdict.
    ///
    /// `None` denotes a legacy cache entry and is fail-closed by readers.
    #[must_use]
    pub fn verifier_fingerprint(&self) -> Option<&VerifierPromptFingerprint> {
        self.verifier_fingerprint.as_ref()
    }
}

/// Track-scoped collection of frozen waiver verdicts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaiverCacheDocument {
    track_id: TrackId,
    entries: Vec<WaiverCacheEntry>,
}

impl WaiverCacheDocument {
    /// Builds a [`WaiverCacheDocument`] for `track_id`.
    #[must_use]
    pub fn new(track_id: TrackId, entries: Vec<WaiverCacheEntry>) -> Self {
        Self { track_id, entries }
    }

    /// Returns the track this cache was frozen for.
    #[must_use]
    pub fn track_id(&self) -> &TrackId {
        &self.track_id
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

    #[test]
    fn detection_rate_accepts_full_percentage_range() {
        assert_eq!(DetectionRatePercent::try_new(0).unwrap().value(), 0);
        assert_eq!(DetectionRatePercent::try_new(100).unwrap().value(), 100);
    }

    #[test]
    fn detection_rate_rejects_above_hundred() {
        assert_eq!(
            DetectionRatePercent::try_new(101),
            Err(ValidationError::InvalidDetectionRate(101))
        );
    }

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

    fn verifier_fingerprint() -> VerifierPromptFingerprint {
        VerifierPromptFingerprint::new(ContentHash::from_bytes([5u8; 32]))
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
        let different_declaration = ObligationFulfillmentCacheKey::new(
            BoundTestsSetHash::new(ContentHash::from_bytes([1u8; 32])),
            DeclarationHash::new(ContentHash::from_bytes([8u8; 32])),
            AnchorTextHash::new(ContentHash::from_bytes([3u8; 32])),
        );
        let different_anchor = ObligationFulfillmentCacheKey::new(
            BoundTestsSetHash::new(ContentHash::from_bytes([1u8; 32])),
            DeclarationHash::new(ContentHash::from_bytes([2u8; 32])),
            AnchorTextHash::new(ContentHash::from_bytes([7u8; 32])),
        );
        assert_ne!(base, different_tests);
        assert_ne!(base, different_declaration);
        assert_ne!(base, different_anchor);
    }

    #[test]
    fn test_waiver_cache_key_changes_with_any_component() {
        let base = waiver_key();
        let different_reason = WaiverCacheKey::new(
            WaivedReasonHash::new(ContentHash::from_bytes([9u8; 32])),
            DeclarationHash::new(ContentHash::from_bytes([2u8; 32])),
            AnchorTextHash::new(ContentHash::from_bytes([3u8; 32])),
        );
        let different_declaration = WaiverCacheKey::new(
            WaivedReasonHash::new(ContentHash::from_bytes([1u8; 32])),
            DeclarationHash::new(ContentHash::from_bytes([8u8; 32])),
            AnchorTextHash::new(ContentHash::from_bytes([3u8; 32])),
        );
        let different_anchor = WaiverCacheKey::new(
            WaivedReasonHash::new(ContentHash::from_bytes([1u8; 32])),
            DeclarationHash::new(ContentHash::from_bytes([2u8; 32])),
            AnchorTextHash::new(ContentHash::from_bytes([7u8; 32])),
        );
        assert_ne!(base, different_reason);
        assert_ne!(base, different_declaration);
        assert_ne!(base, different_anchor);
    }

    #[test]
    fn test_fulfillment_cache_document_round_trips() {
        let fingerprint = verifier_fingerprint();
        let entry = ObligationFulfillmentCacheEntry::new(
            edge_id(),
            obligation_id(),
            fulfillment_key(),
            ObligationFulfillmentVerdict::Fulfilled { citation: citation("cite") },
            Some(fingerprint.clone()),
        );
        let doc = ObligationFulfillmentCacheDocument::new(
            TrackId::try_new("my-track").unwrap(),
            vec![entry],
        );
        assert_eq!(doc.entries().len(), 1);
        let [entry] = doc.entries() else {
            panic!("expected exactly one fulfillment cache entry");
        };
        assert_eq!(entry.verifier_fingerprint(), Some(&fingerprint));
    }

    #[test]
    fn test_waiver_cache_document_round_trips() {
        let fingerprint = verifier_fingerprint();
        let entry = WaiverCacheEntry::new(
            edge_id(),
            None,
            waiver_key(),
            WaiverVerdict::Waived { citation: citation("cite") },
            Some(fingerprint.clone()),
        );
        let doc = WaiverCacheDocument::new(TrackId::try_new("my-track").unwrap(), vec![entry]);
        assert_eq!(doc.entries().len(), 1);
        let [entry] = doc.entries() else {
            panic!("expected exactly one waiver cache entry");
        };
        assert_eq!(entry.verifier_fingerprint(), Some(&fingerprint));
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
