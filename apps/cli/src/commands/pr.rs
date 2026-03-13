//! CLI subcommands for pull-request workflow wrappers.

use std::process::{Command, ExitCode, Output};
use std::thread;
use std::time::{Duration, Instant};

use clap::{Args, Subcommand};
use serde::Deserialize;
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

#[derive(Debug, Deserialize)]
struct PrCheck {
    #[serde(default)]
    name: String,
    #[serde(default)]
    state: String,
    #[serde(default)]
    bucket: String,
}

pub fn execute(cmd: PrCommand) -> ExitCode {
    match cmd {
        PrCommand::Status(args) => status(&args.pr),
        PrCommand::WaitAndMerge(args) => {
            wait_and_merge(&args.pr, args.interval, args.timeout, &args.method)
        }
    }
}

fn run_gh(args: &[&str]) -> Result<Output, String> {
    Command::new("gh")
        .args(args)
        .output()
        .map_err(|err| format!("failed to run gh {}: {err}", args.join(" ")))
}

fn normalize_check_status(check: &PrCheck) -> PrCheckStatus {
    let state = if !check.bucket.is_empty() { check.bucket.as_str() } else { check.state.as_str() };

    match state.to_uppercase().as_str() {
        "SUCCESS" | "PASS" | "SKIPPING" => PrCheckStatus::Passed,
        "FAILURE" | "FAIL" | "CANCEL" => PrCheckStatus::Failed,
        _ => PrCheckStatus::Pending,
    }
}

fn decode_pr_checks(stdout: &[u8], pr: &str) -> Result<Vec<PrCheck>, String> {
    serde_json::from_slice(stdout)
        .map_err(|err| format!("failed to decode gh pr checks JSON for PR #{pr}: {err}"))
}

fn get_pr_checks_with<F>(pr: &str, run_gh: &F) -> Result<Vec<PrCheck>, String>
where
    F: Fn(&[&str]) -> Result<Output, String>,
{
    let output = run_gh(&["pr", "checks", pr, "--json", "name,state,bucket,completedAt"])?;
    if !output.stdout.is_empty() {
        return decode_pr_checks(&output.stdout, pr);
    }
    if output.status.success() {
        return Ok(Vec::new());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    Err(if stderr.is_empty() {
        format!("gh pr checks {pr} failed")
    } else {
        format!("gh pr checks {pr} failed: {stderr}")
    })
}

fn get_pr_checks(pr: &str) -> Result<Vec<PrCheck>, String> {
    get_pr_checks_with(pr, &run_gh)
}

fn checks_summary(checks: &[PrCheck]) -> CheckSummary {
    let checks = checks
        .iter()
        .map(|check| PrCheckView {
            name: check.name.clone(),
            status: normalize_check_status(check),
        })
        .collect::<Vec<_>>();
    summarize_checks(&checks)
}

fn get_pr_url_with<F>(pr: &str, run_gh: &F) -> String
where
    F: Fn(&[&str]) -> Result<Output, String>,
{
    let Ok(output) = run_gh(&["pr", "view", pr, "--json", "url", "-q", ".url"]) else {
        return format!("PR #{pr}");
    };
    if !output.status.success() {
        return format!("PR #{pr}");
    }
    let url = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if url.is_empty() { format!("PR #{pr}") } else { url }
}

fn get_pr_url(pr: &str) -> String {
    get_pr_url_with(pr, &run_gh)
}

fn status(pr: &str) -> ExitCode {
    let checks = match get_pr_checks(pr) {
        Ok(checks) => checks,
        Err(err) => {
            println!("[ERROR] {err}");
            return ExitCode::FAILURE;
        }
    };

    let url = get_pr_url(pr);
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

fn merge_pr_with<F>(pr: &str, method: &str, run_gh: &F) -> ExitCode
where
    F: Fn(&[&str]) -> Result<Output, String>,
{
    println!("[OK] All checks passed. Merging...");
    let args = ["pr", "merge", pr, &format!("--{method}")];
    match run_gh(&args) {
        Ok(output) if output.status.success() => {
            println!("[OK] PR #{pr} merged ({method}).");
            ExitCode::SUCCESS
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
            println!("[ERROR] Merge failed: {stderr}");
            ExitCode::FAILURE
        }
        Err(err) => {
            println!("[ERROR] Merge failed: {err}");
            ExitCode::FAILURE
        }
    }
}

fn wait_and_merge_with<Run, Sleep>(
    pr: &str,
    interval: u64,
    timeout: u64,
    method: &str,
    run_gh: &Run,
    sleep: &Sleep,
) -> ExitCode
where
    Run: Fn(&[&str]) -> Result<Output, String>,
    Sleep: Fn(Duration),
{
    let url = get_pr_url_with(pr, run_gh);
    println!("PR: {url}");
    println!("Polling checks every {interval}s (timeout {timeout}s)...");

    let start = Instant::now();
    loop {
        let elapsed = start.elapsed().as_secs();
        let checks = match get_pr_checks_with(pr, run_gh) {
            Ok(checks) => checks,
            Err(err) => {
                println!("[ERROR] {err}");
                return ExitCode::FAILURE;
            }
        };
        match decide_wait_action(checks_summary(&checks), elapsed, timeout, interval) {
            WaitDecision::MergeNow => return merge_pr_with(pr, method, run_gh),
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
    wait_and_merge_with(pr, interval, timeout, method, &run_gh, &thread::sleep)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use std::cell::RefCell;
    use std::os::unix::process::ExitStatusExt;
    use std::process::{ExitCode, ExitStatus, Output};

    use super::{
        CheckSummary, PrCheck, checks_summary, decode_pr_checks, get_pr_checks_with,
        normalize_check_status, wait_and_merge_with,
    };
    use usecase::pr_workflow::PrCheckStatus;

    fn output(code: i32, stdout: &str, stderr: &str) -> Output {
        Output {
            status: ExitStatus::from_raw(code),
            stdout: stdout.as_bytes().to_vec(),
            stderr: stderr.as_bytes().to_vec(),
        }
    }

    #[test]
    fn decode_pr_checks_accepts_bucket_field() {
        let checks =
            decode_pr_checks(br#"[{"name":"ci","state":"PENDING","bucket":"pending"}]"#, "123")
                .unwrap();

        assert_eq!(checks.len(), 1);
        assert_eq!(checks[0].bucket, "pending");
    }

    #[test]
    fn normalize_check_status_prefers_bucket_over_state() {
        let check = PrCheck {
            name: "ci".to_owned(),
            state: "SUCCESS".to_owned(),
            bucket: "pending".to_owned(),
        };

        assert_eq!(normalize_check_status(&check), PrCheckStatus::Pending);
    }

    #[test]
    fn get_pr_checks_with_accepts_json_on_nonzero_exit() {
        let checks = get_pr_checks_with("123", &|args| {
            assert_eq!(args, ["pr", "checks", "123", "--json", "name,state,bucket,completedAt"]);
            Ok(output(1, r#"[{"name":"ci","state":"PENDING","bucket":"pending"}]"#, ""))
        })
        .unwrap();

        assert_eq!(checks.len(), 1);
        assert_eq!(checks[0].name, "ci");
    }

    #[test]
    fn get_pr_checks_with_propagates_stderr_when_no_json_is_present() {
        let err = get_pr_checks_with("123", &|_| Ok(output(1, "", "gh exploded"))).unwrap_err();

        assert_eq!(err, "gh pr checks 123 failed: gh exploded");
    }

    #[test]
    fn checks_summary_maps_normalized_cli_checks_to_pending() {
        let checks = vec![PrCheck {
            name: "ci".to_owned(),
            state: "SUCCESS".to_owned(),
            bucket: "pending".to_owned(),
        }];

        assert_eq!(checks_summary(&checks), CheckSummary::Pending(vec!["ci".to_owned()]));
    }

    #[test]
    fn wait_and_merge_with_merges_after_all_checks_pass() {
        let calls = RefCell::new(Vec::<Vec<String>>::new());
        let result = wait_and_merge_with(
            "123",
            15,
            600,
            "squash",
            &|args| {
                calls.borrow_mut().push(args.iter().map(|arg| (*arg).to_owned()).collect());
                match args {
                    ["pr", "view", "123", "--json", "url", "-q", ".url"] => {
                        Ok(output(0, "https://example.invalid/pr/123\n", ""))
                    }
                    ["pr", "checks", "123", "--json", "name,state,bucket,completedAt"] => {
                        Ok(output(0, r#"[{"name":"ci","state":"SUCCESS","bucket":""}]"#, ""))
                    }
                    ["pr", "merge", "123", "--squash"] => Ok(output(0, "", "")),
                    other => panic!("unexpected args: {other:?}"),
                }
            },
            &|_| panic!("sleep should not be called"),
        );

        assert_eq!(result, ExitCode::SUCCESS);
        assert_eq!(
            calls.into_inner(),
            vec![
                vec!["pr", "view", "123", "--json", "url", "-q", ".url"],
                vec!["pr", "checks", "123", "--json", "name,state,bucket,completedAt"],
                vec!["pr", "merge", "123", "--squash"],
            ]
        );
    }

    #[test]
    fn wait_and_merge_with_returns_failure_when_checks_api_errors() {
        let result = wait_and_merge_with(
            "123",
            15,
            600,
            "merge",
            &|args| match args {
                ["pr", "view", "123", "--json", "url", "-q", ".url"] => Ok(output(0, "", "")),
                ["pr", "checks", "123", "--json", "name,state,bucket,completedAt"] => {
                    Err("boom".to_owned())
                }
                other => panic!("unexpected args: {other:?}"),
            },
            &|_| panic!("sleep should not be called"),
        );

        assert_eq!(result, ExitCode::FAILURE);
    }

    #[test]
    fn wait_and_merge_with_times_out_pending_checks_without_sleep_when_deadline_reached() {
        let sleeps = RefCell::new(Vec::new());
        let result = wait_and_merge_with(
            "123",
            15,
            0,
            "merge",
            &|args| match args {
                ["pr", "view", "123", "--json", "url", "-q", ".url"] => Ok(output(0, "", "")),
                ["pr", "checks", "123", "--json", "name,state,bucket,completedAt"] => {
                    Ok(output(1, r#"[{"name":"ci","state":"PENDING","bucket":"pending"}]"#, ""))
                }
                other => panic!("unexpected args: {other:?}"),
            },
            &|duration| sleeps.borrow_mut().push(duration),
        );

        assert_eq!(result, ExitCode::FAILURE);
        assert!(sleeps.borrow().is_empty());
    }
}
