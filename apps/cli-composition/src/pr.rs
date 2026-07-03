//! `pr` command family composition root methods.
//!
//! Implements the full PR workflow: push, ensure-pr, status, wait-and-merge,
//! trigger-review, poll-review, and review-cycle. All methods use concrete
//! infrastructure types internally; the public API exposes only primitives and
//! `CommandOutcome` (CN-02).
//!
//! Private polling and review helpers are in `poll` (see `pr/poll.rs`).

mod poll;
mod poll_adapters;

use std::fs;
use std::thread;
use std::time::{Duration, Instant};

use crate::{CommandOutcome, error::CompositionError};

// ── Per-context composition root ──────────────────────────────────────────────

/// Composition root for the `pr` command family.
///
/// Unit struct: no adapter dependencies are injected at construction time.
pub struct PrCompositionRoot;

impl PrCompositionRoot {
    /// Create a new `PrCompositionRoot`.
    pub fn new() -> Self {
        Self
    }
}

impl Default for PrCompositionRoot {
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
// PrCompositionRoot implementations
// ---------------------------------------------------------------------------

impl PrCompositionRoot {
    /// Build a wired [`cli_driver::pr::PrDriver`] for the `pr` family.
    ///
    /// Constructs all closures that wire infrastructure + usecase together and
    /// injects them into a [`usecase::pr::PrCommandInteractor`], which is then
    /// handed to [`cli_driver::pr::PrDriver`].
    pub fn pr_driver(&self) -> cli_driver::pr::PrDriver {
        use std::sync::Arc;
        use usecase::pr::{PrCommandInteractor, PrCommandOutput};

        let push_fn = Arc::new(|track_id: Option<String>| {
            let root = PrCompositionRoot::new();
            match root.pr_push(track_id) {
                Ok(o) => {
                    PrCommandOutput { stdout: o.stdout, stderr: o.stderr, exit_code: o.exit_code }
                }
                Err(e) => PrCommandOutput::failure(Some(e.to_string())),
            }
        });
        let ensure_fn = Arc::new(|track_id: Option<String>, base: String| {
            let root = PrCompositionRoot::new();
            match root.pr_ensure(track_id, base) {
                Ok(o) => {
                    PrCommandOutput { stdout: o.stdout, stderr: o.stderr, exit_code: o.exit_code }
                }
                Err(e) => PrCommandOutput::failure(Some(e.to_string())),
            }
        });
        let status_fn = Arc::new(|pr: String| {
            let root = PrCompositionRoot::new();
            match root.pr_status(pr) {
                Ok(o) => {
                    PrCommandOutput { stdout: o.stdout, stderr: o.stderr, exit_code: o.exit_code }
                }
                Err(e) => PrCommandOutput::failure(Some(e.to_string())),
            }
        });
        let wait_fn = Arc::new(|pr: String, interval: u64, timeout: u64, method: String| {
            let root = PrCompositionRoot::new();
            match root.pr_wait_and_merge(pr, interval, timeout, method) {
                Ok(o) => {
                    PrCommandOutput { stdout: o.stdout, stderr: o.stderr, exit_code: o.exit_code }
                }
                Err(e) => PrCommandOutput::failure(Some(e.to_string())),
            }
        });
        let trigger_fn = Arc::new(|pr: String| {
            let root = PrCompositionRoot::new();
            match root.pr_trigger_review(pr) {
                Ok(o) => {
                    PrCommandOutput { stdout: o.stdout, stderr: o.stderr, exit_code: o.exit_code }
                }
                Err(e) => PrCommandOutput::failure(Some(e.to_string())),
            }
        });
        let poll_fn =
            Arc::new(|pr: String, trigger_timestamp: String, interval: u64, timeout: u64| {
                let root = PrCompositionRoot::new();
                match root.pr_poll_review(pr, trigger_timestamp, interval, timeout) {
                    Ok(o) => PrCommandOutput {
                        stdout: o.stdout,
                        stderr: o.stderr,
                        exit_code: o.exit_code,
                    },
                    Err(e) => PrCommandOutput::failure(Some(e.to_string())),
                }
            });
        let cycle_fn = Arc::new(|track_id: Option<String>, resume: bool| {
            let root = PrCompositionRoot::new();
            match root.pr_review_cycle(track_id, resume) {
                Ok(o) => {
                    PrCommandOutput { stdout: o.stdout, stderr: o.stderr, exit_code: o.exit_code }
                }
                Err(e) => PrCommandOutput::failure(Some(e.to_string())),
            }
        });

        let service = Arc::new(PrCommandInteractor::new(
            push_fn, ensure_fn, status_fn, wait_fn, trigger_fn, poll_fn, cycle_fn,
        ));
        cli_driver::pr::PrDriver::new(service)
    }

    /// Push the current track branch to origin.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn pr_push(&self, track_id: Option<String>) -> Result<CommandOutcome, CompositionError> {
        use infrastructure::git_cli::{GitRepository as _, SystemGitRepo};

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
    pub fn pr_ensure(
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
    pub fn pr_status(&self, pr: String) -> Result<CommandOutcome, CompositionError> {
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
    pub fn pr_wait_and_merge(
        &self,
        pr: String,
        interval: u64,
        timeout: u64,
        method: String,
    ) -> Result<CommandOutcome, CompositionError> {
        use infrastructure::gh_cli::{GhClient as _, SystemGhClient};
        use infrastructure::git_cli::{GitRepository as _, SystemGitRepo};
        use usecase::branch_strategy::BranchStrategyPort as _;
        use usecase::pr_workflow::{WaitDecision, decide_wait_action};

        let client = SystemGhClient;
        let branch = client
            .pr_head_branch(&pr)
            .map_err(|e| CompositionError::Infrastructure(e.to_string()))?;
        let repo =
            SystemGitRepo::discover().map_err(|e| CompositionError::AdapterInit(e.to_string()))?;
        // Use an explicit refspec (+refs/heads/<branch>:refs/remotes/origin/<branch>) so that
        // refs/remotes/origin/<branch> is reliably updated. A bare `git fetch origin <branch>`
        // only refreshes FETCH_HEAD and does not guarantee that `origin/<branch>` is updated,
        // which would cause subsequent `git show origin/<branch>:…` reads to see a stale ref.
        //
        // Fetch runs BEFORE the merge-method resolution so that an omitted `--method` can be
        // resolved from the PR head's `branch_strategy_snapshot.merge_method` via
        // `git show origin/<branch>:track/items/<track_id>/metadata.json`, not from the local
        // worktree (which may not contain the PR's track metadata when invoked from a fresh
        // checkout or the configured base branch).
        let refspec = format!("+refs/heads/{branch}:refs/remotes/origin/{branch}");
        match repo.output(&["fetch", "origin", &refspec]) {
            Ok(o) if !o.status.success() => {
                let stderr = String::from_utf8_lossy(&o.stderr);
                return Err(CompositionError::Infrastructure(format!(
                    "git fetch origin/{branch} failed: {stderr}"
                )));
            }
            Err(e) => {
                return Err(CompositionError::Infrastructure(format!(
                    "failed to run git fetch: {e}"
                )));
            }
            Ok(_) => {}
        }
        let method = if method.is_empty() {
            let track_id = branch.strip_prefix("track/").unwrap_or(&branch);
            let port = branch_strategy_port_for_pr_ref(&repo, &branch, track_id)?;
            merge_method_to_arg(port.merge_method()).to_owned()
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
    pub fn pr_trigger_review(&self, pr: String) -> Result<CommandOutcome, CompositionError> {
        use infrastructure::agent_profiles::{AGENT_PROFILES_PATH, AgentProfiles, RoundType};
        use infrastructure::gh_cli::{GhClient as _, SystemGhClient};
        use infrastructure::git_cli::{GitRepository as _, SystemGitRepo};

        let git_repo =
            SystemGitRepo::discover().map_err(|e| CompositionError::AdapterInit(e.to_string()))?;
        let profiles_path = git_repo.root().join(AGENT_PROFILES_PATH);
        let profiles = AgentProfiles::load(&profiles_path)
            .map_err(|e| CompositionError::ConfigLoad(format!("{e}")))?;
        let resolved =
            profiles.resolve_execution("pr-reviewer", RoundType::Final).ok_or_else(|| {
                CompositionError::WiringFailed(
                    "pr-reviewer capability not defined in agent-profiles.json".to_owned(),
                )
            })?;
        usecase::pr_review::validate_reviewer_provider(&resolved.provider)
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
    pub fn pr_poll_review(
        &self,
        pr: String,
        trigger_timestamp: String,
        interval: u64,
        timeout: u64,
    ) -> Result<CommandOutcome, CompositionError> {
        use infrastructure::gh_cli::{GhClient as _, SystemGhClient};
        use infrastructure::git_cli::{GitRepository as _, SystemGitRepo};
        use usecase::pr_review_polling::{
            PrReviewPollingCommand, PrReviewPollingOutput, PrReviewPollingService as _,
        };

        let head = SystemGitRepo::discover().ok().and_then(|r| {
            r.output(&["rev-parse", "HEAD"])
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        });

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
    pub fn pr_review_cycle(
        &self,
        track_id: Option<String>,
        resume: bool,
    ) -> Result<CommandOutcome, CompositionError> {
        use infrastructure::agent_profiles::{AGENT_PROFILES_PATH, AgentProfiles, RoundType};
        use infrastructure::gh_cli::{GhClient as _, SystemGhClient};
        use infrastructure::git_cli::{GitRepository as _, SystemGitRepo};

        let repo =
            SystemGitRepo::discover().map_err(|e| CompositionError::AdapterInit(e.to_string()))?;

        let profiles_path = repo.root().join(AGENT_PROFILES_PATH);
        let profiles = AgentProfiles::load(&profiles_path)
            .map_err(|e| CompositionError::ConfigLoad(format!("{e}")))?;
        let resolved =
            profiles.resolve_execution("pr-reviewer", RoundType::Final).ok_or_else(|| {
                CompositionError::WiringFailed(
                    "pr-reviewer capability not defined in agent-profiles.json".to_owned(),
                )
            })?;
        usecase::pr_review::validate_reviewer_provider(&resolved.provider)
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
            cleanup_trigger_state(&active_track_id);
        }

        result
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
    use infrastructure::git_cli::{GitRepository as _, SystemGitRepo};
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
    repo: &infrastructure::git_cli::SystemGitRepo,
    branch: &str,
    track_id: &str,
) -> Result<infrastructure::branch_strategy::SnapshotBranchStrategyAdapter, CompositionError> {
    use infrastructure::git_cli::GitRepository as _;

    let path = format!("track/items/{track_id}/metadata.json");
    let output = repo.output(&["show", &format!("origin/{branch}:{path}")]).map_err(|e| {
        CompositionError::Infrastructure(format!(
            "failed to run git show origin/{branch}:{path}: {e}"
        ))
    })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CompositionError::WiringFailed(format!(
            "track '{track_id}' metadata not found on origin/{branch}: {stderr}"
        )));
    }
    let json = String::from_utf8(output.stdout).map_err(|e| {
        CompositionError::Infrastructure(format!(
            "metadata.json on origin/{branch} is not UTF-8: {e}"
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
