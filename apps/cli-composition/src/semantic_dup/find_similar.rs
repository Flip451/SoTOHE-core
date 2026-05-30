//! `find-similar` subcommand — input DTO and [`crate::CliApp`] impl.

use std::path::PathBuf;
use std::sync::Arc;

use domain::semantic_dup::{CodeFragment, TopK};
use infrastructure::semantic_dup::{
    embedding::FastEmbedAdapter, index::LanceDbSemanticIndexAdapter,
};
use usecase::semantic_dup::{FindSimilarCommand, FindSimilarInteractor, FindSimilarService as _};

use crate::{CliApp, CommandOutcome};

use super::common::truncate_snippet;

/// Input DTO for `sotp find-similar`.
#[derive(Debug, Clone)]
pub struct FindSimilarInput {
    /// The query text fragment, or the content read from a file.
    pub fragment_text: String,
    /// Number of top-k results to return. Default: 5.
    pub top_k: usize,
    /// Path to the local LanceDB database.
    pub db_path: PathBuf,
}

impl CliApp {
    /// Run `sotp find-similar`: embed the query fragment and retrieve top-k
    /// similar entries from the index.
    ///
    /// CN-05: information-only — always exits 0.
    ///
    /// # Errors
    ///
    /// Returns `Err` if adapter construction or the interactor call fails.
    pub fn semantic_dup_find_similar(
        &self,
        input: FindSimilarInput,
    ) -> Result<CommandOutcome, String> {
        let top_k = TopK::new(input.top_k).map_err(|e| format!("invalid --top-k value: {e}"))?;

        let fragment = CodeFragment::new(PathBuf::from("<query>"), input.fragment_text.clone())
            .map_err(|e| format!("invalid query fragment: {e}"))?;

        let embedding_port = Arc::new(
            FastEmbedAdapter::new().map_err(|e| format!("failed to load embedding model: {e}"))?,
        );
        let index_port =
            Arc::new(LanceDbSemanticIndexAdapter::new(input.db_path.clone()).map_err(|e| {
                format!("failed to open index at {}: {e}", input.db_path.display())
            })?);

        let interactor = FindSimilarInteractor::new(embedding_port, index_port);
        let output = interactor
            .find_similar(&FindSimilarCommand { fragment, top_k })
            .map_err(|e| format!("find-similar failed: {e}"))?;

        if output.results.is_empty() {
            return Ok(CommandOutcome::success(Some("(no results found)".to_owned())));
        }

        let mut lines = Vec::new();
        for sf in &output.results {
            let snippet = truncate_snippet(sf.fragment.content(), 80);
            lines.push(format!(
                "{} | {:.4} | {}",
                sf.fragment.source_path.display(),
                sf.score.value(),
                snippet
            ));
        }

        Ok(CommandOutcome::success(Some(lines.join("\n"))))
    }
}
