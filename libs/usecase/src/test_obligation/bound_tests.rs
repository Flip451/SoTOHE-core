//! Source-derived fulfillment evidence resolution.

use std::fmt;
use std::sync::Arc;

use domain::tddd::test_obligation::binding::NonEmptyTestLocations;
use domain::tddd::test_obligation::errors::TestSourceScanError;
use domain::tddd::test_obligation::hashes::BoundTestsSetHash;
use domain::tddd::test_obligation::ids::unavailable_diagnostic_message;
use domain::tddd::test_obligation::ports::TestSourceScannerPort;

use super::hasher::ContentHasherPort;
use super::ports::ResolvedBoundTestsResolverPort;

/// Evidence derived from a single source scan of the bound test locations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedBoundTests {
    set_hash: BoundTestsSetHash,
    locations: NonEmptyTestLocations,
}

impl ResolvedBoundTests {
    /// Returns the hash calculated from the resolved test bodies.
    #[must_use]
    pub fn set_hash(&self) -> &BoundTestsSetHash {
        &self.set_hash
    }

    /// Returns the bound-test locations used to calculate the hash.
    #[must_use]
    pub fn locations(&self) -> &NonEmptyTestLocations {
        &self.locations
    }
}

/// Resolves test locations into source-derived bound-test evidence.
#[derive(Clone)]
pub struct ResolvedBoundTestsResolver {
    source_scanner: Arc<dyn TestSourceScannerPort + Send + Sync>,
    hasher: Arc<dyn ContentHasherPort + Send + Sync>,
}

impl fmt::Debug for ResolvedBoundTestsResolver {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("ResolvedBoundTestsResolver").finish_non_exhaustive()
    }
}

impl ResolvedBoundTestsResolver {
    /// Builds a resolver from the source scanner and bound-tests hasher.
    #[must_use]
    pub fn new(
        source_scanner: Arc<dyn TestSourceScannerPort + Send + Sync>,
        hasher: Arc<dyn ContentHasherPort + Send + Sync>,
    ) -> Self {
        Self { source_scanner, hasher }
    }

    /// Rebinds this resolver to the scanner selected by the application service.
    #[must_use]
    pub(super) fn with_source_scanner(
        &self,
        source_scanner: Arc<dyn TestSourceScannerPort + Send + Sync>,
    ) -> Self {
        Self { source_scanner, hasher: Arc::clone(&self.hasher) }
    }

    /// Scans every location and derives evidence from exactly that source.
    ///
    /// # Errors
    ///
    /// Returns a source-scan error when an input test cannot be read or is not
    /// present in the worktree.
    pub fn resolve(
        &self,
        locations: NonEmptyTestLocations,
    ) -> Result<ResolvedBoundTests, TestSourceScanError> {
        self.resolve_source(locations).map(|(resolved, _)| resolved)
    }

    pub(super) fn resolve_source(
        &self,
        locations: NonEmptyTestLocations,
    ) -> Result<(ResolvedBoundTests, String), TestSourceScanError> {
        let mut source = String::new();
        for location in locations.as_slice() {
            let body = self
                .source_scanner
                .scan_test_body(location)?
                .ok_or_else(|| TestSourceScanError::Io(unavailable_diagnostic_message()))?;
            source.push_str(&body);
            source.push('\n');
        }
        let set_hash = BoundTestsSetHash::new(self.hasher.sha256(source.as_bytes()));
        Ok((ResolvedBoundTests { set_hash, locations }, source))
    }
}

impl ResolvedBoundTestsResolverPort for ResolvedBoundTestsResolver {
    /// Scans every location and derives evidence from exactly that source.
    ///
    /// # Errors
    ///
    /// Returns a source-scan error when an input test cannot be read or is not
    /// present in the worktree.
    fn resolve(
        &self,
        locations: NonEmptyTestLocations,
    ) -> Result<ResolvedBoundTests, TestSourceScanError> {
        self.resolve(locations)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    use domain::ContentHash;
    use domain::tddd::LayerId;
    use domain::tddd::test_obligation::binding::TestLocation;
    use domain::tddd::test_obligation::hashes::TestBodySpanHash;
    use domain::tddd::test_obligation::ids::{TestFunctionName, TestModulePath};

    struct FixtureScanner;

    impl TestSourceScannerPort for FixtureScanner {
        fn scan_test_body(
            &self,
            _location: &TestLocation,
        ) -> Result<Option<String>, TestSourceScanError> {
            Ok(Some("assert!(scanned);".to_owned()))
        }

        fn hash_test_body(&self, source: &str) -> TestBodySpanHash {
            TestBodySpanHash::new(ContentHash::from_bytes([source.len() as u8; 32]))
        }
    }

    struct SourceLengthHasher;

    impl ContentHasherPort for SourceLengthHasher {
        fn sha256(&self, bytes: &[u8]) -> ContentHash {
            ContentHash::from_bytes([bytes.len() as u8; 32])
        }
    }

    #[test]
    fn test_resolved_bound_tests_resolver_with_scanned_source_returns_coupled_evidence() {
        let location = TestLocation::new(
            LayerId::try_new("usecase".to_owned()).unwrap(),
            TestModulePath::try_new("usecase::fixture::tests".to_owned()).unwrap(),
            TestFunctionName::try_new("test_scanned".to_owned()).unwrap(),
        );
        let resolver =
            ResolvedBoundTestsResolver::new(Arc::new(FixtureScanner), Arc::new(SourceLengthHasher));
        let port: &dyn ResolvedBoundTestsResolverPort = &resolver;

        let resolution =
            port.resolve(NonEmptyTestLocations::new(location.clone(), Vec::new())).unwrap();

        assert_eq!(resolution.locations().as_slice(), &[location]);
        assert_eq!(resolution.set_hash().as_hash(), &ContentHash::from_bytes([18; 32]));
    }

    #[test]
    fn test_resolved_bound_tests_resolver_with_missing_source_returns_unavailable_diagnostic() {
        struct MissingSourceScanner;

        impl TestSourceScannerPort for MissingSourceScanner {
            fn scan_test_body(
                &self,
                _location: &TestLocation,
            ) -> Result<Option<String>, TestSourceScanError> {
                Ok(None)
            }

            fn hash_test_body(&self, _source: &str) -> TestBodySpanHash {
                TestBodySpanHash::new(ContentHash::from_bytes([0; 32]))
            }
        }

        let location = TestLocation::new(
            LayerId::try_new("usecase".to_owned()).unwrap(),
            TestModulePath::try_new("usecase::fixture::tests".to_owned()).unwrap(),
            TestFunctionName::try_new("test_missing".to_owned()).unwrap(),
        );
        let resolver = ResolvedBoundTestsResolver::new(
            Arc::new(MissingSourceScanner),
            Arc::new(SourceLengthHasher),
        );

        assert!(matches!(
            resolver.resolve(NonEmptyTestLocations::new(location, Vec::new())),
            Err(TestSourceScanError::Io(message))
                if message.as_str() == "diagnostic detail unavailable"
        ));
    }
}
