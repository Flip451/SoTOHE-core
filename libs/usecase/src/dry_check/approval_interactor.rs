//! [`DryCheckApprovalInteractor`] — read-only D5 dry-check approval gate.
//!
//! Evaluates the dry-check approval gate from a pre-computed set of current
//! diff fragment `FragmentRef`s, the persisted coverage manifest, and the
//! dry-check verdict history. No embedding, no similarity search, no agent
//! invocation — composition computes the current `FragmentRef` set.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use domain::TrackId;
use domain::dry_check::{
    DryCheckApprovalVerdict, DryCheckConfigFingerprint, DryCheckCorpusFingerprint, DryCheckPairKey,
    DryCheckReader, DryCheckRecord, DryCheckVerdict, FragmentRef,
};

use super::errors::DryCheckCycleError;
use super::ports::DryCheckCoveragePort;
use super::services::DryCheckApprovalService;

const FAIL_CLOSED_CORPUS_FINGERPRINT_HEX: &str =
    "0000000000000000000000000000000000000000000000000000000000000000";

fn is_fail_closed_corpus_fingerprint(fingerprint: &DryCheckCorpusFingerprint) -> bool {
    fingerprint.as_str() == FAIL_CLOSED_CORPUS_FINGERPRINT_HEX
}

// ── DryCheckApprovalInteractor ────────────────────────────────────────────────

/// Interactor implementing the D5 read-only [`DryCheckApprovalService`].
///
/// Holds only a [`DryCheckReader`] (for verdict history), a
/// [`DryCheckCoveragePort`] (for the coverage manifest), and the
/// `current_config_fingerprint` (SHA-256 fingerprint of the current
/// `dry-check.json` settings). The implementation performs hash matching,
/// config-fingerprint comparison, and verdict scanning — no embedding / index
/// ports.
///
/// The constructor return type is written as `DryCheckApprovalInteractor` (not
/// `Self`) so the type-signal evaluator's exact-string match succeeds.
pub struct DryCheckApprovalInteractor {
    reader: Arc<dyn DryCheckReader>,
    coverage: Arc<dyn DryCheckCoveragePort>,
    current_config_fingerprint: DryCheckConfigFingerprint,
    current_corpus_fingerprint: DryCheckCorpusFingerprint,
}

impl DryCheckApprovalInteractor {
    /// Create a new [`DryCheckApprovalInteractor`].
    ///
    /// # Parameters
    ///
    /// - `reader`: port for reading the dry-check history (`DryCheckRecord`s).
    /// - `coverage`: port for reading the D5 coverage manifest.
    /// - `current_config_fingerprint`: fingerprint of the current
    ///   `dry-check.json` settings. `check_approved` compares this against the
    ///   fingerprint stored in the coverage manifest; a mismatch means the config
    ///   changed since the last `dry write` run → return `Blocked`.
    /// - `current_corpus_fingerprint`: SHA-256 fingerprint of the current corpus
    ///   (all `*.rs` files in the workspace). `check_approved` compares this
    ///   against the fingerprint stored in the coverage manifest; a mismatch means
    ///   the corpus changed (file added/removed/modified) since the last
    ///   `dry write` run → return `Blocked`.
    #[must_use]
    pub fn new(
        reader: Arc<dyn DryCheckReader>,
        coverage: Arc<dyn DryCheckCoveragePort>,
        current_config_fingerprint: DryCheckConfigFingerprint,
        current_corpus_fingerprint: DryCheckCorpusFingerprint,
    ) -> DryCheckApprovalInteractor {
        DryCheckApprovalInteractor {
            reader,
            coverage,
            current_config_fingerprint,
            current_corpus_fingerprint,
        }
    }
}

impl DryCheckApprovalService for DryCheckApprovalInteractor {
    /// Evaluate the dry-check gate for the current diff scope (pure-read).
    ///
    /// See [`DryCheckApprovalService::check_approved`] for the algorithm.
    fn check_approved(
        &self,
        track_id: &TrackId,
        current_fragment_refs: &BTreeSet<FragmentRef>,
    ) -> Result<DryCheckApprovalVerdict, DryCheckCycleError> {
        // ── Step 1: Read coverage manifest (CN-08 fail-closed when missing). ──
        let coverage_record = match self.coverage.read_coverage(track_id)? {
            Some(record) => record,
            None => {
                // Convention: report 1 unresolved pair to communicate "Blocked"
                // without overstating the actual count (the count is unknown
                // at this stage — there is no manifest to compare against).
                return Ok(DryCheckApprovalVerdict::Blocked { unresolved_pair_count: 1 });
            }
        };

        // ── Step 1b: Config fingerprint — reject coverage built under a different config.
        //
        // When `.harness/config/dry-check.json` changes (e.g., threshold lowered from
        // 0.85 to 0.70) without changing source fragment contents, the old coverage
        // manifest is still fragment-ref–fresh (all current FragmentRefs are covered)
        // but was computed under a different threshold: new candidate pairs that would
        // be in-scope under the new threshold are silently absent.
        //
        // By comparing fingerprints here we detect the config change and return Blocked
        // immediately, forcing a fresh `dry write` run with the new config.  We report
        // `unresolved_pair_count: 1` to mirror the missing-manifest fail-closed pattern
        // (count unknown at this stage).
        if coverage_record.config_fingerprint() != &self.current_config_fingerprint {
            return Ok(DryCheckApprovalVerdict::Blocked { unresolved_pair_count: 1 });
        }

        // ── Step 1c: Corpus fingerprint — reject coverage built under a different corpus.
        //
        // When the workspace corpus changes (a `*.rs` file is added, removed, or
        // modified) while the diff fragments keep the same `(path, content_hash)`,
        // the old coverage manifest looks fragment-ref–fresh but was computed against
        // a different corpus: new corpus pairs that would be in-scope are silently
        // absent.
        //
        // By comparing the stored corpus fingerprint against the current one we detect
        // the corpus change and return Blocked immediately, forcing a fresh `dry write`
        // run.  We report `unresolved_pair_count: 1` to mirror the missing-manifest
        // fail-closed pattern (count unknown at this stage).
        if is_fail_closed_corpus_fingerprint(coverage_record.corpus_fingerprint())
            || is_fail_closed_corpus_fingerprint(&self.current_corpus_fingerprint)
            || coverage_record.corpus_fingerprint() != &self.current_corpus_fingerprint
        {
            return Ok(DryCheckApprovalVerdict::Blocked { unresolved_pair_count: 1 });
        }

        // ── Step 2: Staleness — every current FragmentRef must be covered. ────
        //
        // FragmentRef = (path + content_hash) identity (IN-06 / CN-08): an
        // identical content_hash at a different path is NOT covered.
        let mut stale_count = 0usize;
        for fragment_ref in current_fragment_refs {
            if !coverage_record.covers(fragment_ref) {
                stale_count += 1;
            }
        }
        if stale_count > 0 {
            return Ok(DryCheckApprovalVerdict::Blocked { unresolved_pair_count: stale_count });
        }

        // ── Step 3: All-resolved — latest-per-pair verdict scan. ──────────────
        //
        // Build the latest-per-pair map: history is iterated in order; later
        // records overwrite earlier ones for the same `DryCheckPairKey`. This
        // is the last-write-wins semantics used by `DryCheckResultsInteractor`.
        let records = self.reader.read_records().map_err(DryCheckCycleError::Reader)?;

        let mut latest_per_pair: BTreeMap<DryCheckPairKey, DryCheckRecord> = BTreeMap::new();
        for record in records {
            latest_per_pair.insert(record.pair_key().clone(), record);
        }

        // Count latest-violation records whose pair touches any current
        // FragmentRef AND whose pair_key was re-examined in the latest `dry write`
        // run (i.e., it is present in the coverage manifest's processed_pair_keys).
        //
        // `touches_current` ensures we do not block on Violations between old
        // fragments unrelated to the current diff.
        //
        // `coverage_record.contains_pair` filters out stale historical Violations
        // whose candidate side was fixed or removed: when the candidate side is
        // fixed, the `DryCheckPairKey` changes (new content_hash → new FragmentRef
        // → new pair_key), so the old pair_key is no longer produced by `dry write`
        // and therefore absent from `processed_pair_keys`. Without this filter the
        // gate would stay Blocked forever even though the violation is resolved.
        let mut active_violation_pair_keys = BTreeSet::new();
        for record in latest_per_pair.values() {
            let touches_current = current_fragment_refs.contains(record.pair_key().low())
                || current_fragment_refs.contains(record.pair_key().high());
            if !touches_current {
                continue;
            }
            // Skip records whose pair_key was not re-judged in the latest run
            // (stale candidate-side pair). Only Violations that were actively
            // re-examined can block the gate.
            if !coverage_record.contains_pair(record.pair_key()) {
                continue;
            }
            if matches!(record.verdict(), DryCheckVerdict::Violation { .. }) {
                active_violation_pair_keys.insert(record.pair_key().clone());
            }
        }

        // ── Step 4: Processed-pair verdict freshness scan. ─────────────────────
        //
        // Without this scan, a manifest whose fingerprints and FragmentRefs are
        // consistent but whose companion `dry-check.json` is missing or truncated
        // (e.g. partial restore, manual edit) would Approve silently — the loop
        // above only visits pairs that actually have records, so a processed_pair
        // with no record at all falls through. Treat any such missing verdict as
        // unresolved. The same applies to records whose verdict was produced under
        // a different config fingerprint: they are stale for this manifest even if
        // their pair key is listed in `processed_pair_keys`.
        let mut stale_or_missing_verdict_pairs = 0usize;
        for pair_key in coverage_record.processed_pair_keys() {
            let touches_current = current_fragment_refs.contains(pair_key.low())
                || current_fragment_refs.contains(pair_key.high());
            if !touches_current {
                continue;
            }
            if active_violation_pair_keys.contains(pair_key) {
                continue;
            }
            match latest_per_pair.get(pair_key) {
                Some(record) if record.config_fingerprint() == &self.current_config_fingerprint => {
                    continue;
                }
                Some(_) | None => stale_or_missing_verdict_pairs += 1,
            }
        }
        let unresolved_pair_count =
            active_violation_pair_keys.len() + stale_or_missing_verdict_pairs;
        if unresolved_pair_count > 0 {
            return Ok(DryCheckApprovalVerdict::Blocked { unresolved_pair_count });
        }

        Ok(DryCheckApprovalVerdict::Approved)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use domain::dry_check::{
        DryCheckConfigFingerprint, DryCheckCorpusFingerprint, DryCheckCoverageRecord,
        DryCheckPairKey, DryCheckReaderError, DryCheckRecord, DryCheckVerdict, FragmentRef,
        RefactorProposal,
    };

    use crate::dry_check::shared::test_mocks::{
        make_dry_check_record_for_tests, make_fragment_ref_for_tests,
    };

    // ── Test doubles ──────────────────────────────────────────────────────────

    struct StubReader {
        records: Vec<DryCheckRecord>,
    }

    impl DryCheckReader for StubReader {
        fn read_records(&self) -> Result<Vec<DryCheckRecord>, DryCheckReaderError> {
            Ok(self.records.clone())
        }
    }

    struct ErrorReader;

    impl DryCheckReader for ErrorReader {
        fn read_records(&self) -> Result<Vec<DryCheckRecord>, DryCheckReaderError> {
            Err(DryCheckReaderError::Io {
                path: "dry-check.json".to_owned(),
                detail: "simulated io error".to_owned(),
            })
        }
    }

    /// Coverage port that returns a fixed `Option<DryCheckCoverageRecord>`.
    struct StubCoverage {
        record: Option<DryCheckCoverageRecord>,
    }

    impl DryCheckCoveragePort for StubCoverage {
        fn read_coverage(
            &self,
            _track_id: &TrackId,
        ) -> Result<Option<DryCheckCoverageRecord>, DryCheckCycleError> {
            Ok(self.record.clone())
        }
        fn write_coverage(
            &self,
            _track_id: &TrackId,
            _record: DryCheckCoverageRecord,
        ) -> Result<(), DryCheckCycleError> {
            panic!("approval interactor never writes coverage")
        }
    }

    /// Coverage port that returns an error.
    struct ErrorCoverage;

    impl DryCheckCoveragePort for ErrorCoverage {
        fn read_coverage(
            &self,
            _track_id: &TrackId,
        ) -> Result<Option<DryCheckCoverageRecord>, DryCheckCycleError> {
            Err(DryCheckCycleError::CoveragePort("simulated read error".to_owned()))
        }
        fn write_coverage(
            &self,
            _track_id: &TrackId,
            _record: DryCheckCoverageRecord,
        ) -> Result<(), DryCheckCycleError> {
            panic!("approval interactor never writes coverage")
        }
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn make_track() -> TrackId {
        TrackId::try_new("test-track-2026").unwrap()
    }

    /// The canonical "current config fingerprint" used in tests that do not test
    /// fingerprint mismatch behaviour (all other tests use this so they agree).
    fn test_fingerprint() -> DryCheckConfigFingerprint {
        DryCheckConfigFingerprint::new("a".repeat(64)).unwrap()
    }

    /// The canonical "current corpus fingerprint" used in tests that do not test
    /// corpus fingerprint mismatch behaviour.
    fn test_corpus_fingerprint() -> DryCheckCorpusFingerprint {
        DryCheckCorpusFingerprint::new("c".repeat(64)).unwrap()
    }

    /// Default timestamp used by tests that do not care about the exact
    /// `recorded_at` value. Tests that DO care pass an explicit timestamp to
    /// [`make_dry_check_record_for_tests`] directly.
    const DEFAULT_RECORDED_AT: &str = "2026-06-13T00:00:00Z";

    fn make_interactor(
        coverage: StubCoverage,
        records: Vec<DryCheckRecord>,
    ) -> DryCheckApprovalInteractor {
        make_interactor_with_corpus_fingerprint(coverage, records, test_corpus_fingerprint())
    }

    fn make_interactor_with_corpus_fingerprint(
        coverage: StubCoverage,
        records: Vec<DryCheckRecord>,
        current_corpus_fingerprint: DryCheckCorpusFingerprint,
    ) -> DryCheckApprovalInteractor {
        DryCheckApprovalInteractor::new(
            Arc::new(StubReader { records }),
            Arc::new(coverage),
            test_fingerprint(),
            current_corpus_fingerprint,
        )
    }

    /// Build a `StubCoverage` covering `refs` with an empty `processed_pair_keys` set
    /// and the canonical test fingerprint.
    ///
    /// Use this for tests where no Violations exist in the history (no pair keys
    /// need to be present) or for staleness-only checks.
    fn coverage_with(refs: Vec<FragmentRef>) -> StubCoverage {
        coverage_with_pairs(refs, Vec::new())
    }

    /// Build a `StubCoverage` covering `refs` and marking `pair_keys` as processed,
    /// using the canonical test fingerprint.
    ///
    /// Use this for tests where a Violation record must be treated as active
    /// (i.e., the pair was re-judged in the latest `dry write` run).
    fn coverage_with_pairs(
        refs: Vec<FragmentRef>,
        pair_keys: Vec<DryCheckPairKey>,
    ) -> StubCoverage {
        coverage_with_pairs_and_fingerprint(refs, pair_keys, test_fingerprint())
    }

    /// Build a `StubCoverage` covering `refs`, marking `pair_keys` as processed,
    /// and using the provided `config_fingerprint` and `corpus_fingerprint`.
    fn coverage_with_pairs_and_fingerprints(
        refs: Vec<FragmentRef>,
        pair_keys: Vec<DryCheckPairKey>,
        config_fingerprint: DryCheckConfigFingerprint,
        corpus_fingerprint: DryCheckCorpusFingerprint,
    ) -> StubCoverage {
        StubCoverage {
            record: Some(DryCheckCoverageRecord::new(
                refs.into_iter().collect(),
                pair_keys.into_iter().collect(),
                config_fingerprint,
                corpus_fingerprint,
            )),
        }
    }

    /// Build a `StubCoverage` covering `refs`, marking `pair_keys` as processed,
    /// and using the provided `config_fingerprint` with the canonical corpus fingerprint.
    fn coverage_with_pairs_and_fingerprint(
        refs: Vec<FragmentRef>,
        pair_keys: Vec<DryCheckPairKey>,
        config_fingerprint: DryCheckConfigFingerprint,
    ) -> StubCoverage {
        coverage_with_pairs_and_fingerprints(
            refs,
            pair_keys,
            config_fingerprint,
            test_corpus_fingerprint(),
        )
    }

    fn current_refs(refs: Vec<FragmentRef>) -> BTreeSet<FragmentRef> {
        refs.into_iter().collect()
    }

    // ── coverage missing → Blocked ────────────────────────────────────────────

    #[test]
    fn test_check_approved_with_missing_coverage_returns_blocked() {
        let a = make_fragment_ref_for_tests("src/a.rs", 'a');
        let interactor = make_interactor(StubCoverage { record: None }, vec![]);
        let result = interactor.check_approved(&make_track(), &current_refs(vec![a])).unwrap();
        assert!(matches!(result, DryCheckApprovalVerdict::Blocked { .. }));
    }

    // ── all current refs covered, no records → Approved ──────────────────────

    #[test]
    fn test_check_approved_all_covered_no_records_returns_approved() {
        let a = make_fragment_ref_for_tests("src/a.rs", 'a');
        let b = make_fragment_ref_for_tests("src/b.rs", 'b');
        let coverage = coverage_with(vec![a.clone(), b.clone()]);
        let interactor = make_interactor(coverage, vec![]);
        let result = interactor.check_approved(&make_track(), &current_refs(vec![a, b])).unwrap();
        assert_eq!(result, DryCheckApprovalVerdict::Approved);
    }

    // ── same hash but different path is stale → Blocked ──────────────────────

    #[test]
    fn test_check_approved_same_hash_different_path_is_stale() {
        // Coverage records `src/a.rs` at hash 'a'. Current diff has `src/b.rs`
        // at the SAME hash 'a' — IN-06 / CN-08 require this to be treated as
        // stale (NOT covered).
        let recorded = make_fragment_ref_for_tests("src/a.rs", 'a');
        let current = make_fragment_ref_for_tests("src/b.rs", 'a');
        assert_eq!(recorded.content_hash(), current.content_hash());

        let coverage = coverage_with(vec![recorded]);
        let interactor = make_interactor(coverage, vec![]);
        let result =
            interactor.check_approved(&make_track(), &current_refs(vec![current])).unwrap();
        assert!(matches!(result, DryCheckApprovalVerdict::Blocked { .. }));
    }

    // ── latest verdict Violation (pair in coverage) → Blocked ────────────────

    #[test]
    fn test_check_approved_latest_violation_returns_blocked() {
        let a = make_fragment_ref_for_tests("src/a.rs", 'a');
        let b = make_fragment_ref_for_tests("src/b.rs", 'b');
        let pair_key = DryCheckPairKey::new(a.clone(), b.clone()).unwrap();
        // The pair must be in processed_pair_keys for the gate to treat the
        // Violation as active (it was re-judged in the latest dry write run).
        let coverage = coverage_with_pairs(vec![a.clone(), b.clone()], vec![pair_key]);
        let proposal = RefactorProposal::new("Extract helper.").unwrap();
        let record = make_dry_check_record_for_tests(
            a.clone(),
            b.clone(),
            DryCheckVerdict::Violation { refactor_proposal: proposal },
            DEFAULT_RECORDED_AT,
        );
        let interactor = make_interactor(coverage, vec![record]);
        let result = interactor.check_approved(&make_track(), &current_refs(vec![a, b])).unwrap();
        assert_eq!(result, DryCheckApprovalVerdict::Blocked { unresolved_pair_count: 1 });
    }

    // ── older Violation, later Accepted → Approved (last-write-wins) ────────

    #[test]
    fn test_check_approved_older_violation_then_accepted_returns_approved() {
        let a = make_fragment_ref_for_tests("src/a.rs", 'a');
        let b = make_fragment_ref_for_tests("src/b.rs", 'b');
        let coverage = coverage_with(vec![a.clone(), b.clone()]);
        let proposal = RefactorProposal::new("Extract helper.").unwrap();
        let violation = make_dry_check_record_for_tests(
            a.clone(),
            b.clone(),
            DryCheckVerdict::Violation { refactor_proposal: proposal },
            "2026-06-01T00:00:00Z",
        );
        let accepted = make_dry_check_record_for_tests(
            a.clone(),
            b.clone(),
            DryCheckVerdict::Accepted,
            "2026-06-02T00:00:00Z",
        );
        // Reader returns records in chronological order; latest-per-pair takes the last.
        let interactor = make_interactor(coverage, vec![violation, accepted]);
        let result = interactor.check_approved(&make_track(), &current_refs(vec![a, b])).unwrap();
        assert_eq!(result, DryCheckApprovalVerdict::Approved);
    }

    // ── latest NotAViolation → Approved ──────────────────────────────────────

    #[test]
    fn test_check_approved_latest_not_a_violation_returns_approved() {
        let a = make_fragment_ref_for_tests("src/a.rs", 'a');
        let b = make_fragment_ref_for_tests("src/b.rs", 'b');
        let coverage = coverage_with(vec![a.clone(), b.clone()]);
        let record = make_dry_check_record_for_tests(
            a.clone(),
            b.clone(),
            DryCheckVerdict::NotAViolation,
            DEFAULT_RECORDED_AT,
        );
        let interactor = make_interactor(coverage, vec![record]);
        let result = interactor.check_approved(&make_track(), &current_refs(vec![a, b])).unwrap();
        assert_eq!(result, DryCheckApprovalVerdict::Approved);
    }

    // ── processed pair without verdict (history missing / truncated) → Blocked ──

    #[test]
    fn test_check_approved_processed_pair_missing_verdict_returns_blocked() {
        // Scenario: coverage manifest survives (FragmentRefs covered, fingerprints
        // match) but `dry-check.json` is missing / truncated, so the pair_key in
        // `processed_pair_keys` has no corresponding record. The pair touches a
        // current FragmentRef, so the gate must Block rather than silently Approve.
        let a = make_fragment_ref_for_tests("src/a.rs", 'a');
        let b = make_fragment_ref_for_tests("src/b.rs", 'b');
        let pair_key = DryCheckPairKey::new(a.clone(), b.clone()).unwrap();
        let coverage = coverage_with_pairs(vec![a.clone(), b.clone()], vec![pair_key]);
        // No records — dry-check.json was lost / never written for this pair.
        let interactor = make_interactor(coverage, vec![]);
        let result = interactor.check_approved(&make_track(), &current_refs(vec![a, b])).unwrap();
        assert_eq!(result, DryCheckApprovalVerdict::Blocked { unresolved_pair_count: 1 });
    }

    // ── Violation between fragments not in current diff → Approved ───────────

    #[test]
    fn test_check_approved_violation_unrelated_to_current_diff_is_ignored() {
        // Current diff touches only `src/c.rs`. History has a Violation between
        // `src/a.rs` and `src/b.rs` — irrelevant to this run.
        let a = make_fragment_ref_for_tests("src/a.rs", 'a');
        let b = make_fragment_ref_for_tests("src/b.rs", 'b');
        let c = make_fragment_ref_for_tests("src/c.rs", 'c');
        let coverage = coverage_with(vec![c.clone()]);
        let proposal = RefactorProposal::new("Extract helper.").unwrap();
        let record = make_dry_check_record_for_tests(
            a,
            b,
            DryCheckVerdict::Violation { refactor_proposal: proposal },
            DEFAULT_RECORDED_AT,
        );
        let interactor = make_interactor(coverage, vec![record]);
        let result = interactor.check_approved(&make_track(), &current_refs(vec![c])).unwrap();
        assert_eq!(result, DryCheckApprovalVerdict::Approved);
    }

    // ── reader error propagates ──────────────────────────────────────────────

    #[test]
    fn test_check_approved_reader_error_propagated() {
        let a = make_fragment_ref_for_tests("src/a.rs", 'a');
        let coverage = coverage_with(vec![a.clone()]);
        let interactor = DryCheckApprovalInteractor::new(
            Arc::new(ErrorReader),
            Arc::new(coverage),
            test_fingerprint(),
            test_corpus_fingerprint(),
        );
        let result = interactor.check_approved(&make_track(), &current_refs(vec![a]));
        assert!(matches!(result, Err(DryCheckCycleError::Reader(_))));
    }

    // ── coverage error propagates ────────────────────────────────────────────

    #[test]
    fn test_check_approved_coverage_error_propagated() {
        let a = make_fragment_ref_for_tests("src/a.rs", 'a');
        let interactor = DryCheckApprovalInteractor::new(
            Arc::new(StubReader { records: vec![] }),
            Arc::new(ErrorCoverage),
            test_fingerprint(),
            test_corpus_fingerprint(),
        );
        let result = interactor.check_approved(&make_track(), &current_refs(vec![a]));
        assert!(matches!(result, Err(DryCheckCycleError::CoveragePort(_))));
    }

    // ── empty current refs + coverage Some(empty) → Approved ─────────────────

    #[test]
    fn test_check_approved_with_empty_current_and_empty_coverage_returns_approved() {
        // Empty diff over an empty coverage record: nothing to be stale, nothing
        // to verify → Approved. (Distinct from the "coverage absent" case.)
        let interactor = make_interactor(coverage_with(vec![]), vec![]);
        let result = interactor.check_approved(&make_track(), &current_refs(vec![])).unwrap();
        assert_eq!(result, DryCheckApprovalVerdict::Approved);
    }

    // ── config fingerprint mismatch → Blocked (round-5 fix) ─────────────────

    #[test]
    fn test_check_approved_with_different_config_fingerprint_returns_blocked() {
        // Scenario: coverage was written under config A (fingerprint_a), but the
        // current config is B (fingerprint_b, e.g., threshold was lowered).
        // All current FragmentRefs ARE covered by the manifest (staleness check
        // would pass), but the fingerprint mismatch must take priority and return
        // Blocked so that `dry write` re-runs with the new config.
        let a = make_fragment_ref_for_tests("src/a.rs", 'a');

        // Build coverage with a DIFFERENT fingerprint than the interactor's current one.
        let old_fingerprint = DryCheckConfigFingerprint::new("b".repeat(64)).unwrap();
        let coverage = StubCoverage {
            record: Some(DryCheckCoverageRecord::new(
                vec![a.clone()].into_iter().collect(),
                std::collections::BTreeSet::new(),
                old_fingerprint,
                test_corpus_fingerprint(),
            )),
        };

        // The interactor holds test_fingerprint() ("aaa..."), which differs from "bbb...".
        let interactor = make_interactor(coverage, vec![]);

        let result = interactor.check_approved(&make_track(), &current_refs(vec![a])).unwrap();
        assert!(
            matches!(result, DryCheckApprovalVerdict::Blocked { .. }),
            "config fingerprint mismatch must return Blocked, got: {result:?}"
        );
    }

    #[test]
    fn test_check_approved_with_matching_config_fingerprint_and_covered_refs_returns_approved() {
        // Scenario: coverage was written under the same config as the current one.
        // All current FragmentRefs are covered, no Violations → Approved.
        let a = make_fragment_ref_for_tests("src/a.rs", 'a');
        // coverage_with uses test_fingerprint() which matches the interactor's fingerprint.
        let coverage = coverage_with(vec![a.clone()]);
        let interactor = make_interactor(coverage, vec![]);
        let result = interactor.check_approved(&make_track(), &current_refs(vec![a])).unwrap();
        assert_eq!(
            result,
            DryCheckApprovalVerdict::Approved,
            "matching fingerprint + covered refs + no Violation must yield Approved"
        );
    }

    #[test]
    fn test_check_approved_processed_pair_with_stale_record_fingerprint_returns_blocked() {
        let a = make_fragment_ref_for_tests("src/a.rs", 'a');
        let b = make_fragment_ref_for_tests("src/b.rs", 'b');
        let pair_key = DryCheckPairKey::new(a.clone(), b.clone()).unwrap();

        let current_fingerprint = DryCheckConfigFingerprint::new("b".repeat(64)).unwrap();
        let coverage = coverage_with_pairs_and_fingerprint(
            vec![a.clone(), b.clone()],
            vec![pair_key],
            current_fingerprint.clone(),
        );
        // Shared helper records use test_fingerprint() ("aaa..."), so this
        // Accepted verdict is stale under current_fingerprint ("bbb...").
        let stale_record = make_dry_check_record_for_tests(
            a.clone(),
            b.clone(),
            DryCheckVerdict::Accepted,
            DEFAULT_RECORDED_AT,
        );
        let interactor = DryCheckApprovalInteractor::new(
            Arc::new(StubReader { records: vec![stale_record] }),
            Arc::new(coverage),
            current_fingerprint,
            test_corpus_fingerprint(),
        );

        let result = interactor.check_approved(&make_track(), &current_refs(vec![a, b])).unwrap();
        assert_eq!(result, DryCheckApprovalVerdict::Blocked { unresolved_pair_count: 1 });
    }

    #[test]
    fn test_check_approved_violation_and_missing_verdict_counts_distinct_pairs() {
        let a = make_fragment_ref_for_tests("src/a.rs", 'a');
        let b = make_fragment_ref_for_tests("src/b.rs", 'b');
        let c = make_fragment_ref_for_tests("src/c.rs", 'c');
        let d = make_fragment_ref_for_tests("src/d.rs", 'd');
        let violation_pair_key = DryCheckPairKey::new(a.clone(), b.clone()).unwrap();
        let missing_pair_key = DryCheckPairKey::new(c.clone(), d.clone()).unwrap();

        let current_fingerprint = DryCheckConfigFingerprint::new("b".repeat(64)).unwrap();
        let coverage = coverage_with_pairs_and_fingerprint(
            vec![a.clone(), b.clone(), c.clone(), d.clone()],
            vec![violation_pair_key, missing_pair_key],
            current_fingerprint.clone(),
        );
        let proposal = RefactorProposal::new("Extract helper.").unwrap();
        let violation_record = make_dry_check_record_for_tests(
            a.clone(),
            b.clone(),
            DryCheckVerdict::Violation { refactor_proposal: proposal },
            DEFAULT_RECORDED_AT,
        );
        let interactor = DryCheckApprovalInteractor::new(
            Arc::new(StubReader { records: vec![violation_record] }),
            Arc::new(coverage),
            current_fingerprint,
            test_corpus_fingerprint(),
        );

        let result =
            interactor.check_approved(&make_track(), &current_refs(vec![a, b, c, d])).unwrap();
        assert_eq!(result, DryCheckApprovalVerdict::Blocked { unresolved_pair_count: 2 });
    }

    #[test]
    fn test_check_approved_with_fail_closed_fingerprint_in_coverage_returns_blocked() {
        // Scenario: the last `dry write` wrote the fail-closed sentinel fingerprint
        // (all zeros) because calibration or a pair-level error occurred.
        // The current config has a valid (non-zero) fingerprint.
        // The gate must return Blocked even if all current FragmentRefs are covered.
        let a = make_fragment_ref_for_tests("src/a.rs", 'a');

        let fail_closed = DryCheckConfigFingerprint::fail_closed();
        let coverage = coverage_with_pairs_and_fingerprint(vec![a.clone()], vec![], fail_closed);

        // test_fingerprint() ("aaa...") != fail_closed ("000...") → Blocked.
        let interactor = make_interactor(coverage, vec![]);
        let result = interactor.check_approved(&make_track(), &current_refs(vec![a])).unwrap();
        assert!(
            matches!(result, DryCheckApprovalVerdict::Blocked { .. }),
            "fail-closed fingerprint in coverage must return Blocked, got: {result:?}"
        );
    }

    // ── corpus fingerprint mismatch → Blocked ────────────────────────────────

    #[test]
    fn test_check_approved_skips_when_corpus_fingerprint_drifted() {
        // Scenario: coverage was written when the corpus had fingerprint X.
        // The current corpus has fingerprint Y (a file was added, removed, or
        // modified in the workspace).  The diff fragment and config are unchanged,
        // so the fragment-ref staleness check and config-fingerprint check would
        // both pass — but the corpus fingerprint mismatch must cause Blocked.
        let a = make_fragment_ref_for_tests("src/a.rs", 'a');

        // Build coverage with a DIFFERENT corpus fingerprint than the interactor's.
        let old_corpus_fp = DryCheckCorpusFingerprint::new("d".repeat(64)).unwrap();
        let coverage = coverage_with_pairs_and_fingerprints(
            vec![a.clone()],
            vec![],
            test_fingerprint(),
            old_corpus_fp,
        );

        // The interactor holds test_corpus_fingerprint() ("ccc..."), which differs
        // from "ddd..." stored in the coverage record.
        let interactor = make_interactor(coverage, vec![]);
        let result = interactor.check_approved(&make_track(), &current_refs(vec![a])).unwrap();
        assert!(
            matches!(result, DryCheckApprovalVerdict::Blocked { .. }),
            "corpus fingerprint mismatch must return Blocked, got: {result:?}"
        );
    }

    #[test]
    fn test_check_approved_with_fail_closed_corpus_fingerprint_in_coverage_returns_blocked() {
        // Scenario: the last `dry write` wrote the fail-closed corpus sentinel
        // (all zeros) because an I/O error occurred during corpus fingerprint
        // computation. The current corpus has a valid (non-zero) fingerprint.
        // The gate must return Blocked even if all current FragmentRefs are covered
        // and the config fingerprint matches.
        let a = make_fragment_ref_for_tests("src/a.rs", 'a');

        let fail_closed_corpus = DryCheckCorpusFingerprint::fail_closed();
        let coverage = coverage_with_pairs_and_fingerprints(
            vec![a.clone()],
            vec![],
            test_fingerprint(),
            fail_closed_corpus,
        );

        // test_corpus_fingerprint() ("ccc...") != fail_closed ("000...") → Blocked.
        let interactor = make_interactor(coverage, vec![]);
        let result = interactor.check_approved(&make_track(), &current_refs(vec![a])).unwrap();
        assert!(
            matches!(result, DryCheckApprovalVerdict::Blocked { .. }),
            "fail-closed corpus fingerprint in coverage must return Blocked, got: {result:?}"
        );
    }

    #[test]
    fn test_check_approved_with_zero_corpus_fingerprint_on_both_sides_returns_blocked() {
        // Scenario: both the persisted manifest and the current read carry the
        // serialized fail-closed sentinel. Equality alone would treat them as a
        // match, but the all-zero fingerprint is never a valid approval basis.
        let a = make_fragment_ref_for_tests("src/a.rs", 'a');

        let zero_corpus = DryCheckCorpusFingerprint::new("0".repeat(64)).unwrap();
        let coverage = coverage_with_pairs_and_fingerprints(
            vec![a.clone()],
            vec![],
            test_fingerprint(),
            zero_corpus.clone(),
        );

        let interactor =
            make_interactor_with_corpus_fingerprint(coverage, vec![], zero_corpus.clone());
        let result = interactor.check_approved(&make_track(), &current_refs(vec![a])).unwrap();
        assert!(
            matches!(result, DryCheckApprovalVerdict::Blocked { .. }),
            "serialized zero corpus fingerprints must return Blocked, got: {result:?}"
        );
    }

    // ── stale Violation (pair NOT in processed_pair_keys) → Approved ─────────

    #[test]
    fn test_check_approved_skips_stale_violation_not_in_processed_pair_keys() {
        // Scenario: a Violation was recorded for (src/a.rs, src/b.rs) in a
        // previous run. The user then fixed src/b.rs (the candidate side) — the
        // diff-side src/a.rs is unchanged, so the current diff's FragmentRef for
        // src/a.rs IS still covered. However, the fixed src/b.rs produces a new
        // content_hash → new FragmentRef → new DryCheckPairKey, so the old
        // pair_key is NOT produced by the latest `dry write` run and is therefore
        // absent from the coverage manifest's `processed_pair_keys`.
        //
        // The gate must return Approved: the stale Violation record is no longer
        // relevant because the candidate side changed.
        let a = make_fragment_ref_for_tests("src/a.rs", 'a'); // diff-side, unchanged
        let b_old = make_fragment_ref_for_tests("src/b.rs", 'b'); // candidate, now fixed

        // Coverage covers src/a.rs (it's in the current diff) but the pair (a, b_old)
        // is NOT in processed_pair_keys — the fixed b produces a new pair_key.
        let coverage = coverage_with(vec![a.clone()]);

        let proposal = RefactorProposal::new("Extract helper.").unwrap();
        let stale_record = make_dry_check_record_for_tests(
            a.clone(),
            b_old,
            DryCheckVerdict::Violation { refactor_proposal: proposal },
            DEFAULT_RECORDED_AT,
        );

        let interactor = make_interactor(coverage, vec![stale_record]);
        // Current diff only touches src/a.rs (b was fixed and is no longer in diff).
        let result = interactor.check_approved(&make_track(), &current_refs(vec![a])).unwrap();
        // The stale Violation must NOT block the gate.
        assert_eq!(
            result,
            DryCheckApprovalVerdict::Approved,
            "stale Violation whose pair_key is absent from processed_pair_keys must not block"
        );
    }
}
