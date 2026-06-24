//! PR command application service port (usecase layer).
//!
//! Provides a thin `PrCommandService` trait that the `cli_driver::PrDriver`
//! calls for all non-polling PR operations (push, ensure, status,
//! wait-and-merge, trigger-review, review-cycle). Polling is delegated to
//! the separate [`crate::pr_review_polling::PrReviewPollingService`].
//!
//! The function-pointer interactor pattern (mirroring `RunReviewInteractor`)
//! lets `cli_composition` inject the full infrastructure wiring without
//! violating the hexagonal boundary.

use std::sync::Arc;

// ── PrCommandOutput ───────────────────────────────────────────────────────────

/// Primitive output from a PR command operation.
///
/// Uses only stdlib types so the driver and usecase layers never import
/// infrastructure or domain types.
#[derive(Debug, Clone)]
pub struct PrCommandOutput {
    /// Optional stdout message.
    pub stdout: Option<String>,
    /// Optional stderr message.
    pub stderr: Option<String>,
    /// Exit code: 0 = success, non-zero = failure.
    pub exit_code: u8,
}

impl PrCommandOutput {
    /// Create a success output with optional message.
    #[must_use]
    pub fn success(msg: Option<String>) -> Self {
        Self { stdout: msg, stderr: None, exit_code: 0 }
    }

    /// Create a failure output with optional message.
    #[must_use]
    pub fn failure(msg: Option<String>) -> Self {
        Self { stdout: None, stderr: msg, exit_code: 1 }
    }

    /// Create an output with all fields specified.
    #[must_use]
    pub fn with_exit_code(stdout: Option<String>, stderr: Option<String>, exit_code: u8) -> Self {
        Self { stdout, stderr, exit_code }
    }
}

// ── PrCommandService ──────────────────────────────────────────────────────────

/// Application service (primary port) for the PR command family.
///
/// Covers all non-polling operations: push, ensure, status,
/// wait-and-merge, trigger-review, and review-cycle. The polling operation
/// (`pr_poll_review`) is handled by the separate
/// [`crate::pr_review_polling::PrReviewPollingService`].
///
/// All parameters use stdlib types only so that `cli_driver` never imports
/// `infrastructure` or `domain` types (CN-01 / architecture-rules.json).
pub trait PrCommandService: Send + Sync {
    /// Push the current track branch to origin.
    ///
    /// # Errors
    /// Returns a human-readable error string on failure.
    fn push(&self, track_id: Option<String>) -> PrCommandOutput;

    /// Create or reuse a PR for the current track branch.
    ///
    /// # Errors
    /// Returns a human-readable error string on failure.
    fn ensure(&self, track_id: Option<String>, base: String) -> PrCommandOutput;

    /// Show current PR check status.
    fn status(&self, pr: String) -> PrCommandOutput;

    /// Poll PR checks until they pass, then merge.
    fn wait_and_merge(
        &self,
        pr: String,
        interval: u64,
        timeout: u64,
        method: String,
    ) -> PrCommandOutput;

    /// Post `@codex review` comment on a PR.
    fn trigger_review(&self, pr: String) -> PrCommandOutput;

    /// Poll for a Codex review completion after triggering.
    fn poll_review(
        &self,
        pr: String,
        trigger_timestamp: String,
        interval: u64,
        timeout: u64,
    ) -> PrCommandOutput;

    /// Full PR review cycle: push → ensure-pr → trigger → poll → parse → report.
    fn review_cycle(&self, track_id: Option<String>, resume: bool) -> PrCommandOutput;
}

// ── PrCommandInteractor ───────────────────────────────────────────────────────

/// Function bundle injected by `cli_composition` to implement [`PrCommandService`].
///
/// Mirrors the `RunReviewInteractor` function-pointer pattern: each operation
/// is a closure supplied from `cli_composition` so that `usecase` never imports
/// `infrastructure` directly.
pub struct PrCommandInteractor {
    push_fn: Arc<dyn Fn(Option<String>) -> PrCommandOutput + Send + Sync>,
    ensure_fn: Arc<dyn Fn(Option<String>, String) -> PrCommandOutput + Send + Sync>,
    status_fn: Arc<dyn Fn(String) -> PrCommandOutput + Send + Sync>,
    wait_and_merge_fn: Arc<dyn Fn(String, u64, u64, String) -> PrCommandOutput + Send + Sync>,
    trigger_review_fn: Arc<dyn Fn(String) -> PrCommandOutput + Send + Sync>,
    poll_review_fn: Arc<dyn Fn(String, String, u64, u64) -> PrCommandOutput + Send + Sync>,
    review_cycle_fn: Arc<dyn Fn(Option<String>, bool) -> PrCommandOutput + Send + Sync>,
}

impl PrCommandInteractor {
    /// Create a new interactor with injected operation functions.
    #[must_use]
    pub fn new(
        push_fn: Arc<dyn Fn(Option<String>) -> PrCommandOutput + Send + Sync>,
        ensure_fn: Arc<dyn Fn(Option<String>, String) -> PrCommandOutput + Send + Sync>,
        status_fn: Arc<dyn Fn(String) -> PrCommandOutput + Send + Sync>,
        wait_and_merge_fn: Arc<dyn Fn(String, u64, u64, String) -> PrCommandOutput + Send + Sync>,
        trigger_review_fn: Arc<dyn Fn(String) -> PrCommandOutput + Send + Sync>,
        poll_review_fn: Arc<dyn Fn(String, String, u64, u64) -> PrCommandOutput + Send + Sync>,
        review_cycle_fn: Arc<dyn Fn(Option<String>, bool) -> PrCommandOutput + Send + Sync>,
    ) -> Self {
        Self {
            push_fn,
            ensure_fn,
            status_fn,
            wait_and_merge_fn,
            trigger_review_fn,
            poll_review_fn,
            review_cycle_fn,
        }
    }
}

impl PrCommandService for PrCommandInteractor {
    fn push(&self, track_id: Option<String>) -> PrCommandOutput {
        (self.push_fn)(track_id)
    }

    fn ensure(&self, track_id: Option<String>, base: String) -> PrCommandOutput {
        (self.ensure_fn)(track_id, base)
    }

    fn status(&self, pr: String) -> PrCommandOutput {
        (self.status_fn)(pr)
    }

    fn wait_and_merge(
        &self,
        pr: String,
        interval: u64,
        timeout: u64,
        method: String,
    ) -> PrCommandOutput {
        (self.wait_and_merge_fn)(pr, interval, timeout, method)
    }

    fn trigger_review(&self, pr: String) -> PrCommandOutput {
        (self.trigger_review_fn)(pr)
    }

    fn poll_review(
        &self,
        pr: String,
        trigger_timestamp: String,
        interval: u64,
        timeout: u64,
    ) -> PrCommandOutput {
        (self.poll_review_fn)(pr, trigger_timestamp, interval, timeout)
    }

    fn review_cycle(&self, track_id: Option<String>, resume: bool) -> PrCommandOutput {
        (self.review_cycle_fn)(track_id, resume)
    }
}
