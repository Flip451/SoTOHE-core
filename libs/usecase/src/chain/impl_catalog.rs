//! Chain ③ (`impl-catalog`) implementation.
//!
//! [`ImplCatalogChain`] implements [`ChainIdentity`] and [`crate::chain::traits::PersistedSoTChain`].
//! The blanket impl in `usecase::chain::traits` derives [`crate::chain::traits::SoTChain`] automatically.
//!
//! # Persistence target
//!
//! The persisted document is [`TypeSignalsDocument`], stored at
//! `<layer>-type-signals.json`.
//!
//! # Input type
//!
//! `Input<'a> = ImplCatalogInput<'a>` — carries the path to the type-signals
//! file and the current catalogue bytes SHA-256 hex digest (pre-computed by
//! the caller from the live `<layer>-types.json` content).
//!
//! # Freshness check (declaration_hash comparison)
//!
//! [`check_freshness`] compares `doc.declaration_hash()` with the current
//! catalogue SHA-256 supplied via `input.current_catalogue_hash`.  A mismatch
//! means the catalogue was edited after the last signal computation.
//!
//! [`check_freshness`]: ImplCatalogChain::check_freshness
//!
//! # calc / load I/O boundary
//!
//! Both [`calc`] and [`load`] require reading `<layer>-type-signals.json`
//! from disk.  Their bodies are placeholder stubs; T007 will add the actual
//! I/O via port injection or by moving the read to the infrastructure caller.
//!
//! [`calc`]: ImplCatalogChain::calc
//! [`load`]: ImplCatalogChain::load

use std::path::Path;

use crate::chain::traits::LoadablePersistedChain;
use domain::{
    ChainId, ChainIdentity, ContentHash, PersistedSoTChainGate, Strictness, TypeSignalsDocument,
    check_type_signals,
    verify::{VerifyFinding, VerifyOutcome},
};

// ── Input type ────────────────────────────────────────────────────────────────

/// Input for chain ③ (`impl-catalog`) operations.
///
/// Carries the path to the `<layer>-type-signals.json` file and the SHA-256
/// hex digest of the current `<layer>-types.json` bytes (pre-computed by the
/// infrastructure caller).
#[derive(Debug, Clone, Copy)]
pub struct ImplCatalogInput<'a> {
    /// Path to the `<layer>-type-signals.json` file.
    pub signals_path: &'a Path,
    /// SHA-256 hex digest of the current `<layer>-types.json` bytes.
    ///
    /// Used by [`check_freshness`] to detect catalogue drift.
    ///
    /// [`check_freshness`]: ImplCatalogChain::check_freshness
    pub current_catalogue_hash: &'a ContentHash,
}

impl<'a> ImplCatalogInput<'a> {
    /// Construct a new input.
    #[must_use]
    pub fn new(signals_path: &'a Path, current_catalogue_hash: &'a ContentHash) -> Self {
        Self { signals_path, current_catalogue_hash }
    }
}

// ── Error types ───────────────────────────────────────────────────────────────

/// Error produced by [`ImplCatalogChain::check_freshness`] when the stored
/// declaration hash diverges from the current catalogue bytes hash.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "impl-catalog signals are stale: stored declaration_hash={stored}, \
     current={current} — run `sotp signal calc-impl-catalog` to update"
)]
pub struct ImplCatalogStaleError {
    /// Hash stored in the document's `declaration_hash` field.
    pub stored: String,
    /// Hash of the current `<layer>-types.json` bytes.
    pub current: String,
}

/// Error produced by [`ImplCatalogChain::calc`] and [`ImplCatalogChain::load`].
///
/// Placeholder until T007.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("impl-catalog calc/load not yet wired — see T007: {0}")]
pub struct ImplCatalogCalcError(pub String);

// ── Chain ③ struct ───────────────────────────────────────────────────────────

/// Chain ③ implementation: implementation ↔ catalogue (TDDD catalogue ↔
/// rustdoc API consistency).
///
/// Unit struct; stateless dispatch.  Implements [`ChainIdentity`] and
/// [`crate::chain::traits::PersistedSoTChain`]; obtains [`crate::chain::traits::SoTChain`] via the usecase blanket impl.
#[derive(Debug, Clone, Copy)]
pub struct ImplCatalogChain;

// ── ChainIdentity ─────────────────────────────────────────────────────────────

impl ChainIdentity for ImplCatalogChain {
    const ID: ChainId = ChainId::ImplCatalog;

    /// Borrowed input carrying the signals path and current catalogue hash.
    type Input<'a> = ImplCatalogInput<'a>;
}

// ── PersistedSoTChainGate (pure domain gate) ──────────────────────────────────

impl PersistedSoTChainGate for ImplCatalogChain {
    /// The persisted signal document type.
    type Persisted = TypeSignalsDocument;
    /// Error produced by [`calc`] and [`load`].
    ///
    /// [`calc`]: ImplCatalogChain::calc
    /// [`load`]: ImplCatalogChain::load
    type CalcError = ImplCatalogCalcError;
    /// Error produced by [`check_freshness`] on stale detection.
    ///
    /// [`check_freshness`]: ImplCatalogChain::check_freshness
    type StaleError = ImplCatalogStaleError;

    /// Delegate to [`domain::check_type_signals`].
    fn evaluate_gate(persisted: &Self::Persisted, strictness: Strictness) -> VerifyOutcome {
        check_type_signals(persisted, strictness)
    }

    /// Convert an [`ImplCatalogCalcError`] into a [`VerifyOutcome`] error finding.
    fn calc_error(error: Self::CalcError) -> VerifyOutcome {
        VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
            "chain ③ (impl-catalog): {error}"
        ))])
    }

    /// Convert an [`ImplCatalogStaleError`] into a [`VerifyOutcome`] error finding.
    fn stale_error(error: Self::StaleError) -> VerifyOutcome {
        VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
            "chain ③ (impl-catalog): {error}"
        ))])
    }
}

// ── LoadablePersistedChain (I/O port, usecase layer) ──────────────────────────

impl LoadablePersistedChain for ImplCatalogChain {
    /// Compute type signals and write `<layer>-type-signals.json`.
    ///
    /// # T007 placeholder
    ///
    /// Running `cargo rustdoc` and computing signals require I/O that belongs
    /// in the infrastructure layer (CN-05).
    fn calc(_input: &Self::Input<'_>) -> Result<Self::Persisted, Self::CalcError> {
        Err(ImplCatalogCalcError("calc not yet wired — see T007".to_owned()))
    }

    /// Load `<layer>-type-signals.json` from disk.
    ///
    /// # T007 placeholder
    ///
    /// Same boundary note as [`calc`].
    ///
    /// [`calc`]: ImplCatalogChain::calc
    fn load(_input: &Self::Input<'_>) -> Result<Self::Persisted, Self::CalcError> {
        Err(ImplCatalogCalcError("load not yet wired — see T007".to_owned()))
    }

    /// Declaration-hash freshness check: compare `doc.declaration_hash()` with
    /// the current catalogue hash supplied in `input`.
    ///
    /// Returns `Ok(())` when the hashes match; returns an
    /// [`ImplCatalogStaleError`] when they differ.
    fn check_freshness(
        input: &Self::Input<'_>,
        persisted: &Self::Persisted,
    ) -> Result<(), Self::StaleError> {
        let stored = persisted.declaration_hash();
        let current = input.current_catalogue_hash.to_hex();
        if stored == current {
            Ok(())
        } else {
            Err(ImplCatalogStaleError { stored: stored.to_owned(), current })
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use std::path::Path;

    use crate::chain::test_support::{assert_persisted_chain_bounds, call_sotchain_check};
    use crate::chain::traits::LoadablePersistedChain;
    use domain::{
        ChainId, ChainIdentity, ConfidenceSignal, ContentHash, PersistedSoTChainGate, Strictness,
        Timestamp, TypeSignal, TypeSignalsDocument, verify::Severity,
    };

    use super::{ImplCatalogChain, ImplCatalogInput, ImplCatalogStaleError};

    // ── static trait-bound assertions ────────────────────────────────────────

    #[test]
    fn test_impl_catalog_chain_satisfies_chain_identity_persisted_sotchain_bounds() {
        assert_persisted_chain_bounds::<ImplCatalogChain>();
    }

    #[test]
    fn test_impl_catalog_chain_id_is_impl_catalog() {
        assert_eq!(ImplCatalogChain::ID, ChainId::ImplCatalog);
    }

    // ── SoTChain::check accepted via blanket impl ─────────────────────────────

    #[test]
    fn test_impl_catalog_chain_accepted_by_sotchain_bound_via_blanket_impl() {
        let hash = ContentHash::from_bytes([0u8; 32]);
        let input = ImplCatalogInput::new(Path::new("/tmp/type-signals.json"), &hash);
        let outcome = call_sotchain_check::<ImplCatalogChain>(&input, Strictness::Interim);
        assert!(outcome.has_errors(), "unwired load must surface as calc_error: {outcome:?}");
    }

    // ── helpers ───────────────────────────────────────────────────────────────

    fn ts() -> Timestamp {
        Timestamp::new("2026-06-18T00:00:00Z").unwrap()
    }

    fn content_hash_hex(hex: &str) -> ContentHash {
        ContentHash::try_from_hex(hex).unwrap()
    }

    fn zero_hex() -> String {
        "0".repeat(64)
    }

    fn one_hex() -> String {
        format!("{}{}", "0".repeat(63), "1")
    }

    fn make_type_signal(name: &str, sig: ConfidenceSignal) -> TypeSignal {
        TypeSignal::new(name, "value_object", sig, true, vec![], vec![], vec![])
    }

    fn make_doc(declaration_hash: &str, signals: Vec<TypeSignal>) -> TypeSignalsDocument {
        TypeSignalsDocument::new(ts(), declaration_hash, signals)
    }

    // ── check_freshness ───────────────────────────────────────────────────────

    #[test]
    fn test_check_freshness_returns_ok_when_hashes_match() {
        let hex = zero_hex();
        let doc = make_doc(&hex, vec![]);
        let current_hash = content_hash_hex(&hex);
        let input = ImplCatalogInput::new(Path::new("/tmp/ts.json"), &current_hash);
        let result = ImplCatalogChain::check_freshness(&input, &doc);
        assert!(result.is_ok(), "matching hashes must return Ok: {result:?}");
    }

    #[test]
    fn test_check_freshness_returns_stale_error_when_hashes_differ() {
        let stored_hex = zero_hex();
        let current_hex = one_hex();
        let doc = make_doc(&stored_hex, vec![]);
        let current_hash = content_hash_hex(&current_hex);
        let input = ImplCatalogInput::new(Path::new("/tmp/ts.json"), &current_hash);
        let result = ImplCatalogChain::check_freshness(&input, &doc);
        assert!(result.is_err(), "hash mismatch must return stale error: {result:?}");
        let err = result.unwrap_err();
        assert_eq!(err.stored, stored_hex);
        assert_eq!(err.current, current_hex);
    }

    // ── evaluate_gate ─────────────────────────────────────────────────────────

    #[test]
    fn test_evaluate_gate_delegates_to_check_type_signals_pass() {
        let hex = zero_hex();
        let doc = make_doc(&hex, vec![make_type_signal("Foo", ConfidenceSignal::Blue)]);
        let outcome = ImplCatalogChain::evaluate_gate(&doc, Strictness::Interim);
        assert!(outcome.findings().is_empty(), "all-blue must pass gate: {outcome:?}");
    }

    #[test]
    fn test_evaluate_gate_delegates_to_check_type_signals_red_error() {
        let hex = zero_hex();
        let doc = make_doc(&hex, vec![make_type_signal("Bar", ConfidenceSignal::Red)]);
        let outcome = ImplCatalogChain::evaluate_gate(&doc, Strictness::Interim);
        assert!(outcome.has_errors(), "red signal must be an error: {outcome:?}");
    }

    #[test]
    fn test_evaluate_gate_delegates_to_check_type_signals_yellow_warning() {
        let hex = zero_hex();
        let doc = make_doc(&hex, vec![make_type_signal("Baz", ConfidenceSignal::Yellow)]);
        let outcome = ImplCatalogChain::evaluate_gate(&doc, Strictness::Interim);
        assert!(!outcome.has_errors(), "yellow in interim must warn, not error: {outcome:?}");
        assert_eq!(outcome.findings()[0].severity(), Severity::Warning);
    }

    // ── stale_error / calc_error convert to VerifyOutcome errors ─────────────

    #[test]
    fn test_stale_error_produces_error_finding() {
        let err = ImplCatalogStaleError { stored: zero_hex(), current: one_hex() };
        let outcome = ImplCatalogChain::stale_error(err);
        assert!(outcome.has_errors(), "stale_error must produce error finding: {outcome:?}");
        assert_eq!(outcome.findings()[0].severity(), Severity::Error);
        assert!(outcome.findings()[0].message().contains("chain ③"));
        assert!(outcome.findings()[0].message().contains("stale"));
    }

    #[test]
    fn test_calc_error_produces_error_finding() {
        use super::ImplCatalogCalcError;
        let e = ImplCatalogCalcError("test error".to_owned());
        let outcome = ImplCatalogChain::calc_error(e);
        assert!(outcome.has_errors(), "calc_error must produce error finding: {outcome:?}");
        assert_eq!(outcome.findings()[0].severity(), Severity::Error);
        assert!(outcome.findings()[0].message().contains("chain ③"));
    }
}
