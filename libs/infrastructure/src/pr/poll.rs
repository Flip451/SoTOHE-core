//! Private polling and review helpers for the system PR command adapter.
//!
//! All items in this module are `pub(super)` — they are implementation details
//! of `apps/cli-composition/src/pr.rs` and must not appear on the public facade.
//!
//! Legacy polling helpers are compiled only for the unit-test suite below.
//! Production polling is delegated to the D4 `PrReviewPollingInteractor`.

use std::fs;
use std::path::{Path, PathBuf};

use super::CompositionError;

// ---------------------------------------------------------------------------
// Outcome of a poll-review cycle
// ---------------------------------------------------------------------------

#[derive(Debug)]
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

fn trigger_state_location(track_id: &str) -> Result<(PathBuf, PathBuf), CompositionError> {
    use infrastructure::git_cli::SystemGitRepo;
    let root = SystemGitRepo::discover()
        .map_err(|e| CompositionError::AdapterInit(e.to_string()))?
        .root()
        .to_path_buf();
    let path = trigger_state_path_under_root(&root, track_id)?;
    Ok((root, path))
}

/// Construct the trigger-state path only from a validated track identifier.
fn trigger_state_path_under_root(root: &Path, track_id: &str) -> Result<PathBuf, CompositionError> {
    use infrastructure::track::symlink_guard::reject_symlinks_up_to_root;

    let track_id = domain::TrackId::try_new(track_id.to_owned()).map_err(|e| {
        CompositionError::WiringFailed(format!("invalid trigger-state track ID '{track_id}': {e}"))
    })?;
    reject_symlinks_up_to_root(root).map_err(|e| {
        CompositionError::Infrastructure(format!(
            "refusing trigger-state I/O below untrusted repo root {}: {e}",
            root.display()
        ))
    })?;
    let root = root.canonicalize().map_err(|e| {
        CompositionError::Infrastructure(format!(
            "failed to normalize trigger-state repo root {}: {e}",
            root.display()
        ))
    })?;
    let track_id: &str = track_id.as_ref();
    let path = root.join("tmp/pr-review-state").join(format!("{track_id}.json"));
    if path.strip_prefix(&root).is_err() {
        return Err(CompositionError::Infrastructure(format!(
            "refusing trigger-state path outside repo root: {}",
            path.display()
        )));
    }
    Ok(path)
}

/// Reject state paths whose root, parent, or leaf is a symlink before I/O.
fn guard_trigger_state_path(path: &Path, root: &Path) -> Result<bool, CompositionError> {
    use infrastructure::track::symlink_guard::{reject_symlinks_below, reject_symlinks_up_to_root};

    reject_symlinks_up_to_root(root).map_err(|e| {
        CompositionError::Infrastructure(format!(
            "refusing trigger-state I/O below untrusted repo root {}: {e}",
            root.display()
        ))
    })?;
    reject_symlinks_below(path, root).map_err(|e| {
        CompositionError::Infrastructure(format!(
            "refusing trigger-state I/O at {}: {e}",
            path.display()
        ))
    })
}

pub(super) fn save_trigger_state(state: &TriggerState) -> Result<(), CompositionError> {
    let (root, path) = trigger_state_location(&state.track_id)?;
    guard_trigger_state_path(&path, &root)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            CompositionError::Infrastructure(format!(
                "failed to create dir {}: {e}",
                parent.display()
            ))
        })?;
    }
    guard_trigger_state_path(&path, &root)?;
    let json = serialize_trigger_state(state)?;
    fs::write(&path, json).map_err(|e| {
        CompositionError::Infrastructure(format!("failed to write {}: {e}", path.display()))
    })?;
    println!("[OK] Saved trigger state to {}", path.display());
    Ok(())
}

/// Serializes trigger state with canonical key order before it is persisted.
fn serialize_trigger_state(state: &TriggerState) -> Result<String, CompositionError> {
    let value = serde_json::to_value(state).map_err(|e| {
        CompositionError::Infrastructure(format!("failed to serialize trigger state: {e}"))
    })?;
    serde_json::to_string_pretty(&value).map_err(|e| {
        CompositionError::Infrastructure(format!("failed to serialize trigger state: {e}"))
    })
}

pub(super) fn load_trigger_state(track_id: &str) -> Result<Option<TriggerState>, CompositionError> {
    let (root, path) = trigger_state_location(track_id)?;
    if !guard_trigger_state_path(&path, &root)? {
        return Ok(None);
    }
    let json = fs::read_to_string(&path).map_err(|e| {
        CompositionError::Infrastructure(format!("failed to read {}: {e}", path.display()))
    })?;
    let state: TriggerState = serde_json::from_str(&json).map_err(|e| {
        CompositionError::Infrastructure(format!("failed to parse trigger state: {e}"))
    })?;
    Ok(Some(state))
}

pub(super) fn cleanup_trigger_state(track_id: &str) -> Result<(), CompositionError> {
    let (root, path) = trigger_state_location(track_id)?;
    guard_trigger_state_path(&path, &root)?;
    fs::remove_file(&path).map_err(|e| {
        CompositionError::Infrastructure(format!("failed to remove {}: {e}", path.display()))
    })?;
    Ok(())
}

// ---------------------------------------------------------------------------
// PR context helpers
// ---------------------------------------------------------------------------

pub(super) fn resolve_branch_context(
    explicit_track_id: Option<&str>,
) -> Result<usecase::pr_workflow::PrBranchContext, CompositionError> {
    use infrastructure::git_cli::SystemGitRepo;
    let repo =
        SystemGitRepo::discover().map_err(|e| CompositionError::AdapterInit(e.to_string()))?;
    let branch = repo
        .current_branch()
        .map_err(|e| CompositionError::Infrastructure(e.to_string()))?
        .ok_or_else(|| {
            CompositionError::WiringFailed("could not determine current branch".to_owned())
        })?;
    usecase::pr_workflow::resolve_pr_branch(&branch, explicit_track_id)
        .map_err(|e| CompositionError::WiringFailed(e.to_string()))
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
) -> Result<PathBuf, CompositionError> {
    use std::io::Write as _;
    use usecase::pr_workflow::pr_body;

    let body_dir = PathBuf::from("tmp");
    fs::create_dir_all(&body_dir)
        .map_err(|e| CompositionError::Infrastructure(format!("failed to create tmp dir: {e}")))?;
    let meta = fs::symlink_metadata(&body_dir)
        .map_err(|e| CompositionError::Infrastructure(format!("failed to stat tmp dir: {e}")))?;
    if meta.file_type().is_symlink() {
        return Err(CompositionError::WiringFailed(
            "tmp/ is a symlink — refusing to write PR body".to_owned(),
        ));
    }
    let body_file = body_dir.join(format!("pr-body-{}.md", std::process::id()));
    let _ = fs::remove_file(&body_file);
    let body_text = pr_body(ctx);
    let mut f =
        fs::OpenOptions::new().write(true).create_new(true).open(&body_file).map_err(|e| {
            CompositionError::Infrastructure(format!("failed to create PR body file: {e}"))
        })?;
    f.write_all(body_text.as_bytes()).map_err(|e| {
        CompositionError::Infrastructure(format!("failed to write PR body file: {e}"))
    })?;
    Ok(body_file)
}

// ---------------------------------------------------------------------------
// Zero-findings detection helpers (legacy polling — unit-test suite only)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod test_support {
    use std::time::{Duration, Instant};

    use super::PollReviewResult;

    pub(super) const CODEX_BOT_LOGINS: &[&str] =
        &["openai-codex[bot]", "codex[bot]", "chatgpt-codex-connector[bot]"];

    pub(super) fn is_codex_bot(login: &str) -> bool {
        let lower = login.to_lowercase();
        CODEX_BOT_LOGINS.iter().any(|known| *known == lower)
    }

    #[derive(Debug, thiserror::Error)]
    #[error("{0}")]
    pub(super) struct PollTestError(String);

    #[cfg(test)]
    pub(super) fn check_reaction_zero_findings<C: infrastructure::gh_cli::GhClient>(
        client: &C,
        repo: &str,
        pr: &str,
        trigger_dt: chrono::DateTime<chrono::FixedOffset>,
    ) -> Result<bool, PollTestError> {
        let reactions_json =
            client.list_reactions(repo, pr).map_err(|e| PollTestError(e.to_string()))?;
        let reactions = usecase::pr_review::parse_paginated_json(&reactions_json)
            .map_err(|e| PollTestError(format!("failed to parse reactions JSON: {e}")))?;
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
                .map_err(|e| PollTestError(format!("invalid reaction created_at: {e}")))?;
            if created_dt >= trigger_dt {
                return Ok(true);
            }
        }
        Ok(false)
    }

    #[cfg(test)]
    pub(super) fn check_comment_zero_findings<C: infrastructure::gh_cli::GhClient>(
        client: &C,
        repo: &str,
        pr: &str,
        trigger_dt: chrono::DateTime<chrono::FixedOffset>,
    ) -> Result<bool, PollTestError> {
        let comments_json =
            client.list_issue_comments(repo, pr).map_err(|e| PollTestError(e.to_string()))?;
        let comments = usecase::pr_review::parse_paginated_json(&comments_json)
            .map_err(|e| PollTestError(format!("failed to parse comments JSON: {e}")))?;
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
                .map_err(|e| PollTestError(format!("invalid comment created_at: {e}")))?;
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
    #[cfg(test)]
    pub(super) fn poll_review_for_cycle<C, Sleep>(
        pr: &str,
        trigger_timestamp: &str,
        interval: u64,
        timeout: u64,
        client: &C,
        sleep: &Sleep,
        head_commit: Option<&str>,
    ) -> Result<PollReviewResult, PollTestError>
    where
        C: infrastructure::gh_cli::GhClient,
        Sleep: Fn(Duration),
    {
        let trigger_time = trigger_timestamp.replace('Z', "+00:00");
        let trigger_dt = chrono::DateTime::parse_from_rfc3339(&trigger_time)
            .map_err(|e| PollTestError(format!("invalid trigger timestamp: {e}")))?;

        let repo_nwo = client.repo_nwo().map_err(|e| PollTestError(e.to_string()))?;
        let deadline = Instant::now() + Duration::from_secs(timeout.min(86400));
        let mut any_bot_activity = false;

        eprintln!(
            "Polling for Codex review on PR #{pr} (interval={interval}s, timeout={timeout}s)..."
        );

        loop {
            if Instant::now() >= deadline {
                break;
            }

            let reviews_json =
                client.list_reviews(&repo_nwo, pr).map_err(|e| PollTestError(e.to_string()))?;
            let reviews = usecase::pr_review::parse_paginated_json(&reviews_json)
                .map_err(|e| PollTestError(format!("failed to parse reviews JSON: {e}")))?;

            // Collect all qualifying Codex bot reviews from this iteration (post-trigger,
            // with a terminal state), then pick the latest one by submitted_at (AC-05 / CN-02).
            let mut qualifying: Vec<&serde_json::Value> = Vec::new();
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
                    .map_err(|e| PollTestError(format!("invalid review submitted_at: {e}")))?;
                if submitted_dt >= trigger_dt {
                    any_bot_activity = true;
                    let state = review.get("state").and_then(|s| s.as_str()).unwrap_or("");
                    if matches!(state, "APPROVED" | "CHANGES_REQUESTED" | "COMMENTED") {
                        qualifying.push(review);
                    }
                }
            }
            if let Some(latest) = find_latest_bot_review_in(&qualifying) {
                let review_id = latest.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
                let state = latest.get("state").and_then(|s| s.as_str()).unwrap_or("");
                eprintln!("[OK] Found Codex review (id={review_id}, state={state})");
                return Ok(PollReviewResult::ReviewFound(latest));
            }

            if head_commit.is_some() {
                if check_reaction_zero_findings(client, &repo_nwo, pr, trigger_dt)? {
                    eprintln!("[OK] Zero-findings detected via +1 reaction.");
                    return Ok(PollReviewResult::ZeroFindings);
                }

                let has_stale_reaction = {
                    let reactions_json = client
                        .list_reactions(&repo_nwo, pr)
                        .map_err(|e| PollTestError(e.to_string()))?;
                    let reactions = usecase::pr_review::parse_paginated_json(&reactions_json)
                        .map_err(|e| {
                            PollTestError(format!("failed to parse reactions JSON: {e}"))
                        })?;
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
                let comments_json = client
                    .list_issue_comments(&repo_nwo, pr)
                    .map_err(|e| PollTestError(e.to_string()))?;
                let comments = usecase::pr_review::parse_paginated_json(&comments_json)
                    .map_err(|e| PollTestError(format!("failed to parse comments JSON: {e}")))?;
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
                        .map_err(|e| PollTestError(format!("invalid comment created_at: {e}")))?;
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

        // Timeout recovery: consider any Codex bot review on the exact same commit SHA.
        // `review_commit == expected_commit` is itself the stale guard — a review on a
        // different SHA cannot be resurrected as the current cycle's result. Restoring
        // the pre-T005 cli behavior (the original `apps/cli/src/commands/pr.rs` did not
        // carry an additional `submitted_after_trigger` predicate here). Per PR #143
        // Codex Cloud reviewer: the extra timestamp filter rejects valid same-SHA
        // reviews from prior cycles even though `commit_id == expected_commit` proves
        // they cover the exact HEAD being reviewed.
        if let Some(expected_commit) = head_commit {
            let recovery_nwo = client.repo_nwo().map_err(|e| PollTestError(e.to_string()))?;
            let recovery_reviews_json =
                client.list_reviews(&recovery_nwo, pr).map_err(|e| PollTestError(e.to_string()))?;
            let recovery_reviews = usecase::pr_review::parse_paginated_json(&recovery_reviews_json)
                .map_err(|e| {
                    PollTestError(format!("recovery: failed to parse reviews JSON: {e}"))
                })?;
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

    #[cfg(test)]
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
}

pub(super) fn ensure_pr_for_cycle<C: infrastructure::gh_cli::GhClient>(
    ctx: &usecase::pr_workflow::PrBranchContext,
    base: &str,
    client: &C,
) -> Result<Option<String>, CompositionError> {
    match client.find_open_pr(&ctx.branch, base) {
        Ok(Some(pr)) => {
            println!("[OK] Reusing existing PR #{pr}");
            return Ok(Some(pr));
        }
        Ok(None) => {}
        Err(err) => {
            return Err(CompositionError::Infrastructure(format!(
                "failed to look up open PR: {err}"
            )));
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
            Err(CompositionError::Infrastructure(format!("failed to create PR: {err}")))
        }
    }
}

// ---------------------------------------------------------------------------
// parse_review helper — passthrough: inline comments only, no interpretation
// ---------------------------------------------------------------------------

pub(super) fn parse_review<C: infrastructure::gh_cli::GhClient>(
    pr: &str,
    review: &serde_json::Value,
    repo_nwo: &str,
    client: &C,
) -> Result<usecase::pr_review::PrReviewResult, CompositionError> {
    let review_id = review.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
    let state = review.get("state").and_then(|s| s.as_str()).unwrap_or("COMMENTED").to_owned();
    let raw_body = review.get("body").and_then(|s| s.as_str()).unwrap_or("");
    // Sanitize body but retain it (D3: review.body is not discarded).
    let body = usecase::pr_review::sanitize_text(raw_body);

    let mut findings: Vec<usecase::pr_review::PrReviewFinding> = Vec::new();
    let mut inline_count: u32 = 0;

    let review_id_str = review_id.to_string();
    let comments_json = client
        .list_review_comments(repo_nwo, pr, &review_id_str)
        .map_err(|e| CompositionError::Infrastructure(e.to_string()))?;
    let comments = usecase::pr_review::parse_paginated_json(&comments_json).map_err(|e| {
        CompositionError::Infrastructure(format!("failed to parse comments JSON: {e}"))
    })?;
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
        // No severity classification (D1): inline comment is a passthrough container.
        findings.push(usecase::pr_review::PrReviewFinding {
            path,
            line,
            end_line,
            body: comment_body,
        });
    }
    // No body-findings extraction (D1): parse_body_findings is removed.

    Ok(usecase::pr_review::PrReviewResult {
        review_id,
        state,
        body,
        findings,
        inline_comment_count: inline_count,
    })
}

pub(super) fn format_review_summary(
    pr: &str,
    result: &usecase::pr_review::PrReviewResult,
) -> String {
    let mut lines = Vec::new();
    lines.push(String::new());
    lines.push("=== PR Review Result: ReviewFound ===".to_owned());
    lines.push(format!("PR: #{pr}"));
    lines.push(format!("Review ID: {}", result.review_id));
    lines.push(format!("State: {}", result.state));
    lines.push(format!("Inline comments: {}", result.inline_comment_count));

    if !result.body.is_empty() {
        lines.push(String::new());
        lines.push("Review Body:".to_owned());
        lines.push(result.body.clone());
    }

    if !result.findings.is_empty() {
        lines.push(String::new());
        lines.push("Inline Comments:".to_owned());
        for (i, f) in result.findings.iter().enumerate() {
            let location = if !f.path.is_empty() && f.line.is_some() {
                format!("{}:{}", f.path, f.line.unwrap_or(0))
            } else if !f.path.is_empty() {
                f.path.clone()
            } else {
                "(no location)".to_owned()
            };
            lines.push(format!("  {}. {}: {}", i + 1, location, f.body));
        }
    }
    lines.join("\n")
}

// ---------------------------------------------------------------------------
// resume_trigger_state
// ---------------------------------------------------------------------------

fn resolve_head_hash() -> Result<String, CompositionError> {
    use infrastructure::FsGitWorkflowAdapter;
    use std::sync::Arc;
    use usecase::git_workflow::{GitPrimitivePort, PrGitInteractor};

    let port: Arc<dyn GitPrimitivePort> = Arc::new(FsGitWorkflowAdapter::new());
    let interactor = PrGitInteractor::new(port);
    interactor
        .resolve_head()
        .map_err(|e| CompositionError::Infrastructure(format!("failed to resolve HEAD: {e}")))?
        .map(|head| head.as_ref().to_owned())
        .ok_or_else(|| CompositionError::WiringFailed("could not determine HEAD".to_owned()))
}

pub(super) fn resume_trigger_state(
    track_id: &str,
) -> Result<(String, String, Option<String>), CompositionError> {
    use infrastructure::git_cli::SystemGitRepo;

    let state = load_trigger_state(track_id)?.ok_or_else(|| {
        CompositionError::WiringFailed(format!(
            "no trigger state file found for track '{track_id}'. \
             Run without --resume to start a new review cycle."
        ))
    })?;

    // Route HEAD resolution through the usecase PrGitInteractor (T007). We
    // still perform `SystemGitRepo::discover` up-front to preserve the
    // fail-closed "no git repo" contract of the surrounding function.
    let _repo =
        SystemGitRepo::discover().map_err(|e| CompositionError::AdapterInit(e.to_string()))?;
    let current_head = resolve_head_hash()?;
    let saved_head = state.head_hash.as_deref().ok_or_else(|| {
        CompositionError::WiringFailed(
            "saved trigger state has no HEAD; run without --resume to start a new review cycle."
                .to_owned(),
        )
    })?;
    if saved_head != current_head {
        cleanup_trigger_state(track_id)?;
        return Err(CompositionError::WiringFailed(format!(
            "HEAD has changed since trigger was posted \
             (saved={saved_head}, current={current_head}). \
             Run without --resume to start a new review cycle."
        )));
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
) -> Result<Option<(String, String, Option<String>)>, CompositionError> {
    use infrastructure::gh_cli::GhClient as _;
    use infrastructure::git_cli::SystemGitRepo;

    let ctx = resolve_branch_context(explicit_track_id)?;
    let repo =
        SystemGitRepo::discover().map_err(|e| CompositionError::AdapterInit(e.to_string()))?;
    println!("Pushing {} to origin...", ctx.branch);
    repo.push_branch(&ctx.branch).map_err(|e| CompositionError::Infrastructure(e.to_string()))?;
    println!("[OK] Pushed {}", ctx.branch);

    // The cycle has pushed its branch, so require a HEAD before making any
    // external review request. Otherwise a failed resolution would leave a
    // posted comment with no resumable state and retries would duplicate it.
    let head_hash = resolve_head_hash()?;

    let base = super::branch_strategy_port_for_track(&ctx.track_id).map(|port| {
        use usecase::branch_strategy::BranchStrategyPort as _;
        port.merge_target().to_owned()
    })?;
    let pr_number = match ensure_pr_for_cycle(&ctx, &base, client)? {
        Some(pr) => pr,
        None => return Ok(None),
    };

    let nwo = client.repo_nwo().map_err(|e| CompositionError::Infrastructure(e.to_string()))?;
    let response = client
        .post_issue_comment(&nwo, &pr_number, "@codex review")
        .map_err(|e| CompositionError::Infrastructure(e.to_string()))?;
    let trigger_timestamp = serde_json::from_str::<serde_json::Value>(&response)
        .ok()
        .and_then(|v| v.get("created_at")?.as_str().map(String::from))
        .unwrap_or_default();
    println!("[OK] Posted '@codex review' on PR #{pr_number} at {trigger_timestamp}");

    if trigger_timestamp.is_empty() {
        return Err(CompositionError::Infrastructure(
            "could not determine trigger timestamp from API response".to_owned(),
        ));
    }

    save_trigger_state(&TriggerState {
        pr_number: pr_number.clone(),
        trigger_timestamp: trigger_timestamp.clone(),
        head_hash: Some(head_hash.clone()),
        track_id: track_id.to_owned(),
    })?;

    Ok(Some((pr_number, trigger_timestamp, Some(head_hash))))
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use std::cell::RefCell;
    use std::path::Path;
    use std::time::Duration;

    use infrastructure::gh_cli::{GhClient, GhError, PrCheckRecord};

    use super::test_support::poll_review_for_cycle;
    use super::{
        PollReviewResult, TriggerState, guard_trigger_state_path, serialize_trigger_state,
        trigger_state_path_under_root,
    };

    // ------------------------------------------------------------------
    // Minimal fake GhClient for poll tests
    // ------------------------------------------------------------------

    struct FakePollClient {
        /// Responses returned by `list_reviews` on successive calls.
        /// The first entry is used in the main loop, the second in recovery.
        reviews_responses: RefCell<Vec<String>>,
        issue_comments: String,
        reactions: String,
    }

    impl FakePollClient {
        fn new(reviews_responses: Vec<String>) -> Self {
            Self {
                reviews_responses: RefCell::new(reviews_responses),
                issue_comments: "[]".to_owned(),
                reactions: "[]".to_owned(),
            }
        }
    }

    impl GhClient for FakePollClient {
        fn pr_checks(&self, _pr: &str) -> Result<Vec<PrCheckRecord>, GhError> {
            Ok(vec![])
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

        fn list_reviews(&self, _repo_nwo: &str, _pr: &str) -> Result<String, GhError> {
            let mut responses = self.reviews_responses.borrow_mut();
            if responses.is_empty() { Ok("[]".to_owned()) } else { Ok(responses.remove(0)) }
        }

        fn list_issue_comments(&self, _repo_nwo: &str, _pr: &str) -> Result<String, GhError> {
            Ok(self.issue_comments.clone())
        }

        fn list_reactions(&self, _repo_nwo: &str, _pr: &str) -> Result<String, GhError> {
            Ok(self.reactions.clone())
        }

        fn repo_nwo(&self) -> Result<String, GhError> {
            Ok("owner/repo".to_owned())
        }
    }

    // ------------------------------------------------------------------
    // Helpers
    // ------------------------------------------------------------------

    /// Build a single-element JSON array representing one Codex review.
    fn make_reviews_json(
        commit_id: &str,
        state: &str,
        submitted_at: &str,
        bot_login: &str,
    ) -> String {
        format!(
            r#"[{{"id":1,"user":{{"login":"{bot_login}"}},"state":"{state}","commit_id":"{commit_id}","submitted_at":"{submitted_at}","body":""}}]"#
        )
    }

    #[cfg(unix)]
    #[test]
    fn test_guard_trigger_state_path_rejects_symlinked_parent() {
        let sandbox = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let tmp = sandbox.path().join("tmp");
        std::fs::create_dir_all(&tmp).unwrap();
        std::os::unix::fs::symlink(outside.path(), tmp.join("pr-review-state")).unwrap();

        let path = tmp.join("pr-review-state/track.json");
        let error = guard_trigger_state_path(&path, sandbox.path()).unwrap_err();

        assert!(error.to_string().contains("refusing trigger-state I/O"));
    }

    #[cfg(unix)]
    #[test]
    fn test_guard_trigger_state_path_rejects_symlinked_leaf() {
        let sandbox = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let state_dir = sandbox.path().join("tmp/pr-review-state");
        std::fs::create_dir_all(&state_dir).unwrap();
        let leaf = state_dir.join("track.json");
        std::os::unix::fs::symlink(outside.path().join("state.json"), &leaf).unwrap();

        let error = guard_trigger_state_path(&leaf, sandbox.path()).unwrap_err();

        assert!(error.to_string().contains("refusing trigger-state I/O"));
    }

    #[test]
    fn test_trigger_state_path_rejects_track_id_escape_before_construction() {
        let sandbox = tempfile::tempdir().unwrap();

        let error = trigger_state_path_under_root(sandbox.path(), "../outside").unwrap_err();

        assert!(error.to_string().contains("invalid trigger-state track ID"));
    }

    #[test]
    fn test_save_trigger_state_with_populated_state_produces_canonical_stable_json() {
        let state = TriggerState {
            pr_number: "1234".to_owned(),
            trigger_timestamp: "2026-08-02T03:04:05Z".to_owned(),
            head_hash: Some("0123456789abcdef".to_owned()),
            track_id: "test-track-2026".to_owned(),
        };

        let first = serialize_trigger_state(&state).unwrap();
        let second = serialize_trigger_state(&state).unwrap();

        assert_eq!(first, second, "identical trigger states must produce identical bytes");
        assert_eq!(
            first,
            concat!(
                "{\n",
                "  \"head_hash\": \"0123456789abcdef\",\n",
                "  \"pr_number\": \"1234\",\n",
                "  \"track_id\": \"test-track-2026\",\n",
                "  \"trigger_timestamp\": \"2026-08-02T03:04:05Z\"\n",
                "}"
            )
        );
    }

    // ------------------------------------------------------------------
    // Tests
    // ------------------------------------------------------------------

    /// Recovery path accepts any same-SHA Codex bot review regardless of
    /// submission timestamp. `commit_id == expected_commit` is itself the
    /// stale guard (a different-SHA review cannot represent the current
    /// HEAD). This matches the pre-T005 cli behavior (`apps/cli/src/commands/
    /// pr.rs` carried no `submitted_after_trigger` predicate here).
    #[test]
    fn test_poll_review_for_cycle_with_timeout_and_pre_trigger_same_sha_review_returns_review() {
        let trigger_timestamp = "2024-01-01T01:00:00Z";
        let review_submitted_at = "2024-01-01T00:30:00Z"; // 30 min before trigger
        let head_commit = "abc123";

        // timeout=0 means the deadline is already past before the loop body runs,
        // so the main loop never calls list_reviews.  The only call is from the
        // recovery path.  The same-SHA review must be accepted.
        let reviews_json =
            make_reviews_json(head_commit, "CHANGES_REQUESTED", review_submitted_at, "codex[bot]");
        let client = FakePollClient::new(vec![
            reviews_json, // recovery fetch (loop body is skipped entirely when timeout=0)
        ]);

        let no_sleep = |_: Duration| {};

        let result = poll_review_for_cycle(
            "1",
            trigger_timestamp,
            1, // interval
            0, // timeout=0 → deadline already expired before the loop body runs
            &client,
            &no_sleep,
            Some(head_commit),
        )
        .unwrap();

        assert!(
            matches!(result, PollReviewResult::ReviewFound(_)),
            "expected ReviewFound — same-SHA review must be accepted in recovery (commit-id is the stale guard)"
        );
    }

    /// Recovery path returns a same-SHA review submitted *after* the trigger
    /// timestamp, e.g. when the review completed but the main polling loop timed
    /// out due to API delays or lost trigger state.
    #[test]
    fn test_poll_review_for_cycle_with_timeout_and_post_trigger_same_sha_review_returns_review() {
        let trigger_timestamp = "2024-01-01T00:00:00Z";
        let review_submitted_at = "2024-01-01T00:30:00Z"; // 30 min after trigger
        let head_commit = "abc123";

        let reviews_json =
            make_reviews_json(head_commit, "CHANGES_REQUESTED", review_submitted_at, "codex[bot]");
        let client = FakePollClient::new(vec![
            reviews_json, // recovery fetch (loop body is skipped entirely when timeout=0)
        ]);

        let no_sleep = |_: Duration| {};

        let result = poll_review_for_cycle(
            "1",
            trigger_timestamp,
            1,
            0,
            &client,
            &no_sleep,
            Some(head_commit),
        )
        .unwrap();

        assert!(
            matches!(result, PollReviewResult::ReviewFound(_)),
            "expected ReviewFound — post-trigger same-SHA review must be accepted in recovery"
        );
    }

    /// Recovery path returns `Timeout` when no review with the expected commit
    /// SHA exists, regardless of timestamps.
    #[test]
    fn test_poll_review_for_cycle_with_timeout_and_no_matching_sha_returns_timeout() {
        let trigger_timestamp = "2024-01-01T00:00:00Z";
        let head_commit = "abc123";
        let different_commit = "deadbeef";

        let reviews_json = make_reviews_json(
            different_commit,
            "CHANGES_REQUESTED",
            "2024-01-01T00:30:00Z",
            "codex[bot]",
        );
        // The loop body is skipped with timeout=0; only recovery fetch runs.
        let client = FakePollClient::new(vec![
            reviews_json, // recovery fetch — different SHA, must not be picked up
        ]);

        let no_sleep = |_: Duration| {};

        let result = poll_review_for_cycle(
            "1",
            trigger_timestamp,
            1,
            0,
            &client,
            &no_sleep,
            Some(head_commit),
        )
        .unwrap();

        assert!(
            matches!(result, PollReviewResult::Timeout),
            "expected Timeout when no same-SHA review exists"
        );
    }

    // ------------------------------------------------------------------
    // T005: passthrough behavior tests (AC-03 to AC-09)
    // ------------------------------------------------------------------

    /// A fake client that also supports list_review_comments and list_reactions/
    /// list_issue_comments for parse_review-based tests.
    struct FullFakePollClient {
        reviews: String,
        review_comments: String,
        issue_comments: String,
        reactions: String,
    }

    impl FullFakePollClient {
        fn new(
            reviews: &str,
            review_comments: &str,
            issue_comments: &str,
            reactions: &str,
        ) -> Self {
            Self {
                reviews: reviews.to_owned(),
                review_comments: review_comments.to_owned(),
                issue_comments: issue_comments.to_owned(),
                reactions: reactions.to_owned(),
            }
        }
    }

    impl GhClient for FullFakePollClient {
        fn pr_checks(&self, _pr: &str) -> Result<Vec<PrCheckRecord>, GhError> {
            Ok(vec![])
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

        fn list_reviews(&self, _repo_nwo: &str, _pr: &str) -> Result<String, GhError> {
            Ok(self.reviews.clone())
        }

        fn list_review_comments(
            &self,
            _repo_nwo: &str,
            _pr: &str,
            _review_id: &str,
        ) -> Result<String, GhError> {
            Ok(self.review_comments.clone())
        }

        fn list_issue_comments(&self, _repo_nwo: &str, _pr: &str) -> Result<String, GhError> {
            Ok(self.issue_comments.clone())
        }

        fn list_reactions(&self, _repo_nwo: &str, _pr: &str) -> Result<String, GhError> {
            Ok(self.reactions.clone())
        }

        fn repo_nwo(&self) -> Result<String, GhError> {
            Ok("owner/repo".to_owned())
        }
    }

    /// AC-09: COMMENTED review state produces ReviewFound, not FAIL.
    /// This test verifies that a COMMENTED review is surfaced as ReviewFound
    /// (the broken behavior before this track was: COMMENTED could never PASS
    /// because actionable_count was always > 0 from body phantom findings).
    #[test]
    fn test_poll_review_for_cycle_with_commented_review_returns_review_found_not_timeout() {
        let client = FakePollClient::new(vec![
            r#"[{"id":10,"user":{"login":"codex[bot]"},"state":"COMMENTED","commit_id":"abc","submitted_at":"2026-05-29T10:05:00Z","body":"Some comments here."}]"#
                .to_owned(),
        ]);
        let result = poll_review_for_cycle(
            "1",
            "2026-05-29T10:00:00Z",
            1,
            60,
            &client,
            &|_| {},
            Some("abc"),
        )
        .unwrap();
        assert!(
            matches!(result, PollReviewResult::ReviewFound(_)),
            "COMMENTED review must produce ReviewFound (AC-09)"
        );
    }

    /// AC-05: When multiple Codex reviews exist, only the one with the latest
    /// submitted_at is returned.
    #[test]
    fn test_poll_review_for_cycle_with_multiple_rounds_returns_latest_submitted_at() {
        // Two reviews: older round (id=1) and newer round (id=2).
        let reviews_json = r#"[
            {"id":1,"user":{"login":"codex[bot]"},"state":"COMMENTED","commit_id":"abc","submitted_at":"2026-05-29T09:00:00Z","body":"old round"},
            {"id":2,"user":{"login":"codex[bot]"},"state":"CHANGES_REQUESTED","commit_id":"abc","submitted_at":"2026-05-29T10:05:00Z","body":"new round"}
        ]"#;
        let client = FakePollClient::new(vec![reviews_json.to_owned()]);
        let result = poll_review_for_cycle(
            "1",
            "2026-05-29T08:00:00Z",
            1,
            60,
            &client,
            &|_| {},
            Some("abc"),
        )
        .unwrap();
        match result {
            PollReviewResult::ReviewFound(v) => {
                let id = v.get("id").and_then(|x| x.as_u64()).unwrap_or(0);
                assert_eq!(id, 2, "must return latest review (id=2, AC-05)");
            }
            other => panic!("expected ReviewFound, got {other:?}"),
        }
    }

    /// AC-03: ReviewFound output contains sanitized review.body.
    #[test]
    fn test_parse_review_with_review_body_includes_sanitized_body_in_result() {
        let home_path = format!("/home/{}", "user");
        let review = serde_json::json!({
            "id": 42,
            "state": "COMMENTED",
            "body": format!("Please check {home_path}/project/src/main.rs for the issue.")
        });
        let client = FullFakePollClient::new("[]", "[]", "[]", "[]");
        let result = super::parse_review("1", &review, "owner/repo", &client).unwrap();
        assert!(
            result.body.contains("[PATH]"),
            "sanitize_text must be applied to review.body (AC-03, AC-08)"
        );
        assert!(
            !result.body.contains(home_path.as_str()),
            "absolute path must be redacted in review.body"
        );
    }

    /// AC-04: ReviewFound output contains inline comment path:line and sanitized body.
    #[test]
    fn test_parse_review_with_inline_comments_includes_path_and_line_in_findings() {
        let review = serde_json::json!({"id": 99, "state": "COMMENTED", "body": ""});
        let comments_json = r#"[{
            "path": "src/main.rs",
            "line": 42,
            "start_line": null,
            "body": "This variable should be renamed."
        }]"#;
        let client = FullFakePollClient::new("[]", comments_json, "[]", "[]");
        let result = super::parse_review("1", &review, "owner/repo", &client).unwrap();
        assert_eq!(result.findings.len(), 1, "should have one inline comment");
        assert_eq!(result.findings[0].path, "src/main.rs");
        assert_eq!(result.findings[0].line, Some(42));
        assert!(
            result.findings[0].body.contains("renamed"),
            "finding body should contain sanitized comment text (AC-04)"
        );
    }

    /// AC-08: sanitize_text is applied to inline comment bodies.
    #[test]
    fn test_parse_review_with_secret_in_inline_comment_body_redacts_it() {
        let review = serde_json::json!({"id": 7, "state": "CHANGES_REQUESTED", "body": ""});
        let comments_json = r#"[{
            "path": "config.rs",
            "line": 5,
            "body": "Token sk-abcdefghijklmnopqrstuvwx should not be committed."
        }]"#;
        let client = FullFakePollClient::new("[]", comments_json, "[]", "[]");
        let result = super::parse_review("1", &review, "owner/repo", &client).unwrap();
        assert_eq!(result.findings.len(), 1);
        assert!(
            !result.findings[0].body.contains("sk-"),
            "secret must be redacted in inline comment body (AC-08)"
        );
        assert!(
            result.findings[0].body.contains("[REDACTED]"),
            "expected [REDACTED] placeholder in inline comment body"
        );
    }

    /// AC-06: Zero-findings via +1 reaction from Codex bot produces ZeroFindings.
    #[test]
    fn test_poll_review_for_cycle_with_post_trigger_thumbsup_reaction_returns_zero_findings() {
        let reactions_json = r#"[{
            "content": "+1",
            "user": {"login": "openai-codex[bot]"},
            "created_at": "2026-05-29T10:05:00Z"
        }]"#;
        let client = FullFakePollClient::new("[]", "[]", "[]", reactions_json);
        let result = poll_review_for_cycle(
            "1",
            "2026-05-29T10:00:00Z",
            1,
            60,
            &client,
            &|_| {},
            Some("abc"),
        )
        .unwrap();
        assert!(
            matches!(result, PollReviewResult::ZeroFindings),
            "post-trigger +1 reaction from Codex bot must produce ZeroFindings (AC-06)"
        );
    }

    /// AC-07: Zero-findings via "Didn't find any major issues" comment.
    #[test]
    fn test_poll_review_for_cycle_with_zero_findings_comment_returns_zero_findings() {
        // Stale reaction (pre-trigger) + post-trigger comment with the magic phrase.
        let reactions_json = r#"[{
            "content": "+1",
            "user": {"login": "openai-codex[bot]"},
            "created_at": "2026-05-29T09:00:00Z"
        }]"#;
        let comments_json = r#"[{
            "user": {"login": "openai-codex[bot]"},
            "body": "Didn't find any major issues with the code.",
            "created_at": "2026-05-29T10:05:00Z"
        }]"#;
        let client = FullFakePollClient::new("[]", "[]", comments_json, reactions_json);
        let result = poll_review_for_cycle(
            "1",
            "2026-05-29T10:00:00Z",
            1,
            60,
            &client,
            &|_| {},
            Some("abc"),
        )
        .unwrap();
        assert!(
            matches!(result, PollReviewResult::ZeroFindings),
            "\"Didn't find any major issues\" comment must produce ZeroFindings (AC-07)"
        );
    }
}
