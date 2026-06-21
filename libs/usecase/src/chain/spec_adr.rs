//! Chain ① (`spec-adr`) implementation.
//!
//! [`SpecAdrChain`] implements [`ChainIdentity`] and [`crate::chain::traits::PersistedSoTChain`].
//! The blanket impl in `usecase::chain::traits` derives [`crate::chain::traits::SoTChain`] automatically.
//!
//! # Persistence target
//!
//! The persisted document is [`SpecDocument`], loaded from `spec.json`.
//!
//! # Input type
//!
//! `Input<'a> = SpecAdrInput<'a>` — a small borrowed struct carrying the path
//! to `spec.json`.  Callers in the infrastructure / CLI layer read the file
//! and pass the loaded document or path-only input to the chain methods.
//!
//! # Freshness check (self-consistency)
//!
//! [`check_freshness`] computes `doc.evaluate_signals()` and compares it with
//! the cached `doc.signals()`.  A mismatch means the spec document was edited
//! after the last `sotp signal calc-spec-adr` run (the signals field is stale).
//!
//! [`check_freshness`]: SpecAdrChain::check_freshness
//!
//! # calc / load I/O boundary
//!
//! Both [`calc`] and [`load`] require reading `spec.json` from disk, which is
//! I/O that belongs in the infrastructure layer (CN-05 hexagonal boundary).
//! Their bodies are left as `unimplemented!()` placeholders; T007 will add the
//! actual I/O via port injection or by moving the read to infrastructure
//! before constructing the input.
//!
//! [`calc`]: SpecAdrChain::calc
//! [`load`]: SpecAdrChain::load

use std::path::Path;

use crate::chain::traits::LoadablePersistedChain;
use domain::{
    ChainId, ChainIdentity, PersistedSoTChainGate, SignalCounts, SpecDocument, Strictness,
    check_spec_doc_signals,
    verify::{VerifyFinding, VerifyOutcome},
};

// ── Input type ────────────────────────────────────────────────────────────────

/// Input for chain ① (`spec-adr`) operations.
///
/// Carries the path to `spec.json`.  `calc` reads from the source and writes
/// the recomputed signals back; `load` reads the existing persisted document.
///
/// ## I/O boundary
///
/// Constructing a `SpecAdrInput` does not perform any I/O.  The path is used
/// by `calc` and `load`, which are placeholders until T007 lands.
#[derive(Debug, Clone, Copy)]
pub struct SpecAdrInput<'a> {
    /// Path to the track's `spec.json` file.
    pub spec_json_path: &'a Path,
}

impl<'a> SpecAdrInput<'a> {
    /// Construct a new input pointing at `spec.json`.
    #[must_use]
    pub fn new(spec_json_path: &'a Path) -> Self {
        Self { spec_json_path }
    }
}

// ── Stale error ───────────────────────────────────────────────────────────────

/// Error produced by [`SpecAdrChain::check_freshness`] when the cached signal
/// counts diverge from a freshly-computed evaluation.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "spec-adr signals are stale: cached={cached:?}, fresh={fresh:?} — \
     run `sotp signal calc-spec-adr --spec-json <path/to/spec.json>` to update"
)]
pub struct SpecAdrStaleError {
    /// Signal counts currently stored in the document's `signals` field.
    pub cached: SignalCounts,
    /// Signal counts from a fresh `evaluate_signals()` run.
    pub fresh: SignalCounts,
}

/// Error produced by [`SpecAdrChain::calc`] and [`SpecAdrChain::load`] when
/// the spec document cannot be read or parsed.
///
/// This is a placeholder type that T007 will replace with a richer error
/// carrying the underlying I/O or parse failure.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("spec-adr calc/load not yet wired — see T007: {0}")]
pub struct SpecAdrCalcError(pub String);

// ── Chain ① struct ───────────────────────────────────────────────────────────

/// Chain ① implementation: spec → ADR (spec requirement grounding completeness).
///
/// Unit struct; stateless dispatch.  Implements [`ChainIdentity`] and
/// [`crate::chain::traits::PersistedSoTChain`]; obtains [`crate::chain::traits::SoTChain`] via the usecase blanket impl.
#[derive(Debug, Clone, Copy)]
pub struct SpecAdrChain;

// ── ChainIdentity ─────────────────────────────────────────────────────────────

impl ChainIdentity for SpecAdrChain {
    const ID: ChainId = ChainId::SpecAdr;

    /// Borrowed input carrying the path to `spec.json`.
    type Input<'a> = SpecAdrInput<'a>;
}

// ── PersistedSoTChainGate (pure domain gate) ──────────────────────────────────

impl PersistedSoTChainGate for SpecAdrChain {
    /// The persisted signal document type.
    type Persisted = SpecDocument;
    /// Error produced by [`calc`] and [`load`].
    ///
    /// [`calc`]: SpecAdrChain::calc
    /// [`load`]: SpecAdrChain::load
    type CalcError = SpecAdrCalcError;
    /// Error produced by [`check_freshness`] on stale detection.
    ///
    /// [`check_freshness`]: SpecAdrChain::check_freshness
    type StaleError = SpecAdrStaleError;

    /// Delegate to `domain::check_spec_doc_signals`.
    fn evaluate_gate(persisted: &Self::Persisted, strictness: Strictness) -> VerifyOutcome {
        check_spec_doc_signals(persisted, strictness)
    }

    /// Convert a [`SpecAdrCalcError`] into a [`VerifyOutcome`] error finding.
    fn calc_error(error: Self::CalcError) -> VerifyOutcome {
        VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
            "chain ① (spec-adr): {error}"
        ))])
    }

    /// Convert a [`SpecAdrStaleError`] into a [`VerifyOutcome`] error finding.
    fn stale_error(error: Self::StaleError) -> VerifyOutcome {
        VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
            "chain ① (spec-adr): {error}"
        ))])
    }
}

// ── LoadablePersistedChain (I/O port, usecase layer) ──────────────────────────

impl LoadablePersistedChain for SpecAdrChain {
    /// Compute signals for `spec.json` and write them back.
    ///
    /// # T007 placeholder
    ///
    /// Reading and parsing `spec.json` and persisting the updated signals
    /// require I/O that belongs in the infrastructure layer (CN-05).  This
    /// body will be implemented in T007 via port injection or by moving the
    /// I/O step to the infrastructure caller before constructing the input.
    fn calc(_input: &Self::Input<'_>) -> Result<Self::Persisted, Self::CalcError> {
        Err(SpecAdrCalcError("calc not yet wired — see T007".to_owned()))
    }

    /// Load the persisted `spec.json` document from disk.
    ///
    /// # T007 placeholder
    ///
    /// Same boundary note as [`calc`].  T007 will supply the actual file read.
    ///
    /// [`calc`]: SpecAdrChain::calc
    fn load(_input: &Self::Input<'_>) -> Result<Self::Persisted, Self::CalcError> {
        Err(SpecAdrCalcError("load not yet wired — see T007".to_owned()))
    }

    /// Self-consistency freshness check: compare cached signals with a fresh
    /// recomputation.
    ///
    /// Returns `Ok(())` when `doc.signals() == Some(&doc.evaluate_signals())`,
    /// meaning the stored signal counts are still valid.  Returns a
    /// [`SpecAdrStaleError`] when:
    /// - `doc.signals()` is `None` (signals never evaluated), or
    /// - the stored counts differ from a fresh `evaluate_signals()` run.
    fn check_freshness(
        _input: &Self::Input<'_>,
        persisted: &Self::Persisted,
    ) -> Result<(), Self::StaleError> {
        let fresh = persisted.evaluate_signals();
        match persisted.signals() {
            Some(cached) if *cached == fresh => Ok(()),
            Some(cached) => Err(SpecAdrStaleError { cached: *cached, fresh }),
            None => Err(SpecAdrStaleError { cached: SignalCounts::new(0, 0, 0), fresh }),
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use std::path::Path;
    use std::path::PathBuf;

    use crate::chain::test_support::{assert_persisted_chain_bounds, call_sotchain_check};
    use crate::chain::traits::LoadablePersistedChain;
    use domain::{
        AdrAnchor, AdrRef, ChainId, ChainIdentity, PersistedSoTChainGate, SignalCounts,
        SpecDocument, SpecElementId, SpecRequirement, SpecScope, Strictness, verify::Severity,
    };

    use super::{SpecAdrChain, SpecAdrInput, SpecAdrStaleError};

    // ── static trait-bound assertions ────────────────────────────────────────

    #[test]
    fn test_spec_adr_chain_satisfies_chain_identity_persisted_sotchain_bounds() {
        assert_persisted_chain_bounds::<SpecAdrChain>();
    }

    #[test]
    fn test_spec_adr_chain_id_is_spec_adr() {
        assert_eq!(SpecAdrChain::ID, ChainId::SpecAdr);
    }

    // ── SoTChain::check accepted via blanket impl ─────────────────────────────

    #[test]
    fn test_spec_adr_chain_accepted_by_sotchain_bound_via_blanket_impl() {
        let input = SpecAdrInput::new(Path::new("/tmp/spec.json"));
        // load is not yet wired; check should return a calc_error finding.
        let outcome = call_sotchain_check::<SpecAdrChain>(&input, Strictness::Interim);
        assert!(outcome.has_errors(), "unwired load must surface as calc_error: {outcome:?}");
    }

    // ── helpers ───────────────────────────────────────────────────────────────

    fn req_blue(id_s: &str, text: &str) -> SpecRequirement {
        SpecRequirement::new(
            SpecElementId::try_new(id_s).unwrap(),
            text,
            vec![AdrRef::new(
                PathBuf::from("knowledge/adr/x.md"),
                AdrAnchor::try_new("D1").unwrap(),
            )],
            vec![],
            vec![],
        )
        .unwrap()
    }

    fn make_doc_with_signals(signals: Option<SignalCounts>) -> SpecDocument {
        SpecDocument::new(
            "Test spec",
            "1.0",
            vec![],
            SpecScope::new(vec![req_blue("IN-01", "in scope")], vec![]),
            vec![],
            vec![],
            vec![],
            vec![],
            signals,
        )
        .unwrap()
    }

    // ── check_freshness ───────────────────────────────────────────────────────

    #[test]
    fn test_check_freshness_returns_ok_when_cached_matches_fresh() {
        // The doc has one Blue requirement.
        let fresh = SignalCounts::new(1, 0, 0);
        let doc = make_doc_with_signals(Some(fresh));
        let input = SpecAdrInput::new(Path::new("/tmp/spec.json"));
        let result = SpecAdrChain::check_freshness(&input, &doc);
        assert!(result.is_ok(), "fresh == cached must return Ok: {result:?}");
    }

    #[test]
    fn test_check_freshness_returns_stale_error_when_counts_differ() {
        // Cached says blue=2 but the doc only has 1 blue req.
        let doc = make_doc_with_signals(Some(SignalCounts::new(2, 0, 0)));
        let input = SpecAdrInput::new(Path::new("/tmp/spec.json"));
        let result = SpecAdrChain::check_freshness(&input, &doc);
        assert!(result.is_err(), "count mismatch must return stale error: {result:?}");
        let err = result.unwrap_err();
        assert_eq!(err.cached, SignalCounts::new(2, 0, 0));
        assert_eq!(err.fresh, SignalCounts::new(1, 0, 0));
    }

    #[test]
    fn test_check_freshness_returns_stale_error_when_signals_are_none() {
        // No cached signals in the document (never evaluated).
        let doc = make_doc_with_signals(None);
        let input = SpecAdrInput::new(Path::new("/tmp/spec.json"));
        let result = SpecAdrChain::check_freshness(&input, &doc);
        assert!(result.is_err(), "None signals must return stale error: {result:?}");
    }

    // ── evaluate_gate ─────────────────────────────────────────────────────────

    #[test]
    fn test_evaluate_gate_delegates_to_check_spec_doc_signals() {
        // A doc with evaluated signals: 1 blue, no red/yellow → pass.
        let doc = make_doc_with_signals(Some(SignalCounts::new(1, 0, 0)));
        let input = SpecAdrInput::new(Path::new("/tmp/spec.json"));
        let _ = input; // input not used by evaluate_gate
        let outcome = SpecAdrChain::evaluate_gate(&doc, Strictness::Interim);
        assert!(outcome.findings().is_empty(), "all-blue spec must pass gate: {outcome:?}");
    }

    #[test]
    fn test_evaluate_gate_returns_error_for_none_signals() {
        // domain::check_spec_doc_signals returns an error when signals is None.
        let doc = make_doc_with_signals(None);
        let outcome = SpecAdrChain::evaluate_gate(&doc, Strictness::Interim);
        assert!(
            outcome.has_errors(),
            "None signals must be an error from evaluate_gate: {outcome:?}"
        );
    }

    // ── stale_error converts to VerifyOutcome error ───────────────────────────

    #[test]
    fn test_stale_error_produces_error_finding() {
        let stale = SpecAdrStaleError {
            cached: SignalCounts::new(2, 0, 0),
            fresh: SignalCounts::new(1, 0, 0),
        };
        let outcome = SpecAdrChain::stale_error(stale);
        assert!(outcome.has_errors(), "stale_error must produce error finding: {outcome:?}");
        assert_eq!(outcome.findings()[0].severity(), Severity::Error);
        assert!(outcome.findings()[0].message().contains("chain ①"));
        assert!(outcome.findings()[0].message().contains("stale"));
    }

    // ── calc_error converts to VerifyOutcome error ────────────────────────────

    #[test]
    fn test_calc_error_produces_error_finding() {
        use super::SpecAdrCalcError;
        let e = SpecAdrCalcError("test error".to_owned());
        let outcome = SpecAdrChain::calc_error(e);
        assert!(outcome.has_errors(), "calc_error must produce error finding: {outcome:?}");
        assert_eq!(outcome.findings()[0].severity(), Severity::Error);
        assert!(outcome.findings()[0].message().contains("chain ①"));
    }
}
