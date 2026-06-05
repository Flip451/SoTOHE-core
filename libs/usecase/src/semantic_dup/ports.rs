//! Secondary ports for the semantic duplicate detection use case.
//!
//! Ports are placed here (not in domain) because embedding and vector-index
//! capabilities are infrastructure concerns — the domain carries no concept of
//! ML inference or ANN search. Analogous to `ReviewHasher`.

use domain::semantic_dup::{CodeFragment, SimilarFragment, TopK};

use super::errors::{EmbeddingError, SemanticIndexError};

// ── Secondary ports ───────────────────────────────────────────────────────────

/// Secondary port for embedding computation.
///
/// Abstracts fastembed-rs / ONNX Runtime from use-case logic. Placed in
/// usecase (not domain) because embedding is an infrastructure capability —
/// the domain carries no concept of ML inference. Analogous to `ReviewHasher`.
pub trait EmbeddingPort: Send + Sync {
    /// Compute an embedding vector for the given code fragment.
    ///
    /// # Errors
    ///
    /// Returns [`EmbeddingError::ModelLoadFailed`] if the model is not yet
    /// loaded or fails to initialise.
    /// Returns [`EmbeddingError::InferenceFailed`] if inference fails.
    fn embed(&self, fragment: &CodeFragment) -> Result<Vec<f32>, EmbeddingError>;

    /// Compute embedding vectors for a batch of code fragments in a single
    /// model inference call.
    ///
    /// Returns one embedding per fragment, in the same order as `fragments`.
    /// An empty input slice returns `Ok(vec![])`.
    ///
    /// # Errors
    ///
    /// Returns [`EmbeddingError::ModelLoadFailed`] if the model is not yet
    /// loaded or fails to initialise.
    /// Returns [`EmbeddingError::InferenceFailed`] if inference fails.
    fn embed_batch(&self, fragments: &[CodeFragment]) -> Result<Vec<Vec<f32>>, EmbeddingError>;
}

/// Secondary port for the local semantic vector index.
///
/// Abstracts LanceDB from use-case logic. Placed in usecase (not domain)
/// because vector indexing is an infrastructure capability with no domain
/// entity semantics. Analogous to `ReviewHasher`.
pub trait SemanticIndexPort: Send + Sync {
    /// Insert a fragment and its embedding vector into the index.
    ///
    /// # Errors
    ///
    /// Returns [`SemanticIndexError::InsertFailed`] if the insert operation fails.
    fn insert(&self, fragment: &CodeFragment, embedding: &[f32]) -> Result<(), SemanticIndexError>;

    /// Insert a batch of (fragment, embedding) pairs into the index in a single
    /// transaction, eliminating per-fragment transaction overhead for large corpora.
    ///
    /// An empty slice is a no-op (returns `Ok(())`). All embeddings in the batch
    /// must have the same dimension; the dimension is inferred from the first item.
    ///
    /// # Errors
    ///
    /// Returns [`SemanticIndexError::InsertFailed`] if the batch insert operation
    /// fails or if any fragment's `source_path` is not valid UTF-8.
    fn insert_batch(&self, items: &[(CodeFragment, Vec<f32>)]) -> Result<(), SemanticIndexError>;

    /// Delete all fragments whose `source_path` equals the given path from the index.
    ///
    /// Used for incremental index maintenance (D7/IN-10): before re-indexing a
    /// changed file, remove all previously-stored fragments for that file so that
    /// stale entries do not accumulate.
    ///
    /// If the table does not yet exist (first-run empty index), returns `Ok(())`
    /// without error (idempotent).  If no rows match `source_path`, returns
    /// `Ok(())` (idempotent).
    ///
    /// `source_path` must be valid UTF-8; a non-UTF-8 path returns
    /// [`SemanticIndexError::DeleteFailed`].
    ///
    /// # Errors
    ///
    /// Returns [`SemanticIndexError::DeleteFailed`] if the delete operation fails
    /// or if `source_path` is not valid UTF-8.
    fn delete_by_source_path(
        &self,
        source_path: &std::path::Path,
    ) -> Result<(), SemanticIndexError>;

    /// Search the index for the top-k fragments nearest to `embedding`.
    ///
    /// # Errors
    ///
    /// Returns [`SemanticIndexError::SearchFailed`] if the search operation fails.
    fn search(
        &self,
        embedding: &[f32],
        top_k: TopK,
    ) -> Result<Vec<SimilarFragment>, SemanticIndexError>;
}
