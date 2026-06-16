//! D4 calibration barrier helpers (T012).
//!
//! This module houses the parallel judgment runner and the calibration logic
//! that was extracted from [`super::interactor`] to keep that module under the
//! 700-line production-code limit.

use domain::semantic_dup::CodeFragment;

use super::errors::DryCheckAgentError;
use super::judgment::DryCheckAgentJudgment;
use super::known_bad::KnownBadProbePair;
use super::ports::{DryCheckAgentPort, DryCheckJudgeTier};

// ── Parallel judgment runner ──────────────────────────────────────────────────

/// Run `agent.judge(diff_fragment, candidate_fragment, tier)` for each pair in
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
/// `tier` is passed through to every `judge()` call.
///
/// Errors from individual `judge()` calls are preserved in the result vector; they
/// are not collapsed here.
pub(super) fn run_parallel_judgments(
    fragment_pairs: &[(&CodeFragment, &CodeFragment)],
    agent: &dyn DryCheckAgentPort,
    max_parallelism: usize,
    tier: DryCheckJudgeTier,
) -> Vec<Result<DryCheckAgentJudgment, DryCheckAgentError>> {
    use std::sync::mpsc;

    let n = fragment_pairs.len();
    if n == 0 {
        return Vec::new();
    }

    // Clamp chunk size: at least 1 so we always make progress.
    let chunk_size = max_parallelism.max(1);

    // Pre-allocate output slots indexed by pair position.
    let mut results: Vec<Option<Result<DryCheckAgentJudgment, DryCheckAgentError>>> =
        (0..n).map(|_| None).collect();

    for (chunk_start, chunk) in fragment_pairs.chunks(chunk_size).enumerate() {
        // Channel scoped to this chunk: each thread sends (chunk-local index, result).
        let (tx, rx) =
            mpsc::channel::<(usize, Result<DryCheckAgentJudgment, DryCheckAgentError>)>();

        // `std::thread::scope` joins all threads in this chunk before continuing
        // to the next chunk.  At most `chunk_size` (= `max_parallelism`) OS threads
        // are live at once because the scope covers exactly one chunk.
        std::thread::scope(|scope| {
            for (chunk_i, (diff_fragment, candidate_fragment)) in chunk.iter().enumerate() {
                let tx_clone = tx.clone();
                scope.spawn(move || {
                    let judgment = agent.judge(diff_fragment, candidate_fragment, tier);
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
            opt.unwrap_or(Err(DryCheckAgentError::Unexpected(
                "internal: judgment result missing after parallel run".to_owned(),
            )))
        })
        .collect()
}

// ── Calibration probe runner ──────────────────────────────────────────────────

/// Run calibration probes and return true if detection rate >= threshold_percent.
///
/// If `probe_pairs` is empty, returns `true` (vacuously calibrated — nothing to check).
///
/// Probe failures (agent errors) count as non-detections — they reduce the detection
/// rate but are NOT propagated into `first_error` (probes are calibration-only and do
/// not affect production error accounting).
pub(super) fn run_calibration_probes(
    agent: &dyn DryCheckAgentPort,
    probe_pairs: &[KnownBadProbePair],
    tier: DryCheckJudgeTier,
    threshold_percent: usize,
) -> bool {
    if probe_pairs.is_empty() {
        return true;
    }
    let mut correctly_detected = 0usize;
    for probe in probe_pairs {
        if let Ok(DryCheckAgentJudgment::Violation { .. }) =
            agent.judge(&probe.changed, &probe.candidate, tier)
        {
            correctly_detected += 1;
        }
    }
    // Integer arithmetic: detected_pct = correctly_detected * 100 / probe_count
    // Pass when detected_pct >= threshold_percent, i.e.
    //   correctly_detected * 100 >= probe_pairs.len() * threshold_percent
    (correctly_detected * 100) >= (probe_pairs.len() * threshold_percent)
}

// ── Final-tier escalation ─────────────────────────────────────────────────────

/// Promote fast judgment results, re-running `Violation` and `Err` pairs with Final tier.
///
/// `NotAViolation` and `Accepted` from the fast tier are trusted without escalation.
/// Only pairs whose fast result was `Violation { .. }` or `Err(_)` are re-run.
pub(super) fn escalate_violations_to_final(
    fragment_pairs: &[(&CodeFragment, &CodeFragment)],
    fast_results: Vec<Result<DryCheckAgentJudgment, DryCheckAgentError>>,
    agent: &dyn DryCheckAgentPort,
    max_parallelism: usize,
) -> Vec<Result<DryCheckAgentJudgment, DryCheckAgentError>> {
    // Collect indices that need escalation.
    let escalation_indices: Vec<usize> = fast_results
        .iter()
        .enumerate()
        .filter(|(_, r)| matches!(r, Ok(DryCheckAgentJudgment::Violation { .. }) | Err(_)))
        .map(|(i, _)| i)
        .collect();

    if escalation_indices.is_empty() {
        return fast_results;
    }

    // Build sub-pairs for escalation.
    let escalation_pairs: Vec<(&CodeFragment, &CodeFragment)> =
        escalation_indices.iter().filter_map(|&i| fragment_pairs.get(i).copied()).collect();

    let escalation_results =
        run_parallel_judgments(&escalation_pairs, agent, max_parallelism, DryCheckJudgeTier::Final);

    // Merge Final results back into the fast_results vector.
    let mut merged = fast_results;
    for (escalation_pos, &original_i) in escalation_indices.iter().enumerate() {
        if let (Some(slot), Some(result)) =
            (merged.get_mut(original_i), escalation_results.get(escalation_pos))
        {
            *slot = result.clone();
        }
    }
    merged
}
