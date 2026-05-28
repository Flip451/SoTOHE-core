//! Private polling and review helpers for the `pr` command family.
//!
//! All items in this module are `pub(super)` — they are implementation details
//! of `apps/cli-composition/src/pr.rs` and must not appear on the public facade.

use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// Known Codex bot login names (case-insensitive comparison).
// ---------------------------------------------------------------------------

pub(super) const CODEX_BOT_LOGINS: &[&str] =
    &["openai-codex[bot]", "codex[bot]", "chatgpt-codex-connector[bot]"];

pub(super) fn is_codex_bot(login: &str) -> bool {
    let lower = login.to_lowercase();
    CODEX_BOT_LOGINS.iter().any(|known| *known == lower)
}

// ---------------------------------------------------------------------------
// Outcome of a poll-review cycle
// ---------------------------------------------------------------------------

pub(super) enum PollReviewResult {
    ReviewFound(serde_json::Value),
    ZeroFindings,
    Timeout,
}

// ---------------------------------------------------------------------------
// Trigger state (persisted to tmp/pr-review-state/<track-id>.json)
// ---------------------------------------------------------------------------

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub(super) struct TriggerState {
    pub(super) pr_number: String,
    pub(super) trigger_timestamp: String,
    pub(super) head_hash: Option<String>,
    pub(super) track_id: String,
}

pub(super) fn trigger_state_path(track_id: &str) -> PathBuf {
    use infrastructure::git_cli::{GitRepository as _, SystemGitRepo};
    let root = SystemGitRepo::discover().map(|r| r.root().to_path_buf()).unwrap_or_default();
    root.join("tmp/pr-review-state").join(format!("{track_id}.json"))
}

pub(super) fn save_trigger_state(state: &TriggerState) -> Result<(), String> {
    let path = trigger_state_path(&state.track_id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create dir {}: {e}", parent.display()))?;
    }
    let json = serde_json::to_string_pretty(state)
        .map_err(|e| format!("failed to serialize trigger state: {e}"))?;
    fs::write(&path, json).map_err(|e| format!("failed to write {}: {e}", path.display()))?;
    println!("[OK] Saved trigger state to {}", path.display());
    Ok(())
}

pub(super) fn load_trigger_state(track_id: &str) -> Result<Option<TriggerState>, String> {
    let path = trigger_state_path(track_id);
    if !path.exists() {
        return Ok(None);
    }
    let json =
        fs::read_to_string(&path).map_err(|e| format!("failed to read {}: {e}", path.display()))?;
    let state: TriggerState =
        serde_json::from_str(&json).map_err(|e| format!("failed to parse trigger state: {e}"))?;
    Ok(Some(state))
}

pub(super) fn cleanup_trigger_state(track_id: &str) {
    let path = trigger_state_path(track_id);
    let _ = fs::remove_file(path);
}

// ---------------------------------------------------------------------------
// PR context helpers
// ---------------------------------------------------------------------------

pub(super) fn resolve_branch_context(
    explicit_track_id: Option<&str>,
) -> Result<usecase::pr_workflow::PrBranchContext, String> {
    use infrastructure::git_cli::{GitRepository as _, SystemGitRepo};
    let repo = SystemGitRepo::discover().map_err(|e| e.to_string())?;
    let branch = repo
        .current_branch()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "could not determine current branch".to_owned())?;
    usecase::pr_workflow::resolve_pr_branch(&branch, explicit_track_id).map_err(|e| e.to_string())
}

pub(super) fn normalize_check_status(
    check: &infrastructure::gh_cli::PrCheckRecord,
) -> usecase::pr_workflow::PrCheckStatus {
    use usecase::pr_workflow::PrCheckStatus;
    let state = if !check.bucket.is_empty() { check.bucket.as_str() } else { check.state.as_str() };
    match state.to_uppercase().as_str() {
        "SUCCESS" | "PASS" | "SKIPPING" => PrCheckStatus::Passed,
        "FAILURE" | "FAIL" | "CANCEL" => PrCheckStatus::Failed,
        _ => PrCheckStatus::Pending,
    }
}

pub(super) fn checks_summary(
    checks: &[infrastructure::gh_cli::PrCheckRecord],
) -> usecase::pr_workflow::CheckSummary {
    use usecase::pr_workflow::{PrCheckView, summarize_checks};
    let views = checks
        .iter()
        .map(|c| PrCheckView { name: c.name.clone(), status: normalize_check_status(c) })
        .collect::<Vec<_>>();
    summarize_checks(&views)
}

// ---------------------------------------------------------------------------
// PR body helpers
// ---------------------------------------------------------------------------

pub(super) fn ensure_pr_body_file(
    ctx: &usecase::pr_workflow::PrBranchContext,
) -> Result<PathBuf, String> {
    use std::io::Write as _;
    use usecase::pr_workflow::pr_body;

    let body_dir = PathBuf::from("tmp");
    fs::create_dir_all(&body_dir).map_err(|e| format!("failed to create tmp dir: {e}"))?;
    let meta =
        fs::symlink_metadata(&body_dir).map_err(|e| format!("failed to stat tmp dir: {e}"))?;
    if meta.file_type().is_symlink() {
        return Err("tmp/ is a symlink — refusing to write PR body".to_owned());
    }
    let body_file = body_dir.join(format!("pr-body-{}.md", std::process::id()));
    let _ = fs::remove_file(&body_file);
    let body_text = pr_body(ctx);
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&body_file)
        .map_err(|e| format!("failed to create PR body file: {e}"))?;
    f.write_all(body_text.as_bytes()).map_err(|e| format!("failed to write PR body file: {e}"))?;
    Ok(body_file)
}

// ---------------------------------------------------------------------------
// Zero-findings detection helpers
// ---------------------------------------------------------------------------

pub(super) fn check_reaction_zero_findings<C: infrastructure::gh_cli::GhClient>(
    client: &C,
    repo: &str,
    pr: &str,
    trigger_dt: chrono::DateTime<chrono::FixedOffset>,
) -> Result<bool, String> {
    let reactions_json = client.list_reactions(repo, pr).map_err(|e| e.to_string())?;
    let reactions = usecase::pr_review::parse_paginated_json(&reactions_json)
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

pub(super) fn check_comment_zero_findings<C: infrastructure::gh_cli::GhClient>(
    client: &C,
    repo: &str,
    pr: &str,
    trigger_dt: chrono::DateTime<chrono::FixedOffset>,
) -> Result<bool, String> {
    let comments_json = client.list_issue_comments(repo, pr).map_err(|e| e.to_string())?;
    let comments = usecase::pr_review::parse_paginated_json(&comments_json)
        .map_err(|e| format!("failed to parse comments JSON: {e}"))?;
    for comment in &comments {
        let author =
            comment.get("user").and_then(|u| u.get("login")).and_then(|l| l.as_str()).unwrap_or("");
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

// ---------------------------------------------------------------------------
// Poll review for cycle
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_lines)]
pub(super) fn poll_review_for_cycle<C, Sleep>(
    pr: &str,
    trigger_timestamp: &str,
    interval: u64,
    timeout: u64,
    client: &C,
    sleep: &Sleep,
    head_commit: Option<&str>,
) -> Result<PollReviewResult, String>
where
    C: infrastructure::gh_cli::GhClient,
    Sleep: Fn(Duration),
{
    let trigger_time = trigger_timestamp.replace('Z', "+00:00");
    let trigger_dt = chrono::DateTime::parse_from_rfc3339(&trigger_time)
        .map_err(|e| format!("invalid trigger timestamp: {e}"))?;

    let repo_nwo = client.repo_nwo().map_err(|e| e.to_string())?;
    let deadline = Instant::now() + Duration::from_secs(timeout.min(86400));
    let mut any_bot_activity = false;

    eprintln!("Polling for Codex review on PR #{pr} (interval={interval}s, timeout={timeout}s)...");

    loop {
        if Instant::now() >= deadline {
            break;
        }

        let reviews_json = client.list_reviews(&repo_nwo, pr).map_err(|e| e.to_string())?;
        let reviews = usecase::pr_review::parse_paginated_json(&reviews_json)
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
            let submitted_raw = review.get("submitted_at").and_then(|s| s.as_str()).unwrap_or("");
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
                            let clean = usecase::pr_review::sanitize_text(body);
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
                let reactions = usecase::pr_review::parse_paginated_json(&reactions_json)
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

            if has_stale_reaction && check_comment_zero_findings(client, &repo_nwo, pr, trigger_dt)?
            {
                eprintln!("[OK] Zero-findings detected via comment text fallback.");
                return Ok(PollReviewResult::ZeroFindings);
            }
        }

        if !any_bot_activity {
            let comments_json =
                client.list_issue_comments(&repo_nwo, pr).map_err(|e| e.to_string())?;
            let comments = usecase::pr_review::parse_paginated_json(&comments_json)
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

    // Timeout recovery: only consider reviews submitted after `trigger_dt` so
    // that a stale review from a prior cycle on the same commit is never
    // resurrected as the result of the current cycle.
    if let Some(expected_commit) = head_commit {
        let recovery_nwo = client.repo_nwo().map_err(|e| e.to_string())?;
        let recovery_reviews_json =
            client.list_reviews(&recovery_nwo, pr).map_err(|e| e.to_string())?;
        let recovery_reviews = usecase::pr_review::parse_paginated_json(&recovery_reviews_json)
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
                // Only accept reviews submitted at or after the trigger timestamp
                // to avoid resurrecting a stale review from a prior cycle.
                let submitted_raw = r.get("submitted_at").and_then(|s| s.as_str()).unwrap_or("");
                let submitted_after_trigger = submitted_raw
                    .replace('Z', "+00:00")
                    .parse::<chrono::DateTime<chrono::FixedOffset>>()
                    .is_ok_and(|dt| dt >= trigger_dt);
                is_codex_bot(author)
                    && matches!(state, "APPROVED" | "CHANGES_REQUESTED" | "COMMENTED")
                    && review_commit == expected_commit
                    && submitted_after_trigger
            })
            .collect();
        if let Some(recovered) = find_latest_bot_review_in(&recovery_refs) {
            eprintln!("[OK] Recovered Codex review after timeout (commit-based fallback).");
            return Ok(PollReviewResult::ReviewFound(recovered));
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

pub(super) fn find_latest_bot_review_in(
    reviews: &[&serde_json::Value],
) -> Option<serde_json::Value> {
    let best = reviews.iter().max_by(|a, b| {
        let ts_a = a.get("submitted_at").and_then(|s| s.as_str()).unwrap_or("");
        let ts_b = b.get("submitted_at").and_then(|s| s.as_str()).unwrap_or("");
        ts_a.cmp(ts_b).then_with(|| {
            let id_a = a.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
            let id_b = b.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
            id_a.cmp(&id_b)
        })
    })?;
    let mut sanitized = (*best).clone();
    if let Some(obj) = sanitized.as_object_mut() {
        if let Some(serde_json::Value::String(body)) = obj.get("body") {
            let clean = usecase::pr_review::sanitize_text(body);
            obj.insert("body".to_owned(), serde_json::Value::String(clean));
        }
    }
    Some(sanitized)
}

pub(super) fn ensure_pr_for_cycle<C: infrastructure::gh_cli::GhClient>(
    ctx: &usecase::pr_workflow::PrBranchContext,
    base: &str,
    client: &C,
) -> Result<Option<String>, String> {
    match client.find_open_pr(&ctx.branch, base) {
        Ok(Some(pr)) => {
            println!("[OK] Reusing existing PR #{pr}");
            return Ok(Some(pr));
        }
        Ok(None) => {}
        Err(err) => {
            return Err(format!("failed to look up open PR: {err}"));
        }
    }

    let body_file = ensure_pr_body_file(ctx)?;

    let title = usecase::pr_workflow::pr_title(ctx);
    match client.create_pr(&ctx.branch, base, &title, &body_file) {
        Ok(pr) => {
            let _ = fs::remove_file(&body_file);
            println!("[OK] Created PR #{pr}");
            Ok(Some(pr))
        }
        Err(err) => {
            let _ = fs::remove_file(&body_file);
            Err(format!("failed to create PR: {err}"))
        }
    }
}

// ---------------------------------------------------------------------------
// parse_review helper — uses usecase::pr_review::PrReviewResult directly
// ---------------------------------------------------------------------------

pub(super) fn parse_review<C: infrastructure::gh_cli::GhClient>(
    pr: &str,
    review: &serde_json::Value,
    repo_nwo: &str,
    client: &C,
) -> Result<usecase::pr_review::PrReviewResult, String> {
    let review_id = review.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
    let state = review.get("state").and_then(|s| s.as_str()).unwrap_or("COMMENTED").to_owned();
    let raw_body = review.get("body").and_then(|s| s.as_str()).unwrap_or("");
    let body = usecase::pr_review::sanitize_text(raw_body);

    let mut findings: Vec<usecase::pr_review::PrReviewFinding> = Vec::new();
    let mut inline_count: u32 = 0;

    let review_id_str = review_id.to_string();
    let comments_json =
        client.list_review_comments(repo_nwo, pr, &review_id_str).map_err(|e| e.to_string())?;
    let comments = usecase::pr_review::parse_paginated_json(&comments_json)
        .map_err(|e| format!("failed to parse comments JSON: {e}"))?;
    for comment in &comments {
        inline_count += 1;
        let comment_body = usecase::pr_review::sanitize_text(
            comment.get("body").and_then(|s| s.as_str()).unwrap_or(""),
        );
        let path = comment.get("path").and_then(|s| s.as_str()).unwrap_or("").to_owned();
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
        let severity = usecase::pr_review::classify_severity(&comment_body);
        findings.push(usecase::pr_review::PrReviewFinding {
            severity: severity.to_owned(),
            path,
            line,
            end_line,
            body: comment_body,
            rule_id: None,
        });
    }

    if !body.is_empty() {
        let body_findings = usecase::pr_review::parse_body_findings(&body);
        findings.extend(body_findings);
    }

    let actionable =
        findings.iter().filter(|f| f.severity == "P0" || f.severity == "P1").count() as u32;
    let passed = state == "APPROVED" || (actionable == 0 && state != "CHANGES_REQUESTED");

    Ok(usecase::pr_review::PrReviewResult {
        review_id,
        state,
        body,
        findings,
        inline_comment_count: inline_count,
        actionable_count: actionable,
        passed,
    })
}

pub(super) fn format_review_summary(
    pr: &str,
    result: &usecase::pr_review::PrReviewResult,
) -> String {
    let status = if result.passed { "PASS" } else { "FAIL" };
    let mut lines = Vec::new();
    lines.push(String::new());
    lines.push(format!("=== PR Review Result: {status} ==="));
    lines.push(format!("PR: #{pr}"));
    lines.push(format!("Review ID: {}", result.review_id));
    lines.push(format!("State: {}", result.state));
    lines.push(format!("Inline comments: {}", result.inline_comment_count));
    lines.push(format!("Total findings: {}", result.findings.len()));
    lines.push(format!("Actionable (P0/P1): {}", result.actionable_count));

    if !result.findings.is_empty() {
        lines.push(String::new());
        lines.push("Findings:".to_owned());
        for (i, f) in result.findings.iter().enumerate() {
            let location = if !f.path.is_empty() && f.line.is_some() {
                format!("{}:{}", f.path, f.line.unwrap_or(0))
            } else if !f.path.is_empty() {
                f.path.clone()
            } else {
                "general".to_owned()
            };
            let truncated_body: String = f.body.chars().take(120).collect();
            lines.push(format!("  {}. [{}] {}: {}", i + 1, f.severity, location, truncated_body));
        }
    }
    lines.join("\n")
}

// ---------------------------------------------------------------------------
// resume_trigger_state
// ---------------------------------------------------------------------------

pub(super) fn resume_trigger_state(
    track_id: &str,
) -> Result<(String, String, Option<String>), String> {
    use infrastructure::git_cli::{GitRepository as _, SystemGitRepo};

    let state = load_trigger_state(track_id)?.ok_or_else(|| {
        format!(
            "no trigger state file found for track '{track_id}'. \
             Run without --resume to start a new review cycle."
        )
    })?;

    let repo = SystemGitRepo::discover().map_err(|e| e.to_string())?;
    let current_head = repo
        .output(&["rev-parse", "HEAD"])
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned());
    if let (Some(saved), Some(current)) = (&state.head_hash, &current_head) {
        if saved != current {
            cleanup_trigger_state(track_id);
            return Err(format!(
                "HEAD has changed since trigger was posted \
                 (saved={saved}, current={current}). \
                 Run without --resume to start a new review cycle."
            ));
        }
    }

    println!("[OK] Resumed trigger state for PR #{}", state.pr_number);
    Ok((state.pr_number, state.trigger_timestamp, state.head_hash))
}

// ---------------------------------------------------------------------------
// trigger_new_review
// ---------------------------------------------------------------------------

pub(super) fn trigger_new_review(
    explicit_track_id: Option<&str>,
    track_id: &str,
    client: &infrastructure::gh_cli::SystemGhClient,
) -> Result<Option<(String, String, Option<String>)>, String> {
    use infrastructure::gh_cli::GhClient as _;
    use infrastructure::git_cli::{GitRepository as _, SystemGitRepo};

    let ctx = resolve_branch_context(explicit_track_id)?;
    let repo = SystemGitRepo::discover().map_err(|e| e.to_string())?;
    println!("Pushing {} to origin...", ctx.branch);
    repo.push_branch(&ctx.branch).map_err(|e| e.to_string())?;
    println!("[OK] Pushed {}", ctx.branch);

    let pr_number = match ensure_pr_for_cycle(&ctx, "main", client)? {
        Some(pr) => pr,
        None => return Ok(None),
    };

    let nwo = client.repo_nwo().map_err(|e| e.to_string())?;
    let response =
        client.post_issue_comment(&nwo, &pr_number, "@codex review").map_err(|e| e.to_string())?;
    let trigger_timestamp = serde_json::from_str::<serde_json::Value>(&response)
        .ok()
        .and_then(|v| v.get("created_at")?.as_str().map(String::from))
        .unwrap_or_default();
    println!("[OK] Posted '@codex review' on PR #{pr_number} at {trigger_timestamp}");

    if trigger_timestamp.is_empty() {
        return Err("could not determine trigger timestamp from API response".to_owned());
    }

    let head_hash = repo
        .output(&["rev-parse", "HEAD"])
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned());

    save_trigger_state(&TriggerState {
        pr_number: pr_number.clone(),
        trigger_timestamp: trigger_timestamp.clone(),
        head_hash: head_hash.clone(),
        track_id: track_id.to_owned(),
    })?;

    Ok(Some((pr_number, trigger_timestamp, head_hash)))
}
