//! [`DryCheckApprovalInteractor`] — implementation of [`DryCheckApprovalService`].
//!
//! Evaluates the dry-check approval gate from current diff fragments and
//! previously recorded DRY verdicts.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use domain::dry_check::{
    DryCheckApprovalVerdict, DryCheckPairKey, DryCheckReader, DryCheckRecord, DryCheckVerdict,
};
use domain::semantic_dup::{CodeFragment, SimilarityThreshold};

use super::errors::DryCheckCycleError;
use super::services::DryCheckApprovalService;
use super::shared::{
    build_corpus_index, candidate_pair_keys_for_diff, collect_above_threshold_candidates,
};
use crate::semantic_dup::{EmbeddingPort, SemanticIndexPort};

// ── DryCheckApprovalInteractor ────────────────────────────────────────────────

/// Interactor implementing [`DryCheckApprovalService`].
///
/// Coordinates dry-check history reads, semantic search ports, and approval
/// gate evaluation.
///
/// The constructor return type is written as `DryCheckApprovalInteractor` (not
/// `Self`) so the ③ evaluator exact-string match succeeds.
pub struct DryCheckApprovalInteractor {
    reader: Arc<dyn DryCheckReader>,
    index_port: Arc<dyn SemanticIndexPort>,
    embedding_port: Arc<dyn EmbeddingPort>,
}

impl DryCheckApprovalInteractor {
    /// Create a new [`DryCheckApprovalInteractor`].
    ///
    /// # Parameters
    ///
    /// - `reader`: port for reading the dry-check history.
    /// - `index_port`: port for the semantic vector index.
    /// - `embedding_port`: port for embedding computation.
    #[must_use]
    pub fn new(
        reader: Arc<dyn DryCheckReader>,
        index_port: Arc<dyn SemanticIndexPort>,
        embedding_port: Arc<dyn EmbeddingPort>,
    ) -> DryCheckApprovalInteractor {
        DryCheckApprovalInteractor { reader, index_port, embedding_port }
    }
}

impl DryCheckApprovalService for DryCheckApprovalInteractor {
    /// Evaluate the dry-check gate for the current diff scope.
    ///
    /// # Algorithm
    ///
    /// 1. Build a fresh whole-codebase index from `corpus_fragments`
    ///    (`EmbeddingPort` + `SemanticIndexPort`).
    /// 2. Read all records via `DryCheckReader::read_records()` and derive the
    ///    latest-per-pair map (key = `record.pair_key()`, value = full
    ///    `DryCheckRecord`).
    /// 3. For each `diff_fragment`:
    ///    a. Compute `changed_ref` (SHA-256 of content + `FilePath`).
    ///    b. Run exhaustive growing-k threshold-boundary loop (k, 2k, 4k, …).
    ///    c. For each above-threshold candidate:
    ///       - Compute `candidate_ref`.
    ///       - `DryCheckPairKey::new(changed_ref, candidate_ref)`:
    ///         `Err(SelfMatch)` → skip (excluded from gate).
    ///         `Ok(pair_key)` → look up in latest-per-pair map (CN-07).
    ///       - Absent → unresolved.
    ///       - Present, verdict `NotAViolation` or `Accepted` → resolved.
    ///       - Present, verdict `Violation { .. }` → unresolved (AC-04/CN-06).
    /// 4. Return `Approved` when zero unresolved pairs; else `Blocked { unresolved_pair_count }`.
    ///
    /// # Errors
    ///
    /// Returns [`DryCheckCycleError`] on embedding, index, or reader failures.
    fn check_approved(
        &self,
        corpus_fragments: Vec<CodeFragment>,
        diff_fragments: &[CodeFragment],
        threshold: SimilarityThreshold,
    ) -> Result<DryCheckApprovalVerdict, DryCheckCycleError> {
        // ── Step 1: Build whole-codebase index from corpus_fragments ──────────
        build_corpus_index(
            corpus_fragments,
            self.embedding_port.as_ref(),
            self.index_port.as_ref(),
        )?;

        // ── Step 2: Build latest-per-pair map from history ────────────────────
        //
        // CN-07: identifier matching — when content changes, content_hash
        // changes, so FragmentRef changes, so DryCheckPairKey changes → no
        // match → the pair is unresolved (unverified).  No separate
        // hash-comparison step.
        let records = self.reader.read_records().map_err(DryCheckCycleError::Reader)?;

        let mut latest_per_pair: BTreeMap<DryCheckPairKey, DryCheckRecord> = BTreeMap::new();
        for record in records {
            latest_per_pair.insert(record.pair_key().clone(), record);
        }

        // ── Step 3: Per diff_fragment loop ────────────────────────────────────
        let mut checked_pairs: BTreeSet<DryCheckPairKey> = BTreeSet::new();
        let mut unresolved_pairs: BTreeSet<DryCheckPairKey> = BTreeSet::new();

        for diff_fragment in diff_fragments {
            let above_threshold_candidates = collect_above_threshold_candidates(
                diff_fragment,
                threshold,
                self.embedding_port.as_ref(),
                self.index_port.as_ref(),
            )?;
            let candidate_pairs =
                candidate_pair_keys_for_diff(diff_fragment, above_threshold_candidates)?;

            // ── Per candidate: gate check ─────────────────────────────────────
            for candidate_pair in candidate_pairs {
                // Growing top-k searches return overlapping windows. Count and
                // check each pair identity once so Blocked reports distinct pairs.
                if !checked_pairs.insert(candidate_pair.pair_key.clone()) {
                    continue;
                }

                // Look up in latest-per-pair map (CN-07 identifier matching).
                let unresolved = match latest_per_pair.get(&candidate_pair.pair_key) {
                    None => {
                        // Absent → unresolved (unverified pair).
                        true
                    }
                    Some(record) => {
                        // Present: check verdict (AC-04/CN-06).
                        if !matches!(
                            record.verdict(),
                            DryCheckVerdict::NotAViolation | DryCheckVerdict::Accepted
                        ) {
                            // Violation { .. } counts as unresolved.
                            true
                        } else {
                            // NotAViolation | Accepted → resolved.
                            false
                        }
                    }
                };

                if unresolved {
                    unresolved_pairs.insert(candidate_pair.pair_key);
                }
            }
        }

        if unresolved_pairs.is_empty() {
            Ok(DryCheckApprovalVerdict::Approved)
        } else {
            Ok(DryCheckApprovalVerdict::Blocked { unresolved_pair_count: unresolved_pairs.len() })
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::type_complexity
)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use domain::dry_check::{
        DryCheckApprovalVerdict, DryCheckEntry, DryCheckPairKey, DryCheckReaderError,
        DryCheckRecord, DryCheckVerdict, FragmentRef, Rationale, RefactorProposal,
    };
    use domain::review_v2::types::FilePath;
    use domain::semantic_dup::{
        CodeFragment, SimilarFragment, SimilarityScore, SimilarityThreshold,
    };
    use domain::{CommitHash, Timestamp};

    use super::*;
    use crate::dry_check::shared::content_hash_of;
    use crate::dry_check::shared::test_mocks::{MockMockEmbeddingPort, MockMockSemanticIndexPort};
    use crate::semantic_dup::{EmbeddingError, SemanticIndexError};

    // ── Mock port definitions ─────────────────────────────────────────────────
    //
    // `MockMockEmbeddingPort` and `MockMockSemanticIndexPort` are defined once in
    // `crate::dry_check::shared::test_mocks` and imported above.

    // ── Stubs ─────────────────────────────────────────────────────────────────

    struct StubReader {
        records: Vec<DryCheckRecord>,
    }

    impl StubReader {
        fn new(records: Vec<DryCheckRecord>) -> Self {
            Self { records }
        }
    }

    impl domain::dry_check::DryCheckReader for StubReader {
        fn read_records(&self) -> Result<Vec<DryCheckRecord>, DryCheckReaderError> {
            Ok(self.records.clone())
        }
    }

    struct ErrorReader;

    impl domain::dry_check::DryCheckReader for ErrorReader {
        fn read_records(&self) -> Result<Vec<DryCheckRecord>, DryCheckReaderError> {
            Err(DryCheckReaderError::Io {
                path: "dry-check.json".to_owned(),
                detail: "simulated io error".to_owned(),
            })
        }
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn make_fragment(path: &str, content: &str) -> CodeFragment {
        CodeFragment::new(PathBuf::from(path), content.to_owned(), 1, 1).unwrap()
    }

    fn make_score(v: f32) -> SimilarityScore {
        SimilarityScore::new(v).unwrap()
    }

    fn make_threshold(v: f32) -> SimilarityThreshold {
        SimilarityThreshold::new(v).unwrap()
    }

    fn make_similar_fragment(path: &str, content: &str, score: f32) -> SimilarFragment {
        SimilarFragment { fragment: make_fragment(path, content), score: make_score(score) }
    }

    fn make_record_for_fragments(
        diff_frag: &CodeFragment,
        cand_frag: &CodeFragment,
        verdict: DryCheckVerdict,
    ) -> DryCheckRecord {
        let diff_hash = content_hash_of(diff_frag.content()).unwrap();
        let cand_hash = content_hash_of(cand_frag.content()).unwrap();
        let diff_path = FilePath::new(diff_frag.source_path.to_string_lossy().as_ref()).unwrap();
        let cand_path = FilePath::new(cand_frag.source_path.to_string_lossy().as_ref()).unwrap();
        let diff_ref = FragmentRef::new(diff_path.clone(), diff_hash);
        let cand_ref = FragmentRef::new(cand_path, cand_hash);
        let pair_key = DryCheckPairKey::new(diff_ref, cand_ref).unwrap();
        let changed_path = diff_path;
        let score = SimilarityScore::new(0.9).unwrap();
        let threshold = SimilarityThreshold::new(0.8).unwrap();
        let base_commit = CommitHash::try_new("a".repeat(40)).unwrap();
        let rationale = Rationale::new("test rationale").unwrap();
        let entry = DryCheckEntry::new(
            pair_key,
            changed_path,
            verdict,
            score,
            threshold,
            base_commit,
            rationale,
        )
        .unwrap();
        DryCheckRecord::from_entry_and_timestamp(
            entry,
            Timestamp::new("2026-06-02T00:00:00Z").unwrap(),
        )
        .unwrap()
    }

    fn make_interactor(
        embed: MockMockEmbeddingPort,
        index: MockMockSemanticIndexPort,
        records: Vec<DryCheckRecord>,
    ) -> DryCheckApprovalInteractor {
        DryCheckApprovalInteractor::new(
            Arc::new(StubReader::new(records)),
            Arc::new(index),
            Arc::new(embed),
        )
    }

    fn make_interactor_empty_history(
        embed: MockMockEmbeddingPort,
        index: MockMockSemanticIndexPort,
    ) -> DryCheckApprovalInteractor {
        make_interactor(embed, index, vec![])
    }

    // ── all-clean returns Approved ────────────────────────────────────────────

    #[test]
    fn test_all_clean_no_above_threshold_candidates_returns_approved() {
        let diff_frag = make_fragment("src/a.rs", "fn unique() {}");

        let mut embed = MockMockEmbeddingPort::new();
        embed.expect_embed().returning(|_| Ok(vec![0.1_f32]));

        let mut index = MockMockSemanticIndexPort::new();
        index.expect_insert_batch().returning(|_| Ok(()));
        // No candidates above threshold — return empty.
        index.expect_search().returning(|_, _| Ok(vec![]));

        let interactor = make_interactor_empty_history(embed, index);

        let result = interactor.check_approved(vec![], &[diff_frag], make_threshold(0.8)).unwrap();
        assert_eq!(result, DryCheckApprovalVerdict::Approved);
    }

    // ── cached Violation returns Blocked ──────────────────────────────────────

    #[test]
    fn test_cached_violation_returns_blocked() {
        let diff_frag = make_fragment("src/a.rs", "fn duplicated() {}");
        let cand_frag = make_fragment("src/b.rs", "fn also_duplicated() {}");

        let proposal = RefactorProposal::new("Extract to shared module.").unwrap();
        let violation_record = make_record_for_fragments(
            &diff_frag,
            &cand_frag,
            DryCheckVerdict::Violation { refactor_proposal: proposal },
        );

        let mut embed = MockMockEmbeddingPort::new();
        embed.expect_embed().returning(|_| Ok(vec![0.1_f32]));

        let mut index = MockMockSemanticIndexPort::new();
        index.expect_insert_batch().returning(|_| Ok(()));
        let cand_clone = cand_frag.clone();
        index.expect_search().returning(move |_, _| {
            Ok(vec![SimilarFragment { fragment: cand_clone.clone(), score: make_score(0.9) }])
        });

        let interactor = make_interactor(embed, index, vec![violation_record]);

        let result = interactor.check_approved(vec![], &[diff_frag], make_threshold(0.8)).unwrap();
        assert_eq!(result, DryCheckApprovalVerdict::Blocked { unresolved_pair_count: 1 });
    }

    // ── unverified pair returns Blocked ───────────────────────────────────────

    #[test]
    fn test_unverified_pair_not_in_history_returns_blocked() {
        let diff_frag = make_fragment("src/a.rs", "fn new_code() {}");
        let cand_frag = make_fragment("src/b.rs", "fn similar_new_code() {}");

        let mut embed = MockMockEmbeddingPort::new();
        embed.expect_embed().returning(|_| Ok(vec![0.1_f32]));

        let mut index = MockMockSemanticIndexPort::new();
        index.expect_insert_batch().returning(|_| Ok(()));
        let cand_clone = cand_frag.clone();
        index.expect_search().returning(move |_, _| {
            Ok(vec![SimilarFragment { fragment: cand_clone.clone(), score: make_score(0.9) }])
        });

        // Empty history → pair not found → unresolved.
        let interactor = make_interactor_empty_history(embed, index);

        let result = interactor.check_approved(vec![], &[diff_frag], make_threshold(0.8)).unwrap();
        assert_eq!(result, DryCheckApprovalVerdict::Blocked { unresolved_pair_count: 1 });
    }

    #[test]
    fn test_check_approved_growing_k_duplicate_windows_counts_distinct_unresolved_pairs() {
        let diff_frag = make_fragment("src/a.rs", "fn new_code() {}");
        let repeated_candidates: Vec<SimilarFragment> = (0..10)
            .map(|i| {
                let path = format!("src/candidate_{i}.rs");
                let content = format!("fn candidate_{i}() {{}}");
                make_similar_fragment(&path, &content, 0.9)
            })
            .collect();

        let mut embed = MockMockEmbeddingPort::new();
        embed.expect_embed().returning(|_| Ok(vec![0.1_f32]));

        let mut index = MockMockSemanticIndexPort::new();
        index.expect_insert_batch().returning(|_| Ok(()));
        let mut search_call = 0usize;
        index.expect_search().times(2).returning(move |_, top_k| {
            search_call += 1;
            match search_call {
                1 => {
                    assert_eq!(top_k.value(), 10);
                    Ok(repeated_candidates.clone())
                }
                2 => {
                    assert_eq!(top_k.value(), 20);
                    let mut batch = repeated_candidates.clone();
                    batch.push(make_similar_fragment("src/boundary.rs", "fn boundary() {}", 0.1));
                    Ok(batch)
                }
                _ => panic!("unexpected search call"),
            }
        });

        let interactor = make_interactor_empty_history(embed, index);

        let result = interactor.check_approved(vec![], &[diff_frag], make_threshold(0.8)).unwrap();
        assert_eq!(result, DryCheckApprovalVerdict::Blocked { unresolved_pair_count: 10 });
    }

    // ── content-changed pair (new hash) returns Blocked ───────────────────────

    #[test]
    fn test_content_changed_pair_new_hash_returns_blocked() {
        // Old history has a NotAViolation for (src/a.rs, old content) × (src/b.rs, cand content).
        let old_diff = make_fragment("src/a.rs", "fn old_impl() {}");
        let cand_frag = make_fragment("src/b.rs", "fn candidate() {}");
        let not_a_violation_record =
            make_record_for_fragments(&old_diff, &cand_frag, DryCheckVerdict::NotAViolation);

        // Now the diff has CHANGED content → new hash → new pair_key → no match → Blocked.
        let new_diff = make_fragment("src/a.rs", "fn new_impl() {}");

        let mut embed = MockMockEmbeddingPort::new();
        embed.expect_embed().returning(|_| Ok(vec![0.1_f32]));

        let mut index = MockMockSemanticIndexPort::new();
        index.expect_insert_batch().returning(|_| Ok(()));
        let cand_clone = cand_frag.clone();
        index.expect_search().returning(move |_, _| {
            Ok(vec![SimilarFragment { fragment: cand_clone.clone(), score: make_score(0.9) }])
        });

        let interactor = make_interactor(embed, index, vec![not_a_violation_record]);

        let result = interactor.check_approved(vec![], &[new_diff], make_threshold(0.8)).unwrap();
        assert_eq!(result, DryCheckApprovalVerdict::Blocked { unresolved_pair_count: 1 });
    }

    // ── roles-swapped same-content returns Approved ───────────────────────────

    #[test]
    fn test_roles_swapped_same_content_returns_approved() {
        // Record stored with (diff, cand) order → DryCheckPairKey normalizes.
        // Now checking with (cand, diff) order → same DryCheckPairKey → Approved.
        let frag_a = make_fragment("src/a.rs", "fn shared_logic() {}");
        let frag_b = make_fragment("src/b.rs", "fn also_shared_logic() {}");

        // Record was stored when diff=frag_a, cand=frag_b.
        let nat_record =
            make_record_for_fragments(&frag_a, &frag_b, DryCheckVerdict::NotAViolation);

        // Now checking with diff=frag_b, cand=frag_a (roles swapped).
        let mut embed = MockMockEmbeddingPort::new();
        embed.expect_embed().returning(|_| Ok(vec![0.1_f32]));

        let mut index = MockMockSemanticIndexPort::new();
        index.expect_insert_batch().returning(|_| Ok(()));
        let frag_a_clone = frag_a.clone();
        index.expect_search().returning(move |_, _| {
            Ok(vec![SimilarFragment { fragment: frag_a_clone.clone(), score: make_score(0.9) }])
        });

        let interactor = make_interactor(embed, index, vec![nat_record]);

        // diff=frag_b, cand=frag_a → DryCheckPairKey same as stored → Approved.
        let result = interactor.check_approved(vec![], &[frag_b], make_threshold(0.8)).unwrap();
        assert_eq!(result, DryCheckApprovalVerdict::Approved);
    }

    // ── self-match (path AND hash equal) excluded ─────────────────────────────

    #[test]
    fn test_self_match_excluded_from_gate() {
        // Diff fragment and candidate share BOTH path AND content → self-match → excluded.
        let content = "fn self_fn() {}";
        let diff_frag = make_fragment("src/a.rs", content);

        let mut embed = MockMockEmbeddingPort::new();
        embed.expect_embed().returning(|_| Ok(vec![0.1_f32]));

        let mut index = MockMockSemanticIndexPort::new();
        index.expect_insert_batch().returning(|_| Ok(()));
        // Candidate is literally the same fragment.
        index
            .expect_search()
            .returning(move |_, _| Ok(vec![make_similar_fragment("src/a.rs", content, 1.0)]));

        let interactor = make_interactor_empty_history(embed, index);

        let result = interactor.check_approved(vec![], &[diff_frag], make_threshold(0.8)).unwrap();
        // Self-match excluded → no unresolved pairs → Approved.
        assert_eq!(result, DryCheckApprovalVerdict::Approved);
    }

    // ── paths-same-hash-different is valid pair ───────────────────────────────

    #[test]
    fn test_paths_same_hash_different_is_valid_pair_not_excluded() {
        // Same path but DIFFERENT content → different hash → valid pair → NOT excluded.
        let diff_content = "fn impl_a() {}";
        let cand_content = "fn impl_b() {}"; // different content → different hash
        let diff_frag = make_fragment("src/a.rs", diff_content);

        let mut embed = MockMockEmbeddingPort::new();
        embed.expect_embed().returning(|_| Ok(vec![0.1_f32]));

        let mut index = MockMockSemanticIndexPort::new();
        index.expect_insert_batch().returning(|_| Ok(()));
        // Same path, different content.
        index
            .expect_search()
            .returning(move |_, _| Ok(vec![make_similar_fragment("src/a.rs", cand_content, 0.9)]));

        // Empty history → pair not found → Blocked (valid pair, not excluded).
        let interactor = make_interactor_empty_history(embed, index);

        let result = interactor.check_approved(vec![], &[diff_frag], make_threshold(0.8)).unwrap();
        assert_eq!(result, DryCheckApprovalVerdict::Blocked { unresolved_pair_count: 1 });
    }

    // ── cached NotAViolation returns Approved ────────────────────────────────

    #[test]
    fn test_cached_not_a_violation_returns_approved() {
        let diff_frag = make_fragment("src/a.rs", "fn clean_fn() {}");
        let cand_frag = make_fragment("src/b.rs", "fn similar_clean() {}");
        let record =
            make_record_for_fragments(&diff_frag, &cand_frag, DryCheckVerdict::NotAViolation);

        let mut embed = MockMockEmbeddingPort::new();
        embed.expect_embed().returning(|_| Ok(vec![0.1_f32]));

        let mut index = MockMockSemanticIndexPort::new();
        index.expect_insert_batch().returning(|_| Ok(()));
        let cand_clone = cand_frag.clone();
        index.expect_search().returning(move |_, _| {
            Ok(vec![SimilarFragment { fragment: cand_clone.clone(), score: make_score(0.9) }])
        });

        let interactor = make_interactor(embed, index, vec![record]);

        let result = interactor.check_approved(vec![], &[diff_frag], make_threshold(0.8)).unwrap();
        assert_eq!(result, DryCheckApprovalVerdict::Approved);
    }

    // ── cached Accepted returns Approved ─────────────────────────────────────

    #[test]
    fn test_cached_accepted_returns_approved() {
        let diff_frag = make_fragment("src/a.rs", "fn cross_layer() {}");
        let cand_frag = make_fragment("src/b.rs", "fn cross_layer_mirror() {}");
        let record = make_record_for_fragments(&diff_frag, &cand_frag, DryCheckVerdict::Accepted);

        let mut embed = MockMockEmbeddingPort::new();
        embed.expect_embed().returning(|_| Ok(vec![0.1_f32]));

        let mut index = MockMockSemanticIndexPort::new();
        index.expect_insert_batch().returning(|_| Ok(()));
        let cand_clone = cand_frag.clone();
        index.expect_search().returning(move |_, _| {
            Ok(vec![SimilarFragment { fragment: cand_clone.clone(), score: make_score(0.9) }])
        });

        let interactor = make_interactor(embed, index, vec![record]);

        let result = interactor.check_approved(vec![], &[diff_frag], make_threshold(0.8)).unwrap();
        assert_eq!(result, DryCheckApprovalVerdict::Approved);
    }

    // ── reader error propagated as DryCheckCycleError::Reader ────────────────

    #[test]
    fn test_reader_error_propagated() {
        let mut embed = MockMockEmbeddingPort::new();
        embed.expect_embed().returning(|_| Ok(vec![0.1_f32]));
        let mut index = MockMockSemanticIndexPort::new();
        index.expect_insert_batch().returning(|_| Ok(()));

        let interactor = DryCheckApprovalInteractor::new(
            Arc::new(ErrorReader),
            Arc::new(index),
            Arc::new(embed),
        );

        let diff_frag = make_fragment("src/a.rs", "fn a() {}");
        let result = interactor.check_approved(vec![], &[diff_frag], make_threshold(0.8));
        assert!(matches!(result, Err(DryCheckCycleError::Reader(_))));
    }

    #[test]
    fn test_check_approved_embedding_error_propagated() {
        let diff_frag = make_fragment("src/a.rs", "fn a() {}");

        let mut embed = MockMockEmbeddingPort::new();
        embed
            .expect_embed()
            .returning(|_| Err(EmbeddingError::InferenceFailed { source: "simulated".to_owned() }));

        let mut index = MockMockSemanticIndexPort::new();
        index.expect_insert_batch().returning(|_| Ok(()));

        let interactor = make_interactor_empty_history(embed, index);
        let result = interactor.check_approved(vec![], &[diff_frag], make_threshold(0.8));

        assert!(matches!(result, Err(DryCheckCycleError::Embedding(_))));
    }

    #[test]
    fn test_check_approved_index_insert_error_propagated() {
        let corpus_frag = make_fragment("src/corpus.rs", "fn corpus() {}");

        let mut embed = MockMockEmbeddingPort::new();
        // Corpus build now uses embed_batch (one call for all fragments).
        embed
            .expect_embed_batch()
            .returning(|frags| Ok(frags.iter().map(|_| vec![0.1_f32]).collect()));

        let mut index = MockMockSemanticIndexPort::new();
        index.expect_insert_batch().returning(|_| {
            Err(SemanticIndexError::InsertFailed { source: "simulated".to_owned() })
        });

        let interactor = make_interactor_empty_history(embed, index);
        let result = interactor.check_approved(vec![corpus_frag], &[], make_threshold(0.8));

        assert!(matches!(result, Err(DryCheckCycleError::Index(_))));
    }

    #[test]
    fn test_check_approved_index_search_error_propagated() {
        let diff_frag = make_fragment("src/a.rs", "fn a() {}");

        let mut embed = MockMockEmbeddingPort::new();
        embed.expect_embed().returning(|_| Ok(vec![0.1_f32]));

        let mut index = MockMockSemanticIndexPort::new();
        index.expect_insert_batch().returning(|_| Ok(()));
        index.expect_search().returning(|_, _| {
            Err(SemanticIndexError::SearchFailed { source: "simulated".to_owned() })
        });

        let interactor = make_interactor_empty_history(embed, index);
        let result = interactor.check_approved(vec![], &[diff_frag], make_threshold(0.8));

        assert!(matches!(result, Err(DryCheckCycleError::Index(_))));
    }

    // ── multiple diff_fragments — one resolved, one not → Blocked ────────────

    #[test]
    fn test_multiple_diff_fragments_partial_resolved_returns_blocked() {
        let diff_a = make_fragment("src/a.rs", "fn fn_a() {}");
        let cand_a = make_fragment("src/x.rs", "fn fn_x() {}");

        let diff_b = make_fragment("src/b.rs", "fn fn_b() {}");
        let cand_b = make_fragment("src/y.rs", "fn fn_y() {}");

        // Record for (diff_a, cand_a) → NotAViolation → resolved.
        let record_a = make_record_for_fragments(&diff_a, &cand_a, DryCheckVerdict::NotAViolation);
        // (diff_b, cand_b) → NOT in history → unresolved.

        let mut embed = MockMockEmbeddingPort::new();
        embed.expect_embed().returning(|_| Ok(vec![0.1_f32]));

        let mut index = MockMockSemanticIndexPort::new();
        index.expect_insert_batch().returning(|_| Ok(()));

        // First search (for diff_a): returns cand_a.
        // Second search (for diff_b): returns cand_b.
        let cand_a_clone = cand_a.clone();
        let cand_b_clone = cand_b.clone();
        let mut search_call = 0u32;
        index.expect_search().returning(move |_, _| {
            search_call += 1;
            if search_call == 1 {
                Ok(vec![SimilarFragment { fragment: cand_a_clone.clone(), score: make_score(0.9) }])
            } else {
                Ok(vec![SimilarFragment { fragment: cand_b_clone.clone(), score: make_score(0.9) }])
            }
        });

        let interactor = make_interactor(embed, index, vec![record_a]);

        let result =
            interactor.check_approved(vec![], &[diff_a, diff_b], make_threshold(0.8)).unwrap();
        assert_eq!(result, DryCheckApprovalVerdict::Blocked { unresolved_pair_count: 1 });
    }
}
