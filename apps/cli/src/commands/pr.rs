//! CLI subcommands for pull-request workflow wrappers.

use std::process::{Command, ExitCode, Output};
use std::thread;
use std::time::{Duration, Instant};

use clap::{Args, Subcommand};
use serde::Deserialize;

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

#[derive(Debug, PartialEq, Eq)]
enum CheckSummary {
    AllPassed,
    Failed(Vec<String>),
    Pending(Vec<String>),
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

fn decode_pr_checks(stdout: &[u8], pr: &str) -> Result<Vec<PrCheck>, String> {
    serde_json::from_slice(stdout)
        .map_err(|err| format!("failed to decode gh pr checks JSON for PR #{pr}: {err}"))
}

fn get_pr_checks(pr: &str) -> Result<Vec<PrCheck>, String> {
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

fn check_status(check: &PrCheck) -> &str {
    if !check.bucket.is_empty() {
        return check.bucket.as_str();
    }
    check.state.as_str()
}

fn checks_summary(checks: &[PrCheck]) -> CheckSummary {
    if checks.is_empty() {
        return CheckSummary::Pending(vec!["(no checks found)".to_owned()]);
    }

    let mut pending = Vec::new();
    let mut failed = Vec::new();
    for check in checks {
        let state = check_status(check).to_uppercase();
        let name = if check.name.is_empty() { "unknown".to_owned() } else { check.name.clone() };
        if matches!(state.as_str(), "SUCCESS" | "PASS" | "SKIPPING") {
            continue;
        }
        if matches!(state.as_str(), "FAILURE" | "FAIL" | "CANCEL") {
            failed.push(name);
        } else {
            pending.push(name);
        }
    }

    if !failed.is_empty() {
        return CheckSummary::Failed(failed);
    }
    if !pending.is_empty() {
        return CheckSummary::Pending(pending);
    }
    CheckSummary::AllPassed
}

fn get_pr_url(pr: &str) -> String {
    let Ok(output) = run_gh(&["pr", "view", pr, "--json", "url", "-q", ".url"]) else {
        return format!("PR #{pr}");
    };
    if !output.status.success() {
        return format!("PR #{pr}");
    }
    let url = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if url.is_empty() { format!("PR #{pr}") } else { url }
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

fn wait_and_merge(pr: &str, interval: u64, timeout: u64, method: &str) -> ExitCode {
    let url = get_pr_url(pr);
    println!("PR: {url}");
    println!("Polling checks every {interval}s (timeout {timeout}s)...");

    let start = Instant::now();
    loop {
        let elapsed = start.elapsed().as_secs();
        let checks = match get_pr_checks(pr) {
            Ok(checks) => checks,
            Err(err) => {
                println!("[ERROR] {err}");
                return ExitCode::FAILURE;
            }
        };
        match checks_summary(&checks) {
            CheckSummary::AllPassed => {
                println!("[OK] All checks passed. Merging...");
                let args = ["pr", "merge", pr, &format!("--{method}")];
                match run_gh(&args) {
                    Ok(output) if output.status.success() => {
                        println!("[OK] PR #{pr} merged ({method}).");
                        return ExitCode::SUCCESS;
                    }
                    Ok(output) => {
                        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
                        println!("[ERROR] Merge failed: {stderr}");
                        return ExitCode::FAILURE;
                    }
                    Err(err) => {
                        println!("[ERROR] Merge failed: {err}");
                        return ExitCode::FAILURE;
                    }
                }
            }
            CheckSummary::Failed(names) => {
                println!("[FAIL] Checks failed: {}", names.join(", "));
                println!("Fix the failures and push again.");
                return ExitCode::FAILURE;
            }
            CheckSummary::Pending(names) => {
                if elapsed >= timeout {
                    println!("[TIMEOUT] Still pending after {timeout}s: {}", names.join(", "));
                    return ExitCode::FAILURE;
                }
                let remaining = timeout.saturating_sub(elapsed);
                let delay = interval.min(remaining);
                println!("  [{elapsed}s] Pending: {} (retry in {delay}s)", names.join(", "));
                thread::sleep(Duration::from_secs(delay));
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::{CheckSummary, PrCheck, checks_summary, decode_pr_checks};

    #[test]
    fn checks_summary_reports_success_when_all_checks_pass() {
        let checks = vec![
            PrCheck { name: "ci".to_owned(), state: "SUCCESS".to_owned(), bucket: String::new() },
            PrCheck { name: "lint".to_owned(), state: "success".to_owned(), bucket: String::new() },
        ];

        assert_eq!(checks_summary(&checks), CheckSummary::AllPassed);
    }

    #[test]
    fn checks_summary_reports_failed_checks() {
        let checks = vec![
            PrCheck { name: "ci".to_owned(), state: "SUCCESS".to_owned(), bucket: String::new() },
            PrCheck {
                name: "review".to_owned(),
                state: "FAILURE".to_owned(),
                bucket: String::new(),
            },
        ];

        assert_eq!(checks_summary(&checks), CheckSummary::Failed(vec!["review".to_owned()]));
    }

    #[test]
    fn checks_summary_reports_pending_checks_and_empty_sets() {
        let checks = vec![
            PrCheck {
                name: "ci".to_owned(),
                state: "IN_PROGRESS".to_owned(),
                bucket: String::new(),
            },
            PrCheck { name: String::new(), state: "queued".to_owned(), bucket: String::new() },
        ];

        assert_eq!(
            checks_summary(&checks),
            CheckSummary::Pending(vec!["ci".to_owned(), "unknown".to_owned()])
        );
        assert_eq!(
            checks_summary(&[]),
            CheckSummary::Pending(vec!["(no checks found)".to_owned()])
        );
    }

    #[test]
    fn checks_summary_prefers_bucket_status_when_present() {
        let checks = vec![
            PrCheck {
                name: "ci".to_owned(),
                state: "SUCCESS".to_owned(),
                bucket: "pass".to_owned(),
            },
            PrCheck {
                name: "lint".to_owned(),
                state: "SUCCESS".to_owned(),
                bucket: "pending".to_owned(),
            },
            PrCheck {
                name: "review".to_owned(),
                state: "SUCCESS".to_owned(),
                bucket: "fail".to_owned(),
            },
        ];

        assert_eq!(checks_summary(&checks), CheckSummary::Failed(vec!["review".to_owned()]));
    }

    #[test]
    fn decode_pr_checks_accepts_bucket_field() {
        let checks =
            decode_pr_checks(br#"[{"name":"ci","state":"PENDING","bucket":"pending"}]"#, "123")
                .unwrap();

        assert_eq!(checks.len(), 1);
        assert_eq!(checks[0].bucket, "pending");
    }
}
