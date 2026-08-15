//! Infrastructure decorator that records successful structured review rounds.

use std::sync::{Arc, Mutex};
use std::time::Instant;

use domain::review_v2::{FastVerdict, LogInfo, ReviewTarget, RoundType, Verdict};
use usecase::review_v2::{ResolvedReviewer, Reviewer, ReviewerError};
use usecase::telemetry::review_yield::ReviewFindingCount;

use crate::telemetry::{
    StructuredReviewRoundDto, TelemetryConfig, TelemetryEvent, TelemetryWriter,
};

/// Reviewer decorator that records every successfully parsed verdict.
pub struct ReviewYieldRecordingReviewer<R> {
    inner: R,
    writer: Option<Arc<TelemetryWriter>>,
    invocation_started_at: Arc<Mutex<Option<Instant>>>,
}

impl<R: ResolvedReviewer> ReviewYieldRecordingReviewer<R> {
    /// Constructs a recorder whose telemetry writer is bound to the wrapped
    /// reviewer's resolved track identity.
    ///
    /// The caller supplies only identity-free writer inputs. Deriving the
    /// writer track from `inner` makes a recorder with a mismatched reviewer
    /// assignment and telemetry sink impossible to construct.
    #[must_use]
    pub fn new(inner: R, config: TelemetryConfig, items_dir: std::path::PathBuf) -> Self {
        let track_id = inner.resolved_assignment().track_id().clone();
        let writer = Arc::new(TelemetryWriter::new(config, track_id, items_dir));
        Self { inner, writer: Some(writer), invocation_started_at: Arc::new(Mutex::new(None)) }
    }

    /// Constructs a reviewer decorator that captures the first invocation
    /// timestamp without emitting a structured `ReviewRound` event.
    ///
    /// Provider-specific review entry points are excluded from review-yield
    /// measurement, but still emit `ExternalSubprocess` diagnostics. They use
    /// this mode so the diagnostic duration has the same reviewer-owned start
    /// boundary as the measured local-review path.
    #[must_use]
    pub fn new_for_subprocess_timing(inner: R) -> Self {
        Self { inner, writer: None, invocation_started_at: Arc::new(Mutex::new(None)) }
    }

    /// Returns a reader for the first reviewer-invocation timestamp.
    ///
    /// The reader can be retained while this recorder is injected into a
    /// `ReviewCycle`; it returns the same timestamp used for review-yield
    /// duration measurement once a reviewer method has been invoked.
    #[must_use]
    pub fn subprocess_started_at_reader(
        &self,
    ) -> Box<dyn Fn() -> Option<Instant> + Send + Sync + 'static> {
        let invocation_started_at = Arc::clone(&self.invocation_started_at);
        Box::new(move || invocation_started_at.lock().ok().and_then(|started_at| *started_at))
    }

    fn record_first_invocation(&self) -> Instant {
        let now = Instant::now();
        match self.invocation_started_at.lock() {
            Ok(mut started_at) => *started_at.get_or_insert(now),
            Err(_) => now,
        }
    }

    fn record_round(
        &self,
        round_type: RoundType,
        findings_count: ReviewFindingCount,
        start: Instant,
    ) {
        let Some(writer) = &self.writer else {
            return;
        };
        let round = StructuredReviewRoundDto::new(
            self.inner.resolved_assignment(),
            round_type,
            findings_count,
        );
        let event = TelemetryEvent::ReviewRound {
            schema_version: 1,
            track_id: writer.track_id().as_ref().to_owned(),
            round,
            duration_ms: start.elapsed().as_millis().try_into().unwrap_or(u64::MAX),
            timestamp: chrono::Utc::now().to_rfc3339(),
        };
        // Review telemetry is diagnostic-only. A sink failure must not change
        // the review result or turn a successful verdict into an error.
        let _ = writer.write(event);
    }
}

impl<R: ResolvedReviewer> Reviewer for ReviewYieldRecordingReviewer<R> {
    fn review(&self, target: &ReviewTarget) -> Result<(Verdict, LogInfo), ReviewerError> {
        let start = self.record_first_invocation();
        let result = self.inner.review(target);
        if let Ok((verdict, _)) = &result {
            self.record_round(RoundType::Final, findings_count_final(verdict), start);
        }
        result
    }

    fn fast_review(&self, target: &ReviewTarget) -> Result<(FastVerdict, LogInfo), ReviewerError> {
        let start = self.record_first_invocation();
        let result = self.inner.fast_review(target);
        if let Ok((verdict, _)) = &result {
            self.record_round(RoundType::Fast, findings_count_fast(verdict), start);
        }
        result
    }
}

fn findings_count_final(verdict: &Verdict) -> ReviewFindingCount {
    let count = match verdict {
        Verdict::ZeroFindings => 0,
        Verdict::FindingsRemain(findings) => {
            findings.as_slice().len().try_into().unwrap_or(u32::MAX)
        }
    };
    ReviewFindingCount::new(count)
}

fn findings_count_fast(verdict: &FastVerdict) -> ReviewFindingCount {
    let count = match verdict {
        FastVerdict::ZeroFindings => 0,
        FastVerdict::FindingsRemain(findings) => {
            findings.as_slice().len().try_into().unwrap_or(u32::MAX)
        }
    };
    ReviewFindingCount::new(count)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::telemetry::TelemetryConfig;
    use domain::TrackId;
    use domain::review_v2::{MainScopeName, ScopeName};
    use std::time::Duration;
    use tempfile::TempDir;
    use usecase::capability_exec::{ModelName, ProviderName, ReasoningEffort};
    use usecase::review_v2::ResolvedReviewerAssignment;

    #[derive(Clone, Copy)]
    enum StubOutcome {
        ZeroFindings,
        Findings,
        Timeout,
    }

    struct StubReviewer {
        assignment: ResolvedReviewerAssignment,
        outcome: StubOutcome,
    }

    impl Reviewer for StubReviewer {
        fn review(&self, _: &ReviewTarget) -> Result<(Verdict, LogInfo), ReviewerError> {
            match self.outcome {
                StubOutcome::ZeroFindings => Ok((Verdict::ZeroFindings, LogInfo::new("final"))),
                StubOutcome::Findings => {
                    let finding =
                        domain::review_v2::ReviewerFinding::new("finding", None, None, None, None)
                            .expect("valid finding");
                    Ok((
                        Verdict::findings_remain(vec![finding]).expect("non-empty verdict"),
                        LogInfo::new("final"),
                    ))
                }
                StubOutcome::Timeout => Err(ReviewerError::Timeout),
            }
        }

        fn fast_review(&self, _: &ReviewTarget) -> Result<(FastVerdict, LogInfo), ReviewerError> {
            match self.outcome {
                StubOutcome::ZeroFindings => Ok((FastVerdict::ZeroFindings, LogInfo::new("fast"))),
                StubOutcome::Findings => {
                    let finding =
                        domain::review_v2::ReviewerFinding::new("finding", None, None, None, None)
                            .expect("valid finding");
                    Ok((
                        FastVerdict::findings_remain(vec![finding]).expect("non-empty verdict"),
                        LogInfo::new("fast"),
                    ))
                }
                StubOutcome::Timeout => Err(ReviewerError::Timeout),
            }
        }
    }

    impl ResolvedReviewer for StubReviewer {
        fn resolved_assignment(&self) -> &ResolvedReviewerAssignment {
            &self.assignment
        }
    }

    fn assignment(track_id: &str) -> ResolvedReviewerAssignment {
        ResolvedReviewerAssignment::new(
            TrackId::try_new(track_id).expect("valid track id"),
            ScopeName::Main(MainScopeName::new("infrastructure").expect("valid scope")),
            ProviderName::try_new("codex").expect("valid provider"),
            ModelName::try_new("gpt-5.4").expect("valid model"),
            ReasoningEffort::High,
        )
    }

    fn telemetry_line(temp_dir: &TempDir) -> serde_json::Value {
        let content = std::fs::read_to_string(temp_dir.path().join("telemetry.jsonl"))
            .expect("telemetry line written");
        serde_json::from_str(content.trim()).expect("valid telemetry JSON")
    }

    #[test]
    fn test_review_yield_recording_reviewer_records_zero_findings_final_verdict() {
        let temp_dir = tempfile::tempdir().expect("temporary directory");
        let reviewer = StubReviewer {
            assignment: assignment("review-yield-test"),
            outcome: StubOutcome::ZeroFindings,
        };
        let resolved_assignment = reviewer.resolved_assignment().clone();
        // The recording decorator obtains every persisted identity axis from
        // ResolvedReviewer::resolved_assignment before writing the round.
        assert_eq!(resolved_assignment.track_id().as_ref(), "review-yield-test");
        assert_eq!(resolved_assignment.scope().to_string(), "infrastructure");
        assert_eq!(resolved_assignment.provider().as_str(), "codex");
        assert_eq!(resolved_assignment.model().as_str(), "gpt-5.4");
        assert_eq!(resolved_assignment.reasoning_effort(), ReasoningEffort::High);
        let recording = recording(&temp_dir, reviewer);

        recording.review(&ReviewTarget::new(vec![])).expect("review succeeds");

        let line = telemetry_line(&temp_dir);
        assert_eq!(line.get("event_type").expect("event type"), "ReviewRound");
        assert_eq!(line.get("track_id").expect("track id"), "review-yield-test");
        let round = line.get("round").expect("round");
        assert_eq!(round.get("scope").expect("round scope"), "infrastructure");
        assert_eq!(round.get("round_type").expect("round type"), "final");
        assert_eq!(round.get("provider").expect("round provider"), "codex");
        assert_eq!(round.get("model").expect("round model"), "gpt-5.4");
        assert_eq!(round.get("reasoning_effort").expect("round reasoning effort"), "high");
        assert_eq!(round.get("findings_count").expect("round findings count"), 0);
    }

    #[test]
    fn test_review_yield_recording_reviewer_derives_fast_round_and_count_from_verdict() {
        let temp_dir = tempfile::tempdir().expect("temporary directory");
        let reviewer = StubReviewer {
            assignment: assignment("review-yield-fast"),
            outcome: StubOutcome::Findings,
        };
        let resolved_assignment = reviewer.resolved_assignment().clone();
        let recording = recording(&temp_dir, reviewer);

        recording.fast_review(&ReviewTarget::new(vec![])).expect("review succeeds");

        let line = telemetry_line(&temp_dir);
        // These direct decorator calls are the included structured-review path;
        // excluded provider-specific entry points are checked in composition tests.
        // Fast rounds use the same resolved assignment snapshot as final rounds.
        assert_eq!(resolved_assignment.scope().to_string(), "infrastructure");
        assert_eq!(resolved_assignment.provider().as_str(), "codex");
        assert_eq!(resolved_assignment.model().as_str(), "gpt-5.4");
        assert_eq!(resolved_assignment.reasoning_effort(), ReasoningEffort::High);
        let round = line.get("round").expect("round");
        assert_eq!(round.get("scope").expect("round scope"), "infrastructure");
        assert_eq!(round.get("round_type").expect("round type"), "fast");
        assert_eq!(round.get("provider").expect("round provider"), "codex");
        assert_eq!(round.get("model").expect("round model"), "gpt-5.4");
        assert_eq!(round.get("reasoning_effort").expect("round reasoning effort"), "high");
        assert_eq!(round.get("findings_count").expect("round findings count"), 1);
    }

    #[test]
    fn test_review_yield_recording_reviewer_measures_from_first_invocation() {
        let temp_dir = tempfile::tempdir().expect("temporary directory");
        let reviewer = StubReviewer {
            assignment: assignment("review-yield-duration"),
            outcome: StubOutcome::ZeroFindings,
        };
        let recording = recording(&temp_dir, reviewer);

        recording.review(&ReviewTarget::new(vec![])).expect("review succeeds");
        std::thread::sleep(Duration::from_millis(20));
        recording.fast_review(&ReviewTarget::new(vec![])).expect("fast review succeeds");

        let content = std::fs::read_to_string(temp_dir.path().join("telemetry.jsonl"))
            .expect("telemetry lines written");
        let lines: Vec<serde_json::Value> = content
            .lines()
            .map(|line| serde_json::from_str(line).expect("valid telemetry JSON"))
            .collect();
        assert_eq!(lines.len(), 2);
        let first_duration = lines
            .first()
            .and_then(|line| line.get("duration_ms"))
            .and_then(serde_json::Value::as_u64)
            .expect("first duration");
        let second_duration = lines
            .get(1)
            .and_then(|line| line.get("duration_ms"))
            .and_then(serde_json::Value::as_u64)
            .expect("second duration");
        assert!(
            second_duration >= first_duration.saturating_add(10),
            "second duration must retain the first invocation start: first={first_duration}, second={second_duration}"
        );
    }

    #[test]
    fn test_review_yield_recording_reviewer_does_not_record_failed_verdict() {
        let temp_dir = tempfile::tempdir().expect("temporary directory");
        let reviewer = StubReviewer {
            assignment: assignment("review-yield-timeout"),
            outcome: StubOutcome::Timeout,
        };
        let recording = recording(&temp_dir, reviewer);

        assert!(matches!(
            recording.review(&ReviewTarget::new(vec![])),
            Err(ReviewerError::Timeout)
        ));
        assert!(!temp_dir.path().join("telemetry.jsonl").exists());
    }

    #[test]
    fn test_review_yield_recording_reviewer_ignores_telemetry_write_failure() {
        let temp_dir = tempfile::tempdir().expect("temporary directory");
        let blocker = temp_dir.path().join("not-a-directory");
        std::fs::write(&blocker, b"blocker").expect("blocker file");
        let reviewer = StubReviewer {
            assignment: assignment("review-yield-write-failure"),
            outcome: StubOutcome::ZeroFindings,
        };
        let recording = temp_env::with_vars(
            [
                ("SOTP_TELEMETRY", Some("1")),
                ("SOTP_TELEMETRY_DIR", Some(blocker.to_str().expect("utf-8 path"))),
            ],
            || {
                ReviewYieldRecordingReviewer::new(
                    reviewer,
                    TelemetryConfig::from_env(),
                    temp_dir.path().to_path_buf(),
                )
            },
        );

        recording
            .review(&ReviewTarget::new(vec![]))
            .expect("telemetry failure must not fail review");
    }

    fn recording(
        temp_dir: &TempDir,
        reviewer: StubReviewer,
    ) -> ReviewYieldRecordingReviewer<StubReviewer> {
        let output_dir = temp_dir.path().to_str().expect("utf-8 path").to_owned();
        temp_env::with_vars(
            [("SOTP_TELEMETRY", Some("1")), ("SOTP_TELEMETRY_DIR", Some(output_dir.as_str()))],
            || {
                ReviewYieldRecordingReviewer::new(
                    reviewer,
                    TelemetryConfig::from_env(),
                    temp_dir.path().to_path_buf(),
                )
            },
        )
    }
}
