use std::path::Path;
use std::process::{Command, Output};

use serde::Deserialize;
use thiserror::Error;

/// Structured error type for gh CLI operations.
#[derive(Debug, Error)]
pub enum GhError {
    #[error("failed to run gh {command}: {source}")]
    Spawn {
        command: String,
        #[source]
        source: std::io::Error,
    },
    #[error("{}", if stderr.is_empty() { format!("gh {command} failed") } else { format!("gh {command} failed: {stderr}") })]
    CommandFailed { command: String, stderr: String },
    #[error("failed to decode gh pr checks JSON for PR #{pr}: {source}")]
    JsonDecode {
        pr: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("body file path is not valid UTF-8: {0}")]
    InvalidBodyPath(String),
    #[error("gh pr create succeeded but could not determine PR number")]
    PrNumberUnknown,
}

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
    fn pr_checks(&self, pr: &str) -> Result<Vec<PrCheckRecord>, GhError>;
    fn pr_url(&self, pr: &str) -> String;
    fn merge_pr(&self, pr: &str, method: &str) -> Result<(), GhError>;

    /// Find an open PR with the given head and base branches.
    ///
    /// Returns the PR number as a string if found, or None.
    fn find_open_pr(&self, head: &str, base: &str) -> Result<Option<String>, GhError>;

    /// Create a PR using a body file to avoid shell escaping / hook issues.
    ///
    /// Returns the PR number as a string.
    fn create_pr(
        &self,
        head: &str,
        base: &str,
        title: &str,
        body_file: &Path,
    ) -> Result<String, GhError>;

    /// Post a comment on a PR (issue comments endpoint).
    ///
    /// Returns the raw JSON response from the GitHub API.
    /// Default implementation returns an error to maintain backward compatibility.
    ///
    /// # Errors
    /// Returns `GhError::CommandFailed` if the gh command exits with a non-zero status.
    fn post_issue_comment(
        &self,
        _repo_nwo: &str,
        _pr: &str,
        _body: &str,
    ) -> Result<String, GhError> {
        Err(GhError::CommandFailed {
            command: "post_issue_comment".to_owned(),
            stderr: "not implemented".to_owned(),
        })
    }

    /// List all reviews on a PR.
    ///
    /// Returns the raw JSON response from the GitHub API.
    /// Default implementation returns an error to maintain backward compatibility.
    ///
    /// # Errors
    /// Returns `GhError::CommandFailed` if the gh command exits with a non-zero status.
    fn list_reviews(&self, _repo_nwo: &str, _pr: &str) -> Result<String, GhError> {
        Err(GhError::CommandFailed {
            command: "list_reviews".to_owned(),
            stderr: "not implemented".to_owned(),
        })
    }

    /// List all issue comments on a PR.
    ///
    /// Returns the raw JSON response from the GitHub API.
    /// Default implementation returns an error to maintain backward compatibility.
    ///
    /// # Errors
    /// Returns `GhError::CommandFailed` if the gh command exits with a non-zero status.
    fn list_issue_comments(&self, _repo_nwo: &str, _pr: &str) -> Result<String, GhError> {
        Err(GhError::CommandFailed {
            command: "list_issue_comments".to_owned(),
            stderr: "not implemented".to_owned(),
        })
    }

    /// List all inline comments for a specific review on a PR.
    ///
    /// Returns the raw JSON response from the GitHub API.
    /// Default implementation returns an error to maintain backward compatibility.
    ///
    /// # Errors
    /// Returns `GhError::CommandFailed` if the gh command exits with a non-zero status.
    fn list_review_comments(
        &self,
        _repo_nwo: &str,
        _pr: &str,
        _review_id: &str,
    ) -> Result<String, GhError> {
        Err(GhError::CommandFailed {
            command: "list_review_comments".to_owned(),
            stderr: "not implemented".to_owned(),
        })
    }

    /// List all reactions on a PR (issue reactions endpoint).
    ///
    /// Returns the raw JSON response from the GitHub API.
    /// Default implementation returns an error to maintain backward compatibility.
    ///
    /// # Errors
    /// Returns `GhError::CommandFailed` if the gh command exits with a non-zero status.
    fn list_reactions(&self, _repo_nwo: &str, _pr: &str) -> Result<String, GhError> {
        Err(GhError::CommandFailed {
            command: "list_reactions".to_owned(),
            stderr: "not implemented".to_owned(),
        })
    }

    /// Return the owner/repo string for the current repository.
    ///
    /// Default implementation returns an error to maintain backward compatibility.
    ///
    /// # Errors
    /// Returns `GhError::CommandFailed` if the gh command exits with a non-zero status.
    fn repo_nwo(&self) -> Result<String, GhError> {
        Err(GhError::CommandFailed {
            command: "repo_nwo".to_owned(),
            stderr: "not implemented".to_owned(),
        })
    }

    /// Return the head branch name for a PR (e.g. `track/my-feature`).
    ///
    /// # Errors
    /// Returns `GhError::CommandFailed` if the gh command fails.
    fn pr_head_branch(&self, _pr: &str) -> Result<String, GhError> {
        Err(GhError::CommandFailed {
            command: "pr_head_branch".to_owned(),
            stderr: "not implemented".to_owned(),
        })
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SystemGhClient;

impl GhClient for SystemGhClient {
    fn pr_checks(&self, pr: &str) -> Result<Vec<PrCheckRecord>, GhError> {
        pr_checks_with(pr, &run_gh)
    }

    fn pr_url(&self, pr: &str) -> String {
        pr_url_with(pr, &run_gh)
    }

    fn merge_pr(&self, pr: &str, method: &str) -> Result<(), GhError> {
        merge_pr_with(pr, method, &run_gh)
    }

    fn find_open_pr(&self, head: &str, base: &str) -> Result<Option<String>, GhError> {
        find_open_pr_with(head, base, &run_gh)
    }

    fn create_pr(
        &self,
        head: &str,
        base: &str,
        title: &str,
        body_file: &Path,
    ) -> Result<String, GhError> {
        create_pr_with(head, base, title, body_file, &run_gh)
    }

    fn post_issue_comment(&self, repo_nwo: &str, pr: &str, body: &str) -> Result<String, GhError> {
        post_issue_comment_with(repo_nwo, pr, body, &run_gh)
    }

    fn list_reviews(&self, repo_nwo: &str, pr: &str) -> Result<String, GhError> {
        list_reviews_with(repo_nwo, pr, &run_gh)
    }

    fn list_issue_comments(&self, repo_nwo: &str, pr: &str) -> Result<String, GhError> {
        list_issue_comments_with(repo_nwo, pr, &run_gh)
    }

    fn list_review_comments(
        &self,
        repo_nwo: &str,
        pr: &str,
        review_id: &str,
    ) -> Result<String, GhError> {
        list_review_comments_with(repo_nwo, pr, review_id, &run_gh)
    }

    fn list_reactions(&self, repo_nwo: &str, pr: &str) -> Result<String, GhError> {
        list_reactions_with(repo_nwo, pr, &run_gh)
    }

    fn repo_nwo(&self) -> Result<String, GhError> {
        repo_nwo_with(&run_gh)
    }

    fn pr_head_branch(&self, pr: &str) -> Result<String, GhError> {
        pr_head_branch_with(pr, &run_gh)
    }
}

fn run_gh(args: &[&str]) -> Result<Output, GhError> {
    Command::new("gh")
        .args(args)
        .output()
        .map_err(|source| GhError::Spawn { command: args.join(" "), source })
}

fn pr_checks_with<F>(pr: &str, run_gh: &F) -> Result<Vec<PrCheckRecord>, GhError>
where
    F: Fn(&[&str]) -> Result<Output, GhError>,
{
    let output = run_gh(&["pr", "checks", pr, "--json", "name,state,bucket,completedAt"])?;
    if !output.stdout.is_empty() {
        return decode_pr_checks(&output.stdout, pr);
    }
    if output.status.success() {
        return Ok(Vec::new());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    Err(GhError::CommandFailed { command: format!("pr checks {pr}"), stderr })
}

fn pr_url_with<F>(pr: &str, run_gh: &F) -> String
where
    F: Fn(&[&str]) -> Result<Output, GhError>,
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

fn merge_pr_with<F>(pr: &str, method: &str, run_gh: &F) -> Result<(), GhError>
where
    F: Fn(&[&str]) -> Result<Output, GhError>,
{
    let method_flag = format!("--{method}");
    let args = ["pr", "merge", pr, &method_flag];
    let output = run_gh(&args)?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    Err(GhError::CommandFailed { command: format!("pr merge {pr} --{method}"), stderr })
}

fn find_open_pr_with<F>(head: &str, base: &str, run_gh: &F) -> Result<Option<String>, GhError>
where
    F: Fn(&[&str]) -> Result<Output, GhError>,
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
        return Err(GhError::CommandFailed { command: "pr list".to_owned(), stderr });
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
) -> Result<String, GhError>
where
    F: Fn(&[&str]) -> Result<Output, GhError>,
{
    let body_path = body_file
        .to_str()
        .ok_or_else(|| GhError::InvalidBodyPath(body_file.display().to_string()))?;
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
        return Err(GhError::CommandFailed { command: "pr create".to_owned(), stderr });
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
        return Err(GhError::CommandFailed { command: format!("pr view {head}"), stderr });
    }
    let number = String::from_utf8_lossy(&view.stdout).trim().to_owned();
    if number.is_empty() || number == "null" { Err(GhError::PrNumberUnknown) } else { Ok(number) }
}

fn post_issue_comment_with<F>(
    repo_nwo: &str,
    pr: &str,
    body: &str,
    run_gh: &F,
) -> Result<String, GhError>
where
    F: Fn(&[&str]) -> Result<Output, GhError>,
{
    let endpoint = format!("repos/{repo_nwo}/issues/{pr}/comments");
    let body_field = format!("body={body}");
    let output = run_gh(&["api", &endpoint, "-f", &body_field])?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    Err(GhError::CommandFailed { command: format!("api {endpoint}"), stderr })
}

fn list_reactions_with<F>(repo_nwo: &str, pr: &str, run_gh: &F) -> Result<String, GhError>
where
    F: Fn(&[&str]) -> Result<Output, GhError>,
{
    let endpoint = format!("repos/{repo_nwo}/issues/{pr}/reactions");
    let output = run_gh(&["api", &endpoint, "--paginate"])?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    Err(GhError::CommandFailed { command: format!("api {endpoint}"), stderr })
}

fn list_reviews_with<F>(repo_nwo: &str, pr: &str, run_gh: &F) -> Result<String, GhError>
where
    F: Fn(&[&str]) -> Result<Output, GhError>,
{
    let endpoint = format!("repos/{repo_nwo}/pulls/{pr}/reviews");
    let output = run_gh(&["api", &endpoint, "--paginate"])?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    Err(GhError::CommandFailed { command: format!("api {endpoint}"), stderr })
}

fn list_issue_comments_with<F>(repo_nwo: &str, pr: &str, run_gh: &F) -> Result<String, GhError>
where
    F: Fn(&[&str]) -> Result<Output, GhError>,
{
    let endpoint = format!("repos/{repo_nwo}/issues/{pr}/comments");
    let output = run_gh(&["api", &endpoint, "--paginate"])?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    Err(GhError::CommandFailed { command: format!("api {endpoint}"), stderr })
}

fn list_review_comments_with<F>(
    repo_nwo: &str,
    pr: &str,
    review_id: &str,
    run_gh: &F,
) -> Result<String, GhError>
where
    F: Fn(&[&str]) -> Result<Output, GhError>,
{
    let endpoint = format!("repos/{repo_nwo}/pulls/{pr}/reviews/{review_id}/comments");
    let output = run_gh(&["api", &endpoint, "--paginate"])?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    Err(GhError::CommandFailed { command: format!("api {endpoint}"), stderr })
}

fn repo_nwo_with<F>(run_gh: &F) -> Result<String, GhError>
where
    F: Fn(&[&str]) -> Result<Output, GhError>,
{
    let output = run_gh(&["repo", "view", "--json", "nameWithOwner", "-q", ".nameWithOwner"])?;
    if output.status.success() {
        let nwo = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        // Fail-closed: reject empty or "null" output to prevent malformed API endpoints.
        if nwo.is_empty() || nwo == "null" {
            return Err(GhError::CommandFailed {
                command: "repo view".to_owned(),
                stderr: "repository nameWithOwner is empty or null".to_owned(),
            });
        }
        return Ok(nwo);
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    Err(GhError::CommandFailed { command: "repo view".to_owned(), stderr })
}

fn pr_head_branch_with<F>(pr: &str, run_gh: &F) -> Result<String, GhError>
where
    F: Fn(&[&str]) -> Result<Output, GhError>,
{
    let output = run_gh(&["pr", "view", pr, "--json", "headRefName", "-q", ".headRefName"])?;
    if output.status.success() {
        let branch = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        if branch.is_empty() || branch == "null" {
            return Err(GhError::CommandFailed {
                command: format!("pr view {pr}"),
                stderr: "headRefName is empty or null".to_owned(),
            });
        }
        return Ok(branch);
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    Err(GhError::CommandFailed { command: format!("pr view {pr}"), stderr })
}

fn decode_pr_checks(stdout: &[u8], pr: &str) -> Result<Vec<PrCheckRecord>, GhError> {
    serde_json::from_slice(stdout)
        .map_err(|source| GhError::JsonDecode { pr: pr.to_owned(), source })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use std::os::unix::process::ExitStatusExt;
    use std::process::{ExitStatus, Output};

    use std::path::Path;

    use rstest::rstest;

    use super::{
        GhError, PrCheckRecord, create_pr_with, decode_pr_checks, find_open_pr_with,
        list_issue_comments_with, list_reactions_with, list_review_comments_with,
        list_reviews_with, merge_pr_with, post_issue_comment_with, pr_checks_with, repo_nwo_with,
    };

    fn output(code: i32, stdout: &str, stderr: &str) -> Output {
        Output {
            status: ExitStatus::from_raw(code << 8),
            stdout: stdout.as_bytes().to_vec(),
            stderr: stderr.as_bytes().to_vec(),
        }
    }

    fn fake_gh(
        code: i32,
        stdout: &'static str,
        stderr: &'static str,
    ) -> impl Fn(&[&str]) -> Result<Output, GhError> {
        move |_args| Ok(output(code, stdout, stderr))
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
        let err = pr_checks_with("123", &fake_gh(1, "", "gh exploded")).unwrap_err().to_string();

        assert!(err.contains("gh exploded"), "got: {err}");
    }

    #[test]
    fn merge_pr_with_surfaces_stderr_on_failure() {
        let err = merge_pr_with("123", "squash", &fake_gh(1, "", "merge exploded"))
            .unwrap_err()
            .to_string();

        assert!(err.contains("merge exploded"), "got: {err}");
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
    fn find_open_pr_with_returns_none_for_absent_pr(#[case] stdout: &'static str) {
        let result = find_open_pr_with("track/feat", "main", &fake_gh(0, stdout, ""));
        assert_eq!(result.unwrap(), None);
    }

    #[test]
    fn find_open_pr_with_surfaces_stderr_on_failure() {
        let err = find_open_pr_with("track/feat", "main", &fake_gh(1, "", "auth error"))
            .unwrap_err()
            .to_string();
        assert!(err.contains("auth error"), "got: {err}");
    }

    // --- create_pr_with tests ---

    #[test]
    fn create_pr_with_extracts_pr_number_from_url() {
        let result = create_pr_with(
            "track/feat",
            "main",
            "title",
            Path::new("body.md"),
            &fake_gh(0, "https://github.com/owner/repo/pull/99\n", ""),
        );
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
        let err = create_pr_with(
            "track/feat",
            "main",
            "title",
            Path::new("body.md"),
            &fake_gh(1, "", "create error"),
        )
        .unwrap_err()
        .to_string();
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
        .unwrap_err()
        .to_string();
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
        .unwrap_err()
        .to_string();
        assert!(err.contains("could not determine PR number"), "got: {err}");
    }

    // --- post_issue_comment_with tests ---

    #[test]
    fn post_issue_comment_with_verifies_argv() {
        let result = post_issue_comment_with("owner/repo", "42", "hello world", &|args| {
            assert_eq!(
                args,
                ["api", "repos/owner/repo/issues/42/comments", "-f", "body=hello world"]
            );
            Ok(output(0, r#"{"id":1,"created_at":"2026-01-01T00:00:00Z"}"#, ""))
        });
        assert!(result.is_ok());
    }

    #[test]
    fn post_issue_comment_with_returns_stdout_on_success() {
        let json = r#"{"id":7,"created_at":"2026-03-01T12:00:00Z"}"#;
        let result =
            post_issue_comment_with("owner/repo", "1", "body", &fake_gh(0, json, "")).unwrap();
        assert_eq!(result, json);
    }

    #[test]
    fn post_issue_comment_with_surfaces_stderr_on_failure() {
        let err = post_issue_comment_with("owner/repo", "1", "body", &fake_gh(1, "", "auth error"))
            .unwrap_err()
            .to_string();
        assert!(err.contains("auth error"), "got: {err}");
    }

    // --- list_reviews_with tests ---

    #[test]
    fn list_reviews_with_verifies_argv() {
        let result = list_reviews_with("owner/repo", "7", &|args| {
            assert_eq!(args, ["api", "repos/owner/repo/pulls/7/reviews", "--paginate"]);
            Ok(output(0, r#"[{"id":1,"state":"APPROVED"}]"#, ""))
        });
        assert!(result.is_ok());
    }

    #[test]
    fn list_reviews_with_returns_stdout_on_success() {
        let json = r#"[{"id":1,"state":"APPROVED"}]"#;
        let result = list_reviews_with("owner/repo", "7", &fake_gh(0, json, "")).unwrap();
        assert_eq!(result, json);
    }

    #[test]
    fn list_reviews_with_surfaces_stderr_on_failure() {
        let err = list_reviews_with("owner/repo", "7", &fake_gh(1, "", "not found"))
            .unwrap_err()
            .to_string();
        assert!(err.contains("not found"), "got: {err}");
    }

    // --- list_issue_comments_with tests ---

    #[test]
    fn list_issue_comments_with_verifies_argv() {
        let result = list_issue_comments_with("owner/repo", "3", &|args| {
            assert_eq!(args, ["api", "repos/owner/repo/issues/3/comments", "--paginate"]);
            Ok(output(0, r#"[{"id":10,"body":"looks good"}]"#, ""))
        });
        assert!(result.is_ok());
    }

    #[test]
    fn list_issue_comments_with_returns_stdout_on_success() {
        let json = r#"[{"id":10,"body":"looks good"}]"#;
        let result = list_issue_comments_with("owner/repo", "3", &fake_gh(0, json, "")).unwrap();
        assert_eq!(result, json);
    }

    #[test]
    fn list_issue_comments_with_surfaces_stderr_on_failure() {
        let err = list_issue_comments_with("owner/repo", "3", &fake_gh(1, "", "forbidden"))
            .unwrap_err()
            .to_string();
        assert!(err.contains("forbidden"), "got: {err}");
    }

    // --- list_reactions_with tests ---

    #[test]
    fn list_reactions_with_verifies_argv() {
        let result = list_reactions_with("owner/repo", "7", &|args| {
            assert_eq!(args, ["api", "repos/owner/repo/issues/7/reactions", "--paginate"]);
            Ok(output(
                0,
                r#"[{"content":"+1","user":{"login":"openai-codex[bot]"},"created_at":"2026-03-18T10:00:00Z"}]"#,
                "",
            ))
        });
        assert!(result.is_ok());
    }

    #[test]
    fn list_reactions_with_returns_stdout_on_success() {
        let json = r#"[{"content":"+1","user":{"login":"openai-codex[bot]"},"created_at":"2026-03-18T10:00:00Z"}]"#;
        let result = list_reactions_with("owner/repo", "7", &fake_gh(0, json, "")).unwrap();
        assert_eq!(result, json);
    }

    #[test]
    fn list_reactions_with_surfaces_stderr_on_failure() {
        let err = list_reactions_with("owner/repo", "7", &fake_gh(1, "", "not found"))
            .unwrap_err()
            .to_string();
        assert!(err.contains("not found"), "got: {err}");
    }

    // --- list_review_comments_with tests ---

    #[test]
    fn list_review_comments_with_verifies_argv() {
        let result = list_review_comments_with("owner/repo", "5", "99", &|args| {
            assert_eq!(args, ["api", "repos/owner/repo/pulls/5/reviews/99/comments", "--paginate"]);
            Ok(output(0, r#"[{"id":20,"body":"nit"}]"#, ""))
        });
        assert!(result.is_ok());
    }

    #[test]
    fn list_review_comments_with_returns_stdout_on_success() {
        let json = r#"[{"id":20,"body":"nit"}]"#;
        let result =
            list_review_comments_with("owner/repo", "5", "99", &fake_gh(0, json, "")).unwrap();
        assert_eq!(result, json);
    }

    #[test]
    fn list_review_comments_with_surfaces_stderr_on_failure() {
        let err =
            list_review_comments_with("owner/repo", "5", "99", &fake_gh(1, "", "server error"))
                .unwrap_err()
                .to_string();
        assert!(err.contains("server error"), "got: {err}");
    }

    // --- repo_nwo_with tests ---

    #[test]
    fn repo_nwo_with_verifies_argv() {
        let result = repo_nwo_with(&|args| {
            assert_eq!(args, ["repo", "view", "--json", "nameWithOwner", "-q", ".nameWithOwner"]);
            Ok(output(0, "owner/repo\n", ""))
        });
        assert_eq!(result.unwrap(), "owner/repo");
    }

    #[test]
    fn repo_nwo_with_returns_trimmed_nwo_on_success() {
        let result = repo_nwo_with(&fake_gh(0, "myorg/myrepo\n", "")).unwrap();
        assert_eq!(result, "myorg/myrepo");
    }

    #[test]
    fn repo_nwo_with_surfaces_stderr_on_failure() {
        let err = repo_nwo_with(&fake_gh(1, "", "not a git repo")).unwrap_err().to_string();
        assert!(err.contains("not a git repo"), "got: {err}");
    }
}
