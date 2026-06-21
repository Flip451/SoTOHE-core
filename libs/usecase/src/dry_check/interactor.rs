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
    DryCheckConfigFingerprint, DryCheckCorpusFingerprint, DryCheckCoverageRecord, DryCheckEntry,
    DryCheckFinding, DryCheckPairKey, DryCheckReader, DryCheckRecord, DryCheckVerdict,
    DryCheckWriter, FragmentRef, Rationale,
};
use domain::review_v2::types::FilePath;
use domain::semantic_dup::{CodeFragment, SimilarityScore, SimilarityThreshold};

use super::calibration::{
    escalate_violations_to_final, run_calibration_probes, run_parallel_judgments,
};
use super::config::DryCheckConfig;
use super::errors::DryCheckCycleError;
use super::judgment::DryCheckAgentJudgment;
use super::known_bad::known_bad_probe_pairs;
use super::ports::{DryCheckAgentPort, DryCheckCoveragePort, DryCheckJudgeTier};
use super::services::DryCheckService;
use super::shared::{
    build_corpus_index, candidate_pair_keys_for_diff, collect_above_threshold_candidates,
    fragment_ref_of,
};
use crate::semantic_dup::{EmbeddingPort, SemanticIndexError, SemanticIndexPort};

// ── DryCheckInteractor ────────────────────────────────────────────────────────

/// Interactor implementing [`DryCheckService`].
///
/// See [`DryCheckService::run_dry_check`] for the authoritative two-phase
/// algorithm description (inquiry → judgment, D3 / T010).
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
    config_fingerprint: DryCheckConfigFingerprint,
    corpus_fingerprint: DryCheckCorpusFingerprint,
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
    ///
    /// `config_fingerprint` is the SHA-256 fingerprint of the `dry-check.json`
    /// fields that affect `dry write` semantics. Composition computes it via
    /// `infra_config.fingerprint()` after loading and passes it here so that the
    /// interactor can include it in the coverage manifest it writes. A subsequent
    /// `check_approved` call compares the manifest fingerprint against the current
    /// config fingerprint to detect config changes.
    ///
    /// `corpus_fingerprint` is the SHA-256 fingerprint of the full set of
    /// `(repo_relative_path, content_hash)` pairs scanned by the corpus indexer.
    /// Composition computes it via `infrastructure::dry_check::compute_corpus_fingerprint`
    /// before constructing the interactor.  A subsequent `check_approved` call
    /// compares the manifest fingerprint against the current corpus fingerprint to
    /// detect when any corpus file changed (added, removed, or modified) since the
    /// last `dry write` run — invalidating the stale index.
    ///
    /// # Arguments
    ///
    /// Construction requires 10 parameters because `DryCheckInteractor` is a
    /// composition-layer value object that bundles all secondary ports with the
    /// domain config.  Each parameter is a distinct, non-groupable concern
    /// (embedding, indexing, agent, write, read, coverage, identity, config,
    /// config_fingerprint, corpus_fingerprint).
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
        config_fingerprint: DryCheckConfigFingerprint,
        corpus_fingerprint: DryCheckCorpusFingerprint,
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
            config_fingerprint,
            corpus_fingerprint,
        }
    }

    fn write_fail_closed_coverage(&self) -> Result<(), DryCheckCycleError> {
        let record = DryCheckCoverageRecord::new(
            BTreeSet::new(),
            BTreeSet::new(),
            DryCheckConfigFingerprint::fail_closed(),
            DryCheckCorpusFingerprint::fail_closed(),
        );
        self.coverage.write_coverage(&self.track_id, record)
    }

    fn fail_closed_before_return(&self, err: DryCheckCycleError) -> DryCheckCycleError {
        match self.write_fail_closed_coverage() {
            Ok(()) => err,
            Err(coverage_err) => DryCheckCycleError::CoveragePort(format!(
                "coverage write failed ({coverage_err}); also: {err}"
            )),
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
        // CN-07: identifier matching — when content changes, content_hash
        // changes, so FragmentRef changes, so DryCheckPairKey changes → no
        // match → re-verified.  No separate hash-comparison step.
        //
        // Config-fingerprint filtering (round-6 P1 fix, corrected): seed
        // `verified_set` only from pairs whose **latest** historical record
        // carries the current config fingerprint.
        //
        // The fix first builds a latest-per-pair map (last-write-wins, same
        // semantics as `DryCheckResultsInteractor` and `check_approved`).
        // Only entries where the *latest* record's fingerprint matches the
        // current config are added to `verified_set`.
        //
        // Without this two-step approach, iterating records in order and
        // inserting any matching-fingerprint record could leave a stale "config A"
        // record in `verified_set` even though a newer "config B" record exists
        // for the same pair (the later B record does not match the current A
        // fingerprint, so the guard skips it, but the already-inserted A entry
        // is never removed). The pair would then be incorrectly skipped under
        // a reverted-to-A config.
        let records = match self.dry_check_reader.read_records().map_err(DryCheckCycleError::Reader)
        {
            Ok(records) => records,
            Err(err) => return Err(self.fail_closed_before_return(err)),
        };

        // Build latest-per-pair: iterate in record order; later records overwrite
        // earlier ones for the same pair_key (last-write-wins).
        let mut latest_per_pair: BTreeMap<DryCheckPairKey, DryCheckRecord> = BTreeMap::new();
        for record in records {
            latest_per_pair.insert(record.pair_key().clone(), record);
        }

        let mut verified_set: BTreeMap<DryCheckPairKey, ()> = BTreeMap::new();
        for (pair_key, record) in &latest_per_pair {
            if record.config_fingerprint() == &self.config_fingerprint {
                verified_set.insert(pair_key.clone(), ());
            }
        }

        // Step 2: Build whole-codebase index from corpus_fragments.
        if let Err(err) = build_corpus_index(
            corpus_fragments,
            self.embedding_port.as_ref(),
            self.index_port.as_ref(),
        ) {
            return Err(self.fail_closed_before_return(err));
        }

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

        // Track every DryCheckPairKey that is "current" in this run: pairs that
        // are above-threshold candidates (whether newly judged or already in the
        // verified-set). The gate uses this to distinguish stale historical
        // Violation records (candidate side fixed/removed) from active ones.
        let mut processed_pair_keys: BTreeSet<DryCheckPairKey> = BTreeSet::new();

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
                // Record this pair_key as current (above-threshold in this run),
                // regardless of whether it is already verified. This allows the
                // gate to distinguish historical Violations whose candidate side
                // has since been fixed (pair_key no longer produced) from active
                // ones that are still above-threshold.
                processed_pair_keys.insert(candidate_pair.pair_key.clone());

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
        let fragment_pairs = match fragment_pairs_result {
            Ok(pairs) => pairs,
            Err(err) => return Err(self.fail_closed_before_return(err)),
        };

        // ── STEP B: Fast phase — judge all production pairs with Fast tier ───────
        let fast_judgment_results = if work_items.is_empty() {
            Vec::new()
        } else {
            run_parallel_judgments(&fragment_pairs, agent, max_parallelism, DryCheckJudgeTier::Fast)
        };

        // ── STEP C: Fast calibration — run known-bad probes with Fast tier ──────
        //
        // Calibration always runs, even when `work_items` is empty (fully cached or
        // no-candidate run).  A broken or regressed agent can invalidate previously
        // cached verdicts; skipping calibration on no-work runs would leave stale
        // cache entries trusted without any agent-quality check on that run.
        //
        // Known-bad probe pairs are fixed in-memory fixtures (see `known_bad.rs`),
        // so probe construction cannot fail due to file-system unavailability.
        //
        // Only the Fast-tier probe run happens here.  When fast calibration fails,
        // the Final-tier probe run is deferred to STEP D/E so that production pairs
        // are NOT re-run if Final calibration also fails (they would be discarded
        // anyway — running them wastes agent calls).
        let (fast_calibration_passed, probe_setup) = {
            let all_probe_pairs = match known_bad_probe_pairs().map_err(|e| {
                DryCheckCycleError::Agent(super::errors::DryCheckAgentError::Unexpected(format!(
                    "calibration probe fixture error: {e}"
                )))
            }) {
                Ok(pairs) => pairs,
                Err(err) => return Err(self.fail_closed_before_return(err)),
            };
            let total_probes = all_probe_pairs.len();
            let injection_rate = self.config.known_bad_injection_rate_percent.as_u8() as usize;
            // Ceiling division: probe_count = ceil(total_probes * injection_rate / 100)
            let probe_count =
                if total_probes == 0 { 0 } else { (total_probes * injection_rate).div_ceil(100) };
            // Always run at least 1 probe when probes exist.
            let probe_count = probe_count.max(if total_probes > 0 { 1 } else { 0 });
            let run_count = probe_count.min(all_probe_pairs.len());

            let threshold_percent =
                self.config.known_bad_detection_threshold_percent.as_u8() as usize;

            let fast_passed = run_calibration_probes(
                agent,
                all_probe_pairs.get(..run_count).unwrap_or(all_probe_pairs.as_slice()),
                DryCheckJudgeTier::Fast,
                threshold_percent,
            );

            (fast_passed, (all_probe_pairs, run_count, probe_count, threshold_percent))
        };

        // ── STEP D/E: Choose final production judgment results ───────────────────
        //
        // When fast calibration failed, run the Final-tier probe calibration FIRST
        // before spending agent calls on production pairs.  If Final calibration also
        // fails the production pairs are discarded anyway, so skipping them avoids
        // unnecessary agent invocations (PR-160 round-8 P1 fix).
        let (final_judgment_results, calibration_error): (
            Vec<Result<DryCheckAgentJudgment, super::errors::DryCheckAgentError>>,
            Option<DryCheckCycleError>,
        ) = if fast_calibration_passed {
            // Fast calibration passed: promote fast results, escalating
            // Violation/Err to Final tier.
            let results = escalate_violations_to_final(
                &fragment_pairs,
                fast_judgment_results,
                agent,
                max_parallelism,
            );
            (results, None)
        } else {
            // Fast calibration failed: probe with Final tier first.
            let (all_probe_pairs, run_count, probe_count, threshold_percent) = probe_setup;
            let final_cal_passed = run_calibration_probes(
                agent,
                all_probe_pairs.get(..run_count).unwrap_or(all_probe_pairs.as_slice()),
                DryCheckJudgeTier::Final,
                threshold_percent,
            );
            if !final_cal_passed && probe_count > 0 {
                // Both tiers failed: agent quality is untrusted.  Skip production
                // reruns — their verdicts will not be trusted regardless.
                let cal_err = DryCheckCycleError::Agent(
                    super::errors::DryCheckAgentError::Unexpected("calibration failed".to_owned()),
                );
                (Vec::new(), Some(cal_err))
            } else {
                // Final calibration passed: discard fast results, re-run all
                // production pairs with Final tier.
                let results = run_parallel_judgments(
                    &fragment_pairs,
                    agent,
                    max_parallelism,
                    DryCheckJudgeTier::Final,
                );
                (results, None)
            }
        };

        // ── STEP F: Append results and collect findings ──────────────────────────
        // Skip pair-entry writes when calibration failed: the agent quality is
        // untrusted, so no production verdicts are persisted.  Coverage is still
        // written below so the current diff fragments do not remain permanently stale.
        let mut findings: Vec<DryCheckFinding> = Vec::new();

        if calibration_error.is_none() {
            for (work_item, judgment_result) in work_items.into_iter().zip(final_judgment_results) {
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
                    self.config_fingerprint.clone(),
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
        }

        // ── Step 6 (D5, T004): persist the coverage manifest ──────────────────
        //
        // `dry check-approved` (T003) reads this record and treats any current
        // diff fragment whose `FragmentRef` is NOT covered as stale → Blocked
        // (IN-06 / AC-11 / CN-08).
        //
        // Write coverage unconditionally — even when a partial failure occurred —
        // so that the coverage manifest is always present for `check_approved`.
        //
        // AC-08 / calibration failure (fail-closed): when calibration fails, write an
        // EMPTY coverage record instead of `processed_refs`.  An empty record means
        // ALL current diff fragments are "uncovered" → `check_approved` returns
        // Blocked rather than Approved.  This preserves fail-closed semantics: the
        // agent's reliability could not be confirmed, so no approval is possible.
        // Writing *some* manifest (even empty) ensures the next `dry write` can
        // overwrite it once calibration succeeds, and the gate does not remain
        // blocked due to a missing manifest from a previous older run.
        //
        // CN-03: a coverage-write failure is collected into `first_error` only when
        // no prior pair-level error was recorded.  This preserves the original
        // pair-level error rather than masking it with a coverage I/O error, while
        // still surfacing the coverage error when it is the only failure.
        // Fail-closed: any error in this run (calibration OR pair-level: embed/index/agent/
        // entry/writer) leaves coverage empty so `check_approved` sees ALL current fragments
        // as uncovered → Blocked. Without this, a partial write where some pairs failed
        // before `append_record` could publish `processed_refs` and let the gate report
        // Approved even though some current pairs have no verdict on record.
        //
        // processed_pair_keys follows the same fail-closed policy: on any error we
        // write an empty set, so the gate treats all historical Violation records as
        // potentially active (cannot prove the candidate was re-checked).
        //
        // config_fingerprint / corpus_fingerprint: on any error (calibration or
        // pair-level), write the fail-closed sentinel (all zeros) instead of the
        // real fingerprints. This ensures `check_approved` returns Blocked regardless
        // of which gate it hits, even if the real fingerprints happen to match the
        // current config/corpus.
        let (coverage_refs, coverage_pair_keys, coverage_fingerprint, coverage_corpus_fp) =
            if calibration_error.is_some() || first_error.is_some() {
                (
                    std::collections::BTreeSet::new(),
                    std::collections::BTreeSet::new(),
                    DryCheckConfigFingerprint::fail_closed(),
                    DryCheckCorpusFingerprint::fail_closed(),
                )
            } else {
                (
                    processed_refs,
                    processed_pair_keys,
                    self.config_fingerprint.clone(),
                    self.corpus_fingerprint.clone(),
                )
            };
        let coverage_record = DryCheckCoverageRecord::new(
            coverage_refs,
            coverage_pair_keys,
            coverage_fingerprint,
            coverage_corpus_fp,
        );
        let coverage_write_error =
            self.coverage.write_coverage(&self.track_id, coverage_record).err();

        // Coverage write failure must never be silently dropped — a failed fail-closed
        // write leaves an old manifest in place and may grant approval incorrectly.
        // When coverage write fails alongside another error, surface both by
        // combining them into a single CoveragePort error. The failed coverage
        // write means the previous manifest may still approve stale state, so it
        // must not be hidden behind a pair-level error.
        if let Some(cov_err) = coverage_write_error {
            if let Some(ref cal_err) = calibration_error {
                // Both calibration and coverage write failed: combine into one error
                // so neither is silently dropped.
                return Err(DryCheckCycleError::CoveragePort(format!(
                    "coverage write failed ({cov_err}); also: {cal_err}"
                )));
            }
            if let Some(ref pair_err) = first_error {
                return Err(DryCheckCycleError::CoveragePort(format!(
                    "coverage write failed ({cov_err}); also: {pair_err}"
                )));
            }
            first_error = Some(cov_err);
        }

        // Return first collected error if any occurred (after coverage is written).
        // Calibration error takes priority over pair-level errors because it
        // signals that the agent itself is unreliable for the entire run.
        if let Some(err) = calibration_error {
            return Err(err);
        }
        if let Some(err) = first_error {
            return Err(err);
        }

        Ok(findings)
    }
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
        DryCheckConfigFingerprint, DryCheckCorpusFingerprint, DryCheckEntry, DryCheckFinding,
        DryCheckPairKey, DryCheckReaderError, DryCheckRecord, DryCheckVerdict, DryCheckWriterError,
        Rationale,
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
                tier: DryCheckJudgeTier,
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

    struct FailingReader;

    impl domain::dry_check::DryCheckReader for FailingReader {
        fn read_records(&self) -> Result<Vec<DryCheckRecord>, DryCheckReaderError> {
            Err(DryCheckReaderError::Io {
                path: "dry-check.json".to_owned(),
                detail: "simulated read error".to_owned(),
            })
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
            false,
        )
    }

    fn test_fingerprint() -> DryCheckConfigFingerprint {
        DryCheckConfigFingerprint::new("a".repeat(64)).unwrap()
    }

    fn test_corpus_fingerprint() -> DryCheckCorpusFingerprint {
        DryCheckCorpusFingerprint::new("b".repeat(64)).unwrap()
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

    fn make_interactor_with_records_config_and_coverage(
        embed: MockMockEmbeddingPort,
        index: MockMockSemanticIndexPort,
        agent: MockMockDryCheckAgentPort,
        writer: Arc<StubWriter>,
        records: Vec<DryCheckRecord>,
        config: DryCheckConfig,
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
            config,
            test_fingerprint(),
            test_corpus_fingerprint(),
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
        make_interactor_with_records_config_and_coverage(
            embed,
            index,
            agent,
            writer,
            records,
            make_config(1),
            coverage,
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

        // Agent must NOT be called for production pairs — pair is already verified.
        // Calibration probes still run (probes always run regardless of work_items).
        let agent =
            make_probe_only_agent("non-probe agent call not expected for already-verified pair");

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
        // Calibration probes (path starts with "probes/") also fire once.
        let agent = make_probe_aware_agent(DryCheckAgentJudgment::NotAViolation {
            rationale: Rationale::new("different content, not a violation").unwrap(),
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

        // Agent must NOT be called for self-match production pairs.
        // Calibration probes still run (probes always run regardless of work_items count).
        let agent = make_probe_only_agent("non-probe agent call not expected for self-match");

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
        // Calibration probes (path starts with "probes/") also fire once.
        let agent = make_probe_aware_agent(DryCheckAgentJudgment::NotAViolation {
            rationale: Rationale::new("intra-file, not a DRY violation").unwrap(),
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

        // Agent called for each production candidate + 1 calibration probe.
        // 10 from first batch + 1 from second (x0..x9 + violation), 11 production total.
        // Each gets judged.  We set NotAViolation for the x* ones and Violation
        // for the last one.  Probe paths ("probes/") always return Violation.
        let agent = make_probe_agent_with_non_probe(move |changed, candidate| {
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

        let agent = make_probe_aware_agent(DryCheckAgentJudgment::NotAViolation {
            rationale: Rationale::new("not a violation").unwrap(),
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

        // Only the above-threshold fragment triggers a production agent call.
        // Calibration probes (path starts with "probes/") also fire once.
        let agent = make_probe_aware_agent(DryCheckAgentJudgment::NotAViolation {
            rationale: Rationale::new("not a violation").unwrap(),
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

        // Calibration probes (path starts with "probes/") fire once.
        let agent = make_probe_aware_agent(DryCheckAgentJudgment::Accepted {
            rationale: Rationale::new("acceptable cross-layer mirror").unwrap(),
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

        // Calibration probes (path starts with "probes/") also fire once.
        let agent = make_probe_aware_agent(DryCheckAgentJudgment::NotAViolation {
            rationale: Rationale::new("entry fields test rationale").unwrap(),
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

        let expected_rationale = "This is the typed rationale";
        // Calibration probes (path starts with "probes/") also fire once.
        let agent = make_probe_aware_agent(DryCheckAgentJudgment::NotAViolation {
            rationale: Rationale::new(expected_rationale).unwrap(),
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

    /// Base scenario builder: calibration passes (probe → Violation) and the
    /// single production pair is judged by `agent`.
    ///
    /// Fragments are `src/a.rs` (`diff_content`) vs `src/b.rs`
    /// (`cand_content`) at similarity 0.9, threshold 0.8.
    /// Returns `(findings, writer)` so callers can assert against both.
    fn run_calibration_success_scenario(
        diff_content: &'static str,
        cand_content: &'static str,
        agent: MockMockDryCheckAgentPort,
    ) -> (Vec<DryCheckFinding>, Arc<StubWriter>) {
        let diff_frag = make_fragment("src/a.rs", diff_content);

        let mut embed = MockMockEmbeddingPort::new();
        embed.expect_embed().returning(|_| Ok(vec![0.1_f32]));

        let mut index = MockMockSemanticIndexPort::new();
        index.expect_insert_batch().returning(|_| Ok(()));
        let cand_frag = make_fragment("src/b.rs", cand_content);
        index.expect_search().returning(move |_, _| {
            Ok(vec![SimilarFragment { fragment: cand_frag.clone(), score: make_score(0.9) }])
        });

        let writer = Arc::new(StubWriter::default());
        let interactor =
            make_interactor_with_config(embed, index, agent, Arc::clone(&writer), make_config(1));

        let findings = interactor
            .run_dry_check(vec![], vec![diff_frag], make_threshold(0.8), make_commit())
            .unwrap();

        (findings, writer)
    }

    /// Shared scenario builder: calibration passes (probe → Violation) and the
    /// single production pair returns Violation with the given `refactor_proposal`.
    fn run_calibration_success_violation_scenario(
        refactor_proposal: &'static str,
    ) -> (Vec<DryCheckFinding>, Arc<StubWriter>) {
        let agent = make_probe_agent_with_non_probe(move |changed, candidate| {
            let changed_ref = fragment_ref_of(changed).unwrap();
            let cand_ref = fragment_ref_of(candidate).unwrap();
            let finding = DryCheckFinding::new(changed_ref, cand_ref, refactor_proposal).unwrap();
            Ok(DryCheckAgentJudgment::Violation {
                rationale: Rationale::new("genuine duplication").unwrap(),
                finding,
            })
        });
        run_calibration_success_scenario("fn duplicated() {}", "fn also_duplicated() {}", agent)
    }

    #[test]
    fn test_violation_produces_verdict_violation_and_finding_in_result_vec() {
        let proposal = "Extract into shared trait.";
        let (findings, writer) = run_calibration_success_violation_scenario(proposal);

        // DryCheckFinding in returned Vec
        assert_eq!(findings.len(), 1);
        let finding = &findings[0];
        assert_eq!(finding.changed_fragment_ref().path().as_str(), "src/a.rs");
        assert_eq!(finding.candidate_fragment_ref().path().as_str(), "src/b.rs");
        assert_eq!(finding.refactor_proposal().as_str(), proposal);

        // DryCheckVerdict::Violation persisted
        let entries = writer.entries.lock().unwrap();
        assert_eq!(entries.len(), 1);
        assert!(
            matches!(entries[0].verdict(), DryCheckVerdict::Violation { .. }),
            "persisted verdict must be Violation"
        );
        if let DryCheckVerdict::Violation { refactor_proposal } = entries[0].verdict() {
            assert_eq!(refactor_proposal.as_str(), proposal);
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

        // Calibration probes (path starts with "probes/") also fire once.
        let agent = make_probe_aware_agent(DryCheckAgentJudgment::Accepted {
            rationale: Rationale::new("accepted duplication").unwrap(),
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

        // No production agent calls (no above-threshold candidates). Calibration
        // probes still run (probes always run regardless of work_items count).
        let agent = make_probe_only_agent(
            "non-probe agent call not expected (no above-threshold candidates)",
        );

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

        // No production agent calls (no above-threshold candidates). Calibration
        // probes still run (probes always run regardless of work_items count).
        let agent = make_probe_only_agent(
            "non-probe agent call not expected (no above-threshold candidates)",
        );

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
    fn test_run_dry_check_writes_coverage_record_with_config_fingerprint() {
        // A successful run must write the config fingerprint into the coverage record.
        let frag_a = make_fragment("src/a.rs", "fn a() {}");

        let mut embed = MockMockEmbeddingPort::new();
        embed.expect_embed().times(1).returning(|_| Ok(vec![0.1_f32]));

        let mut index = MockMockSemanticIndexPort::new();
        index.expect_insert_batch().times(1).withf(|items| items.is_empty()).returning(|_| Ok(()));
        // No candidates above threshold.
        index.expect_search().times(1).returning(|_, _| Ok(vec![]));

        let agent = make_probe_only_agent("non-probe agent call not expected");

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
            .run_dry_check(vec![], vec![frag_a], make_threshold(0.8), make_commit())
            .unwrap();

        let recorded = coverage.last_written().expect("coverage written");
        // The coverage record must carry the test fingerprint (not fail-closed zeros).
        assert_eq!(
            recorded.config_fingerprint(),
            &test_fingerprint(),
            "successful run must write the real config fingerprint"
        );
    }

    #[test]
    fn test_run_dry_check_with_empty_diff_writes_empty_coverage_record() {
        // Empty diff → write_coverage still called once, with an empty record.
        let embed = MockMockEmbeddingPort::new();
        let mut index = MockMockSemanticIndexPort::new();
        index.expect_insert_batch().times(1).withf(|items| items.is_empty()).returning(|_| Ok(()));
        // No production agent calls (empty diff → no candidates). Calibration
        // probes still run (probes always run regardless of work_items count).
        let agent = make_probe_only_agent("non-probe agent call not expected (empty diff)");

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
    fn test_run_dry_check_writes_coverage_record_with_processed_pair_keys() {
        // When there is an above-threshold candidate pair, the coverage manifest
        // must include the pair_key in `processed_pair_keys` so the gate can
        // distinguish active Violations from stale ones.
        let diff_frag = make_fragment("src/a.rs", "fn a() {}");
        let cand_content = "fn b() {}";

        let mut embed = MockMockEmbeddingPort::new();
        embed.expect_embed().returning(|_| Ok(vec![0.1_f32]));

        let mut index = MockMockSemanticIndexPort::new();
        index.expect_insert_batch().returning(|_| Ok(()));
        index
            .expect_search()
            .returning(move |_, _| Ok(vec![make_similar_fragment("src/b.rs", cand_content, 0.9)]));

        // Calibration probes and the single production pair all use NotAViolation.
        let agent = make_probe_aware_agent(DryCheckAgentJudgment::NotAViolation {
            rationale: Rationale::new("not a violation").unwrap(),
        });

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

        interactor
            .run_dry_check(vec![], vec![diff_frag.clone()], make_threshold(0.8), make_commit())
            .unwrap();

        let recorded = coverage.last_written().expect("coverage written");

        // The diff-fragment FragmentRef must appear in fragment_refs.
        let expected_diff_ref = fragment_ref_of(&diff_frag).unwrap();
        assert!(recorded.covers(&expected_diff_ref), "diff fragment ref must be covered");

        // The pair_key (diff, candidate) must appear in processed_pair_keys.
        let cand_frag = make_fragment("src/b.rs", cand_content);
        let expected_diff_ref2 = fragment_ref_of(&diff_frag).unwrap();
        let expected_cand_ref = fragment_ref_of(&cand_frag).unwrap();
        let expected_pair = DryCheckPairKey::new(expected_diff_ref2, expected_cand_ref).unwrap();
        assert!(
            recorded.contains_pair(&expected_pair),
            "processed pair_key must appear in coverage processed_pair_keys"
        );
    }

    #[test]
    fn test_run_dry_check_reader_error_writes_fail_closed_coverage() {
        let embed = MockMockEmbeddingPort::new();
        let index = MockMockSemanticIndexPort::new();
        let agent = MockMockDryCheckAgentPort::new();
        let writer = Arc::new(StubWriter::default());
        let coverage = Arc::new(StubCoverage::new());

        let interactor = DryCheckInteractor::new(
            Arc::new(embed),
            Arc::new(index),
            Arc::new(agent),
            writer,
            Arc::new(FailingReader),
            Arc::clone(&coverage) as Arc<dyn DryCheckCoveragePort>,
            make_track(),
            make_config(1),
            test_fingerprint(),
            test_corpus_fingerprint(),
        );

        let result = interactor.run_dry_check(
            vec![],
            vec![make_fragment("src/a.rs", "fn a() {}")],
            make_threshold(0.8),
            make_commit(),
        );

        assert!(
            matches!(result, Err(DryCheckCycleError::Reader(_))),
            "expected Reader error, got: {result:?}"
        );
        assert_eq!(coverage.write_call_count(), 1);

        let recorded = coverage.last_written().expect("coverage written");
        assert!(recorded.fragment_refs().is_empty());
        assert!(recorded.processed_pair_keys().is_empty());
        assert_eq!(recorded.config_fingerprint(), &DryCheckConfigFingerprint::fail_closed());
        assert_eq!(recorded.corpus_fingerprint().as_str(), "0".repeat(64));
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
        // No production agent calls (no above-threshold candidates). Calibration
        // probes still run (probes always run regardless of work_items count).
        let agent = make_probe_only_agent(
            "non-probe agent call not expected (no above-threshold candidates)",
        );

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

        // 3 production calls + 1 calibration probe call.
        // Calibration probes (path starts with "probes/") return Violation.
        let agent = make_probe_aware_agent(DryCheckAgentJudgment::NotAViolation {
            rationale: Rationale::new("not a violation").unwrap(),
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
            test_fingerprint(),
            test_corpus_fingerprint(),
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

        // 2 production calls + 1 calibration probe call.
        // Calibration probes (path starts with "probes/") return Violation.
        let agent = make_probe_aware_agent(DryCheckAgentJudgment::NotAViolation {
            rationale: Rationale::new("not a violation").unwrap(),
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
            test_fingerprint(),
            test_corpus_fingerprint(),
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
        // Calibration probes (changed path starts with "probes/") return Violation.
        let call_count = Arc::new(Mutex::new(0u32));
        let agent = make_probe_agent_with_non_probe(move |_changed, candidate| {
            if candidate.source_path == std::path::Path::new("src/b.rs") {
                // Fail specifically for src/b.rs (which sorts lowest and is first).
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
            test_fingerprint(),
            test_corpus_fingerprint(),
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

    /// Fail-closed: when a pair-level error (agent/entry/writer) sets `first_error`,
    /// the coverage manifest must be written with an EMPTY ref set.
    ///
    /// Background: the fix changed `if calibration_error.is_some()` to
    /// `if calibration_error.is_some() || first_error.is_some()` when selecting
    /// `coverage_refs`.  This test pins that behaviour: any pair-level failure forces
    /// empty coverage so `check_approved` sees all current fragments as uncovered →
    /// Blocked, preserving fail-closed semantics.
    #[test]
    fn test_pair_level_error_forces_empty_coverage_fail_closed() {
        let diff_frag = make_fragment("src/a.rs", "fn a() {}");

        let mut embed = MockMockEmbeddingPort::new();
        embed.expect_embed().returning(|_| Ok(vec![0.1_f32]));

        let mut index = MockMockSemanticIndexPort::new();
        index.expect_insert_batch().returning(|_| Ok(()));
        // Return one candidate so the agent is called for a production pair.
        index
            .expect_search()
            .returning(|_, _| Ok(vec![make_similar_fragment("src/b.rs", "fn b() {}", 0.9)]));

        // Calibration passes (probe → Violation); the single production pair
        // fails with an agent Timeout → first_error is set.
        let agent = make_probe_agent_with_non_probe(|_changed, _candidate| {
            Err(DryCheckAgentError::Timeout)
        });

        let coverage = Arc::new(StubCoverage::new());
        let interactor = make_interactor_with_coverage(
            embed,
            index,
            agent,
            Arc::new(StubWriter::default()),
            vec![],
            Arc::clone(&coverage),
        );

        let result =
            interactor.run_dry_check(vec![], vec![diff_frag], make_threshold(0.8), make_commit());

        // The pair-level error must be returned.
        assert!(
            matches!(result, Err(DryCheckCycleError::Agent(DryCheckAgentError::Timeout))),
            "expected Agent(Timeout) error, got: {result:?}"
        );

        // Coverage MUST be written exactly once — but with an EMPTY ref set.
        // An empty record means all current diff fragments are uncovered →
        // `check_approved` returns Blocked (fail-closed).
        assert_eq!(
            coverage.write_call_count(),
            1,
            "coverage must be written exactly once even when a pair-level error occurs"
        );
        assert!(
            coverage.last_written().is_some_and(|r| r.fragment_refs().is_empty()),
            "coverage written on pair-level error must be empty (fail-closed)"
        );
    }

    #[test]
    fn test_pair_level_error_with_coverage_write_error_returns_coverage_error() {
        let diff_frag = make_fragment("src/a.rs", "fn a() {}");

        let mut embed = MockMockEmbeddingPort::new();
        embed.expect_embed().returning(|_| Ok(vec![0.1_f32]));

        let mut index = MockMockSemanticIndexPort::new();
        index.expect_insert_batch().returning(|_| Ok(()));
        index
            .expect_search()
            .returning(|_, _| Ok(vec![make_similar_fragment("src/b.rs", "fn b() {}", 0.9)]));

        let agent = make_probe_agent_with_non_probe(|_changed, _candidate| {
            Err(DryCheckAgentError::Timeout)
        });
        let coverage = Arc::new(StubCoverage::failing());
        let interactor = make_interactor_with_coverage(
            embed,
            index,
            agent,
            Arc::new(StubWriter::default()),
            vec![],
            Arc::clone(&coverage),
        );

        let result =
            interactor.run_dry_check(vec![], vec![diff_frag], make_threshold(0.8), make_commit());

        let Err(DryCheckCycleError::CoveragePort(message)) = result else {
            panic!("expected CoveragePort error, got: {result:?}");
        };
        assert!(message.contains("coverage write failed"));
        assert!(message.contains("timed out"));
        assert_eq!(coverage.write_call_count(), 1);
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

        // 2 production calls + 1 calibration probe call.
        // Calibration probes (path starts with "probes/") return Violation.
        let agent = make_probe_aware_agent(DryCheckAgentJudgment::NotAViolation {
            rationale: Rationale::new("ok").unwrap(),
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
            test_fingerprint(),
            test_corpus_fingerprint(),
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

    // ── T012: D4 calibration barrier tests ───────────────────────────────────

    /// Build a mock agent where probe paths always return a `Violation` judgment
    /// and non-probe paths are handled by `on_non_probe`.
    ///
    /// Centralises the probe detection rule (`starts_with("probes/")`) and the
    /// authoritative probe `Violation` construction so callers only provide the
    /// non-probe branch.
    fn make_probe_agent_with_non_probe<F>(on_non_probe: F) -> MockMockDryCheckAgentPort
    where
        F: Fn(&CodeFragment, &CodeFragment) -> Result<DryCheckAgentJudgment, DryCheckAgentError>
            + Send
            + 'static,
    {
        let mut agent = MockMockDryCheckAgentPort::new();
        agent.expect_judge().returning(move |changed, candidate, _tier| {
            if changed.source_path.to_string_lossy().starts_with("probes/") {
                let changed_ref = fragment_ref_of(changed).unwrap();
                let cand_ref = fragment_ref_of(candidate).unwrap();
                Ok(DryCheckAgentJudgment::Violation {
                    rationale: Rationale::new("probe: known bad").unwrap(),
                    finding: DryCheckFinding::new(changed_ref, cand_ref, "probe violation")
                        .unwrap(),
                })
            } else {
                on_non_probe(changed, candidate)
            }
        });
        agent
    }

    fn make_probe_aware_agent(
        production_verdict: DryCheckAgentJudgment,
    ) -> MockMockDryCheckAgentPort {
        make_probe_agent_with_non_probe(move |_, _| Ok(production_verdict.clone()))
    }

    /// Helper: build a probe-aware agent that returns `Violation` for probe paths
    /// and panics with `non_probe_panic_msg` for any non-probe (production) call.
    ///
    /// Use this in tests that assert no production agent call should occur (e.g.
    /// the verified-pair cache test, self-match exclusion, or no-candidates paths).
    fn make_probe_only_agent(non_probe_panic_msg: &'static str) -> MockMockDryCheckAgentPort {
        make_probe_agent_with_non_probe(move |_, _| panic!("{non_probe_panic_msg}"))
    }

    #[derive(Clone, Copy)]
    enum FinalProductionPairBehavior {
        ReturnNotAViolation,
        PanicIfCalled(&'static str),
    }

    /// Helper: build an agent where probe paths always return `NotAViolation`
    /// so both Fast and Final calibration fail. Fast production calls return
    /// `NotAViolation`; Final production behavior is the test-specific variant.
    fn make_final_calibration_failure_agent(
        final_production_behavior: FinalProductionPairBehavior,
    ) -> MockMockDryCheckAgentPort {
        let mut agent = MockMockDryCheckAgentPort::new();
        agent.expect_judge().returning(move |changed, _, tier| {
            if changed.source_path.to_string_lossy().starts_with("probes/") {
                Ok(DryCheckAgentJudgment::NotAViolation {
                    rationale: Rationale::new("probe: not detected").unwrap(),
                })
            } else if tier == DryCheckJudgeTier::Final {
                match final_production_behavior {
                    FinalProductionPairBehavior::ReturnNotAViolation => {
                        Ok(DryCheckAgentJudgment::NotAViolation {
                            rationale: Rationale::new("production final: not a violation").unwrap(),
                        })
                    }
                    FinalProductionPairBehavior::PanicIfCalled(message) => panic!("{message}"),
                }
            } else {
                Ok(DryCheckAgentJudgment::NotAViolation {
                    rationale: Rationale::new("production fast: not a violation").unwrap(),
                })
            }
        });
        agent
    }

    /// Helper: build a DryCheckInteractor with the given config.
    ///
    /// Delegates to `make_interactor_with_config_and_coverage` with a default
    /// `StubCoverage` so constructor wiring exists in exactly one place.
    fn make_interactor_with_config(
        embed: MockMockEmbeddingPort,
        index: MockMockSemanticIndexPort,
        agent: MockMockDryCheckAgentPort,
        writer: Arc<StubWriter>,
        config: DryCheckConfig,
    ) -> DryCheckInteractor {
        make_interactor_with_config_and_coverage(
            embed,
            index,
            agent,
            writer,
            config,
            Arc::new(StubCoverage::new()),
        )
    }

    /// Core helper: build a `DryCheckInteractor` with the given config and an
    /// explicit `coverage` stub so tests can inspect `write_call_count()` /
    /// `last_written()`.
    ///
    /// `make_interactor_with_config` delegates to this function, supplying a
    /// default `StubCoverage::new()` so the constructor wiring lives here only.
    fn make_interactor_with_config_and_coverage(
        embed: MockMockEmbeddingPort,
        index: MockMockSemanticIndexPort,
        agent: MockMockDryCheckAgentPort,
        writer: Arc<StubWriter>,
        config: DryCheckConfig,
        coverage: Arc<StubCoverage>,
    ) -> DryCheckInteractor {
        make_interactor_with_records_config_and_coverage(
            embed,
            index,
            agent,
            writer,
            vec![],
            config,
            coverage,
        )
    }

    fn run_final_calibration_failure_scenario(
        final_production_behavior: FinalProductionPairBehavior,
    ) -> (Result<Vec<DryCheckFinding>, DryCheckCycleError>, Arc<StubWriter>, Arc<StubCoverage>)
    {
        let diff_frag = make_fragment("src/a.rs", "fn a() {}");

        let mut embed = MockMockEmbeddingPort::new();
        embed.expect_embed().returning(|_| Ok(vec![0.1_f32]));

        let mut index = MockMockSemanticIndexPort::new();
        index.expect_insert_batch().returning(|_| Ok(()));
        index
            .expect_search()
            .returning(|_, _| Ok(vec![make_similar_fragment("src/b.rs", "fn b() {}", 0.9)]));

        let config = DryCheckConfig::new(
            DryCheckPercent::try_new(100).unwrap(),
            DryCheckPercent::try_new(90).unwrap(),
            DryCheckParallelism::try_new(1).unwrap(),
            false,
        );

        let writer = Arc::new(StubWriter::default());
        let coverage = Arc::new(StubCoverage::new());
        let agent = make_final_calibration_failure_agent(final_production_behavior);
        let interactor = make_interactor_with_config_and_coverage(
            embed,
            index,
            agent,
            Arc::clone(&writer),
            config,
            Arc::clone(&coverage),
        );

        let result =
            interactor.run_dry_check(vec![], vec![diff_frag], make_threshold(0.8), make_commit());

        (result, writer, coverage)
    }

    fn assert_final_calibration_failure_fail_closed(
        result: Result<Vec<DryCheckFinding>, DryCheckCycleError>,
        writer: &StubWriter,
        coverage: &StubCoverage,
    ) {
        assert!(
            matches!(
                &result,
                Err(DryCheckCycleError::Agent(DryCheckAgentError::Unexpected(msg)))
                    if msg.as_str() == "calibration failed"
            ),
            "expected calibration-failed error, got: {result:?}"
        );
        assert!(
            writer.entries.lock().unwrap().is_empty(),
            "pair-entry writer must not be called when calibration fails"
        );
        assert_eq!(
            coverage.write_call_count(),
            1,
            "coverage must be written exactly once even when calibration fails"
        );
        assert!(
            coverage.last_written().is_some_and(|r| r.fragment_refs().is_empty()),
            "coverage written on calibration failure must be empty (fail-closed)"
        );
    }

    /// T012-1: Fast tier is used for production pairs when agent path is NOT a probe.
    ///
    /// With calibration passing (probe returns Violation) and a production
    /// NotAViolation result, the writer must record exactly 1 entry (no escalation,
    /// no double-write).
    #[test]
    fn test_fast_calibration_success_not_a_violation_pair_not_escalated_to_final() {
        let agent = make_probe_aware_agent(DryCheckAgentJudgment::NotAViolation {
            rationale: Rationale::new("not a violation").unwrap(),
        });
        let (findings, writer) = run_calibration_success_scenario("fn a() {}", "fn b() {}", agent);

        assert!(findings.is_empty(), "NotAViolation must produce no findings");
        // Exactly 1 production pair written (probe not written CN-06).
        assert_eq!(
            writer.entries.lock().unwrap().len(),
            1,
            "exactly 1 production record written after calibration passes"
        );
    }

    /// T012-2: When fast calibration fails and Final calibration also fails, the
    /// error is returned fail-closed — but coverage IS written first (T011
    /// preservation) so the current diff fragments do not remain permanently stale.
    ///
    /// With injection_rate=100 and 3 probes, probe_count = 3 (all probes run).
    /// All probes return NotAViolation → detection rate = 0% < threshold 90% →
    /// fast calibration fails.  Final probes also return NotAViolation → final
    /// calibration also fails → fail-closed error returned after coverage write.
    ///
    /// Crucially, production pairs are NOT re-run at Final tier: their verdicts
    /// would be discarded anyway, so skipping them avoids unnecessary agent calls
    /// (PR-160 round-8 P1 fix).  See T012-2b for the strict no-production-call
    /// assertion.
    #[test]
    fn test_fast_calibration_failure_final_also_fails_returns_calibration_error() {
        let (result, writer, coverage) = run_final_calibration_failure_scenario(
            FinalProductionPairBehavior::ReturnNotAViolation,
        );
        // Pair-level writer must NOT be called: agent quality is untrusted, so no
        // production verdicts are persisted when calibration fails.
        // Coverage manifest MUST be written on calibration failure — but with an
        // EMPTY set of refs (AC-08 fail-closed).  An empty coverage record means
        // all current diff fragments are "uncovered" → `check_approved` returns
        // Blocked rather than Approved.  Writing the (empty) manifest ensures the
        // gate does not remain blocked due to a missing manifest from an older run.
        assert_final_calibration_failure_fail_closed(result, writer.as_ref(), coverage.as_ref());
    }

    /// T012-2b: When fast calibration fails AND Final calibration also fails,
    /// `agent.judge` is NOT called for any production pair at Final tier.
    ///
    /// This is the strict non-call assertion for the PR-160 round-8 P1 fix:
    /// production pairs must be skipped when Final calibration has already shown the
    /// agent is unreliable, since their verdicts will be discarded anyway.
    ///
    /// Note: production pairs ARE called at Fast tier (STEP B runs before calibration
    /// — this is by design and cannot be avoided).  The fix only skips the production
    /// re-run at Final tier after both calibration tiers have failed.
    ///
    /// Setup: injection_rate=100 (all 3 probes run), threshold=90.
    /// Fast probes → NotAViolation (fails, 0% detection < 90%).
    /// Final probes → NotAViolation (fails, 0% detection < 90%).
    /// Agent panics if called for a non-probe path at Final tier, proving production
    /// re-run was skipped.
    #[test]
    fn test_fast_calibration_failure_final_also_fails_production_pairs_not_called() {
        let (result, writer, coverage) =
            run_final_calibration_failure_scenario(FinalProductionPairBehavior::PanicIfCalled(
                "production pair must NOT be judged at Final tier when both calibration tiers fail",
            ));

        // Calibration error must still be returned fail-closed.
        // No production entries written.
        // Coverage still written (fail-closed empty manifest).
        assert_final_calibration_failure_fail_closed(result, writer.as_ref(), coverage.as_ref());
    }

    /// T012-3: When fast calibration passes but a production pair returns Violation,
    /// that pair is escalated to Final tier. Writer records the Final tier verdict.
    #[test]
    fn test_fast_calibration_success_violation_pair_escalated_to_final_tier() {
        let (findings, writer) =
            run_calibration_success_violation_scenario("Extract shared logic.");

        // Violation finding must be returned (after Final tier escalation).
        assert_eq!(findings.len(), 1, "one violation finding expected");
        assert_eq!(findings[0].changed_fragment_ref().path().as_str(), "src/a.rs");
        assert_eq!(findings[0].candidate_fragment_ref().path().as_str(), "src/b.rs");
        // Writer must record exactly 1 production entry.
        assert_eq!(
            writer.entries.lock().unwrap().len(),
            1,
            "exactly 1 production entry written (probe not recorded CN-06)"
        );
    }

    /// T012-3b: When a production Violation is escalated to Final tier, the writer
    /// records the **Final-tier verdict**, not the provisional Fast-tier verdict.
    ///
    /// This is the tier-discriminating escalation test.  The agent returns:
    /// - probe path → Violation (calibration passes for both tiers)
    /// - production pair + Fast tier → Violation
    /// - production pair + Final tier → NotAViolation
    ///
    /// The written verdict must be NotAViolation (from Final), proving that the
    /// interactor actually re-judged the pair with Final instead of persisting the
    /// provisional Fast result.
    #[test]
    fn test_escalation_persists_final_tier_verdict_not_provisional_fast_verdict() {
        let mut agent = MockMockDryCheckAgentPort::new();
        agent.expect_judge().returning(|changed, candidate, tier| {
            if changed.source_path.to_string_lossy().starts_with("probes/") {
                // Probe: always Violation so calibration passes.
                let changed_ref = fragment_ref_of(changed).unwrap();
                let cand_ref = fragment_ref_of(candidate).unwrap();
                return Ok(DryCheckAgentJudgment::Violation {
                    rationale: Rationale::new("probe: known bad").unwrap(),
                    finding: DryCheckFinding::new(changed_ref, cand_ref, "probe violation")
                        .unwrap(),
                });
            }
            // Production pair: tier-discriminating response.
            match tier {
                DryCheckJudgeTier::Fast => {
                    let changed_ref = fragment_ref_of(changed).unwrap();
                    let cand_ref = fragment_ref_of(candidate).unwrap();
                    Ok(DryCheckAgentJudgment::Violation {
                        rationale: Rationale::new("fast: provisional violation").unwrap(),
                        finding: DryCheckFinding::new(changed_ref, cand_ref, "fast proposal")
                            .unwrap(),
                    })
                }
                DryCheckJudgeTier::Final => Ok(DryCheckAgentJudgment::NotAViolation {
                    rationale: Rationale::new("final: not a violation after deeper check").unwrap(),
                }),
            }
        });

        let (findings, writer) =
            run_calibration_success_scenario("fn duplicated() {}", "fn also_dup() {}", agent);

        // The Final verdict (NotAViolation) must prevail — no finding returned.
        assert!(
            findings.is_empty(),
            "Final-tier NotAViolation must override provisional Fast Violation; got findings: \
             {findings:?}"
        );
        // The writer must record NotAViolation (Final tier), not Violation (Fast tier).
        let entries = writer.entries.lock().unwrap();
        assert_eq!(entries.len(), 1, "exactly 1 production entry written");
        assert!(
            matches!(entries[0].verdict(), DryCheckVerdict::NotAViolation),
            "persisted verdict must be NotAViolation (Final tier), not Violation (Fast tier)"
        );
    }

    /// T012-4: Probe verdicts are NOT written to dry_check_writer (CN-06).
    ///
    /// With 1 production pair (NotAViolation) and 1 probe (Violation via
    /// make_probe_aware_agent), writer must have exactly 1 entry — not 2.
    #[test]
    fn test_probe_verdict_not_written_to_dry_check_writer_cn06() {
        let agent = make_probe_aware_agent(DryCheckAgentJudgment::NotAViolation {
            rationale: Rationale::new("not a violation").unwrap(),
        });
        let (_findings, writer) = run_calibration_success_scenario("fn a() {}", "fn b() {}", agent);

        // Only the production pair must be written — probe Violation must NOT appear.
        let entries = writer.entries.lock().unwrap();
        assert_eq!(
            entries.len(),
            1,
            "probe Violation must not be written to dry_check_writer (CN-06), got {} entries",
            entries.len()
        );
        // The written entry must be the production pair (not a probe path).
        let changed_path = entries[0].changed_path().as_str();
        assert!(
            !changed_path.starts_with("probes/"),
            "written entry must be a production pair, not a probe; got changed_path={changed_path}"
        );
    }

    /// T012-5: Final calibration passes when fast calibration fails but Final probes
    /// detect violations. Writer records all production Final-tier results.
    #[test]
    fn test_fast_calibration_failure_final_passes_production_pairs_written_with_final_results() {
        let diff_frag = make_fragment("src/a.rs", "fn a() {}");

        let mut embed = MockMockEmbeddingPort::new();
        embed.expect_embed().returning(|_| Ok(vec![0.1_f32]));

        let mut index = MockMockSemanticIndexPort::new();
        index.expect_insert_batch().returning(|_| Ok(()));
        index
            .expect_search()
            .returning(|_, _| Ok(vec![make_similar_fragment("src/b.rs", "fn b() {}", 0.9)]));

        // Fast probes → NotAViolation (fast calibration fails).
        // Final probes → Violation (final calibration passes).
        // Production Fast → NotAViolation.
        // Production Final → NotAViolation.
        let mut agent = MockMockDryCheckAgentPort::new();
        agent.expect_judge().returning(move |changed, candidate, tier| {
            if changed.source_path.to_string_lossy().starts_with("probes/") {
                match tier {
                    DryCheckJudgeTier::Fast => Ok(DryCheckAgentJudgment::NotAViolation {
                        rationale: Rationale::new("probe fast: not detected").unwrap(),
                    }),
                    DryCheckJudgeTier::Final => {
                        let changed_ref = fragment_ref_of(changed).unwrap();
                        let cand_ref = fragment_ref_of(candidate).unwrap();
                        Ok(DryCheckAgentJudgment::Violation {
                            rationale: Rationale::new("probe final: detected").unwrap(),
                            finding: DryCheckFinding::new(changed_ref, cand_ref, "probe").unwrap(),
                        })
                    }
                }
            } else {
                Ok(DryCheckAgentJudgment::NotAViolation {
                    rationale: Rationale::new("production: not a violation").unwrap(),
                })
            }
        });

        let writer = Arc::new(StubWriter::default());
        // injection_rate=100 so all 3 probes are run; threshold=50 so that even
        // 1 out of 3 fast detections (33%) would fail (<50%) but 3/3 final detections pass.
        let config = DryCheckConfig::new(
            DryCheckPercent::try_new(100).unwrap(),
            DryCheckPercent::try_new(50).unwrap(),
            DryCheckParallelism::try_new(1).unwrap(),
            false,
        );
        let interactor =
            make_interactor_with_config(embed, index, agent, Arc::clone(&writer), config);

        let findings = interactor
            .run_dry_check(vec![], vec![diff_frag], make_threshold(0.8), make_commit())
            .unwrap();

        assert!(findings.is_empty(), "production NotAViolation must produce no findings");
        // Production pair written exactly once (Final tier result).
        assert_eq!(
            writer.entries.lock().unwrap().len(),
            1,
            "exactly 1 production entry after fast-fail + final-pass"
        );
    }

    // ── Config-fingerprint verified_set filtering (round-6 P1) ──────────────

    /// When a historical record carries a DIFFERENT config fingerprint, its pair
    /// must NOT be added to `verified_set`, so the agent is called for that pair
    /// in the new run.
    ///
    /// Scenario: one prior record under fingerprint `"b" * 64`; the interactor
    /// uses `"a" * 64` (the test fingerprint).  The pair should be re-judged and
    /// a new record should be written.
    #[test]
    fn test_run_dry_check_skips_verified_set_seeding_for_records_under_old_fingerprint() {
        // Build a prior record for (src/a.rs, src/b.rs) under a DIFFERENT fingerprint.
        let diff_content = "fn shared() {}";
        let cand_content = "fn shared_candidate() {}";

        let low_ref = make_fragment_ref_from_content("src/a.rs", diff_content);
        let high_ref = make_fragment_ref_from_content("src/b.rs", cand_content);

        // Stale fingerprint — differs from the test interactor's "a"*64.
        let stale_fp = DryCheckConfigFingerprint::new("b".repeat(64)).unwrap();
        let changed_path = low_ref.path().clone();
        let stale_entry = DryCheckEntry::new(
            DryCheckPairKey::new(low_ref, high_ref).unwrap(),
            changed_path,
            DryCheckVerdict::NotAViolation,
            domain::semantic_dup::SimilarityScore::new(0.9).unwrap(),
            domain::semantic_dup::SimilarityThreshold::new(0.8).unwrap(),
            make_commit(),
            domain::dry_check::Rationale::new("test").unwrap(),
            stale_fp,
        )
        .unwrap();
        let stale_record = DryCheckRecord::from_entry_and_timestamp(
            stale_entry,
            domain::Timestamp::new("2026-06-01T00:00:00Z").unwrap(),
        )
        .unwrap();

        let diff_frag = make_fragment("src/a.rs", diff_content);

        let mut embed = MockMockEmbeddingPort::new();
        embed.expect_embed().returning(|_| Ok(vec![0.1_f32]));

        let mut index = MockMockSemanticIndexPort::new();
        index.expect_insert_batch().returning(|_| Ok(()));
        // The search returns the candidate — the pair must not be skipped even
        // though there is a historical record, because the fingerprint differs.
        let results = vec![make_similar_fragment("src/b.rs", cand_content, 0.9)];
        index.expect_search().returning(move |_, _| Ok(results.clone()));

        // Agent IS called: the stale-fingerprint record must NOT populate
        // verified_set, so the pair is treated as unverified.
        let agent = make_probe_aware_agent(DryCheckAgentJudgment::NotAViolation {
            rationale: Rationale::new("re-judged under new config").unwrap(),
        });

        let writer = Arc::new(StubWriter::default());
        let interactor =
            make_interactor(embed, index, agent, Arc::clone(&writer), vec![stale_record]);

        let result = interactor
            .run_dry_check(vec![], vec![diff_frag], make_threshold(0.8), make_commit())
            .unwrap();

        assert!(result.is_empty(), "NotAViolation should return no findings");
        assert_eq!(
            writer.entries.lock().unwrap().len(),
            1,
            "the pair must be re-judged and a new record must be written (stale fingerprint \
             excluded from verified_set)"
        );
        // The new record must carry the current (test) fingerprint.
        assert_eq!(
            writer.entries.lock().unwrap()[0].config_fingerprint(),
            &test_fingerprint(),
            "written entry must carry the current config fingerprint, not the stale one"
        );
    }

    /// When a pair's record history is [config_A, config_B], and the current config
    /// reverts to A, the pair must NOT be treated as verified.
    ///
    /// The latest record for this pair is the B record. Because the latest record
    /// carries fingerprint B (≠ current A), the pair must be re-judged even though
    /// an older A record exists in the history. The non-latest A record must not
    /// "resurrect" the verified status when B is the effective (latest) state.
    #[test]
    fn test_verified_set_uses_latest_per_pair_not_any_matching_record() {
        let diff_content = "fn config_revert_test() {}";
        let cand_content = "fn config_revert_cand() {}";

        let low_ref = make_fragment_ref_from_content("src/a.rs", diff_content);
        let high_ref = make_fragment_ref_from_content("src/b.rs", cand_content);

        let fp_a = test_fingerprint(); // "a"*64 — the current config fingerprint
        let fp_b = DryCheckConfigFingerprint::new("b".repeat(64)).unwrap(); // config B

        // Record 1: judged under config A (old, but fingerprint matches current).
        let changed_path_a = low_ref.path().clone();
        let entry_a = DryCheckEntry::new(
            DryCheckPairKey::new(low_ref.clone(), high_ref.clone()).unwrap(),
            changed_path_a,
            DryCheckVerdict::NotAViolation,
            domain::semantic_dup::SimilarityScore::new(0.9).unwrap(),
            domain::semantic_dup::SimilarityThreshold::new(0.8).unwrap(),
            make_commit(),
            domain::dry_check::Rationale::new("old A judgment").unwrap(),
            fp_a,
        )
        .unwrap();
        let record_a = DryCheckRecord::from_entry_and_timestamp(
            entry_a,
            domain::Timestamp::new("2026-06-01T00:00:00Z").unwrap(),
        )
        .unwrap();

        // Record 2: later judged under config B (the latest record for this pair).
        let changed_path_b = low_ref.path().clone();
        let entry_b = DryCheckEntry::new(
            DryCheckPairKey::new(low_ref, high_ref).unwrap(),
            changed_path_b,
            DryCheckVerdict::NotAViolation,
            domain::semantic_dup::SimilarityScore::new(0.9).unwrap(),
            domain::semantic_dup::SimilarityThreshold::new(0.8).unwrap(),
            make_commit(),
            domain::dry_check::Rationale::new("B judgment").unwrap(),
            fp_b,
        )
        .unwrap();
        let record_b = DryCheckRecord::from_entry_and_timestamp(
            entry_b,
            domain::Timestamp::new("2026-06-02T00:00:00Z").unwrap(),
        )
        .unwrap();

        // History: [record_a (config A), record_b (config B)].
        // Current config is A. Latest record is B → pair must be re-judged.
        let diff_frag = make_fragment("src/a.rs", diff_content);

        let mut embed = MockMockEmbeddingPort::new();
        embed.expect_embed().returning(|_| Ok(vec![0.1_f32]));

        let mut index = MockMockSemanticIndexPort::new();
        index.expect_insert_batch().returning(|_| Ok(()));
        let results = vec![make_similar_fragment("src/b.rs", cand_content, 0.9)];
        index.expect_search().returning(move |_, _| Ok(results.clone()));

        // Agent MUST be called: the latest record carries B fingerprint (≠ current A),
        // so the pair is not in verified_set.
        let agent = make_probe_aware_agent(DryCheckAgentJudgment::NotAViolation {
            rationale: Rationale::new("re-judged after config revert to A").unwrap(),
        });

        let writer = Arc::new(StubWriter::default());
        let interactor =
            make_interactor(embed, index, agent, Arc::clone(&writer), vec![record_a, record_b]);

        let result = interactor
            .run_dry_check(vec![], vec![diff_frag], make_threshold(0.8), make_commit())
            .unwrap();

        assert!(result.is_empty(), "NotAViolation should return no findings");
        assert_eq!(
            writer.entries.lock().unwrap().len(),
            1,
            "pair must be re-judged when latest record carries a different fingerprint, \
             even though an older record with the current fingerprint exists"
        );
        // Written entry must carry the current (A) fingerprint.
        assert_eq!(
            writer.entries.lock().unwrap()[0].config_fingerprint(),
            &test_fingerprint(),
            "written entry must carry the current config fingerprint"
        );
    }

    /// T012-5b: When fast calibration fails and final calibration passes, production
    /// pairs are re-judged with the **Final tier** and the Final-tier verdict is
    /// persisted — NOT the discarded provisional Fast result.
    ///
    /// Tier-discriminating version of T012-5:
    /// - Probe Fast → NotAViolation (fast calibration fails)
    /// - Probe Final → Violation (final calibration passes)
    /// - Production Fast → Violation (provisional, must be discarded)
    /// - Production Final → NotAViolation (must be persisted)
    ///
    /// If the interactor incorrectly reused the discarded Fast provisional result,
    /// the writer would record `Violation`. This test catches that regression.
    #[test]
    fn test_fast_calibration_failure_production_re_run_with_final_tier_verdict_persisted() {
        let diff_frag = make_fragment("src/a.rs", "fn a() {}");

        let mut embed = MockMockEmbeddingPort::new();
        embed.expect_embed().returning(|_| Ok(vec![0.1_f32]));

        let mut index = MockMockSemanticIndexPort::new();
        index.expect_insert_batch().returning(|_| Ok(()));
        index
            .expect_search()
            .returning(|_, _| Ok(vec![make_similar_fragment("src/b.rs", "fn b() {}", 0.9)]));

        let mut agent = MockMockDryCheckAgentPort::new();
        agent.expect_judge().returning(move |changed, candidate, tier| {
            if changed.source_path.to_string_lossy().starts_with("probes/") {
                // Probe: Fast fails calibration, Final passes calibration.
                match tier {
                    DryCheckJudgeTier::Fast => Ok(DryCheckAgentJudgment::NotAViolation {
                        rationale: Rationale::new("probe fast: undetected").unwrap(),
                    }),
                    DryCheckJudgeTier::Final => {
                        let changed_ref = fragment_ref_of(changed).unwrap();
                        let cand_ref = fragment_ref_of(candidate).unwrap();
                        Ok(DryCheckAgentJudgment::Violation {
                            rationale: Rationale::new("probe final: detected").unwrap(),
                            finding: DryCheckFinding::new(changed_ref, cand_ref, "probe").unwrap(),
                        })
                    }
                }
            } else {
                // Production: Fast returns Violation (provisional, must be discarded);
                // Final returns NotAViolation (must be the persisted verdict).
                match tier {
                    DryCheckJudgeTier::Fast => {
                        let changed_ref = fragment_ref_of(changed).unwrap();
                        let cand_ref = fragment_ref_of(candidate).unwrap();
                        Ok(DryCheckAgentJudgment::Violation {
                            rationale: Rationale::new("fast: provisional violation").unwrap(),
                            finding: DryCheckFinding::new(
                                changed_ref,
                                cand_ref,
                                "fast proposal (must be discarded)",
                            )
                            .unwrap(),
                        })
                    }
                    DryCheckJudgeTier::Final => Ok(DryCheckAgentJudgment::NotAViolation {
                        rationale: Rationale::new("final: not a violation").unwrap(),
                    }),
                }
            }
        });

        let writer = Arc::new(StubWriter::default());
        // injection_rate=100; threshold=50 so Fast calibration (0/3 = 0%) fails
        // but Final calibration (3/3 = 100%) passes.
        let config = DryCheckConfig::new(
            DryCheckPercent::try_new(100).unwrap(),
            DryCheckPercent::try_new(50).unwrap(),
            DryCheckParallelism::try_new(1).unwrap(),
            false,
        );
        let interactor =
            make_interactor_with_config(embed, index, agent, Arc::clone(&writer), config);

        let findings = interactor
            .run_dry_check(vec![], vec![diff_frag], make_threshold(0.8), make_commit())
            .unwrap();

        // Final verdict is NotAViolation — no finding must be returned.
        assert!(
            findings.is_empty(),
            "Final-tier NotAViolation must be the persisted result; got findings: {findings:?}"
        );
        // Writer must record the Final-tier verdict (NotAViolation), not the Fast provisional.
        let entries = writer.entries.lock().unwrap();
        assert_eq!(entries.len(), 1, "exactly 1 production entry written");
        assert!(
            matches!(entries[0].verdict(), DryCheckVerdict::NotAViolation),
            "persisted verdict must be NotAViolation (from Final tier, not provisional Fast \
             Violation)"
        );
    }

    // ── Round-7 P1 fix: calibration always runs even with no production pairs ──

    /// When `dry_write` is called and no diff fragment produces an above-threshold
    /// candidate (`work_items` is empty), calibration probes must still be invoked.
    ///
    /// Calibration guards agent quality on every run.  A regressed or broken agent
    /// could invalidate previously cached verdicts; skipping calibration on no-work
    /// runs would leave stale entries trusted without any quality check on that run.
    ///
    /// The default config (injection_rate=10, threshold=90) with 3 known-bad fixtures
    /// yields exactly 1 probe call: `ceil(3 * 10 / 100) = 1`.  The mock asserts
    /// `.times(1)` so a regression that skips calibration would cause the test to fail
    /// at mock-drop time with "expected 1 call, got 0".
    #[test]
    fn test_no_above_threshold_candidates_still_runs_calibration_probes() {
        let diff_frag = make_fragment("src/a.rs", "fn a() {}");

        let mut embed = MockMockEmbeddingPort::new();
        embed.expect_embed().times(1).returning(|_| Ok(vec![0.1_f32]));

        let mut index = MockMockSemanticIndexPort::new();
        index.expect_insert_batch().times(1).withf(|items| items.is_empty()).returning(|_| Ok(()));
        // No candidates above threshold — work_items will be empty.
        index.expect_search().times(1).returning(|_, _| Ok(vec![]));

        // Agent must be called exactly once for the calibration probe (injection_rate=10,
        // 3 known-bad fixtures → ceil(3*10/100)=1 probe).  No production calls expected.
        // The `.times(1)` expectation is verified at mock-drop: if calibration is skipped,
        // the mock panics with "expected 1 call, got 0", catching the regression.
        let mut agent = MockMockDryCheckAgentPort::new();
        agent.expect_judge().times(1).returning(|changed, candidate, _tier| {
            assert!(
                changed.source_path.to_string_lossy().starts_with("probes/"),
                "only probe paths expected in this no-work test; got: {:?}",
                changed.source_path
            );
            let changed_ref = fragment_ref_of(changed).unwrap();
            let cand_ref = fragment_ref_of(candidate).unwrap();
            Ok(DryCheckAgentJudgment::Violation {
                rationale: Rationale::new("probe: known bad").unwrap(),
                finding: DryCheckFinding::new(changed_ref, cand_ref, "probe violation").unwrap(),
            })
        });

        let coverage = Arc::new(StubCoverage::new());
        let interactor = make_interactor_with_coverage(
            embed,
            index,
            agent,
            Arc::new(StubWriter::default()),
            vec![],
            Arc::clone(&coverage),
        );

        let result = interactor.run_dry_check(
            vec![],
            vec![diff_frag.clone()],
            make_threshold(0.8),
            make_commit(),
        );

        // Must succeed — calibration passes (probe returns Violation), no pair-level error.
        assert!(result.is_ok(), "no-work run must succeed; got: {result:?}");
        let findings = result.unwrap();
        assert!(findings.is_empty(), "no findings expected when no candidates are above threshold");

        // Coverage must be written exactly once with the current (non-fail-closed) fingerprint
        // and empty pair_keys — there is nothing to fail-close on.
        assert_eq!(
            coverage.write_call_count(),
            1,
            "coverage must be written exactly once on a no-work run"
        );
        let recorded = coverage.last_written().expect("coverage must have been written");
        assert!(
            recorded.fragment_refs().len() == 1,
            "coverage must include the single diff fragment ref"
        );
        let expected_ref = fragment_ref_of(&diff_frag).unwrap();
        assert!(recorded.covers(&expected_ref), "coverage must cover the diff fragment ref");
        assert_eq!(
            recorded.config_fingerprint(),
            &test_fingerprint(),
            "no-work run must write the real config fingerprint, not the fail-closed sentinel"
        );
        assert_eq!(
            recorded.corpus_fingerprint(),
            &test_corpus_fingerprint(),
            "no-work run must write the real corpus fingerprint, not the fail-closed sentinel"
        );
    }
}
