//! CLI subcommands for pull-request workflow wrappers.
//!
//! Thin delegation layer: all logic lives in `cli_composition::CliApp`.
//! Production code has no direct dependency on `infrastructure` or `usecase`.

use std::process::ExitCode;

use clap::{Args, Subcommand};
use cli_composition::CliApp;

use crate::CliError;

#[derive(Debug, Subcommand)]
pub enum PrCommand {
    /// Push the current track branch to origin.
    Push(PushArgs),
    /// Create or reuse a PR for the current track branch.
    EnsurePr(EnsurePrArgs),
    /// Show current PR check status.
    Status(StatusArgs),
    /// Poll PR checks until they pass, then merge.
    WaitAndMerge(WaitAndMergeArgs),
    /// Post '@codex review' comment on a PR.
    TriggerReview(TriggerReviewArgs),
    /// Poll GitHub API for a Codex bot review.
    PollReview(PollReviewArgs),
    /// Full PR review cycle: push → ensure-pr → trigger → poll → parse → report.
    ReviewCycle(ReviewCycleArgs),
}

#[derive(Debug, Args)]
pub struct PushArgs {
    /// Deprecated compatibility option; PR commands resolve from the current track branch.
    #[arg(long)]
    pub track_id: Option<String>,
}

#[derive(Debug, Args)]
pub struct EnsurePrArgs {
    /// Deprecated compatibility option; PR commands resolve from the current track branch.
    #[arg(long)]
    pub track_id: Option<String>,
    /// Base branch for the PR.
    #[arg(long, default_value = "main")]
    pub base: String,
}

#[derive(Debug, Args)]
pub struct StatusArgs {
    pub pr: String,
}

#[derive(Debug, Args)]
pub struct WaitAndMergeArgs {
    pub pr: String,
    #[arg(long, default_value_t = 15)]
    pub interval: u64,
    #[arg(long, default_value_t = 600)]
    pub timeout: u64,
    #[arg(long, value_parser = ["merge", "squash", "rebase"], default_value = "merge")]
    pub method: String,
}

#[derive(Debug, Args)]
pub struct TriggerReviewArgs {
    /// PR number.
    pub pr: String,
}

#[derive(Debug, Args)]
pub struct PollReviewArgs {
    /// PR number.
    pub pr: String,
    /// ISO 8601 trigger timestamp (from trigger-review output).
    pub trigger_timestamp: String,
    /// Poll interval in seconds.
    #[arg(long, default_value_t = 15)]
    pub interval: u64,
    /// Poll timeout in seconds.
    #[arg(long, default_value_t = 600)]
    pub timeout: u64,
}

#[derive(Debug, Args)]
pub struct ReviewCycleArgs {
    /// Deprecated compatibility option; PR commands resolve from the current track branch.
    #[arg(long)]
    pub track_id: Option<String>,
    /// Resume polling from a previously persisted trigger state file.
    #[arg(long)]
    pub resume: bool,
}

/// Dispatch a `PrCommand` to the appropriate `CliApp` method.
pub fn execute(cmd: PrCommand) -> ExitCode {
    let app = CliApp::new();
    match cmd {
        PrCommand::Push(args) => run(app.pr_push(args.track_id)),
        PrCommand::EnsurePr(args) => run(app.pr_ensure(args.track_id, args.base)),
        PrCommand::Status(args) => run(app.pr_status(args.pr)),
        PrCommand::WaitAndMerge(args) => {
            run(app.pr_wait_and_merge(args.pr, args.interval, args.timeout, args.method))
        }
        PrCommand::TriggerReview(args) => run(app.pr_trigger_review(args.pr)),
        PrCommand::PollReview(args) => {
            run(app.pr_poll_review(args.pr, args.trigger_timestamp, args.interval, args.timeout))
        }
        PrCommand::ReviewCycle(args) => run(app.pr_review_cycle(args.track_id, args.resume)),
    }
}

fn run(result: Result<cli_composition::CommandOutcome, String>) -> ExitCode {
    match result {
        Ok(outcome) => {
            if let Some(ref s) = outcome.stdout {
                println!("{s}");
            }
            if let Some(ref s) = outcome.stderr {
                eprintln!("{s}");
            }
            ExitCode::from(outcome.exit_code)
        }
        Err(err) => {
            let cli_err = CliError::Message(err);
            eprintln!("{cli_err}");
            cli_err.exit_code()
        }
    }
}

// ---------------------------------------------------------------------------
// Test-only: thin re-implementation stubs so pr_tests.rs can use generic
// helpers that exercise PR logic with fake GhClient implementations.
//
// These functions delegate to the infrastructure layer directly within the
// `#[cfg(test)]` scope, keeping production code free of those imports
// (AC-06 compliant). They are *not* dead code — pr_tests.rs uses `super::`.
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(dead_code)]
mod test_helpers {
    use std::fs;
    use std::path::PathBuf;
    use std::process::ExitCode;
    use std::time::{Duration, Instant};

    use infrastructure::gh_cli::{GhClient, PrCheckRecord};
    use usecase::pr_review;
    use usecase::pr_workflow::{
        CheckSummary, PrBranchContext, PrCheckStatus, PrCheckView, WaitDecision,
        decide_wait_action, pr_body, pr_title, summarize_checks,
    };

    const CODEX_BOT_LOGINS: &[&str] =
        &["openai-codex[bot]", "codex[bot]", "chatgpt-codex-connector[bot]"];

    pub fn is_codex_bot(login: &str) -> bool {
        let lower = login.to_lowercase();
        CODEX_BOT_LOGINS.iter().any(|known| *known == lower)
    }

    pub fn normalize_check_status(check: &PrCheckRecord) -> PrCheckStatus {
        let state =
            if !check.bucket.is_empty() { check.bucket.as_str() } else { check.state.as_str() };
        match state.to_uppercase().as_str() {
            "SUCCESS" | "PASS" | "SKIPPING" => PrCheckStatus::Passed,
            "FAILURE" | "FAIL" | "CANCEL" => PrCheckStatus::Failed,
            _ => PrCheckStatus::Pending,
        }
    }

    pub fn checks_summary(checks: &[PrCheckRecord]) -> CheckSummary {
        let views = checks
            .iter()
            .map(|c| PrCheckView { name: c.name.clone(), status: normalize_check_status(c) })
            .collect::<Vec<_>>();
        summarize_checks(&views)
    }

    pub fn status_with<C: GhClient>(pr: &str, client: &C) -> ExitCode {
        let checks = match client.pr_checks(pr) {
            Ok(checks) => checks,
            Err(err) => {
                println!("[ERROR] {err}");
                return ExitCode::FAILURE;
            }
        };
        let url = client.pr_url(pr);
        println!("PR: {url}");
        match checks_summary(&checks) {
            CheckSummary::AllPassed => {
                println!("[OK] All checks passed.");
                ExitCode::SUCCESS
            }
            CheckSummary::Failed(names) => {
                println!("[FAIL] Failed checks: {}", names.join(", "));
                ExitCode::FAILURE
            }
            CheckSummary::Pending(names) => {
                println!("[PENDING] Waiting: {}", names.join(", "));
                ExitCode::from(2)
            }
        }
    }

    pub fn wait_and_merge_with<C, Sleep>(
        pr: &str,
        interval: u64,
        timeout: u64,
        method: &str,
        client: &C,
        sleep: &Sleep,
    ) -> ExitCode
    where
        C: GhClient,
        Sleep: Fn(Duration),
    {
        let url = client.pr_url(pr);
        println!("PR: {url}");
        println!("Polling checks every {interval}s (timeout {timeout}s)...");

        let start = Instant::now();
        loop {
            let elapsed = start.elapsed().as_secs();
            let checks = match client.pr_checks(pr) {
                Ok(checks) => checks,
                Err(err) => {
                    println!("[ERROR] {err}");
                    return ExitCode::FAILURE;
                }
            };
            match decide_wait_action(checks_summary(&checks), elapsed, timeout, interval) {
                WaitDecision::MergeNow => {
                    println!("[OK] All checks passed. Merging...");
                    match client.merge_pr(pr, method) {
                        Ok(()) => {
                            println!("[OK] PR #{pr} merged ({method}).");
                            return ExitCode::SUCCESS;
                        }
                        Err(err) => {
                            println!("[ERROR] Merge failed: {err}");
                            return ExitCode::FAILURE;
                        }
                    }
                }
                WaitDecision::FailChecks(names) => {
                    println!("[FAIL] Checks failed: {}", names.join(", "));
                    println!("Fix the failures and push again.");
                    return ExitCode::FAILURE;
                }
                WaitDecision::Timeout(names) => {
                    println!("[TIMEOUT] Still pending after {timeout}s: {}", names.join(", "));
                    return ExitCode::FAILURE;
                }
                WaitDecision::Wait { pending, delay_seconds } => {
                    println!(
                        "  [{elapsed}s] Pending: {} (retry in {delay_seconds}s)",
                        pending.join(", ")
                    );
                    sleep(Duration::from_secs(delay_seconds));
                }
            }
        }
    }

    pub fn ensure_pr_with<C: GhClient>(ctx: &PrBranchContext, base: &str, client: &C) -> ExitCode {
        match client.find_open_pr(&ctx.branch, base) {
            Ok(Some(pr)) => {
                println!("[OK] Reusing existing PR #{pr}");
                return ExitCode::SUCCESS;
            }
            Ok(None) => {}
            Err(err) => {
                eprintln!("[ERROR] {err}");
                return ExitCode::FAILURE;
            }
        }

        let body_dir = PathBuf::from("tmp");
        if let Err(err) = fs::create_dir_all(&body_dir) {
            eprintln!("[ERROR] failed to create tmp dir: {err}");
            return ExitCode::FAILURE;
        }
        let body_file = body_dir.join(format!("pr-body-{}.md", std::process::id()));
        let body_text = pr_body(ctx);
        if let Err(err) = fs::write(&body_file, &body_text) {
            eprintln!("[ERROR] failed to write PR body file: {err}");
            return ExitCode::FAILURE;
        }

        let title = pr_title(ctx);
        match client.create_pr(&ctx.branch, base, &title, &body_file) {
            Ok(pr) => {
                let _ = fs::remove_file(&body_file);
                println!("[OK] Created PR #{pr}");
                ExitCode::SUCCESS
            }
            Err(err) => {
                let _ = fs::remove_file(&body_file);
                eprintln!("[ERROR] {err}");
                ExitCode::FAILURE
            }
        }
    }

    /// Outcome of a poll-review operation.
    #[derive(Debug)]
    pub enum PollReviewResult {
        ReviewFound(serde_json::Value),
        ZeroFindings,
        Timeout,
    }

    fn check_reaction_zero_findings<C: GhClient>(
        client: &C,
        repo: &str,
        pr: &str,
        trigger_dt: chrono::DateTime<chrono::FixedOffset>,
    ) -> Result<bool, String> {
        let reactions_json = client.list_reactions(repo, pr).map_err(|e| e.to_string())?;
        let reactions = pr_review::parse_paginated_json(&reactions_json)
            .map_err(|e| format!("failed to parse reactions JSON: {e}"))?;
        for reaction in &reactions {
            let content = reaction.get("content").and_then(|c| c.as_str()).unwrap_or("");
            if content != "+1" {
                continue;
            }
            let author = reaction
                .get("user")
                .and_then(|u| u.get("login"))
                .and_then(|l| l.as_str())
                .unwrap_or("");
            if !is_codex_bot(author) {
                continue;
            }
            let created_raw = reaction.get("created_at").and_then(|s| s.as_str()).unwrap_or("");
            if created_raw.is_empty() {
                continue;
            }
            let created_str = created_raw.replace('Z', "+00:00");
            let created_dt = chrono::DateTime::parse_from_rfc3339(&created_str)
                .map_err(|e| format!("invalid reaction created_at: {e}"))?;
            if created_dt >= trigger_dt {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn check_comment_zero_findings<C: GhClient>(
        client: &C,
        repo: &str,
        pr: &str,
        trigger_dt: chrono::DateTime<chrono::FixedOffset>,
    ) -> Result<bool, String> {
        let comments_json = client.list_issue_comments(repo, pr).map_err(|e| e.to_string())?;
        let comments = pr_review::parse_paginated_json(&comments_json)
            .map_err(|e| format!("failed to parse comments JSON: {e}"))?;
        for comment in &comments {
            let author = comment
                .get("user")
                .and_then(|u| u.get("login"))
                .and_then(|l| l.as_str())
                .unwrap_or("");
            if !is_codex_bot(author) {
                continue;
            }
            let created_raw = comment.get("created_at").and_then(|s| s.as_str()).unwrap_or("");
            if created_raw.is_empty() {
                continue;
            }
            let created_str = created_raw.replace('Z', "+00:00");
            let created_dt = chrono::DateTime::parse_from_rfc3339(&created_str)
                .map_err(|e| format!("invalid comment created_at: {e}"))?;
            if created_dt < trigger_dt {
                continue;
            }
            let body = comment.get("body").and_then(|b| b.as_str()).unwrap_or("");
            if body.contains("Didn't find any major issues") {
                return Ok(true);
            }
        }
        Ok(false)
    }

    #[allow(clippy::too_many_lines)]
    pub fn poll_review_for_cycle<C, Sleep>(
        pr: &str,
        trigger_timestamp: &str,
        interval: u64,
        timeout: u64,
        client: &C,
        sleep: &Sleep,
        head_commit: Option<&str>,
    ) -> Result<PollReviewResult, String>
    where
        C: GhClient,
        Sleep: Fn(Duration),
    {
        let trigger_time = trigger_timestamp.replace('Z', "+00:00");
        let trigger_dt = chrono::DateTime::parse_from_rfc3339(&trigger_time)
            .map_err(|e| format!("invalid trigger timestamp: {e}"))?;

        let repo_nwo = client.repo_nwo().map_err(|e| e.to_string())?;
        let deadline = Instant::now() + Duration::from_secs(timeout.min(86400));
        let mut any_bot_activity = false;

        eprintln!(
            "Polling for Codex review on PR #{pr} (interval={interval}s, timeout={timeout}s)..."
        );

        loop {
            if Instant::now() >= deadline {
                break;
            }

            let reviews_json = client.list_reviews(&repo_nwo, pr).map_err(|e| e.to_string())?;
            let reviews = pr_review::parse_paginated_json(&reviews_json)
                .map_err(|e| format!("failed to parse reviews JSON: {e}"))?;
            for review in &reviews {
                let author = review
                    .get("user")
                    .and_then(|u| u.get("login"))
                    .and_then(|l| l.as_str())
                    .unwrap_or("");
                if !is_codex_bot(author) {
                    continue;
                }
                let submitted_raw =
                    review.get("submitted_at").and_then(|s| s.as_str()).unwrap_or("");
                if submitted_raw.is_empty() {
                    continue;
                }
                let submitted_str = submitted_raw.replace('Z', "+00:00");
                let submitted_dt = chrono::DateTime::parse_from_rfc3339(&submitted_str)
                    .map_err(|e| format!("invalid review submitted_at: {e}"))?;
                if submitted_dt >= trigger_dt {
                    any_bot_activity = true;
                    let state = review.get("state").and_then(|s| s.as_str()).unwrap_or("");
                    if matches!(state, "APPROVED" | "CHANGES_REQUESTED" | "COMMENTED") {
                        let review_id = review.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
                        eprintln!("[OK] Found Codex review (id={review_id}, state={state})");
                        let mut sanitized = review.clone();
                        if let Some(obj) = sanitized.as_object_mut() {
                            if let Some(serde_json::Value::String(body)) = obj.get("body") {
                                let clean = pr_review::sanitize_text(body);
                                obj.insert("body".to_owned(), serde_json::Value::String(clean));
                            }
                        }
                        return Ok(PollReviewResult::ReviewFound(sanitized));
                    }
                }
            }

            if head_commit.is_some() {
                if check_reaction_zero_findings(client, &repo_nwo, pr, trigger_dt)? {
                    eprintln!("[OK] Zero-findings detected via +1 reaction.");
                    return Ok(PollReviewResult::ZeroFindings);
                }

                let has_stale_reaction = {
                    let reactions_json =
                        client.list_reactions(&repo_nwo, pr).map_err(|e| e.to_string())?;
                    let reactions = pr_review::parse_paginated_json(&reactions_json)
                        .map_err(|e| format!("failed to parse reactions JSON: {e}"))?;
                    reactions.iter().any(|r| {
                        let content = r.get("content").and_then(|c| c.as_str()).unwrap_or("");
                        let author = r
                            .get("user")
                            .and_then(|u| u.get("login"))
                            .and_then(|l| l.as_str())
                            .unwrap_or("");
                        content == "+1" && is_codex_bot(author)
                    })
                };

                if has_stale_reaction
                    && check_comment_zero_findings(client, &repo_nwo, pr, trigger_dt)?
                {
                    eprintln!("[OK] Zero-findings detected via comment text fallback.");
                    return Ok(PollReviewResult::ZeroFindings);
                }
            }

            if !any_bot_activity {
                let comments_json =
                    client.list_issue_comments(&repo_nwo, pr).map_err(|e| e.to_string())?;
                let comments = pr_review::parse_paginated_json(&comments_json)
                    .map_err(|e| format!("failed to parse comments JSON: {e}"))?;
                for comment in &comments {
                    let author = comment
                        .get("user")
                        .and_then(|u| u.get("login"))
                        .and_then(|l| l.as_str())
                        .unwrap_or("");
                    if !is_codex_bot(author) {
                        continue;
                    }
                    let created_raw =
                        comment.get("created_at").and_then(|s| s.as_str()).unwrap_or("");
                    if created_raw.is_empty() {
                        continue;
                    }
                    let created_str = created_raw.replace('Z', "+00:00");
                    let created_dt = chrono::DateTime::parse_from_rfc3339(&created_str)
                        .map_err(|e| format!("invalid comment created_at: {e}"))?;
                    if created_dt >= trigger_dt {
                        any_bot_activity = true;
                        break;
                    }
                }
            }

            let remaining = deadline.saturating_duration_since(Instant::now()).as_secs();
            eprintln!("  Waiting... ({remaining}s remaining)");
            sleep(Duration::from_secs(interval));
        }

        // Timeout recovery
        if let Some(expected_commit) = head_commit {
            let recovery_nwo = client.repo_nwo().map_err(|e| e.to_string())?;
            let recovery_reviews_json =
                client.list_reviews(&recovery_nwo, pr).map_err(|e| e.to_string())?;
            let recovery_reviews = pr_review::parse_paginated_json(&recovery_reviews_json)
                .map_err(|e| format!("recovery: failed to parse reviews JSON: {e}"))?;
            let recovery_refs: Vec<&serde_json::Value> = recovery_reviews
                .iter()
                .filter(|r| {
                    let author = r
                        .get("user")
                        .and_then(|u| u.get("login"))
                        .and_then(|l| l.as_str())
                        .unwrap_or("");
                    let state = r.get("state").and_then(|s| s.as_str()).unwrap_or("");
                    let review_commit = r.get("commit_id").and_then(|s| s.as_str()).unwrap_or("");
                    is_codex_bot(author)
                        && matches!(state, "APPROVED" | "CHANGES_REQUESTED" | "COMMENTED")
                        && review_commit == expected_commit
                })
                .collect();
            if let Some(best) = recovery_refs.iter().max_by(|a, b| {
                let ts_a = a.get("submitted_at").and_then(|s| s.as_str()).unwrap_or("");
                let ts_b = b.get("submitted_at").and_then(|s| s.as_str()).unwrap_or("");
                ts_a.cmp(ts_b).then_with(|| {
                    let id_a = a.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
                    let id_b = b.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
                    id_a.cmp(&id_b)
                })
            }) {
                eprintln!("[OK] Recovered Codex review after timeout (commit-based fallback).");
                let mut sanitized = (*best).clone();
                if let Some(obj) = sanitized.as_object_mut() {
                    if let Some(serde_json::Value::String(body)) = obj.get("body") {
                        let clean = pr_review::sanitize_text(body);
                        obj.insert("body".to_owned(), serde_json::Value::String(clean));
                    }
                }
                return Ok(PollReviewResult::ReviewFound(sanitized));
            }
        }

        if !any_bot_activity {
            eprintln!(
                "[ERROR] Timeout: No Codex bot activity detected. \
                 Ensure the Codex Cloud GitHub App is installed."
            );
        } else {
            eprintln!("[ERROR] Timeout: Codex bot active but review not yet completed.");
        }
        Ok(PollReviewResult::Timeout)
    }

    pub fn poll_review<C, Sleep>(
        pr: &str,
        trigger_timestamp: &str,
        interval: u64,
        timeout: u64,
        client: &C,
        sleep: &Sleep,
        head_commit: Option<&str>,
    ) -> Result<ExitCode, String>
    where
        C: GhClient,
        Sleep: Fn(Duration),
    {
        match poll_review_for_cycle(
            pr,
            trigger_timestamp,
            interval,
            timeout,
            client,
            sleep,
            head_commit,
        )? {
            PollReviewResult::ReviewFound(review) => {
                let review_str = serde_json::to_string(&review).unwrap_or_default();
                println!("{review_str}");
                Ok(ExitCode::SUCCESS)
            }
            PollReviewResult::ZeroFindings => {
                println!(r#"{{"verdict":"zero_findings","findings":[]}}"#);
                Ok(ExitCode::SUCCESS)
            }
            PollReviewResult::Timeout => Ok(ExitCode::FAILURE),
        }
    }
}

// Re-export test helpers at `super::` level so pr_tests.rs can access them
// with `use super::{checks_summary, CheckSummary, ...}`.
#[cfg(test)]
pub(super) use test_helpers::{
    PollReviewResult, checks_summary, ensure_pr_with, is_codex_bot, normalize_check_status,
    poll_review, poll_review_for_cycle, status_with, wait_and_merge_with,
};
#[cfg(test)]
pub(super) use usecase::pr_workflow::CheckSummary;

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
#[path = "pr_tests.rs"]
mod tests;
