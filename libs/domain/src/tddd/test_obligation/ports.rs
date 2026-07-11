//! Domain ports for the test-obligation gate.
//!
//! Holds the driven ports the four use cases depend on. All are synchronous to
//! match the existing domain port baseline; concrete adapters (JSON codecs,
//! worktree scanners, LLM verifiers) live in the infrastructure layer.
//!
//! * [`TestObligationRulesLoaderPort`] — the decision-table config loader
//!   (IN-02 / IN-04 / AC-02).
//! * [`ObligationsArtifactPort`] / [`TestBindingsArtifactPort`] — artifact
//!   repositories (IN-05 / IN-06 / AC-03 / AC-04).
//! * [`ObligationFulfillmentCachePort`] / [`WaiverCachePort`] — verdict-cache
//!   persistence (IN-09 / CN-04 / AC-06).
//! * [`ObligationFulfillmentVerifierPort`] / [`WaiverVerifierPort`] — semantic
//!   judgement (IN-09 / IN-11 / CN-08 / AC-07).
//! * [`TestSourceScannerPort`] — worktree test-body evidence (IN-06 / IN-09).

use crate::tddd::test_obligation::binding::{TestBindingsDocument, TestLocation};
use crate::tddd::test_obligation::errors::{
    ArtifactCodecError, SemanticVerifierError, TestObligationRulesLoadError, TestSourceScanError,
    VerifyCacheError,
};
use crate::tddd::test_obligation::hashes::TestBodySpanHash;
use crate::tddd::test_obligation::ids::DiagnosticMessage;
use crate::tddd::test_obligation::obligations::ObligationsDocument;
use crate::tddd::test_obligation::rules::TestObligationRulesDocument;
use crate::tddd::test_obligation::verdict::{
    ObligationFulfillmentCacheDocument, ObligationFulfillmentVerdict, WaiverCacheDocument,
    WaiverVerdict,
};
use crate::{ModelTier, TrackId};

/// Loads and validates the test-obligation decision-table config.
///
/// Synchronous to match the existing domain port baseline. The implementation
/// performs the fail-closed load-time totality validation and returns the parsed
/// [`TestObligationRulesDocument`], or a [`TestObligationRulesLoadError`]
/// describing the first failure.
pub trait TestObligationRulesLoaderPort {
    /// Loads the decision table from the configured source.
    ///
    /// # Errors
    ///
    /// Returns a [`TestObligationRulesLoadError`] when the config cannot be read,
    /// is malformed, omits a role, leaves `obligations` implicit, or names an
    /// unknown role.
    fn load(&self) -> Result<TestObligationRulesDocument, TestObligationRulesLoadError>;
}

/// Loads and persists the derived obligations artifact for a track.
pub trait ObligationsArtifactPort {
    /// Loads the obligations artifact for `track_id`, if it exists.
    ///
    /// # Errors
    ///
    /// Returns an [`ArtifactCodecError`] when the artifact cannot be read or is
    /// malformed.
    fn load(&self, track_id: &TrackId) -> Result<Option<ObligationsDocument>, ArtifactCodecError>;

    /// Persist an obligations document.
    ///
    /// # Errors
    ///
    /// Returns a [`DiagnosticMessage`] describing the failure when the artifact
    /// cannot be written.
    fn save(&self, doc: &ObligationsDocument) -> Result<(), DiagnosticMessage>;
}

/// Loads and persists the test-bindings artifact for a track.
pub trait TestBindingsArtifactPort {
    /// Loads the test-bindings artifact for `track_id`, if it exists.
    ///
    /// # Errors
    ///
    /// Returns an [`ArtifactCodecError`] when the artifact cannot be read or is
    /// malformed.
    fn load(&self, track_id: &TrackId) -> Result<Option<TestBindingsDocument>, ArtifactCodecError>;

    /// Persist a test-bindings document.
    ///
    /// # Errors
    ///
    /// Returns a [`DiagnosticMessage`] describing the failure when the artifact
    /// cannot be written.
    fn save(&self, doc: &TestBindingsDocument) -> Result<(), DiagnosticMessage>;
}

/// Loads and persists the obligation-fulfillment verdict cache for a track.
pub trait ObligationFulfillmentCachePort {
    /// Loads the fulfillment cache for `track_id`, if it exists.
    ///
    /// # Errors
    ///
    /// Returns a [`VerifyCacheError`] when the cache cannot be read or is
    /// malformed.
    fn load(
        &self,
        track_id: &TrackId,
    ) -> Result<Option<ObligationFulfillmentCacheDocument>, VerifyCacheError>;

    /// Persist an obligation-fulfillment cache document.
    ///
    /// # Errors
    ///
    /// Returns a [`DiagnosticMessage`] describing the failure when the cache
    /// cannot be written.
    fn save(&self, doc: &ObligationFulfillmentCacheDocument) -> Result<(), DiagnosticMessage>;
}

/// Loads and persists the waiver verdict cache for a track.
pub trait WaiverCachePort {
    /// Loads the waiver cache for `track_id`, if it exists.
    ///
    /// # Errors
    ///
    /// Returns a [`VerifyCacheError`] when the cache cannot be read or is
    /// malformed.
    fn load(&self, track_id: &TrackId) -> Result<Option<WaiverCacheDocument>, VerifyCacheError>;

    /// Persist a waiver cache document.
    ///
    /// # Errors
    ///
    /// Returns a [`DiagnosticMessage`] describing the failure when the cache
    /// cannot be written.
    fn save(&self, doc: &WaiverCacheDocument) -> Result<(), DiagnosticMessage>;
}

/// Semantically verifies an obligation-fulfillment pair (tests vs anchor).
pub trait ObligationFulfillmentVerifierPort {
    /// Verifies whether `tests_source` fulfills `anchor_text` for
    /// `entry_declaration` at the given model `tier`.
    ///
    /// # Errors
    ///
    /// Returns a [`SemanticVerifierError`] when the verifier provider fails to
    /// return a verdict.
    fn verify_pair(
        &self,
        tests_source: &str,
        entry_declaration: &str,
        anchor_text: &str,
        tier: ModelTier,
    ) -> Result<ObligationFulfillmentVerdict, SemanticVerifierError>;
}

/// Semantically verifies a waiver pair (waived reason vs anchor).
pub trait WaiverVerifierPort {
    /// Verifies whether `waived_reason` holds for `anchor_text` and
    /// `entry_declaration` at the given model `tier`.
    ///
    /// # Errors
    ///
    /// Returns a [`SemanticVerifierError`] when the verifier provider fails to
    /// return a verdict.
    fn verify_pair(
        &self,
        waived_reason: &str,
        entry_declaration: &str,
        anchor_text: &str,
        tier: ModelTier,
    ) -> Result<WaiverVerdict, SemanticVerifierError>;
}

/// Scans a bound test's source body from the worktree and hashes it.
pub trait TestSourceScannerPort {
    /// Scans the source body of the test at `location`, if it exists.
    ///
    /// # Errors
    ///
    /// Returns a [`TestSourceScanError`] when the source cannot be read or
    /// parsed.
    fn scan_test_body(
        &self,
        location: &TestLocation,
    ) -> Result<Option<String>, TestSourceScanError>;

    /// Hashes a test body `source` into a [`TestBodySpanHash`].
    fn hash_test_body(&self, source: &str) -> TestBodySpanHash;
}
