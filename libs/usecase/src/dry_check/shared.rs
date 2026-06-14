//! Shared helpers for the dry-check interactors (T004 and T005).
//!
//! Extracted to avoid duplicating SHA-256, `FragmentRef` construction, corpus
//! indexing, and threshold-boundary candidate collection across
//! `DryCheckInteractor`, `DryCheckResultsInteractor`, and `DryCheckApprovalInteractor`.

use sha2::Digest as _;

use domain::dry_check::{DryCheckPairKey, DryCheckPairKeyError, FragmentContentHash, FragmentRef};
use domain::review_v2::types::FilePath;
use domain::semantic_dup::{CodeFragment, SimilarityScore, SimilarityThreshold, TopK};

use super::errors::DryCheckCycleError;
use crate::semantic_dup::{EmbeddingError, EmbeddingPort, SemanticIndexError, SemanticIndexPort};

/// Candidate pair produced by the shared threshold-boundary search pipeline.
pub(crate) struct CandidatePair {
    pub(crate) candidate_fragment: CodeFragment,
    pub(crate) pair_key: DryCheckPairKey,
    pub(crate) similarity_score: SimilarityScore,
}

/// Compute the SHA-256 of `content` and return a validated [`FragmentContentHash`].
///
/// # Errors
///
/// Returns a [`String`] error description when [`FragmentContentHash::new`] rejects
/// the hex string (should not happen in practice for a well-formed SHA-256 digest,
/// but propagated as an error to keep production code panic-free).
pub(crate) fn content_hash_of(content: &str) -> Result<FragmentContentHash, String> {
    let digest = sha2::Sha256::digest(content.as_bytes());
    let hex = format!("{digest:x}");
    FragmentContentHash::new(hex).map_err(|e| format!("content hash: {e}"))
}

/// Build a [`FragmentRef`] from a [`CodeFragment`].
///
/// Computes the SHA-256 of `fragment.content()` to produce the
/// [`FragmentContentHash`]. The path comes from `fragment.source_path` (via
/// `to_string_lossy` — the same convention used throughout the workspace).
///
/// # Errors
///
/// Returns a [`String`] error description when `FilePath::new` rejects the
/// path (e.g., absolute path or traversal) or when `content_hash_of` fails.
pub fn fragment_ref_of(fragment: &CodeFragment) -> Result<FragmentRef, String> {
    let path_str = fragment.source_path.to_string_lossy().into_owned();
    let file_path = FilePath::new(path_str).map_err(|e| format!("invalid source_path: {e}"))?;
    let content_hash = content_hash_of(fragment.content())?;
    Ok(FragmentRef::new(file_path, content_hash))
}

/// Embed all corpus fragments and insert them into the semantic index in one batch.
///
/// All fragments are passed to [`EmbeddingPort::embed_batch`] in a single
/// model inference call, eliminating the per-fragment CPU-inference loop as
/// the dominant cost.  The resulting (fragment, embedding) pairs are then
/// inserted via [`SemanticIndexPort::insert_batch`].
///
/// # Errors
///
/// Returns [`DryCheckCycleError`] when embedding or index insertion fails.
pub(crate) fn build_corpus_index(
    corpus_fragments: Vec<CodeFragment>,
    embedding_port: &dyn EmbeddingPort,
    index_port: &dyn SemanticIndexPort,
) -> Result<(), DryCheckCycleError> {
    if corpus_fragments.is_empty() {
        return index_port.insert_batch(&[]).map_err(DryCheckCycleError::Index);
    }
    let embeddings =
        embedding_port.embed_batch(&corpus_fragments).map_err(DryCheckCycleError::Embedding)?;
    if embeddings.len() != corpus_fragments.len() {
        return Err(DryCheckCycleError::Embedding(EmbeddingError::InferenceFailed {
            source: format!(
                "embed_batch returned {} embeddings for {} corpus fragments",
                embeddings.len(),
                corpus_fragments.len()
            ),
        }));
    }
    let corpus_items: Vec<(CodeFragment, Vec<f32>)> =
        corpus_fragments.into_iter().zip(embeddings).collect();
    index_port.insert_batch(&corpus_items).map_err(DryCheckCycleError::Index)
}

/// Collect all candidates for `diff_fragment` that meet or exceed `threshold`.
///
/// Uses the shared growing-k threshold-boundary loop:
/// k, 2k, 4k, ... until a below-threshold result appears, no results are
/// returned, or the index is exhausted.
///
/// # Errors
///
/// Returns [`DryCheckCycleError`] when embedding, searching, or `TopK`
/// construction fails.
pub(crate) fn collect_above_threshold_candidates(
    diff_fragment: &CodeFragment,
    threshold: SimilarityThreshold,
    embedding_port: &dyn EmbeddingPort,
    index_port: &dyn SemanticIndexPort,
) -> Result<Vec<(CodeFragment, SimilarityScore)>, DryCheckCycleError> {
    let query_embedding =
        embedding_port.embed(diff_fragment).map_err(DryCheckCycleError::Embedding)?;
    let mut k: usize = 10;
    let mut above_threshold_candidates: Vec<(CodeFragment, SimilarityScore)> = Vec::new();

    loop {
        let top_k = TopK::new(k).map_err(|_| {
            DryCheckCycleError::Index(SemanticIndexError::SearchFailed {
                source: "internal: k overflowed usize".to_owned(),
            })
        })?;

        let batch =
            index_port.search(&query_embedding, top_k).map_err(DryCheckCycleError::Index)?;

        if batch.is_empty() {
            break;
        }

        let returned_count = batch.len();
        let mut found_boundary = false;

        for similar in batch {
            let score = similar.score;
            let candidate = similar.fragment;

            if score.value() < threshold.value() {
                found_boundary = true;
                continue;
            }

            above_threshold_candidates.push((candidate, score));
        }

        if found_boundary || returned_count < k {
            break;
        }

        k = k.saturating_mul(2);
    }

    Ok(above_threshold_candidates)
}

/// Convert candidate fragments to pair keys for a single diff fragment.
///
/// Self-matches are excluded through `DryCheckPairKey::new`; same path with
/// different content remains a valid pair because the content hashes differ.
///
/// # Errors
///
/// Returns [`DryCheckCycleError`] when fragment refs cannot be constructed.
pub(crate) fn candidate_pair_keys_for_diff(
    diff_fragment: &CodeFragment,
    candidates: Vec<(CodeFragment, SimilarityScore)>,
) -> Result<Vec<CandidatePair>, DryCheckCycleError> {
    let changed_ref = fragment_ref_of(diff_fragment).map_err(|e| {
        DryCheckCycleError::Index(SemanticIndexError::SearchFailed {
            source: format!("changed_fragment path error: {e}"),
        })
    })?;

    let mut pairs = Vec::with_capacity(candidates.len());
    for (candidate_fragment, similarity_score) in candidates {
        let candidate_ref = fragment_ref_of(&candidate_fragment).map_err(|e| {
            DryCheckCycleError::Index(SemanticIndexError::SearchFailed {
                source: format!("candidate_fragment path error: {e}"),
            })
        })?;

        let pair_key = match DryCheckPairKey::new(changed_ref.clone(), candidate_ref) {
            Err(DryCheckPairKeyError::SelfMatch) => continue,
            Ok(pair_key) => pair_key,
        };

        pairs.push(CandidatePair { candidate_fragment, pair_key, similarity_score });
    }

    Ok(pairs)
}

// ── Shared test mocks (crate-internal) ───────────────────────────────────────

/// Shared mock implementations of `EmbeddingPort` and `SemanticIndexPort` for
/// use across all `dry_check` test modules.
///
/// All sibling interactor tests (`interactor.rs`, `approval_interactor.rs`)
/// import from here instead of redefining the same `mockall::mock!` blocks.
#[cfg(test)]
pub(crate) mod test_mocks {
    use domain::semantic_dup::{CodeFragment, SimilarFragment, TopK};
    use mockall::mock;

    use crate::semantic_dup::{EmbeddingError, SemanticIndexError};

    mock! {
        pub MockEmbeddingPort {}
        impl crate::semantic_dup::EmbeddingPort for MockEmbeddingPort {
            fn embed(&self, fragment: &CodeFragment) -> Result<Vec<f32>, EmbeddingError>;
            fn embed_batch(&self, fragments: &[CodeFragment]) -> Result<Vec<Vec<f32>>, EmbeddingError>;
        }
    }

    mock! {
        pub MockSemanticIndexPort {}
        impl crate::semantic_dup::SemanticIndexPort for MockSemanticIndexPort {
            fn insert(
                &self,
                fragment: &CodeFragment,
                embedding: &[f32],
            ) -> Result<(), SemanticIndexError>;

            fn insert_batch(
                &self,
                items: &[(CodeFragment, Vec<f32>)],
            ) -> Result<(), SemanticIndexError>;

            fn delete_by_source_path(
                &self,
                source_path: &std::path::Path,
            ) -> Result<(), SemanticIndexError>;

            fn search(
                &self,
                embedding: &[f32],
                top_k: TopK,
            ) -> Result<Vec<SimilarFragment>, SemanticIndexError>;
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::type_complexity
)]
mod tests {
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    use domain::semantic_dup::CodeFragment;

    use super::test_mocks::{MockMockEmbeddingPort, MockMockSemanticIndexPort};
    use super::*;
    use crate::semantic_dup::EmbeddingError;

    fn make_fragment(path: &str, content: &str) -> CodeFragment {
        CodeFragment::new(PathBuf::from(path), content.to_owned(), 1, 1).unwrap()
    }

    // ── build_corpus_index: embed_batch called once, ordering preserved ────────

    /// Verify that `build_corpus_index` calls `embed_batch` exactly once with
    /// all fragments, and that the (fragment, embedding) pairs passed to
    /// `insert_batch` are in the same order as the input fragments.
    #[test]
    fn test_build_corpus_index_calls_embed_batch_once_and_preserves_order() {
        let frag_a = make_fragment("src/a.rs", "fn a() {}");
        let frag_b = make_fragment("src/b.rs", "fn b() {}");
        let frag_c = make_fragment("src/c.rs", "fn c() {}");

        // Assign a distinct embedding per fragment to verify ordering.
        let embeddings = vec![vec![1.0_f32], vec![2.0_f32], vec![3.0_f32]];

        let mut embed = MockMockEmbeddingPort::new();
        let embeddings_clone = embeddings.clone();
        // embed_batch must be called exactly once with all 3 fragments.
        embed
            .expect_embed_batch()
            .times(1)
            .withf(|frags| frags.len() == 3)
            .returning(move |_| Ok(embeddings_clone.clone()));

        // Capture the items passed to insert_batch for ordering verification.
        let captured: Arc<Mutex<Vec<(PathBuf, Vec<f32>)>>> = Arc::new(Mutex::new(Vec::new()));
        let captured_clone = Arc::clone(&captured);
        let mut index = MockMockSemanticIndexPort::new();
        index.expect_insert_batch().times(1).returning(move |items| {
            let mut guard = captured_clone.lock().unwrap();
            for (frag, emb) in items {
                guard.push((frag.source_path.clone(), emb.clone()));
            }
            Ok(())
        });

        build_corpus_index(
            vec![frag_a, frag_b, frag_c],
            &embed as &dyn EmbeddingPort,
            &index as &dyn SemanticIndexPort,
        )
        .unwrap();

        let pairs = captured.lock().unwrap();
        assert_eq!(pairs.len(), 3, "all 3 corpus items must be inserted");
        // Verify input order is preserved: a→[1.0], b→[2.0], c→[3.0].
        assert_eq!(pairs[0], (PathBuf::from("src/a.rs"), vec![1.0_f32]));
        assert_eq!(pairs[1], (PathBuf::from("src/b.rs"), vec![2.0_f32]));
        assert_eq!(pairs[2], (PathBuf::from("src/c.rs"), vec![3.0_f32]));
    }

    /// Verify that `build_corpus_index` with an empty corpus slice calls
    /// `insert_batch` with an empty slice and does NOT call `embed_batch`.
    #[test]
    fn test_build_corpus_index_with_empty_corpus_calls_insert_batch_with_empty_slice() {
        let embed = MockMockEmbeddingPort::new();
        // embed_batch must NOT be called.

        let mut index = MockMockSemanticIndexPort::new();
        index.expect_insert_batch().times(1).withf(|items| items.is_empty()).returning(|_| Ok(()));

        build_corpus_index(vec![], &embed as &dyn EmbeddingPort, &index as &dyn SemanticIndexPort)
            .unwrap();
    }

    /// Verify that an `EmbeddingError` from `embed_batch` is propagated as
    /// `DryCheckCycleError::Embedding`.
    #[test]
    fn test_build_corpus_index_propagates_embed_batch_error() {
        let frag = make_fragment("src/a.rs", "fn a() {}");

        let mut embed = MockMockEmbeddingPort::new();
        embed.expect_embed_batch().returning(|_| {
            Err(EmbeddingError::InferenceFailed { source: "batch inference failed".to_owned() })
        });

        let index = MockMockSemanticIndexPort::new();

        let result = build_corpus_index(
            vec![frag],
            &embed as &dyn EmbeddingPort,
            &index as &dyn SemanticIndexPort,
        );

        assert!(
            matches!(result, Err(DryCheckCycleError::Embedding(_))),
            "embed_batch error must be propagated as DryCheckCycleError::Embedding"
        );
    }

    /// Verify that mismatched batch output fails instead of partially indexing
    /// the corpus via `zip` truncation.
    #[test]
    fn test_build_corpus_index_rejects_embed_batch_length_mismatch() {
        let frag_a = make_fragment("src/a.rs", "fn a() {}");
        let frag_b = make_fragment("src/b.rs", "fn b() {}");

        let mut embed = MockMockEmbeddingPort::new();
        embed.expect_embed_batch().returning(|_| Ok(vec![vec![1.0_f32]]));

        let mut index = MockMockSemanticIndexPort::new();
        index.expect_insert_batch().never();

        let result = build_corpus_index(
            vec![frag_a, frag_b],
            &embed as &dyn EmbeddingPort,
            &index as &dyn SemanticIndexPort,
        );

        assert!(
            matches!(result, Err(DryCheckCycleError::Embedding(_))),
            "embed_batch length mismatch must fail before inserting into the index"
        );
    }
}
