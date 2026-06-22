//! No-op adapter for [`usecase::semantic_dup::SemanticIndexPort`].
//!
//! Relocated from `cli_composition::semantic_dup::measure_quality` per ADR 1328 D7.
//!
//! [`NoopSemanticIndexPort`] is a null-object implementation of [`SemanticIndexPort`] for use
//! by [`usecase::semantic_dup::MeasureQualityInteractor`], which only computes embedding metrics
//! and never reads from or writes to an index.

use std::path::Path;

use domain::semantic_dup::{CodeFragment, SimilarFragment, TopK};
use usecase::semantic_dup::{SemanticIndexError, SemanticIndexPort};

/// No-op implementation of [`SemanticIndexPort`].
///
/// All write operations return `Ok(())` and `search` returns an empty [`Vec`].
/// Using a no-op port removes the spurious dependency on LanceDB state /
/// filesystem permissions that would otherwise be required by the real adapter.
pub struct NoopSemanticIndexPort;

impl SemanticIndexPort for NoopSemanticIndexPort {
    fn insert(
        &self,
        _fragment: &CodeFragment,
        _embedding: &[f32],
    ) -> Result<(), SemanticIndexError> {
        Ok(())
    }

    fn insert_batch(&self, _items: &[(CodeFragment, Vec<f32>)]) -> Result<(), SemanticIndexError> {
        Ok(())
    }

    fn delete_by_source_path(&self, _source_path: &Path) -> Result<(), SemanticIndexError> {
        Ok(())
    }

    fn search(
        &self,
        _embedding: &[f32],
        _top_k: TopK,
    ) -> Result<Vec<SimilarFragment>, SemanticIndexError> {
        Ok(Vec::new())
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use std::path::PathBuf;

    use domain::semantic_dup::{CodeFragment, TopK};
    use usecase::semantic_dup::SemanticIndexPort as _;

    use super::NoopSemanticIndexPort;

    #[test]
    fn test_noop_semantic_index_port_insert_returns_ok() {
        let port = NoopSemanticIndexPort;
        let fragment =
            CodeFragment::new(PathBuf::from("src/lib.rs"), "fn foo() {}".to_owned(), 1, 1)
                .expect("valid fragment");
        let embedding = vec![0.1_f32, 0.2, 0.3];
        let result = port.insert(&fragment, &embedding);
        assert!(result.is_ok(), "NoopSemanticIndexPort::insert must always return Ok");
    }

    #[test]
    fn test_noop_semantic_index_port_search_returns_empty_vec() {
        let port = NoopSemanticIndexPort;
        let embedding = vec![0.1_f32, 0.2, 0.3];
        let top_k = TopK::new(5).expect("valid top_k");
        let result = port.search(&embedding, top_k);
        assert!(result.is_ok(), "NoopSemanticIndexPort::search must return Ok");
        assert!(
            result.unwrap().is_empty(),
            "NoopSemanticIndexPort::search must return an empty Vec"
        );
    }

    #[test]
    fn test_noop_semantic_index_port_delete_by_source_path_returns_ok() {
        let port = NoopSemanticIndexPort;
        let result = port.delete_by_source_path(std::path::Path::new("src/lib.rs"));
        assert!(result.is_ok(), "NoopSemanticIndexPort::delete_by_source_path must return Ok");
    }
}
