//! Fragment reference types: `FragmentRef` and `DryCheckPairKey`.

use thiserror::Error;

use crate::review_v2::types::FilePath;

use super::value_objects::FragmentContentHash;

// ── FragmentRef ───────────────────────────────────────────────────────────────

/// Fragment identifier: the pair (repo-relative path, content_hash) uniquely
/// identifies a code fragment by both location and content (D8/IN-06/CN-07).
///
/// Two `FragmentRef`s are equal iff both `path` AND `content_hash` match — this
/// is the basis for self-match detection in `DryCheckPairKey::new()`.
/// `Ord` is lexicographic `(path, content_hash)` so `DryCheckPairKey::new()`
/// can sort two `FragmentRef`s into `(low, high)` deterministically.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FragmentRef {
    path: FilePath,
    content_hash: FragmentContentHash,
}

impl FragmentRef {
    /// Construct a [`FragmentRef`] (infallible — both components are already
    /// validated value objects).
    pub fn new(path: FilePath, content_hash: FragmentContentHash) -> FragmentRef {
        FragmentRef { path, content_hash }
    }

    /// Return the repo-relative file path.
    pub fn path(&self) -> &FilePath {
        &self.path
    }

    /// Return the SHA-256 content hash.
    pub fn content_hash(&self) -> &FragmentContentHash {
        &self.content_hash
    }
}

// ── DryCheckPairKey ───────────────────────────────────────────────────────────

/// Normalized (sorted) pair of [`FragmentRef`]s used as the dry-check dedup/cache key.
///
/// `low <= high` by `(path, content_hash)` lexicographic order, ensuring `(X,Y)`
/// and `(Y,X)` produce the same key (CN-08). Self-match (both refs equal) is
/// rejected at construction. Paths-different-hash-same (complete copies in
/// different files) is NOT a self-match and produces a valid pair.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DryCheckPairKey {
    low: FragmentRef,
    high: FragmentRef,
}

impl DryCheckPairKey {
    /// Construct a [`DryCheckPairKey`] from two [`FragmentRef`]s.
    ///
    /// Sorts `a` and `b` into `(low, high)` so `(X,Y)` and `(Y,X)` produce the
    /// same key. Rejects self-match when `a == b` on BOTH path AND content_hash.
    ///
    /// # Errors
    ///
    /// Returns [`DryCheckPairKeyError::SelfMatch`] when both refs are equal.
    pub fn new(a: FragmentRef, b: FragmentRef) -> Result<DryCheckPairKey, DryCheckPairKeyError> {
        if a == b {
            return Err(DryCheckPairKeyError::SelfMatch);
        }
        let (low, high) = if a <= b { (a, b) } else { (b, a) };
        Ok(DryCheckPairKey { low, high })
    }

    /// Return the lower [`FragmentRef`] in `(path, content_hash)` order.
    pub fn low(&self) -> &FragmentRef {
        &self.low
    }

    /// Return the higher [`FragmentRef`] in `(path, content_hash)` order.
    pub fn high(&self) -> &FragmentRef {
        &self.high
    }
}

/// Error from [`DryCheckPairKey::new`].
#[derive(Debug, Error)]
pub enum DryCheckPairKeyError {
    /// Both [`FragmentRef`] arguments are equal (path AND content_hash both match).
    #[error("self-match: both fragment refs are identical (same path and content_hash)")]
    SelfMatch,
}
