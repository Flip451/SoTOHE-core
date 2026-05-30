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
        // Retrieve all above-threshold candidates per fragment. Using usize::MAX
        // signals to the index adapter "return as many as you have"; the adapter
        // clamps to its actual row count so the value is safe.  A fixed small
        // constant (e.g. 10) would silently truncate when there are more matches,
        // making DupCheckWarning.similar_fragments incomplete.
        //
        // SAFETY: usize::MAX >= 1, so TopK::new always returns Ok.
        let top_k = match TopK::new(usize::MAX) {
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
