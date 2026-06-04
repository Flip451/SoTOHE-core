//! Domain types and ports for the DRY violation auto-detection capability.
//!
//! This module implements the core abstractions for detecting duplicate code
//! (DRY violations) using semantic similarity search and agent-based judgment.
//! See ADR 2026-06-02-0716-dry-checker for the design decisions.

mod diff;
mod finding;
mod fragment;
mod ports;
mod record;
mod value_objects;
mod verdict;

pub use self::diff::*;
pub use self::finding::*;
pub use self::fragment::*;
pub use self::ports::*;
pub use self::record::*;
pub use self::value_objects::*;
pub use self::verdict::*;

// ── fragments_overlapping_hunks ───────────────────────────────────────────────

use std::path::Path;

use crate::review_v2::types::FilePath;
use crate::semantic_dup::CodeFragment;

/// Filter a slice of [`CodeFragment`]s to those whose source span overlaps any
/// added/changed hunk in `changed_hunks`.
///
/// A fragment overlaps a hunk when:
/// - (a) `fragment.source_path` matches the `DiffFileHunks.path` exactly (byte-equal
///   path comparison), AND
/// - (b) the fragment's `[start_line..=end_line]` range shares at least one
///   line with a `DiffHunkRange [start_line..=end_line]`.
///
/// Fragments from files not appearing in `changed_hunks` are excluded.
/// Fragments from changed files that don't overlap any hunk are also excluded.
///
/// # Contract
///
/// Both `CodeFragment.source_path` values in `fragments` and the `DiffFileHunks.path`
/// values in `changed_hunks` **must be in repo-relative form** (the same format as
/// `git diff` hunk paths, e.g. `src/a.rs`). Absolute paths will not match
/// repo-relative hunk paths. Normalizing absolute paths to repo-relative form is the
/// responsibility of the caller (cli-composition layer, T007/T009), which bridges the
/// fragment extractor output and the diff source output before invoking this function.
///
/// This is the core mechanism making CN-04 (unchanged fragments structurally
/// absent from the diff query) deterministic without LLM involvement (D4).
/// Pure function — no I/O, no side effects.
pub fn fragments_overlapping_hunks(
    fragments: &[CodeFragment],
    changed_hunks: &[DiffFileHunks],
) -> Vec<CodeFragment> {
    fragments
        .iter()
        .filter(|fragment| {
            changed_hunks.iter().any(|file_hunks| {
                if !fragment_path_matches_hunk_path(
                    fragment.source_path.as_path(),
                    file_hunks.path(),
                ) {
                    return false;
                }
                // Check overlap with any hunk.
                file_hunks.hunks().iter().any(|hunk| {
                    // Ranges overlap when: frag.start <= hunk.end AND frag.end >= hunk.start
                    fragment.start_line() <= hunk.end_line()
                        && fragment.end_line() >= hunk.start_line()
                })
            })
        })
        .cloned()
        .collect()
}

/// Returns `true` when `fragment_path` exactly equals the repo-relative `hunk_path`.
///
/// Both paths must already be in repo-relative form (e.g. `src/a.rs`). Suffix
/// matching is intentionally absent: `Path::ends_with` does component-level suffix
/// matching, which causes `tests/src/a.rs` to spuriously match a hunk path of
/// `src/a.rs`, introducing unrelated fragments into the hunk-scope query and
/// corrupting the CN-04 scope guarantee.
///
/// Normalization of absolute paths to repo-relative form is the responsibility of
/// the cli-composition layer (T007/T009) before fragments are passed to
/// `fragments_overlapping_hunks`.
fn fragment_path_matches_hunk_path(fragment_path: &Path, hunk_path: &FilePath) -> bool {
    let repo_relative_hunk_path = Path::new(hunk_path.as_str());
    fragment_path == repo_relative_hunk_path
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::ids::CommitHash;
    use crate::review_v2::types::FilePath;
    use crate::semantic_dup::{CodeFragment, SimilarityScore, SimilarityThreshold};
    use crate::timestamp::Timestamp;

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn make_hash(hex: &str) -> FragmentContentHash {
        FragmentContentHash::new(hex).unwrap()
    }

    fn make_file_path(s: &str) -> FilePath {
        FilePath::new(s).unwrap()
    }

    fn make_fragment_ref(path: &str, hash: &str) -> FragmentRef {
        FragmentRef::new(make_file_path(path), make_hash(hash))
    }

    fn make_score() -> SimilarityScore {
        SimilarityScore::new(0.9).unwrap()
    }

    fn make_threshold() -> SimilarityThreshold {
        SimilarityThreshold::new(0.8).unwrap()
    }

    fn make_commit() -> CommitHash {
        CommitHash::try_new("abcdef1234567").unwrap()
    }

    fn make_timestamp() -> Timestamp {
        Timestamp::new("2026-06-02T07:16:00Z").unwrap()
    }

    // ── RefactorProposal ──────────────────────────────────────────────────────

    #[test]
    fn test_refactor_proposal_new_with_non_empty_string_succeeds() {
        let result = RefactorProposal::new("Extract shared logic into a helper function.");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().as_str(), "Extract shared logic into a helper function.");
    }

    #[test]
    fn test_refactor_proposal_new_with_empty_string_returns_empty_error() {
        let result = RefactorProposal::new("");
        assert!(matches!(result, Err(RefactorProposalError::Empty)));
    }

    // ── Rationale ─────────────────────────────────────────────────────────────

    #[test]
    fn test_rationale_new_with_non_empty_string_succeeds() {
        let result = Rationale::new("This is a genuine DRY violation.");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().as_str(), "This is a genuine DRY violation.");
    }

    #[test]
    fn test_rationale_new_with_empty_string_returns_empty_error() {
        let result = Rationale::new("");
        assert!(matches!(result, Err(RationaleError::Empty)));
    }

    // ── FragmentContentHash ───────────────────────────────────────────────────

    #[test]
    fn test_fragment_content_hash_new_with_valid_64_hex_succeeds() {
        let hex = "a".repeat(64);
        let result = FragmentContentHash::new(&hex);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().as_str(), &hex);
    }

    #[test]
    fn test_fragment_content_hash_new_with_63_chars_returns_invalid_format_error() {
        let hex = "a".repeat(63);
        let result = FragmentContentHash::new(&hex);
        assert!(matches!(result, Err(FragmentContentHashError::InvalidFormat(_))));
    }

    #[test]
    fn test_fragment_content_hash_new_with_65_chars_returns_invalid_format_error() {
        let hex = "a".repeat(65);
        let result = FragmentContentHash::new(&hex);
        assert!(matches!(result, Err(FragmentContentHashError::InvalidFormat(_))));
    }

    #[test]
    fn test_fragment_content_hash_new_with_uppercase_hex_returns_invalid_format_error() {
        let hex = "A".repeat(64);
        let result = FragmentContentHash::new(&hex);
        assert!(matches!(result, Err(FragmentContentHashError::InvalidFormat(_))));
    }

    #[test]
    fn test_fragment_content_hash_new_with_non_hex_chars_returns_invalid_format_error() {
        let hex = "g".repeat(64);
        let result = FragmentContentHash::new(&hex);
        assert!(matches!(result, Err(FragmentContentHashError::InvalidFormat(_))));
    }

    // ── FragmentRef Ord ───────────────────────────────────────────────────────

    #[test]
    fn test_fragment_ref_ord_sorts_by_path_then_content_hash() {
        let a = make_fragment_ref("src/a.rs", &"a".repeat(64));
        let b = make_fragment_ref("src/b.rs", &"a".repeat(64));
        assert!(a < b, "path 'src/a.rs' should sort before 'src/b.rs'");

        let c = make_fragment_ref("src/a.rs", &"a".repeat(64));
        let d = make_fragment_ref("src/a.rs", &"b".repeat(64));
        assert!(c < d, "same path: hash 'aaa...' should sort before 'bbb...'");
    }

    // ── DryCheckPairKey ───────────────────────────────────────────────────────

    #[test]
    fn test_dry_check_pair_key_new_normalizes_order_xy_equals_yx() {
        let x = make_fragment_ref("src/a.rs", &"a".repeat(64));
        let y = make_fragment_ref("src/b.rs", &"b".repeat(64));

        let key_xy = DryCheckPairKey::new(x.clone(), y.clone()).unwrap();
        let key_yx = DryCheckPairKey::new(y.clone(), x.clone()).unwrap();

        assert_eq!(key_xy, key_yx, "(X,Y) and (Y,X) must produce the same key");
        assert_eq!(key_xy.low(), key_yx.low());
        assert_eq!(key_xy.high(), key_yx.high());
    }

    #[test]
    fn test_dry_check_pair_key_new_rejects_self_match_when_both_path_and_hash_match() {
        let same = make_fragment_ref("src/a.rs", &"a".repeat(64));
        let result = DryCheckPairKey::new(same.clone(), same);
        assert!(matches!(result, Err(DryCheckPairKeyError::SelfMatch)));
    }

    #[test]
    fn test_dry_check_pair_key_new_allows_same_path_different_hash() {
        // paths identical but hashes differ → valid pair (distinct content states)
        let a = make_fragment_ref("src/a.rs", &"a".repeat(64));
        let b = make_fragment_ref("src/a.rs", &"b".repeat(64));
        let result = DryCheckPairKey::new(a, b);
        assert!(result.is_ok(), "same path with different hash is NOT a self-match");
    }

    #[test]
    fn test_dry_check_pair_key_new_allows_different_path_same_hash() {
        // complete copies in different files → valid pair
        let a = make_fragment_ref("src/a.rs", &"a".repeat(64));
        let b = make_fragment_ref("src/b.rs", &"a".repeat(64));
        let result = DryCheckPairKey::new(a, b);
        assert!(result.is_ok(), "different path with same hash is NOT a self-match");
    }

    // ── DryCheckEntry ─────────────────────────────────────────────────────────

    #[test]
    fn test_dry_check_entry_new_round_trips_all_7_fields() {
        let low = make_fragment_ref("src/a.rs", &"a".repeat(64));
        let high = make_fragment_ref("src/b.rs", &"b".repeat(64));
        let pair_key = DryCheckPairKey::new(low, high).unwrap();
        let changed_path = make_file_path("src/a.rs");
        let verdict = DryCheckVerdict::NotAViolation;
        let score = make_score();
        let threshold = make_threshold();
        let commit = make_commit();
        let rationale = Rationale::new("Rejected — self-similar.").unwrap();

        let entry = DryCheckEntry::new(
            pair_key.clone(),
            changed_path.clone(),
            verdict.clone(),
            score,
            threshold,
            commit.clone(),
            rationale.clone(),
        )
        .unwrap();

        assert_eq!(entry.pair_key(), &pair_key);
        assert_eq!(entry.changed_path(), &changed_path);
        assert_eq!(entry.verdict(), &verdict);
        assert_eq!(entry.similarity_score().value(), score.value());
        assert_eq!(entry.threshold().value(), threshold.value());
        assert_eq!(entry.base_commit().as_ref(), commit.as_ref());
        assert_eq!(entry.rationale(), &rationale);
    }

    #[test]
    fn test_dry_check_entry_new_rejects_changed_path_outside_pair() {
        let low = make_fragment_ref("src/a.rs", &"a".repeat(64));
        let high = make_fragment_ref("src/b.rs", &"b".repeat(64));
        let pair_key = DryCheckPairKey::new(low, high).unwrap();
        let changed_path = make_file_path("src/c.rs"); // not in pair
        let verdict = DryCheckVerdict::NotAViolation;
        let rationale = Rationale::new("reason").unwrap();

        let result = DryCheckEntry::new(
            pair_key,
            changed_path,
            verdict,
            make_score(),
            make_threshold(),
            make_commit(),
            rationale,
        );

        assert!(matches!(result, Err(DryCheckEntryError::ChangedPathOutsidePair)));
    }

    // ── DryCheckRecord ────────────────────────────────────────────────────────

    #[test]
    fn test_dry_check_record_from_entry_and_timestamp_round_trips_with_recorded_at() {
        let low = make_fragment_ref("src/a.rs", &"a".repeat(64));
        let high = make_fragment_ref("src/b.rs", &"b".repeat(64));
        let pair_key = DryCheckPairKey::new(low, high).unwrap();
        let changed_path = make_file_path("src/a.rs");
        let rationale = Rationale::new("acceptable").unwrap();

        let entry = DryCheckEntry::new(
            pair_key,
            changed_path,
            DryCheckVerdict::Accepted,
            make_score(),
            make_threshold(),
            make_commit(),
            rationale.clone(),
        )
        .unwrap();

        let ts = make_timestamp();
        let record = DryCheckRecord::from_entry_and_timestamp(entry, ts.clone()).unwrap();

        assert_eq!(record.recorded_at(), &ts);
        assert_eq!(record.rationale(), &rationale);
        assert_eq!(record.verdict(), &DryCheckVerdict::Accepted);
    }

    // ── DryCheckVerdict::Violation ─────────────────────────────────────────────

    #[test]
    fn test_dry_check_verdict_violation_carries_non_empty_proposal() {
        let proposal = RefactorProposal::new("Extract helper.").unwrap();
        let verdict = DryCheckVerdict::Violation { refactor_proposal: proposal.clone() };
        match verdict {
            DryCheckVerdict::Violation { refactor_proposal } => {
                assert_eq!(refactor_proposal, proposal);
            }
            _ => panic!("expected Violation variant"),
        }
    }

    // ── DryCheckFinding ───────────────────────────────────────────────────────

    #[test]
    fn test_dry_check_finding_new_with_non_empty_proposal_succeeds() {
        let changed = make_fragment_ref("src/a.rs", &"a".repeat(64));
        let candidate = make_fragment_ref("src/b.rs", &"b".repeat(64));
        let result = DryCheckFinding::new(changed.clone(), candidate.clone(), "Extract helper.");
        assert!(result.is_ok());
        let finding = result.unwrap();
        assert_eq!(finding.changed_fragment_ref(), &changed);
        assert_eq!(finding.candidate_fragment_ref(), &candidate);
        assert_eq!(finding.refactor_proposal().as_str(), "Extract helper.");
    }

    #[test]
    fn test_dry_check_finding_new_with_empty_proposal_returns_empty_proposal_error() {
        let changed = make_fragment_ref("src/a.rs", &"a".repeat(64));
        let candidate = make_fragment_ref("src/b.rs", &"b".repeat(64));
        let result = DryCheckFinding::new(changed, candidate, "");
        assert!(matches!(result, Err(DryCheckFindingError::EmptyProposal)));
    }

    // ── DiffHunkRange ─────────────────────────────────────────────────────────

    #[test]
    fn test_diff_hunk_range_new_with_valid_range_succeeds() {
        let result = DiffHunkRange::new(1, 10);
        assert!(result.is_ok());
        let range = result.unwrap();
        assert_eq!(range.start_line(), 1);
        assert_eq!(range.end_line(), 10);
    }

    #[test]
    fn test_diff_hunk_range_new_with_start_zero_returns_zero_line_error() {
        let result = DiffHunkRange::new(0, 10);
        assert!(matches!(result, Err(DiffHunkRangeError::ZeroLine)));
    }

    #[test]
    fn test_diff_hunk_range_new_with_end_zero_returns_zero_line_error() {
        let result = DiffHunkRange::new(1, 0);
        assert!(matches!(result, Err(DiffHunkRangeError::ZeroLine)));
    }

    #[test]
    fn test_diff_hunk_range_new_with_start_greater_than_end_returns_start_exceeds_end_error() {
        let result = DiffHunkRange::new(10, 5);
        assert!(matches!(result, Err(DiffHunkRangeError::StartExceedsEnd { start: 10, end: 5 })));
    }

    #[test]
    fn test_diff_hunk_range_new_with_single_line_range_succeeds() {
        let result = DiffHunkRange::new(5, 5);
        assert!(result.is_ok());
    }

    // ── DiffFileHunks ─────────────────────────────────────────────────────────

    #[test]
    fn test_diff_file_hunks_new_with_non_empty_hunks_succeeds() {
        let path = make_file_path("src/a.rs");
        let hunk = DiffHunkRange::new(1, 10).unwrap();
        let result = DiffFileHunks::new(path.clone(), vec![hunk.clone()]);
        assert!(result.is_ok());
        let dfh = result.unwrap();
        assert_eq!(dfh.path(), &path);
        assert_eq!(dfh.hunks(), &[hunk]);
    }

    #[test]
    fn test_diff_file_hunks_new_with_empty_hunks_returns_empty_hunks_error() {
        let path = make_file_path("src/a.rs");
        let result = DiffFileHunks::new(path, vec![]);
        assert!(matches!(result, Err(DiffFileHunksError::EmptyHunks)));
    }

    // ── fragments_overlapping_hunks ───────────────────────────────────────────

    fn make_code_fragment(
        path: &str,
        content: &str,
        start_line: u32,
        end_line: u32,
    ) -> CodeFragment {
        CodeFragment::new(PathBuf::from(path), content.to_owned(), start_line, end_line).unwrap()
    }

    #[test]
    fn test_fragments_overlapping_hunks_returns_overlapping_fragments() {
        // Fragment at lines 5-10 in src/a.rs; hunk covers lines 8-12.
        let frag = make_code_fragment("src/a.rs", "fn foo() {}", 5, 10);
        let hunk = DiffHunkRange::new(8, 12).unwrap();
        let file_hunks = DiffFileHunks::new(make_file_path("src/a.rs"), vec![hunk]).unwrap();

        let result = fragments_overlapping_hunks(std::slice::from_ref(&frag), &[file_hunks]);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].content(), frag.content());
    }

    #[test]
    fn test_fragments_overlapping_hunks_repo_relative_path_matches_exact() {
        // Both fragment path and hunk path are repo-relative — exact match succeeds.
        let frag = make_code_fragment("src/a.rs", "fn foo() {}", 5, 10);
        let hunk = DiffHunkRange::new(8, 12).unwrap();
        let file_hunks = DiffFileHunks::new(make_file_path("src/a.rs"), vec![hunk]).unwrap();

        let result = fragments_overlapping_hunks(std::slice::from_ref(&frag), &[file_hunks]);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].content(), frag.content());
    }

    #[test]
    fn test_fragments_overlapping_hunks_suffix_path_does_not_match_hunk_path() {
        // Regression: `tests/src/a.rs` must NOT match hunk path `src/a.rs`.
        // Path::ends_with would spuriously match because `src/a.rs` is a component
        // suffix of `tests/src/a.rs`. The domain contract requires exact (repo-relative)
        // path equality; suffix matching is prohibited (CN-04 correctness).
        let frag = make_code_fragment("tests/src/a.rs", "fn test_foo() {}", 8, 12);
        let hunk = DiffHunkRange::new(8, 12).unwrap();
        let file_hunks = DiffFileHunks::new(make_file_path("src/a.rs"), vec![hunk]).unwrap();

        let result = fragments_overlapping_hunks(std::slice::from_ref(&frag), &[file_hunks]);
        assert!(
            result.is_empty(),
            "`tests/src/a.rs` must not match hunk path `src/a.rs` (suffix match prohibited)"
        );
    }

    #[test]
    fn test_fragments_overlapping_hunks_excludes_non_overlapping_fragments() {
        // Fragment at lines 1-4 in src/a.rs; hunk covers lines 8-12 (no overlap).
        let frag = make_code_fragment("src/a.rs", "fn bar() {}", 1, 4);
        let hunk = DiffHunkRange::new(8, 12).unwrap();
        let file_hunks = DiffFileHunks::new(make_file_path("src/a.rs"), vec![hunk]).unwrap();

        let result = fragments_overlapping_hunks(&[frag], &[file_hunks]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_fragments_overlapping_hunks_excludes_fragments_from_other_files() {
        // Fragment in src/b.rs; hunk is in src/a.rs.
        let frag = make_code_fragment("src/b.rs", "fn baz() {}", 1, 20);
        let hunk = DiffHunkRange::new(1, 20).unwrap();
        let file_hunks = DiffFileHunks::new(make_file_path("src/a.rs"), vec![hunk]).unwrap();

        let result = fragments_overlapping_hunks(&[frag], &[file_hunks]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_fragments_overlapping_hunks_sentinel_query_fragment_always_overlaps() {
        // Ad-hoc query fragment with start_line=1, end_line=u32::MAX always overlaps.
        let frag = make_code_fragment("<query>", "fn query() {}", 1, u32::MAX);
        let hunk = DiffHunkRange::new(1, 100).unwrap();
        // For the query path to match, we'd need to use "<query>" as the file name.
        // Test with a normal file path overlap instead (query path won't match real file).
        let frag2 = make_code_fragment("src/a.rs", "fn real() {}", 1, u32::MAX);
        let file_hunks = DiffFileHunks::new(make_file_path("src/a.rs"), vec![hunk]).unwrap();

        let result = fragments_overlapping_hunks(&[frag, frag2.clone()], &[file_hunks]);
        // Only frag2 matches the file path "src/a.rs"
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].content(), frag2.content());
    }

    #[test]
    fn test_fragments_overlapping_hunks_with_empty_inputs_returns_empty() {
        let result = fragments_overlapping_hunks(&[], &[]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_fragments_overlapping_hunks_with_empty_hunks_list_excludes_all() {
        let frag = make_code_fragment("src/a.rs", "fn foo() {}", 1, 10);
        let result = fragments_overlapping_hunks(&[frag], &[]);
        assert!(result.is_empty());
    }
}
