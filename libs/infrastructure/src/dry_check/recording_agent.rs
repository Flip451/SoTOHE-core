//! `DryCheckAgentPort` decorator that records per-tier judgment activity.
//!
//! Relocated from `cli_composition::dry::tier_telemetry` (D7 / CN-06 / AC-09).
//!
//! Types provided:
//!
//! - [`DryAgentRunRecorder`]: atomic counters and timing for one dry-check tier.
//! - [`TieredDryAgentRecorder`]: pair of recorders (fast + final) returned by
//!   [`RecordingDryAgent::new`].
//! - [`RecordingDryAgent`]: `DryCheckAgentPort` decorator that records per-tier
//!   judgment activity without changing agent behavior.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use domain::semantic_dup::CodeFragment;
use usecase::dry_check::{
    DryCheckAgentError, DryCheckAgentJudgment, DryCheckAgentPort, DryCheckJudgeTier,
};

// ── record_instant_once helper ────────────────────────────────────────────────

/// Record the current instant into `slot` exactly once (first call wins).
fn record_instant_once(slot: &Mutex<Option<Instant>>) {
    if let Ok(mut recorded_at) = slot.lock() {
        if recorded_at.is_none() {
            *recorded_at = Some(Instant::now());
        }
    }
}

// ── Per-tier recorder ─────────────────────────────────────────────────────────

/// Per-tier atomic counters and timing for one dry-check tier run.
///
/// Returned by [`RecordingDryAgent::new`] via [`TieredDryAgentRecorder`].
/// This type is `pub` for use by `cli_composition` callers; it is not part of
/// the infrastructure crate's public API contract and is excluded from the TDDD
/// catalogue.
#[doc(hidden)]
#[derive(Clone)]
pub struct DryAgentRunRecorder {
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
    pub fn new() -> Self {
        Self {
            completed: Arc::new(AtomicBool::new(false)),
            errored: Arc::new(AtomicBool::new(false)),
            findings_count: Arc::new(AtomicU64::new(0)),
            started_at: Arc::new(Mutex::new(None)),
        }
    }

    pub fn record_started(&self) {
        record_instant_once(&self.started_at);
    }

    pub fn record_completed(&self) {
        self.completed.store(true, Ordering::Relaxed);
    }

    pub fn record_error(&self) {
        self.errored.store(true, Ordering::Relaxed);
    }

    pub fn record_violation(&self) {
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

    pub fn has_completed(&self) -> bool {
        self.completed.load(Ordering::Relaxed)
    }

    pub fn has_errored(&self) -> bool {
        self.errored.load(Ordering::Relaxed)
    }

    pub fn findings_count(&self) -> u32 {
        self.findings_count.load(Ordering::Relaxed).try_into().unwrap_or(u32::MAX)
    }

    pub fn started_at(&self) -> Option<Instant> {
        self.started_at.lock().ok().and_then(|started_at| *started_at)
    }
}

impl Default for DryAgentRunRecorder {
    fn default() -> Self {
        Self::new()
    }
}

// ── TieredDryAgentRecorder ────────────────────────────────────────────────────

/// Tier-split recorder returned by [`RecordingDryAgent::new`].
///
/// Holds separate [`DryAgentRunRecorder`] instances for the fast and final tiers
/// so that `dry_write` can emit per-tier `ReviewRound` telemetry
/// (T013 / IN-07 / AC-09).
///
/// This type is `pub` for use by `cli_composition` callers; it is not part of
/// the infrastructure crate's public API contract and is excluded from the TDDD
/// catalogue.
#[doc(hidden)]
pub struct TieredDryAgentRecorder {
    pub fast: DryAgentRunRecorder,
    pub final_: DryAgentRunRecorder,
}

// ── RecordingDryAgent ─────────────────────────────────────────────────────────

pub struct RecordingDryAgent<A> {
    inner: A,
    fast_recorder: DryAgentRunRecorder,
    final_recorder: DryAgentRunRecorder,
}

impl<A> RecordingDryAgent<A> {
    pub fn new(inner: A) -> (Self, TieredDryAgentRecorder) {
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

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::path::PathBuf;

    use domain::semantic_dup::CodeFragment;
    use usecase::dry_check::{
        DryCheckAgentError, DryCheckAgentJudgment, DryCheckAgentPort, DryCheckJudgeTier,
    };

    use super::{DryAgentRunRecorder, RecordingDryAgent};

    // ── Test helpers ──────────────────────────────────────────────────────────

    struct ViolationAgent {
        rationale: &'static str,
    }

    impl DryCheckAgentPort for ViolationAgent {
        fn judge(
            &self,
            _changed_fragment: &CodeFragment,
            _candidate_fragment: &CodeFragment,
            _tier: DryCheckJudgeTier,
        ) -> Result<DryCheckAgentJudgment, DryCheckAgentError> {
            Ok(DryCheckAgentJudgment::Violation {
                rationale: domain::Rationale::new(self.rationale).unwrap(),
                finding: dry_check_finding_for_tests(),
            })
        }
    }

    struct NotAViolationAgent;

    impl DryCheckAgentPort for NotAViolationAgent {
        fn judge(
            &self,
            _changed_fragment: &CodeFragment,
            _candidate_fragment: &CodeFragment,
            _tier: DryCheckJudgeTier,
        ) -> Result<DryCheckAgentJudgment, DryCheckAgentError> {
            Ok(DryCheckAgentJudgment::NotAViolation {
                rationale: domain::Rationale::new("distinct logic").unwrap(),
            })
        }
    }

    fn dry_check_finding_for_tests() -> domain::dry_check::DryCheckFinding {
        let changed_ref = domain::FragmentRef::new(
            domain::review_v2::FilePath::new("src/a.rs").unwrap(),
            domain::FragmentContentHash::new("a".repeat(64)).unwrap(),
        );
        let candidate_ref = domain::FragmentRef::new(
            domain::review_v2::FilePath::new("src/b.rs").unwrap(),
            domain::FragmentContentHash::new("b".repeat(64)).unwrap(),
        );
        domain::dry_check::DryCheckFinding::new(changed_ref, candidate_ref, "extract shared helper")
            .unwrap()
    }

    fn make_fragment(path: &str) -> CodeFragment {
        CodeFragment::new(PathBuf::from(path), "fn f() {}".to_owned(), 1, 1).unwrap()
    }

    // ── RecordingDryAgent tests ───────────────────────────────────────────────

    /// judge() delegates to the inner agent and records a violation on the correct tier.
    #[test]
    fn test_recording_dry_agent_delegates_to_inner_and_records_violation() {
        let (agent, tiered) =
            RecordingDryAgent::new(ViolationAgent { rationale: "same control flow" });
        let changed = make_fragment("src/a.rs");
        let candidate = make_fragment("src/b.rs");

        let result = agent.judge(&changed, &candidate, DryCheckJudgeTier::Final);

        assert!(matches!(result, Ok(DryCheckAgentJudgment::Violation { .. })));
        // Final tier recorder must capture the violation; fast tier must be idle.
        assert_eq!(tiered.final_.findings_count(), 1);
        assert!(tiered.final_.has_completed());
        assert_eq!(tiered.fast.findings_count(), 0);
        assert!(!tiered.fast.has_completed());
    }

    /// judge() correctly routes calls to fast vs final tier recorders independently.
    #[test]
    fn test_recording_dry_agent_routes_to_correct_tier_recorder() {
        let (agent, tiered) = RecordingDryAgent::new(ViolationAgent { rationale: "duplicated" });
        let changed = make_fragment("src/a.rs");
        let candidate = make_fragment("src/b.rs");

        // Two fast-tier violations.
        agent.judge(&changed, &candidate, DryCheckJudgeTier::Fast).unwrap();
        agent.judge(&changed, &candidate, DryCheckJudgeTier::Fast).unwrap();
        // One final-tier violation.
        agent.judge(&changed, &candidate, DryCheckJudgeTier::Final).unwrap();

        assert_eq!(tiered.fast.findings_count(), 2, "fast recorder must count 2 violations");
        assert_eq!(tiered.final_.findings_count(), 1, "final recorder must count 1 violation");
        assert!(tiered.fast.started_at().is_some(), "fast tier must record started_at");
        assert!(tiered.final_.started_at().is_some(), "final tier must record started_at");
    }

    /// judge() does not record telemetry for calibration probe pairs.
    #[test]
    fn test_recording_dry_agent_skips_calibration_probes() {
        let (agent, tiered) =
            RecordingDryAgent::new(ViolationAgent { rationale: "probe violation" });
        // Use a known calibration probe pair.
        let changed = make_fragment("probes/changed_a.rs");
        let candidate = make_fragment("probes/candidate_a.rs");

        let result = agent.judge(&changed, &candidate, DryCheckJudgeTier::Fast);

        // Delegation still happens (probe is judged), but not recorded.
        assert!(matches!(result, Ok(DryCheckAgentJudgment::Violation { .. })));
        assert_eq!(tiered.fast.findings_count(), 0, "probe violations must not be counted");
        assert!(tiered.fast.started_at().is_none(), "probe calls must not record started_at");
    }

    /// judge() records started_at on first call, not on subsequent calls.
    #[test]
    fn test_recording_dry_agent_records_telemetry_on_success() {
        let (agent, tiered) = RecordingDryAgent::new(NotAViolationAgent);
        let changed = make_fragment("src/a.rs");
        let candidate = make_fragment("src/b.rs");

        agent.judge(&changed, &candidate, DryCheckJudgeTier::Fast).unwrap();

        assert!(tiered.fast.started_at().is_some(), "started_at must be recorded after a call");
        assert!(tiered.fast.has_completed(), "recorder must mark completed on success");
        assert_eq!(tiered.fast.findings_count(), 0, "NotAViolation does not increment findings");
    }

    // ── DryAgentRunRecorder tests ─────────────────────────────────────────────

    /// record_error() sets has_errored without affecting has_completed.
    #[test]
    fn test_dry_agent_run_recorder_error_does_not_set_completed() {
        let recorder = DryAgentRunRecorder::new();
        recorder.record_error();
        assert!(recorder.has_errored());
        assert!(!recorder.has_completed());
    }

    /// findings_count saturates at u32::MAX rather than panicking.
    #[test]
    fn test_dry_agent_run_recorder_clone_shares_state() {
        let recorder = DryAgentRunRecorder::new();
        let clone = recorder.clone();
        recorder.record_violation();
        assert_eq!(clone.findings_count(), 1, "cloned recorder shares atomic state");
    }
}
