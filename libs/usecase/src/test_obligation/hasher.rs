//! Content-hashing secondary port for the obligation-fulfillment gate.
//!
//! [`ContentHasherPort`] freezes the three-component cache keys of the
//! fulfillment / waiver verdict caches (`anchor_text_hash`, `declaration_hash`,
//! `bound_tests_set_hash`) at evaluate time (D6 / IN-09 / AC-06 / CN-04).
//!
//! It lives in the usecase layer — not the domain — because content hashing is
//! an infrastructure service rather than a domain concept (hexagonal port
//! placement rule). The concrete SHA-256 adapter lives in the infrastructure
//! layer alongside the existing semantic-verify hashing helpers.

use domain::ContentHash;

/// Computes SHA-256 content hashes for the verdict-cache key freeze (IN-09 / AC-06).
pub trait ContentHasherPort: Send + Sync {
    /// Computes a SHA-256 [`ContentHash`] over `bytes`.
    fn sha256(&self, bytes: &[u8]) -> ContentHash;
}
