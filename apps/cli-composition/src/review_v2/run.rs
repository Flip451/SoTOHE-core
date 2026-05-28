//! Review cycle execution helpers (Codex and Claude).

use std::path::Path;

use domain::TrackId;

use infrastructure::review_v2::{ClaudeReviewer, CodexReviewer};

use super::shared::{
    CodexReviewOutcome, build_review_v2_with_claude_reviewer, build_review_v2_with_reviewer,
};

/// Runs the full Codex review cycle from string inputs.
///
/// Encapsulates `TrackId`, `ScopeName`, `RoundType`, `ReviewOutcome`, `Verdict`,
/// `FastVerdict`, `ReviewWriter`, and `ReviewerFinding` conversions so the CLI
/// layer never imports these domain types directly (CN-01 / AC-03).
///
/// Steps performed:
/// 1. Validates `track_id_str` and `group_str` (rejects invalid identifiers).
/// 2. Builds the v2 review composition with the provided `CodexReviewer`.
/// 3. Dispatches the review round (`review` or `fast_review`) per `round_type_str`.
/// 4. Writes the verdict to `review.json` via `ReviewWriter`.
/// 5. Returns `CodexReviewOutcome` describing the result.
///
/// # Errors
/// Returns a human-readable error string on failure at any step.
pub fn run_codex_review_str(
    track_id_str: &str,
    items_dir: &Path,
    group_str: &str,
    round_type_str: &str, // "fast" | "final"
    reviewer: CodexReviewer,
) -> Result<CodexReviewOutcome, String> {
    use domain::review_v2::{
        FastVerdict, MainScopeName, ReviewOutcome, ReviewWriter, ReviewerFinding, ScopeName,
        Verdict,
    };
    use usecase::review_workflow::{
        ReviewFinalPayload, ReviewPayloadVerdict, render_review_payload,
    };

    let track_id =
        TrackId::try_new(track_id_str).map_err(|e| format!("[ERROR] invalid track id: {e}"))?;

    let scope = if group_str == "other" {
        ScopeName::Other
    } else {
        match MainScopeName::new(group_str) {
            Ok(main) => ScopeName::Main(main),
            Err(e) => return Err(format!("[ERROR] invalid scope name: {e}")),
        }
    };

    let comp = build_review_v2_with_reviewer(&track_id, items_dir, reviewer)
        .map_err(|e| format!("[ERROR] v2 composition failed: {e}"))?;

    fn finding_to_payload(f: &ReviewerFinding) -> usecase::review_workflow::ReviewFinding {
        usecase::review_workflow::ReviewFinding {
            message: f.message().to_owned(),
            severity: f.severity().map(str::to_owned),
            file: f.file().map(str::to_owned),
            line: f.line(),
            category: f.category().map(str::to_owned),
        }
    }

    fn render_verdict_final(verdict: &Verdict) -> Result<(String, u8), String> {
        let (payload, exit_code) = match verdict {
            Verdict::ZeroFindings => (
                ReviewFinalPayload {
                    verdict: ReviewPayloadVerdict::ZeroFindings,
                    findings: vec![],
                },
                0u8,
            ),
            Verdict::FindingsRemain(nef) => {
                let findings = nef.as_slice().iter().map(finding_to_payload).collect();
                (
                    ReviewFinalPayload { verdict: ReviewPayloadVerdict::FindingsRemain, findings },
                    2u8,
                )
            }
        };
        let json = render_review_payload(&payload).map_err(|e| format!("[ERROR] {e}"))?;
        Ok((json, exit_code))
    }

    fn render_verdict_fast(verdict: &FastVerdict) -> Result<(String, u8), String> {
        let (payload, exit_code) = match verdict {
            FastVerdict::ZeroFindings => (
                ReviewFinalPayload {
                    verdict: ReviewPayloadVerdict::ZeroFindings,
                    findings: vec![],
                },
                0u8,
            ),
            FastVerdict::FindingsRemain(nef) => {
                let findings = nef.as_slice().iter().map(finding_to_payload).collect();
                (
                    ReviewFinalPayload { verdict: ReviewPayloadVerdict::FindingsRemain, findings },
                    2u8,
                )
            }
        };
        let json = render_review_payload(&payload).map_err(|e| format!("[ERROR] {e}"))?;
        Ok((json, exit_code))
    }

    match round_type_str {
        "final" => match comp.cycle.review(&scope) {
            Ok(ReviewOutcome::Skipped) => {
                Ok(CodexReviewOutcome::Skipped { scope_label: group_str.to_owned() })
            }
            Ok(ReviewOutcome::Reviewed { verdict, hash, .. }) => {
                comp.review_store
                    .write_verdict(&scope, &verdict, &hash)
                    .map_err(|e| format!("[ERROR] record failed: {e}"))?;
                let (json, exit_code) = render_verdict_final(&verdict)?;
                Ok(CodexReviewOutcome::FinalCompleted { verdict_json: json, exit_code })
            }
            Err(e) => Err(format!("[ERROR] {e}")),
        },
        "fast" => match comp.cycle.fast_review(&scope) {
            Ok(ReviewOutcome::Skipped) => {
                Ok(CodexReviewOutcome::Skipped { scope_label: group_str.to_owned() })
            }
            Ok(ReviewOutcome::Reviewed { verdict, hash, .. }) => {
                comp.review_store
                    .write_fast_verdict(&scope, &verdict, &hash)
                    .map_err(|e| format!("[ERROR] record failed: {e}"))?;
                let (json, exit_code) = render_verdict_fast(&verdict)?;
                Ok(CodexReviewOutcome::FastCompleted { verdict_json: json, exit_code })
            }
            Err(e) => Err(format!("[ERROR] {e}")),
        },
        other => Err(format!("[ERROR] unknown round type: '{other}' (expected 'fast' or 'final')")),
    }
}

/// Runs the full Claude review cycle from string inputs.
///
/// Mirrors [`run_codex_review_str`] with `ClaudeReviewer` in place of `CodexReviewer`.
///
/// Encapsulates `TrackId`, `ScopeName`, `RoundType`, `ReviewOutcome`, `Verdict`,
/// `FastVerdict`, `ReviewWriter`, and `ReviewerFinding` conversions so the CLI
/// layer never imports these domain types directly (CN-01 / AC-03).
///
/// Steps performed:
/// 1. Validates `track_id_str` and `group_str` (rejects invalid identifiers).
/// 2. Builds the v2 review composition with the provided `ClaudeReviewer`.
/// 3. Dispatches the review round (`review` or `fast_review`) per `round_type_str`.
/// 4. Writes the verdict to `review.json` via `ReviewWriter`.
/// 5. Returns `CodexReviewOutcome` describing the result.
///
/// # Errors
/// Returns a human-readable error string on failure at any step.
pub fn run_claude_review_str(
    track_id_str: &str,
    items_dir: &Path,
    group_str: &str,
    round_type_str: &str, // "fast" | "final"
    reviewer: ClaudeReviewer,
) -> Result<CodexReviewOutcome, String> {
    use domain::review_v2::{
        FastVerdict, MainScopeName, ReviewOutcome, ReviewWriter, ReviewerFinding, ScopeName,
        Verdict,
    };
    use usecase::review_workflow::{
        ReviewFinalPayload, ReviewPayloadVerdict, render_review_payload,
    };

    let track_id =
        TrackId::try_new(track_id_str).map_err(|e| format!("[ERROR] invalid track id: {e}"))?;

    let scope = if group_str == "other" {
        ScopeName::Other
    } else {
        match MainScopeName::new(group_str) {
            Ok(main) => ScopeName::Main(main),
            Err(e) => return Err(format!("[ERROR] invalid scope name: {e}")),
        }
    };

    let comp = build_review_v2_with_claude_reviewer(&track_id, items_dir, reviewer)
        .map_err(|e| format!("[ERROR] v2 composition failed: {e}"))?;

    fn finding_to_payload(f: &ReviewerFinding) -> usecase::review_workflow::ReviewFinding {
        usecase::review_workflow::ReviewFinding {
            message: f.message().to_owned(),
            severity: f.severity().map(str::to_owned),
            file: f.file().map(str::to_owned),
            line: f.line(),
            category: f.category().map(str::to_owned),
        }
    }

    fn render_verdict_final(verdict: &Verdict) -> Result<(String, u8), String> {
        let (payload, exit_code) = match verdict {
            Verdict::ZeroFindings => (
                ReviewFinalPayload {
                    verdict: ReviewPayloadVerdict::ZeroFindings,
                    findings: vec![],
                },
                0u8,
            ),
            Verdict::FindingsRemain(nef) => {
                let findings = nef.as_slice().iter().map(finding_to_payload).collect();
                (
                    ReviewFinalPayload { verdict: ReviewPayloadVerdict::FindingsRemain, findings },
                    2u8,
                )
            }
        };
        let json = render_review_payload(&payload).map_err(|e| format!("[ERROR] {e}"))?;
        Ok((json, exit_code))
    }

    fn render_verdict_fast(verdict: &FastVerdict) -> Result<(String, u8), String> {
        let (payload, exit_code) = match verdict {
            FastVerdict::ZeroFindings => (
                ReviewFinalPayload {
                    verdict: ReviewPayloadVerdict::ZeroFindings,
                    findings: vec![],
                },
                0u8,
            ),
            FastVerdict::FindingsRemain(nef) => {
                let findings = nef.as_slice().iter().map(finding_to_payload).collect();
                (
                    ReviewFinalPayload { verdict: ReviewPayloadVerdict::FindingsRemain, findings },
                    2u8,
                )
            }
        };
        let json = render_review_payload(&payload).map_err(|e| format!("[ERROR] {e}"))?;
        Ok((json, exit_code))
    }

    match round_type_str {
        "final" => match comp.cycle.review(&scope) {
            Ok(ReviewOutcome::Skipped) => {
                Ok(CodexReviewOutcome::Skipped { scope_label: group_str.to_owned() })
            }
            Ok(ReviewOutcome::Reviewed { verdict, hash, .. }) => {
                comp.review_store
                    .write_verdict(&scope, &verdict, &hash)
                    .map_err(|e| format!("[ERROR] record failed: {e}"))?;
                let (json, exit_code) = render_verdict_final(&verdict)?;
                Ok(CodexReviewOutcome::FinalCompleted { verdict_json: json, exit_code })
            }
            Err(e) => Err(format!("[ERROR] {e}")),
        },
        "fast" => match comp.cycle.fast_review(&scope) {
            Ok(ReviewOutcome::Skipped) => {
                Ok(CodexReviewOutcome::Skipped { scope_label: group_str.to_owned() })
            }
            Ok(ReviewOutcome::Reviewed { verdict, hash, .. }) => {
                comp.review_store
                    .write_fast_verdict(&scope, &verdict, &hash)
                    .map_err(|e| format!("[ERROR] record failed: {e}"))?;
                let (json, exit_code) = render_verdict_fast(&verdict)?;
                Ok(CodexReviewOutcome::FastCompleted { verdict_json: json, exit_code })
            }
            Err(e) => Err(format!("[ERROR] {e}")),
        },
        other => Err(format!("[ERROR] unknown round type: '{other}' (expected 'fast' or 'final')")),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use std::process::Command;
    use std::time::Duration;

    use infrastructure::review_v2::ClaudeReviewer;

    use super::*;
    use crate::review_v2::shared::{
        CodexReviewOutcome, build_review_v2_with_claude_reviewer,
        build_review_v2_with_claude_reviewer_str,
    };

    // Mutex so tests that mutate cwd do not race.
    static ENV_LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    fn env_lock() -> &'static std::sync::Mutex<()> {
        ENV_LOCK.get_or_init(|| std::sync::Mutex::new(()))
    }

    /// Guard that restores the working directory when dropped.
    struct CwdGuard {
        original: std::path::PathBuf,
    }

    impl CwdGuard {
        fn change_to(path: &std::path::Path) -> Self {
            let original = std::env::current_dir().unwrap();
            std::env::set_current_dir(path).unwrap();
            Self { original }
        }
    }

    impl Drop for CwdGuard {
        fn drop(&mut self) {
            let _ = std::env::set_current_dir(&self.original);
        }
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
        let _cwd = CwdGuard::change_to(dir.path());

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
    #[test]
    fn test_run_claude_review_str_fast_zero_findings_writes_verdict_and_returns_outcome() {
        // AC-03 / write-first / fail-closed: after a zero-findings verdict the verdict is
        // written to review.json before being returned, and a FastCompleted outcome is produced.
        let _guard = env_lock().lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let base_sha = setup_test_git_repo(dir.path());
        let _cwd = CwdGuard::change_to(dir.path());

        let script = dir.path().join("fake-claude.sh");
        write_fake_claude_script(
            &script,
            r#"{"type":"result","structured_output":{"verdict":"zero_findings","findings":[]}}"#,
        );

        let items_dir = dir.path().join("track/items");
        let track_id = "my-test-track-2026";
        let track_dir = items_dir.join(track_id);
        std::fs::create_dir_all(&track_dir).unwrap();
        // Write diff base pointing to the first commit so `src/lib.rs` appears in the diff.
        std::fs::write(track_dir.join(".commit_hash"), &base_sha).unwrap();

        let reviewer = make_reviewer_with_bin(&script);
        let result = run_claude_review_str(track_id, &items_dir, "infra", "fast", reviewer);

        let outcome = result.expect("fast zero-findings review must succeed");
        assert!(
            matches!(outcome, CodexReviewOutcome::FastCompleted { exit_code: 0, .. }),
            "expected FastCompleted with exit_code 0"
        );

        // write-first: review.json must have been written (fail-closed guarantee).
        let review_json = track_dir.join("review.json");
        assert!(review_json.exists(), "review.json must be written (write-first contract)");
    }

    #[cfg(unix)]
    #[test]
    fn test_run_claude_review_str_fast_findings_remain_writes_verdict_and_returns_outcome() {
        // AC-03: findings_remain case also writes review.json (write-first / fail-closed).
        let _guard = env_lock().lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let base_sha = setup_test_git_repo(dir.path());
        let _cwd = CwdGuard::change_to(dir.path());

        let script = dir.path().join("fake-claude-findings.sh");
        write_fake_claude_script(
            &script,
            r#"{"type":"result","structured_output":{"verdict":"findings_remain","findings":[{"message":"A finding","severity":"P2","file":"src/lib.rs","line":1,"category":"style"}]}}"#,
        );

        let items_dir = dir.path().join("track/items");
        let track_id = "my-test-track-2026";
        let track_dir = items_dir.join(track_id);
        std::fs::create_dir_all(&track_dir).unwrap();
        // Write diff base pointing to the first commit so `src/lib.rs` appears in the diff.
        std::fs::write(track_dir.join(".commit_hash"), &base_sha).unwrap();

        let reviewer = make_reviewer_with_bin(&script);
        let result = run_claude_review_str(track_id, &items_dir, "infra", "fast", reviewer);

        let outcome = result.expect("fast findings_remain review must succeed");
        assert!(
            matches!(outcome, CodexReviewOutcome::FastCompleted { exit_code: 2, .. }),
            "expected FastCompleted with exit_code 2"
        );

        // write-first: review.json must have been written before returning the outcome.
        let review_json = track_dir.join("review.json");
        assert!(review_json.exists(), "review.json must be written (write-first contract)");
    }

    #[cfg(unix)]
    #[test]
    fn test_run_claude_review_str_final_zero_findings_writes_verdict_and_returns_outcome() {
        // AC-03 / final-round path: a zero-findings final verdict writes review.json
        // and returns FinalCompleted with exit_code 0.
        let _guard = env_lock().lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let base_sha = setup_test_git_repo(dir.path());
        let _cwd = CwdGuard::change_to(dir.path());

        let script = dir.path().join("fake-claude-final.sh");
        write_fake_claude_script(
            &script,
            r#"{"type":"result","structured_output":{"verdict":"zero_findings","findings":[]}}"#,
        );

        let items_dir = dir.path().join("track/items");
        let track_id = "my-test-track-final-2026";
        let track_dir = items_dir.join(track_id);
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(track_dir.join(".commit_hash"), &base_sha).unwrap();

        let reviewer = make_reviewer_with_bin(&script);
        let result = run_claude_review_str(track_id, &items_dir, "infra", "final", reviewer);

        let outcome = result.expect("final zero-findings review must succeed");
        assert!(
            matches!(outcome, CodexReviewOutcome::FinalCompleted { exit_code: 0, .. }),
            "expected FinalCompleted with exit_code 0"
        );

        // write-first: review.json must have been written (fail-closed guarantee).
        let review_json = track_dir.join("review.json");
        assert!(review_json.exists(), "review.json must be written (write-first contract)");
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
        let _cwd = CwdGuard::change_to(dir.path());

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
}
