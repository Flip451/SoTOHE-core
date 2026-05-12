//! Stage 2 signal gate: `check_type_signals`.
//!
//! T022: `check_type_signals` is now a pure function over `TypeSignalsDocument`
//! (the evaluation-result document), independent of the catalogue declaration.
//! The coverage check and declared-entry-only Yellow filter are removed because
//! the `declaration_hash` freshness check in callers already guarantees that the
//! signal file was generated from the current declaration bytes, so every signal
//! present in the document corresponds to a live declaration (no stale/missing
//! coverage gap is possible). The signal evaluator only emits Yellow for
//! declared entries (ADR 2026-05-11-2330 §D4), so all Yellows count.

use crate::ConfidenceSignal;
use crate::tddd::type_signals_doc::TypeSignalsDocument;
use crate::verify::{VerifyFinding, VerifyOutcome};

// ---------------------------------------------------------------------------
// Stage 2 signal gate (check_type_signals)
// ---------------------------------------------------------------------------

/// Evaluates Stage 2 signal gate rules against a [`TypeSignalsDocument`].
///
/// Pure function used by both the CI path (`verify_from_spec_json` Stage 2)
/// and the merge gate (`check_strict_merge_gate` Stage 2). Callers are
/// responsible for the `declaration_hash` freshness check (catalogue bytes
/// SHA-256 must match `signals_doc.declaration_hash()`) before calling this
/// function — a stale document must never reach this gate.
///
/// # Rules
///
/// - No signals → `VerifyOutcome::pass()` (empty declaration; opt-in model)
/// - Any Red signal → `VerifyFinding::error` (unconditional, all regions)
/// - Yellow signal, `strict = true` → `VerifyFinding::error`
/// - Yellow signal, `strict = false` → `VerifyFinding::warning`
/// - All Blue / no Yellow → `VerifyOutcome::pass()`
///
/// Note: the coverage check (every declared entry has a matching signal) and
/// the declared-entry-only Yellow filter are intentionally absent. In v3 the
/// signal evaluator guarantees that signals are always current relative to the
/// declaration (enforced by the `declaration_hash` freshness check in callers),
/// so a coverage gap cannot exist after a fresh evaluation run. All Yellows
/// in the document correspond to declared entries per ADR 2026-05-11-2330 §D4.
///
/// # Errors
///
/// Returns findings when Red signals are present (both modes) or Yellow signals
/// are present in strict mode.
///
/// Reference: ADR `knowledge/adr/2026-05-11-2330-catalogue-impl-signals-command-layering.md` §D4.
#[must_use]
pub fn check_type_signals(signals_doc: &TypeSignalsDocument, strict: bool) -> VerifyOutcome {
    let signals = signals_doc.signals();

    // Empty signals → pass (empty catalogue / no declarations).
    // ADR 2026-04-19-1242 §D6.4: zero type declarations are valid for tracks
    // that only reuse pre-existing types. An empty signal list here means the
    // evaluator found nothing to report, which is a clean state.
    if signals.is_empty() {
        return VerifyOutcome::pass();
    }

    // Red check: applies to all signals (forward + reverse, all regions).
    let all_red: Vec<&str> = signals
        .iter()
        .filter(|s| s.signal() == ConfidenceSignal::Red)
        .map(|s| s.type_name())
        .collect();
    if !all_red.is_empty() {
        return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
            "{} type(s) have Red signal (TDDD violation — run /track:design): {}",
            all_red.len(),
            all_red.join(", ")
        ))]);
    }

    // Yellow check: all Yellow signals count (the evaluator only emits Yellow
    // for declared entries per ADR 2026-05-11-2330 §D4; no declared-entry
    // filter is needed).
    let all_yellow: Vec<&str> = signals
        .iter()
        .filter(|s| s.signal() == ConfidenceSignal::Yellow)
        .map(|s| s.type_name())
        .collect();

    if !all_yellow.is_empty() {
        let message = format!(
            "{} type(s) have Yellow signal: {} — merge gate will block these until upgraded to Blue. Resolve each type (implement or remove per its declared action) and re-run `sotp track type-signals`.",
            all_yellow.len(),
            all_yellow.join(", ")
        );
        if strict {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(message)]);
        }
        return VerifyOutcome::from_findings(vec![VerifyFinding::warning(message)]);
    }

    VerifyOutcome::pass()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::ConfidenceSignal;
    use crate::Timestamp;
    use crate::tddd::catalogue::TypeSignal;
    use crate::tddd::type_signals_doc::TypeSignalsDocument;

    fn ts() -> Timestamp {
        Timestamp::new("2026-05-08T00:00:00Z").unwrap()
    }

    fn make_signal(name: &str, kind: &str, signal: ConfidenceSignal) -> TypeSignal {
        TypeSignal::new(name, kind, signal, true, Vec::new(), Vec::new(), Vec::new())
    }

    fn make_doc(signals: Vec<TypeSignal>) -> TypeSignalsDocument {
        TypeSignalsDocument::new(ts(), "deadbeef", signals)
    }

    #[test]
    fn test_check_type_signals_empty_signals_passes_per_adr_d64() {
        // Empty signal list → pass (no declarations / empty catalogue opt-in).
        let doc = make_doc(vec![]);
        let outcome = check_type_signals(&doc, false);
        assert!(outcome.findings().is_empty(), "empty signals must pass per D6.4: {outcome:?}");
    }

    #[test]
    fn test_check_type_signals_red_is_error_regardless_of_mode() {
        let doc = make_doc(vec![make_signal("TrackId", "value_object", ConfidenceSignal::Red)]);
        let outcome_interim = check_type_signals(&doc, false);
        assert!(
            outcome_interim.has_errors(),
            "red in interim must be an error: {outcome_interim:?}"
        );
        let outcome_strict = check_type_signals(&doc, true);
        assert!(outcome_strict.has_errors(), "red in strict must be an error: {outcome_strict:?}");
    }

    #[test]
    fn test_check_type_signals_yellow_is_warning_in_interim_mode() {
        let doc = make_doc(vec![
            make_signal("TrackId", "value_object", ConfidenceSignal::Blue),
            make_signal("ReviewState", "value_object", ConfidenceSignal::Yellow),
        ]);
        let outcome = check_type_signals(&doc, false);
        assert!(!outcome.has_errors(), "yellow in interim must not be an error: {outcome:?}");
        let findings = outcome.findings();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity(), crate::verify::Severity::Warning);
        let msg = findings[0].message();
        assert!(msg.contains("1 type(s)"), "must mention count: {msg}");
        assert!(msg.contains("ReviewState"), "must list the type name: {msg}");
        assert!(msg.contains("merge gate will block"), "must warn: {msg}");
    }

    #[test]
    fn test_check_type_signals_yellow_is_error_in_strict_mode() {
        let doc = make_doc(vec![make_signal("TrackId", "value_object", ConfidenceSignal::Yellow)]);
        let outcome = check_type_signals(&doc, true);
        assert!(outcome.has_errors(), "yellow in strict must be an error: {outcome:?}");
        let findings = outcome.findings();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity(), crate::verify::Severity::Error);
        assert!(findings[0].message().contains("TrackId"), "must name the type: {:?}", findings[0]);
    }

    #[test]
    fn test_check_type_signals_all_blue_passes_in_both_modes() {
        let doc = make_doc(vec![
            make_signal("TrackId", "value_object", ConfidenceSignal::Blue),
            make_signal("ReviewState", "value_object", ConfidenceSignal::Blue),
        ]);

        let outcome_interim = check_type_signals(&doc, false);
        assert!(!outcome_interim.has_errors(), "all-blue interim: {outcome_interim:?}");
        assert!(outcome_interim.findings().is_empty());

        let outcome_strict = check_type_signals(&doc, true);
        assert!(!outcome_strict.has_errors(), "all-blue strict: {outcome_strict:?}");
        assert!(outcome_strict.findings().is_empty());
    }

    /// D4 regression: documents that `check_type_signals(signals_doc, strict=false)`
    /// blocks on an "undeclared implementation" (`C\(S∪D)` region → 🔴 signal).
    ///
    /// ADR `2026-05-11-2330` §D4 states:
    ///   "Undeclared implementations in the `C\(S∪D)` region, which are 🔴 entries in
    ///   `<layer>-type-signals.json`, are blocked by the existing commit gate
    ///   (`check_type_signals(doc, strict=false)`) unconditionally."
    ///
    /// A `CMinusSUnionD` entry corresponds to a type present in the current code (C)
    /// but not declared in the catalogue (S) or as a delete target (D). In v3 this
    /// appears as a Red signal in the `TypeSignalsDocument` for an undeclared type.
    #[test]
    fn test_check_type_signals_cminussunion_d_red_blocks_commit_gate_d4_regression() {
        // A TypeSignalsDocument that contains a Red entry for "UndeclaredImpl"
        // (the CMinusSUnionD region: present in C but absent from S ∪ D).
        let doc = make_doc(vec![TypeSignal::new(
            "UndeclaredImpl",
            "undeclared_type",
            ConfidenceSignal::Red,
            /* found_type = */ true,
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )]);

        // Even in non-strict (commit-gate) mode, a Red signal blocks.
        let outcome = check_type_signals(&doc, /* strict = */ false);
        assert!(
            outcome.has_errors(),
            "CMinusSUnionD Red signal must block commit gate (has_errors must be true): {outcome:?}"
        );
        // Error message must identify the undeclared type.
        let msg = outcome.findings()[0].message();
        assert!(
            msg.contains("UndeclaredImpl"),
            "error finding must name the undeclared implementation: {msg}"
        );
    }

    #[test]
    fn test_check_type_signals_red_in_empty_catalogue_blocks() {
        // A Red signal with an empty (zero-entry) catalogue scenario:
        // the evaluator still reports it (reverse-direction drift).
        let doc = make_doc(vec![TypeSignal::new(
            "UndeclaredType",
            "undeclared_type",
            ConfidenceSignal::Red,
            true,
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )]);
        let outcome = check_type_signals(&doc, false);
        assert!(outcome.has_errors(), "red reverse signal must be an error: {outcome:?}");
        let msg = outcome.findings()[0].message();
        assert!(msg.contains("UndeclaredType"), "must mention the offending type: {msg}");
    }

    #[test]
    fn test_check_type_signals_yellow_only_in_empty_catalogue_warns() {
        // Yellow signal (no declared entries) → warning in interim, error in strict.
        // In v3 the evaluator only emits Yellow for declared entries, but the gate
        // does not need to verify that invariant — it just applies the Yellow rule.
        let doc = make_doc(vec![TypeSignal::new(
            "SomeType",
            "value_object",
            ConfidenceSignal::Yellow,
            false,
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )]);
        let outcome_interim = check_type_signals(&doc, false);
        assert!(!outcome_interim.has_errors(), "yellow in interim must warn, not error");
        assert!(!outcome_interim.findings().is_empty(), "must have a warning finding");

        let outcome_strict = check_type_signals(&doc, true);
        assert!(outcome_strict.has_errors(), "yellow in strict must be an error");
    }
}
