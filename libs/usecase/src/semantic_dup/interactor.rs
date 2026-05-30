//! Application-service traits and interactor implementations for semantic
//! duplicate detection.

use std::fmt;
use std::sync::Arc;

use domain::semantic_dup::TopK;

use super::command::{
    BuildIndexCommand, BuildIndexOutput, DupCheckCommand, DupCheckOutput, DupCheckWarning,
    FindSimilarCommand, FindSimilarOutput, MeasureQualityCommand, QualityMetrics,
};
use super::errors::{
    BuildIndexError, DupCheckError, FindSimilarError, MeasureQualityError, SemanticIndexError,
};
use super::ports::{EmbeddingPort, SemanticIndexPort};

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
    /// A candidate whose `source_path` and `content` are both identical to
    /// the input fragment is treated as the fragment's own indexed copy and
    /// is excluded from warnings. This prevents false-positive self-match
    /// warnings (score ≈ 1.0) when the index already contains the exact
    /// fragment being checked (e.g. after a full index rebuild). Candidates
    /// with the same `source_path` but different `content` (e.g. a modified
    /// version of a function in the same file) are still reported, preserving
    /// detection of intra-file and modified-version near-duplicates.
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

            let above_threshold: Vec<_> = candidates
                .into_iter()
                .filter(|sf| {
                    sf.score.value() >= cmd.threshold.value()
                        && !(sf.fragment.source_path == fragment.source_path
                            && sf.fragment.content() == fragment.content())
                })
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
/// Receives pre-extracted [`domain::semantic_dup::CodeFragment`]s (via [`BuildIndexCommand::fragments`],
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
    /// Embed all fragments, sample bounded cross-file fragment pairs, compute
    /// cosine similarities on the sample, and return the resulting distribution
    /// as [`QualityMetrics`].
    ///
    /// Similarities are computed only on **cross-file pairs** (fragments from
    /// different source paths), which is the PoC proxy for false-positive risk
    /// (AC-03/IN-05).  Self-pairs (same source path) are always excluded.
    ///
    /// When fewer than two fragments are present, or no cross-file pairs
    /// exist, all metrics are returned as `0.0`.
    ///
    /// ## Implementation: cross-file-pair-count-based branching (AC-03 conformant)
    ///
    /// The method is **deterministic** for a given input: all random choices
    /// derive from a seed that is a pure function of `n` (the fragment count),
    /// with no wall-clock or external entropy, satisfying CN-04/AC-03.
    ///
    /// Let `cross_file_pairs` = C(n,2) − Σ C(|bucket|,2) where each bucket
    /// groups fragments sharing the same `source_path`.  This count is computed
    /// in O(n) without any O(n²) scan.
    ///
    /// - **Exact branch** (`cross_file_pairs ≤ PAIR_BUDGET`): enumerate
    ///   cross-file pairs DIRECTLY by iterating over each unordered pair of
    ///   distinct buckets and their cartesian product, computing one cosine per
    ///   pair.  Produces exactly `cross_file_pairs` cosines in O(cross_file_pairs).
    ///   Metrics are exact (no sampling approximation).
    ///
    /// - **Sampled branch** (`cross_file_pairs > PAIR_BUDGET`): O(b)-preprocessing
    ///   row-CDF two-step sampling (b = number of distinct source paths; b ≤ n).
    ///   A b-entry row CDF is built in O(b); each draw uses `lemire_exact_u128`
    ///   (bias-free, Lemire 2019 scaled to u128), an O(log b) binary search, and O(1) arithmetic.
    ///   Every draw is a valid cross-file pair; the budget is always filled
    ///   regardless of same-file-pair density.  Seeded from `(n as u64) | 1`
    ///   (deterministic).  Metrics are computed from the sample (an unbiased
    ///   estimate per AC-03).
    ///
    /// All metrics (mean, population std-dev, 7 percentiles, above-threshold
    /// rate) are derived from the collected sample `Vec<f32>`.
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
        // Embed all fragments (O(n) — linear and necessary).
        let embeddings: Vec<Vec<f32>> = cmd
            .fragments
            .iter()
            .map(|f| self.embedding_port.embed(f).map_err(MeasureQualityError::from))
            .collect::<Result<Vec<_>, _>>()?;

        // Default threshold for the above-threshold-rate proxy (0.8 is the
        // typical starting point for the soft gate; calibrated from the
        // percentile distribution).
        let default_threshold: f32 = 0.8;
        // Budget for the number of cross-file cosine computations.
        // When total cross-file pairs ≤ PAIR_BUDGET the exact branch is taken.
        const PAIR_BUDGET: usize = 10_000;

        let n = cmd.fragments.len();
        let sample = sample_cross_file_cosines(&cmd.fragments, &embeddings, n, PAIR_BUDGET);

        if sample.is_empty() {
            return Ok(QualityMetrics {
                mean_cosine: 0.0,
                cosine_std_dev: 0.0,
                cosine_percentiles: vec![0.0; 7],
                above_threshold_rate: 0.0,
            });
        }

        // Compute mean and population variance over the sample.
        let count = sample.len();
        let mean_f64: f64 = sample.iter().map(|&x| f64::from(x)).sum::<f64>() / count as f64;
        let variance_f64: f64 = sample
            .iter()
            .map(|&x| {
                let d = f64::from(x) - mean_f64;
                d * d
            })
            .sum::<f64>()
            / count as f64;
        let mean_cosine = mean_f64 as f32;
        let cosine_std_dev = variance_f64.sqrt() as f32;

        // Percentiles from sorted sample.
        let mut sorted = sample.clone();
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

        // above_threshold_rate: fraction of sample entries at or above threshold.
        let above = sample.iter().filter(|&&s| s >= default_threshold).count();
        let above_threshold_rate = above as f32 / sample.len() as f32;

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

/// Advance a XorShift64 PRNG state by one step.
///
/// The caller must ensure `state != 0` before the first call.
/// The standard XorShift64 parameters (13, 7, 17) are used.
#[inline]
fn xorshift64_next(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

/// Draw a bias-free uniform random index in `[0, range)` for a u128 `range`
/// using two XorShift64 draws combined into a 128-bit value.
///
/// Applies the Lemire 2019 multiply-high approach scaled to 128-bit inputs,
/// with a rejection step to eliminate residual bias.  `range` must be > 0.
fn lemire_exact_u128(rng: &mut u64, range: u128) -> u128 {
    // Draw a 128-bit random word from two consecutive u64 PRNG values.
    let draw_u128 = |rng: &mut u64| -> u128 {
        let lo = xorshift64_next(rng) as u128;
        let hi = xorshift64_next(rng) as u128;
        (hi << 64) | lo
    };
    // floor(x * range / 2^128): 4-limb schoolbook multiply, keeping top 128 bits.
    // x = x_hi*2^64 + x_lo, range = r_hi*2^64 + r_lo; mid carries accumulate at 2^64.
    let mul_high_u128 = |x: u128| -> u128 {
        let (x_lo, x_hi) = (x & (u64::MAX as u128), x >> 64);
        let (r_lo, r_hi) = (range & (u64::MAX as u128), range >> 64);
        let (ll, lh, hl, hh) = (
            x_lo.wrapping_mul(r_lo),
            x_lo.wrapping_mul(r_hi),
            x_hi.wrapping_mul(r_lo),
            x_hi.wrapping_mul(r_hi),
        );
        let mid =
            (ll >> 64).wrapping_add(lh & (u64::MAX as u128)).wrapping_add(hl & (u64::MAX as u128));
        hh.wrapping_add(lh >> 64).wrapping_add(hl >> 64).wrapping_add(mid >> 64)
    };

    let mut x = draw_u128(rng);
    let mut m = mul_high_u128(x);
    // x * range mod 2^128 (low 128 bits of the 256-bit product).
    let mut leftover = x.wrapping_mul(range);
    if leftover < range {
        let threshold = range.wrapping_neg() % range;
        while leftover < threshold {
            x = draw_u128(rng);
            m = mul_high_u128(x);
            leftover = x.wrapping_mul(range);
        }
    }
    m
}

/// Collect a bounded sample of cross-file cosine similarities from `fragments`
/// and their pre-computed `embeddings`.
///
/// Returns a `Vec<f32>` of at most `pair_budget` cosine similarity values,
/// computed only for cross-file pairs (different `source_path`).
///
/// ## Algorithm
///
/// Fragments are grouped into buckets by `source_path` using O(n log n)
/// sort-then-contiguous-run grouping.  An index vector `order` (length n) is
/// sorted by `fragments[i].source_path`, then walked once (O(n)) to form
/// contiguous runs of equal `source_path` into buckets.  This is deterministic
/// and uses no external dependencies.
///
/// Let `cross_file_pairs` = C(n,2) − Σ C(|bucket|,2).
///
/// - **Exact branch** (`cross_file_pairs ≤ pair_budget`): enumerate
///   cross-file pairs DIRECTLY by iterating over each unordered pair of
///   distinct buckets and their cartesian product.  Produces exactly
///   `cross_file_pairs` cosines in O(cross_file_pairs) — no O(n²) scan.
///   This is correct even when `total_pairs = n*(n-1)/2 > pair_budget` but
///   `cross_file_pairs ≤ pair_budget` (sparse cross-file corpus).
///
/// - **Sampled branch** (`cross_file_pairs > pair_budget`): O(b)-preprocessing
///   row-CDF two-step sampling (b = number of distinct buckets, b ≤ n).  A
///   b-entry row CDF is built where row_weight[i] = |bucket_i| × tail_count[i],
///   uniformly distributing the selection over all cross-file pairs in O(b).
///   Each draw uses `lemire_exact_u128` (exact bias-free Lemire 2019, u128 range) to pick a
///   row-CDF position, an O(log b) binary search to find the row bucket, then
///   O(1) arithmetic to resolve the specific pair.  Every draw is a valid
///   cross-file pair; the budget is always filled regardless of same-file-pair
///   density.  Total: O(b + pair_budget × log b) time, O(b) space.  Seeded
///   from `(n as u64) | 1` (deterministic).
///
/// Returns an empty `Vec` when `n < 2` or no cross-file pairs exist.
fn sample_cross_file_cosines(
    fragments: &[domain::semantic_dup::CodeFragment],
    embeddings: &[Vec<f32>],
    n: usize,
    pair_budget: usize,
) -> Vec<f32> {
    if n < 2 || pair_budget == 0 {
        return Vec::new();
    }

    // Group fragment indices by source_path using O(n log n) sort + O(n) contiguous-run walk.
    // Build an index vector and sort it by source_path — total order, no panic.
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| {
        // Guarded access: both indices are in [0, n) by construction.
        let path_a = fragments.get(a).map(|f| f.source_path.as_path());
        let path_b = fragments.get(b).map(|f| f.source_path.as_path());
        match (path_a, path_b) {
            (Some(pa), Some(pb)) => pa.cmp(pb),
            (None, Some(_)) => std::cmp::Ordering::Less,
            (Some(_), None) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        }
    });

    // Walk sorted order once, grouping contiguous runs of equal source_path into buckets.
    // Each bucket is Vec<usize> of original fragment indices sharing a source_path.
    let mut buckets: Vec<Vec<usize>> = Vec::new();
    let mut prev_path: Option<&std::path::Path> = None;
    for &i in &order {
        let Some(f) = fragments.get(i) else { continue };
        let path = f.source_path.as_path();
        if prev_path == Some(path) {
            // Extend the last bucket (same source_path as previous).
            if let Some(last) = buckets.last_mut() {
                last.push(i);
            }
        } else {
            // New source_path: start a new bucket.
            buckets.push(vec![i]);
            prev_path = Some(path);
        }
    }

    // Compute cross_file_pairs = C(n,2) - sum_over_buckets C(|bucket|,2).
    // Use u128 to avoid overflow on large n.
    let n_u128 = n as u128;
    let total_pairs_u128: u128 = n_u128.saturating_mul(n_u128.saturating_sub(1)) / 2;
    let same_file_pairs_u128: u128 = buckets.iter().fold(0u128, |acc, indices| {
        let m = indices.len() as u128;
        acc.saturating_add(m.saturating_mul(m.saturating_sub(1)) / 2)
    });
    let cross_file_pairs_u128 = total_pairs_u128.saturating_sub(same_file_pairs_u128);

    if cross_file_pairs_u128 == 0 {
        return Vec::new();
    }

    // Branch decision is on cross_file_pairs, not total_pairs.
    let cross_file_pairs_fits = cross_file_pairs_u128 <= pair_budget as u128;

    if cross_file_pairs_fits {
        // Exact branch: enumerate cross-file pairs DIRECTLY via distinct-bucket
        // cartesian products.  O(cross_file_pairs) — no O(n²) all-pairs scan.
        let capacity = cross_file_pairs_u128 as usize;
        let mut sample = Vec::with_capacity(capacity);
        for bi in 0..buckets.len() {
            for bj in (bi + 1)..buckets.len() {
                // Both bi and bj are in-bounds by construction (loop bounds).
                let Some(bucket_i) = buckets.get(bi) else { continue };
                let Some(bucket_j) = buckets.get(bj) else { continue };
                for &idx_i in bucket_i {
                    for &idx_j in bucket_j {
                        let Some(ei) = embeddings.get(idx_i) else { continue };
                        let Some(ej) = embeddings.get(idx_j) else { continue };
                        sample.push(cosine_similarity(ei, ej));
                    }
                }
            }
        }
        sample
    } else {
        // Sampled branch: O(b) preprocessing, O(log b) per draw (b = distinct buckets).
        // Row CDF: row_cdf[i] = Σ(k≤i) |bucket_k| × tail_count[k], where
        // tail_count[k] = Σ(j>k) |bucket_j|.  Each draw uses lemire_exact_u128 to pick a
        // position, binary-searches row_cdf to find bi, then resolves
        // (r_in_row / tc_i, r_in_row % tc_i) to the exact (fragment_in_bi,
        // tail_fragment) pair.  Always fills budget; no cross-file pair rejection.
        // Seed is a pure function of n — deterministic, no wall-clock.
        let mut rng_state: u64 = (n as u64) | 1;
        let nb = buckets.len();

        // Bucket sizes and start offsets within `order` (sorted by source_path).
        let mut bucket_sizes: Vec<usize> = Vec::with_capacity(nb);
        let mut bucket_starts: Vec<usize> = Vec::with_capacity(nb + 1);
        let mut start = 0usize;
        for b in &buckets {
            bucket_starts.push(start);
            bucket_sizes.push(b.len());
            start = start.saturating_add(b.len());
        }
        bucket_starts.push(n);

        // tail_count[i] = Σ(k>i) bucket_sizes[k].
        let mut tail_count: Vec<usize> = vec![0usize; nb];
        let mut tail = 0usize;
        for i in (0..nb).rev() {
            if let Some(slot) = tail_count.get_mut(i) {
                *slot = tail;
            }
            tail = tail.saturating_add(bucket_sizes.get(i).copied().unwrap_or(0));
        }

        // Build row CDF using u128 to match cross_file_pairs_u128 range.
        let mut row_cdf: Vec<u128> = Vec::with_capacity(nb);
        let mut cumulative: u128 = 0;
        for i in 0..nb {
            let w = (bucket_sizes.get(i).copied().unwrap_or(0) as u128)
                .saturating_mul(tail_count.get(i).copied().unwrap_or(0) as u128);
            cumulative = cumulative.saturating_add(w);
            row_cdf.push(cumulative);
        }
        let total_row_weight = cumulative;
        if total_row_weight == 0 {
            return Vec::new();
        }
        let mut sample = Vec::with_capacity(pair_budget);
        while sample.len() < pair_budget {
            // lemire_exact_u128 draws a bias-free uniform value in [0, total_row_weight).
            // Uses u128 range to handle cross_file_pairs that exceed u64::MAX.
            let r = lemire_exact_u128(&mut rng_state, total_row_weight);
            let bi = row_cdf.partition_point(|&cum| cum <= r);
            let Some(&bsz_i) = bucket_sizes.get(bi) else { continue };
            let Some(&tc_i) = tail_count.get(bi) else { continue };
            if bsz_i == 0 || tc_i == 0 {
                continue;
            }
            let prev_cum = if bi > 0 { row_cdf.get(bi - 1).copied().unwrap_or(0) } else { 0 };
            // r_in_row stays in u128; ki and r_tail fit in usize (bounded by bucket sizes).
            let r_in_row = r.saturating_sub(prev_cum);
            let ki = (r_in_row / tc_i as u128) as usize;
            let r_tail = (r_in_row % tc_i as u128) as usize;
            let tail_start = bucket_starts.get(bi + 1).copied().unwrap_or(n);
            let order_pos = tail_start + r_tail;
            let Some(bucket_i) = buckets.get(bi) else { continue };
            let Some(&idx_i) = bucket_i.get(ki) else { continue };
            let Some(&idx_j) = order.get(order_pos) else { continue };
            let Some(ei) = embeddings.get(idx_i) else { continue };
            let Some(ej) = embeddings.get(idx_j) else { continue };
            sample.push(cosine_similarity(ei, ej));
        }

        sample
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::indexing_slicing,
        clippy::type_complexity
    )]

    use std::path::PathBuf;
    use std::sync::Arc;

    use domain::semantic_dup::{CodeFragment, SimilarFragment, SimilarityScore};
    use mockall::mock;

    use super::*;
    use crate::semantic_dup::errors::{EmbeddingError, SemanticIndexError};

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
        let threshold = domain::semantic_dup::SimilarityThreshold::new(0.8).unwrap();
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
        let threshold = domain::semantic_dup::SimilarityThreshold::new(0.8).unwrap();
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
        let threshold = domain::semantic_dup::SimilarityThreshold::new(0.8).unwrap();
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
        let threshold = domain::semantic_dup::SimilarityThreshold::new(0.8).unwrap();

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
        let threshold = domain::semantic_dup::SimilarityThreshold::new(0.8).unwrap();
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
        let threshold = domain::semantic_dup::SimilarityThreshold::new(0.8).unwrap();
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
        let threshold = domain::semantic_dup::SimilarityThreshold::new(0.8).unwrap();
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

    #[test]
    fn test_dup_check_self_match_excluded_real_dup_retained() {
        // When the index returns both an exact self-match (same source_path AND
        // same content) and a genuine near-duplicate (different source_path),
        // only the genuine near-duplicate should appear in the warning.
        let threshold = domain::semantic_dup::SimilarityThreshold::new(0.8).unwrap();
        let frag = make_fragment("src/new.rs", "fn new_impl() {}");

        // Self-match: same path and same content, score 1.0.
        let self_match = make_similar_fragment("src/new.rs", "fn new_impl() {}", 1.0);
        // Genuine duplicate: different path, score 0.9 >= threshold.
        let real_dup = make_similar_fragment("src/existing.rs", "fn existing_impl() {}", 0.9);

        let mut mock_embed = MockMockEmbeddingPort::new();
        mock_embed.expect_embed().times(1).returning(|_| Ok(vec![0.1, 0.2]));

        let mut mock_index = MockMockSemanticIndexPort::new();
        {
            let results = vec![self_match, real_dup.clone()];
            mock_index.expect_search().times(1).returning(move |_, _| Ok(results.clone()));
        }

        let interactor = DupCheckInteractor::new(Arc::new(mock_embed), Arc::new(mock_index));
        let output =
            interactor.dup_check(&DupCheckCommand { fragments: vec![frag], threshold }).unwrap();

        assert_eq!(output.warnings.len(), 1, "expected one warning (self-match removed)");
        let sims = &output.warnings[0].similar_fragments;
        assert_eq!(sims.len(), 1, "only the real duplicate should remain");
        assert_eq!(sims[0].fragment.source_path, std::path::PathBuf::from("src/existing.rs"));
        assert!(
            (sims[0].score.value() - 0.9).abs() < 1e-6,
            "real dup score should be 0.9, got {}",
            sims[0].score.value()
        );
    }

    #[test]
    fn test_dup_check_only_self_match_produces_no_warning() {
        // When the index returns only the exact self-match, the warning list
        // should be empty (no genuine near-duplicates found).
        let threshold = domain::semantic_dup::SimilarityThreshold::new(0.8).unwrap();
        let frag = make_fragment("src/new.rs", "fn new_impl() {}");

        // Only the self-match is returned.
        let self_match = make_similar_fragment("src/new.rs", "fn new_impl() {}", 1.0);

        let mut mock_embed = MockMockEmbeddingPort::new();
        mock_embed.expect_embed().times(1).returning(|_| Ok(vec![0.1, 0.2]));

        let mut mock_index = MockMockSemanticIndexPort::new();
        mock_index.expect_search().times(1).returning(move |_, _| Ok(vec![self_match.clone()]));

        let interactor = DupCheckInteractor::new(Arc::new(mock_embed), Arc::new(mock_index));
        let output =
            interactor.dup_check(&DupCheckCommand { fragments: vec![frag], threshold }).unwrap();

        assert!(output.warnings.is_empty(), "self-match only should produce no warning");
    }

    #[test]
    fn test_dup_check_same_path_different_content_is_not_excluded() {
        // A candidate with the same source_path but different content is a
        // legitimate intra-file or modified-version near-duplicate and must
        // NOT be excluded.
        let threshold = domain::semantic_dup::SimilarityThreshold::new(0.8).unwrap();
        let frag = make_fragment("src/new.rs", "fn new_impl() {}");

        // Same path, but different content (a different function in the same file).
        let intra_file_dup =
            make_similar_fragment("src/new.rs", "fn other_impl_in_same_file() {}", 0.85);

        let mut mock_embed = MockMockEmbeddingPort::new();
        mock_embed.expect_embed().times(1).returning(|_| Ok(vec![0.1, 0.2]));

        let mut mock_index = MockMockSemanticIndexPort::new();
        {
            let results = vec![intra_file_dup];
            mock_index.expect_search().times(1).returning(move |_, _| Ok(results.clone()));
        }

        let interactor = DupCheckInteractor::new(Arc::new(mock_embed), Arc::new(mock_index));
        let output =
            interactor.dup_check(&DupCheckCommand { fragments: vec![frag], threshold }).unwrap();

        assert_eq!(output.warnings.len(), 1, "intra-file near-duplicate should be reported");
        assert_eq!(output.warnings[0].similar_fragments.len(), 1);
        assert_eq!(
            output.warnings[0].similar_fragments[0].fragment.source_path,
            std::path::PathBuf::from("src/new.rs")
        );
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
        use mockall::predicate;

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

    // ── MeasureQualityInteractor: streaming / reservoir correctness ───────────
    //
    // These tests verify that the Welford streaming mean/variance and the
    // reservoir-based threshold-rate produce the same values as a naïve
    // materialised-Vec approach for small inputs (where the two are
    // mathematically identical), and that they are well-behaved for larger
    // synthetic inputs.

    #[test]
    fn test_measure_quality_streaming_mean_matches_naive_for_four_fragments() {
        // Four fragments on four distinct paths → six cross-file pairs.
        // All embeddings identical ([1,0]) → all cosine similarities = 1.0.
        let n = 4;
        let frags: Vec<CodeFragment> = (0..n)
            .map(|i| make_fragment(&format!("src/{i}.rs"), &format!("fn f{i}() {{}}")))
            .collect();

        let mut mock_embed = MockMockEmbeddingPort::new();
        mock_embed.expect_embed().times(n).returning(|_| Ok(vec![1.0_f32, 0.0]));
        let mock_index = MockMockSemanticIndexPort::new();

        let interactor = MeasureQualityInteractor::new(Arc::new(mock_embed), Arc::new(mock_index));
        let metrics =
            interactor.measure_quality(&MeasureQualityCommand { fragments: frags }).unwrap();

        // All similarities = 1.0 → mean = 1.0, std_dev = 0.0, all above threshold.
        assert!(
            (metrics.mean_cosine - 1.0).abs() < 1e-4,
            "expected mean_cosine = 1.0, got {}",
            metrics.mean_cosine
        );
        assert!(
            metrics.cosine_std_dev.abs() < 1e-4,
            "expected cosine_std_dev = 0.0, got {}",
            metrics.cosine_std_dev
        );
        assert!(
            (metrics.above_threshold_rate - 1.0).abs() < 1e-4,
            "expected above_threshold_rate = 1.0, got {}",
            metrics.above_threshold_rate
        );
    }

    #[test]
    fn test_measure_quality_streaming_std_dev_correct_for_two_known_pairs() {
        // Three fragments on three distinct paths → three cross-file pairs.
        // Embeddings chosen so cosine similarities are 1.0, 0.0, 0.0.
        //   a=[1,0], b=[1,0]: cosine = 1.0
        //   a=[1,0], c=[0,1]: cosine = 0.0
        //   b=[1,0], c=[0,1]: cosine = 0.0
        // Mean = (1.0 + 0.0 + 0.0) / 3 ≈ 0.3333
        // Population variance = ((1-mean)^2 + (0-mean)^2 + (0-mean)^2) / 3
        let frags = vec![
            make_fragment("src/a.rs", "fn a() {}"),
            make_fragment("src/b.rs", "fn b() {}"),
            make_fragment("src/c.rs", "fn c() {}"),
        ];
        let embeds = vec![vec![1.0_f32, 0.0], vec![1.0_f32, 0.0], vec![0.0_f32, 1.0]];

        let mut call_count = 0usize;
        let mut mock_embed = MockMockEmbeddingPort::new();
        let embeds_clone = embeds.clone();
        mock_embed.expect_embed().times(3).returning(move |_| {
            let e = embeds_clone[call_count].clone();
            call_count += 1;
            Ok(e)
        });
        let mock_index = MockMockSemanticIndexPort::new();

        let interactor = MeasureQualityInteractor::new(Arc::new(mock_embed), Arc::new(mock_index));
        let metrics =
            interactor.measure_quality(&MeasureQualityCommand { fragments: frags }).unwrap();

        let expected_mean = 1.0_f32 / 3.0;
        let expected_variance = ((1.0 - expected_mean).powi(2)
            + (0.0 - expected_mean).powi(2)
            + (0.0 - expected_mean).powi(2))
            / 3.0;
        let expected_std_dev = expected_variance.sqrt();

        assert!(
            (metrics.mean_cosine - expected_mean).abs() < 1e-4,
            "mean_cosine mismatch: expected {expected_mean}, got {}",
            metrics.mean_cosine
        );
        assert!(
            (metrics.cosine_std_dev - expected_std_dev).abs() < 1e-4,
            "cosine_std_dev mismatch: expected {expected_std_dev}, got {}",
            metrics.cosine_std_dev
        );
        // 1 of 3 pairs is above threshold 0.8 → rate ≈ 0.333.
        assert!(
            (metrics.above_threshold_rate - 1.0_f32 / 3.0).abs() < 1e-4,
            "above_threshold_rate mismatch: expected ≈ 0.333, got {}",
            metrics.above_threshold_rate
        );
    }

    #[test]
    fn test_measure_quality_streaming_returns_seven_percentiles() {
        // Minimal test: two fragments → one pair → all percentiles = that pair's similarity.
        let frags =
            vec![make_fragment("src/a.rs", "fn a() {}"), make_fragment("src/b.rs", "fn b() {}")];

        let mut mock_embed = MockMockEmbeddingPort::new();
        mock_embed.expect_embed().times(2).returning(|_| Ok(vec![1.0_f32, 0.0]));
        let mock_index = MockMockSemanticIndexPort::new();

        let interactor = MeasureQualityInteractor::new(Arc::new(mock_embed), Arc::new(mock_index));
        let metrics =
            interactor.measure_quality(&MeasureQualityCommand { fragments: frags }).unwrap();

        assert_eq!(
            metrics.cosine_percentiles.len(),
            7,
            "QualityMetrics must always return exactly 7 percentile values"
        );
    }

    #[test]
    fn test_measure_quality_no_memory_explosion_for_large_synthetic_corpus() {
        // 200 fragments on 200 distinct paths → 200*199/2 = 19_900 cross-file pairs.
        // All embeddings are unit vectors [1,0] → all similarities = 1.0.
        // The streaming path must handle this without materialising 19_900 floats.
        // We verify correctness, not memory usage (no way to assert heap usage in tests).
        let n = 200usize;
        let frags: Vec<CodeFragment> = (0..n)
            .map(|i| make_fragment(&format!("src/{i}.rs"), &format!("fn f{i}() {{}}")))
            .collect();

        let mut mock_embed = MockMockEmbeddingPort::new();
        mock_embed.expect_embed().times(n).returning(|_| Ok(vec![1.0_f32, 0.0]));
        let mock_index = MockMockSemanticIndexPort::new();

        let interactor = MeasureQualityInteractor::new(Arc::new(mock_embed), Arc::new(mock_index));
        let metrics =
            interactor.measure_quality(&MeasureQualityCommand { fragments: frags }).unwrap();

        // All similarities = 1.0 → mean = 1.0, std_dev = 0.0, rate = 1.0.
        assert!(
            (metrics.mean_cosine - 1.0).abs() < 1e-4,
            "mean_cosine should be 1.0 for identical unit vectors, got {}",
            metrics.mean_cosine
        );
        assert!(
            metrics.cosine_std_dev.abs() < 1e-4,
            "cosine_std_dev should be 0.0 for identical vectors, got {}",
            metrics.cosine_std_dev
        );
        assert!(
            (metrics.above_threshold_rate - 1.0).abs() < 1e-4,
            "above_threshold_rate should be 1.0 when all similarities = 1.0, got {}",
            metrics.above_threshold_rate
        );
        assert_eq!(metrics.cosine_percentiles.len(), 7);
    }

    // ── MeasureQualityInteractor: bounded pair-sampling behaviour ─────────────

    #[test]
    fn test_measure_quality_deterministic_same_input_yields_identical_metrics() {
        // Determinism: two calls with the same fragments must return identical metrics.
        // Uses 200 fragments on 200 distinct paths so cross_file_pairs = 19_900 >
        // PAIR_BUDGET (10_000), exercising the sampled branch.
        let n = 200usize;
        let frags: Vec<CodeFragment> = (0..n)
            .map(|i| make_fragment(&format!("src/{i}.rs"), &format!("fn f{i}() {{}}")))
            .collect();

        // Alternating unit vectors so cosine similarities are either 1.0 or 0.0,
        // giving a non-trivial distribution (not all-1.0).
        let embeds: Vec<Vec<f32>> = (0..n)
            .map(|i| if i % 2 == 0 { vec![1.0_f32, 0.0] } else { vec![0.0_f32, 1.0] })
            .collect();

        // Build both interactors with separate mock objects returning the same data.
        let mut mock_embed1 = MockMockEmbeddingPort::new();
        let embeds1 = embeds.clone();
        {
            let mut c = 0usize;
            mock_embed1.expect_embed().times(n).returning(move |_| {
                let v = embeds1[c].clone();
                c += 1;
                Ok(v)
            });
        }

        let mut mock_embed2 = MockMockEmbeddingPort::new();
        let embeds2 = embeds.clone();
        {
            let mut c = 0usize;
            mock_embed2.expect_embed().times(n).returning(move |_| {
                let v = embeds2[c].clone();
                c += 1;
                Ok(v)
            });
        }

        let interactor1 = MeasureQualityInteractor::new(
            Arc::new(mock_embed1),
            Arc::new(MockMockSemanticIndexPort::new()),
        );
        let interactor2 = MeasureQualityInteractor::new(
            Arc::new(mock_embed2),
            Arc::new(MockMockSemanticIndexPort::new()),
        );

        let cmd = MeasureQualityCommand { fragments: frags };
        let m1 = interactor1.measure_quality(&cmd).unwrap();
        let m2 = interactor2.measure_quality(&cmd).unwrap();

        assert_eq!(
            m1.mean_cosine, m2.mean_cosine,
            "mean_cosine must be identical across two calls with the same input"
        );
        assert_eq!(
            m1.cosine_std_dev, m2.cosine_std_dev,
            "cosine_std_dev must be identical across two calls with the same input"
        );
        assert_eq!(
            m1.above_threshold_rate, m2.above_threshold_rate,
            "above_threshold_rate must be identical across two calls with the same input"
        );
        assert_eq!(
            m1.cosine_percentiles, m2.cosine_percentiles,
            "cosine_percentiles must be identical across two calls with the same input"
        );
    }

    #[test]
    fn test_measure_quality_small_input_exact_metrics_match_hand_computed() {
        // Small-input exactness: 3 fragments on 3 paths → 3 cross-file pairs.
        // cross_file_pairs = 3 ≤ PAIR_BUDGET (10_000) → exact branch.
        // Embeddings:
        //   a = [1, 0], b = [1, 0], c = [0, 1]
        // Cross-file cosines:
        //   (a,b) = 1.0,  (a,c) = 0.0,  (b,c) = 0.0
        // Mean = 1/3, population variance = ((1-1/3)^2 + 2*(0-1/3)^2) / 3
        let frags = vec![
            make_fragment("src/a.rs", "fn a() {}"),
            make_fragment("src/b.rs", "fn b() {}"),
            make_fragment("src/c.rs", "fn c() {}"),
        ];
        let embeds_data = vec![vec![1.0_f32, 0.0], vec![1.0_f32, 0.0], vec![0.0_f32, 1.0]];

        let mut call_idx = 0usize;
        let mut mock_embed = MockMockEmbeddingPort::new();
        let embeds_clone = embeds_data.clone();
        mock_embed.expect_embed().times(3).returning(move |_| {
            let v = embeds_clone[call_idx].clone();
            call_idx += 1;
            Ok(v)
        });
        let mock_index = MockMockSemanticIndexPort::new();

        let interactor = MeasureQualityInteractor::new(Arc::new(mock_embed), Arc::new(mock_index));
        let metrics =
            interactor.measure_quality(&MeasureQualityCommand { fragments: frags }).unwrap();

        // Hand-computed expected values.
        let expected_mean = 1.0_f32 / 3.0;
        let expected_variance = ((1.0 - expected_mean).powi(2)
            + (0.0 - expected_mean).powi(2)
            + (0.0 - expected_mean).powi(2))
            / 3.0;
        let expected_std_dev = expected_variance.sqrt();

        assert!(
            (metrics.mean_cosine - expected_mean).abs() < 1e-5,
            "mean_cosine mismatch: expected {expected_mean}, got {}",
            metrics.mean_cosine
        );
        assert!(
            (metrics.cosine_std_dev - expected_std_dev).abs() < 1e-5,
            "cosine_std_dev mismatch: expected {expected_std_dev}, got {}",
            metrics.cosine_std_dev
        );
        // 1 of 3 sampled cosines >= 0.8 → above_threshold_rate = 1/3.
        assert!(
            (metrics.above_threshold_rate - 1.0_f32 / 3.0).abs() < 1e-5,
            "above_threshold_rate mismatch: expected ≈ 0.3333, got {}",
            metrics.above_threshold_rate
        );
        assert_eq!(metrics.cosine_percentiles.len(), 7);
    }

    #[test]
    fn test_measure_quality_less_than_two_fragments_returns_zero_metrics() {
        // < 2 fragments → no pairs possible → all-zero metrics.
        // Zero fragments case.
        let mock_embed_empty = MockMockEmbeddingPort::new();
        let interactor_empty = MeasureQualityInteractor::new(
            Arc::new(mock_embed_empty),
            Arc::new(MockMockSemanticIndexPort::new()),
        );
        let m_empty =
            interactor_empty.measure_quality(&MeasureQualityCommand { fragments: vec![] }).unwrap();
        assert_eq!(m_empty.mean_cosine, 0.0);
        assert_eq!(m_empty.cosine_std_dev, 0.0);
        assert_eq!(m_empty.above_threshold_rate, 0.0);
        assert_eq!(m_empty.cosine_percentiles, vec![0.0; 7]);

        // One fragment case.
        let frag = make_fragment("src/a.rs", "fn a() {}");
        let mut mock_embed_one = MockMockEmbeddingPort::new();
        mock_embed_one.expect_embed().times(1).returning(|_| Ok(vec![1.0, 0.0]));
        let interactor_one = MeasureQualityInteractor::new(
            Arc::new(mock_embed_one),
            Arc::new(MockMockSemanticIndexPort::new()),
        );
        let m_one = interactor_one
            .measure_quality(&MeasureQualityCommand { fragments: vec![frag] })
            .unwrap();
        assert_eq!(m_one.mean_cosine, 0.0);
        assert_eq!(m_one.cosine_std_dev, 0.0);
        assert_eq!(m_one.above_threshold_rate, 0.0);
    }

    #[test]
    fn test_measure_quality_same_file_pairs_excluded_returns_zero_metrics() {
        // All fragments on the same source_path → no cross-file pairs → all-zero.
        let frags = vec![
            make_fragment("src/a.rs", "fn x() {}"),
            make_fragment("src/a.rs", "fn y() {}"),
            make_fragment("src/a.rs", "fn z() {}"),
        ];
        let mut mock_embed = MockMockEmbeddingPort::new();
        mock_embed.expect_embed().times(3).returning(|_| Ok(vec![1.0_f32, 0.0]));
        let mock_index = MockMockSemanticIndexPort::new();

        let interactor = MeasureQualityInteractor::new(Arc::new(mock_embed), Arc::new(mock_index));
        let metrics =
            interactor.measure_quality(&MeasureQualityCommand { fragments: frags }).unwrap();

        assert_eq!(metrics.mean_cosine, 0.0, "same-path-only input must yield zero mean");
        assert_eq!(metrics.cosine_std_dev, 0.0);
        assert_eq!(metrics.above_threshold_rate, 0.0);
        assert_eq!(metrics.cosine_percentiles, vec![0.0; 7]);
    }

    #[test]
    fn test_measure_quality_large_corpus_sampled_branch_returns_finite_capped_metrics() {
        // Large-input bounding: n=200 → total_pairs=19_900 > PAIR_BUDGET (10_000).
        // Exercises the sampled branch.  Asserts:
        // (a) the call completes without panic,
        // (b) all metric values are in the expected [-1, 1] range (finite, bounded),
        // (c) exactly 7 percentiles are returned.
        //
        // Embeddings alternate between two orthogonal unit vectors so cosines
        // are either 1.0 (same-direction pairs) or 0.0 (orthogonal pairs) —
        // both in [-1, 1], giving a non-trivial but predictable distribution.
        let n = 200usize;
        let frags: Vec<CodeFragment> = (0..n)
            .map(|i| make_fragment(&format!("src/{i}.rs"), &format!("fn f{i}() {{}}")))
            .collect();
        let mut mock_embed = MockMockEmbeddingPort::new();
        {
            let mut c = 0usize;
            mock_embed.expect_embed().times(n).returning(move |_| {
                let v = if c % 2 == 0 { vec![1.0_f32, 0.0] } else { vec![0.0_f32, 1.0] };
                c += 1;
                Ok(v)
            });
        }
        let mock_index = MockMockSemanticIndexPort::new();

        let interactor = MeasureQualityInteractor::new(Arc::new(mock_embed), Arc::new(mock_index));
        let metrics = interactor
            .measure_quality(&MeasureQualityCommand { fragments: frags })
            .expect("measure_quality must not fail on large corpus");

        assert!(
            (-1.0_f32..=1.0).contains(&metrics.mean_cosine),
            "mean_cosine out of [-1,1]: {}",
            metrics.mean_cosine
        );
        assert!(
            (0.0_f32..=1.0).contains(&metrics.cosine_std_dev),
            "cosine_std_dev out of [0,1]: {}",
            metrics.cosine_std_dev
        );
        assert!(
            (0.0_f32..=1.0).contains(&metrics.above_threshold_rate),
            "above_threshold_rate out of [0,1]: {}",
            metrics.above_threshold_rate
        );
        assert_eq!(metrics.cosine_percentiles.len(), 7, "must return exactly 7 percentile values");
        for (i, &p) in metrics.cosine_percentiles.iter().enumerate() {
            assert!((-1.0_f32..=1.0).contains(&p), "cosine_percentiles[{i}] out of [-1,1]: {p}");
        }
    }

    // ── sample_cross_file_cosines unit tests (private helper) ─────────────────

    #[test]
    fn test_sample_cross_file_cosines_exact_branch_two_paths() {
        // 2 fragments on 2 paths → 1 pair → total_pairs (1) ≤ budget → exact branch.
        let frags =
            vec![make_fragment("src/a.rs", "fn a() {}"), make_fragment("src/b.rs", "fn b() {}")];
        let embeddings = vec![vec![1.0_f32, 0.0], vec![1.0_f32, 0.0]];
        let result = sample_cross_file_cosines(&frags, &embeddings, 2, 10_000);
        assert_eq!(result.len(), 1, "must collect exactly 1 cross-file cosine");
        assert!((result[0] - 1.0).abs() < 1e-6, "cosine of identical unit vectors must be 1.0");
    }

    #[test]
    fn test_sample_cross_file_cosines_exact_branch_same_path_excluded() {
        // 2 fragments on the SAME path → 0 cross-file pairs → empty result.
        let frags =
            vec![make_fragment("src/a.rs", "fn a() {}"), make_fragment("src/a.rs", "fn b() {}")];
        let embeddings = vec![vec![1.0_f32, 0.0], vec![1.0_f32, 0.0]];
        let result = sample_cross_file_cosines(&frags, &embeddings, 2, 10_000);
        assert!(result.is_empty(), "same-path pair must be excluded");
    }

    #[test]
    fn test_sample_cross_file_cosines_n_less_than_2_returns_empty() {
        // n < 2 → no pairs → empty result.
        let frags = vec![make_fragment("src/a.rs", "fn a() {}")];
        let embeddings = vec![vec![1.0_f32, 0.0]];
        let result = sample_cross_file_cosines(&frags, &embeddings, 1, 10_000);
        assert!(result.is_empty(), "n<2 must return empty");

        let result_zero = sample_cross_file_cosines(&[], &[], 0, 10_000);
        assert!(result_zero.is_empty(), "n=0 must return empty");
    }

    #[test]
    fn test_sample_cross_file_cosines_sampled_branch_budget_respected() {
        // n=200 on 200 distinct paths → cross_file_pairs = 19_900 > budget (10) → sampled branch.
        // Assert the returned sample has at most `budget` entries.
        let n = 200usize;
        let frags: Vec<_> = (0..n)
            .map(|i| make_fragment(&format!("src/{i}.rs"), &format!("fn f{i}() {{}}")))
            .collect();
        let embeddings: Vec<Vec<f32>> = (0..n).map(|_| vec![1.0_f32, 0.0]).collect();
        let budget = 10usize;
        let result = sample_cross_file_cosines(&frags, &embeddings, n, budget);
        assert!(
            result.len() <= budget,
            "sampled branch must collect at most budget={budget} cosines, got {}",
            result.len()
        );
        // With 200 distinct-path fragments, cross-file pairs are plentiful;
        // the result should reach exactly the budget.
        assert_eq!(
            result.len(),
            budget,
            "sampled branch should reach the full budget when cross-file pairs are plentiful"
        );
    }

    #[test]
    fn test_sample_cross_file_cosines_deterministic_for_same_n() {
        // Two calls with the same fragments and n must return identical Vec<f32>.
        let n = 200usize;
        let frags: Vec<_> = (0..n)
            .map(|i| make_fragment(&format!("src/{i}.rs"), &format!("fn f{i}() {{}}")))
            .collect();
        let embeddings: Vec<Vec<f32>> = (0..n)
            .map(|i| if i % 2 == 0 { vec![1.0_f32, 0.0] } else { vec![0.0_f32, 1.0] })
            .collect();
        let budget = 500usize;
        let r1 = sample_cross_file_cosines(&frags, &embeddings, n, budget);
        let r2 = sample_cross_file_cosines(&frags, &embeddings, n, budget);
        assert_eq!(r1, r2, "sample_cross_file_cosines must be deterministic for same input");
    }

    // ── Sparse-cross-file corpus: the Codex P1 correctness case ──────────────

    #[test]
    fn test_sample_cross_file_cosines_sparse_cross_file_exact_branch_taken() {
        // The Codex P1 finding: a corpus where total_pairs > PAIR_BUDGET but
        // cross_file_pairs ≤ PAIR_BUDGET must take the EXACT branch and measure
        // ALL cross-file pairs — not the biased tiny subset that rejection sampling
        // would produce.
        //
        // Arithmetic (PAIR_BUDGET = 10_000):
        //   File A: 145 fragments → C(145,2) = 145*144/2 = 10_440 (same-file pairs)
        //   File B: 1 fragment   → C(1,2)   = 0
        //   total n = 146
        //   total_pairs  = C(146,2) = 146*145/2 = 10_585  > 10_000  (would force sampled
        //                                                              branch under old code)
        //   cross_file_pairs = 10_585 - 10_440 - 0 = 145  ≤ 10_000  (exact branch correct)
        //
        // All embeddings are identical unit vectors [1,0], so every cross-file
        // cosine = 1.0.  With the exact branch we expect exactly 145 cosines all = 1.0.
        let n_a = 145usize;
        let n_b = 1usize;
        let n = n_a + n_b;

        // PAIR_BUDGET hard-coded to match the constant in measure_quality.
        let pair_budget: usize = 10_000;

        // Verify the arithmetic used in the test comment (not product code, just assertion).
        let total_pairs = n * (n - 1) / 2;
        let same_file_pairs_a = n_a * (n_a - 1) / 2;
        let cross_file_pairs = total_pairs - same_file_pairs_a;
        assert!(total_pairs > pair_budget, "total_pairs={total_pairs} must exceed budget");
        assert!(
            cross_file_pairs <= pair_budget,
            "cross_file_pairs={cross_file_pairs} must be within budget"
        );
        assert_eq!(cross_file_pairs, n_a, "cross_file_pairs should equal n_a={n_a}");

        // Build corpus: first n_a fragments in "src/a.rs", last 1 in "src/b.rs".
        let mut frags: Vec<_> =
            (0..n_a).map(|i| make_fragment("src/a.rs", &format!("fn fa{i}() {{}}"))).collect();
        frags.push(make_fragment("src/b.rs", "fn fb0() {}"));

        // All unit vectors [1,0] → cosine similarity = 1.0 for every pair.
        let embeddings: Vec<Vec<f32>> = (0..n).map(|_| vec![1.0_f32, 0.0]).collect();

        let result = sample_cross_file_cosines(&frags, &embeddings, n, pair_budget);

        // Exact branch: must measure ALL cross_file_pairs cosines (none skipped).
        assert_eq!(
            result.len(),
            cross_file_pairs,
            "exact branch must produce exactly cross_file_pairs={cross_file_pairs} cosines, got {}",
            result.len()
        );
        // Every cosine should be 1.0 (identical unit vectors).
        for (idx, &c) in result.iter().enumerate() {
            assert!(
                (c - 1.0_f32).abs() < 1e-6,
                "cosine[{idx}] should be 1.0 (identical unit vectors), got {c}"
            );
        }
    }

    #[test]
    fn test_measure_quality_sparse_cross_file_corpus_exact_metrics() {
        // Integration-level version of the P1 finding: MeasureQualityInteractor
        // must return exact metrics (not a biased sample) for a sparse-cross-file
        // corpus where total_pairs > PAIR_BUDGET but cross_file_pairs ≤ PAIR_BUDGET.
        //
        // Same arithmetic as the unit test above: 145 fragments in a.rs + 1 in b.rs.
        // cross_file_pairs = 145 ≤ 10_000; all cosines = 1.0 (identical embeddings).
        //
        // Expected metrics from the full 145-pair exact set:
        //   mean_cosine = 1.0, std_dev = 0.0, above_threshold_rate = 1.0.
        let n_a = 145usize;
        let n_b = 1usize;
        let n = n_a + n_b;

        let mut frags: Vec<_> =
            (0..n_a).map(|i| make_fragment("src/a.rs", &format!("fn fa{i}() {{}}"))).collect();
        frags.push(make_fragment("src/b.rs", "fn fb0() {}"));

        let mut mock_embed = MockMockEmbeddingPort::new();
        mock_embed.expect_embed().times(n).returning(|_| Ok(vec![1.0_f32, 0.0]));
        let mock_index = MockMockSemanticIndexPort::new();

        let interactor = MeasureQualityInteractor::new(Arc::new(mock_embed), Arc::new(mock_index));
        let metrics =
            interactor.measure_quality(&MeasureQualityCommand { fragments: frags }).unwrap();

        assert!(
            (metrics.mean_cosine - 1.0).abs() < 1e-4,
            "sparse-cross-file exact branch: mean_cosine must be 1.0, got {}",
            metrics.mean_cosine
        );
        assert!(
            metrics.cosine_std_dev.abs() < 1e-4,
            "sparse-cross-file exact branch: cosine_std_dev must be 0.0, got {}",
            metrics.cosine_std_dev
        );
        assert!(
            (metrics.above_threshold_rate - 1.0).abs() < 1e-4,
            "sparse-cross-file exact branch: above_threshold_rate must be 1.0, got {}",
            metrics.above_threshold_rate
        );
        assert_eq!(metrics.cosine_percentiles.len(), 7);
        for (i, &p) in metrics.cosine_percentiles.iter().enumerate() {
            assert!(
                (p - 1.0).abs() < 1e-4,
                "cosine_percentiles[{i}] must be 1.0 in sparse-cross-file exact branch, got {p}"
            );
        }
    }

    #[test]
    fn test_sample_cross_file_cosines_many_distinct_single_fragment_files_exact_and_deterministic()
    {
        // Exercises the O(n log n) sort-based grouping path with many distinct
        // paths (50 files × 1 fragment each) — the common workspace case.
        //
        // With 50 single-fragment files:
        //   same_file_pairs = Σ C(1,2) = 0
        //   cross_file_pairs = C(50,2) = 50*49/2 = 1_225  ≤  10_000  (exact branch)
        //
        // All embeddings are identical unit vectors [1,0] → every cosine = 1.0.
        // Metrics must be exact (not sampled) and deterministic across two calls.
        let n = 50usize;
        let frags: Vec<_> = (0..n)
            .map(|i| make_fragment(&format!("src/file_{i:02}.rs"), &format!("fn f{i}() {{}}")))
            .collect();
        let embeddings: Vec<Vec<f32>> = (0..n).map(|_| vec![1.0_f32, 0.0]).collect();
        let pair_budget = 10_000usize;

        let expected_cross_file_pairs = n * (n - 1) / 2; // 1_225

        // First call.
        let r1 = sample_cross_file_cosines(&frags, &embeddings, n, pair_budget);
        // Second call — must be identical (determinism).
        let r2 = sample_cross_file_cosines(&frags, &embeddings, n, pair_budget);

        assert_eq!(
            r1.len(),
            expected_cross_file_pairs,
            "50 distinct-file corpus: exact branch must produce {expected_cross_file_pairs} cosines, got {}",
            r1.len()
        );
        assert_eq!(r1, r2, "results must be deterministic across two calls with the same input");

        // All cosines must be 1.0 (identical unit vectors).
        for (idx, &c) in r1.iter().enumerate() {
            assert!(
                (c - 1.0_f32).abs() < 1e-6,
                "cosine[{idx}] should be 1.0 (identical unit vectors), got {c}"
            );
        }

        // Verify aggregate metrics via the full interactor path.
        let mut mock_embed = MockMockEmbeddingPort::new();
        mock_embed.expect_embed().times(n).returning(|_| Ok(vec![1.0_f32, 0.0]));
        let mock_index = MockMockSemanticIndexPort::new();
        let interactor = MeasureQualityInteractor::new(Arc::new(mock_embed), Arc::new(mock_index));
        let metrics =
            interactor.measure_quality(&MeasureQualityCommand { fragments: frags }).unwrap();

        assert!(
            (metrics.mean_cosine - 1.0).abs() < 1e-4,
            "mean_cosine must be 1.0 for many-distinct-paths corpus, got {}",
            metrics.mean_cosine
        );
        assert!(
            metrics.cosine_std_dev.abs() < 1e-4,
            "cosine_std_dev must be 0.0, got {}",
            metrics.cosine_std_dev
        );
        assert!(
            (metrics.above_threshold_rate - 1.0).abs() < 1e-4,
            "above_threshold_rate must be 1.0, got {}",
            metrics.above_threshold_rate
        );
        assert_eq!(metrics.cosine_percentiles.len(), 7);
    }
}
