use std::path::Path;
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

    /// Find an open PR with the given head and base branches.
    ///
    /// Returns the PR number as a string if found, or None.
    fn find_open_pr(&self, head: &str, base: &str) -> Result<Option<String>, String>;

    /// Create a PR using a body file to avoid shell escaping / hook issues.
    ///
    /// Returns the PR number as a string.
    fn create_pr(
        &self,
        head: &str,
        base: &str,
        title: &str,
        body_file: &Path,
    ) -> Result<String, String>;
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

    fn find_open_pr(&self, head: &str, base: &str) -> Result<Option<String>, String> {
        find_open_pr_with(head, base, &run_gh)
    }

    fn create_pr(
        &self,
        head: &str,
        base: &str,
        title: &str,
        body_file: &Path,
    ) -> Result<String, String> {
        create_pr_with(head, base, title, body_file, &run_gh)
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

fn find_open_pr_with<F>(head: &str, base: &str, run_gh: &F) -> Result<Option<String>, String>
where
    F: Fn(&[&str]) -> Result<Output, String>,
{
    let output = run_gh(&[
        "pr",
        "list",
        "--head",
        head,
        "--base",
        base,
        "--state",
        "open",
        "--json",
        "number",
        "-q",
        ".[0].number",
    ])?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        return Err(if stderr.is_empty() {
            "gh pr list failed".to_owned()
        } else {
            format!("gh pr list failed: {stderr}")
        });
    }
    let number = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    // gh pr list -q '.[0].number' returns "null" (literal) when no PR exists.
    if number.is_empty() || number == "null" { Ok(None) } else { Ok(Some(number)) }
}

fn create_pr_with<F>(
    head: &str,
    base: &str,
    title: &str,
    body_file: &Path,
    run_gh: &F,
) -> Result<String, String>
where
    F: Fn(&[&str]) -> Result<Output, String>,
{
    let body_path = body_file
        .to_str()
        .ok_or_else(|| format!("body file path is not valid UTF-8: {}", body_file.display()))?;
    let output = run_gh(&[
        "pr",
        "create",
        "--head",
        head,
        "--base",
        base,
        "--title",
        title,
        "--body-file",
        body_path,
    ])?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        return Err(if stderr.is_empty() {
            "gh pr create failed".to_owned()
        } else {
            format!("gh pr create failed: {stderr}")
        });
    }
    let url = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    // Extract PR number from URL (e.g., https://github.com/owner/repo/pull/14)
    if let Some(num) = url.rsplit('/').next().and_then(|s| s.parse::<u64>().ok()) {
        return Ok(num.to_string());
    }
    // Fallback: try gh pr view
    let view = run_gh(&["pr", "view", head, "--json", "number", "-q", ".number"])?;
    if !view.status.success() {
        let stderr = String::from_utf8_lossy(&view.stderr).trim().to_owned();
        return Err(if stderr.is_empty() {
            "gh pr create succeeded but gh pr view failed".to_owned()
        } else {
            format!("gh pr create succeeded but gh pr view failed: {stderr}")
        });
    }
    let number = String::from_utf8_lossy(&view.stdout).trim().to_owned();
    if number.is_empty() || number == "null" {
        Err("gh pr create succeeded but could not determine PR number".to_owned())
    } else {
        Ok(number)
    }
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

    use std::path::Path;

    use rstest::rstest;

    use super::{
        PrCheckRecord, create_pr_with, decode_pr_checks, find_open_pr_with, merge_pr_with,
        pr_checks_with,
    };

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

    // --- find_open_pr_with tests ---

    #[test]
    fn find_open_pr_with_returns_some_when_pr_exists() {
        let result = find_open_pr_with("track/feat", "main", &|args| {
            assert_eq!(
                args,
                [
                    "pr",
                    "list",
                    "--head",
                    "track/feat",
                    "--base",
                    "main",
                    "--state",
                    "open",
                    "--json",
                    "number",
                    "-q",
                    ".[0].number"
                ]
            );
            Ok(output(0, "42\n", ""))
        });
        assert_eq!(result.unwrap(), Some("42".to_owned()));
    }

    #[rstest]
    #[case::null_stdout("null\n")]
    #[case::empty_stdout("")]
    fn find_open_pr_with_returns_none_for_absent_pr(#[case] stdout: &str) {
        let result = find_open_pr_with("track/feat", "main", &|_| Ok(output(0, stdout, "")));
        assert_eq!(result.unwrap(), None);
    }

    #[test]
    fn find_open_pr_with_surfaces_stderr_on_failure() {
        let err = find_open_pr_with("track/feat", "main", &|_| Ok(output(1, "", "auth error")))
            .unwrap_err();
        assert!(err.contains("auth error"), "got: {err}");
    }

    // --- create_pr_with tests ---

    #[test]
    fn create_pr_with_extracts_pr_number_from_url() {
        let result = create_pr_with("track/feat", "main", "title", Path::new("body.md"), &|_| {
            Ok(output(0, "https://github.com/owner/repo/pull/99\n", ""))
        });
        assert_eq!(result.unwrap(), "99");
    }

    #[test]
    fn create_pr_with_falls_back_to_view_on_non_url_stdout() {
        let call_count = std::cell::Cell::new(0);
        let result = create_pr_with("track/feat", "main", "title", Path::new("body.md"), &|args| {
            let n = call_count.get();
            call_count.set(n + 1);
            if n == 0 {
                // gh pr create succeeds but returns non-URL output
                Ok(output(0, "Created pull request\n", ""))
            } else {
                // gh pr view fallback — verify argv
                assert_eq!(args, ["pr", "view", "track/feat", "--json", "number", "-q", ".number"]);
                Ok(output(0, "55\n", ""))
            }
        });
        assert_eq!(result.unwrap(), "55");
    }

    #[test]
    fn create_pr_with_verifies_argv() {
        let result = create_pr_with("track/feat", "main", "my title", Path::new("b.md"), &|args| {
            assert_eq!(
                args,
                [
                    "pr",
                    "create",
                    "--head",
                    "track/feat",
                    "--base",
                    "main",
                    "--title",
                    "my title",
                    "--body-file",
                    "b.md"
                ]
            );
            Ok(output(0, "https://github.com/o/r/pull/1\n", ""))
        });
        assert_eq!(result.unwrap(), "1");
    }

    #[test]
    fn create_pr_with_surfaces_stderr_on_create_failure() {
        let err = create_pr_with("track/feat", "main", "title", Path::new("body.md"), &|_| {
            Ok(output(1, "", "create error"))
        })
        .unwrap_err();
        assert!(err.contains("create error"), "got: {err}");
    }

    #[test]
    fn create_pr_with_surfaces_stderr_when_view_fallback_fails() {
        let call_count = std::cell::Cell::new(0);
        let err = create_pr_with("track/feat", "main", "title", Path::new("body.md"), &|_| {
            let n = call_count.get();
            call_count.set(n + 1);
            if n == 0 {
                // gh pr create succeeds but returns non-URL output
                Ok(output(0, "not-a-url\n", ""))
            } else {
                // gh pr view fails
                Ok(output(1, "", "view auth error"))
            }
        })
        .unwrap_err();
        assert!(err.contains("view auth error"), "got: {err}");
    }

    #[test]
    fn create_pr_with_returns_error_when_view_returns_null() {
        let call_count = std::cell::Cell::new(0);
        let err = create_pr_with("track/feat", "main", "title", Path::new("body.md"), &|_| {
            let n = call_count.get();
            call_count.set(n + 1);
            if n == 0 { Ok(output(0, "not-a-url\n", "")) } else { Ok(output(0, "null\n", "")) }
        })
        .unwrap_err();
        assert!(err.contains("could not determine PR number"), "got: {err}");
    }
}
