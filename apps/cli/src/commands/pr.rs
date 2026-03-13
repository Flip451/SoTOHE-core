//! CLI subcommands for pull-request workflow wrappers.

use std::process::ExitCode;
use std::thread;
use std::time::{Duration, Instant};

use clap::{Args, Subcommand};
use infrastructure::gh_cli::{GhClient, PrCheckRecord, SystemGhClient};
use usecase::pr_workflow::{
    CheckSummary, PrCheckStatus, PrCheckView, WaitDecision, decide_wait_action, summarize_checks,
};

#[derive(Debug, Subcommand)]
pub enum PrCommand {
    /// Show current PR check status.
    Status(StatusArgs),
    /// Poll PR checks until they pass, then merge.
    WaitAndMerge(WaitAndMergeArgs),
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
        PrCommand::Status(args) => status(&args.pr),
        PrCommand::WaitAndMerge(args) => {
            wait_and_merge(&args.pr, args.interval, args.timeout, &args.method)
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

    use super::{
        CheckSummary, checks_summary, normalize_check_status, status_with, wait_and_merge_with,
    };
    use infrastructure::gh_cli::{GhClient, PrCheckRecord};
    use usecase::pr_workflow::PrCheckStatus;

    struct FakeGhClient {
        checks: RefCell<Vec<Result<Vec<PrCheckRecord>, String>>>,
        url: String,
        merge_calls: RefCell<Vec<(String, String)>>,
        merge_result: RefCell<Result<(), String>>,
    }

    impl GhClient for FakeGhClient {
        fn pr_checks(&self, _pr: &str) -> Result<Vec<PrCheckRecord>, String> {
            self.checks.borrow_mut().remove(0)
        }

        fn pr_url(&self, pr: &str) -> String {
            if self.url.is_empty() { format!("PR #{pr}") } else { self.url.clone() }
        }

        fn merge_pr(&self, pr: &str, method: &str) -> Result<(), String> {
            self.merge_calls.borrow_mut().push((pr.to_owned(), method.to_owned()));
            self.merge_result.borrow().clone()
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
            checks: RefCell::new(vec![Err("gh exploded".to_owned())]),
            url: String::new(),
            merge_calls: RefCell::new(Vec::new()),
            merge_result: RefCell::new(Ok(())),
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
            merge_calls: RefCell::new(Vec::new()),
            merge_result: RefCell::new(Ok(())),
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
            checks: RefCell::new(vec![Err("boom".to_owned())]),
            url: String::new(),
            merge_calls: RefCell::new(Vec::new()),
            merge_result: RefCell::new(Ok(())),
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
            url: String::new(),
            merge_calls: RefCell::new(Vec::new()),
            merge_result: RefCell::new(Ok(())),
        };
        let result = wait_and_merge_with("123", 15, 0, "merge", &client, &|duration| {
            sleeps.borrow_mut().push(duration)
        });

        assert_eq!(result, ExitCode::FAILURE);
        assert!(sleeps.borrow().is_empty());
    }
}
