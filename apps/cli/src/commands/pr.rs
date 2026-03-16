//! CLI subcommands for pull-request workflow wrappers.

use std::process::ExitCode;
use std::thread;
use std::time::{Duration, Instant};

use std::fs;
use std::path::PathBuf;

use clap::{Args, Subcommand};
use infrastructure::gh_cli::{GhClient, PrCheckRecord, SystemGhClient};
use infrastructure::git_cli::{GitRepository, SystemGitRepo};
use usecase::pr_review::{self, PrReviewFinding, PrReviewResult, sanitize_text};
use usecase::pr_workflow::{
    CheckSummary, PrBranchContext, PrCheckStatus, PrCheckView, WaitDecision, decide_wait_action,
    pr_body, pr_title, resolve_pr_branch, summarize_checks,
};

use crate::CliError;

#[derive(Debug, Subcommand)]
pub enum PrCommand {
    /// Push the current track/plan branch to origin.
    Push(PushArgs),
    /// Create or reuse a PR for the current track/plan branch.
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
    /// Explicit track ID (required on plan/ branches, ignored on track/ branches).
    #[arg(long)]
    pub track_id: Option<String>,
}

#[derive(Debug, Args)]
pub struct EnsurePrArgs {
    /// Explicit track ID (required on plan/ branches, ignored on track/ branches).
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
    /// Explicit track ID (required on plan/ branches, ignored on track/ branches).
    #[arg(long)]
    pub track_id: Option<String>,
}

pub fn execute(cmd: PrCommand) -> ExitCode {
    match cmd {
        PrCommand::Push(args) => match push(args.track_id.as_deref()) {
            Ok(code) => code,
            Err(err) => {
                eprintln!("{err}");
                err.exit_code()
            }
        },
        PrCommand::EnsurePr(args) => match ensure_pr(args.track_id.as_deref(), &args.base) {
            Ok(code) => code,
            Err(err) => {
                eprintln!("{err}");
                err.exit_code()
            }
        },
        PrCommand::Status(args) => status(&args.pr),
        PrCommand::WaitAndMerge(args) => {
            wait_and_merge(&args.pr, args.interval, args.timeout, &args.method)
        }
        PrCommand::TriggerReview(args) => match trigger_review(&args.pr, &SystemGhClient) {
            Ok(code) => code,
            Err(err) => {
                eprintln!("{err}");
                err.exit_code()
            }
        },
        PrCommand::PollReview(args) => {
            match poll_review(
                &args.pr,
                &args.trigger_timestamp,
                args.interval,
                args.timeout,
                &SystemGhClient,
                &thread::sleep,
            ) {
                Ok(code) => code,
                Err(err) => {
                    eprintln!("{err}");
                    err.exit_code()
                }
            }
        }
        PrCommand::ReviewCycle(args) => match review_cycle(args.track_id.as_deref()) {
            Ok(code) => code,
            Err(err) => {
                eprintln!("{err}");
                err.exit_code()
            }
        },
    }
}

fn resolve_branch_context(explicit_track_id: Option<&str>) -> Result<PrBranchContext, CliError> {
    let repo = SystemGitRepo::discover()?;
    let branch = repo
        .current_branch()?
        .ok_or_else(|| CliError::Message("could not determine current branch".to_owned()))?;
    resolve_pr_branch(&branch, explicit_track_id).map_err(CliError::from)
}

fn push(explicit_track_id: Option<&str>) -> Result<ExitCode, CliError> {
    let ctx = resolve_branch_context(explicit_track_id)?;
    let repo = SystemGitRepo::discover()?;
    println!("Pushing {} to origin...", ctx.branch);
    repo.push_branch(&ctx.branch)?;
    println!("[OK] Pushed {}", ctx.branch);
    Ok(ExitCode::SUCCESS)
}

fn ensure_pr(explicit_track_id: Option<&str>, base: &str) -> Result<ExitCode, CliError> {
    let ctx = resolve_branch_context(explicit_track_id)?;
    let client = SystemGhClient;
    Ok(ensure_pr_with(&ctx, base, &client))
}

fn ensure_pr_with<C: GhClient>(ctx: &PrBranchContext, base: &str, client: &C) -> ExitCode {
    // Check for existing PR
    match client.find_open_pr(&ctx.branch, base) {
        Ok(Some(pr)) => {
            println!("[OK] Reusing existing PR #{pr}");
            return ExitCode::SUCCESS;
        }
        Ok(None) => {} // create new
        Err(err) => {
            eprintln!("[ERROR] {err}");
            return ExitCode::FAILURE;
        }
    }

    // Write body to a uniquely-named temp file to avoid races.
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
            // Clean up body file
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

fn normalize_check_status(check: &PrCheckRecord) -> PrCheckStatus {
    let state = if !check.bucket.is_empty() { check.bucket.as_str() } else { check.state.as_str() };

    match state.to_uppercase().as_str() {
        "SUCCESS" | "PASS" | "SKIPPING" => PrCheckStatus::Passed,
        "FAILURE" | "FAIL" | "CANCEL" => PrCheckStatus::Failed,
        _ => PrCheckStatus::Pending,
    }
}

fn checks_summary(checks: &[PrCheckRecord]) -> CheckSummary {
    let checks = checks
        .iter()
        .map(|check| PrCheckView {
            name: check.name.clone(),
            status: normalize_check_status(check),
        })
        .collect::<Vec<_>>();
    summarize_checks(&checks)
}

fn status(pr: &str) -> ExitCode {
    let client = SystemGhClient;
    status_with(pr, &client)
}

fn status_with<C>(pr: &str, client: &C) -> ExitCode
where
    C: GhClient,
{
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

fn merge_pr_with<C>(pr: &str, method: &str, client: &C) -> ExitCode
where
    C: GhClient,
{
    println!("[OK] All checks passed. Merging...");
    match client.merge_pr(pr, method) {
        Ok(()) => {
            println!("[OK] PR #{pr} merged ({method}).");
            ExitCode::SUCCESS
        }
        Err(err) => {
            println!("[ERROR] Merge failed: {err}");
            ExitCode::FAILURE
        }
    }
}

fn wait_and_merge_with<C, Sleep>(
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
            WaitDecision::MergeNow => return merge_pr_with(pr, method, client),
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

fn wait_and_merge(pr: &str, interval: u64, timeout: u64, method: &str) -> ExitCode {
    let client = SystemGhClient;
    wait_and_merge_with(pr, interval, timeout, method, &client, &thread::sleep)
}

// ---------------------------------------------------------------------------
// T004: trigger-review
// ---------------------------------------------------------------------------

/// Known Codex bot login names (case-insensitive comparison).
///
/// GitHub App bots use the `<app-slug>[bot]` login convention.
/// We match against exact known login names to prevent unrelated GitHub Apps
/// (e.g., `evil-codex-helper[bot]`) from being treated as the trusted reviewer.
const CODEX_BOT_LOGINS: &[&str] = &["openai-codex[bot]", "codex[bot]"];

fn is_codex_bot(login: &str) -> bool {
    let lower = login.to_lowercase();
    CODEX_BOT_LOGINS.iter().any(|known| *known == lower)
}

fn trigger_review<C: GhClient>(pr: &str, client: &C) -> Result<ExitCode, CliError> {
    // Fail-closed: validate reviewer provider (resolve from repo root)
    let git_repo = SystemGitRepo::discover()?;
    let profiles_path = git_repo.root().join(".claude/agent-profiles.json");
    pr_review::resolve_reviewer_provider(&profiles_path)?;

    let repo = client.repo_nwo()?;
    let response = client.post_issue_comment(&repo, pr, "@codex review")?;

    // Extract server-side created_at from the response JSON (fail-closed).
    let created_at = serde_json::from_str::<serde_json::Value>(&response)
        .ok()
        .and_then(|v| v.get("created_at")?.as_str().map(String::from))
        .unwrap_or_default();

    if created_at.is_empty() {
        return Err(CliError::Message(
            "could not determine trigger timestamp from API response".to_owned(),
        ));
    }
    println!("[OK] Posted '@codex review' on PR #{pr} at {created_at}");
    println!("TRIGGER_TIMESTAMP={created_at}");
    Ok(ExitCode::SUCCESS)
}

// ---------------------------------------------------------------------------
// T005: poll-review
// ---------------------------------------------------------------------------

fn poll_review<C, Sleep>(
    pr: &str,
    trigger_timestamp: &str,
    interval: u64,
    timeout: u64,
    client: &C,
    sleep: &Sleep,
) -> Result<ExitCode, CliError>
where
    C: GhClient,
    Sleep: Fn(Duration),
{
    let trigger_time = trigger_timestamp.replace('Z', "+00:00");
    let trigger_dt = chrono::DateTime::parse_from_rfc3339(&trigger_time)
        .map_err(|e| CliError::Message(format!("invalid trigger timestamp: {e}")))?;

    let repo = client.repo_nwo()?;
    // Cap timeout to prevent Instant overflow panic on extremely large values.
    let deadline = Instant::now() + Duration::from_secs(timeout.min(86400));
    let mut any_bot_activity = false;

    eprintln!("Polling for Codex review on PR #{pr} (interval={interval}s, timeout={timeout}s)...");

    loop {
        if Instant::now() >= deadline {
            break;
        }

        // Fetch reviews — propagate API errors (fail-closed)
        let reviews_json = client.list_reviews(&repo, pr)?;
        let reviews = pr_review::parse_paginated_json(&reviews_json)
            .map_err(|e| CliError::Message(format!("failed to parse reviews JSON: {e}")))?;
        for review in &reviews {
            let author = review
                .get("user")
                .and_then(|u| u.get("login"))
                .and_then(|l| l.as_str())
                .unwrap_or("");
            if !is_codex_bot(author) {
                continue;
            }
            let submitted_raw = review.get("submitted_at").and_then(|s| s.as_str()).unwrap_or("");
            if submitted_raw.is_empty() {
                // PENDING review (no submitted_at) — cannot tie to this trigger,
                // so skip without marking bot activity to avoid suppressing the
                // issue-comment fallback check.
                continue;
            }
            let submitted_str = submitted_raw.replace('Z', "+00:00");
            let submitted_dt = chrono::DateTime::parse_from_rfc3339(&submitted_str)
                .map_err(|e| CliError::Message(format!("invalid review submitted_at: {e}")))?;
            if submitted_dt >= trigger_dt {
                // Post-trigger review — record bot activity.
                any_bot_activity = true;
                let state = review.get("state").and_then(|s| s.as_str()).unwrap_or("");
                if matches!(state, "APPROVED" | "CHANGES_REQUESTED" | "COMMENTED") {
                    let review_id = review.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
                    eprintln!("[OK] Found Codex review (id={review_id}, state={state})");
                    // Sanitize review body before stdout output to prevent
                    // leaking absolute paths, secrets, or internal URLs.
                    let mut sanitized = review.clone();
                    if let Some(obj) = sanitized.as_object_mut() {
                        if let Some(serde_json::Value::String(body)) = obj.get("body") {
                            let clean = sanitize_text(body);
                            obj.insert("body".to_owned(), serde_json::Value::String(clean));
                        }
                    }
                    let review_str = serde_json::to_string(&sanitized).unwrap_or_default();
                    println!("{review_str}");
                    return Ok(ExitCode::SUCCESS);
                }
            }
        }

        // Check comments for bot activity — propagate API errors (fail-closed)
        if !any_bot_activity {
            let comments_json = client.list_issue_comments(&repo, pr)?;
            let comments = pr_review::parse_paginated_json(&comments_json)
                .map_err(|e| CliError::Message(format!("failed to parse comments JSON: {e}")))?;
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
                    .map_err(|e| CliError::Message(format!("invalid comment created_at: {e}")))?;
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

    // Timeout — distinguish cause
    if !any_bot_activity {
        eprintln!(
            "[ERROR] Timeout: No Codex bot activity detected on this PR. \
             Ensure the Codex Cloud GitHub App is installed on this repository. \
             See: https://github.com/apps/openai-codex"
        );
    } else {
        eprintln!(
            "[ERROR] Timeout: Codex bot is active but review not yet completed. \
             The review may still be in progress. Try again later or increase timeout."
        );
    }
    Ok(ExitCode::FAILURE)
}

// ---------------------------------------------------------------------------
// T006: review-cycle
// ---------------------------------------------------------------------------

fn review_cycle(explicit_track_id: Option<&str>) -> Result<ExitCode, CliError> {
    // Fail-closed: must be on a track/ branch (not plan/)
    let repo = SystemGitRepo::discover()?;

    // Fail-closed: validate reviewer provider (resolve from repo root)
    let profiles_path = repo.root().join(".claude/agent-profiles.json");
    pr_review::resolve_reviewer_provider(&profiles_path)?;
    let branch = repo
        .current_branch()?
        .ok_or_else(|| CliError::Message("could not determine current branch".to_owned()))?;
    if !branch.starts_with("track/") {
        return Err(CliError::Message(
            "not on a track branch (expected track/<id>). \
             For planning-only tracks, run /track:activate <track-id> first."
                .to_owned(),
        ));
    }

    let client = SystemGhClient;

    // Step 1: Push
    let ctx = resolve_branch_context(explicit_track_id)?;
    println!("Pushing {} to origin...", ctx.branch);
    repo.push_branch(&ctx.branch)?;
    println!("[OK] Pushed {}", ctx.branch);

    // Step 2: Ensure PR
    let pr_number = match ensure_pr_for_cycle(&ctx, "main", &client)? {
        Some(pr) => pr,
        None => return Ok(ExitCode::FAILURE),
    };

    // Step 3: Trigger review
    let nwo = client.repo_nwo()?;
    let response = client.post_issue_comment(&nwo, &pr_number, "@codex review")?;
    let trigger_timestamp = serde_json::from_str::<serde_json::Value>(&response)
        .ok()
        .and_then(|v| v.get("created_at")?.as_str().map(String::from))
        .unwrap_or_default();
    println!("[OK] Posted '@codex review' on PR #{pr_number} at {trigger_timestamp}");

    if trigger_timestamp.is_empty() {
        return Err(CliError::Message(
            "could not determine trigger timestamp from API response".to_owned(),
        ));
    }

    // Step 4: Poll for review
    let review_json =
        poll_review_for_cycle(&pr_number, &trigger_timestamp, 15, 600, &client, &thread::sleep)?;
    let Some(review) = review_json else {
        return Ok(ExitCode::FAILURE);
    };

    // Step 5: Parse review
    let parsed = parse_review(&pr_number, &review, &nwo, &client)?;

    // Step 6: Report
    print_review_summary(&pr_number, &parsed);

    if parsed.passed { Ok(ExitCode::SUCCESS) } else { Ok(ExitCode::FAILURE) }
}

/// Ensure PR exists for review cycle, returning the PR number.
fn ensure_pr_for_cycle<C: GhClient>(
    ctx: &PrBranchContext,
    base: &str,
    client: &C,
) -> Result<Option<String>, CliError> {
    match client.find_open_pr(&ctx.branch, base) {
        Ok(Some(pr)) => {
            println!("[OK] Reusing existing PR #{pr}");
            return Ok(Some(pr));
        }
        Ok(None) => {}
        Err(err) => {
            eprintln!("[ERROR] {err}");
            return Ok(None);
        }
    }

    let body_dir = PathBuf::from("tmp");
    fs::create_dir_all(&body_dir)
        .map_err(|e| CliError::Message(format!("failed to create tmp dir: {e}")))?;
    // Verify tmp/ is a real directory, not a symlink (prevents directory traversal).
    let meta = fs::symlink_metadata(&body_dir)
        .map_err(|e| CliError::Message(format!("failed to stat tmp dir: {e}")))?;
    if meta.file_type().is_symlink() {
        return Err(CliError::Message("tmp/ is a symlink — refusing to write PR body".to_owned()));
    }
    let body_file = body_dir.join(format!("pr-body-{}.md", std::process::id()));
    // Remove any pre-existing file/symlink to prevent symlink-following attacks,
    // then create exclusively (O_CREAT | O_EXCL via create_new).
    let _ = fs::remove_file(&body_file);
    let body_text = pr_body(ctx);
    {
        use std::io::Write;
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&body_file)
            .map_err(|e| CliError::Message(format!("failed to create PR body file: {e}")))?;
        f.write_all(body_text.as_bytes())
            .map_err(|e| CliError::Message(format!("failed to write PR body file: {e}")))?;
    }

    let title = pr_title(ctx);
    match client.create_pr(&ctx.branch, base, &title, &body_file) {
        Ok(pr) => {
            let _ = fs::remove_file(&body_file);
            println!("[OK] Created PR #{pr}");
            Ok(Some(pr))
        }
        Err(err) => {
            let _ = fs::remove_file(&body_file);
            eprintln!("[ERROR] {err}");
            Ok(None)
        }
    }
}

/// Poll for review in cycle context, returning the review JSON value.
fn poll_review_for_cycle<C, Sleep>(
    pr: &str,
    trigger_timestamp: &str,
    interval: u64,
    timeout: u64,
    client: &C,
    sleep: &Sleep,
) -> Result<Option<serde_json::Value>, CliError>
where
    C: GhClient,
    Sleep: Fn(Duration),
{
    let trigger_time = trigger_timestamp.replace('Z', "+00:00");
    let trigger_dt = chrono::DateTime::parse_from_rfc3339(&trigger_time)
        .map_err(|e| CliError::Message(format!("invalid trigger timestamp: {e}")))?;

    let repo_nwo = client.repo_nwo()?;
    // Cap timeout to prevent Instant overflow panic on extremely large values.
    let deadline = Instant::now() + Duration::from_secs(timeout.min(86400));
    let mut any_bot_activity = false;

    eprintln!("Polling for Codex review on PR #{pr} (interval={interval}s, timeout={timeout}s)...");

    loop {
        if Instant::now() >= deadline {
            break;
        }

        // Fetch reviews — propagate API errors (fail-closed)
        let reviews_json = client.list_reviews(&repo_nwo, pr)?;
        let reviews = pr_review::parse_paginated_json(&reviews_json)
            .map_err(|e| CliError::Message(format!("failed to parse reviews JSON: {e}")))?;
        for review in &reviews {
            let author = review
                .get("user")
                .and_then(|u| u.get("login"))
                .and_then(|l| l.as_str())
                .unwrap_or("");
            if !is_codex_bot(author) {
                continue;
            }
            let submitted_raw = review.get("submitted_at").and_then(|s| s.as_str()).unwrap_or("");
            if submitted_raw.is_empty() {
                // PENDING review (no submitted_at) — cannot tie to this trigger,
                // so skip without marking bot activity to avoid suppressing the
                // issue-comment fallback check.
                continue;
            }
            let submitted_str = submitted_raw.replace('Z', "+00:00");
            let submitted_dt = chrono::DateTime::parse_from_rfc3339(&submitted_str)
                .map_err(|e| CliError::Message(format!("invalid review submitted_at: {e}")))?;
            if submitted_dt >= trigger_dt {
                // Post-trigger review — record bot activity.
                any_bot_activity = true;
                let state = review.get("state").and_then(|s| s.as_str()).unwrap_or("");
                if matches!(state, "APPROVED" | "CHANGES_REQUESTED" | "COMMENTED") {
                    let review_id = review.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
                    eprintln!("[OK] Found Codex review (id={review_id}, state={state})");
                    // Sanitize review body before returning to prevent leaking
                    // absolute paths, secrets, or internal URLs.
                    let mut sanitized = review.clone();
                    if let Some(obj) = sanitized.as_object_mut() {
                        if let Some(serde_json::Value::String(body)) = obj.get("body") {
                            let clean = sanitize_text(body);
                            obj.insert("body".to_owned(), serde_json::Value::String(clean));
                        }
                    }
                    return Ok(Some(sanitized));
                }
            }
        }

        // Check comments for bot activity — propagate API errors (fail-closed)
        if !any_bot_activity {
            let comments_json = client.list_issue_comments(&repo_nwo, pr)?;
            let comments = pr_review::parse_paginated_json(&comments_json)
                .map_err(|e| CliError::Message(format!("failed to parse comments JSON: {e}")))?;
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
                    .map_err(|e| CliError::Message(format!("invalid comment created_at: {e}")))?;
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

    if !any_bot_activity {
        eprintln!(
            "[ERROR] Timeout: No Codex bot activity detected. \
             Ensure the Codex Cloud GitHub App is installed."
        );
    } else {
        eprintln!("[ERROR] Timeout: Codex bot active but review not yet completed.");
    }
    Ok(None)
}

/// Parse a review JSON into a normalized PrReviewResult.
fn parse_review<C: GhClient>(
    pr: &str,
    review: &serde_json::Value,
    repo_nwo: &str,
    client: &C,
) -> Result<PrReviewResult, CliError> {
    let review_id = review.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
    let state = review.get("state").and_then(|s| s.as_str()).unwrap_or("COMMENTED").to_owned();
    let raw_body = review.get("body").and_then(|s| s.as_str()).unwrap_or("");
    let body = sanitize_text(raw_body);

    // Fetch inline comments for this review
    let mut findings: Vec<PrReviewFinding> = Vec::new();
    let mut inline_count: u32 = 0;

    // Fetch inline comments — propagate API errors (fail-closed)
    let review_id_str = review_id.to_string();
    let comments_json = client.list_review_comments(repo_nwo, pr, &review_id_str)?;
    let comments = pr_review::parse_paginated_json(&comments_json)
        .map_err(|e| CliError::Message(format!("failed to parse comments JSON: {e}")))?;
    for comment in &comments {
        inline_count += 1;
        let comment_body =
            sanitize_text(comment.get("body").and_then(|s| s.as_str()).unwrap_or(""));
        let path = comment.get("path").and_then(|s| s.as_str()).unwrap_or("").to_owned();
        // GitHub API: start_line = first line, line = last line
        let start = comment
            .get("start_line")
            .and_then(|v| v.as_u64())
            .or_else(|| comment.get("original_start_line").and_then(|v| v.as_u64()));
        let end = comment
            .get("line")
            .and_then(|v| v.as_u64())
            .or_else(|| comment.get("original_line").and_then(|v| v.as_u64()));
        let line = start.or(end).map(|v| v as u32);
        let end_line = end.map(|v| v as u32);

        let severity = pr_review::classify_severity(&comment_body);
        findings.push(PrReviewFinding {
            severity: severity.to_owned(),
            path,
            line,
            end_line,
            body: comment_body,
            rule_id: None,
        });
    }

    // Parse findings from review body
    if !body.is_empty() {
        let body_findings = pr_review::parse_body_findings(&body);
        findings.extend(body_findings);
    }

    let actionable =
        findings.iter().filter(|f| f.severity == "P0" || f.severity == "P1").count() as u32;
    // APPROVED reviews pass even with inline nits — the reviewer explicitly approved.
    // CHANGES_REQUESTED always fails. COMMENTED uses actionable count.
    let passed = state == "APPROVED" || (actionable == 0 && state != "CHANGES_REQUESTED");

    Ok(PrReviewResult {
        review_id,
        state,
        body,
        findings,
        inline_comment_count: inline_count,
        actionable_count: actionable,
        passed,
    })
}

fn print_review_summary(pr: &str, result: &PrReviewResult) {
    let status = if result.passed { "PASS" } else { "FAIL" };
    println!();
    println!("=== PR Review Result: {status} ===");
    println!("PR: #{pr}");
    println!("Review ID: {}", result.review_id);
    println!("State: {}", result.state);
    println!("Inline comments: {}", result.inline_comment_count);
    println!("Total findings: {}", result.findings.len());
    println!("Actionable (P0/P1): {}", result.actionable_count);

    if !result.findings.is_empty() {
        println!();
        println!("Findings:");
        for (i, f) in result.findings.iter().enumerate() {
            let location = if !f.path.is_empty() && f.line.is_some() {
                format!("{}:{}", f.path, f.line.unwrap_or(0))
            } else if !f.path.is_empty() {
                f.path.clone()
            } else {
                "general".to_owned()
            };
            let truncated_body: String = f.body.chars().take(120).collect();
            println!("  {}. [{}] {}: {}", i + 1, f.severity, location, truncated_body);
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use std::cell::RefCell;
    use std::process::ExitCode;

    use std::path::Path;

    use super::{
        CheckSummary, checks_summary, ensure_pr_with, normalize_check_status, status_with,
        wait_and_merge_with,
    };
    use infrastructure::gh_cli::{GhClient, GhError, PrCheckRecord};
    use usecase::pr_workflow::{PrBranchContext, PrCheckStatus};

    struct FakeGhClient {
        checks: RefCell<Vec<Result<Vec<PrCheckRecord>, GhError>>>,
        url: String,
        merge_calls: RefCell<Vec<(String, String)>>,
        merge_result: RefCell<Option<String>>,
        find_pr_result: RefCell<Result<Option<String>, String>>,
        create_pr_result: RefCell<Result<String, String>>,
    }

    impl FakeGhClient {
        fn default_for_pr() -> Self {
            Self {
                checks: RefCell::new(Vec::new()),
                url: String::new(),
                merge_calls: RefCell::new(Vec::new()),
                merge_result: RefCell::new(None),
                find_pr_result: RefCell::new(Ok(None)),
                create_pr_result: RefCell::new(Ok("42".to_owned())),
            }
        }
    }

    impl GhClient for FakeGhClient {
        fn pr_checks(&self, _pr: &str) -> Result<Vec<PrCheckRecord>, GhError> {
            self.checks.borrow_mut().remove(0)
        }

        fn pr_url(&self, pr: &str) -> String {
            if self.url.is_empty() { format!("PR #{pr}") } else { self.url.clone() }
        }

        fn merge_pr(&self, pr: &str, method: &str) -> Result<(), GhError> {
            self.merge_calls.borrow_mut().push((pr.to_owned(), method.to_owned()));
            match self.merge_result.borrow().as_deref() {
                None => Ok(()),
                Some(stderr) => Err(GhError::CommandFailed {
                    command: format!("pr merge {pr} --{method}"),
                    stderr: stderr.to_owned(),
                }),
            }
        }

        fn find_open_pr(&self, _head: &str, _base: &str) -> Result<Option<String>, GhError> {
            self.find_pr_result
                .borrow()
                .clone()
                .map_err(|stderr| GhError::CommandFailed { command: "pr list".to_owned(), stderr })
        }

        fn create_pr(
            &self,
            _head: &str,
            _base: &str,
            _title: &str,
            _body_file: &Path,
        ) -> Result<String, GhError> {
            self.create_pr_result.borrow().clone().map_err(|stderr| GhError::CommandFailed {
                command: "pr create".to_owned(),
                stderr,
            })
        }

        fn post_issue_comment(
            &self,
            _repo_nwo: &str,
            _pr: &str,
            _body: &str,
        ) -> Result<String, GhError> {
            Ok("{}".to_owned())
        }

        fn list_reviews(&self, _repo_nwo: &str, _pr: &str) -> Result<String, GhError> {
            Ok("[]".to_owned())
        }

        fn list_issue_comments(&self, _repo_nwo: &str, _pr: &str) -> Result<String, GhError> {
            Ok("[]".to_owned())
        }

        fn list_review_comments(
            &self,
            _repo_nwo: &str,
            _pr: &str,
            _review_id: &str,
        ) -> Result<String, GhError> {
            Ok("[]".to_owned())
        }

        fn repo_nwo(&self) -> Result<String, GhError> {
            Ok("owner/repo".to_owned())
        }
    }

    #[test]
    fn normalize_check_status_prefers_bucket_over_state() {
        let check = PrCheckRecord {
            name: "ci".to_owned(),
            state: "SUCCESS".to_owned(),
            bucket: "pending".to_owned(),
        };

        assert_eq!(normalize_check_status(&check), PrCheckStatus::Pending);
    }

    #[test]
    fn status_with_propagates_adapter_errors() {
        let client = FakeGhClient {
            checks: RefCell::new(vec![Err(GhError::CommandFailed {
                command: "pr checks 123".to_owned(),
                stderr: "gh exploded".to_owned(),
            })]),
            ..FakeGhClient::default_for_pr()
        };

        assert_eq!(status_with("123", &client), ExitCode::FAILURE);
    }

    #[test]
    fn checks_summary_maps_normalized_cli_checks_to_pending() {
        let checks = vec![PrCheckRecord {
            name: "ci".to_owned(),
            state: "SUCCESS".to_owned(),
            bucket: "pending".to_owned(),
        }];

        assert_eq!(checks_summary(&checks), CheckSummary::Pending(vec!["ci".to_owned()]));
    }

    #[test]
    fn wait_and_merge_with_merges_after_all_checks_pass() {
        let client = FakeGhClient {
            checks: RefCell::new(vec![Ok(vec![PrCheckRecord {
                name: "ci".to_owned(),
                state: "SUCCESS".to_owned(),
                bucket: String::new(),
            }])]),
            url: "https://example.invalid/pr/123".to_owned(),
            ..FakeGhClient::default_for_pr()
        };
        let result = wait_and_merge_with("123", 15, 600, "squash", &client, &|_| {
            panic!("sleep should not be called")
        });

        assert_eq!(result, ExitCode::SUCCESS);
        assert_eq!(client.merge_calls.into_inner(), vec![("123".to_owned(), "squash".to_owned())]);
    }

    #[test]
    fn wait_and_merge_with_returns_failure_when_checks_api_errors() {
        let client = FakeGhClient {
            checks: RefCell::new(vec![Err(GhError::CommandFailed {
                command: "pr checks 123".to_owned(),
                stderr: "boom".to_owned(),
            })]),
            ..FakeGhClient::default_for_pr()
        };
        let result = wait_and_merge_with("123", 15, 600, "merge", &client, &|_| {
            panic!("sleep should not be called")
        });

        assert_eq!(result, ExitCode::FAILURE);
    }

    #[test]
    fn wait_and_merge_with_times_out_pending_checks_without_sleep_when_deadline_reached() {
        let sleeps = RefCell::new(Vec::new());
        let client = FakeGhClient {
            checks: RefCell::new(vec![Ok(vec![PrCheckRecord {
                name: "ci".to_owned(),
                state: "PENDING".to_owned(),
                bucket: "pending".to_owned(),
            }])]),
            ..FakeGhClient::default_for_pr()
        };
        let result = wait_and_merge_with("123", 15, 0, "merge", &client, &|duration| {
            sleeps.borrow_mut().push(duration)
        });

        assert_eq!(result, ExitCode::FAILURE);
        assert!(sleeps.borrow().is_empty());
    }

    // --- ensure_pr_with tests ---

    #[test]
    fn ensure_pr_with_reuses_existing_pr() {
        let client = FakeGhClient {
            find_pr_result: RefCell::new(Ok(Some("99".to_owned()))),
            ..FakeGhClient::default_for_pr()
        };
        let ctx = PrBranchContext {
            branch: "track/my-feature".to_owned(),
            track_id: "my-feature".to_owned(),
            is_plan_branch: false,
        };
        assert_eq!(ensure_pr_with(&ctx, "main", &client), ExitCode::SUCCESS);
    }

    #[test]
    fn ensure_pr_with_creates_new_pr_when_none_exists() {
        let client = FakeGhClient {
            find_pr_result: RefCell::new(Ok(None)),
            create_pr_result: RefCell::new(Ok("42".to_owned())),
            ..FakeGhClient::default_for_pr()
        };
        let ctx = PrBranchContext {
            branch: "track/my-feature".to_owned(),
            track_id: "my-feature".to_owned(),
            is_plan_branch: false,
        };
        assert_eq!(ensure_pr_with(&ctx, "main", &client), ExitCode::SUCCESS);
    }

    #[test]
    fn ensure_pr_with_returns_failure_on_find_error() {
        let client = FakeGhClient {
            find_pr_result: RefCell::new(Err("gh exploded".to_owned())),
            ..FakeGhClient::default_for_pr()
        };
        let ctx = PrBranchContext {
            branch: "track/my-feature".to_owned(),
            track_id: "my-feature".to_owned(),
            is_plan_branch: false,
        };
        assert_eq!(ensure_pr_with(&ctx, "main", &client), ExitCode::FAILURE);
    }

    #[test]
    fn ensure_pr_with_returns_failure_on_create_error() {
        let client = FakeGhClient {
            find_pr_result: RefCell::new(Ok(None)),
            create_pr_result: RefCell::new(Err("create failed".to_owned())),
            ..FakeGhClient::default_for_pr()
        };
        let ctx = PrBranchContext {
            branch: "track/my-feature".to_owned(),
            track_id: "my-feature".to_owned(),
            is_plan_branch: false,
        };
        assert_eq!(ensure_pr_with(&ctx, "main", &client), ExitCode::FAILURE);
    }

    // --- is_codex_bot tests ---

    #[test]
    fn is_codex_bot_matches_known_bot_logins() {
        assert!(super::is_codex_bot("openai-codex[bot]"));
        assert!(super::is_codex_bot("OpenAI-Codex[bot]"));
        assert!(super::is_codex_bot("codex[bot]"));
    }

    #[test]
    fn is_codex_bot_rejects_human_with_codex_in_name() {
        assert!(!super::is_codex_bot("codex-user"));
        assert!(!super::is_codex_bot("my-codex-tool"));
    }

    #[test]
    fn is_codex_bot_rejects_unknown_codex_app() {
        // Unrelated GitHub App with "codex" in name should not match
        assert!(!super::is_codex_bot("evil-codex-helper[bot]"));
    }

    #[test]
    fn is_codex_bot_rejects_non_codex_bot() {
        assert!(!super::is_codex_bot("dependabot[bot]"));
        assert!(!super::is_codex_bot("github-actions[bot]"));
    }

    // --- poll_review tests ---

    // --- Minimal GhClient for poll_review tests ---

    /// Minimal GhClient stub for poll_review tests.
    /// Only implements methods needed by the polling logic.
    struct PollTestClient {
        reviews: String,
        comments: String,
    }

    impl PollTestClient {
        fn with_reviews(reviews: &str) -> Self {
            Self { reviews: reviews.to_owned(), comments: "[]".to_owned() }
        }
    }

    impl GhClient for PollTestClient {
        fn pr_checks(&self, _pr: &str) -> Result<Vec<PrCheckRecord>, GhError> {
            Ok(Vec::new())
        }

        fn pr_url(&self, pr: &str) -> String {
            format!("PR #{pr}")
        }

        fn merge_pr(&self, _pr: &str, _method: &str) -> Result<(), GhError> {
            Ok(())
        }

        fn find_open_pr(&self, _head: &str, _base: &str) -> Result<Option<String>, GhError> {
            Ok(None)
        }

        fn create_pr(
            &self,
            _head: &str,
            _base: &str,
            _title: &str,
            _body_file: &Path,
        ) -> Result<String, GhError> {
            Ok("1".to_owned())
        }

        fn list_reviews(&self, _nwo: &str, _pr: &str) -> Result<String, GhError> {
            Ok(self.reviews.clone())
        }

        fn list_issue_comments(&self, _nwo: &str, _pr: &str) -> Result<String, GhError> {
            Ok(self.comments.clone())
        }

        fn repo_nwo(&self) -> Result<String, GhError> {
            Ok("owner/repo".to_owned())
        }
    }

    // --- poll_review tests ---

    #[test]
    fn poll_review_finds_post_trigger_codex_review() {
        let client = PollTestClient::with_reviews(
            r#"[{
                "id": 1,
                "user": {"login": "openai-codex[bot]"},
                "submitted_at": "2026-03-16T10:00:00Z",
                "state": "APPROVED",
                "body": "LGTM"
            }]"#,
        );
        // Use timeout=60 so at least one poll iteration runs; sleep is a no-op.
        let result = super::poll_review("1", "2026-03-16T09:00:00Z", 15, 60, &client, &|_| {});
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), ExitCode::SUCCESS);
    }

    #[test]
    fn poll_review_ignores_pre_trigger_review() {
        let client = PollTestClient::with_reviews(
            r#"[{
                "id": 1,
                "user": {"login": "openai-codex[bot]"},
                "submitted_at": "2026-03-16T08:00:00Z",
                "state": "APPROVED",
                "body": "old review"
            }]"#,
        );
        let result = super::poll_review("1", "2026-03-16T09:00:00Z", 15, 0, &client, &|_| {});
        // Should timeout (FAILURE) because the only review is pre-trigger
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), ExitCode::FAILURE);
    }

    #[test]
    fn poll_review_stale_review_does_not_suppress_comment_check() {
        // Old (pre-trigger) review should NOT set any_bot_activity, so the
        // comment check path should still run and timeout as "no bot activity".
        let client = PollTestClient::with_reviews(
            r#"[{
                "id": 1,
                "user": {"login": "openai-codex[bot]"},
                "submitted_at": "2026-03-16T08:00:00Z",
                "state": "APPROVED",
                "body": "old review"
            }]"#,
        );
        let result = super::poll_review("1", "2026-03-16T09:00:00Z", 15, 0, &client, &|_| {});
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), ExitCode::FAILURE);
    }

    #[test]
    fn poll_review_sanitizes_review_body_on_stdout() {
        let client = PollTestClient::with_reviews(
            r#"[{
                "id": 1,
                "user": {"login": "openai-codex[bot]"},
                "submitted_at": "2026-03-16T10:00:00Z",
                "state": "APPROVED",
                "body": "Found issue at /home/user/project/src/main.rs"
            }]"#,
        );
        // The function should succeed (found a post-trigger APPROVED review)
        let result = super::poll_review("1", "2026-03-16T09:00:00Z", 15, 60, &client, &|_| {});
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), ExitCode::SUCCESS);
    }
}
