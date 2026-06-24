//! `pr` command family — primary adapter driver.
//!
//! `PrDriver` holds an injected [`usecase::pr::PrCommandService`] and exposes
//! `handle(input) -> CommandOutcome`. All PR operations (push, ensure, status,
//! wait-and-merge, trigger-review, poll-review, review-cycle) delegate through
//! the service so `cli_driver` never imports `infrastructure` or `domain`.

use std::sync::Arc;

use usecase::pr::{PrCommandOutput, PrCommandService};

use crate::render::CommandOutcome;

// ---------------------------------------------------------------------------
// Input type
// ---------------------------------------------------------------------------

/// Typed input for the `pr` command family.
pub enum PrInput {
    /// Push the current track branch to origin.
    Push {
        /// Optional track ID override (auto-detected from branch if `None`).
        track_id: Option<String>,
    },
    /// Create or reuse a PR for the current track branch.
    Ensure {
        /// Optional track ID override.
        track_id: Option<String>,
        /// Base branch for the PR (e.g. `"main"`).
        base: String,
    },
    /// Show current PR check status.
    Status {
        /// PR number or identifier string.
        pr: String,
    },
    /// Poll PR checks until they pass, then merge.
    WaitAndMerge {
        /// PR number or identifier string.
        pr: String,
        /// Poll interval in seconds.
        interval: u64,
        /// Maximum wait time in seconds before giving up.
        timeout: u64,
        /// Merge method: `"merge"`, `"squash"`, or `"rebase"`.
        method: String,
    },
    /// Post `@codex review` comment on a PR.
    TriggerReview {
        /// PR number or identifier string.
        pr: String,
    },
    /// Poll for a Codex review completion after triggering.
    PollReview {
        /// PR number or identifier string.
        pr: String,
        /// Timestamp at which the review trigger was posted (ISO-8601).
        trigger_timestamp: String,
        /// Poll interval in seconds.
        interval: u64,
        /// Maximum wait time in seconds before giving up.
        timeout: u64,
    },
    /// Full PR review cycle: push → ensure-pr → trigger → poll → parse → report.
    ReviewCycle {
        /// Optional track ID override.
        track_id: Option<String>,
        /// Resume from a saved trigger-state file rather than starting fresh.
        resume: bool,
    },
}

// ---------------------------------------------------------------------------
// Driver
// ---------------------------------------------------------------------------

/// Primary adapter driver for the `pr` command family.
///
/// Holds an injected [`PrCommandService`]; exposes `handle(input) -> CommandOutcome`.
pub struct PrDriver {
    service: Arc<dyn PrCommandService>,
}

impl PrDriver {
    /// Create a new `PrDriver` with the given service.
    pub fn new(service: Arc<dyn PrCommandService>) -> Self {
        Self { service }
    }

    /// Handle a pr command.
    pub fn handle(&self, input: PrInput) -> CommandOutcome {
        match input {
            PrInput::Push { track_id } => to_outcome(self.service.push(track_id)),
            PrInput::Ensure { track_id, base } => to_outcome(self.service.ensure(track_id, base)),
            PrInput::Status { pr } => to_outcome(self.service.status(pr)),
            PrInput::WaitAndMerge { pr, interval, timeout, method } => {
                to_outcome(self.service.wait_and_merge(pr, interval, timeout, method))
            }
            PrInput::TriggerReview { pr } => to_outcome(self.service.trigger_review(pr)),
            PrInput::PollReview { pr, trigger_timestamp, interval, timeout } => {
                to_outcome(self.service.poll_review(pr, trigger_timestamp, interval, timeout))
            }
            PrInput::ReviewCycle { track_id, resume } => {
                render_review_cycle(self.service.review_cycle(track_id, resume))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn to_outcome(out: PrCommandOutput) -> CommandOutcome {
    CommandOutcome { stdout: out.stdout, stderr: out.stderr, exit_code: out.exit_code }
}

/// Render the structured JSON emitted by the `review_cycle` service into a
/// human-readable `CommandOutcome`.
///
/// `cli_composition` returns a JSON-encoded envelope in `out.stdout` so that
/// rendering stays in `cli_driver` (Fix B / presentation-layer separation).
/// Two envelope types are handled:
/// - `{"type":"zero_findings","pr_number":"<n>"}` → PASS banner.
/// - `{"type":"review_found",...}` → full review summary.
///
/// Any failure (non-zero exit or missing/malformed JSON) is passed through
/// unchanged.
fn render_review_cycle(out: PrCommandOutput) -> CommandOutcome {
    if out.exit_code != 0 {
        return CommandOutcome { stdout: out.stdout, stderr: out.stderr, exit_code: out.exit_code };
    }
    let Some(ref raw) = out.stdout else {
        return CommandOutcome { stdout: out.stdout, stderr: out.stderr, exit_code: out.exit_code };
    };
    let Ok(val) = serde_json::from_str::<serde_json::Value>(raw) else {
        // Fallback: pass through as-is if the JSON is not parseable.
        return CommandOutcome { stdout: out.stdout, stderr: out.stderr, exit_code: out.exit_code };
    };
    let kind = val.get("type").and_then(|v| v.as_str()).unwrap_or("");
    match kind {
        "zero_findings" => {
            let pr_number = val.get("pr_number").and_then(|v| v.as_str()).unwrap_or("?");
            let stdout = format!(
                "\n=== PR Review Result: PASS ===\nPR: #{pr_number}\n\
                 Zero findings detected (bot signalled no issues)."
            );
            CommandOutcome::success(Some(stdout))
        }
        "review_found" => {
            let pr_number = val.get("pr_number").and_then(|v| v.as_str()).unwrap_or("?");
            let summary = format_review_summary(pr_number, &val);
            // ReviewFound always exits 0 (D1/AC-09): pass/fail judgment is
            // delegated to the calling agent; Rust no longer gates on findings.
            CommandOutcome::success(Some(summary))
        }
        _ => {
            // Unknown envelope type — pass through unchanged.
            CommandOutcome { stdout: out.stdout, stderr: out.stderr, exit_code: out.exit_code }
        }
    }
}

/// Format a `"review_found"` JSON envelope into a human-readable review summary.
///
/// This rendering function intentionally lives in `cli_driver` — it formats
/// infrastructure-fetched review data for terminal display (Fix B).
fn format_review_summary(pr: &str, val: &serde_json::Value) -> String {
    let review_id = val.get("review_id").and_then(|v| v.as_u64()).unwrap_or(0);
    let state = val.get("state").and_then(|v| v.as_str()).unwrap_or("");
    let body = val.get("body").and_then(|v| v.as_str()).unwrap_or("");
    let inline_count = val.get("inline_comment_count").and_then(|v| v.as_u64()).unwrap_or(0);
    let findings = val.get("findings").and_then(|v| v.as_array()).map(Vec::as_slice).unwrap_or(&[]);

    let mut lines = Vec::new();
    lines.push(String::new());
    lines.push("=== PR Review Result: ReviewFound ===".to_owned());
    lines.push(format!("PR: #{pr}"));
    lines.push(format!("Review ID: {review_id}"));
    lines.push(format!("State: {state}"));
    lines.push(format!("Inline comments: {inline_count}"));

    if !body.is_empty() {
        lines.push(String::new());
        lines.push("Review Body:".to_owned());
        lines.push(body.to_owned());
    }

    if !findings.is_empty() {
        lines.push(String::new());
        lines.push("Inline Comments:".to_owned());
        for (i, f) in findings.iter().enumerate() {
            let path = f.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let line = f.get("line").and_then(|v| v.as_u64());
            let f_body = f.get("body").and_then(|v| v.as_str()).unwrap_or("");
            let location = if !path.is_empty() && line.is_some() {
                format!("{}:{}", path, line.unwrap_or(0))
            } else if !path.is_empty() {
                path.to_owned()
            } else {
                "(no location)".to_owned()
            };
            lines.push(format!("  {}. {}: {}", i + 1, location, f_body));
        }
    }
    lines.join("\n")
}
