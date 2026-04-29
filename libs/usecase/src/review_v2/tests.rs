use std::cell::RefCell;
use std::collections::HashMap;

use domain::review_v2::{
    FastVerdict, FilePath, LogInfo, MainScopeName, NotRequiredReason, RequiredReason,
    ReviewApprovalVerdict, ReviewHash, ReviewOutcome, ReviewReader, ReviewReaderError,
    ReviewScopeConfig, ReviewState, ReviewerFinding, ScopeName, Verdict,
};
use domain::{CommitHash, TrackId};

use super::cycle::ReviewCycle;
use super::error::{DiffGetError, ReviewCycleError, ReviewHasherError, ReviewerError};
use super::ports::{DiffGetter, ReviewHasher, Reviewer};

// ── Mock implementations ──────────────────────────────────────────────

struct MockReviewer {
    verdict: RefCell<Option<Verdict>>,
    fast_verdict: RefCell<Option<FastVerdict>>,
}

impl MockReviewer {
    fn zero_findings() -> Self {
        Self {
            verdict: RefCell::new(Some(Verdict::ZeroFindings)),
            fast_verdict: RefCell::new(Some(FastVerdict::ZeroFindings)),
        }
    }

    fn with_findings() -> Self {
        let finding =
            ReviewerFinding::new("bug found", Some("P1".to_owned()), None, None, None).unwrap();
        Self {
            verdict: RefCell::new(Some(Verdict::findings_remain(vec![finding.clone()]).unwrap())),
            fast_verdict: RefCell::new(Some(FastVerdict::findings_remain(vec![finding]).unwrap())),
        }
    }
}

impl Reviewer for MockReviewer {
    fn review(
        &self,
        _target: &domain::review_v2::ReviewTarget,
    ) -> Result<(Verdict, LogInfo), ReviewerError> {
        let v = self.verdict.borrow().clone().ok_or(ReviewerError::ReviewerAbort)?;
        Ok((v, LogInfo::new("mock review log")))
    }

    fn fast_review(
        &self,
        _target: &domain::review_v2::ReviewTarget,
    ) -> Result<(FastVerdict, LogInfo), ReviewerError> {
        let v = self.fast_verdict.borrow().clone().ok_or(ReviewerError::ReviewerAbort)?;
        Ok((v, LogInfo::new("mock fast review log")))
    }
}

struct MockDiffGetter {
    files: Vec<FilePath>,
}

impl MockDiffGetter {
    fn new(paths: &[&str]) -> Self {
        Self { files: paths.iter().map(|p| FilePath::new(*p).unwrap()).collect() }
    }

    fn empty() -> Self {
        Self { files: vec![] }
    }
}

impl DiffGetter for MockDiffGetter {
    fn list_diff_files(&self, _base: &CommitHash) -> Result<Vec<FilePath>, DiffGetError> {
        Ok(self.files.clone())
    }
}

struct FailingDiffGetter;

impl DiffGetter for FailingDiffGetter {
    fn list_diff_files(&self, _base: &CommitHash) -> Result<Vec<FilePath>, DiffGetError> {
        Err(DiffGetError::Failed("simulated diff failure".to_owned()))
    }
}

struct FailingHasher;

impl ReviewHasher for FailingHasher {
    fn calc(
        &self,
        _target: &domain::review_v2::ReviewTarget,
    ) -> Result<ReviewHash, ReviewHasherError> {
        Err(ReviewHasherError::Failed("simulated hash failure".to_owned()))
    }
}

struct FailingReviewReader;

impl ReviewReader for FailingReviewReader {
    fn read_latest_finals(
        &self,
    ) -> Result<HashMap<ScopeName, (Verdict, ReviewHash)>, ReviewReaderError> {
        Err(ReviewReaderError::Io {
            path: "review.json".to_owned(),
            detail: "simulated I/O failure".to_owned(),
        })
    }
}

/// Mock hasher that returns a deterministic hash based on file count.
struct MockHasher;

impl ReviewHasher for MockHasher {
    fn calc(
        &self,
        target: &domain::review_v2::ReviewTarget,
    ) -> Result<ReviewHash, ReviewHasherError> {
        if target.is_empty() {
            return Ok(ReviewHash::Empty);
        }
        let hash_str = format!("rvw1:sha256:{:064x}", target.files().len());
        ReviewHash::computed(hash_str).map_err(|e| ReviewHasherError::Failed(e.to_string()))
    }
}

/// Mock hasher that changes hash on second call (simulates file change during review).
struct MutatingHasher {
    call_count: RefCell<u32>,
}

impl MutatingHasher {
    fn new() -> Self {
        Self { call_count: RefCell::new(0) }
    }
}

impl ReviewHasher for MutatingHasher {
    fn calc(
        &self,
        target: &domain::review_v2::ReviewTarget,
    ) -> Result<ReviewHash, ReviewHasherError> {
        if target.is_empty() {
            return Ok(ReviewHash::Empty);
        }
        let mut count = self.call_count.borrow_mut();
        *count += 1;
        let hash_str = format!("rvw1:sha256:{:064x}", *count);
        ReviewHash::computed(hash_str).map_err(|e| ReviewHasherError::Failed(e.to_string()))
    }
}

struct MockReviewReader {
    finals: HashMap<ScopeName, (Verdict, ReviewHash)>,
}

impl MockReviewReader {
    fn empty() -> Self {
        Self { finals: HashMap::new() }
    }

    fn with_approved(scope: ScopeName, hash: ReviewHash) -> Self {
        let mut finals = HashMap::new();
        finals.insert(scope, (Verdict::ZeroFindings, hash));
        Self { finals }
    }

    fn with_finals(entries: Vec<(ScopeName, Verdict, ReviewHash)>) -> Self {
        let mut finals = HashMap::new();
        for (scope, verdict, hash) in entries {
            finals.insert(scope, (verdict, hash));
        }
        Self { finals }
    }
}

impl ReviewReader for MockReviewReader {
    fn read_latest_finals(
        &self,
    ) -> Result<HashMap<ScopeName, (Verdict, ReviewHash)>, ReviewReaderError> {
        Ok(self.finals.clone())
    }
}

// ── Helpers ───────────────────────────────────────────────────────────

fn track_id() -> TrackId {
    TrackId::try_new("test-track-2026-04-05").unwrap()
}

fn base_commit() -> CommitHash {
    CommitHash::try_new("abcdef1234567").unwrap()
}

fn basic_config() -> ReviewScopeConfig {
    ReviewScopeConfig::new(
        &track_id(),
        vec![("domain".to_owned(), vec!["libs/domain/**".to_owned()], None)],
        vec![],
        vec![],
    )
    .unwrap()
}

fn domain_scope() -> ScopeName {
    ScopeName::Main(MainScopeName::new("domain").unwrap())
}

// ── review() tests ────────────────────────────────────────────────────

#[test]
fn test_review_zero_findings_returns_reviewed() {
    let cycle = ReviewCycle::new(
        base_commit(),
        basic_config(),
        MockReviewer::zero_findings(),
        MockDiffGetter::new(&["libs/domain/src/lib.rs"]),
        MockHasher,
    );

    let result = cycle.review(&domain_scope()).unwrap();
    assert!(matches!(result, ReviewOutcome::Reviewed { verdict: Verdict::ZeroFindings, .. }));
}

#[test]
fn test_review_findings_remain_returns_reviewed() {
    let cycle = ReviewCycle::new(
        base_commit(),
        basic_config(),
        MockReviewer::with_findings(),
        MockDiffGetter::new(&["libs/domain/src/lib.rs"]),
        MockHasher,
    );

    let result = cycle.review(&domain_scope()).unwrap();
    assert!(matches!(result, ReviewOutcome::Reviewed { verdict: Verdict::FindingsRemain(_), .. }));
}

#[test]
fn test_review_empty_scope_returns_skipped() {
    let cycle = ReviewCycle::new(
        base_commit(),
        basic_config(),
        MockReviewer::zero_findings(),
        MockDiffGetter::empty(),
        MockHasher,
    );

    let result = cycle.review(&domain_scope()).unwrap();
    assert!(matches!(result, ReviewOutcome::Skipped));
}

#[test]
fn test_review_unknown_scope_returns_error() {
    let cycle = ReviewCycle::new(
        base_commit(),
        basic_config(),
        MockReviewer::zero_findings(),
        MockDiffGetter::new(&["libs/domain/src/lib.rs"]),
        MockHasher,
    );

    let unknown = ScopeName::Main(MainScopeName::new("unknown").unwrap());
    let result = cycle.review(&unknown);
    assert!(matches!(result, Err(ReviewCycleError::UnknownScope(_))));
}

#[test]
fn test_review_file_changed_during_review_returns_error() {
    let cycle = ReviewCycle::new(
        base_commit(),
        basic_config(),
        MockReviewer::zero_findings(),
        MockDiffGetter::new(&["libs/domain/src/lib.rs"]),
        MutatingHasher::new(),
    );

    let result = cycle.review(&domain_scope());
    assert!(matches!(result, Err(ReviewCycleError::FileChangedDuringReview)));
}

// ── fast_review() tests ───────────────────────────────────────────────

#[test]
fn test_fast_review_zero_findings() {
    let cycle = ReviewCycle::new(
        base_commit(),
        basic_config(),
        MockReviewer::zero_findings(),
        MockDiffGetter::new(&["libs/domain/src/lib.rs"]),
        MockHasher,
    );

    let result = cycle.fast_review(&domain_scope()).unwrap();
    assert!(matches!(result, ReviewOutcome::Reviewed { verdict: FastVerdict::ZeroFindings, .. }));
}

#[test]
fn test_fast_review_skipped_on_empty() {
    let cycle = ReviewCycle::new(
        base_commit(),
        basic_config(),
        MockReviewer::zero_findings(),
        MockDiffGetter::empty(),
        MockHasher,
    );

    let result = cycle.fast_review(&domain_scope()).unwrap();
    assert!(matches!(result, ReviewOutcome::Skipped));
}

// ── get_review_targets() tests ────────────────────────────────────────

#[test]
fn test_get_review_targets_classifies_files() {
    let cycle = ReviewCycle::new(
        base_commit(),
        basic_config(),
        MockReviewer::zero_findings(),
        MockDiffGetter::new(&["libs/domain/src/lib.rs", "Cargo.toml"]),
        MockHasher,
    );

    let targets = cycle.get_review_targets().unwrap();
    assert!(targets.contains_key(&domain_scope()));
    assert!(targets.contains_key(&ScopeName::Other));
}

// ── get_review_states() tests ─────────────────────────────────────────

#[test]
fn test_get_review_states_not_started() {
    let cycle = ReviewCycle::new(
        base_commit(),
        basic_config(),
        MockReviewer::zero_findings(),
        MockDiffGetter::new(&["libs/domain/src/lib.rs"]),
        MockHasher,
    );

    let states = cycle.get_review_states(&MockReviewReader::empty()).unwrap();
    assert_eq!(
        states.get(&domain_scope()),
        Some(&ReviewState::Required(RequiredReason::NotStarted))
    );
}

#[test]
fn test_get_review_states_approved() {
    let hash = ReviewHash::computed(format!("rvw1:sha256:{:064x}", 1)).unwrap();
    let cycle = ReviewCycle::new(
        base_commit(),
        basic_config(),
        MockReviewer::zero_findings(),
        MockDiffGetter::new(&["libs/domain/src/lib.rs"]),
        MockHasher,
    );

    let reader = MockReviewReader::with_approved(domain_scope(), hash);
    let states = cycle.get_review_states(&reader).unwrap();
    assert_eq!(
        states.get(&domain_scope()),
        Some(&ReviewState::NotRequired(NotRequiredReason::ZeroFindings))
    );
}

#[test]
fn test_get_review_states_stale_hash() {
    let stale_hash = ReviewHash::computed("rvw1:sha256:deadbeef").unwrap();
    let cycle = ReviewCycle::new(
        base_commit(),
        basic_config(),
        MockReviewer::zero_findings(),
        MockDiffGetter::new(&["libs/domain/src/lib.rs"]),
        MockHasher,
    );

    let reader = MockReviewReader::with_approved(domain_scope(), stale_hash);
    let states = cycle.get_review_states(&reader).unwrap();
    assert_eq!(
        states.get(&domain_scope()),
        Some(&ReviewState::Required(RequiredReason::StaleHash))
    );
}

#[test]
fn test_get_review_states_empty_scope_not_required() {
    let cycle = ReviewCycle::new(
        base_commit(),
        basic_config(),
        MockReviewer::zero_findings(),
        MockDiffGetter::empty(),
        MockHasher,
    );

    let states = cycle.get_review_states(&MockReviewReader::empty()).unwrap();
    // domain scope is configured but has no files → Empty
    assert_eq!(
        states.get(&domain_scope()),
        Some(&ReviewState::NotRequired(NotRequiredReason::Empty))
    );
}

#[test]
fn test_get_review_states_all_not_required_means_approved() {
    let hash = ReviewHash::computed(format!("rvw1:sha256:{:064x}", 1)).unwrap();
    let cycle = ReviewCycle::new(
        base_commit(),
        basic_config(),
        MockReviewer::zero_findings(),
        MockDiffGetter::new(&["libs/domain/src/lib.rs"]),
        MockHasher,
    );

    let reader = MockReviewReader::with_approved(domain_scope(), hash);
    let states = cycle.get_review_states(&reader).unwrap();
    let approved = states.values().all(|s| s.is_approved());
    assert!(approved);
}

// ── Other scope tests ─────────────────────────────────────────────────

#[test]
fn test_review_other_scope() {
    let cycle = ReviewCycle::new(
        base_commit(),
        basic_config(),
        MockReviewer::zero_findings(),
        MockDiffGetter::new(&["Cargo.toml"]),
        MockHasher,
    );

    let result = cycle.review(&ScopeName::Other).unwrap();
    assert!(matches!(result, ReviewOutcome::Reviewed { verdict: Verdict::ZeroFindings, .. }));
}

// ── evaluate_approval() tests ─────────────────────────────────────────

fn two_scope_config() -> ReviewScopeConfig {
    ReviewScopeConfig::new(
        &track_id(),
        vec![
            ("domain".to_owned(), vec!["libs/domain/**".to_owned()], None),
            ("usecase".to_owned(), vec!["libs/usecase/**".to_owned()], None),
        ],
        vec![],
        vec![],
    )
    .unwrap()
}

/// Case 1: all scopes NotRequired(*) → Approved (regardless of review_json_exists).
#[test]
fn test_evaluate_approval_all_not_required_returns_approved() {
    // Both scopes have matching ZeroFindings hashes → NotRequired(ZeroFindings)
    let domain_hash = ReviewHash::computed(format!("rvw1:sha256:{:064x}", 1)).unwrap();
    let cycle = ReviewCycle::new(
        base_commit(),
        basic_config(),
        MockReviewer::zero_findings(),
        MockDiffGetter::new(&["libs/domain/src/lib.rs"]),
        MockHasher,
    );
    let reader = MockReviewReader::with_approved(domain_scope(), domain_hash);

    let verdict = cycle.evaluate_approval(&reader, false).unwrap();
    assert_eq!(verdict, ReviewApprovalVerdict::Approved);

    let verdict_with_file = cycle.evaluate_approval(&reader, true).unwrap();
    assert_eq!(verdict_with_file, ReviewApprovalVerdict::Approved);
}

/// Case 2: all Required scopes are NotStarted + review_json_exists == false → ApprovedWithBypass.
#[test]
fn test_evaluate_approval_all_not_started_no_file_returns_bypass() {
    // Empty reader → all Required(NotStarted); review.json absent
    let cycle = ReviewCycle::new(
        base_commit(),
        basic_config(),
        MockReviewer::zero_findings(),
        MockDiffGetter::new(&["libs/domain/src/lib.rs"]),
        MockHasher,
    );
    let reader = MockReviewReader::empty();

    let verdict = cycle.evaluate_approval(&reader, false).unwrap();
    assert_eq!(verdict, ReviewApprovalVerdict::ApprovedWithBypass { not_started_count: 1 });
}

/// Case 3: all Required scopes are NotStarted + review_json_exists == true → Blocked.
#[test]
fn test_evaluate_approval_all_not_started_with_file_returns_blocked() {
    let cycle = ReviewCycle::new(
        base_commit(),
        basic_config(),
        MockReviewer::zero_findings(),
        MockDiffGetter::new(&["libs/domain/src/lib.rs"]),
        MockHasher,
    );
    let reader = MockReviewReader::empty();

    let verdict = cycle.evaluate_approval(&reader, true).unwrap();
    assert!(
        matches!(verdict, ReviewApprovalVerdict::Blocked { required_scopes } if required_scopes.len() == 1)
    );
}

/// Case 4: some Required scopes have FindingsRemain → Blocked regardless of bypass.
#[test]
fn test_evaluate_approval_findings_remain_returns_blocked() {
    let finding =
        ReviewerFinding::new("critical bug", Some("P1".to_owned()), None, None, None).unwrap();
    let findings_verdict = Verdict::findings_remain(vec![finding]).unwrap();
    // Stale hash so the stored hash does not match the current computed hash → FindingsRemain
    let stale_hash = ReviewHash::computed("rvw1:sha256:deadbeef").unwrap();

    let cycle = ReviewCycle::new(
        base_commit(),
        two_scope_config(),
        MockReviewer::zero_findings(),
        MockDiffGetter::new(&["libs/domain/src/lib.rs"]),
        MockHasher,
    );
    // domain → FindingsRemain (stored verdict is findings_remain)
    let reader =
        MockReviewReader::with_finals(vec![(domain_scope(), findings_verdict, stale_hash)]);

    // review_json_exists = false should still be Blocked (bypass only applies to all-NotStarted)
    let verdict = cycle.evaluate_approval(&reader, false).unwrap();
    assert!(matches!(verdict, ReviewApprovalVerdict::Blocked { .. }));
}

/// Case 5: mixed Required(StaleHash) + Required(NotStarted) → Blocked (not all NotStarted).
#[test]
fn test_evaluate_approval_mixed_stale_and_not_started_returns_blocked() {
    let stale_hash = ReviewHash::computed("rvw1:sha256:deadbeef").unwrap();

    let cycle = ReviewCycle::new(
        base_commit(),
        two_scope_config(),
        MockReviewer::zero_findings(),
        // Both scopes have files: domain (1 file) and usecase (1 file)
        MockDiffGetter::new(&["libs/domain/src/lib.rs", "libs/usecase/src/lib.rs"]),
        MockHasher,
    );
    // domain → StaleHash (stored ZeroFindings but hash mismatch)
    // usecase → NotStarted (no entry in reader)
    let reader =
        MockReviewReader::with_finals(vec![(domain_scope(), Verdict::ZeroFindings, stale_hash)]);

    // review_json_exists = false: bypass does NOT apply because not all are NotStarted
    let verdict = cycle.evaluate_approval(&reader, false).unwrap();
    assert!(matches!(verdict, ReviewApprovalVerdict::Blocked { required_scopes }
        if required_scopes.len() == 2));
}

/// Case 6: diff getter failure propagates as ReviewCycleError from evaluate_approval.
#[test]
fn test_evaluate_approval_diff_error_propagates() {
    let cycle = ReviewCycle::new(
        base_commit(),
        basic_config(),
        MockReviewer::zero_findings(),
        FailingDiffGetter,
        MockHasher,
    );
    let reader = MockReviewReader::empty();

    let result = cycle.evaluate_approval(&reader, false);
    assert!(matches!(result, Err(ReviewCycleError::Diff(_))));
}

/// Case 8: hasher failure propagates as ReviewCycleError::Hash from evaluate_approval.
#[test]
fn test_evaluate_approval_hash_error_propagates() {
    let cycle = ReviewCycle::new(
        base_commit(),
        basic_config(),
        MockReviewer::zero_findings(),
        MockDiffGetter::new(&["libs/domain/src/lib.rs"]),
        FailingHasher,
    );
    let reader = MockReviewReader::empty();

    let result = cycle.evaluate_approval(&reader, false);
    assert!(matches!(result, Err(ReviewCycleError::Hash(_))));
}

/// Case 9: reader failure propagates as ReviewCycleError::Reader from evaluate_approval.
#[test]
fn test_evaluate_approval_reader_error_propagates() {
    let cycle = ReviewCycle::new(
        base_commit(),
        basic_config(),
        MockReviewer::zero_findings(),
        MockDiffGetter::new(&["libs/domain/src/lib.rs"]),
        MockHasher,
    );

    let result = cycle.evaluate_approval(&FailingReviewReader, false);
    assert!(matches!(result, Err(ReviewCycleError::Reader(_))));
}

/// Case 7: Blocked result contains the expected scope names.
#[test]
fn test_evaluate_approval_blocked_contains_expected_scopes() {
    let usecase_scope = ScopeName::Main(MainScopeName::new("usecase").unwrap());
    let cycle = ReviewCycle::new(
        base_commit(),
        two_scope_config(),
        MockReviewer::zero_findings(),
        // Both scopes have files
        MockDiffGetter::new(&["libs/domain/src/lib.rs", "libs/usecase/src/lib.rs"]),
        MockHasher,
    );
    // No stored finals → both are Required(NotStarted), but review.json exists → Blocked
    let reader = MockReviewReader::empty();

    let verdict = cycle.evaluate_approval(&reader, true).unwrap();
    match verdict {
        ReviewApprovalVerdict::Blocked { required_scopes } => {
            assert!(required_scopes.contains(&domain_scope()), "domain should be blocked");
            assert!(required_scopes.contains(&usecase_scope), "usecase should be blocked");
            assert_eq!(required_scopes.len(), 2);
        }
        other => panic!("expected Blocked, got {other:?}"),
    }
}
