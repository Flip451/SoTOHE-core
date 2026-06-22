//! `find-similar` subcommand — input DTO and [`crate::CliApp`] impl.

use std::path::PathBuf;
use std::sync::Arc;

use domain::semantic_dup::{CodeFragment, SimilarFragment, TopK};
use infrastructure::semantic_dup::{
    embedding::FastEmbedAdapter, index::LanceDbSemanticIndexAdapter,
};
use usecase::semantic_dup::{FindSimilarCommand, FindSimilarInteractor, FindSimilarService as _};

use super::SemanticDupCompositionRoot;
use crate::{CommandOutcome, error::CompositionError};

use super::common::{LANCEDB_TABLE_MARKER, is_recognizable_lancedb_index};

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

/// Format a slice of [`SimilarFragment`]s into a human-readable, non-lossy
/// string for display.
///
/// Each result is rendered as a numbered block:
/// ```text
/// # 1  path/to/file.rs  (score: 0.9123)
/// <full fragment body, verbatim>
///
/// # 2  path/to/other.rs  (score: 0.8456)
/// <full fragment body, verbatim>
/// ```
///
/// Results are separated by a blank line.  The full fragment body is printed
/// verbatim — no truncation is applied.
fn format_find_similar_results(results: &[SimilarFragment]) -> String {
    results
        .iter()
        .enumerate()
        .map(|(i, sf)| {
            format!(
                "# {}  {}  (score: {:.4})\n{}",
                i + 1,
                sf.fragment.source_path.display(),
                sf.score.value(),
                sf.fragment.content(),
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

impl SemanticDupCompositionRoot {
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
    /// Each result is printed as a numbered block containing the source file
    /// path, similarity score, and the **full fragment body** (verbatim, not
    /// truncated) so the user can inspect the matched implementation without
    /// opening the file separately.
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
    ) -> Result<CommandOutcome, CompositionError> {
        let top_k = TopK::new(input.top_k)
            .map_err(|e| CompositionError::WiringFailed(format!("invalid --top-k value: {e}")))?;

        // Query fragments use start_line=1 / end_line=u32::MAX as a sentinel
        // so they are never excluded by hunk-level filtering.
        let fragment = CodeFragment::new(
            PathBuf::from("<query>"),
            input.fragment_text.clone(),
            1,
            u32::MAX,
        )
        .map_err(|e| CompositionError::WiringFailed(format!("invalid query fragment: {e}")))?;

        // Validate that the semantic index exists and is recognizable BEFORE
        // constructing the embedding or index adapters.  An absent or typo'd
        // --db-path would otherwise be treated as an empty index (no
        // `fragments` table → no search results → exit 0 "no results found"),
        // silently hiding the misconfiguration.  A missing index is an
        // operational error, distinct from a query that legitimately found
        // nothing.
        if !is_recognizable_lancedb_index(&input.db_path) {
            return Err(CompositionError::WiringFailed(format!(
                "find-similar: no semantic index found at '{}' (missing '{}' marker); \
                 run `sotp dup-index build` first",
                input.db_path.display(),
                LANCEDB_TABLE_MARKER,
            )));
        }

        let embedding_port = Arc::new(FastEmbedAdapter::new().map_err(|e| {
            CompositionError::AdapterInit(format!("failed to load embedding model: {e}"))
        })?);
        let index_port =
            Arc::new(LanceDbSemanticIndexAdapter::new(input.db_path.clone()).map_err(|e| {
                CompositionError::AdapterInit(format!(
                    "failed to open index at {}: {e}",
                    input.db_path.display()
                ))
            })?);

        let interactor = FindSimilarInteractor::new(embedding_port, index_port);
        let output = interactor
            .find_similar(&FindSimilarCommand { fragment, top_k })
            .map_err(|e| CompositionError::Usecase(format!("find-similar failed: {e}")))?;

        if output.results.is_empty() {
            return Ok(CommandOutcome::success(Some("(no results found)".to_owned())));
        }

        Ok(CommandOutcome::success(Some(format_find_similar_results(&output.results))))
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use std::path::PathBuf;

    use domain::semantic_dup::{CodeFragment, SimilarFragment, SimilarityScore};

    use super::*;

    // ── format_find_similar_results (pure formatter) ──────────────────────────

    fn make_fragment(path: &str, content: &str) -> CodeFragment {
        CodeFragment::new(PathBuf::from(path), content.to_owned(), 1, 1).unwrap()
    }

    fn make_score(v: f32) -> SimilarityScore {
        SimilarityScore::new(v).unwrap()
    }

    #[test]
    fn test_format_find_similar_results_empty_slice_returns_empty_string() {
        let output = format_find_similar_results(&[]);
        assert_eq!(output, "");
    }

    #[test]
    fn test_format_find_similar_results_single_result_contains_path_score_and_full_body() {
        let body = "fn hello() {\n    println!(\"hello\");\n}";
        let sf = SimilarFragment {
            fragment: make_fragment("src/hello.rs", body),
            score: make_score(0.9123),
        };
        let output = format_find_similar_results(&[sf]);

        // Header line must contain the path and score.
        assert!(output.contains("src/hello.rs"), "output must contain the source path");
        assert!(output.contains("0.9123"), "output must contain the score");
        // The FULL body must appear verbatim — including lines beyond the first.
        assert!(output.contains("fn hello() {"), "output must contain the first line of the body");
        assert!(
            output.contains("    println!(\"hello\");"),
            "output must contain the second line (non-lossy)"
        );
        assert!(output.contains('}'), "output must contain the closing brace");
        // Must not be truncated: body has more than 80 chars total? not needed here,
        // but body is multi-line and the second line must be present.
    }

    #[test]
    fn test_format_find_similar_results_multiline_body_shown_in_full_not_truncated() {
        // Build a fragment whose first line is short but whose full body is
        // much longer than 80 characters (the old truncation limit).
        let long_line = "x".repeat(200);
        let body = format!("fn long_body() {{\n    let x = \"{long_line}\";\n}}");
        let sf = SimilarFragment {
            fragment: make_fragment("src/long.rs", &body),
            score: make_score(0.7500),
        };
        let output = format_find_similar_results(&[sf]);

        // The 200-char string must appear in full.
        assert!(
            output.contains(&long_line),
            "output must contain the full 200-char string (non-lossy)"
        );
        assert!(!output.contains('…'), "output must NOT contain a truncation ellipsis");
    }

    #[test]
    fn test_format_find_similar_results_multiple_results_numbered_and_separated() {
        let sf1 = SimilarFragment {
            fragment: make_fragment("src/a.rs", "fn alpha() {}"),
            score: make_score(0.9500),
        };
        let sf2 = SimilarFragment {
            fragment: make_fragment("src/b.rs", "fn beta() {}"),
            score: make_score(0.8000),
        };
        let output = format_find_similar_results(&[sf1, sf2]);

        // Both paths must appear.
        assert!(output.contains("src/a.rs"), "output must contain path of result 1");
        assert!(output.contains("src/b.rs"), "output must contain path of result 2");
        // Index markers.
        assert!(output.contains("# 1"), "output must contain '# 1' index marker");
        assert!(output.contains("# 2"), "output must contain '# 2' index marker");
        // Both bodies must appear.
        assert!(output.contains("fn alpha() {}"), "output must contain body of result 1");
        assert!(output.contains("fn beta() {}"), "output must contain body of result 2");
        // Results must be separated by a blank line.
        assert!(output.contains("\n\n"), "results must be separated by a blank line");
    }

    #[test]
    fn test_format_find_similar_results_index_starts_at_1() {
        let sf = SimilarFragment {
            fragment: make_fragment("src/x.rs", "fn x() {}"),
            score: make_score(0.5),
        };
        let output = format_find_similar_results(&[sf]);
        assert!(output.starts_with("# 1"), "first result must start with '# 1'");
    }

    // ── Missing-index guard (P1 fail-open fix) ────────────────────────────────

    #[test]
    fn test_find_similar_with_nonexistent_db_path_returns_err_missing_index() {
        // P1 fix: a valid top_k + valid fragment_text + a non-existent db_path
        // must produce Err (from the index guard), not a silent "no results found".
        let dir = tempfile::tempdir().unwrap();
        // db_path points to a path that does not exist at all.
        let db_path = dir.path().join("nonexistent_index.db");
        assert!(!db_path.exists(), "pre-condition: db_path must not exist");

        let app = crate::semantic_dup::SemanticDupCompositionRoot::new();
        let input =
            FindSimilarInput { fragment_text: "fn guard_test() {}".to_owned(), top_k: 5, db_path };

        let result = app.semantic_dup_find_similar(input);
        assert!(
            result.is_err(),
            "find-similar must return Err when db_path does not exist, got: {:?}",
            result.ok()
        );
        let msg = result.unwrap_err().to_string();
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

        let app = crate::semantic_dup::SemanticDupCompositionRoot::new();
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
        let msg = result.unwrap_err().to_string();
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
