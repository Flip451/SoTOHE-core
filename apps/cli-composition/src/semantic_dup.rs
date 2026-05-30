//! `semantic_dup` command family — CliApp impl methods and input DTOs.
//!
//! Composition root for all four semantic-dup subcommands:
//! - `find_similar`: embed a query fragment and retrieve top-k results.
//! - `dup_index_build`: extract workspace fragments, embed, and insert into LanceDB.
//! - `dup_check`: check diff fragments against the index (soft gate, always exit 0).
//! - `dup_index_measure_quality`: compute embedding quality metrics over workspace.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use domain::semantic_dup::{CodeFragment, SimilarityThreshold, TopK};
use infrastructure::semantic_dup::{
    embedding::FastEmbedAdapter, extractor::extract_code_fragments,
    index::LanceDbSemanticIndexAdapter,
};
use usecase::semantic_dup::{
    BuildIndexCommand, BuildIndexService as _, DupCheckCommand, DupCheckInteractor,
    DupCheckService as _, FindSimilarCommand, FindSimilarInteractor, FindSimilarService as _,
    MeasureQualityCommand, MeasureQualityInteractor, MeasureQualityService as _,
};

use crate::{CliApp, CommandOutcome};

// ── find-similar ─────────────────────────────────────────────────────────────

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

// ── dup-index build ───────────────────────────────────────────────────────────

/// Input DTO for `sotp dup-index build`.
#[derive(Debug, Clone)]
pub struct DupIndexBuildInput {
    /// Root of the workspace to scan for Rust sources.
    pub workspace_root: PathBuf,
    /// Path to the local LanceDB database.
    pub db_path: PathBuf,
}

impl CliApp {
    /// Run `sotp dup-index build`: extract workspace fragments, embed each,
    /// and insert into the LanceDB index.
    ///
    /// AC-02: exits 0 on success; no network calls at build time (model weights
    /// are loaded from the local fastembed cache).
    ///
    /// # Errors
    ///
    /// Returns `Err` if extraction, adapter construction, or indexing fails.
    pub fn semantic_dup_index_build(
        &self,
        input: DupIndexBuildInput,
    ) -> Result<CommandOutcome, String> {
        let fragments = extract_code_fragments(&input.workspace_root)
            .map_err(|e| format!("fragment extraction failed: {e}"))?;

        let fragment_count = fragments.len();

        let embedding_port = Arc::new(
            FastEmbedAdapter::new().map_err(|e| format!("failed to load embedding model: {e}"))?,
        );
        let index_port =
            Arc::new(LanceDbSemanticIndexAdapter::new(input.db_path.clone()).map_err(|e| {
                format!("failed to open index at {}: {e}", input.db_path.display())
            })?);

        use usecase::semantic_dup::BuildIndexInteractor;
        let interactor = BuildIndexInteractor::new(embedding_port, index_port);
        let output = interactor
            .build_index(&BuildIndexCommand { fragments })
            .map_err(|e| format!("build-index failed: {e}"))?;

        Ok(CommandOutcome::success(Some(format!(
            "Indexed {} fragment(s) (extracted: {fragment_count})",
            output.fragments_indexed
        ))))
    }
}

// ── dup-check ─────────────────────────────────────────────────────────────────

/// Input DTO for `sotp dup-check`.
#[derive(Debug, Clone)]
pub struct DupCheckInput {
    /// List of paths to individual fragment text files (one file per fragment).
    pub fragment_files: Vec<PathBuf>,
    /// Cosine similarity threshold above which a match is flagged (0.0–1.0).
    pub threshold: f32,
    /// Path to the local LanceDB database.
    pub db_path: PathBuf,
    /// Optional path to the ack-set file.  When provided:
    /// - fragments whose content hash already appears in the ack set are
    ///   silently suppressed (AC-05).
    /// - after the run, any new warnings whose fragments the user chose to ack
    ///   (via `--ack`) are written into this file.
    pub ack_file: Option<PathBuf>,
    /// When `true`, all warnings from this run are acked and written to
    /// `ack_file` (AC-05).
    pub ack: bool,
}

/// Read the ack-set (a newline-separated list of SHA-256 hex hashes) from `path`.
///
/// Returns an empty set when the file does not exist yet (first run).
fn read_ack_set(path: &Path) -> Result<std::collections::HashSet<String>, String> {
    match std::fs::read_to_string(path) {
        Ok(contents) => Ok(contents
            .lines()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(std::collections::HashSet::new()),
        Err(e) => Err(format!("cannot read ack file {}: {e}", path.display())),
    }
}

/// Write the ack-set to `path`.
fn write_ack_set(path: &Path, set: &std::collections::HashSet<String>) -> Result<(), String> {
    let mut sorted: Vec<&str> = set.iter().map(String::as_str).collect();
    sorted.sort_unstable();
    let contents = sorted.join("\n") + "\n";
    std::fs::write(path, contents)
        .map_err(|e| format!("cannot write ack file {}: {e}", path.display()))
}

/// Compute a stable content hash for a fragment (SHA-256 of the content bytes).
fn fragment_content_hash(content: &str) -> String {
    // Simple stable hash using std only: we XOR-fold SHA-256 via FNV-1a 64-bit
    // so there are no extra dependencies.  For ack suppression, collision
    // resistance is not critical; what matters is stable reproducibility across
    // runs for the same content.
    //
    // Using FNV-1a 64-bit hash of the UTF-8 content bytes.
    const FNV_OFFSET: u64 = 14_695_981_039_346_656_037;
    const FNV_PRIME: u64 = 1_099_511_628_211;
    let mut hash: u64 = FNV_OFFSET;
    for byte in content.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    format!("{hash:016x}")
}

impl CliApp {
    /// Run `sotp dup-check`: check diff fragments against the semantic index.
    ///
    /// CN-02/AC-04: always exits 0 (soft gate — warnings to stderr, no block).
    /// AC-05: fragments whose content hash appears in the ack file are suppressed.
    ///
    /// # Errors
    ///
    /// Returns `Err` only on hard infrastructure failures (adapter construction
    /// or file I/O errors for the ack file), not on warnings.
    pub fn semantic_dup_check(&self, input: DupCheckInput) -> Result<CommandOutcome, String> {
        // Reject the illegal combination: --ack requires --ack-file to be set.
        if input.ack && input.ack_file.is_none() {
            return Err("--ack requires --ack-file to be specified".to_owned());
        }

        let threshold = SimilarityThreshold::new(input.threshold)
            .map_err(|e| format!("invalid --threshold value: {e}"))?;

        // Read all fragment files.
        let mut fragments: Vec<CodeFragment> = Vec::new();
        for path in &input.fragment_files {
            let content = std::fs::read_to_string(path)
                .map_err(|e| format!("cannot read fragment file {}: {e}", path.display()))?;
            let fragment = CodeFragment::new(path.clone(), content)
                .map_err(|e| format!("invalid fragment in {}: {e}", path.display()))?;
            fragments.push(fragment);
        }

        if fragments.is_empty() {
            return Ok(CommandOutcome::success(Some(
                "dup-check: no fragments to check".to_owned(),
            )));
        }

        // Load the ack set (empty on first run).
        let ack_path_opt = input.ack_file.as_deref();
        let ack_set = match ack_path_opt {
            Some(p) => read_ack_set(p)?,
            None => std::collections::HashSet::new(),
        };

        // Filter out already-acked fragments (AC-05: suppress on re-run).
        let (acked_fragments, check_fragments): (Vec<_>, Vec<_>) =
            fragments.into_iter().partition(|f| {
                let hash = fragment_content_hash(f.content());
                ack_set.contains(&hash)
            });

        let _ = acked_fragments; // suppressed — no warning emitted.

        if check_fragments.is_empty() {
            return Ok(CommandOutcome::success(Some(
                "dup-check: all fragments already acked; no warnings".to_owned(),
            )));
        }

        let embedding_port = Arc::new(
            FastEmbedAdapter::new().map_err(|e| format!("failed to load embedding model: {e}"))?,
        );
        let index_port =
            Arc::new(LanceDbSemanticIndexAdapter::new(input.db_path.clone()).map_err(|e| {
                format!("failed to open index at {}: {e}", input.db_path.display())
            })?);

        let interactor = DupCheckInteractor::new(embedding_port, index_port);
        let output = interactor
            .dup_check(&DupCheckCommand { fragments: check_fragments, threshold })
            .map_err(|e| format!("dup-check failed: {e}"))?;

        // Build stderr warnings string.
        let mut warning_lines: Vec<String> = Vec::new();
        let mut warn_hashes: Vec<String> = Vec::new();

        for warning in &output.warnings {
            warning_lines.push(format!(
                "[dup-check WARNING] fragment '{}' has {} near-duplicate(s):",
                warning.input_fragment.source_path.display(),
                warning.similar_fragments.len()
            ));
            for sf in &warning.similar_fragments {
                let snippet = truncate_snippet(sf.fragment.content(), 60);
                warning_lines.push(format!(
                    "  similar: {} (score={:.4}) | {}",
                    sf.fragment.source_path.display(),
                    sf.score.value(),
                    snippet
                ));
            }
            warn_hashes.push(fragment_content_hash(warning.input_fragment.content()));
        }

        // Handle ack: write acknowledged hashes to the ack file (AC-05).
        if input.ack {
            if let Some(p) = ack_path_opt {
                let mut updated_ack_set = ack_set.clone();
                for h in &warn_hashes {
                    updated_ack_set.insert(h.clone());
                }
                write_ack_set(p, &updated_ack_set)?;
            }
        }

        // Soft gate: warnings go to stderr, exit 0 always (CN-02/AC-04).
        if warning_lines.is_empty() {
            Ok(CommandOutcome::success(Some(
                "dup-check: no near-duplicates found above threshold".to_owned(),
            )))
        } else {
            Ok(CommandOutcome {
                stdout: Some(
                    "dup-check: near-duplicates found (see stderr for details)".to_owned(),
                ),
                stderr: Some(warning_lines.join("\n")),
                exit_code: 0, // soft gate — always exit 0
            })
        }
    }
}

// ── dup-index measure-quality ─────────────────────────────────────────────────

/// Input DTO for `sotp dup-index measure-quality`.
#[derive(Debug, Clone)]
pub struct DupIndexMeasureQualityInput {
    /// Root of the workspace to scan for Rust sources.
    pub workspace_root: PathBuf,
    /// Path to the local LanceDB database (used by the index port in the
    /// interactor even though measure-quality operates in-memory).
    pub db_path: PathBuf,
}

impl CliApp {
    /// Run `sotp dup-index measure-quality`: compute embedding model quality
    /// metrics over workspace fragments and output JSON to stdout (AC-03).
    ///
    /// # Errors
    ///
    /// Returns `Err` if extraction, adapter construction, or the interactor call
    /// fails.
    pub fn semantic_dup_index_measure_quality(
        &self,
        input: DupIndexMeasureQualityInput,
    ) -> Result<CommandOutcome, String> {
        let fragments = extract_code_fragments(&input.workspace_root)
            .map_err(|e| format!("fragment extraction failed: {e}"))?;

        let embedding_port = Arc::new(
            FastEmbedAdapter::new().map_err(|e| format!("failed to load embedding model: {e}"))?,
        );
        let index_port =
            Arc::new(LanceDbSemanticIndexAdapter::new(input.db_path.clone()).map_err(|e| {
                format!("failed to open index at {}: {e}", input.db_path.display())
            })?);

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

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Truncate `s` to at most `max_chars` characters, appending `…` if truncated.
fn truncate_snippet(s: &str, max_chars: usize) -> String {
    let first_line = s.lines().next().unwrap_or("");
    if first_line.chars().count() <= max_chars {
        first_line.to_owned()
    } else {
        let truncated: String = first_line.chars().take(max_chars).collect();
        format!("{truncated}…")
    }
}
