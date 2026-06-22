// STAGED FOR T021 — not yet compiled; Cargo.toml + workspace member added atomically in T021 per CN-06.
//
//! `pr` command family — primary adapter driver.
//!
//! `PrDriver` holds injected use-case interactors and exposes
//! `handle(input) -> CommandOutcome`.  The render helpers here mirror those in
//! `apps/cli-composition/src/pr.rs` (lines 85-113 `pr_ensure` banner,
//! lines 104-113 `create_pr` banner, lines 129-143 `pr_status` banner,
//! lines 471-484 `pr_review_cycle` result) and `pr/poll.rs`
//! (lines 524-557 `format_review_summary`);
//! T021 removes the `cli_composition` duplicates when the live path is flipped.

// TODO(T021): add use-case + infrastructure imports once Cargo.toml is materialized.
// use std::path::PathBuf;
// use std::thread;
// use std::time::{Duration, Instant};
// use infrastructure::gh_cli::{GhClient as _, SystemGhClient};
// use infrastructure::git_cli::{GitRepository as _, SystemGitRepo};
// use usecase::pr_workflow::{WaitDecision, CheckSummary, decide_wait_action, pr_title};
// use usecase::pr_review_polling::{
//     PrReviewPollingCommand, PrReviewPollingOutput, PrReviewPollingService as _,
// };

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
/// Holds injected use-case interactors; exposes `handle(input) -> CommandOutcome`.
pub struct PrDriver {
    // TODO(T021): inject use-case interactors here (currently this family has
    // no injectable adapter dependencies — infrastructure functions are called
    // inline, same as cli_composition::PrCompositionRoot).
}

impl PrDriver {
    /// Create a new `PrDriver`.
    ///
    /// TODO(T021): accept injected interactors as parameters once the crate
    /// dependency graph is materialized.
    pub fn new() -> Self {
        Self {}
    }

    /// Handle a pr command.
    ///
    /// TODO(T021): wire real use-case invocation once Cargo.toml is materialized.
    pub fn handle(&self, input: PrInput) -> CommandOutcome {
        match input {
            PrInput::Push { track_id } => self.pr_push(track_id),
            PrInput::Ensure { track_id, base } => self.pr_ensure(track_id, base),
            PrInput::Status { pr } => self.pr_status(pr),
            PrInput::WaitAndMerge { pr, interval, timeout, method } => {
                self.pr_wait_and_merge(pr, interval, timeout, method)
            }
            PrInput::TriggerReview { pr } => self.pr_trigger_review(pr),
            PrInput::PollReview { pr, trigger_timestamp, interval, timeout } => {
                self.pr_poll_review(pr, trigger_timestamp, interval, timeout)
            }
            PrInput::ReviewCycle { track_id, resume } => self.pr_review_cycle(track_id, resume),
        }
    }

    // -----------------------------------------------------------------------
    // Render helpers (logic duplicated from cli_composition/src/pr.rs
    // and pr/poll.rs; T021 removes the cli_composition copies).
    // -----------------------------------------------------------------------

    fn pr_push(&self, _track_id: Option<String>) -> CommandOutcome {
        // TODO(T021): resolve branch context via resolve_branch_context, discover git repo,
        // push branch via SystemGitRepo::push_branch, then format:
        //   format!("[OK] Pushed {}", ctx.branch)
        // Mirrors cli_composition/src/pr.rs PrCompositionRoot::pr_push.
        CommandOutcome::success(None)
    }

    fn pr_ensure(&self, _track_id: Option<String>, _base: String) -> CommandOutcome {
        // TODO(T021): resolve branch context, look for existing PR via SystemGhClient::find_open_pr:
        //   Some(pr) → "[OK] Reusing existing PR #{pr}"
        //   None     → ensure_pr_body_file + pr_title + SystemGhClient::create_pr
        //               "[OK] Created PR #{pr}"
        // Mirrors cli_composition/src/pr.rs PrCompositionRoot::pr_ensure (lines 85-113).
        CommandOutcome::success(None)
    }

    fn pr_status(&self, _pr: String) -> CommandOutcome {
        // TODO(T021): fetch checks via SystemGhClient::pr_checks, then build lines:
        //   format!("PR: {url}")
        //   CheckSummary::AllPassed  → "[OK] All checks passed."  (exit 0)
        //   CheckSummary::Failed(ns) → format!("[FAIL] Failed checks: {}", ns.join(", "))  (exit 1)
        //   CheckSummary::Pending(ns)→ format!("[PENDING] Waiting: {}", ns.join(", "))  (exit 2)
        // Mirrors cli_composition/src/pr.rs PrCompositionRoot::pr_status (lines 129-143).
        CommandOutcome::success(None)
    }

    fn pr_wait_and_merge(
        &self,
        _pr: String,
        _interval: u64,
        _timeout: u64,
        _method: String,
    ) -> CommandOutcome {
        // TODO(T021): fetch PR head branch, git fetch with explicit refspec, read task
        // completion via check_tasks_resolved_from_git_ref, load SignalGateMatrix from
        // branch ref, then poll decide_wait_action in a loop:
        //   WaitDecision::Merge     → client.merge_pr(pr, method)
        //   WaitDecision::Fail(msg) → CommandOutcome::failure(Some(msg))
        //   WaitDecision::Wait{..}  → println!("[{elapsed}s] Pending: …") + thread::sleep
        // Mirrors cli_composition/src/pr.rs PrCompositionRoot::pr_wait_and_merge.
        CommandOutcome::success(None)
    }

    fn pr_trigger_review(&self, _pr: String) -> CommandOutcome {
        // TODO(T021): resolve agent profiles, validate reviewer provider, post
        // "@codex review" comment via SystemGhClient, save trigger state, then format:
        //   "[OK] Triggered @codex review on PR #{pr}"
        // Mirrors cli_composition/src/pr.rs PrCompositionRoot::pr_trigger_review.
        CommandOutcome::success(None)
    }

    fn pr_poll_review(
        &self,
        _pr: String,
        _trigger_timestamp: String,
        _interval: u64,
        _timeout: u64,
    ) -> CommandOutcome {
        // TODO(T021): read HEAD commit, build PrReviewPollingCommand, invoke
        // PrReviewPollingInteractor::poll:
        //   ReviewFound(v) → serde_json::to_string(&review)
        //   ZeroFindings   → r#"{"verdict":"zero_findings","findings":[]}"#
        //   Timeout        → CommandOutcome::failure(None)
        // Mirrors cli_composition/src/pr.rs PrCompositionRoot::pr_poll_review.
        CommandOutcome::success(None)
    }

    fn pr_review_cycle(&self, _track_id: Option<String>, _resume: bool) -> CommandOutcome {
        // TODO(T021): load agent profiles, resolve pr-reviewer, detect branch, trigger
        // or resume review state, invoke PrReviewPollingInteractor::poll, then map result:
        //   ZeroFindings  → format!("\n=== PR Review Result: PASS ===\nPR: #{pr}\n\
        //                    Zero findings detected (bot signalled no issues).")
        //   ReviewFound   → format_review_summary(&pr_number, &parsed)
        //   Timeout       → CommandOutcome::failure(None)
        // Mirrors cli_composition/src/pr.rs PrCompositionRoot::pr_review_cycle (lines 469-484).
        CommandOutcome::success(None)
    }
}

impl Default for PrDriver {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Render helpers (duplicated from cli_composition/src/pr/poll.rs lines 524-557;
// T021 removes the cli_composition copy and moves these to cli_driver::render).
// ---------------------------------------------------------------------------

/// Format the PR review summary from a parsed review result.
///
/// Mirrors `cli_composition::pr::poll::format_review_summary`
/// (poll.rs lines 524-557).
///
/// Temporary stand-in for `usecase::pr_review::PrReviewFinding`.
///
/// TODO(T021): remove when `format_review_summary_stub` takes the real
/// `PrReviewResult`.
#[allow(dead_code)]
struct PrReviewFindingStub {
    path: String,
    line: Option<u32>,
    body: String,
}

/// Temporary stand-in for `usecase::pr_review::PrReviewResult`.
///
/// TODO(T021): replace with `usecase::pr_review::PrReviewResult` once the
/// dependency graph is materialized.
#[allow(dead_code)]
struct PrReviewResultStub {
    review_id: String,
    state: String,
    body: String,
    findings: Vec<PrReviewFindingStub>,
    inline_comment_count: usize,
}

/// TODO(T021): call real `usecase::pr_review::PrReviewResult` once the
/// dependency graph is materialized.
#[allow(dead_code)]
fn format_review_summary_stub(pr: &str, result: &PrReviewResultStub) -> String {
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
