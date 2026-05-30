//! Use-case layer for semantic duplicate detection.
//!
//! Ports, error types, command/output types, application-service traits, and
//! interactors for the discoverability soft-gate feature
//! (ADR 2026-05-29-1118-semantic-dup-detection-discoverability-gate).
//!
//! Ports are placed here (not in domain) because embedding and vector-index
//! capabilities are infrastructure concerns — the domain carries no concept of
//! ML inference or ANN search. Analogous to `ReviewHasher`.

use std::fmt;
use std::sync::Arc;

use domain::semantic_dup::{CodeFragment, SimilarFragment, SimilarityThreshold, TopK};
use thiserror::Error;

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

// ── Error types ───────────────────────────────────────────────────────────────

/// Error type for the [`EmbeddingPort`].
///
/// `source` is an opaque string from fastembed-rs — no domain concept.
#[derive(Debug)]
pub enum EmbeddingError {
    /// The embedding model failed to load or initialise.
    ModelLoadFailed {
        /// Opaque error string from the underlying fastembed-rs error.
        source: String,
    },
    /// Inference over a fragment failed.
    InferenceFailed {
        /// Opaque error string from the underlying fastembed-rs error.
        source: String,
    },
}

impl fmt::Display for EmbeddingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ModelLoadFailed { source } => {
                write!(f, "embedding model load failed: {source}")
            }
            Self::InferenceFailed { source } => {
                write!(f, "embedding inference failed: {source}")
            }
        }
    }
}

impl std::error::Error for EmbeddingError {}

/// Error type for the [`SemanticIndexPort`].
///
/// `source` is an opaque string from LanceDB — no domain concept.
#[derive(Debug)]
pub enum SemanticIndexError {
    /// Opening (or creating) the vector index failed.
    OpenFailed {
        /// Opaque error string from the underlying LanceDB error.
        source: String,
    },
    /// Inserting a fragment+embedding into the index failed.
    InsertFailed {
        /// Opaque error string from the underlying LanceDB error.
        source: String,
    },
    /// Searching the index failed.
    SearchFailed {
        /// Opaque error string from the underlying LanceDB error.
        source: String,
    },
}

impl fmt::Display for SemanticIndexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OpenFailed { source } => {
                write!(f, "semantic index open failed: {source}")
            }
            Self::InsertFailed { source } => {
                write!(f, "semantic index insert failed: {source}")
            }
            Self::SearchFailed { source } => {
                write!(f, "semantic index search failed: {source}")
            }
        }
    }
}

impl std::error::Error for SemanticIndexError {}

/// Composite error for the find-similar use case.
#[derive(Debug, Error)]
pub enum FindSimilarError {
    /// An embedding operation failed.
    #[error(transparent)]
    Embedding(#[from] EmbeddingError),
    /// An index operation failed.
    #[error(transparent)]
    Index(#[from] SemanticIndexError),
}

/// Composite error for the dup-check use case.
#[derive(Debug, Error)]
pub enum DupCheckError {
    /// An embedding operation failed.
    #[error(transparent)]
    Embedding(#[from] EmbeddingError),
    /// An index operation failed.
    #[error(transparent)]
    Index(#[from] SemanticIndexError),
}

/// Composite error for the build-index use case.
#[derive(Debug)]
pub enum BuildIndexError {
    /// An embedding operation failed.
    Embedding(EmbeddingError),
    /// An index operation failed.
    Index(SemanticIndexError),
    /// A filesystem I/O operation failed.
    Io {
        /// The path that was being accessed when the error occurred.
        path: std::path::PathBuf,
        /// Opaque error string from the underlying I/O error.
        source: String,
    },
}

impl fmt::Display for BuildIndexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Embedding(e) => fmt::Display::fmt(e, f),
            Self::Index(e) => fmt::Display::fmt(e, f),
            Self::Io { path, source } => {
                write!(f, "I/O error at {}: {source}", path.display())
            }
        }
    }
}

impl std::error::Error for BuildIndexError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Embedding(e) => Some(e),
            Self::Index(e) => Some(e),
            Self::Io { .. } => None,
        }
    }
}

impl From<EmbeddingError> for BuildIndexError {
    fn from(e: EmbeddingError) -> Self {
        Self::Embedding(e)
    }
}

impl From<SemanticIndexError> for BuildIndexError {
    fn from(e: SemanticIndexError) -> Self {
        Self::Index(e)
    }
}

/// Composite error for the measure-quality use case.
#[derive(Debug)]
pub enum MeasureQualityError {
    /// An embedding operation failed.
    Embedding(EmbeddingError),
    /// An index operation failed.
    Index(SemanticIndexError),
    /// A filesystem I/O operation failed.
    Io {
        /// The path that was being accessed when the error occurred.
        path: std::path::PathBuf,
        /// Opaque error string from the underlying I/O error.
        source: String,
    },
}

impl fmt::Display for MeasureQualityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Embedding(e) => fmt::Display::fmt(e, f),
            Self::Index(e) => fmt::Display::fmt(e, f),
            Self::Io { path, source } => {
                write!(f, "I/O error at {}: {source}", path.display())
            }
        }
    }
}

impl std::error::Error for MeasureQualityError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Embedding(e) => Some(e),
            Self::Index(e) => Some(e),
            Self::Io { .. } => None,
        }
    }
}

impl From<EmbeddingError> for MeasureQualityError {
    fn from(e: EmbeddingError) -> Self {
        Self::Embedding(e)
    }
}

impl From<SemanticIndexError> for MeasureQualityError {
    fn from(e: SemanticIndexError) -> Self {
        Self::Index(e)
    }
}

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
/// - `cosine_percentiles` is `[p10, p25, p50, p75, p90, p95, p99]` — the full
///   distribution needed to calibrate the soft-gate threshold (AC-03).
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
    /// similarity distribution.
    pub cosine_percentiles: Vec<f32>,
    /// Fraction of cross-file fragment pairs whose raw cosine exceeds the
    /// default similarity threshold (false-positive proxy).
    pub above_threshold_rate: f32,
}

// ── Application-service traits ────────────────────────────────────────────────

/// Application service for the find-similar use case (`sotp find-similar`).
pub trait FindSimilarService {
    /// Retrieve the top-k fragments most semantically similar to the query
    /// fragment in the index.
    ///
    /// Implements CN-05: information-only, never blocks.
    ///
    /// # Errors
    ///
    /// Returns [`FindSimilarError::Embedding`] if embedding the query fragment
    /// fails.
    /// Returns [`FindSimilarError::Index`] if the index search fails.
    fn find_similar(&self, cmd: &FindSimilarCommand)
    -> Result<FindSimilarOutput, FindSimilarError>;
}

/// Application service for the dup-check soft gate (`sotp dup-check`).
pub trait DupCheckService {
    /// Check the diff fragments in `cmd` against the index and return a
    /// warning for each fragment with at least one near-duplicate above the
    /// threshold.
    ///
    /// Implements CN-03: operates only on the command's fragments (diff-only
    /// scope, no full-codebase scan).
    ///
    /// # Errors
    ///
    /// Returns [`DupCheckError::Embedding`] if embedding a fragment fails.
    /// Returns [`DupCheckError::Index`] if an index search fails.
    fn dup_check(&self, cmd: &DupCheckCommand) -> Result<DupCheckOutput, DupCheckError>;
}

/// Application service for the index-build command (`sotp dup-index build`).
pub trait BuildIndexService {
    /// Embed all fragments in `cmd` and insert them into the semantic index.
    ///
    /// # Errors
    ///
    /// Returns [`BuildIndexError::Embedding`] if embedding a fragment fails.
    /// Returns [`BuildIndexError::Index`] if inserting into the index fails.
    fn build_index(&self, cmd: &BuildIndexCommand) -> Result<BuildIndexOutput, BuildIndexError>;
}

/// Application service for the PoC quality measurement command
/// (`sotp dup-index measure-quality`).
pub trait MeasureQualityService {
    /// Embed all fragments, compute pairwise cosine similarities, and return
    /// the resulting [`QualityMetrics`].
    ///
    /// # Errors
    ///
    /// Returns [`MeasureQualityError::Embedding`] if embedding a fragment
    /// fails.
    /// Returns [`MeasureQualityError::Index`] if an index operation fails.
    fn measure_quality(
        &self,
        cmd: &MeasureQualityCommand,
    ) -> Result<QualityMetrics, MeasureQualityError>;
}

// ── Interactors ───────────────────────────────────────────────────────────────

/// Interactor implementing [`FindSimilarService`].
///
/// Orchestrates [`EmbeddingPort`] (embed query) and [`SemanticIndexPort`]
/// (search index).
pub struct FindSimilarInteractor {
    embedding_port: Arc<dyn EmbeddingPort>,
    index_port: Arc<dyn SemanticIndexPort>,
}

impl fmt::Debug for FindSimilarInteractor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FindSimilarInteractor")
            .field("embedding_port", &"<dyn EmbeddingPort>")
            .field("index_port", &"<dyn SemanticIndexPort>")
            .finish()
    }
}

impl FindSimilarInteractor {
    /// Create a new [`FindSimilarInteractor`].
    #[must_use]
    pub fn new(
        embedding_port: Arc<dyn EmbeddingPort>,
        index_port: Arc<dyn SemanticIndexPort>,
    ) -> Self {
        Self { embedding_port, index_port }
    }
}

impl FindSimilarService for FindSimilarInteractor {
    /// Retrieve the top-k fragments most semantically similar to the query
    /// fragment in the index.
    ///
    /// # Errors
    ///
    /// Returns [`FindSimilarError::Embedding`] if embedding the query fragment
    /// fails.
    /// Returns [`FindSimilarError::Index`] if the index search fails.
    fn find_similar(
        &self,
        cmd: &FindSimilarCommand,
    ) -> Result<FindSimilarOutput, FindSimilarError> {
        let embedding = self.embedding_port.embed(&cmd.fragment)?;
        let results = self.index_port.search(&embedding, cmd.top_k)?;
        Ok(FindSimilarOutput { results })
    }
}

/// Interactor implementing [`DupCheckService`].
///
/// Orchestrates [`EmbeddingPort`] and [`SemanticIndexPort`] to detect
/// soft-gate duplicates.
pub struct DupCheckInteractor {
    embedding_port: Arc<dyn EmbeddingPort>,
    index_port: Arc<dyn SemanticIndexPort>,
}

impl fmt::Debug for DupCheckInteractor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DupCheckInteractor")
            .field("embedding_port", &"<dyn EmbeddingPort>")
            .field("index_port", &"<dyn SemanticIndexPort>")
            .finish()
    }
}

impl DupCheckInteractor {
    /// Create a new [`DupCheckInteractor`].
    #[must_use]
    pub fn new(
        embedding_port: Arc<dyn EmbeddingPort>,
        index_port: Arc<dyn SemanticIndexPort>,
    ) -> Self {
        Self { embedding_port, index_port }
    }
}

impl DupCheckService for DupCheckInteractor {
    /// Check the diff fragments in `cmd` against the index.
    ///
    /// Implements CN-03: operates only on `cmd.fragments` — no full-codebase
    /// scan is performed.
    ///
    /// For each fragment, its embedding is computed and the index is searched.
    /// Any matches whose similarity score meets or exceeds `cmd.threshold` are
    /// collected as a [`DupCheckWarning`]. Fragments with no matches above the
    /// threshold produce no warning.
    ///
    /// # Errors
    ///
    /// Returns [`DupCheckError::Embedding`] if embedding a fragment fails.
    /// Returns [`DupCheckError::Index`] if an index search fails.
    fn dup_check(&self, cmd: &DupCheckCommand) -> Result<DupCheckOutput, DupCheckError> {
        // Use a generous-but-bounded cap instead of usize::MAX so the LanceDB
        // adapter never receives an unbounded `.limit(usize::MAX)` call.
        //
        // 100_000 is large enough to surface all realistic near-duplicate
        // candidates in a workspace without risking an unbounded result set.
        // The LanceDB adapter additionally clamps to a safe maximum on its
        // side (see `LanceDbSemanticIndexAdapter::search`), so the effective
        // limit is `min(DUP_CHECK_MAX_RESULTS, adapter_cap)`.
        //
        // A fixed small constant (e.g. 10) would silently truncate when there
        // are more matches, making DupCheckWarning.similar_fragments incomplete.
        const DUP_CHECK_MAX_RESULTS: usize = 100_000;
        // DUP_CHECK_MAX_RESULTS >= 1, so TopK::new always returns Ok.
        let top_k = match TopK::new(DUP_CHECK_MAX_RESULTS) {
            Ok(k) => k,
            Err(_) => {
                return Err(DupCheckError::Index(SemanticIndexError::SearchFailed {
                    source: "internal: invalid top-k constant".to_owned(),
                }));
            }
        };
        let mut warnings = Vec::new();

        for fragment in &cmd.fragments {
            let embedding = self.embedding_port.embed(fragment)?;
            let candidates = self.index_port.search(&embedding, top_k)?;

            let above_threshold: Vec<SimilarFragment> = candidates
                .into_iter()
                .filter(|sf| sf.score.value() >= cmd.threshold.value())
                .collect();

            if !above_threshold.is_empty() {
                warnings.push(DupCheckWarning {
                    input_fragment: fragment.clone(),
                    similar_fragments: above_threshold,
                });
            }
        }

        Ok(DupCheckOutput { warnings })
    }
}

/// Interactor implementing [`BuildIndexService`].
///
/// Receives pre-extracted [`CodeFragment`]s (via [`BuildIndexCommand::fragments`],
/// extracted by the CLI using the infrastructure extractor), embeds each
/// fragment via [`EmbeddingPort`], and inserts into the index via
/// [`SemanticIndexPort`]. No filesystem I/O in this interactor.
pub struct BuildIndexInteractor {
    embedding_port: Arc<dyn EmbeddingPort>,
    index_port: Arc<dyn SemanticIndexPort>,
}

impl fmt::Debug for BuildIndexInteractor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BuildIndexInteractor")
            .field("embedding_port", &"<dyn EmbeddingPort>")
            .field("index_port", &"<dyn SemanticIndexPort>")
            .finish()
    }
}

impl BuildIndexInteractor {
    /// Create a new [`BuildIndexInteractor`].
    #[must_use]
    pub fn new(
        embedding_port: Arc<dyn EmbeddingPort>,
        index_port: Arc<dyn SemanticIndexPort>,
    ) -> Self {
        Self { embedding_port, index_port }
    }
}

impl BuildIndexService for BuildIndexInteractor {
    /// Embed each fragment in `cmd` and insert it into the semantic index.
    ///
    /// Returns the count of fragments successfully indexed.
    ///
    /// # Errors
    ///
    /// Returns [`BuildIndexError::Embedding`] if embedding a fragment fails.
    /// Returns [`BuildIndexError::Index`] if inserting a fragment into the
    /// index fails.
    fn build_index(&self, cmd: &BuildIndexCommand) -> Result<BuildIndexOutput, BuildIndexError> {
        let mut fragments_indexed: usize = 0;

        for fragment in &cmd.fragments {
            let embedding = self.embedding_port.embed(fragment)?;
            self.index_port.insert(fragment, &embedding)?;
            fragments_indexed += 1;
        }

        Ok(BuildIndexOutput { fragments_indexed })
    }
}

/// Interactor implementing [`MeasureQualityService`].
///
/// Drives the PoC quality measurement pipeline: embeds all fragments,
/// computes pairwise cosine similarities, and assembles [`QualityMetrics`].
pub struct MeasureQualityInteractor {
    embedding_port: Arc<dyn EmbeddingPort>,
    /// Reserved for future index-backed quality checks; not used in the
    /// current PoC implementation which computes pairwise similarities
    /// in-memory from embedded fragments.
    #[allow(dead_code)]
    index_port: Arc<dyn SemanticIndexPort>,
}

impl fmt::Debug for MeasureQualityInteractor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MeasureQualityInteractor")
            .field("embedding_port", &"<dyn EmbeddingPort>")
            .field("index_port", &"<dyn SemanticIndexPort>")
            .finish()
    }
}

impl MeasureQualityInteractor {
    /// Create a new [`MeasureQualityInteractor`].
    #[must_use]
    pub fn new(
        embedding_port: Arc<dyn EmbeddingPort>,
        index_port: Arc<dyn SemanticIndexPort>,
    ) -> Self {
        Self { embedding_port, index_port }
    }
}

impl MeasureQualityService for MeasureQualityInteractor {
    /// Embed all fragments, compute pairwise cosine similarities, and return
    /// the resulting distribution as [`QualityMetrics`].
    ///
    /// Pairwise similarities are computed only between fragments from
    /// different source paths (cross-file pairs), which is the PoC proxy
    /// for false-positive risk (AC-03/IN-05). Self-pairs (same source path)
    /// are excluded.
    ///
    /// When fewer than two fragments are present, or no cross-file pairs
    /// exist, all metrics are returned as `0.0`.
    ///
    /// # Errors
    ///
    /// Returns [`MeasureQualityError::Embedding`] if embedding a fragment
    /// fails.
    /// Returns [`MeasureQualityError::Index`] if an index operation fails.
    fn measure_quality(
        &self,
        cmd: &MeasureQualityCommand,
    ) -> Result<QualityMetrics, MeasureQualityError> {
        // Embed all fragments.
        let embeddings: Vec<Vec<f32>> = cmd
            .fragments
            .iter()
            .map(|f| self.embedding_port.embed(f).map_err(MeasureQualityError::from))
            .collect::<Result<Vec<_>, _>>()?;

        // Compute pairwise cosine similarities for cross-file fragment pairs.
        let mut similarities: Vec<f32> = Vec::new();

        let n_frags = cmd.fragments.len();
        for i in 0..n_frags {
            for j in (i + 1)..n_frags {
                // Only cross-file pairs (CN-03 / AC-03 scope).
                let Some(fi) = cmd.fragments.get(i) else { continue };
                let Some(fj) = cmd.fragments.get(j) else { continue };
                if fi.source_path == fj.source_path {
                    continue;
                }
                let Some(ei) = embeddings.get(i) else { continue };
                let Some(ej) = embeddings.get(j) else { continue };
                let sim = cosine_similarity(ei, ej);
                similarities.push(sim);
            }
        }

        if similarities.is_empty() {
            return Ok(QualityMetrics {
                mean_cosine: 0.0,
                cosine_std_dev: 0.0,
                cosine_percentiles: vec![0.0; 7],
                above_threshold_rate: 0.0,
            });
        }

        let n = similarities.len() as f32;
        let mean_cosine = similarities.iter().copied().sum::<f32>() / n;

        let variance = similarities.iter().map(|&s| (s - mean_cosine).powi(2)).sum::<f32>() / n;
        let cosine_std_dev = variance.sqrt();

        let mut sorted = similarities.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let percentile = |p: f32| -> f32 {
            let idx = ((p / 100.0) * (sorted.len() as f32 - 1.0)).round() as usize;
            sorted.get(idx).copied().unwrap_or(0.0)
        };

        let cosine_percentiles = vec![
            percentile(10.0),
            percentile(25.0),
            percentile(50.0),
            percentile(75.0),
            percentile(90.0),
            percentile(95.0),
            percentile(99.0),
        ];

        // Default threshold for the above-threshold-rate proxy (0.8 is the
        // typical starting point for the soft gate; the exact value is
        // calibrated from the percentile distribution).
        //
        // AC-03 defines `above_threshold_rate` as the fraction of *randomly
        // sampled* cross-file fragment pairs exceeding the threshold.  When the
        // total number of pairs is small the full population IS the sample, so
        // no subsampling is needed.  When it is large we apply a deterministic
        // subsample (XorShift64 PRNG seeded with the pair count) so the
        // measure remains O(MAX_SAMPLE) rather than O(n^2) at scale.
        let default_threshold: f32 = 0.8;
        const MAX_SAMPLE: usize = 10_000;
        let above_threshold_rate = if similarities.len() <= MAX_SAMPLE {
            let above = similarities.iter().filter(|&&s| s >= default_threshold).count();
            above as f32 / similarities.len() as f32
        } else {
            // Deterministic XorShift64 subsample: seed from pair count for
            // reproducibility.  This avoids an external `rand` dependency while
            // satisfying the "randomly sampled" requirement of AC-03.
            let mut state: u64 = similarities.len() as u64 | 1; // seed must be non-zero
            let mut above = 0usize;
            for _ in 0..MAX_SAMPLE {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                let idx = (state as usize) % similarities.len();
                if similarities.get(idx).copied().unwrap_or(0.0) >= default_threshold {
                    above += 1;
                }
            }
            above as f32 / MAX_SAMPLE as f32
        };

        Ok(QualityMetrics { mean_cosine, cosine_std_dev, cosine_percentiles, above_threshold_rate })
    }
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Compute the cosine similarity between two embedding vectors.
///
/// Returns `0.0` when either vector has zero magnitude.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let mag_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let mag_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if mag_a == 0.0 || mag_b == 0.0 {
        return 0.0;
    }

    dot / (mag_a * mag_b)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::type_complexity)]

    use std::path::PathBuf;

    use domain::semantic_dup::{CodeFragment, SimilarFragment, SimilarityScore};
    use mockall::{mock, predicate};

    use super::*;

    // ── Mock port definitions ─────────────────────────────────────────────────
    //
    // We use `mock!` (not `#[automock]`) so that production trait definitions
    // are not modified and the catalogued public shapes remain unchanged.

    mock! {
        pub MockEmbeddingPort {}
        impl EmbeddingPort for MockEmbeddingPort {
            fn embed(&self, fragment: &CodeFragment) -> Result<Vec<f32>, EmbeddingError>;
        }
    }

    mock! {
        pub MockSemanticIndexPort {}
        impl SemanticIndexPort for MockSemanticIndexPort {
            fn insert(
                &self,
                fragment: &CodeFragment,
                embedding: &[f32],
            ) -> Result<(), SemanticIndexError>;

            fn search(
                &self,
                embedding: &[f32],
                top_k: domain::semantic_dup::TopK,
            ) -> Result<Vec<SimilarFragment>, SemanticIndexError>;
        }
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn make_fragment(path: &str, content: &str) -> CodeFragment {
        CodeFragment::new(PathBuf::from(path), content.to_owned()).unwrap()
    }

    fn make_score(v: f32) -> SimilarityScore {
        SimilarityScore::new(v).unwrap()
    }

    fn make_similar_fragment(path: &str, content: &str, score: f32) -> SimilarFragment {
        SimilarFragment { fragment: make_fragment(path, content), score: make_score(score) }
    }

    // ── FindSimilarInteractor ─────────────────────────────────────────────────

    #[test]
    fn test_find_similar_interactor_delegates_embed_then_search() {
        let query = make_fragment("<query>", "fn query() {}");
        let top_k = domain::semantic_dup::TopK::new(3).unwrap();

        let expected_embedding = vec![0.1_f32, 0.2, 0.3];
        let expected_results = vec![make_similar_fragment("src/foo.rs", "fn foo() {}", 0.9)];

        let mut mock_embed = MockMockEmbeddingPort::new();
        {
            let emb = expected_embedding.clone();
            mock_embed.expect_embed().times(1).returning(move |_| Ok(emb.clone()));
        }

        let mut mock_index = MockMockSemanticIndexPort::new();
        {
            let results = expected_results.clone();
            let expected_emb = expected_embedding.clone();
            let expected_top_k = top_k.value();
            // Verify that search receives the exact embedding returned by embed and the
            // caller-supplied top_k.
            mock_index
                .expect_search()
                .times(1)
                .withf(move |emb, k| emb == expected_emb.as_slice() && k.value() == expected_top_k)
                .returning(move |_, _| Ok(results.clone()));
        }

        let interactor = FindSimilarInteractor::new(Arc::new(mock_embed), Arc::new(mock_index));
        let output =
            interactor.find_similar(&FindSimilarCommand { fragment: query, top_k }).unwrap();

        assert_eq!(output.results.len(), 1);
        assert_eq!(output.results[0].fragment.source_path, PathBuf::from("src/foo.rs"));
    }

    #[test]
    fn test_find_similar_interactor_propagates_index_error() {
        let query = make_fragment("<query>", "fn query() {}");
        let top_k = domain::semantic_dup::TopK::new(1).unwrap();

        let mut mock_embed = MockMockEmbeddingPort::new();
        mock_embed.expect_embed().times(1).returning(|_| Ok(vec![0.1, 0.2]));

        let mut mock_index = MockMockSemanticIndexPort::new();
        // search is called (embed succeeded) but returns an error.
        mock_index.expect_search().times(1).returning(|_, _| {
            Err(SemanticIndexError::SearchFailed { source: "search error".to_owned() })
        });

        let interactor = FindSimilarInteractor::new(Arc::new(mock_embed), Arc::new(mock_index));
        let result = interactor.find_similar(&FindSimilarCommand { fragment: query, top_k });

        assert!(matches!(result, Err(FindSimilarError::Index(_))));
    }

    #[test]
    fn test_find_similar_interactor_propagates_embedding_error() {
        let query = make_fragment("<query>", "fn query() {}");
        let top_k = domain::semantic_dup::TopK::new(1).unwrap();

        let mut mock_embed = MockMockEmbeddingPort::new();
        mock_embed.expect_embed().times(1).returning(|_| {
            Err(EmbeddingError::InferenceFailed { source: "inference error".to_owned() })
        });

        let mock_index = MockMockSemanticIndexPort::new();
        // search must NOT be called when embed fails.

        let interactor = FindSimilarInteractor::new(Arc::new(mock_embed), Arc::new(mock_index));
        let result = interactor.find_similar(&FindSimilarCommand { fragment: query, top_k });

        assert!(matches!(result, Err(FindSimilarError::Embedding(_))));
    }

    // ── DupCheckInteractor ────────────────────────────────────────────────────

    #[test]
    fn test_dup_check_interactor_produces_warning_for_fragment_above_threshold() {
        // CN-03: only diff fragments are checked.
        let threshold = SimilarityThreshold::new(0.8).unwrap();
        let frag = make_fragment("src/new.rs", "fn new_impl() {}");

        // The mock returns a result with score 0.9 > threshold 0.8.
        let above_threshold_result =
            vec![make_similar_fragment("src/existing.rs", "fn existing() {}", 0.9)];

        let mut mock_embed = MockMockEmbeddingPort::new();
        mock_embed.expect_embed().times(1).returning(|_| Ok(vec![0.1, 0.2]));

        let mut mock_index = MockMockSemanticIndexPort::new();
        {
            let res = above_threshold_result.clone();
            mock_index.expect_search().times(1).returning(move |_, _| Ok(res.clone()));
        }

        let interactor = DupCheckInteractor::new(Arc::new(mock_embed), Arc::new(mock_index));
        let output =
            interactor.dup_check(&DupCheckCommand { fragments: vec![frag], threshold }).unwrap();

        assert_eq!(output.warnings.len(), 1, "expected one warning for the above-threshold result");
        assert_eq!(output.warnings[0].similar_fragments.len(), 1);
        assert_eq!(output.warnings[0].similar_fragments[0].score.value(), 0.9);
    }

    #[test]
    fn test_dup_check_interactor_produces_no_warning_for_fragment_below_threshold() {
        let threshold = SimilarityThreshold::new(0.8).unwrap();
        let frag = make_fragment("src/new.rs", "fn new_impl() {}");

        // The mock returns a result with score 0.7 < threshold 0.8.
        let below_threshold_result =
            vec![make_similar_fragment("src/existing.rs", "fn existing() {}", 0.7)];

        let mut mock_embed = MockMockEmbeddingPort::new();
        mock_embed.expect_embed().times(1).returning(|_| Ok(vec![0.1, 0.2]));

        let mut mock_index = MockMockSemanticIndexPort::new();
        {
            let res = below_threshold_result.clone();
            mock_index.expect_search().times(1).returning(move |_, _| Ok(res.clone()));
        }

        let interactor = DupCheckInteractor::new(Arc::new(mock_embed), Arc::new(mock_index));
        let output =
            interactor.dup_check(&DupCheckCommand { fragments: vec![frag], threshold }).unwrap();

        assert!(output.warnings.is_empty(), "expected no warnings for below-threshold result");
    }

    #[test]
    fn test_dup_check_interactor_produces_warning_when_score_equals_threshold() {
        // Boundary: score exactly at threshold is >= threshold so should warn.
        let threshold = SimilarityThreshold::new(0.8).unwrap();
        let frag = make_fragment("src/new.rs", "fn new_impl() {}");

        let at_threshold_result =
            vec![make_similar_fragment("src/existing.rs", "fn existing() {}", 0.8)];

        let mut mock_embed = MockMockEmbeddingPort::new();
        mock_embed.expect_embed().times(1).returning(|_| Ok(vec![0.1, 0.2]));

        let mut mock_index = MockMockSemanticIndexPort::new();
        {
            let res = at_threshold_result.clone();
            mock_index.expect_search().times(1).returning(move |_, _| Ok(res.clone()));
        }

        let interactor = DupCheckInteractor::new(Arc::new(mock_embed), Arc::new(mock_index));
        let output =
            interactor.dup_check(&DupCheckCommand { fragments: vec![frag], threshold }).unwrap();

        assert_eq!(
            output.warnings.len(),
            1,
            "expected warning when score equals threshold (>= comparison)"
        );
    }

    #[test]
    fn test_dup_check_interactor_with_empty_fragments_returns_no_warnings() {
        let threshold = SimilarityThreshold::new(0.8).unwrap();

        let mock_embed = MockMockEmbeddingPort::new();
        let mock_index = MockMockSemanticIndexPort::new();
        // Neither embed nor search should be called for empty input.

        let interactor = DupCheckInteractor::new(Arc::new(mock_embed), Arc::new(mock_index));
        let output =
            interactor.dup_check(&DupCheckCommand { fragments: vec![], threshold }).unwrap();

        assert!(output.warnings.is_empty());
    }

    #[test]
    fn test_dup_check_interactor_checks_only_supplied_diff_fragments_not_full_codebase() {
        // CN-03: only the supplied fragments are embedded and searched.
        let threshold = SimilarityThreshold::new(0.8).unwrap();
        let frag1 = make_fragment("src/a.rs", "fn a() {}");
        let frag2 = make_fragment("src/b.rs", "fn b() {}");

        let mut mock_embed = MockMockEmbeddingPort::new();
        // embed must be called exactly twice — once per supplied fragment.
        mock_embed.expect_embed().times(2).returning(|_| Ok(vec![0.1]));

        let mut mock_index = MockMockSemanticIndexPort::new();
        // search must be called exactly twice — once per supplied fragment.
        mock_index.expect_search().times(2).returning(|_, _| Ok(vec![]));

        let interactor = DupCheckInteractor::new(Arc::new(mock_embed), Arc::new(mock_index));
        let output = interactor
            .dup_check(&DupCheckCommand { fragments: vec![frag1, frag2], threshold })
            .unwrap();

        // Both results are below threshold (empty results from mock), no warnings.
        assert!(output.warnings.is_empty());
    }

    #[test]
    fn test_dup_check_interactor_propagates_embedding_error() {
        let threshold = SimilarityThreshold::new(0.8).unwrap();
        let frag = make_fragment("src/new.rs", "fn new_impl() {}");

        let mut mock_embed = MockMockEmbeddingPort::new();
        mock_embed.expect_embed().times(1).returning(|_| {
            Err(EmbeddingError::InferenceFailed { source: "embed error".to_owned() })
        });

        let mock_index = MockMockSemanticIndexPort::new();
        // search must NOT be called when embed fails.

        let interactor = DupCheckInteractor::new(Arc::new(mock_embed), Arc::new(mock_index));
        let result = interactor.dup_check(&DupCheckCommand { fragments: vec![frag], threshold });

        assert!(matches!(result, Err(DupCheckError::Embedding(_))));
    }

    #[test]
    fn test_dup_check_interactor_propagates_index_error() {
        let threshold = SimilarityThreshold::new(0.8).unwrap();
        let frag = make_fragment("src/new.rs", "fn new_impl() {}");

        let mut mock_embed = MockMockEmbeddingPort::new();
        mock_embed.expect_embed().times(1).returning(|_| Ok(vec![0.1, 0.2]));

        let mut mock_index = MockMockSemanticIndexPort::new();
        // search is called (embed succeeded) but returns an error.
        mock_index.expect_search().times(1).returning(|_, _| {
            Err(SemanticIndexError::SearchFailed { source: "search error".to_owned() })
        });

        let interactor = DupCheckInteractor::new(Arc::new(mock_embed), Arc::new(mock_index));
        let result = interactor.dup_check(&DupCheckCommand { fragments: vec![frag], threshold });

        assert!(matches!(result, Err(DupCheckError::Index(_))));
    }

    // ── BuildIndexInteractor ──────────────────────────────────────────────────

    #[test]
    fn test_build_index_interactor_calls_insert_for_each_fragment() {
        use std::sync::Mutex;

        let frags = vec![
            make_fragment("src/a.rs", "fn a() {}"),
            make_fragment("src/b.rs", "fn b() {}"),
            make_fragment("src/c.rs", "fn c() {}"),
        ];

        // Each fragment gets a distinct embedding so we can verify the interactor
        // passes the correct per-fragment embedding to insert.
        let embed_counter = Arc::new(Mutex::new(0u32));
        let mut mock_embed = MockMockEmbeddingPort::new();
        let counter_clone = Arc::clone(&embed_counter);
        mock_embed.expect_embed().times(3).returning(move |_| {
            let mut c = counter_clone.lock().unwrap();
            let v = *c as f32;
            *c += 1;
            Ok(vec![v, 0.0]) // embedding [0,0], [1,0], [2,0] for frags a, b, c
        });

        // Capture (source_path, embedding) pairs passed to insert to verify correct
        // per-fragment delegation: each fragment must be inserted with its own embedding.
        let inserted: Arc<Mutex<Vec<(PathBuf, Vec<f32>)>>> = Arc::new(Mutex::new(Vec::new()));
        let inserted_clone = Arc::clone(&inserted);
        let mut mock_index = MockMockSemanticIndexPort::new();
        // insert called once per fragment; capture both arguments.
        mock_index.expect_insert().times(3).returning(move |frag, emb| {
            inserted_clone.lock().unwrap().push((frag.source_path.clone(), emb.to_vec()));
            Ok(())
        });

        let interactor = BuildIndexInteractor::new(Arc::new(mock_embed), Arc::new(mock_index));
        let output = interactor.build_index(&BuildIndexCommand { fragments: frags }).unwrap();

        assert_eq!(output.fragments_indexed, 3);
        // Verify each insert received the correct fragment (by path) and its corresponding
        // per-fragment embedding, confirming the embed-to-insert delegation is correct.
        let calls = inserted.lock().unwrap();
        assert_eq!(calls.len(), 3);
        assert_eq!(calls[0], (PathBuf::from("src/a.rs"), vec![0.0_f32, 0.0]));
        assert_eq!(calls[1], (PathBuf::from("src/b.rs"), vec![1.0_f32, 0.0]));
        assert_eq!(calls[2], (PathBuf::from("src/c.rs"), vec![2.0_f32, 0.0]));
    }

    #[test]
    fn test_build_index_interactor_with_empty_fragments_indexes_zero() {
        let mock_embed = MockMockEmbeddingPort::new();
        let mock_index = MockMockSemanticIndexPort::new();
        // Neither embed nor insert should be called.

        let interactor = BuildIndexInteractor::new(Arc::new(mock_embed), Arc::new(mock_index));
        let output = interactor.build_index(&BuildIndexCommand { fragments: vec![] }).unwrap();

        assert_eq!(output.fragments_indexed, 0);
    }

    #[test]
    fn test_build_index_interactor_propagates_embedding_error() {
        let frags = vec![make_fragment("src/a.rs", "fn a() {}")];

        let mut mock_embed = MockMockEmbeddingPort::new();
        mock_embed.expect_embed().times(1).returning(|_| {
            Err(EmbeddingError::ModelLoadFailed { source: "model failed".to_owned() })
        });

        let mock_index = MockMockSemanticIndexPort::new();

        let interactor = BuildIndexInteractor::new(Arc::new(mock_embed), Arc::new(mock_index));
        let result = interactor.build_index(&BuildIndexCommand { fragments: frags });

        assert!(matches!(result, Err(BuildIndexError::Embedding(_))));
    }

    #[test]
    fn test_build_index_interactor_propagates_index_insert_error() {
        let frags = vec![make_fragment("src/a.rs", "fn a() {}")];

        let mut mock_embed = MockMockEmbeddingPort::new();
        mock_embed.expect_embed().times(1).returning(|_| Ok(vec![1.0, 0.0]));

        let mut mock_index = MockMockSemanticIndexPort::new();
        // insert is called (embed succeeded) but returns an error.
        mock_index
            .expect_insert()
            .times(1)
            .with(predicate::always(), predicate::always())
            .returning(|_, _| {
                Err(SemanticIndexError::InsertFailed { source: "insert error".to_owned() })
            });

        let interactor = BuildIndexInteractor::new(Arc::new(mock_embed), Arc::new(mock_index));
        let result = interactor.build_index(&BuildIndexCommand { fragments: frags });

        assert!(matches!(result, Err(BuildIndexError::Index(_))));
    }

    // ── MeasureQualityInteractor ──────────────────────────────────────────────

    #[test]
    fn test_measure_quality_interactor_returns_metrics_for_cross_file_fragments() {
        // Two fragments from different paths — one cross-file pair exists.
        let frags =
            vec![make_fragment("src/a.rs", "fn a() {}"), make_fragment("src/b.rs", "fn b() {}")];

        // Use identical unit vectors: cosine similarity = 1.0 (non-zero, distinguishable
        // from the no-pairs zero-metrics path).
        let mut mock_embed = MockMockEmbeddingPort::new();
        mock_embed.expect_embed().times(2).returning(|_| Ok(vec![1.0_f32, 0.0]));

        let mock_index = MockMockSemanticIndexPort::new();

        let interactor = MeasureQualityInteractor::new(Arc::new(mock_embed), Arc::new(mock_index));
        let metrics =
            interactor.measure_quality(&MeasureQualityCommand { fragments: frags }).unwrap();

        // Cosine similarity of [1,0] and [1,0] = 1.0; mean and all percentiles should be 1.0.
        assert!(
            (metrics.mean_cosine - 1.0).abs() < 1e-4,
            "expected mean_cosine ≈ 1.0, got {}",
            metrics.mean_cosine
        );
        // Single pair → variance = 0; std dev must be 0.0.
        assert!(
            metrics.cosine_std_dev.abs() < 1e-4,
            "expected cosine_std_dev ≈ 0.0, got {}",
            metrics.cosine_std_dev
        );
        assert_eq!(metrics.cosine_percentiles.len(), 7);
        for (i, &p) in metrics.cosine_percentiles.iter().enumerate() {
            assert!((p - 1.0).abs() < 1e-4, "cosine_percentiles[{}] should be ≈ 1.0, got {}", i, p);
        }
        // Similarity 1.0 >= default threshold 0.8, so above_threshold_rate must be 1.0.
        assert!(
            (metrics.above_threshold_rate - 1.0).abs() < 1e-4,
            "expected above_threshold_rate ≈ 1.0, got {}",
            metrics.above_threshold_rate
        );
    }

    #[test]
    fn test_measure_quality_interactor_with_single_fragment_returns_zero_metrics() {
        let frags = vec![make_fragment("src/a.rs", "fn a() {}")];

        let mut mock_embed = MockMockEmbeddingPort::new();
        mock_embed.expect_embed().times(1).returning(|_| Ok(vec![1.0, 0.0]));

        let mock_index = MockMockSemanticIndexPort::new();

        let interactor = MeasureQualityInteractor::new(Arc::new(mock_embed), Arc::new(mock_index));
        let metrics =
            interactor.measure_quality(&MeasureQualityCommand { fragments: frags }).unwrap();

        // Single fragment = no cross-file pairs = zero metrics.
        assert_eq!(metrics.mean_cosine, 0.0);
        assert_eq!(metrics.cosine_std_dev, 0.0);
        assert_eq!(metrics.above_threshold_rate, 0.0);
    }

    #[test]
    fn test_measure_quality_interactor_with_same_path_fragments_returns_zero_metrics() {
        // Two fragments from the SAME path: cross-file filter excludes them.
        let frags =
            vec![make_fragment("src/a.rs", "fn a() {}"), make_fragment("src/a.rs", "fn b() {}")];

        let mut mock_embed = MockMockEmbeddingPort::new();
        mock_embed.expect_embed().times(2).returning(|_| Ok(vec![1.0, 0.0]));

        let mock_index = MockMockSemanticIndexPort::new();

        let interactor = MeasureQualityInteractor::new(Arc::new(mock_embed), Arc::new(mock_index));
        let metrics =
            interactor.measure_quality(&MeasureQualityCommand { fragments: frags }).unwrap();

        assert_eq!(metrics.mean_cosine, 0.0, "same-path pairs should be excluded");
    }

    #[test]
    fn test_measure_quality_interactor_propagates_embedding_error() {
        let frags = vec![make_fragment("src/a.rs", "fn a() {}")];

        let mut mock_embed = MockMockEmbeddingPort::new();
        mock_embed
            .expect_embed()
            .times(1)
            .returning(|_| Err(EmbeddingError::InferenceFailed { source: "bad".to_owned() }));

        let mock_index = MockMockSemanticIndexPort::new();

        let interactor = MeasureQualityInteractor::new(Arc::new(mock_embed), Arc::new(mock_index));
        let result = interactor.measure_quality(&MeasureQualityCommand { fragments: frags });

        assert!(matches!(result, Err(MeasureQualityError::Embedding(_))));
    }
}
