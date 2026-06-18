//! Chain ⓪ (`adr-user`) implementation.
//!
//! [`AdrUserChain`] implements [`ChainIdentity`], [`SoTChain`], and
//! [`LiveSoTChain`]. It does **not** implement [`domain::PersistedSoTChain`] — live
//! scan only, no persistence file.
//!
//! # Input type
//!
//! `Input<'a> = &'a std::path::Path` — the project root directory.  The live
//! calculation scans `knowledge/adr/` under that root via the
//! [`domain::AdrFilePort`] secondary port (wired in T006).
//!
//! # Live calc placeholder
//!
//! [`AdrUserChain::calc_live`] is a placeholder for T006.  The body currently
//! returns an error because the filesystem port (`FsAdrFileAdapter`) lives in
//! the infrastructure layer and cannot be constructed here (hexagonal boundary
//! CN-05).  T006 will provide the wiring path:
//!
//! - either expose a free function that accepts `&dyn AdrFilePort` alongside
//!   the path and return the report, which the CLI composition root wires up;
//! - or redesign `Input<'a>` to carry an `Arc<dyn AdrFilePort>` so callers
//!   can inject the adapter before calling `SoTChain::check`.
//!
//! # SoTChain::check
//!
//! Calls `calc_live`, then applies the ADR signal gate:
//! - `red_count >= 1` → `VerifyFinding::error` (unconditional).
//! - `yellow_count >= 1` and `strict=true` → `VerifyFinding::error`.
//! - `yellow_count >= 1` and `strict=false` → `VerifyFinding::warning`.
//! - All clean → `VerifyOutcome::pass()`.

use std::path::Path;

use domain::verify::{VerifyFinding, VerifyOutcome};
use domain::{AdrFilePortError, AdrVerifyReport, ChainId, ChainIdentity, LiveSoTChain, SoTChain};

// ── Chain ⓪ struct ───────────────────────────────────────────────────────────

/// Chain ⓪ implementation: ADR → user decision (provenance completeness).
///
/// Unit struct; stateless dispatch. Implements [`ChainIdentity`], [`SoTChain`],
/// and [`LiveSoTChain`]. Does **not** implement [`domain::PersistedSoTChain`].
///
/// See module documentation for the T006 wiring open question.
#[derive(Debug, Clone, Copy)]
pub struct AdrUserChain;

// ── ChainIdentity ─────────────────────────────────────────────────────────────

impl ChainIdentity for AdrUserChain {
    const ID: ChainId = ChainId::AdrUser;

    /// Project root directory.  Live calculation scans `<root>/knowledge/adr/`.
    type Input<'a> = &'a Path;
}

// ── SoTChain (direct impl — chain ⓪ does not use blanket impl) ───────────────

impl SoTChain for AdrUserChain {
    /// Evaluate chain ⓪ gate: call [`calc_live`], then apply strictness.
    ///
    /// If `calc_live` fails (T006 not yet wired), returns an error finding
    /// that prompts the operator to wire the ADR file port.
    ///
    /// [`calc_live`]: AdrUserChain::calc_live
    fn check(input: &Self::Input<'_>, strict: bool) -> VerifyOutcome {
        let report = match Self::calc_live(input) {
            Ok(r) => r,
            Err(e) => {
                return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                    "chain ⓪ (adr-user): failed to compute live ADR signals: {e}"
                ))]);
            }
        };
        adr_report_to_outcome(&report, strict)
    }
}

// ── LiveSoTChain ──────────────────────────────────────────────────────────────

impl LiveSoTChain for AdrUserChain {
    /// The live-computed ADR signal aggregate.
    type LiveCalc = AdrVerifyReport;
    /// Error produced when the live calculation cannot complete.
    type CalcError = AdrFilePortError;

    /// Compute ADR signals live from the project root path.
    ///
    /// # T006 open question
    ///
    /// This method requires access to an [`AdrFilePort`] adapter (e.g.
    /// `FsAdrFileAdapter`), which lives in the infrastructure layer.  Because
    /// `calc_live` is an associated function (no `&self` receiver), the port
    /// cannot be held on the struct.  Two options for T006:
    ///
    /// 1. Change `Input<'a>` to carry `(&'a Path, &'a dyn AdrFilePort)` — zero
    ///    cost at the language level, but changes the public input shape.
    /// 2. Expose a standalone helper (`calc_live_with_port`) that takes an
    ///    explicit port reference; the CLI composition root constructs the
    ///    adapter and calls through.
    ///
    /// Until T006 lands, this placeholder returns `CalcError::ListPaths("not
    /// yet wired — see T006")` so all compile-time trait bounds are satisfied
    /// and tests can substitute a mock.
    ///
    /// [`AdrFilePort`]: domain::AdrFilePort
    fn calc_live(_input: &Self::Input<'_>) -> Result<Self::LiveCalc, Self::CalcError> {
        // T006: wire filesystem adapter here.
        // Cannot construct FsAdrFileAdapter in the usecase layer (CN-05).
        Err(AdrFilePortError::ListPaths(
            "AdrUserChain::calc_live is not yet wired — see T006".to_owned(),
        ))
    }
}

// ── Gate helpers ─────────────────────────────────────────────────────────────

/// Apply ADR signal gate rules to an [`AdrVerifyReport`].
///
/// Rules (parallel to `check_spec_doc_signals`):
/// - `red_count >= 1` → `VerifyFinding::error` (unconditional).
/// - `yellow_count >= 1`, `strict=true` → `VerifyFinding::error`.
/// - `yellow_count >= 1`, `strict=false` → `VerifyFinding::warning`.
/// - Everything else → `VerifyOutcome::pass()`.
///
/// Grandfathered decisions are intentionally excluded from both error and
/// warning evaluation — they are informational only (back-fill debt).
#[doc(hidden)]
#[must_use]
pub fn adr_report_to_outcome(report: &AdrVerifyReport, strict: bool) -> VerifyOutcome {
    if report.red_count() >= 1 {
        return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
            "{} ADR decision(s) have Red signal (no traced grounds) — \
             add `user_decision_ref` or `review_finding_ref` to each entry",
            report.red_count()
        ))]);
    }
    if report.yellow_count() >= 1 {
        let message = format!(
            "{} ADR decision(s) have Yellow signal (review-process derived only) — \
             merge gate will block these until upgraded to Blue. \
             Add a `user_decision_ref` to each entry.",
            report.yellow_count()
        );
        if strict {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(message)]);
        }
        return VerifyOutcome::from_findings(vec![VerifyFinding::warning(message)]);
    }
    VerifyOutcome::pass()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use std::path::Path;

    use domain::verify::Severity;
    use domain::{AdrVerifyReport, ChainId, ChainIdentity, LiveSoTChain, SoTChain};

    use super::{AdrUserChain, adr_report_to_outcome};

    // ── static trait-bound assertions ────────────────────────────────────────

    /// Assert AdrUserChain satisfies ChainIdentity + SoTChain + LiveSoTChain
    /// at the type level.  This function is never called; existence is
    /// sufficient to prove the bounds.
    fn _assert_chain_bounds<T>()
    where
        T: ChainIdentity + SoTChain + LiveSoTChain,
    {
    }

    #[test]
    fn test_adr_user_chain_satisfies_chain_identity_sotchain_livesotchain_bounds() {
        // Triggers the trait-bound check at compile time.
        _assert_chain_bounds::<AdrUserChain>();
    }

    #[test]
    fn test_adr_user_chain_id_is_adr_user() {
        assert_eq!(AdrUserChain::ID, ChainId::AdrUser);
    }

    // ── adr_report_to_outcome: gate logic ────────────────────────────────────

    #[test]
    fn test_adr_report_to_outcome_all_blue_passes() {
        let report = AdrVerifyReport::new(5, 0, 0, 1);
        let outcome = adr_report_to_outcome(&report, false);
        assert!(outcome.findings().is_empty(), "all-blue must pass: {outcome:?}");
    }

    #[test]
    fn test_adr_report_to_outcome_red_is_error_regardless_of_strict() {
        let report = AdrVerifyReport::new(0, 0, 2, 0);

        let outcome_interim = adr_report_to_outcome(&report, false);
        assert!(outcome_interim.has_errors(), "red must error in interim: {outcome_interim:?}");
        assert!(outcome_interim.findings()[0].message().contains("2 ADR decision"));

        let outcome_strict = adr_report_to_outcome(&report, true);
        assert!(outcome_strict.has_errors(), "red must error in strict: {outcome_strict:?}");
    }

    #[test]
    fn test_adr_report_to_outcome_yellow_is_warning_in_interim_mode() {
        let report = AdrVerifyReport::new(3, 2, 0, 0);
        let outcome = adr_report_to_outcome(&report, false);
        assert!(!outcome.has_errors(), "yellow in interim must not error: {outcome:?}");
        let findings = outcome.findings();
        assert_eq!(findings.len(), 1, "expected exactly one finding");
        assert_eq!(findings[0].severity(), Severity::Warning);
        assert!(findings[0].message().contains("2 ADR decision"));
        assert!(findings[0].message().contains("merge gate will block"));
    }

    #[test]
    fn test_adr_report_to_outcome_yellow_is_error_in_strict_mode() {
        let report = AdrVerifyReport::new(3, 1, 0, 0);
        let outcome = adr_report_to_outcome(&report, true);
        assert!(outcome.has_errors(), "yellow in strict must error: {outcome:?}");
        let findings = outcome.findings();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity(), Severity::Error);
    }

    #[test]
    fn test_adr_report_to_outcome_grandfathered_does_not_affect_gate() {
        // 10 grandfathered decisions should not block or warn.
        let report = AdrVerifyReport::new(5, 0, 0, 10);
        let outcome = adr_report_to_outcome(&report, true);
        assert!(outcome.findings().is_empty(), "grandfathered must not affect gate: {outcome:?}");
    }

    #[test]
    fn test_adr_report_to_outcome_red_takes_priority_over_yellow() {
        // When both red and yellow are non-zero, red error fires first.
        let report = AdrVerifyReport::new(0, 1, 1, 0);
        let outcome_interim = adr_report_to_outcome(&report, false);
        assert!(outcome_interim.has_errors(), "red > yellow must still error: {outcome_interim:?}");
        assert!(outcome_interim.findings()[0].message().contains("Red signal"));
    }

    // ── SoTChain::check delegates to calc_live ────────────────────────────────

    #[test]
    fn test_check_returns_error_when_calc_live_not_yet_wired() {
        // The placeholder impl returns an error; check must surface it.
        let path = Path::new("/tmp/project");
        let outcome = AdrUserChain::check(&path, false);
        assert!(outcome.has_errors(), "unwired calc_live must surface as error: {outcome:?}");
        assert!(
            outcome.findings()[0].message().contains("not yet wired"),
            "error must mention T006 wiring: {:?}",
            outcome.findings()[0].message()
        );
    }
}
