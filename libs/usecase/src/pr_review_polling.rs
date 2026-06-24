//! PR review polling service (D4 orchestration extracted from cli_composition).
//!
//! Hosts the polling loop, ports (`SleepPort`, `PrListReviewsPort`,
//! `PrListReactionsPort`, `PrListIssueCommentsPort`, `PrRepoNwoPort`), and the
//! application service `PrReviewPollingService` + `PrReviewPollingInteractor`.
//! Co-located in this sibling module to keep `pr_review.rs` under the
//! module-size limit (700 lines).

use std::time::Duration;

use crate::pr_review::{parse_paginated_json, sanitize_text};

// ── SleepPort ─────────────────────────────────────────────────────────────────

/// Secondary port abstracting `thread::sleep`.
///
/// Allows the polling interactor to be tested with a mock that records sleep
/// calls rather than actually blocking, keeping the usecase layer free of
/// `thread::sleep` (which belongs in infrastructure).
pub trait SleepPort: Send + Sync {
    /// Sleep for the given duration.
    fn sleep(&self, duration: Duration);
}

// ── PrGhApiError ──────────────────────────────────────────────────────────────

/// Error type for GitHub PR API ports (`PrListReviewsPort`, `PrListReactionsPort`, `PrListIssueCommentsPort`).
#[derive(Debug, thiserror::Error)]
pub enum PrGhApiError {
    /// The GitHub API call failed.
    #[error("{0}")]
    ApiFailure(String),
}

// ── PrListReviewsPort ─────────────────────────────────────────────────────────

/// Secondary port for listing GitHub PR reviews.
///
/// Abstracts `GhClient::list_reviews` so the polling interactor does not
/// import from `infrastructure` directly.
pub trait PrListReviewsPort: Send + Sync {
    /// List all reviews for the given PR as a JSON string.
    ///
    /// # Errors
    ///
    /// Returns [`PrGhApiError`] on API failure.
    fn list_reviews(&self, repo_nwo: &str, pr: &str) -> Result<String, PrGhApiError>;
}

// ── PrListReactionsPort ───────────────────────────────────────────────────────

/// Secondary port for listing GitHub PR reactions (used by zero-findings detection).
pub trait PrListReactionsPort: Send + Sync {
    /// List all reactions for the given PR as a JSON string.
    ///
    /// # Errors
    ///
    /// Returns [`PrGhApiError`] on API failure.
    fn list_reactions(&self, repo_nwo: &str, pr: &str) -> Result<String, PrGhApiError>;
}

// ── PrListIssueCommentsPort ───────────────────────────────────────────────────

/// Secondary port for listing GitHub PR issue comments.
pub trait PrListIssueCommentsPort: Send + Sync {
    /// List all issue comments for the given PR as a JSON string.
    ///
    /// # Errors
    ///
    /// Returns [`PrGhApiError`] on API failure.
    fn list_issue_comments(&self, repo_nwo: &str, pr: &str) -> Result<String, PrGhApiError>;
}

// ── PrRepoNwoError ────────────────────────────────────────────────────────────

/// Error type for [`PrRepoNwoPort::repo_nwo`].
#[derive(Debug, thiserror::Error)]
pub enum PrRepoNwoError {
    /// NWO resolution failed.
    #[error("{0}")]
    Unavailable(String),
}

// ── PrRepoNwoPort ────────────────────────────────────────────────────────────

/// Secondary port for resolving the repository NWO (owner/name string).
pub trait PrRepoNwoPort: Send + Sync {
    /// Return the repository NWO string (e.g. `"owner/repo"`).
    ///
    /// # Errors
    ///
    /// Returns [`PrRepoNwoError`] when NWO resolution fails.
    fn repo_nwo(&self) -> Result<String, PrRepoNwoError>;
}

// ── Known Codex bot logins ─────────────────────────────────────────────────────

const POLLING_CODEX_BOT_LOGINS: &[&str] =
    &["openai-codex[bot]", "codex[bot]", "chatgpt-codex-connector[bot]"];

fn is_codex_bot(login: &str) -> bool {
    let lower = login.to_lowercase();
    POLLING_CODEX_BOT_LOGINS.iter().any(|known| *known == lower)
}

fn find_latest_bot_review(reviews: &[&serde_json::Value]) -> Option<serde_json::Value> {
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
            let clean = sanitize_text(body);
            obj.insert("body".to_owned(), serde_json::Value::String(clean));
        }
    }
    Some(sanitized)
}

// ── PrReviewPollingOutput ─────────────────────────────────────────────────────

/// Outcome variants for a single PR review polling cycle.
#[derive(Debug, Clone)]
pub enum PrReviewPollingOutput {
    /// A qualifying Codex bot review was found (the latest by `submitted_at`).
    ReviewFound(serde_json::Value),
    /// Zero-findings signal detected (bot +1 reaction or comment text fallback).
    ZeroFindings,
    /// The polling deadline was reached without finding a qualifying review.
    Timeout,
}

// ── PrReviewPollingCommand ────────────────────────────────────────────────────

/// CQRS command for the D4 PR review polling application service.
#[derive(Debug, Clone)]
pub struct PrReviewPollingCommand {
    /// PR number as a string (e.g. `"123"`).
    pub pr: String,
    /// Repository NWO string (e.g. `"owner/repo"`), pre-resolved by the caller.
    pub repo_nwo: String,
    /// Trigger timestamp in RFC 3339 format; reviews submitted before this are
    /// ignored.
    pub trigger_timestamp: String,
    /// Poll interval in seconds.
    pub interval_secs: u64,
    /// Maximum number of poll iterations (`timeout_secs / interval_secs`).
    /// Callers derive this from the requested timeout; the interactor uses this
    /// count with a deterministic elapsed-interval budget so timeout behavior is
    /// driven by command parameters rather than reading runtime time.
    pub max_iterations: u64,
    /// Expected HEAD commit SHA for commit-based timeout recovery and
    /// zero-findings detection. `None` disables these paths.
    pub head_commit: Option<String>,
}

// ── PrReviewPollingService ────────────────────────────────────────────────────

/// Application service (primary port) for the D4 PR review polling extraction.
///
/// Owns the polling loop logic previously embedded in
/// `cli_composition::pr::poll::poll_review_for_cycle`.
pub trait PrReviewPollingService: Send + Sync {
    /// Run the polling loop and return the outcome.
    ///
    /// # Errors
    ///
    /// Returns [`crate::d4_orchestration::D4OrchestrationError::PrPolling`] on
    /// timestamp parsing failures or API errors.
    fn poll(
        &self,
        cmd: PrReviewPollingCommand,
    ) -> Result<PrReviewPollingOutput, crate::d4_orchestration::D4OrchestrationError>;
}

// ── PrReviewPollingInteractor ─────────────────────────────────────────────────

/// Interactor implementing [`PrReviewPollingService`].
///
/// Holds injected secondary ports:
/// - `reviews`: lists PR reviews via GitHub API.
/// - `reactions`: lists PR reactions (zero-findings detection).
/// - `comments`: lists PR issue comments (bot activity + zero-findings fallback).
/// - `sleep`: abstracts `thread::sleep` so tests can verify sleep calls.
///
/// The deadline is enforced with a deterministic interval budget derived from
/// `interval_secs * max_iterations`; `max_iterations` remains as a second guard
/// against zero-interval spin loops.
pub struct PrReviewPollingInteractor {
    reviews: std::sync::Arc<dyn PrListReviewsPort>,
    reactions: std::sync::Arc<dyn PrListReactionsPort>,
    comments: std::sync::Arc<dyn PrListIssueCommentsPort>,
    sleep: std::sync::Arc<dyn SleepPort>,
}

impl PrReviewPollingInteractor {
    /// Construct with injected ports.
    #[must_use]
    pub fn new(
        reviews: std::sync::Arc<dyn PrListReviewsPort>,
        reactions: std::sync::Arc<dyn PrListReactionsPort>,
        comments: std::sync::Arc<dyn PrListIssueCommentsPort>,
        sleep: std::sync::Arc<dyn SleepPort>,
    ) -> Self {
        Self { reviews, reactions, comments, sleep }
    }
}

impl PrReviewPollingService for PrReviewPollingInteractor {
    #[allow(clippy::too_many_lines)]
    fn poll(
        &self,
        cmd: PrReviewPollingCommand,
    ) -> Result<PrReviewPollingOutput, crate::d4_orchestration::D4OrchestrationError> {
        let PrReviewPollingCommand {
            pr,
            repo_nwo,
            trigger_timestamp,
            interval_secs,
            max_iterations,
            head_commit,
        } = cmd;

        let trigger_time = trigger_timestamp.replace('Z', "+00:00");
        let trigger_dt = chrono::DateTime::parse_from_rfc3339(&trigger_time).map_err(|e| {
            crate::d4_orchestration::D4OrchestrationError::PrPolling(format!(
                "invalid trigger timestamp: {e}"
            ))
        })?;

        let mut any_bot_activity = false;
        let mut iterations = 0u64;
        let timeout_secs = interval_secs.max(1).saturating_mul(max_iterations).min(86_400);
        let effective_timeout = Duration::from_secs(timeout_secs);
        let interval_budget = Duration::from_secs(interval_secs.max(1));
        let mut elapsed_budget = Duration::ZERO;

        loop {
            if iterations >= max_iterations || elapsed_budget >= effective_timeout {
                break;
            }
            iterations += 1;

            let reviews_json = self.reviews.list_reviews(&repo_nwo, &pr).map_err(|e| {
                crate::d4_orchestration::D4OrchestrationError::PrPolling(e.to_string())
            })?;
            let reviews = parse_paginated_json(&reviews_json).map_err(|e| {
                crate::d4_orchestration::D4OrchestrationError::PrPolling(format!(
                    "failed to parse reviews JSON: {e}"
                ))
            })?;

            // Collect qualifying Codex bot reviews post-trigger with terminal state.
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
                let submitted_dt =
                    chrono::DateTime::parse_from_rfc3339(&submitted_str).map_err(|e| {
                        crate::d4_orchestration::D4OrchestrationError::PrPolling(format!(
                            "invalid review submitted_at: {e}"
                        ))
                    })?;
                if submitted_dt >= trigger_dt {
                    any_bot_activity = true;
                    let state = review.get("state").and_then(|s| s.as_str()).unwrap_or("");
                    if matches!(state, "APPROVED" | "CHANGES_REQUESTED" | "COMMENTED") {
                        qualifying.push(review);
                    }
                }
            }
            if let Some(latest) = find_latest_bot_review(&qualifying) {
                return Ok(PrReviewPollingOutput::ReviewFound(latest));
            }

            if head_commit.is_some() {
                // Check +1 reaction (primary zero-findings signal).
                if self.check_reaction_zero_findings(&repo_nwo, &pr, trigger_dt)? {
                    return Ok(PrReviewPollingOutput::ZeroFindings);
                }

                // Check stale-reaction + comment text fallback.
                let reactions_json =
                    self.reactions.list_reactions(&repo_nwo, &pr).map_err(|e| {
                        crate::d4_orchestration::D4OrchestrationError::PrPolling(e.to_string())
                    })?;
                let reactions = parse_paginated_json(&reactions_json).map_err(|e| {
                    crate::d4_orchestration::D4OrchestrationError::PrPolling(format!(
                        "failed to parse reactions JSON: {e}"
                    ))
                })?;
                let has_stale_reaction = reactions.iter().any(|r| {
                    let content = r.get("content").and_then(|c| c.as_str()).unwrap_or("");
                    let author = r
                        .get("user")
                        .and_then(|u| u.get("login"))
                        .and_then(|l| l.as_str())
                        .unwrap_or("");
                    content == "+1" && is_codex_bot(author)
                });

                if has_stale_reaction
                    && self.check_comment_zero_findings(&repo_nwo, &pr, trigger_dt)?
                {
                    return Ok(PrReviewPollingOutput::ZeroFindings);
                }
            }

            if !any_bot_activity {
                let comments_json =
                    self.comments.list_issue_comments(&repo_nwo, &pr).map_err(|e| {
                        crate::d4_orchestration::D4OrchestrationError::PrPolling(e.to_string())
                    })?;
                let comment_list = parse_paginated_json(&comments_json).map_err(|e| {
                    crate::d4_orchestration::D4OrchestrationError::PrPolling(format!(
                        "failed to parse comments JSON: {e}"
                    ))
                })?;
                for comment in &comment_list {
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
                    let created_dt =
                        chrono::DateTime::parse_from_rfc3339(&created_str).map_err(|e| {
                            crate::d4_orchestration::D4OrchestrationError::PrPolling(format!(
                                "invalid comment created_at: {e}"
                            ))
                        })?;
                    if created_dt >= trigger_dt {
                        any_bot_activity = true;
                        break;
                    }
                }
            }

            if elapsed_budget >= effective_timeout {
                break;
            }
            self.sleep.sleep(Duration::from_secs(interval_secs));
            elapsed_budget = elapsed_budget.saturating_add(interval_budget);
        }

        // Timeout recovery: look for a review on the exact HEAD commit SHA.
        if let Some(ref expected_commit) = head_commit {
            let recovery_reviews_json = self.reviews.list_reviews(&repo_nwo, &pr).map_err(|e| {
                crate::d4_orchestration::D4OrchestrationError::PrPolling(e.to_string())
            })?;
            let recovery_reviews = parse_paginated_json(&recovery_reviews_json).map_err(|e| {
                crate::d4_orchestration::D4OrchestrationError::PrPolling(format!(
                    "recovery: failed to parse reviews JSON: {e}"
                ))
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
                        && review_commit == expected_commit.as_str()
                })
                .collect();
            if let Some(recovered) = find_latest_bot_review(&recovery_refs) {
                return Ok(PrReviewPollingOutput::ReviewFound(recovered));
            }
        }

        Ok(PrReviewPollingOutput::Timeout)
    }
}

impl PrReviewPollingInteractor {
    fn check_reaction_zero_findings(
        &self,
        repo_nwo: &str,
        pr: &str,
        trigger_dt: chrono::DateTime<chrono::FixedOffset>,
    ) -> Result<bool, crate::d4_orchestration::D4OrchestrationError> {
        let reactions_json = self
            .reactions
            .list_reactions(repo_nwo, pr)
            .map_err(|e| crate::d4_orchestration::D4OrchestrationError::PrPolling(e.to_string()))?;
        let reactions = parse_paginated_json(&reactions_json).map_err(|e| {
            crate::d4_orchestration::D4OrchestrationError::PrPolling(format!(
                "failed to parse reactions JSON: {e}"
            ))
        })?;
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
            let created_dt = chrono::DateTime::parse_from_rfc3339(&created_str).map_err(|e| {
                crate::d4_orchestration::D4OrchestrationError::PrPolling(format!(
                    "invalid reaction created_at: {e}"
                ))
            })?;
            if created_dt >= trigger_dt {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn check_comment_zero_findings(
        &self,
        repo_nwo: &str,
        pr: &str,
        trigger_dt: chrono::DateTime<chrono::FixedOffset>,
    ) -> Result<bool, crate::d4_orchestration::D4OrchestrationError> {
        let comments_json = self
            .comments
            .list_issue_comments(repo_nwo, pr)
            .map_err(|e| crate::d4_orchestration::D4OrchestrationError::PrPolling(e.to_string()))?;
        let comments = parse_paginated_json(&comments_json).map_err(|e| {
            crate::d4_orchestration::D4OrchestrationError::PrPolling(format!(
                "failed to parse comments JSON: {e}"
            ))
        })?;
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
            let created_dt = chrono::DateTime::parse_from_rfc3339(&created_str).map_err(|e| {
                crate::d4_orchestration::D4OrchestrationError::PrPolling(format!(
                    "invalid comment created_at: {e}"
                ))
            })?;
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
}

// ---------------------------------------------------------------------------
// Tests (written first — TDD)
// ---------------------------------------------------------------------------
