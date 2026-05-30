//! Command / output DTOs for the semantic duplicate detection use case.

use domain::semantic_dup::{CodeFragment, SimilarFragment, SimilarityThreshold, TopK};

// ── Command / output types ────────────────────────────────────────────────────

/// Query input for find-similar: given a code fragment, retrieve top-k
/// semantically similar fragments from the index.
#[derive(Debug, Clone)]
pub struct FindSimilarCommand {
    /// The query fragment whose similar counterparts are to be retrieved.
    pub fragment: CodeFragment,
    /// How many similar fragments to return at most.
    pub top_k: TopK,
}

/// Output from find-similar: the top-k semantically similar fragments
/// retrieved from the index.
#[derive(Debug, Clone)]
pub struct FindSimilarOutput {
    /// The retrieved similar fragments, ordered by descending similarity score.
    pub results: Vec<SimilarFragment>,
}

/// Query input for dup-check: a list of diff fragments to check against the
/// index at a given similarity threshold.
///
/// Implements CN-03 (diff fragments only).
#[derive(Debug, Clone)]
pub struct DupCheckCommand {
    /// The fragments from the current diff to check for near-duplicates.
    pub fragments: Vec<CodeFragment>,
    /// The cosine similarity threshold above which a match is flagged.
    pub threshold: SimilarityThreshold,
}

/// A single soft-gate warning: one input fragment together with the similar
/// existing fragments that exceed the threshold.
#[derive(Debug, Clone)]
pub struct DupCheckWarning {
    /// The input fragment that triggered the warning.
    pub input_fragment: CodeFragment,
    /// Existing fragments in the index whose similarity to `input_fragment`
    /// exceeds the configured threshold.
    pub similar_fragments: Vec<SimilarFragment>,
}

/// Output from dup-check: all soft-gate warnings for the supplied diff
/// fragments.
///
/// Empty `warnings` means no duplicates were found above the threshold.
#[derive(Debug, Clone)]
pub struct DupCheckOutput {
    /// Warnings for each input fragment that has at least one near-duplicate
    /// above the threshold. Empty if no duplicates were found.
    pub warnings: Vec<DupCheckWarning>,
}

/// Command to build the semantic index.
///
/// Carries pre-extracted [`CodeFragment`]s (extracted by the CLI from workspace
/// Rust sources via the infrastructure-layer extractor, T007), and instructs
/// the interactor to compute embeddings and populate the local LanceDB index.
#[derive(Debug, Clone)]
pub struct BuildIndexCommand {
    /// The pre-extracted code fragments to embed and insert into the index.
    pub fragments: Vec<CodeFragment>,
}

/// Output from build-index: count of code fragments successfully indexed.
#[derive(Debug, Clone)]
pub struct BuildIndexOutput {
    /// The number of fragments that were successfully embedded and indexed.
    pub fragments_indexed: usize,
}

/// Command to measure embedding model quality.
///
/// Carries pre-extracted [`CodeFragment`]s (extracted by the CLI from workspace
/// Rust sources via the infrastructure-layer extractor, T007). The interactor
/// computes pairwise cosine similarities and assembles [`QualityMetrics`].
#[derive(Debug, Clone)]
pub struct MeasureQualityCommand {
    /// The pre-extracted code fragments to use as the evaluation corpus.
    pub fragments: Vec<CodeFragment>,
}

/// Output from measure-quality: the cosine similarity distribution (AC-03/IN-05).
///
/// Represented as mean, standard deviation, and percentile buckets for threshold
/// calibration.
///
/// - `mean_cosine` and `cosine_std_dev` are the raw cosine similarity mean and
///   standard deviation (in `[-1.0, 1.0]`).
/// - `cosine_percentiles` is `[p10, p25, p50, p75, p90, p95, p99]` of the
///   cosine similarity distribution. Exact when the cross-file pair count is
///   ≤ `MAX_SAMPLE` (10 000); for larger corpora, computed from a deterministic
///   reservoir sample (PoC approximation, sufficient for threshold calibration
///   per AC-03).
/// - `above_threshold_rate` is the fraction of randomly sampled cross-file
///   fragment pairs whose raw cosine exceeds the default threshold (the PoC
///   proxy for false-positive risk, as defined in AC-03/IN-05).
///
/// Raw `f32` values; no domain constraint enforced at this PoC stage.
#[derive(Debug, Clone)]
pub struct QualityMetrics {
    /// Mean raw cosine similarity across the sampled fragment pairs.
    pub mean_cosine: f32,
    /// Standard deviation of raw cosine similarity across the sampled pairs.
    pub cosine_std_dev: f32,
    /// Percentile values `[p10, p25, p50, p75, p90, p95, p99]` of the cosine
    /// similarity distribution. Exact when cross-file pair count ≤ 10 000;
    /// a deterministic reservoir-sample approximation for larger corpora (PoC).
    pub cosine_percentiles: Vec<f32>,
    /// Fraction of cross-file fragment pairs whose raw cosine exceeds the
    /// default similarity threshold (false-positive proxy).
    pub above_threshold_rate: f32,
}
