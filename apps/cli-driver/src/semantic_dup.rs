// STAGED FOR T021 — not yet compiled; Cargo.toml + workspace member added atomically in T021 per CN-06.
//
//! `semantic_dup` command family — primary adapter driver.
//!
//! `SemanticDupDriver` holds injected use-case interactors and exposes
//! `handle(input) -> CommandOutcome`.  The render helper here mirrors the
//! JSON assembly in
//! `apps/cli-composition/src/semantic_dup/measure_quality.rs` (lines 92-106
//! `dup_index_measure_quality` JSON output);
//! T021 removes the `cli_composition` duplicate when the live path is flipped.

// TODO(T021): add use-case + infrastructure imports once Cargo.toml is materialized.
// use std::path::PathBuf;
// use std::sync::Arc;
// use infrastructure::semantic_dup::{
//     embedding::FastEmbedAdapter, extractor::extract_code_fragments,
// };
// use infrastructure::semantic_dup::noop_adapter::NoopSemanticIndexPort;
// use usecase::semantic_dup::{
//     MeasureQualityCommand, MeasureQualityInteractor, MeasureQualityService as _,
// };

use std::path::PathBuf;

use crate::render::CommandOutcome;

// ---------------------------------------------------------------------------
// Input type
// ---------------------------------------------------------------------------

/// Typed input for the `semantic_dup` command family.
pub enum SemanticDupInput {
    /// Run the semantic similarity search for a given fragment.
    FindSimilar {
        /// Inline fragment text to search for.
        fragment_text: String,
        /// Number of top-k similar fragments to return.
        top_k: usize,
        /// Path to the local LanceDB semantic index database.
        db_path: PathBuf,
    },
    /// Build (or rebuild) the semantic index from workspace Rust sources.
    IndexBuild {
        /// Workspace root to scan for `*.rs` source files.
        workspace_root: PathBuf,
        /// Path to the local LanceDB semantic index database.
        db_path: PathBuf,
    },
    /// Measure embedding quality metrics over workspace fragments (JSON output).
    IndexMeasureQuality {
        /// Root of the workspace to scan for Rust sources.
        workspace_root: PathBuf,
    },
    /// Soft gate: check fragments for near-duplicates in the semantic index.
    DupCheck {
        /// List of fragment file paths to check.
        fragment_files: Vec<PathBuf>,
        /// Cosine similarity threshold (0.0–1.0) above which a match is flagged.
        threshold: f32,
        /// Path to the local LanceDB semantic index database.
        db_path: PathBuf,
        /// Optional path to the acknowledgement file (newline-separated hash list).
        ack_file: Option<PathBuf>,
        /// Whether to acknowledge all warnings from this run.
        ack: bool,
    },
}

// ---------------------------------------------------------------------------
// Driver
// ---------------------------------------------------------------------------

/// Primary adapter driver for the `semantic_dup` command family.
///
/// Holds injected use-case interactors; exposes `handle(input) -> CommandOutcome`.
pub struct SemanticDupDriver {
    // TODO(T021): inject use-case interactors here once the crate dependency
    // graph is materialized.
}

impl SemanticDupDriver {
    /// Create a new `SemanticDupDriver`.
    ///
    /// TODO(T021): accept injected interactors as parameters once the crate
    /// dependency graph is materialized.
    pub fn new() -> Self {
        Self {}
    }

    /// Handle a semantic_dup command.
    ///
    /// TODO(T021): wire real use-case invocation once Cargo.toml is materialized.
    pub fn handle(&self, input: SemanticDupInput) -> CommandOutcome {
        match input {
            SemanticDupInput::FindSimilar { fragment_text, top_k, db_path } => {
                self.find_similar(fragment_text, top_k, db_path)
            }
            SemanticDupInput::IndexBuild { workspace_root, db_path } => {
                self.index_build(workspace_root, db_path)
            }
            SemanticDupInput::IndexMeasureQuality { workspace_root } => {
                self.index_measure_quality(workspace_root)
            }
            SemanticDupInput::DupCheck { fragment_files, threshold, db_path, ack_file, ack } => {
                self.dup_check(fragment_files, threshold, db_path, ack_file, ack)
            }
        }
    }

    // -----------------------------------------------------------------------
    // Render helpers (logic duplicated from
    // cli_composition/src/semantic_dup/measure_quality.rs lines 92-106;
    // T021 removes the cli_composition copies).
    // -----------------------------------------------------------------------

    fn find_similar(
        &self,
        _fragment_text: String,
        _top_k: usize,
        _db_path: PathBuf,
    ) -> CommandOutcome {
        // TODO(T021): build FindSimilarInput, invoke SemanticDupCompositionRoot,
        // propagate CompositionError to exit-code 1.
        // Mirrors cli_composition/src/semantic_dup/find_similar.rs.
        CommandOutcome::success(None)
    }

    fn index_build(&self, _workspace_root: PathBuf, _db_path: PathBuf) -> CommandOutcome {
        // TODO(T021): extract_code_fragments, open LanceDB via FastEmbedAdapter,
        // invoke build interactor.
        // Mirrors cli_composition/src/semantic_dup/build.rs.
        CommandOutcome::success(None)
    }

    fn index_measure_quality(&self, _workspace_root: PathBuf) -> CommandOutcome {
        // TODO(T021): extract fragments, build FastEmbedAdapter + NoopSemanticIndexPort,
        // invoke MeasureQualityInteractor, then format output via format_measure_quality_json.
        // Mirrors cli_composition/src/semantic_dup/measure_quality.rs lines 39-77.
        CommandOutcome::success(None)
    }

    fn dup_check(
        &self,
        _fragment_files: Vec<PathBuf>,
        _threshold: f32,
        _db_path: PathBuf,
        _ack_file: Option<PathBuf>,
        _ack: bool,
    ) -> CommandOutcome {
        // TODO(T021): build DupCheckInput, invoke SemanticDupCompositionRoot::semantic_dup_check.
        // Mirrors cli_composition/src/semantic_dup/check.rs.
        CommandOutcome::success(None)
    }
}

impl Default for SemanticDupDriver {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Render helpers (duplicated from
// cli_composition/src/semantic_dup/measure_quality.rs lines 92-106;
// T021 removes the cli_composition copies and moves these to cli_driver::render).
// ---------------------------------------------------------------------------

/// Format `sotp dup-index measure-quality` output as a JSON string.
///
/// Mirrors the JSON assembly in
/// `cli_composition::semantic_dup::measure_quality::SemanticDupCompositionRoot::semantic_dup_index_measure_quality`
/// (measure_quality.rs lines 57-77 — the `serde_json::json!` block and
/// `serde_json::to_string_pretty` call).
///
/// TODO(T021): wire real `usecase::semantic_dup::MeasureQualityMetrics` once
/// the dependency graph is materialized.
/// Currently returns a placeholder so the staged file is self-consistent.
///
/// # Errors
///
/// Returns a human-readable error string on serialization failure.
#[allow(dead_code)]
fn format_measure_quality_json(
    // TODO(T021): replace with real `usecase::semantic_dup::MeasureQualityMetrics`.
    _mean_cosine: f64,
    _cosine_std_dev: f64,
    _cosine_percentiles: &[f64],
    _above_threshold_rate: f64,
) -> Result<String, String> {
    // TODO(T021): paste the JSON assembly from
    // cli_composition/src/semantic_dup/measure_quality.rs lines 57-77 once
    // `serde_json` and the metrics type are available as dependencies.
    // The implementation:
    //
    //   let p = cosine_percentiles;
    //   let json = serde_json::to_string_pretty(&serde_json::json!({
    //       "mean_cosine": mean_cosine,
    //       "cosine_std_dev": cosine_std_dev,
    //       "cosine_percentiles": {
    //           "p10": p.first().copied().unwrap_or(0.0),
    //           "p25": p.get(1).copied().unwrap_or(0.0),
    //           "p50": p.get(2).copied().unwrap_or(0.0),
    //           "p75": p.get(3).copied().unwrap_or(0.0),
    //           "p90": p.get(4).copied().unwrap_or(0.0),
    //           "p95": p.get(5).copied().unwrap_or(0.0),
    //           "p99": p.get(6).copied().unwrap_or(0.0),
    //       },
    //       "above_threshold_rate": above_threshold_rate,
    //   }))
    //   .map_err(|e| format!("failed to serialize metrics to JSON: {e}"))
    Ok(String::new())
}
