//! Chain ② (`catalog-spec`) implementation.
//!
//! [`CatalogSpecChain`] implements [`ChainIdentity`] and [`PersistedSoTChain`].
//! The blanket impl in `domain::chain` derives [`domain::SoTChain`] automatically.
//!
//! # Persistence target
//!
//! The persisted document is [`CatalogueSpecSignalsDocument`], stored at
//! `<layer>-catalogue-spec-signals.json`.
//!
//! # Input type
//!
//! `Input<'a> = CatalogSpecInput<'a>` — carries the path to the signals file,
//! the current catalogue bytes SHA-256 hex digest, and the current per-entry
//! SHA-256 hashes (all pre-computed by the caller from the live
//! `<layer>-types.json` content).
//!
//! # Freshness check (entry_hash comparison)
//!
//! [`check_freshness`] compares `doc.catalogue_declaration_hash` with the
//! current catalogue SHA-256 supplied via `input.current_catalogue_hash`, then
//! compares every persisted `entry_hash` with the current hash for the same
//! catalogue entry. A mismatch means the catalogue was edited after the last
//! signal computation or the signal file was manually trimmed.
//!
//! [`check_freshness`]: CatalogSpecChain::check_freshness
//!
//! # calc / load I/O boundary
//!
//! Both [`calc`] and [`load`] require reading `<layer>-catalogue-spec-signals.json`
//! from disk.  Their bodies are placeholder stubs; T008 will add the actual
//! I/O via port injection or by moving the read to the infrastructure caller.
//!
//! [`calc`]: CatalogSpecChain::calc
//! [`load`]: CatalogSpecChain::load

use std::path::Path;

use domain::{
    CatalogueSpecSignalsDocument, ChainId, ChainIdentity, ContentHash, PersistedSoTChain,
    check_catalogue_spec_signals,
    verify::{VerifyFinding, VerifyOutcome},
};

// ── Input type ────────────────────────────────────────────────────────────────

/// Input for chain ② (`catalog-spec`) operations.
///
/// Carries the path to the `<layer>-catalogue-spec-signals.json` signals file,
/// the SHA-256 hex string of the current `<layer>-types.json` bytes, and the
/// current per-entry hashes (pre-computed by the infrastructure caller — never
/// computed here).
#[doc(hidden)]
#[derive(Debug, Clone, Copy)]
pub struct CatalogSpecInput<'a> {
    /// Path to the `<layer>-catalogue-spec-signals.json` file.
    pub signals_path: &'a Path,
    /// SHA-256 hex digest of the current `<layer>-types.json` bytes.
    ///
    /// Used by [`check_freshness`] to detect catalogue drift.
    ///
    /// [`check_freshness`]: CatalogSpecChain::check_freshness
    pub current_catalogue_hash: &'a ContentHash,
    /// Current per-entry SHA-256 hashes keyed by catalogue entry name.
    ///
    /// Used to fail closed when a persisted signal has a stale/missing
    /// `entry_hash` or when the persisted signal file is missing an entry.
    pub current_entry_hashes: &'a [(&'a str, &'a ContentHash)],
}

impl<'a> CatalogSpecInput<'a> {
    /// Construct a new input.
    #[must_use]
    pub fn new(
        signals_path: &'a Path,
        current_catalogue_hash: &'a ContentHash,
        current_entry_hashes: &'a [(&'a str, &'a ContentHash)],
    ) -> Self {
        Self { signals_path, current_catalogue_hash, current_entry_hashes }
    }
}

// ── Error types ───────────────────────────────────────────────────────────────

/// Error produced by [`CatalogSpecChain::check_freshness`] when the stored
/// catalogue hash diverges from the current catalogue bytes hash.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[doc(hidden)]
pub enum CatalogSpecStaleError {
    /// Hash stored in the document's `catalogue_declaration_hash` field differs
    /// from the hash of the current `<layer>-types.json` bytes.
    #[error(
        "catalog-spec signals are stale: stored catalogue_declaration_hash={stored}, \
         current={current} — run `sotp signal calc-catalog-spec` to update"
    )]
    CatalogueHash { stored: ContentHash, current: ContentHash },
    /// A persisted signal has no corresponding current entry hash.
    #[error(
        "catalog-spec signals are stale: no current entry hash for signal `{entry_name}` — \
         run `sotp signal calc-catalog-spec` to update"
    )]
    MissingCurrentEntryHash { entry_name: String },
    /// A current catalogue entry has no persisted signal.
    #[error(
        "catalog-spec signals are stale: missing signal for current catalogue entry \
         `{entry_name}` — run `sotp signal calc-catalog-spec` to update"
    )]
    MissingSignal { entry_name: String },
    /// A persisted signal's `entry_hash` differs from the current entry hash.
    #[error(
        "catalog-spec signals are stale: entry_hash mismatch for `{entry_name}`: \
         stored={stored}, current={current} — run `sotp signal calc-catalog-spec` \
         to update"
    )]
    EntryHash { entry_name: String, stored: ContentHash, current: ContentHash },
    /// A persisted signal file contains duplicate entries for the same catalogue
    /// entry name.
    #[error(
        "catalog-spec signals are stale: duplicate signal for catalogue entry \
         `{entry_name}` — run `sotp signal calc-catalog-spec` to update"
    )]
    DuplicateSignal { entry_name: String },
}

/// Error produced by [`CatalogSpecChain::calc`] and [`CatalogSpecChain::load`].
///
/// Placeholder until T008.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("catalog-spec calc/load not yet wired — see T008: {0}")]
#[doc(hidden)]
pub struct CatalogSpecCalcError(pub String);

// ── Chain ② struct ───────────────────────────────────────────────────────────

/// Chain ② implementation: catalogue → spec (type catalogue spec-reference
/// completeness).
///
/// Unit struct; stateless dispatch.  Implements [`ChainIdentity`] and
/// [`PersistedSoTChain`]; obtains [`domain::SoTChain`] via the domain blanket impl.
#[derive(Debug, Clone, Copy)]
pub struct CatalogSpecChain;

// ── ChainIdentity ─────────────────────────────────────────────────────────────

impl ChainIdentity for CatalogSpecChain {
    const ID: ChainId = ChainId::CatalogSpec;

    /// Borrowed input carrying the signals path and current catalogue hash.
    type Input<'a> = CatalogSpecInput<'a>;
}

// ── PersistedSoTChain ─────────────────────────────────────────────────────────

impl PersistedSoTChain for CatalogSpecChain {
    /// The persisted signal document type.
    type Persisted = CatalogueSpecSignalsDocument;
    /// Error produced by [`calc`] and [`load`].
    ///
    /// [`calc`]: CatalogSpecChain::calc
    /// [`load`]: CatalogSpecChain::load
    type CalcError = CatalogSpecCalcError;
    /// Error produced by [`check_freshness`] on stale detection.
    ///
    /// [`check_freshness`]: CatalogSpecChain::check_freshness
    type StaleError = CatalogSpecStaleError;

    /// Compute catalogue-spec signals and write `<layer>-catalogue-spec-signals.json`.
    ///
    /// # T008 placeholder
    ///
    /// Reading `<layer>-types.json` and writing the generated signals document
    /// require I/O that belongs in the infrastructure layer (CN-05).
    fn calc(_input: &Self::Input<'_>) -> Result<Self::Persisted, Self::CalcError> {
        Err(CatalogSpecCalcError("calc not yet wired — see T008".to_owned()))
    }

    /// Load `<layer>-catalogue-spec-signals.json` from disk.
    ///
    /// # T008 placeholder
    ///
    /// Same boundary note as [`calc`].
    ///
    /// [`calc`]: CatalogSpecChain::calc
    fn load(_input: &Self::Input<'_>) -> Result<Self::Persisted, Self::CalcError> {
        Err(CatalogSpecCalcError("load not yet wired — see T008".to_owned()))
    }

    /// Freshness check: compare stored `catalogue_declaration_hash` with the
    /// current catalogue bytes SHA-256 supplied in `input`, then compare every
    /// persisted per-entry `entry_hash` with its current entry hash.
    ///
    /// Returns `Ok(())` when the hashes match; returns a
    /// [`CatalogSpecStaleError`] when the document hash differs, a persisted
    /// signal is missing or duplicated, or an entry hash differs.
    fn check_freshness(
        input: &Self::Input<'_>,
        persisted: &Self::Persisted,
    ) -> Result<(), Self::StaleError> {
        let stored = &persisted.catalogue_declaration_hash;
        let current = input.current_catalogue_hash;
        if stored != current {
            return Err(CatalogSpecStaleError::CatalogueHash {
                stored: stored.clone(),
                current: current.clone(),
            });
        }

        for (idx, signal) in persisted.signals.iter().enumerate() {
            if persisted
                .signals
                .iter()
                .take(idx)
                .any(|seen| seen.type_name.as_str() == signal.type_name.as_str())
            {
                return Err(CatalogSpecStaleError::DuplicateSignal {
                    entry_name: signal.type_name.clone(),
                });
            }

            let current_entry_hash = input
                .current_entry_hashes
                .iter()
                .find(|(entry_name, _)| *entry_name == signal.type_name.as_str())
                .map(|(_, hash)| *hash)
                .ok_or_else(|| CatalogSpecStaleError::MissingCurrentEntryHash {
                    entry_name: signal.type_name.clone(),
                })?;

            if signal.entry_hash() != current_entry_hash {
                return Err(CatalogSpecStaleError::EntryHash {
                    entry_name: signal.type_name.clone(),
                    stored: signal.entry_hash().clone(),
                    current: current_entry_hash.clone(),
                });
            }
        }

        for (entry_name, _) in input.current_entry_hashes {
            if !persisted.signals.iter().any(|signal| signal.type_name.as_str() == *entry_name) {
                return Err(CatalogSpecStaleError::MissingSignal {
                    entry_name: (*entry_name).to_owned(),
                });
            }
        }

        Ok(())
    }

    /// Delegate to [`domain::check_catalogue_spec_signals`].
    fn evaluate_gate(persisted: &Self::Persisted, strict: bool) -> VerifyOutcome {
        check_catalogue_spec_signals(persisted, strict)
    }

    /// Convert a [`CatalogSpecCalcError`] into a [`VerifyOutcome`] error finding.
    fn calc_error(error: Self::CalcError) -> VerifyOutcome {
        VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
            "chain ② (catalog-spec): {error}"
        ))])
    }

    /// Convert a [`CatalogSpecStaleError`] into a [`VerifyOutcome`] error finding.
    fn stale_error(error: Self::StaleError) -> VerifyOutcome {
        VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
            "chain ② (catalog-spec): {error}"
        ))])
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use std::path::Path;

    use crate::chain::test_support::{assert_persisted_chain_bounds, call_sotchain_check};
    use domain::{
        CatalogueSpecSignal, CatalogueSpecSignalsDocument, ChainId, ChainIdentity,
        ConfidenceSignal, ContentHash, PersistedSoTChain, verify::Severity,
    };

    use super::{CatalogSpecChain, CatalogSpecInput, CatalogSpecStaleError};

    // ── static trait-bound assertions ────────────────────────────────────────

    #[test]
    fn test_catalog_spec_chain_satisfies_chain_identity_persisted_sotchain_bounds() {
        assert_persisted_chain_bounds::<CatalogSpecChain>();
    }

    #[test]
    fn test_catalog_spec_chain_id_is_catalog_spec() {
        assert_eq!(CatalogSpecChain::ID, ChainId::CatalogSpec);
    }

    // ── SoTChain::check accepted via blanket impl ─────────────────────────────

    #[test]
    fn test_catalog_spec_chain_accepted_by_sotchain_bound_via_blanket_impl() {
        let hash = ContentHash::from_bytes([0u8; 32]);
        let input = CatalogSpecInput::new(Path::new("/tmp/sig.json"), &hash, &[]);
        let outcome = call_sotchain_check::<CatalogSpecChain>(&input, false);
        assert!(outcome.has_errors(), "unwired load must surface as calc_error: {outcome:?}");
    }

    // ── helpers ───────────────────────────────────────────────────────────────

    fn hash(byte: u8) -> ContentHash {
        ContentHash::from_bytes([byte; 32])
    }

    fn signal(name: &str, sig: ConfidenceSignal) -> CatalogueSpecSignal {
        signal_with_hash(name, sig, hash(0x00))
    }

    fn signal_with_hash(
        name: &str,
        sig: ConfidenceSignal,
        entry_hash: ContentHash,
    ) -> CatalogueSpecSignal {
        CatalogueSpecSignal::new(name, sig, entry_hash)
    }

    fn doc_with_hash(
        catalogue_hash: ContentHash,
        signals: Vec<CatalogueSpecSignal>,
    ) -> CatalogueSpecSignalsDocument {
        CatalogueSpecSignalsDocument::new(catalogue_hash, signals)
    }

    // ── check_freshness ───────────────────────────────────────────────────────

    #[test]
    fn test_check_freshness_returns_ok_when_hashes_match() {
        let h = hash(0xaa);
        let doc = doc_with_hash(h.clone(), vec![]);
        let input = CatalogSpecInput::new(Path::new("/tmp/sig.json"), &h, &[]);
        let result = CatalogSpecChain::check_freshness(&input, &doc);
        assert!(result.is_ok(), "matching hashes must return Ok: {result:?}");
    }

    #[test]
    fn test_check_freshness_returns_ok_when_entry_hashes_match() {
        let catalogue_hash = hash(0xaa);
        let entry_hash = hash(0x11);
        let doc = doc_with_hash(
            catalogue_hash.clone(),
            vec![signal_with_hash("TypeA", ConfidenceSignal::Blue, entry_hash.clone())],
        );
        let current_entry_hashes = [("TypeA", &entry_hash)];
        let input = CatalogSpecInput::new(
            Path::new("/tmp/sig.json"),
            &catalogue_hash,
            &current_entry_hashes,
        );
        let result = CatalogSpecChain::check_freshness(&input, &doc);
        assert!(result.is_ok(), "matching entry hashes must return Ok: {result:?}");
    }

    #[test]
    fn test_check_freshness_returns_stale_error_when_hashes_differ() {
        let stored_hash = hash(0xaa);
        let current_hash = hash(0xbb);
        let doc = doc_with_hash(stored_hash.clone(), vec![]);
        let input = CatalogSpecInput::new(Path::new("/tmp/sig.json"), &current_hash, &[]);
        let result = CatalogSpecChain::check_freshness(&input, &doc);
        assert!(result.is_err(), "hash mismatch must return stale error: {result:?}");
        match result.unwrap_err() {
            CatalogSpecStaleError::CatalogueHash { stored, current } => {
                assert_eq!(stored, stored_hash);
                assert_eq!(current, current_hash);
            }
            other => panic!("expected catalogue hash stale error, got {other:?}"),
        }
    }

    #[test]
    fn test_check_freshness_returns_stale_error_when_entry_hash_differs() {
        let catalogue_hash = hash(0xaa);
        let stored_entry_hash = hash(0x11);
        let current_entry_hash = hash(0x22);
        let doc = doc_with_hash(
            catalogue_hash.clone(),
            vec![signal_with_hash("TypeA", ConfidenceSignal::Blue, stored_entry_hash.clone())],
        );
        let current_entry_hashes = [("TypeA", &current_entry_hash)];
        let input = CatalogSpecInput::new(
            Path::new("/tmp/sig.json"),
            &catalogue_hash,
            &current_entry_hashes,
        );
        let result = CatalogSpecChain::check_freshness(&input, &doc);
        assert!(result.is_err(), "entry hash mismatch must return stale error: {result:?}");
        match result.unwrap_err() {
            CatalogSpecStaleError::EntryHash { entry_name, stored, current } => {
                assert_eq!(entry_name, "TypeA");
                assert_eq!(stored, stored_entry_hash);
                assert_eq!(current, current_entry_hash);
            }
            other => panic!("expected entry hash stale error, got {other:?}"),
        }
    }

    #[test]
    fn test_check_freshness_returns_stale_error_when_current_entry_hash_missing() {
        let catalogue_hash = hash(0xaa);
        let entry_hash = hash(0x11);
        let doc = doc_with_hash(
            catalogue_hash.clone(),
            vec![signal_with_hash("TypeA", ConfidenceSignal::Blue, entry_hash)],
        );
        let input = CatalogSpecInput::new(Path::new("/tmp/sig.json"), &catalogue_hash, &[]);
        let result = CatalogSpecChain::check_freshness(&input, &doc);
        assert!(result.is_err(), "missing current entry hash must fail closed: {result:?}");
        match result.unwrap_err() {
            CatalogSpecStaleError::MissingCurrentEntryHash { entry_name } => {
                assert_eq!(entry_name, "TypeA");
            }
            other => panic!("expected missing current entry hash, got {other:?}"),
        }
    }

    #[test]
    fn test_check_freshness_returns_stale_error_when_signal_missing_for_current_entry() {
        let catalogue_hash = hash(0xaa);
        let entry_hash = hash(0x11);
        let doc = doc_with_hash(catalogue_hash.clone(), vec![]);
        let current_entry_hashes = [("TypeA", &entry_hash)];
        let input = CatalogSpecInput::new(
            Path::new("/tmp/sig.json"),
            &catalogue_hash,
            &current_entry_hashes,
        );
        let result = CatalogSpecChain::check_freshness(&input, &doc);
        assert!(result.is_err(), "missing persisted signal must fail closed: {result:?}");
        match result.unwrap_err() {
            CatalogSpecStaleError::MissingSignal { entry_name } => {
                assert_eq!(entry_name, "TypeA");
            }
            other => panic!("expected missing signal stale error, got {other:?}"),
        }
    }

    #[test]
    fn test_check_freshness_returns_stale_error_when_signal_is_duplicated() {
        let catalogue_hash = hash(0xaa);
        let entry_hash = hash(0x11);
        let doc = doc_with_hash(
            catalogue_hash.clone(),
            vec![
                signal_with_hash("TypeA", ConfidenceSignal::Blue, entry_hash.clone()),
                signal_with_hash("TypeA", ConfidenceSignal::Blue, entry_hash.clone()),
            ],
        );
        let current_entry_hashes = [("TypeA", &entry_hash)];
        let input = CatalogSpecInput::new(
            Path::new("/tmp/sig.json"),
            &catalogue_hash,
            &current_entry_hashes,
        );
        let result = CatalogSpecChain::check_freshness(&input, &doc);
        assert!(result.is_err(), "duplicate persisted signal must fail closed: {result:?}");
        match result.unwrap_err() {
            CatalogSpecStaleError::DuplicateSignal { entry_name } => {
                assert_eq!(entry_name, "TypeA");
            }
            other => panic!("expected duplicate signal stale error, got {other:?}"),
        }
    }

    // ── evaluate_gate ─────────────────────────────────────────────────────────

    #[test]
    fn test_evaluate_gate_delegates_to_check_catalogue_spec_signals_pass() {
        let doc = doc_with_hash(hash(0x00), vec![signal("TypeA", ConfidenceSignal::Blue)]);
        let outcome = CatalogSpecChain::evaluate_gate(&doc, false);
        assert!(outcome.findings().is_empty(), "all-blue doc must pass gate: {outcome:?}");
    }

    #[test]
    fn test_evaluate_gate_delegates_to_check_catalogue_spec_signals_red_error() {
        let doc = doc_with_hash(hash(0x00), vec![signal("TypeA", ConfidenceSignal::Red)]);
        let outcome = CatalogSpecChain::evaluate_gate(&doc, false);
        assert!(outcome.has_errors(), "red signal must be an error: {outcome:?}");
    }

    #[test]
    fn test_evaluate_gate_delegates_to_check_catalogue_spec_signals_yellow_warning() {
        let doc = doc_with_hash(hash(0x00), vec![signal("TypeA", ConfidenceSignal::Yellow)]);
        let outcome = CatalogSpecChain::evaluate_gate(&doc, false);
        assert!(!outcome.has_errors(), "yellow in interim must warn, not error: {outcome:?}");
        assert_eq!(outcome.findings()[0].severity(), Severity::Warning);
    }

    // ── stale_error / calc_error convert to VerifyOutcome errors ─────────────

    #[test]
    fn test_stale_error_produces_error_finding() {
        let err = CatalogSpecStaleError::CatalogueHash { stored: hash(0xaa), current: hash(0xbb) };
        let outcome = CatalogSpecChain::stale_error(err);
        assert!(outcome.has_errors(), "stale_error must produce error finding: {outcome:?}");
        assert_eq!(outcome.findings()[0].severity(), Severity::Error);
        assert!(outcome.findings()[0].message().contains("chain ②"));
        assert!(outcome.findings()[0].message().contains("stale"));
    }

    #[test]
    fn test_calc_error_produces_error_finding() {
        use super::CatalogSpecCalcError;
        let e = CatalogSpecCalcError("test error".to_owned());
        let outcome = CatalogSpecChain::calc_error(e);
        assert!(outcome.has_errors(), "calc_error must produce error finding: {outcome:?}");
        assert_eq!(outcome.findings()[0].severity(), Severity::Error);
        assert!(outcome.findings()[0].message().contains("chain ②"));
    }
}
