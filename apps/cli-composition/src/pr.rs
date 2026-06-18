//! `pr` command family — CliApp impl methods.
//!
//! Implements the full PR workflow: push, ensure-pr, status, wait-and-merge,
//! trigger-review, poll-review, and review-cycle. All methods use concrete
//! infrastructure types internally; the public API exposes only primitives and
//! `CommandOutcome` (CN-02).
//!
//! Private polling and review helpers are in `poll` (see `pr/poll.rs`).

mod poll;

use std::fs;
use std::thread;
use std::time::{Duration, Instant};

use crate::{CliApp, CommandOutcome};

use poll::{
    PollReviewResult, checks_summary, cleanup_trigger_state, ensure_pr_body_file,
    format_review_summary, parse_review, poll_review_for_cycle, resolve_branch_context,
    resume_trigger_state, trigger_new_review,
};

// ---------------------------------------------------------------------------
// CliApp implementations
// ---------------------------------------------------------------------------

impl CliApp {
    /// Push the current track branch to origin.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn pr_push(&self, track_id: Option<String>) -> Result<CommandOutcome, String> {
        use infrastructure::git_cli::{GitRepository as _, SystemGitRepo};

        let ctx = resolve_branch_context(track_id.as_deref())?;
        let repo = SystemGitRepo::discover().map_err(|e| e.to_string())?;
        println!("Pushing {} to origin...", ctx.branch);
        repo.push_branch(&ctx.branch).map_err(|e| e.to_string())?;
        let stdout = format!("[OK] Pushed {}", ctx.branch);
        Ok(CommandOutcome::success(Some(stdout)))
    }

    /// Create or reuse a PR for the current track branch.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn pr_ensure(
        &self,
        track_id: Option<String>,
        base: String,
    ) -> Result<CommandOutcome, String> {
        use infrastructure::gh_cli::{GhClient as _, SystemGhClient};
        use usecase::pr_workflow::pr_title;

        let ctx = resolve_branch_context(track_id.as_deref())?;
        let client = SystemGhClient;

        match client.find_open_pr(&ctx.branch, &base) {
            Ok(Some(pr)) => {
                return Ok(CommandOutcome::success(Some(format!(
                    "[OK] Reusing existing PR #{pr}"
                ))));
            }
            Ok(None) => {}
            Err(err) => {
                eprintln!("[ERROR] {err}");
                return Ok(CommandOutcome::failure(None));
            }
        }

        let body_file = ensure_pr_body_file(&ctx).map_err(|e| {
            eprintln!("[ERROR] {e}");
            e
        })?;
        let title = pr_title(&ctx);
        match client.create_pr(&ctx.branch, &base, &title, &body_file) {
            Ok(pr) => {
                let _ = fs::remove_file(&body_file);
                Ok(CommandOutcome::success(Some(format!("[OK] Created PR #{pr}"))))
            }
            Err(err) => {
                let _ = fs::remove_file(&body_file);
                eprintln!("[ERROR] {err}");
                Ok(CommandOutcome::failure(None))
            }
        }
    }

    /// Show current PR check status.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn pr_status(&self, pr: String) -> Result<CommandOutcome, String> {
        use infrastructure::gh_cli::{GhClient as _, SystemGhClient};
        use usecase::pr_workflow::CheckSummary;

        let client = SystemGhClient;
        let checks = client.pr_checks(&pr).map_err(|e| e.to_string())?;
        let url = client.pr_url(&pr);
        let mut lines = vec![format!("PR: {url}")];
        let exit_code = match checks_summary(&checks) {
            CheckSummary::AllPassed => {
                lines.push("[OK] All checks passed.".to_owned());
                0u8
            }
            CheckSummary::Failed(names) => {
                lines.push(format!("[FAIL] Failed checks: {}", names.join(", ")));
                1u8
            }
            CheckSummary::Pending(names) => {
                lines.push(format!("[PENDING] Waiting: {}", names.join(", ")));
                2u8
            }
        };
        Ok(CommandOutcome { stdout: Some(lines.join("\n")), stderr: None, exit_code })
    }

    /// Poll PR checks until they pass, then merge.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn pr_wait_and_merge(
        &self,
        pr: String,
        interval: u64,
        timeout: u64,
        method: String,
    ) -> Result<CommandOutcome, String> {
        use infrastructure::gh_cli::{GhClient as _, SystemGhClient};
        use infrastructure::git_cli::{GitRepository as _, SystemGitRepo};
        use usecase::pr_workflow::{WaitDecision, decide_wait_action};

        let client = SystemGhClient;
        let branch = client.pr_head_branch(&pr).map_err(|e| e.to_string())?;
        let repo = SystemGitRepo::discover().map_err(|e| e.to_string())?;
        match repo.output(&["fetch", "origin", &branch]) {
            Ok(o) if !o.status.success() => {
                let stderr = String::from_utf8_lossy(&o.stderr);
                return Err(format!("git fetch origin/{branch} failed: {stderr}"));
            }
            Err(e) => {
                return Err(format!("failed to run git fetch: {e}"));
            }
            Ok(_) => {}
        }

        let reader = infrastructure::verify::merge_gate_adapter::GitShowTrackBlobReader::new(
            repo.root().to_path_buf(),
        );

        let task_outcome =
            usecase::task_completion::check_tasks_resolved_from_git_ref(&branch, &reader);
        if task_outcome.has_errors() {
            let mut lines = Vec::new();
            for finding in task_outcome.findings() {
                lines.push(format!("[BLOCKED] {}", finding.message()));
            }
            lines.push("Run track-transition to mark tasks as done before merging.".to_owned());
            return Ok(CommandOutcome {
                stdout: None,
                stderr: Some(lines.join("\n")),
                exit_code: 1,
            });
        }

        // Load SignalGateMatrix from `.harness/config/signal-gates.json`.
        // The config lives in the repo root, resolved from the git repo discovery.
        // If the file is absent or invalid, block fail-closed so that a missing
        // config does not silently bypass the gate.
        let signal_gates_path = repo.root().join(".harness/config/signal-gates.json");
        let gate_matrix =
            match infrastructure::verify::signal_gates_config::load_signal_gates_config(
                signal_gates_path.clone(),
            ) {
                Ok(matrix) => matrix,
                Err(e) => {
                    return Ok(CommandOutcome {
                        stdout: None,
                        stderr: Some(format!(
                            "[BLOCKED] failed to load signal-gates config from {}: {e}",
                            signal_gates_path.display()
                        )),
                        exit_code: 1,
                    });
                }
            };

        let gate_outcome =
            usecase::merge_gate::check_strict_merge_gate(&branch, &reader, &gate_matrix);
        if gate_outcome.has_errors() {
            let mut lines = vec!["[BLOCKED] strict spec signal gate failed:".to_owned()];
            for finding in gate_outcome.findings() {
                lines.push(format!("[BLOCKED] {}", finding.message()));
            }
            return Ok(CommandOutcome {
                stdout: None,
                stderr: Some(lines.join("\n")),
                exit_code: 1,
            });
        }

        let url = client.pr_url(&pr);
        println!("PR: {url}");
        println!("Polling checks every {interval}s (timeout {timeout}s)...");

        let start = Instant::now();
        loop {
            let elapsed = start.elapsed().as_secs();
            let checks = client.pr_checks(&pr).map_err(|e| e.to_string())?;
            match decide_wait_action(checks_summary(&checks), elapsed, timeout, interval) {
                WaitDecision::MergeNow => {
                    println!("[OK] All checks passed. Merging...");
                    client.merge_pr(&pr, &method).map_err(|e| e.to_string())?;
                    return Ok(CommandOutcome::success(Some(format!(
                        "[OK] PR #{pr} merged ({method})."
                    ))));
                }
                WaitDecision::FailChecks(names) => {
                    println!("[FAIL] Checks failed: {}", names.join(", "));
                    println!("Fix the failures and push again.");
                    return Ok(CommandOutcome::failure(None));
                }
                WaitDecision::Timeout(names) => {
                    println!("[TIMEOUT] Still pending after {timeout}s: {}", names.join(", "));
                    return Ok(CommandOutcome::failure(None));
                }
                WaitDecision::Wait { pending, delay_seconds } => {
                    println!(
                        "  [{elapsed}s] Pending: {} (retry in {delay_seconds}s)",
                        pending.join(", ")
                    );
                    thread::sleep(Duration::from_secs(delay_seconds));
                }
            }
        }
    }

    /// Post `@codex review` comment on a PR.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn pr_trigger_review(&self, pr: String) -> Result<CommandOutcome, String> {
        use infrastructure::agent_profiles::{AGENT_PROFILES_PATH, AgentProfiles, RoundType};
        use infrastructure::gh_cli::{GhClient as _, SystemGhClient};
        use infrastructure::git_cli::{GitRepository as _, SystemGitRepo};

        let git_repo = SystemGitRepo::discover().map_err(|e| e.to_string())?;
        let profiles_path = git_repo.root().join(AGENT_PROFILES_PATH);
        let profiles = AgentProfiles::load(&profiles_path).map_err(|e| format!("{e}"))?;
        let resolved =
            profiles.resolve_execution("pr-reviewer", RoundType::Final).ok_or_else(|| {
                "pr-reviewer capability not defined in agent-profiles.json".to_owned()
            })?;
        usecase::pr_review::validate_reviewer_provider(&resolved.provider)
            .map_err(|e| e.to_string())?;

        let client = SystemGhClient;
        let repo = client.repo_nwo().map_err(|e| e.to_string())?;
        let response =
            client.post_issue_comment(&repo, &pr, "@codex review").map_err(|e| e.to_string())?;

        let created_at = serde_json::from_str::<serde_json::Value>(&response)
            .ok()
            .and_then(|v| v.get("created_at")?.as_str().map(String::from))
            .unwrap_or_default();

        if created_at.is_empty() {
            return Err("could not determine trigger timestamp from API response".to_owned());
        }

        let stdout = format!(
            "[OK] Posted '@codex review' on PR #{pr} at {created_at}\nTRIGGER_TIMESTAMP={created_at}"
        );
        Ok(CommandOutcome::success(Some(stdout)))
    }

    /// Poll GitHub API for a Codex bot review.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn pr_poll_review(
        &self,
        pr: String,
        trigger_timestamp: String,
        interval: u64,
        timeout: u64,
    ) -> Result<CommandOutcome, String> {
        use infrastructure::gh_cli::SystemGhClient;
        use infrastructure::git_cli::{GitRepository as _, SystemGitRepo};

        let head = SystemGitRepo::discover().ok().and_then(|r| {
            r.output(&["rev-parse", "HEAD"])
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        });

        match poll_review_for_cycle(
            &pr,
            &trigger_timestamp,
            interval,
            timeout,
            &SystemGhClient,
            &thread::sleep,
            head.as_deref(),
        )? {
            PollReviewResult::ReviewFound(review) => {
                let review_str = serde_json::to_string(&review).unwrap_or_default();
                Ok(CommandOutcome::success(Some(review_str)))
            }
            PollReviewResult::ZeroFindings => Ok(CommandOutcome::success(Some(
                r#"{"verdict":"zero_findings","findings":[]}"#.to_owned(),
            ))),
            PollReviewResult::Timeout => Ok(CommandOutcome::failure(None)),
        }
    }

    /// Full PR review cycle: push → ensure-pr → trigger → poll → parse → report.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn pr_review_cycle(
        &self,
        track_id: Option<String>,
        resume: bool,
    ) -> Result<CommandOutcome, String> {
        use infrastructure::agent_profiles::{AGENT_PROFILES_PATH, AgentProfiles, RoundType};
        use infrastructure::gh_cli::{GhClient as _, SystemGhClient};
        use infrastructure::git_cli::{GitRepository as _, SystemGitRepo};

        let repo = SystemGitRepo::discover().map_err(|e| e.to_string())?;

        let profiles_path = repo.root().join(AGENT_PROFILES_PATH);
        let profiles = AgentProfiles::load(&profiles_path).map_err(|e| format!("{e}"))?;
        let resolved =
            profiles.resolve_execution("pr-reviewer", RoundType::Final).ok_or_else(|| {
                "pr-reviewer capability not defined in agent-profiles.json".to_owned()
            })?;
        usecase::pr_review::validate_reviewer_provider(&resolved.provider)
            .map_err(|e| e.to_string())?;

        let branch = repo
            .current_branch()
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "could not determine current branch".to_owned())?;
        if !branch.starts_with("track/") {
            return Err("not on a track branch (expected track/<id>); \
                 switch to the track branch and retry."
                .to_owned());
        }

        let active_track_id = branch.strip_prefix("track/").unwrap_or(&branch).to_owned();
        let client = SystemGhClient;

        let (pr_number, trigger_timestamp, head_ref_owned) = if resume {
            resume_trigger_state(&active_track_id)?
        } else {
            match trigger_new_review(track_id.as_deref(), &active_track_id, &client)? {
                Some(tuple) => tuple,
                None => return Ok(CommandOutcome::failure(None)),
            }
        };

        let nwo = client.repo_nwo().map_err(|e| e.to_string())?;
        let head_ref = head_ref_owned.as_deref();

        let poll_result = poll_review_for_cycle(
            &pr_number,
            &trigger_timestamp,
            15,
            600,
            &client,
            &thread::sleep,
            head_ref,
        )?;

        let result = match poll_result {
            PollReviewResult::ZeroFindings => {
                let stdout = format!(
                    "\n=== PR Review Result: PASS ===\nPR: #{pr_number}\n\
                     Zero findings detected (bot signalled no issues)."
                );
                Ok(CommandOutcome::success(Some(stdout)))
            }
            PollReviewResult::Timeout => Ok(CommandOutcome::failure(None)),
            PollReviewResult::ReviewFound(review) => {
                let parsed = parse_review(&pr_number, &review, &nwo, &client)?;
                let summary = format_review_summary(&pr_number, &parsed);
                // ReviewFound always exits 0 (D1/AC-09): pass/fail judgment is
                // delegated to the calling agent; Rust no longer gates on findings.
                Ok(CommandOutcome::success(Some(summary)))
            }
        };

        // Clean up trigger state on successful completion (not on timeout).
        if matches!(&result, Ok(outcome) if outcome.exit_code == 0) {
            cleanup_trigger_state(&active_track_id);
        }

        result
    }
}
