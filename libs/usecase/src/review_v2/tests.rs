use std::cell::RefCell;
use std::collections::HashMap;

use domain::review_v2::{
    FastVerdict, FilePath, Finding, LogInfo, MainScopeName, NotRequiredReason, RequiredReason,
    ReviewHash, ReviewOutcome, ReviewReader, ReviewReaderError, ReviewScopeConfig, ReviewState,
    ScopeName, Verdict,
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
        let finding = Finding::new("bug found", Some("P1".to_owned()), None, None, None).unwrap();
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
        vec![("domain".to_owned(), vec!["libs/domain/**".to_owned()])],
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
