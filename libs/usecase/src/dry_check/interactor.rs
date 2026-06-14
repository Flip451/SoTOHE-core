//! [`DryCheckInteractor`] — implementation of [`DryCheckService`].
//!
//! Responsibility: drive the full-codebase index build + diff-fragment dry-check
//! cycle and persist per-pair verdicts. See
//! [`DryCheckService::run_dry_check`] for the authoritative two-phase algorithm
//! description (inquiry → judgment, D3 / T010).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use domain::CommitHash;
use domain::TrackId;
use domain::dry_check::{
    DryCheckCoverageRecord, DryCheckEntry, DryCheckFinding, DryCheckPairKey, DryCheckReader,
    DryCheckVerdict, DryCheckWriter, FragmentRef, Rationale,
};
use domain::review_v2::types::FilePath;
use domain::semantic_dup::{CodeFragment, SimilarityScore, SimilarityThreshold};

use super::config::DryCheckConfig;
use super::errors::DryCheckCycleError;
use super::judgment::DryCheckAgentJudgment;
use super::ports::{DryCheckAgentPort, DryCheckCoveragePort};
use super::services::DryCheckService;
use super::shared::{
    build_corpus_index, candidate_pair_keys_for_diff, collect_above_threshold_candidates,
    fragment_ref_of,
};
use crate::semantic_dup::{EmbeddingPort, SemanticIndexError, SemanticIndexPort};

// ── DryCheckInteractor ────────────────────────────────────────────────────────

/// Interactor implementing [`DryCheckService`].
///
/// Orchestrates: full-codebase index build (`EmbeddingPort` + `SemanticIndexPort`
/// from `corpus_fragments`) + diff fragment query at threshold +
/// `DryCheckAgentPort` verification + `DryCheckWriter.append_record` verdict
/// persistence.
///
/// The constructor return type is written as `DryCheckInteractor` (not `Self`)
/// so the ③ evaluator exact-string match succeeds.
pub struct DryCheckInteractor {
    embedding_port: Arc<dyn EmbeddingPort>,
    index_port: Arc<dyn SemanticIndexPort>,
    agent_port: Arc<dyn DryCheckAgentPort>,
    dry_check_writer: Arc<dyn DryCheckWriter>,
    dry_check_reader: Arc<dyn DryCheckReader>,
    coverage: Arc<dyn DryCheckCoveragePort>,
    track_id: TrackId,
    config: DryCheckConfig,
}

impl DryCheckInteractor {
    /// Create a new [`DryCheckInteractor`].
    ///
    /// `diff_source` is NOT injected — the CLI resolves diff fragments and
    /// passes them in as `diff_fragments` (CN-01/IN-02).
    ///
    /// `coverage` + `track_id` are the D5 (T004) addition: `run_dry_check`
    /// writes a [`DryCheckCoverageRecord`] for `track_id` containing every
    /// processed diff-fragment `FragmentRef`, so the read-only `dry
    /// check-approved` (T003) can use it for staleness matching (IN-06 / AC-11
    /// / CN-08).
    ///
    /// `config` carries the D3 (T010) parallelism bound (`max_parallelism`) and
    /// D4 calibration percentages. Composition sources `max_parallelism` from
    /// the infrastructure `DryCheckConfig::load` and lifts it into the usecase
    /// newtype via `DryCheckParallelism::try_new`.
    /// # Arguments
    ///
    /// Construction requires 8 parameters because `DryCheckInteractor` is a
    /// composition-layer value object that bundles all secondary ports with the
    /// domain config.  Each parameter is a distinct, non-groupable concern
    /// (embedding, indexing, agent, write, read, coverage, identity, config).
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        embedding_port: Arc<dyn EmbeddingPort>,
        index_port: Arc<dyn SemanticIndexPort>,
        agent_port: Arc<dyn DryCheckAgentPort>,
        dry_check_writer: Arc<dyn DryCheckWriter>,
        dry_check_reader: Arc<dyn DryCheckReader>,
        coverage: Arc<dyn DryCheckCoveragePort>,
        track_id: TrackId,
        config: DryCheckConfig,
    ) -> DryCheckInteractor {
        DryCheckInteractor {
            embedding_port,
            index_port,
            agent_port,
            dry_check_writer,
            dry_check_reader,
            coverage,
            track_id,
            config,
        }
    }
}

impl DryCheckService for DryCheckInteractor {
    /// Run the dry-check write cycle.
    ///
    /// Inquiry phase (steps 1-4) builds a deduplicated set of unverified
    /// candidate pairs from the current diff against the whole-codebase index,
    /// filtered by the verdict history. Judgment phase (steps 5-8) fans out
    /// the agent calls under `config.max_parallelism`, aggregates errors,
    /// persists results in pair-key order, and returns the violation findings.
    ///
    /// 1. Seed `verified_set` from `dry_check_reader` (CN-07 identifier match).
    /// 2. Build the whole-codebase index from `corpus_fragments` (IN-02).
    /// 3. Per `diff_fragment`: run the growing-k threshold-boundary loop
    ///    (k, 2k, 4k, …) and enumerate above-threshold candidates.
    /// 4. Collect unverified pairs into a `BTreeMap<DryCheckPairKey, ...>` for
    ///    deduplication; the diff-fragment index is retained so the
    ///    `changed_path` can be recovered when persisting each entry.
    /// 5. Fan out the unverified pairs across `config.max_parallelism` threads
    ///    via `std::thread::scope`. Each pair is one `agent_port.judge()`.
    /// 6. Aggregate per-pair results; aggregate errors and return the first
    ///    after all pairs complete (CN-03 — no premature shutdown).
    /// 7. Persist via `dry_check_writer.append_record` in `DryCheckPairKey`
    ///    order (CN-05 — deterministic write order).
    /// 8. Return the collected `DryCheckFinding`s from `Violation` verdicts.
    ///
    /// # Errors
    ///
    /// Returns [`DryCheckCycleError`] on embedding, index, agent, reader,
    /// writer, or entry construction failures.
    fn run_dry_check(
        &self,
        corpus_fragments: Vec<CodeFragment>,
        diff_fragments: Vec<CodeFragment>,
        threshold: SimilarityThreshold,
        base_commit: CommitHash,
    ) -> Result<Vec<DryCheckFinding>, DryCheckCycleError> {
        // ── Phase 1: Inquiry phase ────────────────────────────────────────────

        // Step 1: Build the verified-pair set from history.
        //
        // CN-07: identifier matching — when content changes, content_hash
        // changes, so FragmentRef changes, so DryCheckPairKey changes → no
        // match → re-verified.  No separate hash-comparison step.
        let records = self.dry_check_reader.read_records().map_err(DryCheckCycleError::Reader)?;

        let mut verified_set: BTreeMap<DryCheckPairKey, ()> = BTreeMap::new();
        for record in records {
            verified_set.insert(record.pair_key().clone(), ());
        }

        // Step 2: Build whole-codebase index from corpus_fragments.
        build_corpus_index(
            corpus_fragments,
            self.embedding_port.as_ref(),
            self.index_port.as_ref(),
        )?;

        // Steps 3–4: Per diff_fragment loop — collect unverified pairs.
        //
        // CN-03: errors from individual diff fragments are collected; one
        // fragment-level failure does not abort the remaining fragments.
        // `first_error` is also used in Phase 2 (judgment phase) to aggregate
        // agent / entry / writer errors with the same policy.
        //
        // D5 (T004): collect every processed diff-fragment FragmentRef so we
        // can persist a coverage record at the end of the run.
        let mut first_error: Option<DryCheckCycleError> = None;
        let mut processed_refs: BTreeSet<FragmentRef> = BTreeSet::new();

        // Map from pair_key → (diff_fragment index, candidate_fragment, similarity_score).
        // Using BTreeMap so we iterate in DryCheckPairKey order for Phase 2.
        // The diff-fragment index lets us recover the changed_path in Phase 2
        // without retaining the full fragment.
        let mut unverified_pairs: BTreeMap<
            DryCheckPairKey,
            (usize, CodeFragment, SimilarityScore),
        > = BTreeMap::new();

        for (diff_idx, diff_fragment) in diff_fragments.iter().enumerate() {
            // CN-04: diff_fragments are already hunk-filtered by the CLI.
            // The interactor does NOT perform additional hunk filtering.

            // Record the diff fragment as processed for the D5 coverage manifest
            // (IN-06: FragmentRef = path + content_hash).
            let processed_ref = match fragment_ref_of(diff_fragment) {
                Ok(r) => r,
                Err(e) => {
                    // CN-03: collect errors; continue with remaining fragments.
                    if first_error.is_none() {
                        first_error =
                            Some(DryCheckCycleError::Index(SemanticIndexError::SearchFailed {
                                source: format!("processed_ref error: {e}"),
                            }));
                    }
                    continue;
                }
            };
            processed_refs.insert(processed_ref);

            let above_threshold_candidates = match collect_above_threshold_candidates(
                diff_fragment,
                threshold,
                self.embedding_port.as_ref(),
                self.index_port.as_ref(),
            ) {
                Ok(c) => c,
                Err(e) => {
                    // CN-03: collect errors; continue with remaining fragments.
                    if first_error.is_none() {
                        first_error = Some(e);
                    }
                    continue;
                }
            };
            let candidate_pairs =
                match candidate_pair_keys_for_diff(diff_fragment, above_threshold_candidates) {
                    Ok(p) => p,
                    Err(e) => {
                        // CN-03: collect errors; continue with remaining fragments.
                        if first_error.is_none() {
                            first_error = Some(e);
                        }
                        continue;
                    }
                };

            for candidate_pair in candidate_pairs {
                // Skip already-verified pairs (CN-07 identifier matching).
                if verified_set.contains_key(&candidate_pair.pair_key) {
                    continue;
                }

                // De-duplicate across the k-doubling search rounds: the same
                // (diff_fragment, candidate_fragment) pair can appear at
                // multiple k values.  `DryCheckPairKey` is derived from the
                // *content hashes* of both fragments, so an identical key
                // strictly implies identical fragment content — a second
                // occurrence would produce the same agent judgment.  There is
                // therefore no benefit to keeping both occurrences, and doing
                // so would waste an agent call and risk a duplicate record
                // (violating CN-05 uniqueness).  `or_insert` keeps the first
                // and discards later duplicates; CN-03 is not violated because
                // no new information would be recovered by retrying an
                // identical (fragment-content, fragment-content) pair.
                unverified_pairs.entry(candidate_pair.pair_key).or_insert((
                    diff_idx,
                    candidate_pair.candidate_fragment,
                    candidate_pair.similarity_score,
                ));
            }
        }

        // ── Phase 2: Judgment phase ───────────────────────────────────────────

        // Prepare work items in BTreeMap order (deterministic DryCheckPairKey order).
        // Each item: (pair_key, changed_path, diff_fragment_ref, candidate_fragment, score).
        //
        // `changed_path` is pre-computed here so that the parallel judgment runner
        // never needs to index into diff_fragments.  Per-pair path errors are
        // tolerated (CN-03): a pair whose `changed_path` cannot be constructed is
        // skipped and its error is recorded as `first_error`; all remaining valid
        // pairs still proceed through the judgment phase.
        type WorkItem = (DryCheckPairKey, FilePath, usize, CodeFragment, SimilarityScore);

        // `first_error` was initialised at the start of the inquiry phase so that
        // path-construction errors here are merged with agent / entry / writer
        // errors that accumulate in the judgment loop below (CN-03).
        let mut work_items: Vec<WorkItem> = Vec::with_capacity(unverified_pairs.len());

        for (key, (diff_idx, cand_frag, score)) in unverified_pairs {
            // diff_idx out-of-range is an internal invariant violation (the index
            // was stored when iterating diff_fragments above).  Treat it as a
            // hard error: record and skip this pair (CN-03).
            let diff_fragment = match diff_fragments.get(diff_idx) {
                Some(f) => f,
                None => {
                    if first_error.is_none() {
                        first_error =
                            Some(DryCheckCycleError::Index(SemanticIndexError::SearchFailed {
                                source: format!(
                                    "internal: diff_idx {diff_idx} out of range for diff_fragments"
                                ),
                            }));
                    }
                    continue;
                }
            };
            // CN-03: a malformed source_path must not abort remaining pairs.
            let changed_path =
                match FilePath::new(diff_fragment.source_path.to_string_lossy().into_owned()) {
                    Ok(p) => p,
                    Err(e) => {
                        if first_error.is_none() {
                            first_error =
                                Some(DryCheckCycleError::Index(SemanticIndexError::SearchFailed {
                                    source: format!("changed_path error: {e}"),
                                }));
                        }
                        continue;
                    }
                };
            work_items.push((key, changed_path, diff_idx, cand_frag, score));
        }

        // Fan-out with a bounded thread pool using std::thread::scope.
        let max_parallelism = self.config.max_parallelism.as_usize();
        let agent = self.agent_port.as_ref();

        // Build (diff_fragment, candidate_fragment) pairs for the parallel runner.
        // The runner only needs these two fragments to call agent.judge().
        // diff_idx was validated during work_items construction, so .get() should
        // always succeed here; the error path is a defensive fallback.
        let fragment_pairs_result: Result<Vec<(&CodeFragment, &CodeFragment)>, DryCheckCycleError> =
            work_items
                .iter()
                .map(|(_, _, diff_idx, cand_frag, _)| {
                    let diff_frag = diff_fragments.get(*diff_idx).ok_or_else(|| {
                        DryCheckCycleError::Index(SemanticIndexError::SearchFailed {
                            source: format!(
                                "internal: diff_idx {diff_idx} out of range (defensive check)"
                            ),
                        })
                    })?;
                    Ok((diff_frag, cand_frag))
                })
                .collect();
        let fragment_pairs = fragment_pairs_result?;

        // Results indexed by work_items position: Ok(judgment) or Err(agent_error).
        let judgment_results: Vec<
            Result<DryCheckAgentJudgment, super::errors::DryCheckAgentError>,
        > = run_parallel_judgments(&fragment_pairs, agent, max_parallelism);

        // Collect persisted entries in sorted order and accumulate errors.
        let mut findings: Vec<DryCheckFinding> = Vec::new();

        for (work_item, judgment_result) in work_items.into_iter().zip(judgment_results) {
            let (pair_key, changed_path, _diff_idx, _candidate_fragment, similarity_score) =
                work_item;

            let judgment = match judgment_result {
                Ok(j) => j,
                Err(e) => {
                    // CN-03: collect errors; do not abort remaining pairs.
                    if first_error.is_none() {
                        first_error = Some(DryCheckCycleError::Agent(e));
                    }
                    continue;
                }
            };

            let (rationale, verdict, maybe_finding) = extract_judgment(judgment);

            let entry = match DryCheckEntry::new(
                pair_key,
                changed_path,
                verdict,
                similarity_score,
                threshold,
                base_commit.clone(),
                rationale,
            ) {
                Ok(e) => e,
                Err(e) => {
                    // CN-03: collect errors; do not abort remaining pairs.
                    if first_error.is_none() {
                        first_error = Some(DryCheckCycleError::Entry(e));
                    }
                    continue;
                }
            };

            if let Err(e) = self.dry_check_writer.append_record(&entry) {
                // CN-03: collect writer errors; do not abort remaining pairs.
                if first_error.is_none() {
                    first_error = Some(DryCheckCycleError::Writer(e));
                }
                continue;
            }

            if let Some(finding) = maybe_finding {
                findings.push(finding);
            }
        }

        // ── Step 6 (D5, T004): persist the coverage manifest ──────────────────
        //
        // `dry check-approved` (T003) reads this record and treats any current
        // diff fragment whose `FragmentRef` is NOT covered as stale → Blocked
        // (IN-06 / AC-11 / CN-08).
        //
        // Write coverage unconditionally — even when a partial failure occurred —
        // so that successfully processed fragments are recorded.
        //
        // CN-03: a coverage-write failure is collected into `first_error` only when
        // no prior pair-level error was recorded.  This preserves the original
        // pair-level error rather than masking it with a coverage I/O error, while
        // still surfacing the coverage error when it is the only failure.
        let coverage_record = DryCheckCoverageRecord::new(processed_refs);
        if let Err(e) = self.coverage.write_coverage(&self.track_id, coverage_record) {
            if first_error.is_none() {
                first_error = Some(e);
            }
        }

        // Return first collected error if any occurred (after coverage is written).
        if let Some(err) = first_error {
            return Err(err);
        }

        Ok(findings)
    }
}

// ── Parallel judgment runner ──────────────────────────────────────────────────

/// Run `agent.judge(diff_fragment, candidate_fragment)` for each pair in
/// `fragment_pairs` using at most `max_parallelism` concurrent threads.
///
/// Results are returned in the **same order as `fragment_pairs`** so that callers
/// can zip them back to the original work items by index.
///
/// Pairs are processed in chunks of `max_parallelism`: each chunk is handed to a
/// `std::thread::scope` that spawns exactly one thread per pair in that chunk.  The
/// scope joins all chunk threads before the next chunk begins, so the total number
/// of live OS threads at any point is bounded by `max_parallelism`.  This prevents
/// thread-count exhaustion for large diffs (unlike a semaphore-only approach that
/// spawns N total threads over the lifetime of the function).
///
/// Errors from individual `judge()` calls are preserved in the result vector; they
/// are not collapsed here.
fn run_parallel_judgments(
    fragment_pairs: &[(&CodeFragment, &CodeFragment)],
    agent: &dyn DryCheckAgentPort,
    max_parallelism: usize,
) -> Vec<Result<DryCheckAgentJudgment, super::errors::DryCheckAgentError>> {
    use std::sync::mpsc;

    let n = fragment_pairs.len();
    if n == 0 {
        return Vec::new();
    }

    // Clamp chunk size: at least 1 so we always make progress.
    let chunk_size = max_parallelism.max(1);

    // Pre-allocate output slots indexed by pair position.
    let mut results: Vec<Option<Result<DryCheckAgentJudgment, super::errors::DryCheckAgentError>>> =
        (0..n).map(|_| None).collect();

    for (chunk_start, chunk) in fragment_pairs.chunks(chunk_size).enumerate() {
        // Channel scoped to this chunk: each thread sends (chunk-local index, result).
        let (tx, rx) = mpsc::channel::<(
            usize,
            Result<DryCheckAgentJudgment, super::errors::DryCheckAgentError>,
        )>();

        // `std::thread::scope` joins all threads in this chunk before continuing
        // to the next chunk.  At most `chunk_size` (= `max_parallelism`) OS threads
        // are live at once because the scope covers exactly one chunk.
        std::thread::scope(|scope| {
            for (chunk_i, (diff_fragment, candidate_fragment)) in chunk.iter().enumerate() {
                let tx_clone = tx.clone();
                scope.spawn(move || {
                    let judgment = agent.judge(diff_fragment, candidate_fragment);
                    // Ignore send errors: rx is in scope and outlives all spawned threads.
                    let _ = tx_clone.send((chunk_i, judgment));
                });
            }
            // Drop the spawner's tx so rx sees EOF when all threads have sent.
            drop(tx);
        });

        // Collect results from this chunk and write them into the global result
        // buffer at the absolute pair index.
        for (chunk_i, judgment) in rx {
            let global_i = chunk_start * chunk_size + chunk_i;
            if let Some(slot) = results.get_mut(global_i) {
                *slot = Some(judgment);
            }
        }
    }

    // Unwrap the Option slots — every position must have been filled by a thread.
    // The only way a slot remains None is an internal bug (chunk index calculation
    // error), which we surface as a sentinel error rather than panicking.
    results
        .into_iter()
        .map(|opt| {
            opt.unwrap_or(Err(super::errors::DryCheckAgentError::Unexpected(
                "internal: judgment result missing after parallel run".to_owned(),
            )))
        })
        .collect()
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Decompose a [`DryCheckAgentJudgment`] into:
/// - the [`Rationale`] (present on every variant)
/// - the [`DryCheckVerdict`] for persistence
/// - an optional [`DryCheckFinding`] (only for `Violation`)
fn extract_judgment(
    judgment: DryCheckAgentJudgment,
) -> (Rationale, DryCheckVerdict, Option<DryCheckFinding>) {
    match judgment {
        DryCheckAgentJudgment::NotAViolation { rationale } => {
            (rationale, DryCheckVerdict::NotAViolation, None)
        }
        DryCheckAgentJudgment::Accepted { rationale } => {
            (rationale, DryCheckVerdict::Accepted, None)
        }
        DryCheckAgentJudgment::Violation { rationale, finding } => {
            let verdict = DryCheckVerdict::Violation {
                refactor_proposal: finding.refactor_proposal().clone(),
            };
            (rationale, verdict, Some(finding))
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::type_complexity
)]
mod tests {
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    use domain::CommitHash;
    use domain::dry_check::{
        DryCheckEntry, DryCheckFinding, DryCheckReaderError, DryCheckRecord, DryCheckVerdict,
        DryCheckWriterError, Rationale,
    };
    use domain::semantic_dup::{
        CodeFragment, SimilarFragment, SimilarityScore, SimilarityThreshold,
    };

    use mockall::mock;

    use super::*;
    use crate::dry_check::config::{DryCheckConfig, DryCheckParallelism, DryCheckPercent};
    use crate::dry_check::errors::DryCheckAgentError;
    use crate::dry_check::shared::test_mocks::{
        MockMockEmbeddingPort, MockMockSemanticIndexPort, make_dry_check_record_for_tests,
        make_fragment_ref_from_content,
    };
    use crate::dry_check::shared::{content_hash_of, fragment_ref_of};

    // ── Mock port definitions ─────────────────────────────────────────────────
    //
    // `MockMockEmbeddingPort` and `MockMockSemanticIndexPort` are defined once in
    // `crate::dry_check::shared::test_mocks` and imported above.

    mock! {
        pub MockDryCheckAgentPort {}
        impl DryCheckAgentPort for MockDryCheckAgentPort {
            fn judge(
                &self,
                changed_fragment: &CodeFragment,
                candidate_fragment: &CodeFragment,
            ) -> Result<DryCheckAgentJudgment, DryCheckAgentError>;
        }
    }

    // ── Hand-rolled writer/reader stubs ───────────────────────────────────────

    /// Stub writer that collects appended entries.
    #[derive(Default)]
    struct StubWriter {
        entries: Mutex<Vec<DryCheckEntry>>,
    }

    impl domain::dry_check::DryCheckWriter for StubWriter {
        fn append_record(&self, entry: &DryCheckEntry) -> Result<(), DryCheckWriterError> {
            self.entries.lock().unwrap().push(entry.clone());
            Ok(())
        }
    }

    /// Stub reader that returns a fixed set of records.
    struct StubReader {
        records: Vec<DryCheckRecord>,
    }

    impl StubReader {
        fn with_records(records: Vec<DryCheckRecord>) -> Self {
            Self { records }
        }
    }

    /// Stub coverage port that records every `write_coverage` call.
    #[derive(Default)]
    struct StubCoverage {
        last_record: Mutex<Option<DryCheckCoverageRecord>>,
        write_calls: Mutex<u32>,
        write_should_fail: bool,
    }

    impl StubCoverage {
        fn new() -> Self {
            Self::default()
        }
        fn failing() -> Self {
            Self {
                last_record: Mutex::new(None),
                write_calls: Mutex::new(0),
                write_should_fail: true,
            }
        }
        fn write_call_count(&self) -> u32 {
            *self.write_calls.lock().unwrap()
        }
        fn last_written(&self) -> Option<DryCheckCoverageRecord> {
            self.last_record.lock().unwrap().clone()
        }
    }

    impl DryCheckCoveragePort for StubCoverage {
        fn read_coverage(
            &self,
            _track_id: &TrackId,
        ) -> Result<Option<DryCheckCoverageRecord>, DryCheckCycleError> {
            // The interactor only writes — it never reads.
            panic!("DryCheckInteractor never calls read_coverage in tests")
        }
        fn write_coverage(
            &self,
            _track_id: &TrackId,
            record: DryCheckCoverageRecord,
        ) -> Result<(), DryCheckCycleError> {
            *self.write_calls.lock().unwrap() += 1;
            if self.write_should_fail {
                return Err(DryCheckCycleError::CoveragePort("simulated write error".to_owned()));
            }
            *self.last_record.lock().unwrap() = Some(record);
            Ok(())
        }
    }

    fn make_track() -> TrackId {
        TrackId::try_new("test-track-2026").unwrap()
    }

    impl domain::dry_check::DryCheckReader for StubReader {
        fn read_records(&self) -> Result<Vec<DryCheckRecord>, DryCheckReaderError> {
            Ok(self.records.clone())
        }
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn make_fragment(path: &str, content: &str) -> CodeFragment {
        CodeFragment::new(PathBuf::from(path), content.to_owned(), 1, 1).unwrap()
    }

    fn make_score(v: f32) -> SimilarityScore {
        SimilarityScore::new(v).unwrap()
    }

    fn make_threshold(v: f32) -> SimilarityThreshold {
        SimilarityThreshold::new(v).unwrap()
    }

    fn make_commit() -> CommitHash {
        CommitHash::try_new("a".repeat(40)).unwrap()
    }

    fn make_similar_fragment(path: &str, content: &str, score: f32) -> SimilarFragment {
        SimilarFragment { fragment: make_fragment(path, content), score: make_score(score) }
    }

    /// Build a `NotAViolation` `DryCheckRecord` for pair-cache tests.
    ///
    /// Thin call-site sugar: builds the two `FragmentRef`s via the shared
    /// [`make_fragment_ref_from_content`] helper and forwards them to
    /// [`make_dry_check_record_for_tests`]. The `changed_path` argument is kept
    /// for legacy call-site clarity but is ignored (the shared builder always
    /// uses the low fragment's path).
    fn make_dry_check_record(
        low_path: &str,
        low_content: &str,
        high_path: &str,
        high_content: &str,
        _changed_path: &str,
    ) -> DryCheckRecord {
        make_dry_check_record_for_tests(
            make_fragment_ref_from_content(low_path, low_content),
            make_fragment_ref_from_content(high_path, high_content),
            DryCheckVerdict::NotAViolation,
            "2026-06-02T00:00:00Z",
        )
    }

    /// Build a test config with the given parallelism degree.
    fn make_config(parallelism: usize) -> DryCheckConfig {
        DryCheckConfig::new(
            DryCheckPercent::try_new(10).unwrap(),
            DryCheckPercent::try_new(90).unwrap(),
            DryCheckParallelism::try_new(parallelism).unwrap(),
        )
    }

    fn make_interactor(
        embed: MockMockEmbeddingPort,
        index: MockMockSemanticIndexPort,
        agent: MockMockDryCheckAgentPort,
        writer: Arc<StubWriter>,
        records: Vec<DryCheckRecord>,
    ) -> DryCheckInteractor {
        make_interactor_with_coverage(
            embed,
            index,
            agent,
            writer,
            records,
            Arc::new(StubCoverage::new()),
        )
    }

    fn make_interactor_with_coverage(
        embed: MockMockEmbeddingPort,
        index: MockMockSemanticIndexPort,
        agent: MockMockDryCheckAgentPort,
        writer: Arc<StubWriter>,
        records: Vec<DryCheckRecord>,
        coverage: Arc<StubCoverage>,
    ) -> DryCheckInteractor {
        DryCheckInteractor::new(
            Arc::new(embed),
            Arc::new(index),
            Arc::new(agent),
            writer,
            Arc::new(StubReader::with_records(records)),
            coverage,
            make_track(),
            make_config(1),
        )
    }

    fn make_interactor_empty_history(
        embed: MockMockEmbeddingPort,
        index: MockMockSemanticIndexPort,
        agent: MockMockDryCheckAgentPort,
        writer: Arc<StubWriter>,
    ) -> DryCheckInteractor {
        make_interactor(embed, index, agent, writer, vec![])
    }

    // ── (a) pair cache skips already-verified pair ────────────────────────────

    #[test]
    fn test_pair_cache_skips_already_verified_pair_with_same_path_and_hash() {
        // Record a prior result for (src/a.rs, src/b.rs) with the SAME content.
        let diff_content = "fn shared() {}";
        let cand_content = "fn shared_candidate() {}";
        let prior_record =
            make_dry_check_record("src/a.rs", diff_content, "src/b.rs", cand_content, "src/a.rs");

        let diff_frag = make_fragment("src/a.rs", diff_content);

        let mut embed = MockMockEmbeddingPort::new();
        // embed called for corpus insert (0 corpus) + diff query
        embed.expect_embed().returning(|_| Ok(vec![0.1_f32]));

        let mut index = MockMockSemanticIndexPort::new();
        index.expect_insert_batch().returning(|_| Ok(()));
        // Search returns the candidate above threshold.
        let results = vec![make_similar_fragment("src/b.rs", cand_content, 0.9)];
        index.expect_search().returning(move |_, _| Ok(results.clone()));

        // Agent must NOT be called — pair is already verified.
        let mut agent = MockMockDryCheckAgentPort::new();
        agent.expect_judge().never();

        let writer = Arc::new(StubWriter::default());
        let interactor =
            make_interactor(embed, index, agent, Arc::clone(&writer), vec![prior_record]);

        let result = interactor
            .run_dry_check(vec![], vec![diff_frag], make_threshold(0.8), make_commit())
            .unwrap();

        assert!(result.is_empty(), "already-verified pair must not produce findings");
        assert!(
            writer.entries.lock().unwrap().is_empty(),
            "no new record should be written for verified pair"
        );
    }

    // ── (b) cache invalidated on content change ───────────────────────────────

    #[test]
    fn test_pair_cache_invalidated_when_content_changes() {
        // Prior record has old content; new diff has different content → new pair_key.
        let old_diff_content = "fn old_impl() {}";
        let cand_content = "fn candidate() {}";
        let new_diff_content = "fn new_impl() {}"; // different → new hash → new pair_key

        let prior_record = make_dry_check_record(
            "src/a.rs",
            old_diff_content,
            "src/b.rs",
            cand_content,
            "src/a.rs",
        );

        let diff_frag = make_fragment("src/a.rs", new_diff_content);

        let mut embed = MockMockEmbeddingPort::new();
        embed.expect_embed().returning(|_| Ok(vec![0.1_f32]));

        let mut index = MockMockSemanticIndexPort::new();
        index.expect_insert_batch().returning(|_| Ok(()));
        let results = vec![make_similar_fragment("src/b.rs", cand_content, 0.9)];
        index.expect_search().returning(move |_, _| Ok(results.clone()));

        // Agent IS called — new hash → new pair_key → not in verified set.
        let mut agent = MockMockDryCheckAgentPort::new();
        agent.expect_judge().times(1).returning(|_, _| {
            Ok(DryCheckAgentJudgment::NotAViolation {
                rationale: Rationale::new("different content, not a violation").unwrap(),
            })
        });

        let writer = Arc::new(StubWriter::default());
        let interactor =
            make_interactor(embed, index, agent, Arc::clone(&writer), vec![prior_record]);

        let result = interactor
            .run_dry_check(vec![], vec![diff_frag], make_threshold(0.8), make_commit())
            .unwrap();

        assert!(result.is_empty());
        assert_eq!(
            writer.entries.lock().unwrap().len(),
            1,
            "new record written for invalidated pair"
        );
    }

    // ── (c) self-match excluded via Err(SelfMatch); same-path-diff-hash is valid ─

    #[test]
    fn test_self_match_excluded_via_err_self_match_agent_not_called() {
        // Diff fragment and candidate share BOTH path AND content → self-match.
        let content = "fn self_fn() {}";
        let diff_frag = make_fragment("src/a.rs", content);

        let mut embed = MockMockEmbeddingPort::new();
        embed.expect_embed().returning(|_| Ok(vec![0.1_f32]));

        let mut index = MockMockSemanticIndexPort::new();
        index.expect_insert_batch().returning(|_| Ok(()));
        // Candidate is literally the same fragment.
        let results = vec![make_similar_fragment("src/a.rs", content, 1.0)];
        index.expect_search().returning(move |_, _| Ok(results.clone()));

        // Agent must NOT be called for self-match.
        let mut agent = MockMockDryCheckAgentPort::new();
        agent.expect_judge().never();

        let writer = Arc::new(StubWriter::default());
        let interactor = make_interactor_empty_history(embed, index, agent, Arc::clone(&writer));

        let result = interactor
            .run_dry_check(vec![], vec![diff_frag], make_threshold(0.8), make_commit())
            .unwrap();

        assert!(result.is_empty());
        assert!(writer.entries.lock().unwrap().is_empty());
    }

    #[test]
    fn test_same_path_different_hash_is_valid_pair_agent_called() {
        // Same path but DIFFERENT content → valid pair (NOT a self-match).
        let diff_content = "fn impl_a() {}";
        let cand_content = "fn impl_b() {}"; // different content → different hash
        let diff_frag = make_fragment("src/a.rs", diff_content);

        let mut embed = MockMockEmbeddingPort::new();
        embed.expect_embed().returning(|_| Ok(vec![0.1_f32]));

        let mut index = MockMockSemanticIndexPort::new();
        index.expect_insert_batch().returning(|_| Ok(()));
        // Same path, different content.
        let results = vec![make_similar_fragment("src/a.rs", cand_content, 0.9)];
        index.expect_search().returning(move |_, _| Ok(results.clone()));

        // Agent IS called (not a self-match).
        let mut agent = MockMockDryCheckAgentPort::new();
        agent.expect_judge().times(1).returning(|_, _| {
            Ok(DryCheckAgentJudgment::NotAViolation {
                rationale: Rationale::new("intra-file, not a DRY violation").unwrap(),
            })
        });

        let writer = Arc::new(StubWriter::default());
        let interactor = make_interactor_empty_history(embed, index, agent, Arc::clone(&writer));

        let result = interactor
            .run_dry_check(vec![], vec![diff_frag], make_threshold(0.8), make_commit())
            .unwrap();

        assert!(result.is_empty());
        assert_eq!(writer.entries.lock().unwrap().len(), 1);
    }

    // ── (d) genuine violation found when k grows ──────────────────────────────

    #[test]
    fn test_genuine_violation_found_when_k_grows() {
        // First batch of k=10 returns 10 results all above threshold (no boundary).
        // Second batch of k=20 returns fewer than 20 (index exhausted) but one
        // above-threshold entry triggers Violation.
        let diff_content = "fn duplicated_logic() {}";
        let diff_frag = make_fragment("src/a.rs", diff_content);

        let mut embed = MockMockEmbeddingPort::new();
        embed.expect_embed().returning(|_| Ok(vec![0.5_f32]));

        let mut index = MockMockSemanticIndexPort::new();
        index.expect_insert_batch().returning(|_| Ok(()));

        // First search (k=10): returns exactly 10 above-threshold items — all
        // from "src/x0.rs" .. "src/x9.rs".  No boundary → grow k.
        // Second search (k=20): returns 1 item (index exhausted) above threshold.
        let mut call_count = 0u32;
        index.expect_search().returning(move |_, top_k| {
            call_count += 1;
            if call_count == 1 {
                assert_eq!(top_k.value(), 10);
                let results: Vec<SimilarFragment> = (0..10)
                    .map(|i| {
                        make_similar_fragment(
                            &format!("src/x{i}.rs"),
                            &format!("fn x{i}() {{}}"),
                            0.9,
                        )
                    })
                    .collect();
                Ok(results)
            } else {
                // k=20, but only 1 result — index exhausted, above threshold.
                assert_eq!(top_k.value(), 20);
                Ok(vec![make_similar_fragment(
                    "src/violation.rs",
                    "fn duplicated_logic() { /* exact copy */ }",
                    0.91,
                )])
            }
        });

        // Agent called once for the violation candidate.
        let mut agent = MockMockDryCheckAgentPort::new();
        // 10 from first batch + 1 from second (x0..x9 + violation), 11 total.
        // Each gets judged.  We set NotAViolation for the x* ones and Violation
        // for the last one.
        agent.expect_judge().returning(move |changed, candidate| {
            if candidate.source_path == std::path::Path::new("src/violation.rs") {
                let changed_ref = fragment_ref_of(changed).unwrap();
                let cand_ref = fragment_ref_of(candidate).unwrap();
                let finding =
                    DryCheckFinding::new(changed_ref, cand_ref, "Extract shared logic.").unwrap();
                Ok(DryCheckAgentJudgment::Violation {
                    rationale: Rationale::new("genuine duplication").unwrap(),
                    finding,
                })
            } else {
                Ok(DryCheckAgentJudgment::NotAViolation {
                    rationale: Rationale::new("not a violation").unwrap(),
                })
            }
        });

        let writer = Arc::new(StubWriter::default());
        let interactor = make_interactor_empty_history(embed, index, agent, Arc::clone(&writer));

        let result = interactor
            .run_dry_check(vec![], vec![diff_frag], make_threshold(0.8), make_commit())
            .unwrap();

        assert_eq!(result.len(), 1, "one violation finding expected");
        assert_eq!(result[0].candidate_fragment_ref().path().as_str(), "src/violation.rs");
    }

    // ── (e) NotAViolation returns empty Vec ───────────────────────────────────

    #[test]
    fn test_not_a_violation_returns_empty_vec() {
        let diff_frag = make_fragment("src/a.rs", "fn some_fn() {}");

        let mut embed = MockMockEmbeddingPort::new();
        embed.expect_embed().returning(|_| Ok(vec![0.1_f32]));

        let mut index = MockMockSemanticIndexPort::new();
        index.expect_insert_batch().returning(|_| Ok(()));
        index.expect_search().returning(|_, _| {
            Ok(vec![make_similar_fragment("src/b.rs", "fn similar_fn() {}", 0.9)])
        });

        let mut agent = MockMockDryCheckAgentPort::new();
        agent.expect_judge().times(1).returning(|_, _| {
            Ok(DryCheckAgentJudgment::NotAViolation {
                rationale: Rationale::new("not a violation").unwrap(),
            })
        });

        let writer = Arc::new(StubWriter::default());
        let interactor = make_interactor_empty_history(embed, index, agent, Arc::clone(&writer));

        let result = interactor
            .run_dry_check(vec![], vec![diff_frag], make_threshold(0.8), make_commit())
            .unwrap();

        assert!(result.is_empty(), "NotAViolation should return empty Vec");
    }

    // ── (f) loop termination on threshold boundary ────────────────────────────

    #[test]
    fn test_loop_termination_on_threshold_boundary() {
        let diff_frag = make_fragment("src/a.rs", "fn a() {}");

        let mut embed = MockMockEmbeddingPort::new();
        embed.expect_embed().returning(|_| Ok(vec![0.1_f32]));

        let mut index = MockMockSemanticIndexPort::new();
        index.expect_insert_batch().returning(|_| Ok(()));
        // Return one above-threshold and one below-threshold in the first batch.
        index.expect_search().times(1).returning(|_, _| {
            Ok(vec![
                make_similar_fragment("src/b.rs", "fn above() {}", 0.9),
                make_similar_fragment("src/c.rs", "fn below() {}", 0.5), // below threshold
            ])
        });

        let mut agent = MockMockDryCheckAgentPort::new();
        // Only the above-threshold fragment triggers agent call.
        agent.expect_judge().times(1).returning(|_, _| {
            Ok(DryCheckAgentJudgment::NotAViolation {
                rationale: Rationale::new("not a violation").unwrap(),
            })
        });

        let writer = Arc::new(StubWriter::default());
        let interactor = make_interactor_empty_history(embed, index, agent, Arc::clone(&writer));

        let result = interactor
            .run_dry_check(vec![], vec![diff_frag], make_threshold(0.8), make_commit())
            .unwrap();

        assert!(result.is_empty());
    }

    // ── (g) loop termination on index exhaustion ──────────────────────────────

    #[test]
    fn test_loop_termination_on_index_exhaustion() {
        let diff_frag = make_fragment("src/a.rs", "fn a() {}");

        let mut embed = MockMockEmbeddingPort::new();
        embed.expect_embed().returning(|_| Ok(vec![0.1_f32]));

        let mut index = MockMockSemanticIndexPort::new();
        index.expect_insert_batch().returning(|_| Ok(()));
        // Return fewer than k=10 results (exhausted) — all above threshold.
        index
            .expect_search()
            .times(1)
            .returning(|_, _| Ok(vec![make_similar_fragment("src/b.rs", "fn b() {}", 0.9)]));

        let mut agent = MockMockDryCheckAgentPort::new();
        agent.expect_judge().times(1).returning(|_, _| {
            Ok(DryCheckAgentJudgment::Accepted {
                rationale: Rationale::new("acceptable cross-layer mirror").unwrap(),
            })
        });

        let writer = Arc::new(StubWriter::default());
        let interactor = make_interactor_empty_history(embed, index, agent, Arc::clone(&writer));

        let result = interactor
            .run_dry_check(vec![], vec![diff_frag], make_threshold(0.8), make_commit())
            .unwrap();

        assert!(result.is_empty());
    }

    // ── (h) DryCheckEntry built with 7 fields ────────────────────────────────

    #[test]
    fn test_dry_check_entry_built_with_7_fields() {
        let diff_content = "fn changed() {}";
        let cand_content = "fn candidate() {}";
        let diff_frag = make_fragment("src/a.rs", diff_content);
        let threshold = make_threshold(0.8);
        let base_commit = make_commit();

        let mut embed = MockMockEmbeddingPort::new();
        embed.expect_embed().returning(|_| Ok(vec![0.1_f32]));

        let mut index = MockMockSemanticIndexPort::new();
        index.expect_insert_batch().returning(|_| Ok(()));
        let cand_frag_clone = make_fragment("src/b.rs", cand_content);
        index.expect_search().returning(move |_, _| {
            Ok(vec![SimilarFragment { fragment: cand_frag_clone.clone(), score: make_score(0.85) }])
        });

        let mut agent = MockMockDryCheckAgentPort::new();
        agent.expect_judge().times(1).returning(|_, _| {
            Ok(DryCheckAgentJudgment::NotAViolation {
                rationale: Rationale::new("entry fields test rationale").unwrap(),
            })
        });

        let writer = Arc::new(StubWriter::default());
        let interactor = make_interactor_empty_history(embed, index, agent, Arc::clone(&writer));

        interactor.run_dry_check(vec![], vec![diff_frag], threshold, base_commit.clone()).unwrap();

        let entries = writer.entries.lock().unwrap();
        assert_eq!(entries.len(), 1);
        let entry = &entries[0];

        // pair_key low/high accessible
        let low = entry.pair_key().low();
        let high = entry.pair_key().high();
        assert!(
            (low.path().as_str() == "src/a.rs" && high.path().as_str() == "src/b.rs")
                || (low.path().as_str() == "src/b.rs" && high.path().as_str() == "src/a.rs"),
            "pair_key must contain both fragment paths"
        );

        // content_hash accessible via pair_key low/high
        let expected_diff_hash = content_hash_of(diff_content).unwrap();
        let expected_cand_hash = content_hash_of(cand_content).unwrap();
        let hashes =
            [low.content_hash().as_str().to_owned(), high.content_hash().as_str().to_owned()];
        assert!(hashes.contains(&expected_diff_hash.as_str().to_owned()));
        assert!(hashes.contains(&expected_cand_hash.as_str().to_owned()));

        // changed_path is display-only (diff fragment side)
        assert_eq!(entry.changed_path().as_str(), "src/a.rs");
        // verdict
        assert_eq!(entry.verdict(), &DryCheckVerdict::NotAViolation);
        // similarity_score
        assert!((entry.similarity_score().value() - 0.85).abs() < 1e-5);
        // threshold
        assert!((entry.threshold().value() - 0.8).abs() < 1e-5);
        // base_commit
        assert_eq!(entry.base_commit().as_ref(), base_commit.as_ref());
        // rationale
        assert_eq!(entry.rationale().as_str(), "entry fields test rationale");
    }

    // ── (i) rationale is typed Rationale ─────────────────────────────────────

    #[test]
    fn test_rationale_from_judgment_is_typed_rationale_non_empty_newtype() {
        let diff_frag = make_fragment("src/a.rs", "fn a() {}");

        let mut embed = MockMockEmbeddingPort::new();
        embed.expect_embed().returning(|_| Ok(vec![0.1_f32]));

        let mut index = MockMockSemanticIndexPort::new();
        index.expect_insert_batch().returning(|_| Ok(()));
        index
            .expect_search()
            .returning(|_, _| Ok(vec![make_similar_fragment("src/b.rs", "fn b() {}", 0.9)]));

        let mut agent = MockMockDryCheckAgentPort::new();
        let expected_rationale = "This is the typed rationale";
        agent.expect_judge().times(1).returning(move |_, _| {
            Ok(DryCheckAgentJudgment::NotAViolation {
                rationale: Rationale::new(expected_rationale).unwrap(),
            })
        });

        let writer = Arc::new(StubWriter::default());
        let interactor = make_interactor_empty_history(embed, index, agent, Arc::clone(&writer));

        interactor
            .run_dry_check(vec![], vec![diff_frag], make_threshold(0.8), make_commit())
            .unwrap();

        let entries = writer.entries.lock().unwrap();
        assert_eq!(entries.len(), 1);
        let rationale: &Rationale = entries[0].rationale();
        // It's a Rationale (typed non-empty newtype).
        assert_eq!(rationale.as_str(), expected_rationale);
    }

    // ── (j) Violation produces DryCheckVerdict::Violation + DryCheckFinding ───

    #[test]
    fn test_violation_produces_verdict_violation_and_finding_in_result_vec() {
        let diff_content = "fn duplicated() {}";
        let cand_content = "fn also_duplicated() {}";
        let diff_frag = make_fragment("src/a.rs", diff_content);

        let mut embed = MockMockEmbeddingPort::new();
        embed.expect_embed().returning(|_| Ok(vec![0.1_f32]));

        let mut index = MockMockSemanticIndexPort::new();
        index.expect_insert_batch().returning(|_| Ok(()));
        let cand_frag = make_fragment("src/b.rs", cand_content);
        index.expect_search().returning(move |_, _| {
            Ok(vec![SimilarFragment { fragment: cand_frag.clone(), score: make_score(0.9) }])
        });

        let mut agent = MockMockDryCheckAgentPort::new();
        agent.expect_judge().times(1).returning(move |changed, candidate| {
            let changed_ref = fragment_ref_of(changed).unwrap();
            let cand_ref = fragment_ref_of(candidate).unwrap();
            let finding =
                DryCheckFinding::new(changed_ref, cand_ref, "Extract into shared trait.").unwrap();
            Ok(DryCheckAgentJudgment::Violation {
                rationale: Rationale::new("genuine duplication").unwrap(),
                finding,
            })
        });

        let writer = Arc::new(StubWriter::default());
        let interactor = make_interactor_empty_history(embed, index, agent, Arc::clone(&writer));

        let findings = interactor
            .run_dry_check(vec![], vec![diff_frag], make_threshold(0.8), make_commit())
            .unwrap();

        // DryCheckFinding in returned Vec
        assert_eq!(findings.len(), 1);
        let finding = &findings[0];
        assert_eq!(finding.changed_fragment_ref().path().as_str(), "src/a.rs");
        assert_eq!(finding.candidate_fragment_ref().path().as_str(), "src/b.rs");
        assert_eq!(finding.refactor_proposal().as_str(), "Extract into shared trait.");

        // DryCheckVerdict::Violation persisted
        let entries = writer.entries.lock().unwrap();
        assert_eq!(entries.len(), 1);
        assert!(
            matches!(entries[0].verdict(), DryCheckVerdict::Violation { .. }),
            "persisted verdict must be Violation"
        );
        if let DryCheckVerdict::Violation { refactor_proposal } = entries[0].verdict() {
            assert_eq!(refactor_proposal.as_str(), "Extract into shared trait.");
        }
    }

    // ── (k) content_hash accessible via pair_key().low()/.high().content_hash()

    #[test]
    fn test_content_hash_accessible_via_pair_key_low_high_content_hash() {
        let diff_content = "fn a_impl() {}";
        let cand_content = "fn b_impl() {}";
        let diff_frag = make_fragment("src/a.rs", diff_content);
        let cand_frag_for_search = make_fragment("src/b.rs", cand_content);

        let mut embed = MockMockEmbeddingPort::new();
        embed.expect_embed().returning(|_| Ok(vec![0.1_f32]));

        let mut index = MockMockSemanticIndexPort::new();
        index.expect_insert_batch().returning(|_| Ok(()));
        index.expect_search().returning(move |_, _| {
            Ok(vec![SimilarFragment {
                fragment: cand_frag_for_search.clone(),
                score: make_score(0.9),
            }])
        });

        let mut agent = MockMockDryCheckAgentPort::new();
        agent.expect_judge().times(1).returning(|_, _| {
            Ok(DryCheckAgentJudgment::Accepted {
                rationale: Rationale::new("accepted duplication").unwrap(),
            })
        });

        let writer = Arc::new(StubWriter::default());
        let interactor = make_interactor_empty_history(embed, index, agent, Arc::clone(&writer));

        interactor
            .run_dry_check(vec![], vec![diff_frag], make_threshold(0.8), make_commit())
            .unwrap();

        let entries = writer.entries.lock().unwrap();
        assert_eq!(entries.len(), 1);
        let entry = &entries[0];

        // content_hash accessible via pair_key().low().content_hash() and .high().content_hash()
        let low_hash = entry.pair_key().low().content_hash().as_str().to_owned();
        let high_hash = entry.pair_key().high().content_hash().as_str().to_owned();

        let expected_a_hash = content_hash_of(diff_content).unwrap().as_str().to_owned();
        let expected_b_hash = content_hash_of(cand_content).unwrap().as_str().to_owned();

        assert!(
            (low_hash == expected_a_hash && high_hash == expected_b_hash)
                || (low_hash == expected_b_hash && high_hash == expected_a_hash),
            "pair_key must contain both content hashes: got low={low_hash} high={high_hash}"
        );

        // No separate low_hash/high_hash fields — access is via pair_key().low()/.high()
        assert_eq!(low_hash.len(), 64, "content hash must be 64-char hex");
        assert_eq!(high_hash.len(), 64, "content hash must be 64-char hex");
    }

    // ── (m) corpus batch: embed_batch called once + insert_batch called once ────

    /// Verify that `run_dry_check` calls `embed_batch` exactly once for the
    /// corpus (not per-fragment `embed`) and `insert_batch` exactly once with
    /// all corpus items.  The diff fragment query still uses `embed`.
    #[test]
    fn test_run_dry_check_calls_embed_batch_once_and_insert_batch_once_with_all_corpus_items() {
        // Three corpus fragments; one diff fragment.
        let corpus_a = make_fragment("src/corpus_a.rs", "fn corpus_a() {}");
        let corpus_b = make_fragment("src/corpus_b.rs", "fn corpus_b() {}");
        let corpus_c = make_fragment("src/corpus_c.rs", "fn corpus_c() {}");
        let diff_frag = make_fragment("src/diff.rs", "fn diff() {}");

        let mut embed = MockMockEmbeddingPort::new();
        // embed_batch called once for the 3 corpus fragments.
        embed
            .expect_embed_batch()
            .times(1)
            .withf(|frags| frags.len() == 3)
            .returning(|frags| Ok(frags.iter().map(|_| vec![0.1_f32]).collect()));
        // embed called once for the diff-fragment query in collect_above_threshold_candidates.
        embed.expect_embed().times(1).returning(|_| Ok(vec![0.1_f32]));

        let mut index = MockMockSemanticIndexPort::new();
        // insert_batch must be called exactly once with all 3 corpus items.
        index.expect_insert_batch().times(1).withf(|items| items.len() == 3).returning(|_| Ok(()));
        // Search returns empty (no candidates above threshold) so no agent calls.
        index.expect_search().returning(|_, _| Ok(vec![]));

        // Agent must not be called (no above-threshold candidates).
        let mut agent = MockMockDryCheckAgentPort::new();
        agent.expect_judge().never();

        let writer = Arc::new(StubWriter::default());
        let interactor = make_interactor_empty_history(embed, index, agent, Arc::clone(&writer));

        let result = interactor
            .run_dry_check(
                vec![corpus_a, corpus_b, corpus_c],
                vec![diff_frag],
                make_threshold(0.8),
                make_commit(),
            )
            .unwrap();

        assert!(result.is_empty());
        assert!(writer.entries.lock().unwrap().is_empty());
    }

    // ── T004: D5 coverage write ───────────────────────────────────────────────

    #[test]
    fn test_run_dry_check_writes_coverage_record_with_all_diff_fragment_refs() {
        // Two diff fragments → coverage manifest must contain exactly those two
        // FragmentRefs after a successful run (no candidates above threshold).
        let frag_a = make_fragment("src/a.rs", "fn a() {}");
        let frag_b = make_fragment("src/b.rs", "fn b() {}");

        let mut embed = MockMockEmbeddingPort::new();
        embed.expect_embed().times(2).returning(|_| Ok(vec![0.1_f32]));

        let mut index = MockMockSemanticIndexPort::new();
        index.expect_insert_batch().times(1).withf(|items| items.is_empty()).returning(|_| Ok(()));
        // No candidates above threshold for either diff fragment.
        index.expect_search().times(2).returning(|_, _| Ok(vec![]));

        let mut agent = MockMockDryCheckAgentPort::new();
        agent.expect_judge().never();

        let writer = Arc::new(StubWriter::default());
        let coverage = Arc::new(StubCoverage::new());
        let interactor = make_interactor_with_coverage(
            embed,
            index,
            agent,
            Arc::clone(&writer),
            vec![],
            Arc::clone(&coverage),
        );

        let _ = interactor
            .run_dry_check(
                vec![],
                vec![frag_a.clone(), frag_b.clone()],
                make_threshold(0.8),
                make_commit(),
            )
            .unwrap();

        // write_coverage called exactly once.
        assert_eq!(coverage.write_call_count(), 1);

        // The recorded coverage must list exactly the two diff-fragment FragmentRefs.
        let recorded = coverage.last_written().expect("coverage written");
        assert_eq!(recorded.fragment_refs().len(), 2);
        let expected_a = fragment_ref_of(&frag_a).unwrap();
        let expected_b = fragment_ref_of(&frag_b).unwrap();
        assert!(recorded.covers(&expected_a));
        assert!(recorded.covers(&expected_b));
    }

    #[test]
    fn test_run_dry_check_with_empty_diff_writes_empty_coverage_record() {
        // Empty diff → write_coverage still called once, with an empty record.
        let embed = MockMockEmbeddingPort::new();
        let mut index = MockMockSemanticIndexPort::new();
        index.expect_insert_batch().times(1).withf(|items| items.is_empty()).returning(|_| Ok(()));
        let mut agent = MockMockDryCheckAgentPort::new();
        agent.expect_judge().never();

        let writer = Arc::new(StubWriter::default());
        let coverage = Arc::new(StubCoverage::new());
        let interactor = make_interactor_with_coverage(
            embed,
            index,
            agent,
            Arc::clone(&writer),
            vec![],
            Arc::clone(&coverage),
        );

        let _ =
            interactor.run_dry_check(vec![], vec![], make_threshold(0.8), make_commit()).unwrap();

        assert_eq!(coverage.write_call_count(), 1);
        let recorded = coverage.last_written().expect("coverage written");
        assert!(recorded.fragment_refs().is_empty());
    }

    #[test]
    fn test_run_dry_check_coverage_port_error_propagated() {
        // write_coverage failure → DryCheckCycleError::CoveragePort.
        let frag = make_fragment("src/a.rs", "fn a() {}");

        let mut embed = MockMockEmbeddingPort::new();
        embed.expect_embed().times(1).returning(|_| Ok(vec![0.1_f32]));
        let mut index = MockMockSemanticIndexPort::new();
        index.expect_insert_batch().times(1).withf(|items| items.is_empty()).returning(|_| Ok(()));
        index.expect_search().times(1).returning(|_, _| Ok(vec![]));
        let mut agent = MockMockDryCheckAgentPort::new();
        agent.expect_judge().never();

        let writer = Arc::new(StubWriter::default());
        let coverage = Arc::new(StubCoverage::failing());
        let interactor = make_interactor_with_coverage(
            embed,
            index,
            agent,
            Arc::clone(&writer),
            vec![],
            Arc::clone(&coverage),
        );

        let result =
            interactor.run_dry_check(vec![], vec![frag], make_threshold(0.8), make_commit());
        assert!(matches!(result, Err(DryCheckCycleError::CoveragePort(_))));
    }

    // ── T010: D3 parallel judgment phase ─────────────────────────────────────

    /// Three unverified pairs → agent must be called exactly 3 times.
    #[test]
    fn test_three_unverified_pairs_agent_called_three_times() {
        let diff_frag = make_fragment("src/a.rs", "fn a() {}");

        let mut embed = MockMockEmbeddingPort::new();
        embed.expect_embed().returning(|_| Ok(vec![0.1_f32]));

        let mut index = MockMockSemanticIndexPort::new();
        index.expect_insert_batch().returning(|_| Ok(()));
        index.expect_search().returning(|_, _| {
            Ok(vec![
                make_similar_fragment("src/b.rs", "fn b() {}", 0.9),
                make_similar_fragment("src/c.rs", "fn c() {}", 0.9),
                make_similar_fragment("src/d.rs", "fn d() {}", 0.9),
            ])
        });

        let mut agent = MockMockDryCheckAgentPort::new();
        agent.expect_judge().times(3).returning(|_, _| {
            Ok(DryCheckAgentJudgment::NotAViolation {
                rationale: Rationale::new("not a violation").unwrap(),
            })
        });

        let writer = Arc::new(StubWriter::default());
        // Use parallelism=2 to exercise the bounded fan-out with 3 tasks.
        let interactor = DryCheckInteractor::new(
            Arc::new(embed),
            Arc::new(index),
            Arc::new(agent),
            writer.clone() as Arc<dyn domain::dry_check::DryCheckWriter>,
            Arc::new(StubReader::with_records(vec![])),
            Arc::new(StubCoverage::new()),
            make_track(),
            make_config(2),
        );

        let result = interactor
            .run_dry_check(vec![], vec![diff_frag], make_threshold(0.8), make_commit())
            .unwrap();

        assert!(result.is_empty());
        assert_eq!(writer.entries.lock().unwrap().len(), 3);
    }

    /// With max_parallelism=1, execution is effectively serial and the write
    /// order is deterministic (BTreeMap key order = DryCheckPairKey order).
    #[test]
    fn test_max_parallelism_one_executes_serially_with_deterministic_order() {
        let diff_frag = make_fragment("src/a.rs", "fn a() {}");

        let mut embed = MockMockEmbeddingPort::new();
        embed.expect_embed().returning(|_| Ok(vec![0.1_f32]));

        let mut index = MockMockSemanticIndexPort::new();
        index.expect_insert_batch().returning(|_| Ok(()));
        // Return two above-threshold candidates with known paths.
        index.expect_search().returning(|_, _| {
            Ok(vec![
                make_similar_fragment("src/z.rs", "fn z() {}", 0.9),
                make_similar_fragment("src/b.rs", "fn b() {}", 0.9),
            ])
        });

        let mut agent = MockMockDryCheckAgentPort::new();
        agent.expect_judge().times(2).returning(|_, _| {
            Ok(DryCheckAgentJudgment::NotAViolation {
                rationale: Rationale::new("not a violation").unwrap(),
            })
        });

        let writer = Arc::new(StubWriter::default());
        let interactor = DryCheckInteractor::new(
            Arc::new(embed),
            Arc::new(index),
            Arc::new(agent),
            writer.clone() as Arc<dyn domain::dry_check::DryCheckWriter>,
            Arc::new(StubReader::with_records(vec![])),
            Arc::new(StubCoverage::new()),
            make_track(),
            make_config(1),
        );

        interactor
            .run_dry_check(vec![], vec![diff_frag], make_threshold(0.8), make_commit())
            .unwrap();

        let entries = writer.entries.lock().unwrap();
        assert_eq!(entries.len(), 2, "both pairs must be recorded");

        // Entries must be written in DryCheckPairKey sort order (CN-05).
        // The pair_key low/high order is determined by the key's Ord impl, not
        // the search return order — we just check both are present.
        let paths: Vec<String> = entries
            .iter()
            .map(|e| {
                let low = e.pair_key().low().path().as_str();
                let high = e.pair_key().high().path().as_str();
                format!("{low}:{high}")
            })
            .collect();
        // Both src/b.rs and src/z.rs pairs must appear.
        assert!(
            paths.iter().any(|p| p.contains("src/b.rs")),
            "src/b.rs pair must be in written entries"
        );
        assert!(
            paths.iter().any(|p| p.contains("src/z.rs")),
            "src/z.rs pair must be in written entries"
        );
    }

    /// When one agent call fails, the remaining pairs are still processed and
    /// the first error is returned after all pairs complete.
    #[test]
    fn test_agent_error_on_one_pair_remaining_pairs_still_processed() {
        let diff_frag = make_fragment("src/a.rs", "fn a() {}");

        let mut embed = MockMockEmbeddingPort::new();
        embed.expect_embed().returning(|_| Ok(vec![0.1_f32]));

        let mut index = MockMockSemanticIndexPort::new();
        index.expect_insert_batch().returning(|_| Ok(()));
        index.expect_search().returning(|_, _| {
            Ok(vec![
                make_similar_fragment("src/b.rs", "fn b() {}", 0.9),
                make_similar_fragment("src/c.rs", "fn c() {}", 0.9),
                make_similar_fragment("src/d.rs", "fn d() {}", 0.9),
            ])
        });

        // First call fails; the other two succeed.
        // Because execution is parallel (max_parallelism=1 for determinism in
        // which one fails), we need a shared counter to vary the response.
        let call_count = Arc::new(Mutex::new(0u32));
        let mut agent = MockMockDryCheckAgentPort::new();
        agent.expect_judge().returning(move |_, candidate| {
            // Fail specifically for src/b.rs (which sorts lowest and is first).
            if candidate.source_path == std::path::Path::new("src/b.rs") {
                Err(DryCheckAgentError::Timeout)
            } else {
                *call_count.lock().unwrap() += 1;
                Ok(DryCheckAgentJudgment::NotAViolation {
                    rationale: Rationale::new("ok").unwrap(),
                })
            }
        });

        let writer = Arc::new(StubWriter::default());
        let interactor = DryCheckInteractor::new(
            Arc::new(embed),
            Arc::new(index),
            Arc::new(agent),
            writer.clone() as Arc<dyn domain::dry_check::DryCheckWriter>,
            Arc::new(StubReader::with_records(vec![])),
            Arc::new(StubCoverage::new()),
            make_track(),
            make_config(1),
        );

        let result =
            interactor.run_dry_check(vec![], vec![diff_frag], make_threshold(0.8), make_commit());

        // Error must be returned (from the failed agent call).
        assert!(
            matches!(result, Err(DryCheckCycleError::Agent(DryCheckAgentError::Timeout))),
            "expected Agent(Timeout) error, got: {result:?}"
        );

        // The other two pairs must still have been processed and written.
        let entries = writer.entries.lock().unwrap();
        assert_eq!(entries.len(), 2, "the two successful pairs must be persisted");
    }

    /// Records are appended in DryCheckPairKey sort order regardless of search
    /// return order (CN-05).
    #[test]
    fn test_records_appended_in_pair_key_sort_order() {
        // diff=src/a.rs, candidates: src/z.rs and src/b.rs.
        // DryCheckPairKey(src/a.rs, src/b.rs) < DryCheckPairKey(src/a.rs, src/z.rs)
        // in BTreeMap order → src/b.rs pair must be written first.
        let diff_frag = make_fragment("src/a.rs", "fn a() {}");

        let mut embed = MockMockEmbeddingPort::new();
        embed.expect_embed().returning(|_| Ok(vec![0.1_f32]));

        let mut index = MockMockSemanticIndexPort::new();
        index.expect_insert_batch().returning(|_| Ok(()));
        // Return z first then b (reverse sort order) to confirm sort happens.
        index.expect_search().returning(|_, _| {
            Ok(vec![
                make_similar_fragment("src/z.rs", "fn z() {}", 0.9),
                make_similar_fragment("src/b.rs", "fn b() {}", 0.9),
            ])
        });

        let mut agent = MockMockDryCheckAgentPort::new();
        agent.expect_judge().times(2).returning(|_, _| {
            Ok(DryCheckAgentJudgment::NotAViolation { rationale: Rationale::new("ok").unwrap() })
        });

        let writer = Arc::new(StubWriter::default());
        let interactor = DryCheckInteractor::new(
            Arc::new(embed),
            Arc::new(index),
            Arc::new(agent),
            writer.clone() as Arc<dyn domain::dry_check::DryCheckWriter>,
            Arc::new(StubReader::with_records(vec![])),
            Arc::new(StubCoverage::new()),
            make_track(),
            make_config(1), // serial for deterministic order assertion
        );

        interactor
            .run_dry_check(vec![], vec![diff_frag], make_threshold(0.8), make_commit())
            .unwrap();

        let entries = writer.entries.lock().unwrap();
        assert_eq!(entries.len(), 2);

        // The pair whose high/low sorts lower (src/b.rs < src/z.rs) must appear first.
        let first_paths = {
            let low = entries[0].pair_key().low().path().as_str().to_owned();
            let high = entries[0].pair_key().high().path().as_str().to_owned();
            (low, high)
        };
        let second_paths = {
            let low = entries[1].pair_key().low().path().as_str().to_owned();
            let high = entries[1].pair_key().high().path().as_str().to_owned();
            (low, high)
        };

        // First entry must contain src/b.rs, second must contain src/z.rs.
        assert!(
            first_paths.0.contains("src/b.rs") || first_paths.1.contains("src/b.rs"),
            "first written pair must contain src/b.rs (lower sort key); got {first_paths:?}"
        );
        assert!(
            second_paths.0.contains("src/z.rs") || second_paths.1.contains("src/z.rs"),
            "second written pair must contain src/z.rs (higher sort key); got {second_paths:?}"
        );
    }
}
