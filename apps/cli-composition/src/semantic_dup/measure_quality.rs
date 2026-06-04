//! `dup-index measure-quality` subcommand — input DTO and [`crate::CliApp`] impl.

use std::path::PathBuf;
use std::sync::Arc;

use domain::semantic_dup::{CodeFragment, SimilarFragment, TopK};
use infrastructure::semantic_dup::{
    embedding::FastEmbedAdapter, extractor::extract_code_fragments,
};
use usecase::semantic_dup::{
    MeasureQualityCommand, MeasureQualityInteractor, MeasureQualityService as _,
    SemanticIndexError, SemanticIndexPort,
};

use crate::{CliApp, CommandOutcome};

/// No-op implementation of [`SemanticIndexPort`] for use by
/// [`MeasureQualityInteractor`], which only computes embedding metrics and
/// never reads from or writes to an index.
///
/// Using a no-op port removes the spurious dependency on LanceDB state /
/// filesystem permissions that would otherwise be required by the real adapter.
struct NoopSemanticIndexPort;

impl SemanticIndexPort for NoopSemanticIndexPort {
    fn insert(
        &self,
        _fragment: &CodeFragment,
        _embedding: &[f32],
    ) -> Result<(), SemanticIndexError> {
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

/// Input DTO for `sotp dup-index measure-quality`.
#[derive(Debug, Clone)]
pub struct DupIndexMeasureQualityInput {
    /// Root of the workspace to scan for Rust sources.
    pub workspace_root: PathBuf,
}

impl CliApp {
    /// Run `sotp dup-index measure-quality`: compute embedding model quality
    /// metrics over workspace fragments and output JSON to stdout (AC-03).
    ///
    /// The index port is not used by [`MeasureQualityInteractor`] (metrics are
    /// computed from embeddings alone, not index lookups), so a no-op port is
    /// supplied here — avoiding a spurious dependency on LanceDB state or
    /// filesystem permissions.
    ///
    /// # Errors
    ///
    /// Returns `Err` if extraction, embedding adapter construction, or the
    /// interactor call fails.
    pub fn semantic_dup_index_measure_quality(
        &self,
        input: DupIndexMeasureQualityInput,
    ) -> Result<CommandOutcome, String> {
        let fragments = extract_code_fragments(&input.workspace_root)
            .map_err(|e| format!("fragment extraction failed: {e}"))?;

        let embedding_port = Arc::new(
            FastEmbedAdapter::new().map_err(|e| format!("failed to load embedding model: {e}"))?,
        );
        let index_port = Arc::new(NoopSemanticIndexPort);

        let interactor = MeasureQualityInteractor::new(embedding_port, index_port);
        let metrics = interactor
            .measure_quality(&MeasureQualityCommand { fragments })
            .map_err(|e| format!("measure-quality failed: {e}"))?;

        let p = &metrics.cosine_percentiles;
        let json = serde_json::to_string_pretty(&serde_json::json!({
            "mean_cosine": metrics.mean_cosine,
            "cosine_std_dev": metrics.cosine_std_dev,
            "cosine_percentiles": {
                "p10": p.first().copied().unwrap_or(0.0),
                "p25": p.get(1).copied().unwrap_or(0.0),
                "p50": p.get(2).copied().unwrap_or(0.0),
                "p75": p.get(3).copied().unwrap_or(0.0),
                "p90": p.get(4).copied().unwrap_or(0.0),
                "p95": p.get(5).copied().unwrap_or(0.0),
                "p99": p.get(6).copied().unwrap_or(0.0),
            },
            "above_threshold_rate": metrics.above_threshold_rate,
        }))
        .map_err(|e| format!("failed to serialize metrics to JSON: {e}"))?;

        Ok(CommandOutcome::success(Some(json)))
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use std::path::PathBuf;

    use domain::semantic_dup::TopK;
    use usecase::semantic_dup::SemanticIndexPort;

    use super::*;

    // ── NoopSemanticIndexPort ─────────────────────────────────────────────────

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
}
