//! Review cycle execution helpers (Codex and Claude).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use domain::TrackId;
use domain::review_v2::{
    FastVerdict, LogInfo, MainScopeName, ReviewOutcome, ReviewTarget, ReviewWriter, ScopeName,
    Verdict,
};
use infrastructure::review_v2::{
    ClaudeReviewer, CodexReviewer, FsReviewStore, GitDiffGetter, SystemReviewHasher,
};
use usecase::review_v2::error::{ReviewCycleError, ReviewerError};
use usecase::review_v2::{DiffGetter, ReviewCycle, ReviewHasher, Reviewer};

use super::shared::{CodexReviewOutcome, build_v2_shared, repo_root_from_items_dir, with_repo_cwd};

// ---------------------------------------------------------------------------
// Shared verdict rendering
// ---------------------------------------------------------------------------

/// Builds a `ReviewFinalPayload`, serialises it, and returns `(json, exit_code)`.
///
/// `zero_findings` — when `true` the payload carries no findings and exit code 0.
/// `findings`      — slice of findings used when `zero_findings` is `false`; ignored
///                   when `zero_findings` is `true`.
fn render_verdict_payload(
    zero_findings: bool,
    findings_slice: &[domain::review_v2::ReviewerFinding],
) -> Result<(String, u8), String> {
    use usecase::review_workflow::{
        ReviewFinalPayload, ReviewPayloadVerdict, render_review_payload,
    };
    let (payload, exit_code) = if zero_findings {
        (ReviewFinalPayload { verdict: ReviewPayloadVerdict::ZeroFindings, findings: vec![] }, 0u8)
    } else {
        let findings = findings_slice.iter().map(finding_to_payload).collect();
        (ReviewFinalPayload { verdict: ReviewPayloadVerdict::FindingsRemain, findings }, 2u8)
    };
    let json = render_review_payload(&payload).map_err(|e| format!("[ERROR] {e}"))?;
    Ok((json, exit_code))
}

fn finding_to_payload(
    f: &domain::review_v2::ReviewerFinding,
) -> usecase::review_workflow::ReviewFinding {
    usecase::review_workflow::ReviewFinding {
        message: f.message().to_owned(),
        severity: f.severity().map(str::to_owned),
        file: f.file().map(str::to_owned),
        line: f.line(),
        category: f.category().map(str::to_owned),
    }
}

// ---------------------------------------------------------------------------
// Shared review dispatch (generic over reviewer type)
// ---------------------------------------------------------------------------

/// Parses inputs and runs one review round using a pre-built `ReviewCycle` and
/// `FsReviewStore`. Shared by [`run_codex_review_str`] and [`run_claude_review_str`].
///
/// # Errors
/// Returns a human-readable error string on failure at any step.
fn dispatch_review_cycle<R, H, D>(
    group_str: &str,
    round_type_str: &str,
    cycle: ReviewCycle<R, H, D>,
    review_store: FsReviewStore,
    findings_recorder: ReviewFindingsRecorder,
) -> Result<CodexReviewOutcome, String>
where
    R: Reviewer,
    H: ReviewHasher,
    D: DiffGetter,
{
    let scope = if group_str == "other" {
        ScopeName::Other
    } else {
        match MainScopeName::new(group_str) {
            Ok(main) => ScopeName::Main(main),
            Err(e) => return Err(format!("[ERROR] invalid scope name: {e}")),
        }
    };

    match round_type_str {
        "final" => match cycle.review(&scope) {
            Ok(ReviewOutcome::Skipped) => {
                Ok(CodexReviewOutcome::Skipped { scope_label: group_str.to_owned() })
            }
            Ok(ReviewOutcome::Reviewed { verdict, hash, .. }) => {
                // Compute findings_count before write_verdict so we can carry it into
                // SubprocessFailed when persistence fails (the subprocess produced a valid
                // verdict — underreporting as 0 would be wrong).
                let findings_count = findings_count_final(&verdict);
                // write_verdict runs after the subprocess. A record-write failure
                // must not suppress telemetry, so convert it to SubprocessFailed
                // (verdict_parse_failed=false — the subprocess produced a valid verdict).
                if let Err(e) = review_store.write_verdict(&scope, &verdict, &hash) {
                    return Ok(CodexReviewOutcome::SubprocessFailed {
                        error: format!("[ERROR] record failed: {e}"),
                        round_type: "final".to_owned(),
                        verdict_parse_failed: false,
                        findings_count,
                        subprocess_started_at: findings_recorder.subprocess_started_at_or_now(),
                    });
                }
                let rendered = match &verdict {
                    Verdict::ZeroFindings => render_verdict_payload(true, &[]),
                    Verdict::FindingsRemain(nef) => render_verdict_payload(false, nef.as_slice()),
                };
                let (json, exit_code) = match rendered {
                    Ok(payload) => payload,
                    Err(e) => {
                        // Rendering happens after the reviewer returned a valid verdict, so
                        // preserve telemetry as a subprocess-involved failure.
                        return Ok(CodexReviewOutcome::SubprocessFailed {
                            error: e,
                            round_type: "final".to_owned(),
                            verdict_parse_failed: false,
                            findings_count,
                            subprocess_started_at: findings_recorder.subprocess_started_at_or_now(),
                        });
                    }
                };
                Ok(CodexReviewOutcome::FinalCompleted {
                    verdict_json: json,
                    exit_code,
                    findings_count,
                    subprocess_started_at: findings_recorder.subprocess_started_at_or_now(),
                })
            }
            // Map only subprocess-involved reviewer failures to SubprocessFailed.
            // Unexpected(_) is overloaded by the adapters: some messages happen
            // before spawn, while the prefixes recognized by
            // reviewer_error_is_subprocess_failure happen after the child exists or
            // after it has produced a verdict-like payload.
            // verdict_parse_failed=true only for IllegalVerdict (stdout unparseable).
            // Pre-subprocess errors (UnknownScope, Diff, Hash) propagate as Err.
            Err(ReviewCycleError::Reviewer(inner)) => {
                if reviewer_error_is_subprocess_failure(&inner) {
                    let verdict_parse_failed = matches!(inner, ReviewerError::IllegalVerdict);
                    Ok(CodexReviewOutcome::SubprocessFailed {
                        error: format!("[ERROR] reviewer error: {inner}"),
                        round_type: "final".to_owned(),
                        verdict_parse_failed,
                        findings_count: 0,
                        subprocess_started_at: findings_recorder.subprocess_started_at_or_now(),
                    })
                } else {
                    Err(format!("[ERROR] reviewer error: {inner}"))
                }
            }
            Err(e @ ReviewCycleError::FileChangedDuringReview) => {
                Ok(CodexReviewOutcome::SubprocessFailed {
                    error: format!("[ERROR] {e}"),
                    round_type: "final".to_owned(),
                    verdict_parse_failed: false,
                    findings_count: findings_recorder.recorded_count().unwrap_or(0),
                    subprocess_started_at: findings_recorder.subprocess_started_at_or_now(),
                })
            }
            Err(e @ ReviewCycleError::Reader(_)) => Ok(CodexReviewOutcome::SubprocessFailed {
                error: format!("[ERROR] {e}"),
                round_type: "final".to_owned(),
                verdict_parse_failed: false,
                findings_count: 0,
                subprocess_started_at: findings_recorder.subprocess_started_at_or_now(),
            }),
            Err(e @ (ReviewCycleError::Diff(_) | ReviewCycleError::Hash(_))) => {
                match findings_recorder.recorded_count() {
                    Some(findings_count) => Ok(CodexReviewOutcome::SubprocessFailed {
                        error: format!("[ERROR] {e}"),
                        round_type: "final".to_owned(),
                        verdict_parse_failed: false,
                        findings_count,
                        subprocess_started_at: findings_recorder.subprocess_started_at_or_now(),
                    }),
                    None => Err(format!("[ERROR] {e}")),
                }
            }
            Err(e) => Err(format!("[ERROR] {e}")),
        },
        "fast" => match cycle.fast_review(&scope) {
            Ok(ReviewOutcome::Skipped) => {
                Ok(CodexReviewOutcome::Skipped { scope_label: group_str.to_owned() })
            }
            Ok(ReviewOutcome::Reviewed { verdict, hash, .. }) => {
                // Compute findings_count before write_fast_verdict (mirrors "final" branch).
                let findings_count = findings_count_fast(&verdict);
                // write_fast_verdict runs after the subprocess. Convert failure to
                // SubprocessFailed to ensure telemetry is emitted (verdict_parse_failed=false).
                if let Err(e) = review_store.write_fast_verdict(&scope, &verdict, &hash) {
                    return Ok(CodexReviewOutcome::SubprocessFailed {
                        error: format!("[ERROR] record failed: {e}"),
                        round_type: "fast".to_owned(),
                        verdict_parse_failed: false,
                        findings_count,
                        subprocess_started_at: findings_recorder.subprocess_started_at_or_now(),
                    });
                }
                let rendered = match &verdict {
                    FastVerdict::ZeroFindings => render_verdict_payload(true, &[]),
                    FastVerdict::FindingsRemain(nef) => {
                        render_verdict_payload(false, nef.as_slice())
                    }
                };
                let (json, exit_code) = match rendered {
                    Ok(payload) => payload,
                    Err(e) => {
                        // Rendering happens after the reviewer returned a valid verdict, so
                        // preserve telemetry as a subprocess-involved failure.
                        return Ok(CodexReviewOutcome::SubprocessFailed {
                            error: e,
                            round_type: "fast".to_owned(),
                            verdict_parse_failed: false,
                            findings_count,
                            subprocess_started_at: findings_recorder.subprocess_started_at_or_now(),
                        });
                    }
                };
                Ok(CodexReviewOutcome::FastCompleted {
                    verdict_json: json,
                    exit_code,
                    findings_count,
                    subprocess_started_at: findings_recorder.subprocess_started_at_or_now(),
                })
            }
            // Subprocess-launched errors (mirror of "final" branch above).
            Err(ReviewCycleError::Reviewer(inner)) => {
                if reviewer_error_is_subprocess_failure(&inner) {
                    let verdict_parse_failed = matches!(inner, ReviewerError::IllegalVerdict);
                    Ok(CodexReviewOutcome::SubprocessFailed {
                        error: format!("[ERROR] reviewer error: {inner}"),
                        round_type: "fast".to_owned(),
                        verdict_parse_failed,
                        findings_count: 0,
                        subprocess_started_at: findings_recorder.subprocess_started_at_or_now(),
                    })
                } else {
                    Err(format!("[ERROR] reviewer error: {inner}"))
                }
            }
            Err(e @ ReviewCycleError::FileChangedDuringReview) => {
                Ok(CodexReviewOutcome::SubprocessFailed {
                    error: format!("[ERROR] {e}"),
                    round_type: "fast".to_owned(),
                    verdict_parse_failed: false,
                    findings_count: findings_recorder.recorded_count().unwrap_or(0),
                    subprocess_started_at: findings_recorder.subprocess_started_at_or_now(),
                })
            }
            Err(e @ ReviewCycleError::Reader(_)) => Ok(CodexReviewOutcome::SubprocessFailed {
                error: format!("[ERROR] {e}"),
                round_type: "fast".to_owned(),
                verdict_parse_failed: false,
                findings_count: 0,
                subprocess_started_at: findings_recorder.subprocess_started_at_or_now(),
            }),
            Err(e @ (ReviewCycleError::Diff(_) | ReviewCycleError::Hash(_))) => {
                match findings_recorder.recorded_count() {
                    Some(findings_count) => Ok(CodexReviewOutcome::SubprocessFailed {
                        error: format!("[ERROR] {e}"),
                        round_type: "fast".to_owned(),
                        verdict_parse_failed: false,
                        findings_count,
                        subprocess_started_at: findings_recorder.subprocess_started_at_or_now(),
                    }),
                    None => Err(format!("[ERROR] {e}")),
                }
            }
            Err(e) => Err(format!("[ERROR] {e}")),
        },
        other => Err(format!("[ERROR] unknown round type: '{other}' (expected 'fast' or 'final')")),
    }
}

// ---------------------------------------------------------------------------
// Public entry points — thin wrappers over run_review_str_inner
// ---------------------------------------------------------------------------

type ReviewDispatchParts<R, H, D> =
    (ReviewCycle<R, H, D>, FsReviewStore, ReviewFindingsRecorder, PathBuf);

/// Runs the full Codex review cycle from string inputs.
///
/// Encapsulates `TrackId`, `ScopeName`, `RoundType`, `ReviewOutcome`, `Verdict`,
/// `FastVerdict`, `ReviewWriter`, and `ReviewerFinding` conversions so the CLI
/// layer never imports these domain types directly (CN-01 / AC-03).
///
/// # Errors
/// Returns a human-readable error string on failure at any step.
pub(crate) fn run_codex_review_str(
    track_id_str: &str,
    items_dir: &Path,
    group_str: &str,
    round_type_str: &str, // "fast" | "final"
    reviewer: CodexReviewer,
) -> Result<CodexReviewOutcome, String> {
    run_review_str_with_reviewer(track_id_str, items_dir, group_str, round_type_str, reviewer)
}

/// Runs the full Claude review cycle from string inputs.
///
/// Mirrors [`run_codex_review_str`] with `ClaudeReviewer` in place of `CodexReviewer`.
///
/// # Errors
/// Returns a human-readable error string on failure at any step.
pub(crate) fn run_claude_review_str(
    track_id_str: &str,
    items_dir: &Path,
    group_str: &str,
    round_type_str: &str, // "fast" | "final"
    reviewer: ClaudeReviewer,
) -> Result<CodexReviewOutcome, String> {
    run_review_str_with_reviewer(track_id_str, items_dir, group_str, round_type_str, reviewer)
}

fn run_review_str_with_reviewer<R>(
    track_id_str: &str,
    items_dir: &Path,
    group_str: &str,
    round_type_str: &str,
    reviewer: R,
) -> Result<CodexReviewOutcome, String>
where
    R: Reviewer,
{
    run_review_str_inner(track_id_str, items_dir, group_str, round_type_str, |tid| {
        build_review_dispatch_parts(tid, items_dir, reviewer)
    })
}

fn build_review_dispatch_parts<R>(
    track_id: &TrackId,
    items_dir: &Path,
    reviewer: R,
) -> Result<ReviewDispatchParts<FindingsCountReviewer<R>, SystemReviewHasher, GitDiffGetter>, String>
where
    R: Reviewer,
{
    let (scope_config, review_store, _commit_hash_store, base) =
        build_v2_shared(track_id, items_dir)?;
    let repo_root = repo_root_from_items_dir(items_dir)?;
    let (reviewer, findings_recorder) = FindingsCountReviewer::new(reviewer);
    let cycle = ReviewCycle::new(base, scope_config, reviewer, GitDiffGetter, SystemReviewHasher);
    Ok((cycle, review_store, findings_recorder, repo_root))
}

/// Shared implementation: parse `track_id`, invoke `builder` to obtain the
/// review adapters plus repo root, then dispatch the review round from that root.
///
/// The `builder` closure accepts a `&TrackId` and returns the concrete
/// `ReviewCycle` pieces. This lets the two public entry points differ only in
/// which reviewer adapter they pass.
///
/// # Errors
/// Returns a human-readable error string on failure at any step.
fn run_review_str_inner<R, H, D, B>(
    track_id_str: &str,
    _items_dir: &Path,
    group_str: &str,
    round_type_str: &str,
    builder: B,
) -> Result<CodexReviewOutcome, String>
where
    R: Reviewer,
    H: ReviewHasher,
    D: DiffGetter,
    B: FnOnce(&TrackId) -> Result<ReviewDispatchParts<R, H, D>, String>,
{
    let track_id =
        TrackId::try_new(track_id_str).map_err(|e| format!("[ERROR] invalid track id: {e}"))?;
    let (cycle, review_store, findings_recorder, repo_root) =
        builder(&track_id).map_err(|e| format!("[ERROR] v2 composition failed: {e}"))?;
    with_repo_cwd(&repo_root, || {
        dispatch_review_cycle(group_str, round_type_str, cycle, review_store, findings_recorder)
    })
    .map_err(|e| if e.starts_with("[ERROR]") { e } else { format!("[ERROR] {e}") })
}

// ---------------------------------------------------------------------------
// Reviewer telemetry capture
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct ReviewFindingsRecorder {
    count: Arc<AtomicU64>,
    subprocess_started_at: Arc<Mutex<Option<Instant>>>,
}

impl ReviewFindingsRecorder {
    const UNSET: u64 = u64::MAX;

    fn new() -> Self {
        Self {
            count: Arc::new(AtomicU64::new(Self::UNSET)),
            subprocess_started_at: Arc::new(Mutex::new(None)),
        }
    }

    fn record_subprocess_started(&self) {
        super::record_instant_once(&self.subprocess_started_at);
    }

    fn record(&self, count: u32) {
        self.count.store(u64::from(count), Ordering::Relaxed);
    }

    fn recorded_count(&self) -> Option<u32> {
        match self.count.load(Ordering::Relaxed) {
            Self::UNSET => None,
            value => u32::try_from(value).ok(),
        }
    }

    fn subprocess_started_at(&self) -> Option<Instant> {
        self.subprocess_started_at.lock().ok().and_then(|started_at| *started_at)
    }

    fn subprocess_started_at_or_now(&self) -> Instant {
        self.subprocess_started_at().unwrap_or_else(Instant::now)
    }
}

struct FindingsCountReviewer<R> {
    inner: R,
    recorder: ReviewFindingsRecorder,
}

impl<R> FindingsCountReviewer<R> {
    fn new(inner: R) -> (Self, ReviewFindingsRecorder) {
        let recorder = ReviewFindingsRecorder::new();
        (Self { inner, recorder: recorder.clone() }, recorder)
    }
}

impl<R: Reviewer> Reviewer for FindingsCountReviewer<R> {
    fn review(&self, target: &ReviewTarget) -> Result<(Verdict, LogInfo), ReviewerError> {
        self.recorder.record_subprocess_started();
        let result = self.inner.review(target);
        if let Ok((verdict, _log_info)) = &result {
            self.recorder.record(findings_count_final(verdict));
        }
        result
    }

    fn fast_review(&self, target: &ReviewTarget) -> Result<(FastVerdict, LogInfo), ReviewerError> {
        self.recorder.record_subprocess_started();
        let result = self.inner.fast_review(target);
        if let Ok((verdict, _log_info)) = &result {
            self.recorder.record(findings_count_fast(verdict));
        }
        result
    }
}

// ---------------------------------------------------------------------------
// Findings-count helpers
// ---------------------------------------------------------------------------

/// Returns the number of findings from a final `Verdict`.
///
/// Used to populate the `findings_count` field of `TelemetryEvent::ReviewRound`.
fn findings_count_final(verdict: &domain::review_v2::Verdict) -> u32 {
    use domain::review_v2::Verdict;
    match verdict {
        Verdict::ZeroFindings => 0,
        Verdict::FindingsRemain(nef) => nef.as_slice().len().try_into().unwrap_or(u32::MAX),
    }
}

/// Returns the number of findings from a fast `FastVerdict`.
///
/// Used to populate the `findings_count` field of `TelemetryEvent::ReviewRound`.
fn findings_count_fast(verdict: &domain::review_v2::FastVerdict) -> u32 {
    use domain::review_v2::FastVerdict;
    match verdict {
        FastVerdict::ZeroFindings => 0,
        FastVerdict::FindingsRemain(nef) => nef.as_slice().len().try_into().unwrap_or(u32::MAX),
    }
}

fn reviewer_error_is_subprocess_failure(error: &ReviewerError) -> bool {
    match error {
        ReviewerError::UserAbort
        | ReviewerError::ReviewerAbort
        | ReviewerError::Timeout
        | ReviewerError::IllegalVerdict => true,
        ReviewerError::Unexpected(message) => reviewer_unexpected_after_spawn(message),
    }
}

fn reviewer_unexpected_after_spawn(message: &str) -> bool {
    message.starts_with("failed to poll reviewer child:")
        || message.starts_with("failed to reap reviewer child:")
        || message.starts_with("failed to read output-last-message ")
        || message.starts_with("verdict construction:")
        || message.starts_with("failed to serialize reviewer final payload:")
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use std::process::Command;
    use std::time::Duration;

    use infrastructure::review_v2::ClaudeReviewer;

    use super::*;
    use crate::review_v2::process_guards::CwdGuard;
    use crate::review_v2::shared::{
        CodexReviewOutcome, build_review_v2_with_claude_reviewer,
        build_review_v2_with_claude_reviewer_str,
    };

    // Mutex so tests that mutate cwd do not race.
    static ENV_LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    fn env_lock() -> &'static std::sync::Mutex<()> {
        ENV_LOCK.get_or_init(|| std::sync::Mutex::new(()))
    }

    /// Sets up a minimal git repo with v2 review-scope.json for testing.
    ///
    /// Creates two commits so that the diff base (first commit SHA) differs from HEAD.
    /// Returns the SHA of the first commit, which callers write to `.commit_hash` so
    /// that the diff is non-empty and review is not skipped.
    fn setup_test_git_repo(root: &std::path::Path) -> String {
        let run =
            |args: &[&str]| Command::new("git").args(args).current_dir(root).output().unwrap();
        run(&["init", "-b", "main"]);
        run(&["config", "user.email", "test@test.com"]);
        run(&["config", "user.name", "Test"]);
        let track_dir = root.join("track");
        std::fs::create_dir_all(&track_dir).unwrap();
        // `infra` scope matches files under `src/`.
        std::fs::write(
            track_dir.join("review-scope.json"),
            r#"{"version": 2, "groups": {"infra": {"patterns": ["src/**"]}}}"#,
        )
        .unwrap();
        std::fs::create_dir_all(root.join("track/items")).unwrap();
        run(&["add", "."]);
        run(&["commit", "-m", "base commit"]);

        // Record the first commit SHA so callers can write it to `.commit_hash`.
        let sha_out =
            Command::new("git").args(["rev-parse", "HEAD"]).current_dir(root).output().unwrap();
        let base_sha = String::from_utf8_lossy(&sha_out.stdout).trim().to_owned();

        // Second commit: add `src/lib.rs` so the diff against base is non-empty.
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/lib.rs"), "// test\n").unwrap();
        run(&["add", "src/lib.rs"]);
        run(&["commit", "-m", "add src/lib.rs"]);

        base_sha
    }

    fn make_claude_reviewer() -> ClaudeReviewer {
        ClaudeReviewer::new("claude-opus-4-7", Duration::from_secs(10), "Review.")
    }

    #[test]
    fn test_run_claude_review_str_rejects_invalid_track_id() {
        let result = run_claude_review_str(
            "../evil",
            std::path::Path::new("track/items"),
            "infrastructure",
            "fast",
            make_claude_reviewer(),
        );
        assert!(result.is_err(), "invalid track id must be rejected");
        let msg = result.err().unwrap();
        assert!(msg.contains("[ERROR]"), "error message must contain [ERROR] prefix: {msg}");
    }

    #[test]
    fn test_run_claude_review_str_rejects_unknown_round_type() {
        // The "unknown round type" branch is reached only after the composition builds
        // successfully. Use a real git repo + track dir to exercise it.
        let _guard = env_lock().lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let base_sha = setup_test_git_repo(dir.path());
        let _cwd = CwdGuard::save_current();
        std::env::set_current_dir(dir.path()).unwrap();

        let items_dir = dir.path().join("track/items");
        let track_id = "my-test-track-2026";
        let track_dir = items_dir.join(track_id);
        std::fs::create_dir_all(&track_dir).unwrap();
        // Write diff base so the composition builds successfully.
        std::fs::write(track_dir.join(".commit_hash"), &base_sha).unwrap();

        let result = run_claude_review_str(
            track_id,
            &items_dir,
            "infra",
            "bogus-round",
            make_claude_reviewer(),
        );
        assert!(result.is_err(), "unknown round type must be rejected");
        let msg = result.err().unwrap();
        assert!(
            msg.contains("unknown round type"),
            "error must mention 'unknown round type': {msg}"
        );
    }

    /// Writes an executable shell script at `path` that outputs the given JSON envelope on stdout.
    #[cfg(unix)]
    fn write_fake_claude_script(path: &std::path::Path, envelope_json: &str) {
        use std::os::unix::fs::PermissionsExt;
        let content = format!(
            "#!/bin/sh\nprintf '%s\\n' '{}'\nexit 0\n",
            envelope_json.replace('\'', "'\\''")
        );
        std::fs::write(path, content).unwrap();
        let mut perms = std::fs::metadata(path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(path, perms).unwrap();
    }

    /// Builds a `ClaudeReviewer` that uses the given binary path instead of `claude`.
    #[cfg(unix)]
    fn make_reviewer_with_bin(bin: impl Into<std::ffi::OsString>) -> ClaudeReviewer {
        ClaudeReviewer::new("claude-opus-4-7", Duration::from_secs(10), "Review.").with_bin(bin)
    }

    #[cfg(unix)]
    fn run_claude_review_with_fake_script(
        script_name: &str,
        envelope_json: &str,
        track_id: &str,
        round_type: &str,
        expect_message: &str,
    ) -> CodexReviewOutcome {
        let _guard = env_lock().lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let base_sha = setup_test_git_repo(dir.path());
        let _cwd = CwdGuard::save_current();
        std::env::set_current_dir(dir.path()).unwrap();

        let script = dir.path().join(script_name);
        write_fake_claude_script(&script, envelope_json);

        let items_dir = dir.path().join("track/items");
        let track_dir = items_dir.join(track_id);
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(track_dir.join(".commit_hash"), &base_sha).unwrap();

        let reviewer = make_reviewer_with_bin(&script);
        let outcome = run_claude_review_str(track_id, &items_dir, "infra", round_type, reviewer)
            .expect(expect_message);

        assert!(
            track_dir.join("review.json").exists(),
            "review.json must be written (write-first contract)"
        );
        outcome
    }

    #[cfg(unix)]
    #[test]
    fn test_run_claude_review_str_accepts_absolute_items_dir_from_outside_repo() {
        let _guard = env_lock().lock().unwrap();
        let repo_dir = tempfile::tempdir().unwrap();
        let outside_dir = tempfile::tempdir().unwrap();
        let base_sha = setup_test_git_repo(repo_dir.path());
        let _cwd = CwdGuard::save_current();
        std::env::set_current_dir(outside_dir.path()).unwrap();

        let script = repo_dir.path().join("fake-claude-outside-repo.sh");
        write_fake_claude_script(
            &script,
            r#"{"type":"result","structured_output":{"verdict":"zero_findings","findings":[]}}"#,
        );

        let items_dir = repo_dir.path().join("track/items");
        let track_id = "my-test-track-outside-2026";
        let track_dir = items_dir.join(track_id);
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(track_dir.join(".commit_hash"), &base_sha).unwrap();

        let reviewer = make_reviewer_with_bin(&script);
        let result = run_claude_review_str(track_id, &items_dir, "infra", "fast", reviewer);

        let outcome = result.expect("absolute items_dir must anchor composition outside cwd");
        assert!(
            matches!(
                outcome,
                CodexReviewOutcome::FastCompleted { exit_code: 0, findings_count: 0, .. }
            ),
            "expected FastCompleted with zero findings"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_run_claude_review_str_fast_zero_findings_writes_verdict_and_returns_outcome() {
        // AC-03 / write-first / fail-closed: after a zero-findings verdict the verdict is
        // written to review.json before being returned, and a FastCompleted outcome is produced.
        let outcome = run_claude_review_with_fake_script(
            "fake-claude.sh",
            r#"{"type":"result","structured_output":{"verdict":"zero_findings","findings":[]}}"#,
            "my-test-track-2026",
            "fast",
            "fast zero-findings review must succeed",
        );

        assert!(
            matches!(
                outcome,
                CodexReviewOutcome::FastCompleted { exit_code: 0, findings_count: 0, .. }
            ),
            "expected FastCompleted with exit_code 0 and findings_count 0"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_run_claude_review_str_fast_findings_remain_writes_verdict_and_returns_outcome() {
        // AC-03: findings_remain case also writes review.json (write-first / fail-closed).
        let outcome = run_claude_review_with_fake_script(
            "fake-claude-findings.sh",
            r#"{"type":"result","structured_output":{"verdict":"findings_remain","findings":[{"message":"A finding","severity":"P2","file":"src/lib.rs","line":1,"category":"style"}]}}"#,
            "my-test-track-2026",
            "fast",
            "fast findings_remain review must succeed",
        );

        assert!(
            matches!(outcome, CodexReviewOutcome::FastCompleted { exit_code: 2, .. }),
            "expected FastCompleted with exit_code 2 (findings_count may be nonzero)"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_run_claude_review_str_final_zero_findings_writes_verdict_and_returns_outcome() {
        // AC-03 / final-round path: a zero-findings final verdict writes review.json
        // and returns FinalCompleted with exit_code 0.
        let outcome = run_claude_review_with_fake_script(
            "fake-claude-final.sh",
            r#"{"type":"result","structured_output":{"verdict":"zero_findings","findings":[]}}"#,
            "my-test-track-final-2026",
            "final",
            "final zero-findings review must succeed",
        );

        assert!(
            matches!(
                outcome,
                CodexReviewOutcome::FinalCompleted { exit_code: 0, findings_count: 0, .. }
            ),
            "expected FinalCompleted with exit_code 0 and findings_count 0"
        );
    }

    #[test]
    fn test_build_review_v2_with_claude_reviewer_str_rejects_invalid_track_id() {
        // build_review_v2_with_claude_reviewer_str validates track_id before any I/O.
        let result = build_review_v2_with_claude_reviewer_str(
            "../evil",
            std::path::Path::new("track/items"),
            make_claude_reviewer(),
        );
        assert!(result.is_err(), "invalid track id must be rejected");
        // Use .err().unwrap() to extract the error string without requiring T: Debug.
        let msg = result.err().unwrap();
        assert!(
            msg.contains("invalid --track-id"),
            "error must mention invalid --track-id, got: {msg}"
        );
    }

    #[test]
    fn test_build_review_v2_with_claude_reviewer_rejects_missing_track_dir() {
        // build_review_v2_with_claude_reviewer rejects a well-formed track_id when
        // the track directory does not exist.
        let _guard = env_lock().lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        setup_test_git_repo(dir.path());
        let _cwd = CwdGuard::save_current();
        std::env::set_current_dir(dir.path()).unwrap();

        let items_dir = dir.path().join("track/items");
        let track_id = domain::TrackId::try_new("missing-track-2026").unwrap();
        // Deliberately do NOT create track/items/missing-track-2026.

        let result =
            build_review_v2_with_claude_reviewer(&track_id, &items_dir, make_claude_reviewer());
        assert!(result.is_err(), "missing track directory must be rejected");
        // Use .err().unwrap() to extract the error string without requiring T: Debug.
        let msg = result.err().unwrap();
        assert!(
            msg.contains("does not exist"),
            "error must mention missing track directory, got: {msg}"
        );
    }

    #[test]
    fn test_reviewer_unexpected_after_spawn_classifies_child_poll_failure() {
        let error = ReviewerError::Unexpected("failed to poll reviewer child: io".to_owned());

        assert!(reviewer_error_is_subprocess_failure(&error));
    }

    #[test]
    fn test_reviewer_unexpected_before_spawn_is_not_subprocess_failure() {
        let error =
            ReviewerError::Unexpected("failed to write output-schema: disk full".to_owned());

        assert!(!reviewer_error_is_subprocess_failure(&error));
    }

    struct StaticDiffGetter;

    impl DiffGetter for StaticDiffGetter {
        fn list_diff_files(
            &self,
            _base: &domain::CommitHash,
        ) -> Result<Vec<domain::review_v2::FilePath>, usecase::review_v2::error::DiffGetError>
        {
            Ok(vec![domain::review_v2::FilePath::new("src/lib.rs").unwrap()])
        }
    }

    struct ChangingHasher {
        calls: std::sync::atomic::AtomicUsize,
    }

    impl ChangingHasher {
        fn new() -> Self {
            Self { calls: std::sync::atomic::AtomicUsize::new(0) }
        }
    }

    impl ReviewHasher for ChangingHasher {
        fn calc(
            &self,
            _target: &ReviewTarget,
        ) -> Result<domain::review_v2::ReviewHash, usecase::review_v2::error::ReviewHasherError>
        {
            let call = self.calls.fetch_add(1, Ordering::SeqCst);
            let suffix = if call == 0 { "1" } else { "2" };
            domain::review_v2::ReviewHash::computed(format!("rvw1:sha256:{suffix}"))
                .map_err(|e| usecase::review_v2::error::ReviewHasherError::Failed(e.to_string()))
        }
    }

    struct FailingSecondHasher {
        calls: std::sync::atomic::AtomicUsize,
    }

    impl FailingSecondHasher {
        fn new() -> Self {
            Self { calls: std::sync::atomic::AtomicUsize::new(0) }
        }
    }

    impl ReviewHasher for FailingSecondHasher {
        fn calc(
            &self,
            _target: &ReviewTarget,
        ) -> Result<domain::review_v2::ReviewHash, usecase::review_v2::error::ReviewHasherError>
        {
            let call = self.calls.fetch_add(1, Ordering::SeqCst);
            if call == 0 {
                domain::review_v2::ReviewHash::computed(format!("rvw1:sha256:{}", "1".repeat(64)))
                    .map_err(|e| {
                        usecase::review_v2::error::ReviewHasherError::Failed(e.to_string())
                    })
            } else {
                Err(usecase::review_v2::error::ReviewHasherError::Failed(
                    "post-review hash failure".to_owned(),
                ))
            }
        }
    }

    struct FindingsReviewer;

    impl FindingsReviewer {
        fn findings() -> Vec<domain::review_v2::ReviewerFinding> {
            vec![
                domain::review_v2::ReviewerFinding::new(
                    "first finding",
                    Some("P1".to_owned()),
                    Some("src/lib.rs".to_owned()),
                    Some(1),
                    Some("correctness".to_owned()),
                )
                .unwrap(),
                domain::review_v2::ReviewerFinding::new(
                    "second finding",
                    Some("P1".to_owned()),
                    Some("src/lib.rs".to_owned()),
                    Some(2),
                    Some("correctness".to_owned()),
                )
                .unwrap(),
            ]
        }
    }

    impl Reviewer for FindingsReviewer {
        fn review(&self, _target: &ReviewTarget) -> Result<(Verdict, LogInfo), ReviewerError> {
            let verdict = Verdict::findings_remain(Self::findings())
                .map_err(|e| ReviewerError::Unexpected(e.to_string()))?;
            Ok((verdict, LogInfo::new("test log")))
        }

        fn fast_review(
            &self,
            _target: &ReviewTarget,
        ) -> Result<(FastVerdict, LogInfo), ReviewerError> {
            let verdict = FastVerdict::findings_remain(Self::findings())
                .map_err(|e| ReviewerError::Unexpected(e.to_string()))?;
            Ok((verdict, LogInfo::new("test log")))
        }
    }

    fn hash_cycle_with_hasher<H>(
        review_store_root: &std::path::Path,
        hasher: H,
    ) -> (
        ReviewCycle<FindingsCountReviewer<FindingsReviewer>, H, StaticDiffGetter>,
        FsReviewStore,
        ReviewFindingsRecorder,
    )
    where
        H: ReviewHasher,
    {
        let track_id = domain::TrackId::try_new("my-test-track-2026").unwrap();
        let scope_config = domain::review_v2::ReviewScopeConfig::new(
            &track_id,
            vec![("infra".to_owned(), vec!["src/**".to_owned()], None)],
            vec![],
            vec![],
        )
        .unwrap();
        let base = domain::CommitHash::try_new("0".repeat(40)).unwrap();
        let review_store = FsReviewStore::new(
            review_store_root.join("review.json"),
            review_store_root.to_path_buf(),
        );
        let (reviewer, findings_recorder) = FindingsCountReviewer::new(FindingsReviewer);
        let cycle = ReviewCycle::new(base, scope_config, reviewer, StaticDiffGetter, hasher);
        (cycle, review_store, findings_recorder)
    }

    fn changed_hash_cycle(
        review_store_root: &std::path::Path,
    ) -> (
        ReviewCycle<FindingsCountReviewer<FindingsReviewer>, ChangingHasher, StaticDiffGetter>,
        FsReviewStore,
        ReviewFindingsRecorder,
    ) {
        hash_cycle_with_hasher(review_store_root, ChangingHasher::new())
    }

    fn post_review_hash_error_cycle(
        review_store_root: &std::path::Path,
    ) -> (
        ReviewCycle<FindingsCountReviewer<FindingsReviewer>, FailingSecondHasher, StaticDiffGetter>,
        FsReviewStore,
        ReviewFindingsRecorder,
    ) {
        hash_cycle_with_hasher(review_store_root, FailingSecondHasher::new())
    }

    fn assert_failure_preserves_findings_count<H>(
        round_type: &str,
        make_cycle: impl FnOnce(
            &std::path::Path,
        ) -> (
            ReviewCycle<FindingsCountReviewer<FindingsReviewer>, H, StaticDiffGetter>,
            FsReviewStore,
            ReviewFindingsRecorder,
        ),
        expect_message: &str,
    ) where
        H: ReviewHasher,
    {
        let dir = tempfile::tempdir().unwrap();
        let (cycle, review_store, findings_recorder) = make_cycle(dir.path());

        let outcome =
            dispatch_review_cycle("infra", round_type, cycle, review_store, findings_recorder)
                .expect(expect_message);

        assert!(
            matches!(
                outcome,
                CodexReviewOutcome::SubprocessFailed {
                    findings_count: 2,
                    verdict_parse_failed: false,
                    ..
                }
            ),
            "expected SubprocessFailed with findings_count=2"
        );
    }

    fn assert_file_changed_preserves_findings_count(round_type: &str) {
        assert_failure_preserves_findings_count(
            round_type,
            changed_hash_cycle,
            "file changed maps to subprocess failure outcome",
        );
    }

    #[test]
    fn test_dispatch_review_cycle_fast_file_changed_preserves_findings_count() {
        assert_file_changed_preserves_findings_count("fast");
    }

    #[test]
    fn test_dispatch_review_cycle_final_file_changed_preserves_findings_count() {
        assert_file_changed_preserves_findings_count("final");
    }

    fn assert_post_review_hash_error_preserves_findings_count(round_type: &str) {
        assert_failure_preserves_findings_count(
            round_type,
            post_review_hash_error_cycle,
            "post-review hash error maps to subprocess failure outcome",
        );
    }

    #[test]
    fn test_dispatch_review_cycle_fast_post_review_hash_error_preserves_findings_count() {
        assert_post_review_hash_error_preserves_findings_count("fast");
    }

    #[test]
    fn test_dispatch_review_cycle_final_post_review_hash_error_preserves_findings_count() {
        assert_post_review_hash_error_preserves_findings_count("final");
    }
}
