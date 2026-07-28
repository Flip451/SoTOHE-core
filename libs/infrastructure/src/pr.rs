//! System PR command adapter.
//!
//! Implements the full PR workflow behind the typed usecase port.
//!
//! Private polling and review helpers are in `poll` (see `pr/poll.rs`).

mod poll;
mod poll_adapters;

use std::fs;
use std::thread;
use std::time::{Duration, Instant};

use thiserror::Error;
use usecase::pr::{PrCommand, PrCommandOutput, PrCommandPort, PrReviewCycleMode};

#[derive(Debug, Error)]
pub(crate) enum PrCommandError {
    #[error("{0}")]
    ConfigLoad(String),
    #[error("{0}")]
    AdapterInit(String),
    #[error("{0}")]
    WiringFailed(String),
    #[error("{0}")]
    Usecase(String),
    #[error("{0}")]
    Infrastructure(String),
}

type CompositionError = PrCommandError;
type CommandOutcome = PrCommandOutput;

// ── Per-context composition root ──────────────────────────────────────────────

/// System secondary adapter for the `pr` command family.
pub struct SystemPrCommandAdapter;

impl SystemPrCommandAdapter {
    /// Creates a system-backed PR command adapter.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for SystemPrCommandAdapter {
    fn default() -> Self {
        Self::new()
    }
}

use poll::{
    PollReviewResult, checks_summary, cleanup_trigger_state, ensure_pr_body_file,
    format_review_summary, parse_review, resolve_branch_context, resume_trigger_state,
    trigger_new_review,
};
use poll_adapters::make_polling_interactor;

// ---------------------------------------------------------------------------
// SystemPrCommandAdapter implementations
// ---------------------------------------------------------------------------

impl SystemPrCommandAdapter {
    /// Push the current track branch to origin.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    fn pr_push(&self, track_id: Option<String>) -> Result<PrCommandOutput, PrCommandError> {
        use infrastructure::git_cli::SystemGitRepo;

        let ctx = resolve_branch_context(track_id.as_deref())?;
        let repo =
            SystemGitRepo::discover().map_err(|e| CompositionError::AdapterInit(e.to_string()))?;
        println!("Pushing {} to origin...", ctx.branch);
        repo.push_branch(&ctx.branch)
            .map_err(|e| CompositionError::Infrastructure(e.to_string()))?;
        let stdout = format!("[OK] Pushed {}", ctx.branch);
        Ok(CommandOutcome::success(Some(stdout)))
    }

    /// Create or reuse a PR for the current track branch.
    ///
    /// `base` is the PR base (merge-target) branch. An explicit non-empty value
    /// always wins; an empty string is the "omitted" sentinel used by
    /// `apps/cli/src/commands/pr.rs` (no valid git branch name is empty), in
    /// which case the current track's `branch_strategy_snapshot.merge_target`
    /// is resolved via [`usecase::branch_strategy::BranchStrategyPort`] (T011 /
    /// D4: post-init operations read the per-track snapshot, never the global
    /// config).
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails, or when
    /// `base` is omitted and the active track/its metadata cannot be resolved.
    fn pr_ensure(
        &self,
        track_id: Option<String>,
        base: String,
    ) -> Result<CommandOutcome, CompositionError> {
        use infrastructure::gh_cli::{GhClient as _, SystemGhClient};
        use usecase::branch_strategy::BranchStrategyPort as _;
        use usecase::pr_workflow::pr_title;

        let ctx = resolve_branch_context(track_id.as_deref())?;
        let base = if base.is_empty() {
            let port = branch_strategy_port_for_track(&ctx.track_id)?;
            port.merge_target().to_owned()
        } else {
            base
        };
        let client = SystemGhClient;

        match client.find_open_pr(&ctx.branch, &base) {
            Ok(Some(pr)) => {
                return Ok(CommandOutcome::success(Some(format!(
                    "[OK] Reusing existing PR #{pr}"
                ))));
            }
            Ok(None) => {}
            Err(err) => {
                eprintln!("[ERROR] {err}");
                return Ok(CommandOutcome::failure(None));
            }
        }

        let body_file = ensure_pr_body_file(&ctx).map_err(|e| {
            eprintln!("[ERROR] {e}");
            e
        })?;
        let title = pr_title(&ctx);
        match client.create_pr(&ctx.branch, &base, &title, &body_file) {
            Ok(pr) => {
                let _ = fs::remove_file(&body_file);
                Ok(CommandOutcome::success(Some(format!("[OK] Created PR #{pr}"))))
            }
            Err(err) => {
                let _ = fs::remove_file(&body_file);
                eprintln!("[ERROR] {err}");
                Ok(CommandOutcome::failure(None))
            }
        }
    }

    /// Show current PR check status.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    fn pr_status(&self, pr: String) -> Result<CommandOutcome, CompositionError> {
        use infrastructure::gh_cli::{GhClient as _, SystemGhClient};
        use usecase::pr_workflow::CheckSummary;

        let client = SystemGhClient;
        let checks =
            client.pr_checks(&pr).map_err(|e| CompositionError::Infrastructure(e.to_string()))?;
        let url = client.pr_url(&pr);
        let mut lines = vec![format!("PR: {url}")];
        let exit_code = match checks_summary(&checks) {
            CheckSummary::AllPassed => {
                lines.push("[OK] All checks passed.".to_owned());
                0u8
            }
            CheckSummary::Failed(names) => {
                lines.push(format!("[FAIL] Failed checks: {}", names.join(", ")));
                1u8
            }
            CheckSummary::Pending(names) => {
                lines.push(format!("[PENDING] Waiting: {}", names.join(", ")));
                2u8
            }
        };
        Ok(CommandOutcome { stdout: Some(lines.join("\n")), stderr: None, exit_code })
    }

    /// Poll PR checks until they pass, then merge.
    ///
    /// `method` is the merge method (`"merge"` / `"squash"` / `"rebase"`). An
    /// explicit non-empty value always wins; an empty string is the "omitted"
    /// sentinel used by `apps/cli/src/commands/pr.rs`, in which case the PR's
    /// track `branch_strategy_snapshot.merge_method` is resolved via
    /// [`usecase::branch_strategy::BranchStrategyPort`] (T011).
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails, or when
    /// `method` is omitted and the PR's track metadata cannot be resolved.
    fn pr_wait_and_merge(
        &self,
        pr: String,
        interval: u64,
        timeout: u64,
        method: String,
    ) -> Result<CommandOutcome, CompositionError> {
        use infrastructure::gh_cli::{GhClient as _, SystemGhClient};
        use infrastructure::git_cli::SystemGitRepo;
        use usecase::branch_strategy::BranchStrategyPort as _;
        use usecase::pr_workflow::{WaitDecision, decide_wait_action};

        let client = SystemGhClient;
        let branch = client
            .pr_head_branch(&pr)
            .map_err(|e| CompositionError::Infrastructure(e.to_string()))?;
        let repo =
            SystemGitRepo::discover().map_err(|e| CompositionError::AdapterInit(e.to_string()))?;

        // Fetch and read the PR branch metadata through PrGitInteractor before
        // any gate reads from origin/<branch>. This preserves the pre-track
        // "refresh remote ref first" behavior without invoking git primitives
        // directly from the composition root.
        let track_id = branch.strip_prefix("track/").unwrap_or(&branch);
        let branch_strategy = branch_strategy_port_for_pr_ref(&repo, &branch, track_id)?;
        let method = if method.is_empty() {
            merge_method_to_arg(branch_strategy.merge_method()).to_owned()
        } else {
            method
        };

        let reader = infrastructure::verify::merge_gate_adapter::GitShowTrackBlobReader::new(
            repo.root().to_path_buf(),
        );

        let task_outcome =
            usecase::task_completion::check_tasks_resolved_from_git_ref(&branch, &reader);
        if task_outcome.has_errors() {
            let mut lines = Vec::new();
            for finding in task_outcome.findings() {
                lines.push(format!("[BLOCKED] {}", finding.message()));
            }
            lines.push("Run track-transition to mark tasks as done before merging.".to_owned());
            return Ok(CommandOutcome {
                stdout: None,
                stderr: Some(lines.join("\n")),
                exit_code: 1,
            });
        }

        // Load SignalGateMatrix from `.harness/config/signal-gates.json` on the PR
        // branch via `git show origin/<branch>:.harness/config/signal-gates.json`.
        // Reading from the branch ref (not the local worktree) ensures that the gate
        // matrix is the one committed on the PR — a locally relaxed config cannot
        // silently bypass the merge gate.
        let gate_matrix =
            match infrastructure::verify::signal_gates_config::load_signal_gates_config_from_branch(
                repo.root(),
                &branch,
            ) {
                Ok(matrix) => matrix,
                Err(e) => {
                    return Ok(CommandOutcome {
                        stdout: None,
                        stderr: Some(format!(
                            "[BLOCKED] failed to load signal-gates config from branch '{branch}': {e}"
                        )),
                        exit_code: 1,
                    });
                }
            };

        let gate_outcome =
            usecase::merge_gate::check_strict_merge_gate(&branch, &reader, &gate_matrix);
        if gate_outcome.has_errors() {
            let mut lines = vec!["[BLOCKED] strict spec signal gate failed:".to_owned()];
            for finding in gate_outcome.findings() {
                lines.push(format!("[BLOCKED] {}", finding.message()));
            }
            return Ok(CommandOutcome {
                stdout: None,
                stderr: Some(lines.join("\n")),
                exit_code: 1,
            });
        }

        let url = client.pr_url(&pr);
        println!("PR: {url}");
        println!("Polling checks every {interval}s (timeout {timeout}s)...");

        let start = Instant::now();
        loop {
            let elapsed = start.elapsed().as_secs();
            let checks = client
                .pr_checks(&pr)
                .map_err(|e| CompositionError::Infrastructure(e.to_string()))?;
            match decide_wait_action(checks_summary(&checks), elapsed, timeout, interval) {
                WaitDecision::MergeNow => {
                    println!("[OK] All checks passed. Merging...");
                    client
                        .merge_pr(&pr, &method)
                        .map_err(|e| CompositionError::Infrastructure(e.to_string()))?;
                    return Ok(CommandOutcome::success(Some(format!(
                        "[OK] PR #{pr} merged ({method})."
                    ))));
                }
                WaitDecision::FailChecks(names) => {
                    println!("[FAIL] Checks failed: {}", names.join(", "));
                    println!("Fix the failures and push again.");
                    return Ok(CommandOutcome::failure(None));
                }
                WaitDecision::Timeout(names) => {
                    println!("[TIMEOUT] Still pending after {timeout}s: {}", names.join(", "));
                    return Ok(CommandOutcome::failure(None));
                }
                WaitDecision::Wait { pending, delay_seconds } => {
                    println!(
                        "  [{elapsed}s] Pending: {} (retry in {delay_seconds}s)",
                        pending.join(", ")
                    );
                    thread::sleep(Duration::from_secs(delay_seconds));
                }
            }
        }
    }

    /// Post `@codex review` comment on a PR.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    fn pr_trigger_review(&self, pr: String) -> Result<CommandOutcome, CompositionError> {
        use infrastructure::agent_profiles::{
            AGENT_PROFILES_PATH, AgentProfiles, ResolvedExecution, RoundType,
        };
        use infrastructure::gh_cli::{GhClient as _, SystemGhClient};
        use infrastructure::git_cli::SystemGitRepo;
        use usecase::dry_write_driver::CapabilityName;

        let git_repo =
            SystemGitRepo::discover().map_err(|e| CompositionError::AdapterInit(e.to_string()))?;
        let profiles_path = git_repo.root().join(AGENT_PROFILES_PATH);
        let profiles = AgentProfiles::load(git_repo.root(), &profiles_path)
            .map_err(|e| CompositionError::ConfigLoad(format!("{e}")))?;
        let capability = CapabilityName::try_new("pr-reviewer")
            .map_err(|error| CompositionError::WiringFailed(error.to_string()))?;
        let resolved = profiles
            .resolve_execution(&capability, RoundType::Final)
            .map_err(|error| CompositionError::WiringFailed(error.to_string()))?;
        let ResolvedExecution::HostedService { provider } = resolved else {
            return Err(CompositionError::WiringFailed(
                "pr-reviewer must use a hosted-service execution profile".to_owned(),
            ));
        };
        usecase::pr_review::validate_reviewer_provider(provider.as_str())
            .map_err(|e| CompositionError::WiringFailed(e.to_string()))?;

        let client = SystemGhClient;
        let repo =
            client.repo_nwo().map_err(|e| CompositionError::Infrastructure(e.to_string()))?;
        let response = client
            .post_issue_comment(&repo, &pr, "@codex review")
            .map_err(|e| CompositionError::Infrastructure(e.to_string()))?;

        let created_at = serde_json::from_str::<serde_json::Value>(&response)
            .ok()
            .and_then(|v| v.get("created_at")?.as_str().map(String::from))
            .unwrap_or_default();

        if created_at.is_empty() {
            return Err(CompositionError::Infrastructure(
                "could not determine trigger timestamp from API response".to_owned(),
            ));
        }

        let stdout = format!(
            "[OK] Posted '@codex review' on PR #{pr} at {created_at}\nTRIGGER_TIMESTAMP={created_at}"
        );
        Ok(CommandOutcome::success(Some(stdout)))
    }

    /// Poll GitHub API for a Codex bot review.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    fn pr_poll_review(
        &self,
        pr: String,
        trigger_timestamp: String,
        interval: u64,
        timeout: u64,
    ) -> Result<CommandOutcome, CompositionError> {
        use std::sync::Arc;

        use infrastructure::FsGitWorkflowAdapter;
        use infrastructure::gh_cli::{GhClient as _, SystemGhClient};
        use usecase::git_workflow::{GitPrimitivePort, PrGitInteractor};
        use usecase::pr_review_polling::{
            PrReviewPollingCommand, PrReviewPollingOutput, PrReviewPollingService as _,
        };

        // Route HEAD resolution through the usecase PrGitInteractor (T007).
        let head = {
            let port: Arc<dyn GitPrimitivePort> = Arc::new(FsGitWorkflowAdapter::new());
            let interactor = PrGitInteractor::new(port);
            interactor.resolve_head().ok().flatten().map(|h| h.as_ref().to_owned())
        };

        let repo_nwo = SystemGhClient
            .repo_nwo()
            .map_err(|e| CompositionError::Infrastructure(e.to_string()))?;
        let bounded_timeout = timeout.min(86400);
        let max_iterations = match (bounded_timeout, interval) {
            (0, _) => 0,
            (_, 0) => 1,
            (timeout, interval) => 1 + (timeout - 1) / interval,
        };

        let interactor = make_polling_interactor();
        let cmd = PrReviewPollingCommand {
            pr: pr.clone(),
            repo_nwo,
            trigger_timestamp,
            interval_secs: interval,
            max_iterations,
            head_commit: head,
        };

        match interactor.poll(cmd).map_err(|e| CompositionError::Usecase(e.to_string()))? {
            PrReviewPollingOutput::ReviewFound(review) => {
                let review_str = serde_json::to_string(&review).unwrap_or_default();
                Ok(CommandOutcome::success(Some(review_str)))
            }
            PrReviewPollingOutput::ZeroFindings => Ok(CommandOutcome::success(Some(
                r#"{"verdict":"zero_findings","findings":[]}"#.to_owned(),
            ))),
            PrReviewPollingOutput::Timeout => Ok(CommandOutcome::failure(None)),
        }
    }

    /// Full PR review cycle: push → ensure-pr → trigger → poll → parse → report.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    fn pr_review_cycle(
        &self,
        track_id: Option<String>,
        resume: bool,
    ) -> Result<CommandOutcome, CompositionError> {
        use infrastructure::agent_profiles::{
            AGENT_PROFILES_PATH, AgentProfiles, ResolvedExecution, RoundType,
        };
        use infrastructure::gh_cli::{GhClient as _, SystemGhClient};
        use infrastructure::git_cli::SystemGitRepo;
        use usecase::dry_write_driver::CapabilityName;

        let repo =
            SystemGitRepo::discover().map_err(|e| CompositionError::AdapterInit(e.to_string()))?;

        let profiles_path = repo.root().join(AGENT_PROFILES_PATH);
        let profiles = AgentProfiles::load(repo.root(), &profiles_path)
            .map_err(|e| CompositionError::ConfigLoad(format!("{e}")))?;
        let capability = CapabilityName::try_new("pr-reviewer")
            .map_err(|error| CompositionError::WiringFailed(error.to_string()))?;
        let resolved = profiles
            .resolve_execution(&capability, RoundType::Final)
            .map_err(|error| CompositionError::WiringFailed(error.to_string()))?;
        let ResolvedExecution::HostedService { provider } = resolved else {
            return Err(CompositionError::WiringFailed(
                "pr-reviewer must use a hosted-service execution profile".to_owned(),
            ));
        };
        usecase::pr_review::validate_reviewer_provider(provider.as_str())
            .map_err(|e| CompositionError::WiringFailed(e.to_string()))?;

        let branch = repo
            .current_branch()
            .map_err(|e| CompositionError::Infrastructure(e.to_string()))?
            .ok_or_else(|| {
                CompositionError::WiringFailed("could not determine current branch".to_owned())
            })?;
        if !branch.starts_with("track/") {
            return Err(CompositionError::WiringFailed(
                "not on a track branch (expected track/<id>); \
                 switch to the track branch and retry."
                    .to_owned(),
            ));
        }

        let active_track_id = branch.strip_prefix("track/").unwrap_or(&branch).to_owned();
        let client = SystemGhClient;

        let (pr_number, trigger_timestamp, head_ref_owned) = if resume {
            resume_trigger_state(&active_track_id)?
        } else {
            match trigger_new_review(track_id.as_deref(), &active_track_id, &client)? {
                Some(tuple) => tuple,
                None => return Ok(CommandOutcome::failure(None)),
            }
        };

        let nwo = client.repo_nwo().map_err(|e| CompositionError::Infrastructure(e.to_string()))?;
        let head_ref = head_ref_owned.as_deref();

        // D4 extraction: delegate to PrReviewPollingInteractor (T008).
        // Timeout=600s, interval=15s → max_iterations=40.
        use usecase::pr_review_polling::{
            PrReviewPollingCommand, PrReviewPollingOutput, PrReviewPollingService as _,
        };
        let interactor = make_polling_interactor();
        let poll_cmd = PrReviewPollingCommand {
            pr: pr_number.clone(),
            repo_nwo: nwo.clone(),
            trigger_timestamp: trigger_timestamp.clone(),
            interval_secs: 15,
            max_iterations: 40, // 600s / 15s
            head_commit: head_ref.map(str::to_owned),
        };
        let poll_result_raw =
            interactor.poll(poll_cmd).map_err(|e| CompositionError::Usecase(e.to_string()))?;

        // Map usecase PrReviewPollingOutput → local PollReviewResult for the
        // parse_review / format_review_summary path below.
        let poll_result = match poll_result_raw {
            PrReviewPollingOutput::ReviewFound(v) => PollReviewResult::ReviewFound(v),
            PrReviewPollingOutput::ZeroFindings => PollReviewResult::ZeroFindings,
            PrReviewPollingOutput::Timeout => PollReviewResult::Timeout,
        };

        let result = match poll_result {
            PollReviewResult::ZeroFindings => {
                let stdout = format!(
                    "\n=== PR Review Result: PASS ===\nPR: #{pr_number}\n\
                     Zero findings detected (bot signalled no issues)."
                );
                Ok(CommandOutcome::success(Some(stdout)))
            }
            PollReviewResult::Timeout => Ok(CommandOutcome::failure(None)),
            PollReviewResult::ReviewFound(review) => {
                let parsed = parse_review(&pr_number, &review, &nwo, &client)?;
                let summary = format_review_summary(&pr_number, &parsed);
                // ReviewFound always exits 0 (D1/AC-09): pass/fail judgment is
                // delegated to the calling agent; Rust no longer gates on findings.
                Ok(CommandOutcome::success(Some(summary)))
            }
        };

        // Clean up trigger state on successful completion (not on timeout).
        if matches!(&result, Ok(outcome) if outcome.exit_code == 0) {
            cleanup_trigger_state(&active_track_id)?;
        }

        result
    }
}

impl PrCommandPort for SystemPrCommandAdapter {
    fn execute(&self, command: PrCommand) -> PrCommandOutput {
        let result = match command {
            PrCommand::Push { track_id } => self.pr_push(track_id.map(|id| id.as_str().to_owned())),
            PrCommand::Ensure { track_id, base } => self.pr_ensure(
                track_id.map(|id| id.as_str().to_owned()),
                base.map(|value| value.as_str().to_owned()).unwrap_or_default(),
            ),
            PrCommand::Status(pr) => self.pr_status(pr.as_str().to_owned()),
            PrCommand::WaitAndMerge { pr, interval, timeout, method } => self.pr_wait_and_merge(
                pr.as_str().to_owned(),
                interval.as_secs(),
                timeout.as_secs(),
                method.map(merge_method_to_arg).unwrap_or_default().to_owned(),
            ),
            PrCommand::TriggerReview(pr) => self.pr_trigger_review(pr.as_str().to_owned()),
            PrCommand::PollReview { pr, trigger_timestamp, interval, timeout } => self
                .pr_poll_review(
                    pr.as_str().to_owned(),
                    trigger_timestamp.as_str().to_owned(),
                    interval.as_secs(),
                    timeout.as_secs(),
                ),
            PrCommand::ReviewCycle { track_id, mode } => self.pr_review_cycle(
                track_id.map(|id| id.as_str().to_owned()),
                matches!(mode, PrReviewCycleMode::Resume),
            ),
        };
        result.unwrap_or_else(|error| PrCommandOutput::failure(Some(error.to_string())))
    }
}

// ---------------------------------------------------------------------------
// Branch strategy resolution helpers (T011)
// ---------------------------------------------------------------------------

/// Resolve a [`infrastructure::branch_strategy::SnapshotBranchStrategyAdapter`]
/// from `track_id`'s `metadata.json#branch_strategy_snapshot` (D4: post-init
/// operations read the per-track snapshot, never the global config).
fn branch_strategy_port_for_track(
    track_id: &str,
) -> Result<infrastructure::branch_strategy::SnapshotBranchStrategyAdapter, CompositionError> {
    use domain::TrackReader as _;
    use infrastructure::git_cli::SystemGitRepo;
    use infrastructure::track::fs_store::FsTrackStore;

    let repo =
        SystemGitRepo::discover().map_err(|e| CompositionError::AdapterInit(e.to_string()))?;
    let items_dir = repo.root().join("track").join("items");
    let id = domain::TrackId::try_new(track_id)
        .map_err(|e| CompositionError::WiringFailed(format!("invalid track ID: {e}")))?;
    let store = FsTrackStore::new(items_dir);
    let metadata = store
        .find(&id)
        .map_err(|e| {
            CompositionError::Infrastructure(format!("failed to read track metadata: {e}"))
        })?
        .ok_or_else(|| CompositionError::WiringFailed(format!("track '{track_id}' not found")))?;
    Ok(infrastructure::branch_strategy::SnapshotBranchStrategyAdapter::new(
        metadata.branch_strategy_snapshot().clone(),
    ))
}

/// Resolve a [`infrastructure::branch_strategy::SnapshotBranchStrategyAdapter`] from
/// `track_id`'s `metadata.json#branch_strategy_snapshot` on the fetched PR ref
/// (`origin/<branch>`), rather than the local worktree.
///
/// Callers that dispatch on a PR head (e.g. `pr wait-and-merge`) must resolve the merge
/// method from the PR's own committed metadata, not from whatever happens to be checked
/// out locally — a fresh checkout or the configured base branch would otherwise `track
/// not found` (or use stale metadata) for a PR whose track was created after the last
/// pull.
fn branch_strategy_port_for_pr_ref(
    _repo: &infrastructure::git_cli::SystemGitRepo,
    branch: &str,
    track_id: &str,
) -> Result<infrastructure::branch_strategy::SnapshotBranchStrategyAdapter, CompositionError> {
    use std::sync::Arc;

    use infrastructure::FsGitWorkflowAdapter;
    use usecase::git_workflow::{GitPrimitivePort, PrGitInteractor};

    // Route the `git show origin/<branch>:track/items/<track_id>/metadata.json`
    // read through the usecase PrGitInteractor (T007 / T008). The interactor
    // internally performs the fetch + show pair so callers don't need to
    // reach into private SystemGitRepo helpers.
    let validated_id = domain::TrackId::try_new(track_id)
        .map_err(|e| CompositionError::WiringFailed(format!("invalid track ID: {e}")))?;
    let port: Arc<dyn GitPrimitivePort> = Arc::new(FsGitWorkflowAdapter::new());
    let interactor = PrGitInteractor::new(port);
    let json = interactor.fetch_and_read_metadata_at_ref(branch, &validated_id).map_err(|e| {
        CompositionError::WiringFailed(format!(
            "track '{track_id}' metadata not found on origin/{branch}: {e}"
        ))
    })?;
    let (metadata, _) = infrastructure::track::codec::decode(&json).map_err(|e| {
        CompositionError::Infrastructure(format!(
            "failed to decode metadata.json on origin/{branch}: {e}"
        ))
    })?;
    Ok(infrastructure::branch_strategy::SnapshotBranchStrategyAdapter::new(
        metadata.branch_strategy_snapshot().clone(),
    ))
}

/// Render a [`domain::MergeMethod`] as the lowercase argument string accepted by
/// `gh pr merge --merge|--squash|--rebase` (mirrors the CLI's `value_parser`).
fn merge_method_to_arg(method: domain::MergeMethod) -> &'static str {
    match method {
        domain::MergeMethod::Squash => "squash",
        domain::MergeMethod::Merge => "merge",
        domain::MergeMethod::Rebase => "rebase",
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::path::Path;
    use std::process::Command;

    use usecase::pr::PrTrackIdOverride;

    use super::*;

    fn run_git(path: &Path, args: &[&str]) {
        let output = Command::new("git").current_dir(path).args(args).output().unwrap();
        assert!(
            output.status.success(),
            "git {args:?} failed:\nstdout={}\nstderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
    }

    #[test]
    fn test_system_pr_command_adapter_new_exposes_typed_port() {
        let adapter = SystemPrCommandAdapter::new();
        let _port: &dyn PrCommandPort = &adapter;
    }

    #[test]
    fn test_system_pr_command_adapter_as_typed_port_pushes_branch_to_remote_ref() {
        let test_binary = std::env::current_exe().unwrap();
        let output = Command::new(test_binary)
            .args([
                "--exact",
                "pr::tests::test_system_pr_command_adapter_pushes_branch_in_subprocess",
                "--ignored",
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "isolated adapter push test failed:\nstdout={}\nstderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
    }

    /// The system adapter discovers its repository from the process CWD. Run
    /// this fixture in a child test process so it cannot race other tests.
    #[test]
    #[ignore]
    fn test_system_pr_command_adapter_pushes_branch_in_subprocess() {
        let sandbox = tempfile::tempdir().unwrap();
        let remote = sandbox.path().join("origin.git");
        let workspace = sandbox.path().join("workspace");
        let track_id = "system-adapter-contract";
        let branch = format!("track/{track_id}");

        let remote_init = Command::new("git")
            .args(["init", "--bare", remote.to_str().unwrap()])
            .output()
            .unwrap();
        assert!(
            remote_init.status.success(),
            "bare remote init failed: {}",
            String::from_utf8_lossy(&remote_init.stderr),
        );
        std::fs::create_dir_all(&workspace).unwrap();
        run_git(&workspace, &["init", "--initial-branch", &branch]);
        run_git(&workspace, &["config", "user.email", "test@example.com"]);
        run_git(&workspace, &["config", "user.name", "Test User"]);
        std::fs::write(workspace.join("contract.txt"), "typed port path\n").unwrap();
        run_git(&workspace, &["add", "contract.txt"]);
        run_git(&workspace, &["commit", "-m", "contract fixture"]);
        run_git(&workspace, &["remote", "add", "origin", remote.to_str().unwrap()]);

        let adapter = SystemPrCommandAdapter::new();
        let port: &dyn PrCommandPort = &adapter;
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&workspace).unwrap();
        let outcome = port.execute(PrCommand::Push {
            track_id: Some(PrTrackIdOverride::new("INVALID".to_owned())),
        });
        std::env::set_current_dir(original_dir).unwrap();

        assert_eq!(outcome.stdout.as_deref(), Some(format!("[OK] Pushed {branch}").as_str()));
        assert_eq!(outcome.stderr, None);
        assert_eq!(outcome.exit_code, 0);

        let local_head = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&workspace)
            .output()
            .unwrap();
        let remote_head = Command::new("git")
            .args([
                "--git-dir",
                remote.to_str().unwrap(),
                "rev-parse",
                &format!("refs/heads/{branch}"),
            ])
            .output()
            .unwrap();
        assert!(local_head.status.success());
        assert!(remote_head.status.success());
        assert_eq!(local_head.stdout, remote_head.stdout);
    }
}
