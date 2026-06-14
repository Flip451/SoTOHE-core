//! Per-tier `ReviewRound` telemetry for `sotp dry write` (T013 / IN-07 / AC-09).
//!
//! This module houses the tiered recorder types and telemetry helpers extracted
//! from `super::dry` to keep the composition module within the 700-line limit:
//!
//! - [`DryAgentRunRecorder`]: atomic counters and timing for one dry-check tier.
//! - [`TieredDryAgentRecorder`]: pair of recorders (fast + final) returned by
//!   [`RecordingDryAgent::new`].
//! - [`RecordingDryAgent`]: `DryCheckAgentPort` decorator that records per-tier
//!   judgment activity without changing agent behavior.
//! - [`DryRoundTelemetry`]: telemetry payload built from a recorder snapshot.
//! - [`dry_tiered_telemetry_for_result`]: builds `(fast, final_)` telemetry from
//!   the cycle result and tiered recorder.
//! - [`dry_agent_error_is_subprocess_failure`]: classifies agent errors as
//!   subprocess failures for telemetry attribution.
//! - [`emit_dry_tier_review_round`]: emits a `ReviewRound` event with
//!   pre-computed or elapsed duration.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use domain::dry_check::DryCheckFinding;
use domain::semantic_dup::CodeFragment;
use usecase::dry_check::{
    DryCheckAgentError, DryCheckAgentJudgment, DryCheckAgentPort, DryCheckCycleError,
    DryCheckJudgeTier,
};

use crate::review_v2::record_instant_once;

// ── Per-tier recorder ─────────────────────────────────────────────────────────

#[derive(Clone)]
pub(super) struct DryAgentRunRecorder {
    completed: Arc<AtomicBool>,
    /// Set to `true` when any `judge()` call for this tier returns `Err(_)`.
    ///
    /// Tracks the tier of a subprocess error directly, avoiding the sticky
    /// `completed` flag which would misattribute the error tier when one
    /// final-tier call completes successfully and a later call fails.
    errored: Arc<AtomicBool>,
    findings_count: Arc<AtomicU64>,
    started_at: Arc<Mutex<Option<Instant>>>,
}

impl DryAgentRunRecorder {
    pub(super) fn new() -> Self {
        Self {
            completed: Arc::new(AtomicBool::new(false)),
            errored: Arc::new(AtomicBool::new(false)),
            findings_count: Arc::new(AtomicU64::new(0)),
            started_at: Arc::new(Mutex::new(None)),
        }
    }

    pub(super) fn record_started(&self) {
        record_instant_once(&self.started_at);
    }

    pub(super) fn record_completed(&self) {
        self.completed.store(true, Ordering::Relaxed);
    }

    pub(super) fn record_error(&self) {
        self.errored.store(true, Ordering::Relaxed);
    }

    pub(super) fn record_violation(&self) {
        let mut current = self.findings_count.load(Ordering::Relaxed);
        while current < u64::from(u32::MAX) {
            match self.findings_count.compare_exchange_weak(
                current,
                current + 1,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => return,
                Err(next) => current = next,
            }
        }
    }

    pub(super) fn has_completed(&self) -> bool {
        self.completed.load(Ordering::Relaxed)
    }

    pub(super) fn has_errored(&self) -> bool {
        self.errored.load(Ordering::Relaxed)
    }

    pub(super) fn findings_count(&self) -> u32 {
        self.findings_count.load(Ordering::Relaxed).try_into().unwrap_or(u32::MAX)
    }

    pub(super) fn started_at(&self) -> Option<Instant> {
        self.started_at.lock().ok().and_then(|started_at| *started_at)
    }
}

// ── TieredDryAgentRecorder ────────────────────────────────────────────────────

/// Tier-split recorder returned by [`RecordingDryAgent::new`].
///
/// Holds separate [`DryAgentRunRecorder`] instances for the fast and final tiers
/// so that `dry_write` can emit per-tier `ReviewRound` telemetry
/// (T013 / IN-07 / AC-09).
pub(super) struct TieredDryAgentRecorder {
    pub(super) fast: DryAgentRunRecorder,
    pub(super) final_: DryAgentRunRecorder,
}

// ── RecordingDryAgent ─────────────────────────────────────────────────────────

pub(super) struct RecordingDryAgent<A> {
    inner: A,
    fast_recorder: DryAgentRunRecorder,
    final_recorder: DryAgentRunRecorder,
}

impl<A> RecordingDryAgent<A> {
    pub(super) fn new(inner: A) -> (Self, TieredDryAgentRecorder) {
        let fast_recorder = DryAgentRunRecorder::new();
        let final_recorder = DryAgentRunRecorder::new();
        let tiered =
            TieredDryAgentRecorder { fast: fast_recorder.clone(), final_: final_recorder.clone() };
        (Self { inner, fast_recorder, final_recorder }, tiered)
    }
}

/// Returns `true` when `(changed, candidate)` is a known-bad calibration probe pair.
///
/// Calibration probe pairs are identified by both fragment paths together.
/// The pairs are hard-coded in `usecase::dry_check::known_bad::known_bad_probe_pairs`
/// and are stable by design — the probe corpus is a fixed in-memory fixture.
///
/// The known probe pairs (changed_path, candidate_path) are:
/// - `(probes/changed_a.rs, probes/candidate_a.rs)`
/// - `(probes/changed_b.rs, probes/candidate_b.rs)`
/// - `(probes/changed_c.rs, probes/candidate_c.rs)`
///
/// Matching the full pair (rather than any single fragment path) ensures that
/// a legitimate production pair involving e.g. `probes/changed_a.rs` alongside
/// a different candidate file is still correctly recorded — only the exact
/// synthetic fixture pairs are excluded.
///
/// Recording calibration probe calls would inflate `started_at` (causing
/// tier telemetry to be emitted even when no production pairs were judged) and
/// inflate `findings_count` (probes are expected to return `Violation`, which
/// is correct agent behaviour, not a production finding).  Both are excluded here.
fn is_calibration_probe_pair(changed: &CodeFragment, candidate: &CodeFragment) -> bool {
    const PROBE_PAIRS: &[(&str, &str)] = &[
        ("probes/changed_a.rs", "probes/candidate_a.rs"),
        ("probes/changed_b.rs", "probes/candidate_b.rs"),
        ("probes/changed_c.rs", "probes/candidate_c.rs"),
    ];
    let changed_str = changed.source_path.to_string_lossy();
    let candidate_str = candidate.source_path.to_string_lossy();
    PROBE_PAIRS.iter().any(|&(ch, ca)| changed_str == ch && candidate_str == ca)
}

impl<A: DryCheckAgentPort> DryCheckAgentPort for RecordingDryAgent<A> {
    fn judge(
        &self,
        changed_fragment: &CodeFragment,
        candidate_fragment: &CodeFragment,
        tier: DryCheckJudgeTier,
    ) -> Result<DryCheckAgentJudgment, DryCheckAgentError> {
        // Skip recording for calibration probe calls so that tier telemetry is
        // only emitted when production pairs were actually judged (F2) and
        // `findings_count` reflects production violations only (F1).
        if is_calibration_probe_pair(changed_fragment, candidate_fragment) {
            return self.inner.judge(changed_fragment, candidate_fragment, tier);
        }

        let recorder = match tier {
            DryCheckJudgeTier::Fast => &self.fast_recorder,
            DryCheckJudgeTier::Final => &self.final_recorder,
        };
        recorder.record_started();
        let result = self.inner.judge(changed_fragment, candidate_fragment, tier);
        match &result {
            Ok(judgment) => {
                if matches!(judgment, DryCheckAgentJudgment::Violation { .. }) {
                    recorder.record_violation();
                }
                recorder.record_completed();
            }
            Err(_) => {
                // Record the error on the tier that produced it so that
                // `has_errored()` can be used to attribute subprocess failures
                // accurately, even when earlier calls on the same tier succeeded.
                recorder.record_error();
            }
        }
        result
    }
}

// ── DryRoundTelemetry ─────────────────────────────────────────────────────────

pub(super) struct DryRoundTelemetry {
    pub(super) findings_count: u32,
    pub(super) verdict_parse_failed: bool,
    pub(super) subprocess_started_at: Option<std::time::Instant>,
    /// Pre-computed duration in milliseconds for this tier.
    ///
    /// `Some(ms)` is used when the tier's end time can be determined precisely
    /// (e.g. fast tier on escalated runs: duration = final_started_at - fast_started_at).
    /// `None` means the caller should use `round_started_at.elapsed()` at emission time.
    pub(super) duration_ms: Option<u64>,
    /// The `Instant` to use for elapsed duration when `duration_ms` is `None`.
    pub(super) round_started_at: Option<std::time::Instant>,
}

// ── dry_tiered_telemetry_for_result ──────────────────────────────────────────

/// Build per-tier telemetry from the interactor result and the tier-split recorder.
///
/// Returns `(fast_telemetry, final_telemetry)`.
/// - `fast_telemetry` is `Some` when the fast tier was invoked for production pairs
///   (calibration-probe-only runs do not set `started_at`).
/// - `final_telemetry` is `Some` only when the final recorder shows production
///   activity (started_at is `Some` after a production final-tier call).
///
/// Calibration probes are excluded from recording by `RecordingDryAgent::judge`
/// (the `is_calibration_probe` guard) so every recorder field reflects only
/// production judgments.
///
/// On escalated runs the fast tier duration is pre-computed as
/// `(final_started_at - fast_started_at)` so that the fast `ReviewRound` event
/// does not erroneously include the final tier's processing time.
///
/// `findings_count` uses per-tier recorder counts for all paths.  Fast
/// `findings_count` is the number of `Violation` judgments returned by the fast
/// tier before any final-tier arbitration.
pub(super) fn dry_tiered_telemetry_for_result(
    dry_result: &Result<Vec<DryCheckFinding>, DryCheckCycleError>,
    tiered: &TieredDryAgentRecorder,
) -> (Option<DryRoundTelemetry>, Option<DryRoundTelemetry>) {
    fn build_fast(
        fast_started_at: Instant,
        final_started_at: Option<Instant>,
        findings_count: u32,
        verdict_parse_failed: bool,
    ) -> DryRoundTelemetry {
        let duration_ms = final_started_at.map(|final_start| {
            final_start.duration_since(fast_started_at).as_millis().try_into().unwrap_or(u64::MAX)
        });
        let round_started_at = if duration_ms.is_none() { Some(fast_started_at) } else { None };
        DryRoundTelemetry {
            findings_count,
            verdict_parse_failed,
            subprocess_started_at: Some(fast_started_at),
            duration_ms,
            round_started_at,
        }
    }

    fn build_final(
        final_started_at: Instant,
        findings_count: u32,
        verdict_parse_failed: bool,
    ) -> DryRoundTelemetry {
        DryRoundTelemetry {
            findings_count,
            verdict_parse_failed,
            subprocess_started_at: Some(final_started_at),
            duration_ms: None,
            round_started_at: Some(final_started_at),
        }
    }

    let final_started_at = tiered.final_.started_at();

    match dry_result {
        Ok(_findings) => {
            // Use per-tier recorder counts.  Calibration probes are excluded from
            // recording by `RecordingDryAgent::judge` (the `is_calibration_probe`
            // guard), so the recorder counts reflect only production judgments.
            //
            // Fast `findings_count` = violations flagged by the fast tier before
            // final-tier arbitration.  When fast flags a pair and final downgrades
            // it to `NotAViolation`, the fast `ReviewRound` still records the fast
            // tier's own count — not the final accepted count.
            let fast = tiered.fast.started_at().map(|started_at| {
                build_fast(started_at, final_started_at, tiered.fast.findings_count(), false)
            });
            let final_ = final_started_at
                .map(|started_at| build_final(started_at, tiered.final_.findings_count(), false));
            (fast, final_)
        }
        Err(DryCheckCycleError::Agent(inner)) => {
            if dry_agent_error_is_subprocess_failure(inner) {
                let is_parse_failure = matches!(inner, DryCheckAgentError::IllegalOutput);
                // Attribute verdict_parse_failed only to the tier that produced
                // the error. `has_errored()` is set directly by `RecordingDryAgent`
                // when `judge()` returns `Err(_)`, so it stays accurate even when
                // earlier calls on the same tier completed successfully.
                let final_is_failing = tiered.final_.has_errored();
                let fast = tiered.fast.started_at().map(|started_at| {
                    build_fast(
                        started_at,
                        final_started_at,
                        tiered.fast.findings_count(),
                        is_parse_failure && !final_is_failing,
                    )
                });
                let final_ = final_started_at.map(|started_at| {
                    build_final(
                        started_at,
                        tiered.final_.findings_count(),
                        is_parse_failure && final_is_failing,
                    )
                });
                (fast, final_)
            } else {
                (None, None)
            }
        }
        // Entry and Writer errors occur after the agent ran.
        Err(DryCheckCycleError::Entry(_)) | Err(DryCheckCycleError::Writer(_)) => {
            let fast = tiered.fast.started_at().map(|started_at| {
                build_fast(started_at, final_started_at, tiered.fast.findings_count(), false)
            });
            let final_ = final_started_at
                .map(|started_at| build_final(started_at, tiered.final_.findings_count(), false));
            (fast, final_)
        }
        // Index error after a successful agent call.
        Err(DryCheckCycleError::Index(_))
            if tiered.fast.has_completed() || tiered.final_.has_completed() =>
        {
            let fast = tiered.fast.started_at().map(|started_at| {
                build_fast(started_at, final_started_at, tiered.fast.findings_count(), false)
            });
            let final_ = final_started_at
                .map(|started_at| build_final(started_at, tiered.final_.findings_count(), false));
            (fast, final_)
        }
        Err(_) => (None, None),
    }
}

// ── subprocess error classification ──────────────────────────────────────────

/// Returns `true` when the agent error is attributable to a subprocess failure
/// (the external process was spawned, ran, and returned an error or bad output).
///
/// Pre-spawn errors are excluded so that telemetry is not emitted for cases
/// where the agent process never started.
pub(super) fn dry_agent_error_is_subprocess_failure(error: &DryCheckAgentError) -> bool {
    match error {
        DryCheckAgentError::UserAbort
        | DryCheckAgentError::AgentAbort
        | DryCheckAgentError::Timeout
        | DryCheckAgentError::IllegalOutput => true,
        DryCheckAgentError::Unexpected(message) => dry_agent_unexpected_after_spawn(message),
    }
}

fn dry_agent_unexpected_after_spawn(message: &str) -> bool {
    message.starts_with("failed to poll dry-check agent child:")
        || message.starts_with("failed to reap dry-check agent child:")
        || message.starts_with("failed to read output-last-message ")
}

// ── dry_duration_ms ───────────────────────────────────────────────────────────

/// Resolve the duration in milliseconds for a dry-check tier telemetry event.
///
/// Uses `duration_ms` when pre-computed (escalated runs where the fast tier's
/// end time equals the final tier's start time).  Falls back to
/// `started_at.elapsed()` at emission time, or to `fallback_start.elapsed()`
/// when `started_at` is also `None`.
///
/// `fallback_start` is only used when both `duration_ms` and `started_at` are
/// `None` — which should not occur in well-formed [`DryRoundTelemetry`] structs.
fn dry_duration_ms(
    duration_ms: Option<u64>,
    started_at: Option<Instant>,
    fallback_start: Instant,
) -> u64 {
    match duration_ms {
        Some(ms) => ms,
        None => {
            let start = started_at.unwrap_or(fallback_start);
            start.elapsed().as_millis().try_into().unwrap_or(u64::MAX)
        }
    }
}

// ── emit_dry_tier_review_round ────────────────────────────────────────────────

/// Emit a `ReviewRound` telemetry event for a dry-check tier (T013 / IN-07).
///
/// Uses the pre-computed `duration_ms` in the telemetry when available (escalated
/// runs where the fast tier's end time equals the final tier's start time), falling
/// back to `round_started_at.elapsed()` for tiers whose end time is the current
/// moment (typically the final tier or a fast-only run).
///
/// `fallback_start` is only used when both `duration_ms` and `round_started_at`
/// are `None` — which should not happen in well-formed telemetry structs.
pub(super) fn emit_dry_tier_review_round(
    writer: &infrastructure::telemetry::TelemetryWriter,
    track_id: &str,
    provider: &str,
    model: &str,
    round_type: &str,
    telemetry: &DryRoundTelemetry,
    fallback_start: Instant,
) {
    let duration_ms =
        dry_duration_ms(telemetry.duration_ms, telemetry.round_started_at, fallback_start);
    use infrastructure::telemetry::TelemetryEvent;
    let event = TelemetryEvent::ReviewRound {
        schema_version: 1,
        track_id: track_id.to_string(),
        provider: provider.to_string(),
        model: model.to_string(),
        round_type: round_type.to_string(),
        duration_ms,
        findings_count: telemetry.findings_count,
        timestamp: crate::telemetry_wiring::now_timestamp(),
    };
    // Fire-and-forget: suppress errors per CN-01.
    let _ = writer.write(event);
}

// ── emit_dry_tier_external_subprocess ────────────────────────────────────────

/// Emit an `ExternalSubprocess` telemetry event for a dry-check tier (T013 / IN-07).
///
/// Uses the same duration logic as [`emit_dry_tier_review_round`] via
/// [`dry_duration_ms`]: when `telemetry.duration_ms` is `Some(ms)`, the
/// pre-computed duration is used so that the fast-tier event on escalated runs
/// does not include final-tier processing time. When `duration_ms` is `None`,
/// the elapsed time since `telemetry.subprocess_started_at` (or `fallback_start`)
/// is used.
///
/// Emits only when `telemetry.subprocess_started_at` is `Some` (i.e., the
/// subprocess was actually launched for this tier).
///
/// `fallback_start` is used when both `duration_ms` and `subprocess_started_at`
/// are `None` — which should not occur in well-formed telemetry structs.
pub(super) fn emit_dry_tier_external_subprocess(
    writer: &infrastructure::telemetry::TelemetryWriter,
    track_id: &str,
    command: &str,
    telemetry: &DryRoundTelemetry,
    fallback_start: Instant,
) {
    let duration_ms =
        dry_duration_ms(telemetry.duration_ms, telemetry.subprocess_started_at, fallback_start);
    use infrastructure::telemetry::TelemetryEvent;
    let event = TelemetryEvent::ExternalSubprocess {
        schema_version: 1,
        track_id: track_id.to_string(),
        command: command.to_string(),
        duration_ms,
        retry_count: 0,
        verdict_parse_failed: telemetry.verdict_parse_failed,
        timestamp: crate::telemetry_wiring::now_timestamp(),
    };
    // Fire-and-forget: suppress errors per CN-01.
    let _ = writer.write(event);
}
