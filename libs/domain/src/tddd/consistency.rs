//! Stage 2 signal gate: `check_type_signals`.
//!
//! T008: `check_consistency`, `ConsistencyReport`, `ActionContradiction`,
//! `ActionContradictionKind` are deleted — they depended on `TypeGraph` and
//! `TypeBaseline` which are removed. Only `check_type_signals` is kept here
//! because it is a pure function over `TypeCatalogueDocument` (the old catalogue)
//! and is still used by `spec_states` verify and `usecase::merge_gate`.

use std::collections::{HashMap, HashSet};

use crate::ConfidenceSignal;
use crate::tddd::catalogue::{TypeCatalogueDocument, TypeDefinitionKind};
use crate::verify::{VerifyFinding, VerifyOutcome};

// ---------------------------------------------------------------------------
// Stage 2 signal gate (check_type_signals)
// ---------------------------------------------------------------------------

/// Evaluates Stage 2 signal gate rules against a `TypeCatalogueDocument`.
///
/// Shared pure function used by both the CI path (`verify_from_spec_json`
/// Stage 2) and the merge gate (via `usecase::merge_gate`). The caller is
/// responsible for handling the `NotFound` case (no catalogue = TDDD not
/// active for the track/layer, per ADR §D2.1 opt-in model).
///
/// # Rules
///
/// - `entries` is empty → `VerifyOutcome::pass()` (unless reverse-Red is present)
/// - `signals` is `None` → `VerifyFinding::error` (unevaluated; run `sotp track type-signals`)
/// - Signal coverage incomplete (entry has no matching signal) → `VerifyFinding::error`
/// - Any Red signal (forward or reverse) → `VerifyFinding::error`
/// - Declared-entry Yellow signal, `strict = true` → `VerifyFinding::error`
/// - Declared-entry Yellow signal, `strict = false` → `VerifyFinding::warning`
/// - All Blue / no declared Yellow → `VerifyOutcome::pass()`
///
/// Reference: ADR `knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md` §D2, §D8.6.
#[must_use]
pub fn check_type_signals(
    doc: &TypeCatalogueDocument,
    strict: bool,
    catalogue_file: &str,
) -> VerifyOutcome {
    // ADR 2026-04-19-1242 §D6.4: empty catalogues (zero type declarations) are a
    // valid state for tracks that only reuse pre-existing types.
    if doc.entries().is_empty() {
        let Some(signals) = doc.signals() else {
            return VerifyOutcome::pass();
        };
        let reds: Vec<&str> = signals
            .iter()
            .filter(|s| s.signal() == ConfidenceSignal::Red)
            .map(|s| s.type_name())
            .collect();
        if reds.is_empty() {
            return VerifyOutcome::pass();
        }
        return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
            "{catalogue_file}: {} type(s) have Red signal (reverse-direction drift on empty catalogue — add the types to the catalogue or remove them from code): {}",
            reds.len(),
            reds.join(", ")
        ))]);
    }

    let Some(signals) = doc.signals() else {
        return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
            "{catalogue_file}: type signals not yet evaluated — run `sotp track type-signals` first",
        ))]);
    };

    // Signal coverage check.
    let signal_keys: HashSet<(&str, &str)> =
        signals.iter().map(|s| (s.type_name(), s.kind_tag())).collect();

    // Multiset: how many free_function signals exist per short name.
    let mut free_fn_signal_counts: HashMap<&str, usize> = HashMap::new();
    for sig in signals.iter().filter(|s| s.kind_tag() == "free_function") {
        *free_fn_signal_counts.entry(sig.type_name()).or_insert(0) += 1;
    }
    let mut uncovered_free_fn_names: Vec<String> = Vec::new();
    let mut free_fn_entry_counts: HashMap<&str, usize> = HashMap::new();
    for entry in
        doc.entries().iter().filter(|e| matches!(e.kind(), TypeDefinitionKind::FreeFunction { .. }))
    {
        *free_fn_entry_counts.entry(entry.name()).or_insert(0) += 1;
    }
    for (name, &needed) in &free_fn_entry_counts {
        let present = free_fn_signal_counts.get(name).copied().unwrap_or(0);
        if present < needed {
            uncovered_free_fn_names.push(name.to_string());
        }
    }

    let uncovered_non_free_fn: Vec<&str> = doc
        .entries()
        .iter()
        .filter(|e| !matches!(e.kind(), TypeDefinitionKind::FreeFunction { .. }))
        .filter(|e| !signal_keys.contains(&(e.name(), e.kind().kind_tag())))
        .map(|e| e.name())
        .collect();

    if !uncovered_non_free_fn.is_empty() || !uncovered_free_fn_names.is_empty() {
        let mut uncovered_names: Vec<String> =
            uncovered_non_free_fn.iter().map(|n| n.to_string()).collect();
        uncovered_names.extend(uncovered_free_fn_names);
        return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
            "{catalogue_file}: {} type(s) have no signal evaluation: {} — re-run `sotp track type-signals`",
            uncovered_names.len(),
            uncovered_names.join(", ")
        ))]);
    }

    // Red check: applies to all signals (forward + reverse).
    let all_red: Vec<&str> = signals
        .iter()
        .filter(|s| s.signal() == ConfidenceSignal::Red)
        .map(|s| s.type_name())
        .collect();
    if !all_red.is_empty() {
        return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
            "{catalogue_file}: {} type(s) have Red signal (TDDD violation — run /track:design): {}",
            all_red.len(),
            all_red.join(", ")
        ))]);
    }

    // Yellow check: declared entries only.
    let entry_keys: HashSet<(&str, &str)> = doc
        .entries()
        .iter()
        .filter(|e| !matches!(e.kind(), TypeDefinitionKind::FreeFunction { .. }))
        .map(|e| (e.name(), e.kind().kind_tag()))
        .collect();
    let yellow_non_free_fn: Vec<&str> = signals
        .iter()
        .filter(|s| s.kind_tag() != "free_function")
        .filter(|s| entry_keys.contains(&(s.type_name(), s.kind_tag())))
        .filter(|s| s.signal() == ConfidenceSignal::Yellow)
        .map(|s| s.type_name())
        .collect();
    // Only count Yellow signals for declared FreeFunction entry names (mirrors
    // the non-free-function path that intersects with entry_keys before counting).
    let yellow_free_fn_count = signals
        .iter()
        .filter(|s| {
            s.kind_tag() == "free_function"
                && s.signal() == ConfidenceSignal::Yellow
                && free_fn_entry_counts.contains_key(s.type_name())
        })
        .count();
    let yellow_entries: Vec<String> = {
        let mut v: Vec<String> = yellow_non_free_fn.iter().map(|n| n.to_string()).collect();
        if yellow_free_fn_count > 0 {
            v.push(format!("{yellow_free_fn_count} FreeFunction(s)"));
        }
        v
    };

    if !yellow_entries.is_empty() {
        let message = format!(
            "{catalogue_file}: {} declared type(s) have Yellow signal: {} — merge gate will block these until upgraded to Blue. Resolve each type (implement or remove per its declared action) and re-run `sotp track type-signals`.",
            yellow_entries.len(),
            yellow_entries.join(", ")
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
    use crate::tddd::catalogue::{
        TypeAction, TypeCatalogueDocument, TypeCatalogueEntry, TypeDefinitionKind, TypeSignal,
    };

    fn make_entry(name: &str) -> TypeCatalogueEntry {
        TypeCatalogueEntry::new(
            name,
            "test entry",
            TypeDefinitionKind::ValueObject {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
            TypeAction::Add,
            true,
        )
        .unwrap()
    }

    fn make_signal(name: &str, signal: ConfidenceSignal) -> TypeSignal {
        TypeSignal::new(name, "value_object", signal, true, Vec::new(), Vec::new(), Vec::new())
    }

    #[test]
    fn test_check_type_signals_empty_entries_passes_per_adr_d64() {
        let doc = TypeCatalogueDocument::new(1, Vec::new());
        let outcome = check_type_signals(&doc, false, "domain-types.json");
        assert!(outcome.findings().is_empty(), "empty entries must pass per D6.4");
    }

    #[test]
    fn test_check_type_signals_empty_entries_with_red_signals_blocks() {
        let mut doc = TypeCatalogueDocument::new(1, Vec::new());
        doc.set_signals(vec![TypeSignal::new(
            "UndeclaredType",
            "undeclared_type",
            ConfidenceSignal::Red,
            true,
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )]);
        let outcome = check_type_signals(&doc, false, "domain-types.json");
        assert!(outcome.has_errors(), "empty entries + red reverse signal must be an error");
        let msg = outcome.findings()[0].message();
        assert!(msg.contains("domain-types.json"), "must mention catalogue file: {msg}");
        assert!(msg.contains("UndeclaredType"), "must mention the offending type: {msg}");
        assert!(msg.contains("reverse-direction drift"), "must name the condition: {msg}");
    }

    #[test]
    fn test_check_type_signals_empty_entries_with_yellow_only_passes() {
        let mut doc = TypeCatalogueDocument::new(1, Vec::new());
        doc.set_signals(vec![TypeSignal::new(
            "UndeclaredType",
            "undeclared_type",
            ConfidenceSignal::Yellow,
            false,
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )]);
        let outcome = check_type_signals(&doc, false, "domain-types.json");
        assert!(outcome.findings().is_empty(), "empty entries + yellow-only reverse must pass");
    }

    #[test]
    fn test_check_type_signals_none_signals_returns_error() {
        let doc = TypeCatalogueDocument::new(1, vec![make_entry("TrackId")]);
        let outcome = check_type_signals(&doc, false, "domain-types.json");
        assert!(outcome.has_errors(), "None signals must be an error");
        assert!(outcome.findings()[0].message().contains("not yet evaluated"));
    }

    #[test]
    fn test_check_type_signals_coverage_gap_returns_error() {
        let mut doc =
            TypeCatalogueDocument::new(1, vec![make_entry("TrackId"), make_entry("ReviewState")]);
        doc.set_signals(vec![make_signal("TrackId", ConfidenceSignal::Blue)]);
        let outcome = check_type_signals(&doc, false, "domain-types.json");
        assert!(outcome.has_errors());
        let msg = outcome.findings()[0].message();
        assert!(msg.contains("no signal evaluation"), "message: {msg}");
        assert!(msg.contains("ReviewState"));
    }

    #[test]
    fn test_check_type_signals_red_is_error_regardless_of_mode() {
        let mut doc = TypeCatalogueDocument::new(1, vec![make_entry("TrackId")]);
        doc.set_signals(vec![make_signal("TrackId", ConfidenceSignal::Red)]);
        let outcome_interim = check_type_signals(&doc, false, "domain-types.json");
        assert!(outcome_interim.has_errors(), "red in interim must be an error");
        let outcome_strict = check_type_signals(&doc, true, "domain-types.json");
        assert!(outcome_strict.has_errors(), "red in strict must be an error");
    }

    #[test]
    fn test_check_type_signals_yellow_is_warning_in_interim_mode() {
        let mut doc =
            TypeCatalogueDocument::new(1, vec![make_entry("TrackId"), make_entry("ReviewState")]);
        doc.set_signals(vec![
            make_signal("TrackId", ConfidenceSignal::Blue),
            make_signal("ReviewState", ConfidenceSignal::Yellow),
        ]);
        let outcome = check_type_signals(&doc, false, "domain-types.json");
        assert!(!outcome.has_errors(), "yellow in interim must not be an error");
        let findings = outcome.findings();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity(), crate::verify::Severity::Warning);
        let msg = findings[0].message();
        assert!(msg.contains("1 declared type"), "must mention count: {msg}");
        assert!(msg.contains("ReviewState"), "must list the type name: {msg}");
        assert!(msg.contains("merge gate will block"), "must warn: {msg}");
    }

    #[test]
    fn test_check_type_signals_yellow_is_error_in_strict_mode() {
        let mut doc = TypeCatalogueDocument::new(1, vec![make_entry("TrackId")]);
        doc.set_signals(vec![make_signal("TrackId", ConfidenceSignal::Yellow)]);
        let outcome = check_type_signals(&doc, true, "domain-types.json");
        assert!(outcome.has_errors());
        let findings = outcome.findings();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity(), crate::verify::Severity::Error);
        assert!(findings[0].message().contains("TrackId"));
    }

    #[test]
    fn test_check_type_signals_all_blue_passes_in_both_modes() {
        let mut doc =
            TypeCatalogueDocument::new(1, vec![make_entry("TrackId"), make_entry("ReviewState")]);
        doc.set_signals(vec![
            make_signal("TrackId", ConfidenceSignal::Blue),
            make_signal("ReviewState", ConfidenceSignal::Blue),
        ]);

        let outcome_interim = check_type_signals(&doc, false, "domain-types.json");
        assert!(!outcome_interim.has_errors());
        assert!(outcome_interim.findings().is_empty());

        let outcome_strict = check_type_signals(&doc, true, "domain-types.json");
        assert!(!outcome_strict.has_errors());
        assert!(outcome_strict.findings().is_empty());
    }

    #[test]
    fn test_check_type_signals_undeclared_yellow_is_not_blocked() {
        let mut doc = TypeCatalogueDocument::new(1, vec![make_entry("TrackId")]);
        doc.set_signals(vec![
            make_signal("TrackId", ConfidenceSignal::Blue),
            TypeSignal::new(
                "UndeclaredType",
                "undeclared_type",
                ConfidenceSignal::Yellow,
                false,
                Vec::new(),
                Vec::new(),
                Vec::new(),
            ),
        ]);

        let outcome_strict = check_type_signals(&doc, true, "domain-types.json");
        assert!(
            !outcome_strict.has_errors(),
            "undeclared Yellow must not block even in strict mode: {outcome_strict:?}"
        );
        assert!(outcome_strict.findings().is_empty());
    }

    #[test]
    fn test_check_type_signals_free_function_covered_by_short_name() {
        let entry = TypeCatalogueEntry::new(
            "save_track",
            "desc",
            TypeDefinitionKind::FreeFunction {
                module_path: Some("usecase::track".to_string()),
                expected_params: vec![],
                expected_returns: vec![],
                expected_is_async: false,
            },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let mut doc = TypeCatalogueDocument::new(1, vec![entry]);
        doc.set_signals(vec![TypeSignal::new(
            "save_track",
            "free_function",
            ConfidenceSignal::Blue,
            true,
            vec![],
            vec![],
            vec![],
        )]);
        let outcome = check_type_signals(&doc, false, "domain-types.json");
        assert!(
            outcome.findings().is_empty(),
            "short-name signal must satisfy FreeFunction coverage check: {outcome:?}"
        );
    }

    #[test]
    fn test_check_type_signals_free_function_multiset_coverage_detects_missing_signal() {
        let entry_a = TypeCatalogueEntry::new(
            "save",
            "desc",
            TypeDefinitionKind::FreeFunction {
                module_path: Some("usecase::track".to_string()),
                expected_params: vec![],
                expected_returns: vec![],
                expected_is_async: false,
            },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let entry_b = TypeCatalogueEntry::new(
            "save",
            "desc",
            TypeDefinitionKind::FreeFunction {
                module_path: Some("usecase::spec".to_string()),
                expected_params: vec![],
                expected_returns: vec![],
                expected_is_async: false,
            },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let mut doc = TypeCatalogueDocument::new(1, vec![entry_a, entry_b]);

        doc.set_signals(vec![TypeSignal::new(
            "save",
            "free_function",
            ConfidenceSignal::Blue,
            true,
            vec![],
            vec![],
            vec![],
        )]);
        let outcome_one_signal = check_type_signals(&doc, false, "domain-types.json");
        assert!(
            outcome_one_signal.has_errors(),
            "one save signal for two save entries must report missing coverage: {outcome_one_signal:?}"
        );

        doc.set_signals(vec![
            TypeSignal::new(
                "save",
                "free_function",
                ConfidenceSignal::Blue,
                true,
                vec![],
                vec![],
                vec![],
            ),
            TypeSignal::new(
                "delete_track",
                "free_function",
                ConfidenceSignal::Blue,
                true,
                vec![],
                vec![],
                vec![],
            ),
        ]);
        let outcome_wrong_name = check_type_signals(&doc, false, "domain-types.json");
        assert!(
            outcome_wrong_name.has_errors(),
            "wrong-name signal must not cover a different entry: {outcome_wrong_name:?}"
        );

        doc.set_signals(vec![
            TypeSignal::new(
                "save",
                "free_function",
                ConfidenceSignal::Blue,
                true,
                vec![],
                vec![],
                vec![],
            ),
            TypeSignal::new(
                "save",
                "free_function",
                ConfidenceSignal::Blue,
                true,
                vec![],
                vec![],
                vec![],
            ),
        ]);
        let outcome_two_signals = check_type_signals(&doc, false, "domain-types.json");
        assert!(
            outcome_two_signals.findings().is_empty(),
            "two save signals for two save entries must pass coverage check: {outcome_two_signals:?}"
        );
    }

    #[test]
    fn test_check_type_signals_undeclared_free_fn_yellow_is_not_blocked() {
        // A stale Yellow signal with kind_tag=="free_function" but whose type_name
        // has no corresponding declared FreeFunction entry must NOT block verification.
        // This mirrors the non-free-fn path's intersection with entry_keys.
        let mut doc = TypeCatalogueDocument::new(1, vec![make_entry("TrackId")]);
        doc.set_signals(vec![
            make_signal("TrackId", ConfidenceSignal::Blue),
            TypeSignal::new(
                "stale_free_fn",
                "free_function",
                ConfidenceSignal::Yellow,
                false,
                Vec::new(),
                Vec::new(),
                Vec::new(),
            ),
        ]);
        let outcome_strict = check_type_signals(&doc, true, "domain-types.json");
        assert!(
            !outcome_strict.has_errors(),
            "undeclared FreeFunction Yellow must not block even in strict mode: {outcome_strict:?}"
        );
        assert!(outcome_strict.findings().is_empty());
    }

    #[test]
    fn test_check_type_signals_yellow_error_mentions_catalogue_file() {
        let mut doc = TypeCatalogueDocument::new(1, vec![make_entry("TrackId")]);
        doc.set_signals(vec![make_signal("TrackId", ConfidenceSignal::Yellow)]);
        let outcome = check_type_signals(&doc, true, "infrastructure-types.json");
        assert!(outcome.has_errors());
        let msg = outcome.findings()[0].message();
        assert!(msg.contains("infrastructure-types.json"), "must mention caller file: {msg}");
        assert!(!msg.contains("domain-types.json"), "must NOT hardcode domain-types.json: {msg}");
    }
}
