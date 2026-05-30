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
