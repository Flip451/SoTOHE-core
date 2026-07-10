//! Content-hash value objects for test-obligation verdict / fulfillment caches.
//!
//! Each newtype wraps a canonical [`ContentHash`] so a cache key can distinguish
//! *which* hashed input it stands for at the type level: anchor text, catalogue
//! declaration, bound-tests set, waiver reason, or a bound test's body span.
//! Keeping them distinct stops two structurally identical hashes from being
//! swapped by accident when a cache key is assembled (IN-05 / IN-09 / AC-06).

use crate::ContentHash;

/// Hash of the anchor text a test obligation is bound to (verdict cache key).
///
/// See AC-06.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnchorTextHash {
    hash: ContentHash,
}

impl AnchorTextHash {
    /// Wraps `hash` as an [`AnchorTextHash`].
    #[must_use]
    pub fn new(hash: ContentHash) -> Self {
        Self { hash }
    }

    /// Borrows the inner content hash.
    #[must_use]
    pub fn as_hash(&self) -> &ContentHash {
        &self.hash
    }
}

/// Hash of a catalogue entry's declaration (verdict cache key).
///
/// See IN-05 / AC-06.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclarationHash {
    hash: ContentHash,
}

impl DeclarationHash {
    /// Wraps `hash` as a [`DeclarationHash`].
    #[must_use]
    pub fn new(hash: ContentHash) -> Self {
        Self { hash }
    }

    /// Borrows the inner content hash.
    #[must_use]
    pub fn as_hash(&self) -> &ContentHash {
        &self.hash
    }
}

/// Hash of the set of tests bound to an obligation (fulfillment cache key).
///
/// See IN-09 / AC-06.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundTestsSetHash {
    hash: ContentHash,
}

impl BoundTestsSetHash {
    /// Wraps `hash` as a [`BoundTestsSetHash`].
    #[must_use]
    pub fn new(hash: ContentHash) -> Self {
        Self { hash }
    }

    /// Borrows the inner content hash.
    #[must_use]
    pub fn as_hash(&self) -> &ContentHash {
        &self.hash
    }
}

/// Hash of a waiver reason (waiver cache key).
///
/// See AC-06.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaivedReasonHash {
    hash: ContentHash,
}

impl WaivedReasonHash {
    /// Wraps `hash` as a [`WaivedReasonHash`].
    #[must_use]
    pub fn new(hash: ContentHash) -> Self {
        Self { hash }
    }

    /// Borrows the inner content hash.
    #[must_use]
    pub fn as_hash(&self) -> &ContentHash {
        &self.hash
    }
}

/// Hash of a bound test function's body span (fulfillment cache key).
///
/// See IN-09 / AC-06.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestBodySpanHash {
    hash: ContentHash,
}

impl TestBodySpanHash {
    /// Wraps `hash` as a [`TestBodySpanHash`].
    #[must_use]
    pub fn new(hash: ContentHash) -> Self {
        Self { hash }
    }

    /// Borrows the inner content hash.
    #[must_use]
    pub fn as_hash(&self) -> &ContentHash {
        &self.hash
    }
}

/// Hash of the judging-prompt preamble that produced a cached verdict.
///
/// This is a validity attribute of a verdict record rather than a cache-key
/// component. A changed verifier prompt must therefore invalidate the record
/// without changing the identity of the pair it judged (IN-09 / CN-04).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifierPromptFingerprint {
    hash: ContentHash,
}

impl VerifierPromptFingerprint {
    /// Wraps `hash` as a [`VerifierPromptFingerprint`].
    #[must_use]
    pub fn new(hash: ContentHash) -> Self {
        Self { hash }
    }

    /// Borrows the inner content hash.
    #[must_use]
    pub fn as_hash(&self) -> &ContentHash {
        &self.hash
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn sample_hash() -> ContentHash {
        ContentHash::from_bytes([7u8; 32])
    }

    #[test]
    fn test_anchor_text_hash_round_trips() {
        let hash = sample_hash();
        let wrapped = AnchorTextHash::new(hash.clone());
        assert_eq!(wrapped.as_hash(), &hash);
    }

    #[test]
    fn test_declaration_hash_round_trips() {
        let hash = sample_hash();
        let wrapped = DeclarationHash::new(hash.clone());
        assert_eq!(wrapped.as_hash(), &hash);
    }

    #[test]
    fn test_bound_tests_set_hash_round_trips() {
        let hash = sample_hash();
        let wrapped = BoundTestsSetHash::new(hash.clone());
        assert_eq!(wrapped.as_hash(), &hash);
    }

    #[test]
    fn test_waived_reason_hash_round_trips() {
        let hash = sample_hash();
        let wrapped = WaivedReasonHash::new(hash.clone());
        assert_eq!(wrapped.as_hash(), &hash);
    }

    #[test]
    fn test_test_body_span_hash_round_trips() {
        let hash = sample_hash();
        let wrapped = TestBodySpanHash::new(hash.clone());
        assert_eq!(wrapped.as_hash(), &hash);
    }

    #[test]
    fn test_verifier_prompt_fingerprint_round_trips() {
        let hash = sample_hash();
        let wrapped = VerifierPromptFingerprint::new(hash.clone());
        assert_eq!(wrapped.as_hash(), &hash);
    }

    #[test]
    fn test_distinct_wrappers_compare_by_inner_hash() {
        let a = AnchorTextHash::new(ContentHash::from_bytes([1u8; 32]));
        let b = AnchorTextHash::new(ContentHash::from_bytes([2u8; 32]));
        assert_ne!(a, b);
    }
}
