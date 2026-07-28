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
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use domain::tddd::LayerId;
    use domain::tddd::test_obligation::binding::{NonEmptyTestLocations, TestLocation};
    use domain::tddd::test_obligation::errors::TestSourceScanError;
    use domain::tddd::test_obligation::hashes::{
        AnchorTextHash, DeclarationHash, TestBodySpanHash,
    };
    use domain::tddd::test_obligation::ids::{TestFunctionName, TestModulePath};
    use domain::tddd::test_obligation::ports::TestSourceScannerPort;
    use domain::tddd::test_obligation::verdict::ObligationFulfillmentCacheKey;
    use usecase::test_obligation::bound_tests::ResolvedBoundTestsResolver;

    struct Scanner;

    impl TestSourceScannerPort for Scanner {
        fn scan_test_body(
            &self,
            _location: &TestLocation,
        ) -> Result<Option<String>, TestSourceScanError> {
            Ok(Some("assert!(resolved);".to_owned()))
        }

        fn hash_test_body(&self, source: &str) -> TestBodySpanHash {
            TestBodySpanHash::new(Sha256ContentHasher::new().sha256(source.as_bytes()))
        }
    }

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

    #[test]
    fn test_sha256_content_hasher_hashes_resolved_bound_test_source() {
        let location = TestLocation::new(
            LayerId::try_new("infrastructure".to_owned()).unwrap(),
            TestModulePath::try_new("fixture".to_owned()).unwrap(),
            TestFunctionName::try_new("entry".to_owned()).unwrap(),
        );
        let resolution = ResolvedBoundTestsResolver::new(
            Arc::new(Scanner),
            Arc::new(Sha256ContentHasher::new()),
        )
        .resolve(NonEmptyTestLocations::new(location, Vec::new()))
        .unwrap();

        assert_eq!(
            resolution.set_hash().as_hash(),
            &Sha256ContentHasher::new().sha256(b"assert!(resolved);\n")
        );
    }

    #[test]
    fn test_sha256_content_hasher_with_different_diagnostics_uses_same_cache_key() {
        let first_location = TestLocation::new(
            LayerId::try_new("infrastructure".to_owned()).unwrap(),
            TestModulePath::try_new("fixture::first".to_owned()).unwrap(),
            TestFunctionName::try_new("entry_first".to_owned()).unwrap(),
        );
        let second_location = TestLocation::new(
            LayerId::try_new("infrastructure".to_owned()).unwrap(),
            TestModulePath::try_new("fixture::second".to_owned()).unwrap(),
            TestFunctionName::try_new("entry_second".to_owned()).unwrap(),
        );
        let resolver = ResolvedBoundTestsResolver::new(
            Arc::new(Scanner),
            Arc::new(Sha256ContentHasher::new()),
        );
        let first_resolution =
            resolver.resolve(NonEmptyTestLocations::new(first_location, Vec::new())).unwrap();
        let second_resolution =
            resolver.resolve(NonEmptyTestLocations::new(second_location, Vec::new())).unwrap();

        assert_ne!(first_resolution.locations(), second_resolution.locations());
        assert_eq!(first_resolution.set_hash(), second_resolution.set_hash());

        let hasher = Sha256ContentHasher::new();
        let declaration_hash = DeclarationHash::new(hasher.sha256(b"declaration"));
        let anchor_hash = AnchorTextHash::new(hasher.sha256(b"anchor"));
        let first_key = ObligationFulfillmentCacheKey::new(
            first_resolution.set_hash().clone(),
            declaration_hash.clone(),
            anchor_hash.clone(),
        );
        let second_key = ObligationFulfillmentCacheKey::new(
            second_resolution.set_hash().clone(),
            declaration_hash,
            anchor_hash,
        );

        assert_eq!(first_key, second_key);
    }
}
