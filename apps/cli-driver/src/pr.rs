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
                to_outcome(self.service.review_cycle(track_id, resume))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

fn to_outcome(out: PrCommandOutput) -> CommandOutcome {
    CommandOutcome { stdout: out.stdout, stderr: out.stderr, exit_code: out.exit_code }
}
