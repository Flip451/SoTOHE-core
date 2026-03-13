use std::process::{Command, Output};

use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PrCheckRecord {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub state: String,
    #[serde(default)]
    pub bucket: String,
}

pub trait GhClient {
    fn pr_checks(&self, pr: &str) -> Result<Vec<PrCheckRecord>, String>;
    fn pr_url(&self, pr: &str) -> String;
    fn merge_pr(&self, pr: &str, method: &str) -> Result<(), String>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SystemGhClient;

impl GhClient for SystemGhClient {
    fn pr_checks(&self, pr: &str) -> Result<Vec<PrCheckRecord>, String> {
        pr_checks_with(pr, &run_gh)
    }

    fn pr_url(&self, pr: &str) -> String {
        pr_url_with(pr, &run_gh)
    }

    fn merge_pr(&self, pr: &str, method: &str) -> Result<(), String> {
        merge_pr_with(pr, method, &run_gh)
    }
}

fn run_gh(args: &[&str]) -> Result<Output, String> {
    Command::new("gh")
        .args(args)
        .output()
        .map_err(|err| format!("failed to run gh {}: {err}", args.join(" ")))
}

fn pr_checks_with<F>(pr: &str, run_gh: &F) -> Result<Vec<PrCheckRecord>, String>
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

fn pr_url_with<F>(pr: &str, run_gh: &F) -> String
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

fn merge_pr_with<F>(pr: &str, method: &str, run_gh: &F) -> Result<(), String>
where
    F: Fn(&[&str]) -> Result<Output, String>,
{
    let args = ["pr", "merge", pr, &format!("--{method}")];
    let output = run_gh(&args)?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    Err(if stderr.is_empty() {
        format!("gh pr merge {pr} --{method} failed")
    } else {
        format!("gh pr merge {pr} --{method} failed: {stderr}")
    })
}

fn decode_pr_checks(stdout: &[u8], pr: &str) -> Result<Vec<PrCheckRecord>, String> {
    serde_json::from_slice(stdout)
        .map_err(|err| format!("failed to decode gh pr checks JSON for PR #{pr}: {err}"))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use std::os::unix::process::ExitStatusExt;
    use std::process::{ExitStatus, Output};

    use super::{PrCheckRecord, decode_pr_checks, merge_pr_with, pr_checks_with};

    fn output(code: i32, stdout: &str, stderr: &str) -> Output {
        Output {
            status: ExitStatus::from_raw(code << 8),
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
        assert_eq!(
            checks[0],
            PrCheckRecord {
                name: "ci".to_owned(),
                state: "PENDING".to_owned(),
                bucket: "pending".to_owned(),
            }
        );
    }

    #[test]
    fn pr_checks_with_accepts_json_on_nonzero_exit() {
        let checks = pr_checks_with("123", &|args| {
            assert_eq!(args, ["pr", "checks", "123", "--json", "name,state,bucket,completedAt"]);
            Ok(output(1, r#"[{"name":"ci","state":"PENDING","bucket":"pending"}]"#, ""))
        })
        .unwrap();

        assert_eq!(checks.len(), 1);
        assert_eq!(checks[0].name, "ci");
    }

    #[test]
    fn pr_checks_with_surfaces_stderr_when_no_json_is_present() {
        let err = pr_checks_with("123", &|_| Ok(output(1, "", "gh exploded"))).unwrap_err();

        assert_eq!(err, "gh pr checks 123 failed: gh exploded");
    }

    #[test]
    fn merge_pr_with_surfaces_stderr_on_failure() {
        let err =
            merge_pr_with("123", "squash", &|_| Ok(output(1, "", "merge exploded"))).unwrap_err();

        assert_eq!(err, "gh pr merge 123 --squash failed: merge exploded");
    }
}
