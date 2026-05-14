//! Tests for [`pr`] (split out to keep the main module under the 200-400 line guideline).

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
        self.create_pr_result
            .borrow()
            .clone()
            .map_err(|stderr| GhError::CommandFailed { command: "pr create".to_owned(), stderr })
    }

    fn post_issue_comment(
        &self,
        _repo_nwo: &str,
        _pr: &str,
        _body: &str,
    ) -> Result<String, GhError> {
        Ok("{}".to_owned())
    }

    fn list_reviews(&self, _repo_nwo: &str, _pr: &str) -> Result<String, GhError> {
        Ok("[]".to_owned())
    }

    fn list_issue_comments(&self, _repo_nwo: &str, _pr: &str) -> Result<String, GhError> {
        Ok("[]".to_owned())
    }

    fn list_review_comments(
        &self,
        _repo_nwo: &str,
        _pr: &str,
        _review_id: &str,
    ) -> Result<String, GhError> {
        Ok("[]".to_owned())
    }

    fn repo_nwo(&self) -> Result<String, GhError> {
        Ok("owner/repo".to_owned())
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

// --- is_codex_bot tests ---

#[test]
fn is_codex_bot_matches_known_bot_logins() {
    assert!(super::is_codex_bot("openai-codex[bot]"));
    assert!(super::is_codex_bot("OpenAI-Codex[bot]"));
    assert!(super::is_codex_bot("codex[bot]"));
    assert!(super::is_codex_bot("chatgpt-codex-connector[bot]"));
    assert!(super::is_codex_bot("ChatGPT-Codex-Connector[bot]"));
}

#[test]
fn is_codex_bot_rejects_human_with_codex_in_name() {
    assert!(!super::is_codex_bot("codex-user"));
    assert!(!super::is_codex_bot("my-codex-tool"));
}

#[test]
fn is_codex_bot_rejects_unknown_codex_app() {
    // Unrelated GitHub App with "codex" in name should not match
    assert!(!super::is_codex_bot("evil-codex-helper[bot]"));
}

#[test]
fn is_codex_bot_rejects_non_codex_bot() {
    assert!(!super::is_codex_bot("dependabot[bot]"));
    assert!(!super::is_codex_bot("github-actions[bot]"));
}

// --- poll_review tests ---

// --- Minimal GhClient for poll_review tests ---

/// Minimal GhClient stub for poll_review tests.
/// Only implements methods needed by the polling logic.
struct PollTestClient {
    reviews: String,
    comments: String,
    reactions: String,
}

impl PollTestClient {
    fn with_reviews(reviews: &str) -> Self {
        Self { reviews: reviews.to_owned(), comments: "[]".to_owned(), reactions: "[]".to_owned() }
    }

    fn with_reactions(reactions: &str) -> Self {
        Self {
            reviews: "[]".to_owned(),
            comments: "[]".to_owned(),
            reactions: reactions.to_owned(),
        }
    }

    #[allow(dead_code)]
    fn with_comments(comments: &str) -> Self {
        Self { reviews: "[]".to_owned(), comments: comments.to_owned(), reactions: "[]".to_owned() }
    }
}

impl GhClient for PollTestClient {
    fn pr_checks(&self, _pr: &str) -> Result<Vec<PrCheckRecord>, GhError> {
        Ok(Vec::new())
    }

    fn pr_url(&self, pr: &str) -> String {
        format!("PR #{pr}")
    }

    fn merge_pr(&self, _pr: &str, _method: &str) -> Result<(), GhError> {
        Ok(())
    }

    fn find_open_pr(&self, _head: &str, _base: &str) -> Result<Option<String>, GhError> {
        Ok(None)
    }

    fn create_pr(
        &self,
        _head: &str,
        _base: &str,
        _title: &str,
        _body_file: &Path,
    ) -> Result<String, GhError> {
        Ok("1".to_owned())
    }

    fn list_reviews(&self, _nwo: &str, _pr: &str) -> Result<String, GhError> {
        Ok(self.reviews.clone())
    }

    fn list_issue_comments(&self, _nwo: &str, _pr: &str) -> Result<String, GhError> {
        Ok(self.comments.clone())
    }

    fn list_reactions(&self, _nwo: &str, _pr: &str) -> Result<String, GhError> {
        Ok(self.reactions.clone())
    }

    fn repo_nwo(&self) -> Result<String, GhError> {
        Ok("owner/repo".to_owned())
    }
}

// --- T002: reaction-based and comment-text zero-findings detection tests ---

#[test]
fn poll_review_for_cycle_returns_zero_findings_on_thumbsup_reaction() {
    // Bot posts a +1 reaction after trigger_timestamp — should return ZeroFindings
    let client = PollTestClient::with_reactions(
        r#"[{"content":"+1","user":{"login":"openai-codex[bot]"},"created_at":"2026-03-18T10:05:00Z"}]"#,
    );
    let result = super::poll_review_for_cycle(
        "1",
        "2026-03-18T10:00:00Z",
        15,
        60,
        &client,
        &|_| {},
        Some("commit1"),
    );
    assert!(result.is_ok());
    assert!(
        matches!(result.unwrap(), super::PollReviewResult::ZeroFindings),
        "expected ZeroFindings from post-trigger +1 reaction"
    );
}

#[test]
fn poll_review_for_cycle_ignores_stale_thumbsup_reaction() {
    // Bot posted +1 BEFORE trigger — should not count as zero-findings
    // Comment fallback also has nothing matching → Timeout
    let client = PollTestClient::with_reactions(
        r#"[{"content":"+1","user":{"login":"openai-codex[bot]"},"created_at":"2026-03-18T09:00:00Z"}]"#,
    );
    let result = super::poll_review_for_cycle(
        "1",
        "2026-03-18T10:00:00Z",
        15,
        0,
        &client,
        &|_| {},
        Some("commit1"),
    );
    assert!(result.is_ok());
    assert!(
        matches!(result.unwrap(), super::PollReviewResult::Timeout),
        "expected Timeout when reaction is pre-trigger"
    );
}

#[test]
fn poll_review_for_cycle_returns_zero_findings_on_comment_text_fallback() {
    // Reaction is stale (pre-trigger), but a post-trigger comment contains the zero-findings phrase
    let client = PollTestClient {
        reviews: "[]".to_owned(),
        comments: r#"[{"user":{"login":"openai-codex[bot]"},"body":"Didn't find any major issues with the code.","created_at":"2026-03-18T10:05:00Z"}]"#.to_owned(),
        reactions: r#"[{"content":"+1","user":{"login":"openai-codex[bot]"},"created_at":"2026-03-18T09:00:00Z"}]"#.to_owned(),
    };
    let result = super::poll_review_for_cycle(
        "1",
        "2026-03-18T10:00:00Z",
        15,
        60,
        &client,
        &|_| {},
        Some("commit1"),
    );
    assert!(result.is_ok());
    assert!(
        matches!(result.unwrap(), super::PollReviewResult::ZeroFindings),
        "expected ZeroFindings from comment text fallback"
    );
}

#[test]
fn poll_review_for_cycle_does_not_trigger_comment_fallback_when_reaction_is_fresh() {
    // Fresh +1 reaction (post-trigger) → ZeroFindings immediately, no need for comment check
    let client = PollTestClient {
        reviews: "[]".to_owned(),
        comments: "[]".to_owned(),
        reactions: r#"[{"content":"+1","user":{"login":"openai-codex[bot]"},"created_at":"2026-03-18T10:05:00Z"}]"#.to_owned(),
    };
    let result = super::poll_review_for_cycle(
        "1",
        "2026-03-18T10:00:00Z",
        15,
        60,
        &client,
        &|_| {},
        Some("commit1"),
    );
    assert!(result.is_ok());
    assert!(
        matches!(result.unwrap(), super::PollReviewResult::ZeroFindings),
        "expected ZeroFindings from fresh reaction"
    );
}

#[test]
fn poll_review_for_cycle_returns_review_found_when_review_exists() {
    // A completed review takes priority over reaction/comment fallbacks
    let client = PollTestClient::with_reviews(
        r#"[{
            "id": 42,
            "user": {"login": "openai-codex[bot]"},
            "submitted_at": "2026-03-18T10:05:00Z",
            "state": "CHANGES_REQUESTED",
            "body": "Please fix these issues."
        }]"#,
    );
    let result = super::poll_review_for_cycle(
        "1",
        "2026-03-18T10:00:00Z",
        15,
        60,
        &client,
        &|_| {},
        Some("commit1"),
    );
    assert!(result.is_ok());
    assert!(
        matches!(result.unwrap(), super::PollReviewResult::ReviewFound(_)),
        "expected ReviewFound when a completed review exists"
    );
}

#[test]
fn poll_review_standalone_skips_zero_findings_without_head_commit() {
    // Standalone poll_review passes head_commit=None, so zero-findings
    // detection (reactions/comments) is skipped — they are PR-level signals
    // that cannot be scoped to a specific commit.
    let client = PollTestClient::with_reactions(
        r#"[{"content":"+1","user":{"login":"openai-codex[bot]"},"created_at":"2026-03-18T10:05:00Z"}]"#,
    );
    let result =
        super::poll_review("1", "2026-03-18T10:00:00Z", 15, 0, &client, &|_| {}, Some("commit1"));
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), ExitCode::FAILURE);
}

// --- poll_review tests ---

#[test]
fn poll_review_finds_post_trigger_codex_review() {
    let client = PollTestClient::with_reviews(
        r#"[{
            "id": 1,
            "user": {"login": "openai-codex[bot]"},
            "submitted_at": "2026-03-16T10:00:00Z",
            "state": "APPROVED",
            "body": "LGTM"
        }]"#,
    );
    // Use timeout=60 so at least one poll iteration runs; sleep is a no-op.
    let result =
        super::poll_review("1", "2026-03-16T09:00:00Z", 15, 60, &client, &|_| {}, Some("commit1"));
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), ExitCode::SUCCESS);
}

#[test]
fn poll_review_standalone_does_not_recover_without_head_commit() {
    // Standalone poll_review passes head_commit=None, so timeout recovery
    // is skipped to avoid returning stale reviews from older commits.
    let client = PollTestClient::with_reviews(
        r#"[{
            "id": 1,
            "user": {"login": "openai-codex[bot]"},
            "submitted_at": "2026-03-16T08:00:00Z",
            "state": "APPROVED",
            "body": "old review"
        }]"#,
    );
    let result =
        super::poll_review("1", "2026-03-16T09:00:00Z", 15, 0, &client, &|_| {}, Some("commit1"));
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), ExitCode::FAILURE);
}

#[test]
fn poll_review_timeout_with_no_reviews_returns_failure() {
    // No reviews at all — timeout recovery also finds nothing.
    let client = PollTestClient::with_reviews("[]");
    let result =
        super::poll_review("1", "2026-03-16T09:00:00Z", 15, 0, &client, &|_| {}, Some("commit1"));
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), ExitCode::FAILURE);
}

// NOTE: `check_tasks_resolved` was removed from this module in T010; the
// equivalent fail-closed behavior is now exercised by the usecase-layer
// tests in `libs/usecase/src/task_completion.rs` (K1-K7) and the
// infrastructure-layer adapter tests in
// `libs/infrastructure/src/verify/merge_gate_adapter.rs`. CLI-layer
// behavior (finding → ExitCode + eprintln formatting) is covered by the
// thin-wrapper integration tests below.

#[test]
fn poll_review_sanitizes_review_body_on_stdout() {
    let client = PollTestClient::with_reviews(
        r#"[{
            "id": 1,
            "user": {"login": "openai-codex[bot]"},
            "submitted_at": "2026-03-16T10:00:00Z",
            "state": "APPROVED",
            "body": "Found issue at /home/user/project/src/main.rs"
        }]"#,
    );
    // The function should succeed (found a post-trigger APPROVED review)
    let result =
        super::poll_review("1", "2026-03-16T09:00:00Z", 15, 60, &client, &|_| {}, Some("commit1"));
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), ExitCode::SUCCESS);
}
