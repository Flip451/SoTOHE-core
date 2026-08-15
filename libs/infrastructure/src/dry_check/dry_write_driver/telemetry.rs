//! Per-tier `ReviewRound`/`ExternalSubprocess` telemetry helpers for `sotp
//! dry write`, backing [`super::RecordingDryTierTelemetryAdapter`].
//!
//! Relocated from `apps/cli-composition/src/dry/tier_telemetry.rs`'s
//! `dry_tiered_telemetry_for_result` / `emit_dry_tier_review_round` /
//! `emit_dry_tier_external_subprocess` (T028) — reimplemented directly
//! against this crate's own [`crate::telemetry`] types (no dependency on
//! `apps/cli-composition`).
//!
//! [`super::RecordingDryTierTelemetryAdapter`] itself (the catalogued
//! `SecondaryAdapter`) is defined directly in the parent `dry_write_driver`
//! module, matching its catalogued `module_path`; only its private helper
//! functions live here.

use std::path::Path;
use std::time::Instant;

use domain::dry_check::DryCheckFinding;
use usecase::dry_check::{DryCheckAgentError, DryCheckCycleError};

use crate::dry_check::recording_agent::TieredDryAgentRecorder;
use crate::telemetry::{TelemetryConfig, TelemetryEvent, TelemetryWriter};

// ── resolve_dry_write_telemetry_writer ────────────────────────────────────────

/// Resolve a branch/kill-switch-gated [`TelemetryWriter`] for a `dry write` run.
///
/// Reproduces the `track/<track_id>` + `SOTP_TELEMETRY=0` gating currently in
/// `apps/cli-composition/src/telemetry_wiring.rs::resolve_telemetry_writer`
/// (composed with `dry.rs`'s `resolve_dry_write_telemetry_writer` explicit
/// `track_id` match filter), reimplemented locally since `infrastructure`
/// cannot depend on `apps/cli-composition`.
///
/// Returns `None` when: the current branch (discovered from `repo_root`)
/// cannot be resolved, the resolved branch does not decode to a track id, the
/// resolved track id does not match `track_id`, or `SOTP_TELEMETRY=0`.
pub(super) fn resolve_dry_write_telemetry_writer(
    repo_root: &Path,
    items_dir: &Path,
    track_id: &str,
) -> Option<TelemetryWriter> {
    use crate::git_cli::SystemGitRepo;

    let repo = SystemGitRepo::discover_from(repo_root).ok()?;
    let branch = repo.current_branch().ok().flatten()?;
    let branch_track_id =
        usecase::track_resolution::resolve_track_id_from_branch(Some(&branch)).ok()?;
    if branch_track_id != track_id {
        return None;
    }

    let config = TelemetryConfig::from_env();
    if !config.is_enabled() {
        return None;
    }

    let track_id = domain::TrackId::try_new(track_id).ok()?;
    Some(TelemetryWriter::new(config, track_id, items_dir.to_path_buf()))
}

// ── DryRoundTelemetry ─────────────────────────────────────────────────────────

pub(super) struct DryRoundTelemetry {
    pub(super) findings_count: u32,
    pub(super) verdict_parse_failed: bool,
    pub(super) subprocess_started_at: Option<Instant>,
    /// Pre-computed duration in milliseconds for this tier.
    ///
    /// `Some(ms)` is used when the tier's end time can be determined precisely
    /// (e.g. fast tier on escalated runs: duration = final_started_at -
    /// fast_started_at). `None` means the caller should use
    /// `round_started_at.elapsed()` at emission time.
    pub(super) duration_ms: Option<u64>,
    /// The `Instant` to use for elapsed duration when `duration_ms` is `None`.
    pub(super) round_started_at: Option<Instant>,
}

// ── dry_tiered_telemetry_for_result ──────────────────────────────────────────

/// Build per-tier telemetry from the interactor result and the tier-split
/// recorder. Returns `(fast_telemetry, final_telemetry)`.
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
        Err(DryCheckCycleError::Entry(_)) | Err(DryCheckCycleError::Writer(_)) => {
            let fast = tiered.fast.started_at().map(|started_at| {
                build_fast(started_at, final_started_at, tiered.fast.findings_count(), false)
            });
            let final_ = final_started_at
                .map(|started_at| build_final(started_at, tiered.final_.findings_count(), false));
            (fast, final_)
        }
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

/// Returns `true` when the agent error is attributable to a subprocess failure
/// (the external process was spawned, ran, and returned an error or bad output).
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

/// Returns an ISO-8601 UTC timestamp string for the current moment.
fn now_timestamp() -> String {
    chrono::Utc::now().to_rfc3339()
}

/// Keep the dry-check round hook as a no-op.
///
/// Dry-check rounds are excluded from structured review-yield telemetry. The
/// caller still emits the existing `ExternalSubprocess` diagnostic event for
/// the tier below, so this preserves the dry-check telemetry path without
/// constructing a structured local-review record.
pub(super) fn emit_dry_tier_review_round(
    _writer: &TelemetryWriter,
    _track_id: &str,
    _provider: &str,
    _model: &str,
    _round_type: &str,
    _telemetry: &DryRoundTelemetry,
    _fallback_start: Instant,
) {
    let _ = (_telemetry.findings_count, _telemetry.round_started_at);
}

/// Emit an `ExternalSubprocess` telemetry event for a dry-check tier.
pub(super) fn emit_dry_tier_external_subprocess(
    writer: &TelemetryWriter,
    track_id: &str,
    command: &str,
    telemetry: &DryRoundTelemetry,
    fallback_start: Instant,
) {
    let duration_ms =
        dry_duration_ms(telemetry.duration_ms, telemetry.subprocess_started_at, fallback_start);
    let event = TelemetryEvent::ExternalSubprocess {
        schema_version: 1,
        track_id: track_id.to_string(),
        command: command.to_string(),
        duration_ms,
        retry_count: 0,
        verdict_parse_failed: telemetry.verdict_parse_failed,
        timestamp: now_timestamp(),
    };
    // Fire-and-forget: suppress errors per CN-01.
    let _ = writer.write(event);
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use crate::dry_check::recording_agent::DryAgentRunRecorder;
    use usecase::dry_write_driver::DryTierTelemetryPort;

    use super::*;

    fn make_tiered_recorder_fast_only(findings: u32) -> TieredDryAgentRecorder {
        let fast = DryAgentRunRecorder::new();
        fast.record_started();
        for _ in 0..findings {
            fast.record_violation();
        }
        fast.record_completed();
        TieredDryAgentRecorder { fast, final_: DryAgentRunRecorder::new() }
    }

    #[test]
    fn test_dry_agent_unexpected_after_spawn_classifies_child_poll_failure() {
        let error =
            DryCheckAgentError::Unexpected("failed to poll dry-check agent child: io".to_owned());
        assert!(dry_agent_error_is_subprocess_failure(&error));
    }

    #[test]
    fn test_dry_agent_unexpected_before_spawn_is_not_subprocess_failure() {
        let error =
            DryCheckAgentError::Unexpected("failed to write output-schema: disk full".to_owned());
        assert!(!dry_agent_error_is_subprocess_failure(&error));
    }

    #[test]
    fn test_dry_tiered_telemetry_for_result_index_after_fast_agent_success_emits_fast() {
        let tiered = make_tiered_recorder_fast_only(2);
        let result: Result<Vec<DryCheckFinding>, DryCheckCycleError> = Err(
            DryCheckCycleError::Index(usecase::semantic_dup::SemanticIndexError::SearchFailed {
                source: "changed_path error: invalid path".to_owned(),
            }),
        );

        let (fast, final_) = dry_tiered_telemetry_for_result(&result, &tiered);
        let fast_t = fast.unwrap();
        assert_eq!(fast_t.findings_count, 2);
        assert!(!fast_t.verdict_parse_failed);
        assert!(fast_t.subprocess_started_at.is_some());
        assert!(final_.is_none());
    }

    #[test]
    fn test_dry_tiered_telemetry_for_result_index_before_agent_returns_none() {
        let tiered = TieredDryAgentRecorder {
            fast: DryAgentRunRecorder::new(),
            final_: DryAgentRunRecorder::new(),
        };
        let result: Result<Vec<DryCheckFinding>, DryCheckCycleError> = Err(
            DryCheckCycleError::Index(usecase::semantic_dup::SemanticIndexError::SearchFailed {
                source: "candidate index failed".to_owned(),
            }),
        );

        let (fast, final_) = dry_tiered_telemetry_for_result(&result, &tiered);
        assert!(fast.is_none());
        assert!(final_.is_none());
    }

    #[test]
    fn test_dry_tiered_telemetry_for_result_success_without_agent_start_skips_subprocess() {
        let tiered = TieredDryAgentRecorder {
            fast: DryAgentRunRecorder::new(),
            final_: DryAgentRunRecorder::new(),
        };
        let result: Result<Vec<DryCheckFinding>, DryCheckCycleError> = Ok(vec![]);

        let (fast, final_) = dry_tiered_telemetry_for_result(&result, &tiered);
        assert!(fast.is_none());
        assert!(final_.is_none());
    }

    #[test]
    fn test_dry_tiered_telemetry_for_result_writer_error_uses_fast_findings_count() {
        let tiered = make_tiered_recorder_fast_only(3);
        let result: Result<Vec<DryCheckFinding>, DryCheckCycleError> =
            Err(DryCheckCycleError::Writer(domain::dry_check::DryCheckWriterError::Codec {
                detail: "serialize failed".to_owned(),
            }));

        let (fast, final_) = dry_tiered_telemetry_for_result(&result, &tiered);
        let fast_t = fast.unwrap();
        assert_eq!(fast_t.findings_count, 3);
        assert!(final_.is_none());
    }

    #[test]
    fn test_recording_dry_tier_telemetry_emits_external_subprocess_without_review_round() {
        let output_dir = tempfile::tempdir().unwrap();
        let output_dir_string = output_dir.path().to_string_lossy().into_owned();
        let config = {
            let mut config = None;
            temp_env::with_vars(
                [
                    ("SOTP_TELEMETRY", Some("1")),
                    ("SOTP_TELEMETRY_DIR", Some(output_dir_string.as_str())),
                ],
                || {
                    config = Some(TelemetryConfig::from_env());
                },
            );
            config.unwrap()
        };
        let writer = TelemetryWriter::new(
            config,
            domain::TrackId::try_new("test-track").unwrap(),
            output_dir.path().to_path_buf(),
        );
        let adapter = super::super::RecordingDryTierTelemetryAdapter::new(
            make_tiered_recorder_fast_only(1),
            "fast-model".to_owned(),
            "final-model".to_owned(),
            "test-track".to_owned(),
            Some(writer),
        );
        let result: Result<Vec<DryCheckFinding>, DryCheckCycleError> = Ok(Vec::new());

        adapter.record(&result);

        let content = std::fs::read_to_string(output_dir.path().join("telemetry.jsonl")).unwrap();
        let events: Vec<serde_json::Value> =
            content.lines().map(serde_json::from_str).collect::<Result<_, _>>().unwrap();
        assert_eq!(
            events
                .iter()
                .filter(|event| event.get("event_type")
                    == Some(&serde_json::json!("ExternalSubprocess")))
                .count(),
            1,
            "a real dry tier must retain ExternalSubprocess telemetry"
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| event.get("event_type") == Some(&serde_json::json!("ReviewRound")))
                .count(),
            0,
            "dry tiers must not emit structured ReviewRound telemetry"
        );
    }
}
