//! `dup-index measure-quality` subcommand — input DTO and [`crate::CliApp`] impl.

use std::path::PathBuf;
use std::sync::Arc;

use infrastructure::semantic_dup::{
    embedding::FastEmbedAdapter, extractor::extract_code_fragments,
};
use usecase::semantic_dup::{
    MeasureQualityCommand, MeasureQualityInteractor, MeasureQualityService as _,
};

use super::SemanticDupCompositionRoot;
use crate::{CommandOutcome, error::CompositionError};

// Re-export shim: implementation relocated to `libs/infrastructure` per ADR 1328 D7.
pub(crate) use infrastructure::semantic_dup::noop_adapter::NoopSemanticIndexPort;

/// Input DTO for `sotp dup-index measure-quality`.
#[derive(Debug, Clone)]
pub struct DupIndexMeasureQualityInput {
    /// Root of the workspace to scan for Rust sources.
    pub workspace_root: PathBuf,
}

impl SemanticDupCompositionRoot {
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
    ) -> Result<CommandOutcome, CompositionError> {
        let fragments = extract_code_fragments(&input.workspace_root).map_err(|e| {
            CompositionError::Infrastructure(format!("fragment extraction failed: {e}"))
        })?;

        let embedding_port = Arc::new(FastEmbedAdapter::new().map_err(|e| {
            CompositionError::AdapterInit(format!("failed to load embedding model: {e}"))
        })?);
        let index_port = Arc::new(NoopSemanticIndexPort);

        let interactor = MeasureQualityInteractor::new(embedding_port, index_port);
        let metrics = interactor
            .measure_quality(&MeasureQualityCommand { fragments })
            .map_err(|e| CompositionError::Usecase(format!("measure-quality failed: {e}")))?;

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
        .map_err(|e| {
            CompositionError::Infrastructure(format!("failed to serialize metrics to JSON: {e}"))
        })?;

        Ok(CommandOutcome::success(Some(json)))
    }
}
