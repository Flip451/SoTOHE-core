//! `pr` command family — CliApp impl methods.

use crate::{CliApp, CommandOutcome};

impl CliApp {
    /// Push the current track branch to origin.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn pr_push(&self, track_id: Option<String>) -> Result<CommandOutcome, String> {
        let _ = track_id;
        Err(String::from("not implemented"))
    }

    /// Create or reuse a PR for the current track branch.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn pr_ensure(
        &self,
        track_id: Option<String>,
        base: String,
    ) -> Result<CommandOutcome, String> {
        let _ = (track_id, base);
        Err(String::from("not implemented"))
    }

    /// Show current PR check status.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn pr_status(&self, pr: String) -> Result<CommandOutcome, String> {
        let _ = pr;
        Err(String::from("not implemented"))
    }

    /// Poll PR checks until they pass, then merge.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn pr_wait_and_merge(
        &self,
        pr: String,
        interval: u64,
        timeout: u64,
        method: String,
    ) -> Result<CommandOutcome, String> {
        let _ = (pr, interval, timeout, method);
        Err(String::from("not implemented"))
    }

    /// Post `@codex review` comment on a PR.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn pr_trigger_review(&self, pr: String) -> Result<CommandOutcome, String> {
        let _ = pr;
        Err(String::from("not implemented"))
    }

    /// Poll GitHub API for a Codex bot review.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn pr_poll_review(
        &self,
        pr: String,
        trigger_timestamp: String,
        interval: u64,
        timeout: u64,
    ) -> Result<CommandOutcome, String> {
        let _ = (pr, trigger_timestamp, interval, timeout);
        Err(String::from("not implemented"))
    }

    /// Full PR review cycle: push → ensure-pr → trigger → poll → parse → report.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn pr_review_cycle(
        &self,
        track_id: Option<String>,
        resume: bool,
    ) -> Result<CommandOutcome, String> {
        let _ = (track_id, resume);
        Err(String::from("not implemented"))
    }
}
