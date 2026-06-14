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
    DryCheckApprovalVerdict, DryCheckPairKey, DryCheckReader, DryCheckRecord, DryCheckVerdict,
    FragmentRef,
};

use super::errors::DryCheckCycleError;
use super::ports::DryCheckCoveragePort;
use super::services::DryCheckApprovalService;

// ── DryCheckApprovalInteractor ────────────────────────────────────────────────

/// Interactor implementing the D5 read-only [`DryCheckApprovalService`].
///
/// Holds only a [`DryCheckReader`] (for verdict history) and a
/// [`DryCheckCoveragePort`] (for the coverage manifest). The implementation
/// performs hash matching and verdict scanning — no embedding / index ports.
///
/// The constructor return type is written as `DryCheckApprovalInteractor` (not
/// `Self`) so the type-signal evaluator's exact-string match succeeds.
pub struct DryCheckApprovalInteractor {
    reader: Arc<dyn DryCheckReader>,
    coverage: Arc<dyn DryCheckCoveragePort>,
}

impl DryCheckApprovalInteractor {
    /// Create a new [`DryCheckApprovalInteractor`].
    ///
    /// # Parameters
    ///
    /// - `reader`: port for reading the dry-check history (`DryCheckRecord`s).
    /// - `coverage`: port for reading the D5 coverage manifest.
    #[must_use]
    pub fn new(
        reader: Arc<dyn DryCheckReader>,
        coverage: Arc<dyn DryCheckCoveragePort>,
    ) -> DryCheckApprovalInteractor {
        DryCheckApprovalInteractor { reader, coverage }
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
        // FragmentRef. (`covers` filters using the same coverage-record set
        // we already proved is a superset of current_fragment_refs, but for
        // the verdict step we strictly require touching the *current* refs
        // — a Violation between two old fragments unrelated to the current
        // diff does not block this run.)
        let mut unresolved_violation_pairs = 0usize;
        for record in latest_per_pair.values() {
            let touches_current = current_fragment_refs.contains(record.pair_key().low())
                || current_fragment_refs.contains(record.pair_key().high());
            if !touches_current {
                continue;
            }
            if matches!(record.verdict(), DryCheckVerdict::Violation { .. }) {
                unresolved_violation_pairs += 1;
            }
        }
        if unresolved_violation_pairs > 0 {
            return Ok(DryCheckApprovalVerdict::Blocked {
                unresolved_pair_count: unresolved_violation_pairs,
            });
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
        DryCheckCoverageRecord, DryCheckReaderError, DryCheckRecord, DryCheckVerdict, FragmentRef,
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

    /// Default timestamp used by tests that do not care about the exact
    /// `recorded_at` value. Tests that DO care pass an explicit timestamp to
    /// [`make_dry_check_record_for_tests`] directly.
    const DEFAULT_RECORDED_AT: &str = "2026-06-13T00:00:00Z";

    fn make_interactor(
        coverage: StubCoverage,
        records: Vec<DryCheckRecord>,
    ) -> DryCheckApprovalInteractor {
        DryCheckApprovalInteractor::new(Arc::new(StubReader { records }), Arc::new(coverage))
    }

    fn coverage_with(refs: Vec<FragmentRef>) -> StubCoverage {
        StubCoverage { record: Some(DryCheckCoverageRecord::new(refs.into_iter().collect())) }
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

    // ── latest verdict Violation → Blocked ───────────────────────────────────

    #[test]
    fn test_check_approved_latest_violation_returns_blocked() {
        let a = make_fragment_ref_for_tests("src/a.rs", 'a');
        let b = make_fragment_ref_for_tests("src/b.rs", 'b');
        let coverage = coverage_with(vec![a.clone(), b.clone()]);
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
        let interactor = DryCheckApprovalInteractor::new(Arc::new(ErrorReader), Arc::new(coverage));
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
}
