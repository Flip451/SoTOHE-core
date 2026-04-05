use rstest::rstest;

use super::error::{FilePathError, FindingError, ReviewHashError, ScopeNameError, VerdictError};
use super::types::*;

// ── helpers ───────────────────────────────────────────────────────────

fn finding(msg: &str) -> Finding {
    Finding::new(msg, None, None, None, None).unwrap()
}

fn finding_full() -> Finding {
    Finding::new(
        "null pointer dereference",
        Some("P1".to_owned()),
        Some("src/lib.rs".to_owned()),
        Some(42),
        Some("correctness".to_owned()),
    )
    .unwrap()
}

fn valid_hash() -> ReviewHash {
    ReviewHash::computed("rvw1:sha256:abc123def456").unwrap()
}

// ── MainScopeName ─────────────────────────────────────────────────────

#[rstest]
#[case::valid_domain("domain")]
#[case::valid_infra("infrastructure")]
#[case::valid_hyphenated("harness-policy")]
fn test_main_scope_name_with_valid_input_succeeds(#[case] input: &str) {
    let result = MainScopeName::new(input);
    assert!(result.is_ok());
    assert_eq!(result.unwrap().as_str(), input);
}

#[test]
fn test_main_scope_name_with_empty_returns_error() {
    assert!(matches!(MainScopeName::new(""), Err(ScopeNameError::Empty)));
}

#[test]
fn test_main_scope_name_with_non_ascii_returns_error() {
    assert!(matches!(MainScopeName::new("ドメイン"), Err(ScopeNameError::NotAscii)));
}

#[test]
fn test_main_scope_name_with_reserved_other_returns_error() {
    assert!(matches!(MainScopeName::new("other"), Err(ScopeNameError::Reserved)));
}

// ── ScopeName ─────────────────────────────────────────────────────────

#[test]
fn test_scope_name_main_displays_inner_name() {
    let scope = ScopeName::Main(MainScopeName::new("domain").unwrap());
    assert_eq!(scope.to_string(), "domain");
}

#[test]
fn test_scope_name_other_displays_other() {
    assert_eq!(ScopeName::Other.to_string(), "other");
}

#[test]
fn test_scope_name_equality() {
    let a = ScopeName::Main(MainScopeName::new("domain").unwrap());
    let b = ScopeName::Main(MainScopeName::new("domain").unwrap());
    assert_eq!(a, b);
    assert_ne!(a, ScopeName::Other);
}

// ── FilePath ──────────────────────────────────────────────────────────

#[test]
fn test_file_path_with_valid_path_succeeds() {
    let fp = FilePath::new("libs/domain/src/lib.rs").unwrap();
    assert_eq!(fp.as_str(), "libs/domain/src/lib.rs");
    assert_eq!(fp.to_string(), "libs/domain/src/lib.rs");
}

#[test]
fn test_file_path_with_empty_returns_error() {
    assert!(matches!(FilePath::new(""), Err(FilePathError::Empty)));
}

#[test]
fn test_file_path_with_absolute_path_returns_error() {
    assert!(matches!(FilePath::new("/etc/passwd"), Err(FilePathError::Absolute(_))));
}

#[test]
fn test_file_path_ordering() {
    let a = FilePath::new("a.rs").unwrap();
    let b = FilePath::new("b.rs").unwrap();
    assert!(a < b);
}

// ── ReviewTarget ──────────────────────────────────────────────────────

#[test]
fn test_review_target_empty() {
    let target = ReviewTarget::new(vec![]);
    assert!(target.is_empty());
    assert!(target.files().is_empty());
}

#[test]
fn test_review_target_with_files() {
    let target =
        ReviewTarget::new(vec![FilePath::new("a.rs").unwrap(), FilePath::new("b.rs").unwrap()]);
    assert!(!target.is_empty());
    assert_eq!(target.files().len(), 2);
}

// ── ReviewHashValue / ReviewHash ──────────────────────────────────────

#[test]
fn test_review_hash_value_with_valid_format_succeeds() {
    let v = ReviewHashValue::new("rvw1:sha256:abc123def456").unwrap();
    assert_eq!(v.as_str(), "rvw1:sha256:abc123def456");
}

#[rstest]
#[case::empty("")]
#[case::no_prefix("sha256:abc123")]
#[case::wrong_prefix("rvw2:sha256:abc123")]
#[case::empty_hex("rvw1:sha256:")]
#[case::non_hex_chars("rvw1:sha256:xyz")]
#[case::uppercase_hex("rvw1:sha256:ABC123")]
fn test_review_hash_value_with_invalid_format_returns_error(#[case] input: &str) {
    assert!(matches!(ReviewHashValue::new(input), Err(ReviewHashError::InvalidFormat(_))));
}

#[test]
fn test_review_hash_empty_is_empty() {
    assert!(ReviewHash::Empty.is_empty());
}

#[test]
fn test_review_hash_computed_is_not_empty() {
    assert!(!valid_hash().is_empty());
}

#[test]
fn test_review_hash_computed_as_str() {
    let hash = valid_hash();
    assert_eq!(hash.as_str(), Some("rvw1:sha256:abc123def456"));
}

#[test]
fn test_review_hash_empty_as_str() {
    assert_eq!(ReviewHash::Empty.as_str(), None);
}

#[test]
fn test_review_hash_equality() {
    let a = ReviewHash::computed("rvw1:sha256:abc").unwrap();
    let b = ReviewHash::computed("rvw1:sha256:abc").unwrap();
    let c = ReviewHash::computed("rvw1:sha256:def").unwrap();
    assert_eq!(a, b);
    assert_ne!(a, c);
    assert_ne!(a, ReviewHash::Empty);
}

#[test]
fn test_review_hash_computed_rejects_invalid_format() {
    assert!(ReviewHash::computed("not-a-hash").is_err());
}

// ── Finding ───────────────────────────────────────────────────────────

#[test]
fn test_finding_with_valid_message_succeeds() {
    let f = finding("something is wrong");
    assert_eq!(f.message(), "something is wrong");
    assert!(f.severity().is_none());
    assert!(f.file().is_none());
    assert!(f.line().is_none());
    assert!(f.category().is_none());
}

#[test]
fn test_finding_with_all_fields() {
    let f = finding_full();
    assert_eq!(f.message(), "null pointer dereference");
    assert_eq!(f.severity(), Some("P1"));
    assert_eq!(f.file(), Some("src/lib.rs"));
    assert_eq!(f.line(), Some(42));
    assert_eq!(f.category(), Some("correctness"));
}

#[test]
fn test_finding_with_empty_message_returns_error() {
    assert!(matches!(Finding::new("", None, None, None, None), Err(FindingError::EmptyMessage)));
}

#[test]
fn test_finding_with_whitespace_only_message_returns_error() {
    assert!(matches!(
        Finding::new("   \t\n", None, None, None, None),
        Err(FindingError::EmptyMessage)
    ));
}

// ── NonEmptyFindings ──────────────────────────────────────────────────

#[test]
fn test_non_empty_findings_with_findings_succeeds() {
    let nef = NonEmptyFindings::new(vec![finding("bug")]).unwrap();
    assert_eq!(nef.as_slice().len(), 1);
}

#[test]
fn test_non_empty_findings_with_empty_vec_returns_error() {
    assert!(matches!(NonEmptyFindings::new(vec![]), Err(VerdictError::EmptyFindings)));
}

#[test]
fn test_non_empty_findings_into_vec() {
    let nef = NonEmptyFindings::new(vec![finding("a"), finding("b")]).unwrap();
    let v = nef.into_vec();
    assert_eq!(v.len(), 2);
}

// ── Verdict ───────────────────────────────────────────────────────────

#[test]
fn test_verdict_zero_findings() {
    let v = Verdict::ZeroFindings;
    assert!(matches!(v, Verdict::ZeroFindings));
}

#[test]
fn test_verdict_findings_remain_with_findings_succeeds() {
    let v = Verdict::findings_remain(vec![finding("bug")]).unwrap();
    assert!(matches!(v, Verdict::FindingsRemain(ref nef) if nef.as_slice().len() == 1));
}

#[test]
fn test_verdict_findings_remain_with_empty_vec_returns_error() {
    assert!(matches!(Verdict::findings_remain(vec![]), Err(VerdictError::EmptyFindings)));
}

#[test]
fn test_verdict_findings_remain_with_multiple_findings() {
    let v = Verdict::findings_remain(vec![finding("bug1"), finding("bug2")]).unwrap();
    if let Verdict::FindingsRemain(nef) = v {
        assert_eq!(nef.as_slice().len(), 2);
    } else {
        panic!("expected FindingsRemain");
    }
}

// ── FastVerdict ───────────────────────────────────────────────────────

#[test]
fn test_fast_verdict_zero_findings() {
    let v = FastVerdict::ZeroFindings;
    assert!(matches!(v, FastVerdict::ZeroFindings));
}

#[test]
fn test_fast_verdict_findings_remain_succeeds() {
    let v = FastVerdict::findings_remain(vec![finding("issue")]).unwrap();
    assert!(matches!(v, FastVerdict::FindingsRemain(ref nef) if nef.as_slice().len() == 1));
}

#[test]
fn test_fast_verdict_findings_remain_with_empty_vec_returns_error() {
    assert!(matches!(FastVerdict::findings_remain(vec![]), Err(VerdictError::EmptyFindings)));
}

// ── LogInfo ───────────────────────────────────────────────────────────

#[test]
fn test_log_info_preserves_value() {
    let info = LogInfo::new("reviewer output log");
    assert_eq!(info.as_str(), "reviewer output log");
}

// ── ReviewOutcome ─────────────────────────────────────────────────────

#[test]
fn test_review_outcome_reviewed() {
    let outcome: ReviewOutcome<Verdict> = ReviewOutcome::Reviewed {
        verdict: Verdict::ZeroFindings,
        log_info: LogInfo::new("ok"),
        hash: valid_hash(),
    };
    assert!(matches!(outcome, ReviewOutcome::Reviewed { .. }));
}

#[test]
fn test_review_outcome_skipped() {
    let outcome: ReviewOutcome<Verdict> = ReviewOutcome::Skipped;
    assert!(matches!(outcome, ReviewOutcome::Skipped));
}

#[test]
fn test_review_outcome_fast_verdict() {
    let outcome: ReviewOutcome<FastVerdict> = ReviewOutcome::Reviewed {
        verdict: FastVerdict::ZeroFindings,
        log_info: LogInfo::new("fast pass"),
        hash: valid_hash(),
    };
    assert!(matches!(outcome, ReviewOutcome::Reviewed { .. }));
}

// ── ReviewState ───────────────────────────────────────────────────────

#[rstest]
#[case::not_started(ReviewState::Required(RequiredReason::NotStarted), false)]
#[case::findings_remain(ReviewState::Required(RequiredReason::FindingsRemain), false)]
#[case::stale_hash(ReviewState::Required(RequiredReason::StaleHash), false)]
#[case::empty(ReviewState::NotRequired(NotRequiredReason::Empty), true)]
#[case::zero_findings(ReviewState::NotRequired(NotRequiredReason::ZeroFindings), true)]
fn test_review_state_is_approved(#[case] state: ReviewState, #[case] expected: bool) {
    assert_eq!(state.is_approved(), expected);
}

#[rstest]
#[case::not_started(ReviewState::Required(RequiredReason::NotStarted), "required (not started)")]
#[case::findings_remain(
    ReviewState::Required(RequiredReason::FindingsRemain),
    "required (findings remain)"
)]
#[case::stale_hash(ReviewState::Required(RequiredReason::StaleHash), "required (stale hash)")]
#[case::empty(ReviewState::NotRequired(NotRequiredReason::Empty), "not required (empty)")]
#[case::approved(ReviewState::NotRequired(NotRequiredReason::ZeroFindings), "approved")]
fn test_review_state_display(#[case] state: ReviewState, #[case] expected: &str) {
    assert_eq!(state.to_string(), expected);
}
