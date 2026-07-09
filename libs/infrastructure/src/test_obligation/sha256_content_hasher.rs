//! SHA-256 content hasher for the test-obligation evaluation cache keys.

use domain::ContentHash;
use usecase::test_obligation::hasher::ContentHasherPort;

/// Concrete [`ContentHasherPort`] backed by the `sha2` crate.
#[derive(Debug, Clone)]
pub struct Sha256ContentHasher {
    _private: (),
}

impl Sha256ContentHasher {
    /// Builds a SHA-256 content hasher.
    #[must_use]
    pub fn new() -> Self {
        Self { _private: () }
    }
}

impl Default for Sha256ContentHasher {
    fn default() -> Self {
        Self::new()
    }
}

impl ContentHasherPort for Sha256ContentHasher {
    fn sha256(&self, bytes: &[u8]) -> ContentHash {
        use sha2::Digest as _;

        let mut hasher = sha2::Sha256::new();
        hasher.update(bytes);
        let mut out = [0u8; 32];
        out.copy_from_slice(&hasher.finalize());
        ContentHash::from_bytes(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sha256_content_hasher_with_empty_input_matches_sha256() {
        let hash = Sha256ContentHasher::new().sha256(b"");
        assert_eq!(
            hash.to_hex(),
            "e3b0c44298fc1c149afbf4c8996fb924\
             27ae41e4649b934ca495991b7852b855"
                .replace(' ', "")
        );
    }

    #[test]
    fn test_sha256_content_hasher_with_different_input_differs() {
        let hasher = Sha256ContentHasher::new();
        assert_ne!(hasher.sha256(b"a"), hasher.sha256(b"b"));
    }
}
