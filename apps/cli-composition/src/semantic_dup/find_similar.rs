//! `find-similar` subcommand — input DTO and [`crate::CliApp`] impl.

use std::path::PathBuf;
use std::sync::Arc;

use domain::semantic_dup::{CodeFragment, TopK};
use infrastructure::semantic_dup::{
    embedding::FastEmbedAdapter, index::LanceDbSemanticIndexAdapter,
};
use usecase::semantic_dup::{FindSimilarCommand, FindSimilarInteractor, FindSimilarService as _};

use crate::{CliApp, CommandOutcome};

use super::common::{LANCEDB_TABLE_MARKER, is_recognizable_lancedb_index, truncate_snippet};

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
    /// CN-05: information-only — never blocks commits or code additions.  An
    /// empty result set means nothing similar was found and exits 0 normally.
    /// However, a **missing or unrecognizable index** is an operational error:
    /// the command returns `Err` instead of silently reporting "no results"
    /// (which would be misleading — "nothing found" and "index absent" are
    /// distinct situations).
    ///
    /// # Errors
    ///
    /// Returns `Err` if:
    /// - `input.db_path` is absent or does not contain the `fragments.lance/`
    ///   marker (i.e. is not a recognizable LanceDB index).
    /// - Adapter construction or the interactor call fails.
    pub fn semantic_dup_find_similar(
        &self,
        input: FindSimilarInput,
    ) -> Result<CommandOutcome, String> {
        let top_k = TopK::new(input.top_k).map_err(|e| format!("invalid --top-k value: {e}"))?;

        let fragment = CodeFragment::new(PathBuf::from("<query>"), input.fragment_text.clone())
            .map_err(|e| format!("invalid query fragment: {e}"))?;

        // Validate that the semantic index exists and is recognizable BEFORE
        // constructing the embedding or index adapters.  An absent or typo'd
        // --db-path would otherwise be treated as an empty index (no
        // `fragments` table → no search results → exit 0 "no results found"),
        // silently hiding the misconfiguration.  A missing index is an
        // operational error, distinct from a query that legitimately found
        // nothing.
        if !is_recognizable_lancedb_index(&input.db_path) {
            return Err(format!(
                "find-similar: no semantic index found at '{}' (missing '{}' marker); \
                 run `sotp dup-index build` first",
                input.db_path.display(),
                LANCEDB_TABLE_MARKER,
            ));
        }

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

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    // ── Missing-index guard (P1 fail-open fix) ────────────────────────────────

    #[test]
    fn test_find_similar_with_nonexistent_db_path_returns_err_missing_index() {
        // P1 fix: a valid top_k + valid fragment_text + a non-existent db_path
        // must produce Err (from the index guard), not a silent "no results found".
        let dir = tempfile::tempdir().unwrap();
        // db_path points to a path that does not exist at all.
        let db_path = dir.path().join("nonexistent_index.db");
        assert!(!db_path.exists(), "pre-condition: db_path must not exist");

        let app = crate::CliApp;
        let input =
            FindSimilarInput { fragment_text: "fn guard_test() {}".to_owned(), top_k: 5, db_path };

        let result = app.semantic_dup_find_similar(input);
        assert!(
            result.is_err(),
            "find-similar must return Err when db_path does not exist, got: {:?}",
            result.ok()
        );
        let msg = result.unwrap_err();
        assert!(
            msg.contains("dup-index build"),
            "error message must reference 'dup-index build', got: {msg}"
        );
        assert!(msg.contains("missing"), "error message must mention 'missing', got: {msg}");
    }

    #[test]
    fn test_find_similar_with_dir_without_marker_returns_err_missing_index() {
        // P1 fix: a db_path that exists as a directory but does NOT contain the
        // `fragments.lance/` marker must produce Err, not a silent "no results found".
        let dir = tempfile::tempdir().unwrap();
        // Create a directory at db_path WITHOUT the LanceDB marker.
        let db_path = dir.path().join("not_an_index");
        std::fs::create_dir_all(&db_path).unwrap();
        std::fs::write(db_path.join("some_other_file.txt"), "unrelated").unwrap();
        assert!(db_path.exists(), "pre-condition: db_path dir must exist");
        assert!(
            !db_path.join(super::super::common::LANCEDB_TABLE_MARKER).exists(),
            "pre-condition: marker must be absent"
        );

        let app = crate::CliApp;
        let input = FindSimilarInput {
            fragment_text: "fn guard_test_dir() {}".to_owned(),
            top_k: 5,
            db_path: db_path.clone(),
        };

        let result = app.semantic_dup_find_similar(input);
        assert!(
            result.is_err(),
            "find-similar must return Err when db_path exists but has no LanceDB marker, got: {:?}",
            result.ok()
        );
        let msg = result.unwrap_err();
        assert!(
            msg.contains("dup-index build"),
            "error message must reference 'dup-index build', got: {msg}"
        );
        // The directory must be completely untouched (no empty DB created).
        assert!(db_path.exists(), "db_path directory must not be deleted on Err");
        assert!(
            db_path.join("some_other_file.txt").exists(),
            "unrelated file inside db_path must be preserved"
        );
    }
}
