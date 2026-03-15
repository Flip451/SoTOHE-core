//! CLI subcommands for pull-request workflow wrappers.

use std::process::ExitCode;
use std::thread;
use std::time::{Duration, Instant};

use std::fs;
use std::path::PathBuf;

use clap::{Args, Subcommand};
use infrastructure::gh_cli::{GhClient, PrCheckRecord, SystemGhClient};
use infrastructure::git_cli::{GitRepository, SystemGitRepo};
use usecase::pr_workflow::{
    CheckSummary, PrBranchContext, PrCheckStatus, PrCheckView, WaitDecision, decide_wait_action,
    pr_body, pr_title, resolve_pr_branch, summarize_checks,
};

use crate::CliError;

#[derive(Debug, Subcommand)]
pub enum PrCommand {
    /// Push the current track/plan branch to origin.
    Push(PushArgs),
    /// Create or reuse a PR for the current track/plan branch.
    EnsurePr(EnsurePrArgs),
    /// Show current PR check status.
    Status(StatusArgs),
    /// Poll PR checks until they pass, then merge.
    WaitAndMerge(WaitAndMergeArgs),
}

#[derive(Debug, Args)]
pub struct PushArgs {
    /// Explicit track ID (required on plan/ branches, ignored on track/ branches).
    #[arg(long)]
    pub track_id: Option<String>,
}

#[derive(Debug, Args)]
pub struct EnsurePrArgs {
    /// Explicit track ID (required on plan/ branches, ignored on track/ branches).
    #[arg(long)]
    pub track_id: Option<String>,
    /// Base branch for the PR.
    #[arg(long, default_value = "main")]
    pub base: String,
}

#[derive(Debug, Args)]
pub struct StatusArgs {
    pub pr: String,
}

#[derive(Debug, Args)]
pub struct WaitAndMergeArgs {
    pub pr: String,
    #[arg(long, default_value_t = 15)]
    pub interval: u64,
    #[arg(long, default_value_t = 600)]
    pub timeout: u64,
    #[arg(long, value_parser = ["merge", "squash", "rebase"], default_value = "merge")]
    pub method: String,
}

pub fn execute(cmd: PrCommand) -> ExitCode {
    match cmd {
        PrCommand::Push(args) => match push(args.track_id.as_deref()) {
            Ok(code) => code,
            Err(err) => {
                eprintln!("{err}");
                err.exit_code()
            }
        },
        PrCommand::EnsurePr(args) => match ensure_pr(args.track_id.as_deref(), &args.base) {
            Ok(code) => code,
            Err(err) => {
                eprintln!("{err}");
                err.exit_code()
            }
        },
        PrCommand::Status(args) => status(&args.pr),
        PrCommand::WaitAndMerge(args) => {
            wait_and_merge(&args.pr, args.interval, args.timeout, &args.method)
        }
    }
}

fn resolve_branch_context(explicit_track_id: Option<&str>) -> Result<PrBranchContext, CliError> {
    let repo = SystemGitRepo::discover()?;
    let branch = repo
        .current_branch()?
        .ok_or_else(|| CliError::Message("could not determine current branch".to_owned()))?;
    resolve_pr_branch(&branch, explicit_track_id).map_err(CliError::from)
}

fn push(explicit_track_id: Option<&str>) -> Result<ExitCode, CliError> {
    let ctx = resolve_branch_context(explicit_track_id)?;
    let repo = SystemGitRepo::discover()?;
    println!("Pushing {} to origin...", ctx.branch);
    repo.push_branch(&ctx.branch)?;
    println!("[OK] Pushed {}", ctx.branch);
    Ok(ExitCode::SUCCESS)
}

fn ensure_pr(explicit_track_id: Option<&str>, base: &str) -> Result<ExitCode, CliError> {
    let ctx = resolve_branch_context(explicit_track_id)?;
    let client = SystemGhClient;
    Ok(ensure_pr_with(&ctx, base, &client))
}

fn ensure_pr_with<C: GhClient>(ctx: &PrBranchContext, base: &str, client: &C) -> ExitCode {
    // Check for existing PR
    match client.find_open_pr(&ctx.branch, base) {
        Ok(Some(pr)) => {
            println!("[OK] Reusing existing PR #{pr}");
            return ExitCode::SUCCESS;
        }
        Ok(None) => {} // create new
        Err(err) => {
            eprintln!("[ERROR] {err}");
            return ExitCode::FAILURE;
        }
    }

    // Write body to a uniquely-named temp file to avoid races.
    let body_dir = PathBuf::from("tmp");
    if let Err(err) = fs::create_dir_all(&body_dir) {
        eprintln!("[ERROR] failed to create tmp dir: {err}");
        return ExitCode::FAILURE;
    }
    let body_file = body_dir.join(format!("pr-body-{}.md", std::process::id()));
    let body_text = pr_body(ctx);
    if let Err(err) = fs::write(&body_file, &body_text) {
        eprintln!("[ERROR] failed to write PR body file: {err}");
        return ExitCode::FAILURE;
    }

    let title = pr_title(ctx);
    match client.create_pr(&ctx.branch, base, &title, &body_file) {
        Ok(pr) => {
            // Clean up body file
            let _ = fs::remove_file(&body_file);
            println!("[OK] Created PR #{pr}");
            ExitCode::SUCCESS
        }
        Err(err) => {
            let _ = fs::remove_file(&body_file);
            eprintln!("[ERROR] {err}");
            ExitCode::FAILURE
        }
    }
}

fn normalize_check_status(check: &PrCheckRecord) -> PrCheckStatus {
    let state = if !check.bucket.is_empty() { check.bucket.as_str() } else { check.state.as_str() };

    match state.to_uppercase().as_str() {
        "SUCCESS" | "PASS" | "SKIPPING" => PrCheckStatus::Passed,
        "FAILURE" | "FAIL" | "CANCEL" => PrCheckStatus::Failed,
        _ => PrCheckStatus::Pending,
    }
}

fn checks_summary(checks: &[PrCheckRecord]) -> CheckSummary {
    let checks = checks
        .iter()
        .map(|check| PrCheckView {
            name: check.name.clone(),
            status: normalize_check_status(check),
        })
        .collect::<Vec<_>>();
    summarize_checks(&checks)
}

fn status(pr: &str) -> ExitCode {
    let client = SystemGhClient;
    status_with(pr, &client)
}

fn status_with<C>(pr: &str, client: &C) -> ExitCode
where
    C: GhClient,
{
    let checks = match client.pr_checks(pr) {
        Ok(checks) => checks,
        Err(err) => {
            println!("[ERROR] {err}");
            return ExitCode::FAILURE;
        }
    };

    let url = client.pr_url(pr);
    println!("PR: {url}");
    match checks_summary(&checks) {
        CheckSummary::AllPassed => {
            println!("[OK] All checks passed.");
            ExitCode::SUCCESS
        }
        CheckSummary::Failed(names) => {
            println!("[FAIL] Failed checks: {}", names.join(", "));
            ExitCode::FAILURE
        }
        CheckSummary::Pending(names) => {
            println!("[PENDING] Waiting: {}", names.join(", "));
            ExitCode::from(2)
        }
    }
}

fn merge_pr_with<C>(pr: &str, method: &str, client: &C) -> ExitCode
where
    C: GhClient,
{
    println!("[OK] All checks passed. Merging...");
    match client.merge_pr(pr, method) {
        Ok(()) => {
            println!("[OK] PR #{pr} merged ({method}).");
            ExitCode::SUCCESS
        }
        Err(err) => {
            println!("[ERROR] Merge failed: {err}");
            ExitCode::FAILURE
        }
    }
}

fn wait_and_merge_with<C, Sleep>(
    pr: &str,
    interval: u64,
    timeout: u64,
    method: &str,
    client: &C,
    sleep: &Sleep,
) -> ExitCode
where
    C: GhClient,
    Sleep: Fn(Duration),
{
    let url = client.pr_url(pr);
    println!("PR: {url}");
    println!("Polling checks every {interval}s (timeout {timeout}s)...");

    let start = Instant::now();
    loop {
        let elapsed = start.elapsed().as_secs();
        let checks = match client.pr_checks(pr) {
            Ok(checks) => checks,
            Err(err) => {
                println!("[ERROR] {err}");
                return ExitCode::FAILURE;
            }
        };
        match decide_wait_action(checks_summary(&checks), elapsed, timeout, interval) {
            WaitDecision::MergeNow => return merge_pr_with(pr, method, client),
            WaitDecision::FailChecks(names) => {
                println!("[FAIL] Checks failed: {}", names.join(", "));
                println!("Fix the failures and push again.");
                return ExitCode::FAILURE;
            }
            WaitDecision::Timeout(names) => {
                println!("[TIMEOUT] Still pending after {timeout}s: {}", names.join(", "));
                return ExitCode::FAILURE;
            }
            WaitDecision::Wait { pending, delay_seconds } => {
                println!(
                    "  [{elapsed}s] Pending: {} (retry in {delay_seconds}s)",
                    pending.join(", ")
                );
                sleep(Duration::from_secs(delay_seconds));
            }
        }
    }
}

fn wait_and_merge(pr: &str, interval: u64, timeout: u64, method: &str) -> ExitCode {
    let client = SystemGhClient;
    wait_and_merge_with(pr, interval, timeout, method, &client, &thread::sleep)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use std::cell::RefCell;
    use std::process::ExitCode;

    use std::path::Path;

    use super::{
        CheckSummary, checks_summary, ensure_pr_with, normalize_check_status, status_with,
        wait_and_merge_with,
    };
    use infrastructure::gh_cli::{GhClient, GhError, PrCheckRecord};
    use usecase::pr_workflow::{PrBranchContext, PrCheckStatus};

    struct FakeGhClient {
        checks: RefCell<Vec<Result<Vec<PrCheckRecord>, GhError>>>,
        url: String,
        merge_calls: RefCell<Vec<(String, String)>>,
        merge_result: RefCell<Option<String>>,
        find_pr_result: RefCell<Result<Option<String>, String>>,
        create_pr_result: RefCell<Result<String, String>>,
    }

    impl FakeGhClient {
        fn default_for_pr() -> Self {
            Self {
                checks: RefCell::new(Vec::new()),
                url: String::new(),
                merge_calls: RefCell::new(Vec::new()),
                merge_result: RefCell::new(None),
                find_pr_result: RefCell::new(Ok(None)),
                create_pr_result: RefCell::new(Ok("42".to_owned())),
            }
        }
    }

    impl GhClient for FakeGhClient {
        fn pr_checks(&self, _pr: &str) -> Result<Vec<PrCheckRecord>, GhError> {
            self.checks.borrow_mut().remove(0)
        }

        fn pr_url(&self, pr: &str) -> String {
            if self.url.is_empty() { format!("PR #{pr}") } else { self.url.clone() }
        }

        fn merge_pr(&self, pr: &str, method: &str) -> Result<(), GhError> {
            self.merge_calls.borrow_mut().push((pr.to_owned(), method.to_owned()));
            match self.merge_result.borrow().as_deref() {
                None => Ok(()),
                Some(stderr) => Err(GhError::CommandFailed {
                    command: format!("pr merge {pr} --{method}"),
                    stderr: stderr.to_owned(),
                }),
            }
        }

        fn find_open_pr(&self, _head: &str, _base: &str) -> Result<Option<String>, GhError> {
            self.find_pr_result
                .borrow()
                .clone()
                .map_err(|stderr| GhError::CommandFailed { command: "pr list".to_owned(), stderr })
        }

        fn create_pr(
            &self,
            _head: &str,
            _base: &str,
            _title: &str,
            _body_file: &Path,
        ) -> Result<String, GhError> {
            self.create_pr_result.borrow().clone().map_err(|stderr| GhError::CommandFailed {
                command: "pr create".to_owned(),
                stderr,
            })
        }
    }

    #[test]
    fn normalize_check_status_prefers_bucket_over_state() {
        let check = PrCheckRecord {
            name: "ci".to_owned(),
            state: "SUCCESS".to_owned(),
            bucket: "pending".to_owned(),
        };

        assert_eq!(normalize_check_status(&check), PrCheckStatus::Pending);
    }

    #[test]
    fn status_with_propagates_adapter_errors() {
        let client = FakeGhClient {
            checks: RefCell::new(vec![Err(GhError::CommandFailed {
                command: "pr checks 123".to_owned(),
                stderr: "gh exploded".to_owned(),
            })]),
            ..FakeGhClient::default_for_pr()
        };

        assert_eq!(status_with("123", &client), ExitCode::FAILURE);
    }

    #[test]
    fn checks_summary_maps_normalized_cli_checks_to_pending() {
        let checks = vec![PrCheckRecord {
            name: "ci".to_owned(),
            state: "SUCCESS".to_owned(),
            bucket: "pending".to_owned(),
        }];

        assert_eq!(checks_summary(&checks), CheckSummary::Pending(vec!["ci".to_owned()]));
    }

    #[test]
    fn wait_and_merge_with_merges_after_all_checks_pass() {
        let client = FakeGhClient {
            checks: RefCell::new(vec![Ok(vec![PrCheckRecord {
                name: "ci".to_owned(),
                state: "SUCCESS".to_owned(),
                bucket: String::new(),
            }])]),
            url: "https://example.invalid/pr/123".to_owned(),
            ..FakeGhClient::default_for_pr()
        };
        let result = wait_and_merge_with("123", 15, 600, "squash", &client, &|_| {
            panic!("sleep should not be called")
        });

        assert_eq!(result, ExitCode::SUCCESS);
        assert_eq!(client.merge_calls.into_inner(), vec![("123".to_owned(), "squash".to_owned())]);
    }

    #[test]
    fn wait_and_merge_with_returns_failure_when_checks_api_errors() {
        let client = FakeGhClient {
            checks: RefCell::new(vec![Err(GhError::CommandFailed {
                command: "pr checks 123".to_owned(),
                stderr: "boom".to_owned(),
            })]),
            ..FakeGhClient::default_for_pr()
        };
        let result = wait_and_merge_with("123", 15, 600, "merge", &client, &|_| {
            panic!("sleep should not be called")
        });

        assert_eq!(result, ExitCode::FAILURE);
    }

    #[test]
    fn wait_and_merge_with_times_out_pending_checks_without_sleep_when_deadline_reached() {
        let sleeps = RefCell::new(Vec::new());
        let client = FakeGhClient {
            checks: RefCell::new(vec![Ok(vec![PrCheckRecord {
                name: "ci".to_owned(),
                state: "PENDING".to_owned(),
                bucket: "pending".to_owned(),
            }])]),
            ..FakeGhClient::default_for_pr()
        };
        let result = wait_and_merge_with("123", 15, 0, "merge", &client, &|duration| {
            sleeps.borrow_mut().push(duration)
        });

        assert_eq!(result, ExitCode::FAILURE);
        assert!(sleeps.borrow().is_empty());
    }

    // --- ensure_pr_with tests ---

    #[test]
    fn ensure_pr_with_reuses_existing_pr() {
        let client = FakeGhClient {
            find_pr_result: RefCell::new(Ok(Some("99".to_owned()))),
            ..FakeGhClient::default_for_pr()
        };
        let ctx = PrBranchContext {
            branch: "track/my-feature".to_owned(),
            track_id: "my-feature".to_owned(),
            is_plan_branch: false,
        };
        assert_eq!(ensure_pr_with(&ctx, "main", &client), ExitCode::SUCCESS);
    }

    #[test]
    fn ensure_pr_with_creates_new_pr_when_none_exists() {
        let client = FakeGhClient {
            find_pr_result: RefCell::new(Ok(None)),
            create_pr_result: RefCell::new(Ok("42".to_owned())),
            ..FakeGhClient::default_for_pr()
        };
        let ctx = PrBranchContext {
            branch: "track/my-feature".to_owned(),
            track_id: "my-feature".to_owned(),
            is_plan_branch: false,
        };
        assert_eq!(ensure_pr_with(&ctx, "main", &client), ExitCode::SUCCESS);
    }

    #[test]
    fn ensure_pr_with_returns_failure_on_find_error() {
        let client = FakeGhClient {
            find_pr_result: RefCell::new(Err("gh exploded".to_owned())),
            ..FakeGhClient::default_for_pr()
        };
        let ctx = PrBranchContext {
            branch: "track/my-feature".to_owned(),
            track_id: "my-feature".to_owned(),
            is_plan_branch: false,
        };
        assert_eq!(ensure_pr_with(&ctx, "main", &client), ExitCode::FAILURE);
    }

    #[test]
    fn ensure_pr_with_returns_failure_on_create_error() {
        let client = FakeGhClient {
            find_pr_result: RefCell::new(Ok(None)),
            create_pr_result: RefCell::new(Err("create failed".to_owned())),
            ..FakeGhClient::default_for_pr()
        };
        let ctx = PrBranchContext {
            branch: "track/my-feature".to_owned(),
            track_id: "my-feature".to_owned(),
            is_plan_branch: false,
        };
        assert_eq!(ensure_pr_with(&ctx, "main", &client), ExitCode::FAILURE);
    }
}
