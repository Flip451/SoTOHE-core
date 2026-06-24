//! PR review data types, text sanitization, agent-profiles resolution, and D4 polling service.
//!
//! This module provides pure (I/O-free) helpers used by the PR review workflow:
//! - `sanitize_text`: redact secrets, absolute paths, localhost URLs, and RFC 1918 IPs
//! - `PrReviewFinding` / `PrReviewResult`: passthrough review data types
//! - `parse_paginated_json`: handle concatenated JSON arrays from `gh api --paginate`
//! - `validate_reviewer_provider`: validate the pr-reviewer capability provider
//! - `SleepPort`: secondary port abstracting `thread::sleep`
//! - `PrListReviewsPort` / `PrListReactionsPort` / `PrListIssueCommentsPort`: ports for
//!   GitHub API list operations used by the polling loop
//! - `PrReviewPollingCommand` / `PrReviewPollingOutput` / `PrReviewPollingService` /
//!   `PrReviewPollingInteractor`: D4 extraction of the PR review polling loop

use std::sync::LazyLock;

use regex::Regex;
use thiserror::Error;

// ---------------------------------------------------------------------------
// Static compiled regexes (patterns ported from scripts/pr_review.py)
// ---------------------------------------------------------------------------

// These regex patterns are constant literals — compilation can only fail if the source
// pattern is wrong, which is a programmer bug caught by tests.  Using `.ok()` to
// eliminate panic paths in library code; if a pattern fails to compile, the
// corresponding sanitization step is skipped (fail-safe).
static ABS_PATH_RE: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(r"(/(?:home|Users|tmp|var|etc|opt|srv|workspace|root|usr/local)/[^\s]+)").ok()
});

static ENV_INFO_RE: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(r"(?:https?://)?(?:localhost|127\.0\.0\.1|0\.0\.0\.0)(?::\d+)?(?:/[^\s]*)?").ok()
});

static SECRET_PATTERN_RE: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(
        r"(?:sk-[a-zA-Z0-9]{20,}|ghp_[a-zA-Z0-9]{36,}|github_pat_[a-zA-Z0-9_]{20,}|glpat-[a-zA-Z0-9\-]{20,}|AKIA[A-Z0-9]{16}|xox[bprs]-[a-zA-Z0-9\-]+)",
    )
    .ok()
});

static RFC1918_RE: LazyLock<Option<Regex>> = LazyLock::new(|| {
    // Rust regex does not support look-around; boundary checks are done in sanitize_rfc1918().
    Regex::new(
        r"(?:10\.\d{1,3}\.\d{1,3}\.\d{1,3}|172\.(?:1[6-9]|2\d|3[01])\.\d{1,3}\.\d{1,3}|192\.168\.\d{1,3}\.\d{1,3})(?::\d+)?",
    )
    .ok()
});

/// Allowed reviewer providers that support structured output.
const STRUCTURED_PROVIDERS: &[&str] = &["codex"];

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors returned by [`validate_reviewer_provider`].
#[derive(Debug, Error)]
pub enum PrReviewError {
    /// The resolved reviewer provider does not support structured output.
    #[error(
        "reviewer provider '{provider}' does not support structured output; requires one of: {allowed}"
    )]
    UnsupportedProvider { provider: String, allowed: String },
}

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// A passthrough inline review comment from the latest Codex review round.
///
/// Carries file path, line range, and sanitized body text.
/// `severity` and `rule_id` fields are removed per D1 (no Rust-side interpretation).
/// String fields are raw GitHub API values: `path` is an opaque filesystem path,
/// `body` is free text — both are truly opaque values with no underlying invariant
/// to enforce.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrReviewFinding {
    /// File path where the inline comment was posted.
    pub path: String,
    /// First (start) line of the comment, if known.
    pub line: Option<u32>,
    /// Last (end) line of the comment, if known.
    pub end_line: Option<u32>,
    /// Sanitized body text of the inline comment.
    pub body: String,
}

/// Parsed passthrough result from a Codex Cloud PR review.
///
/// Carries the sanitized review body and inline comments for agent consumption.
/// `actionable_count` and `passed` fields are removed per D1/AC-02 — pass/fail
/// judgment is delegated to the agent.
/// `state`: `String` is a raw GitHub API field (`'APPROVED'`/`'CHANGES_REQUESTED'`/
/// `'COMMENTED'`); it is surfaced as-is without interpretation (truly opaque external
/// API value).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrReviewResult {
    /// GitHub review ID.
    pub review_id: u64,
    /// Review state: `"APPROVED"`, `"CHANGES_REQUESTED"`, or `"COMMENTED"`.
    pub state: String,
    /// Sanitized review body.
    pub body: String,
    /// Passthrough inline comments from the latest review round.
    pub findings: Vec<PrReviewFinding>,
    /// Number of inline comments fetched from the GitHub API.
    pub inline_comment_count: u32,
}

// ---------------------------------------------------------------------------
// Public functions
// ---------------------------------------------------------------------------

/// Remove absolute paths, secrets, localhost references, and RFC 1918 IPs from `text`.
///
/// Application order (matches Python):
/// 1. Secrets (`[REDACTED]`)
/// 2. Absolute paths (`[PATH]`)
/// 3. Localhost / env URLs (`[INTERNAL]`)
/// 4. RFC 1918 IPs (`[INTERNAL_IP]`)
///
/// # Examples
///
/// ```
/// use usecase::pr_review::sanitize_text;
/// let out = sanitize_text("Error in /home/user/main.rs");
/// assert!(out.contains("[PATH]"));
/// assert!(!out.contains("/home/user"));
/// ```
#[must_use]
pub fn sanitize_text(text: &str) -> String {
    let text = match SECRET_PATTERN_RE.as_ref() {
        Some(re) => re.replace_all(text, "[REDACTED]").into_owned(),
        None => text.to_owned(),
    };
    let text = match ABS_PATH_RE.as_ref() {
        Some(re) => re.replace_all(&text, "[PATH]").into_owned(),
        None => text,
    };
    let text = match ENV_INFO_RE.as_ref() {
        Some(re) => re.replace_all(&text, "[INTERNAL]").into_owned(),
        None => text,
    };
    sanitize_rfc1918(&text)
}

/// Replace RFC 1918 addresses with `[INTERNAL_IP]`, emulating Python look-around.
///
/// The Rust `regex` crate does not support look-behind/look-ahead, so we manually
/// check whether the character immediately before/after the match is a digit.
fn sanitize_rfc1918(text: &str) -> String {
    let Some(re) = RFC1918_RE.as_ref() else {
        return text.to_owned();
    };
    let bytes = text.as_bytes();
    let mut result = String::with_capacity(text.len());
    let mut last_end = 0;

    for m in re.find_iter(text) {
        let start = m.start();
        let end = m.end();
        // Emulate (?<!\d): reject if preceded by a digit
        if start > 0 && bytes.get(start - 1).is_some_and(|b| b.is_ascii_digit()) {
            continue;
        }
        // Emulate (?!\d): reject if followed by a digit
        if bytes.get(end).is_some_and(|b| b.is_ascii_digit()) {
            continue;
        }
        result.push_str(&text[last_end..start]);
        result.push_str("[INTERNAL_IP]");
        last_end = end;
    }
    result.push_str(&text[last_end..]);
    result
}

/// Parse potentially paginated `gh api --paginate` JSON output into a flat list.
///
/// Handles:
/// - Empty string → empty vec
/// - Single JSON array → items
/// - Concatenated JSON arrays (edge case) → merged items
///
/// Non-array top-level values (objects, scalars) are rejected as errors.
/// GitHub list endpoints always return arrays; a single object (e.g.,
/// `{"message":"Not Found"}`) indicates an API error, not valid data.
///
/// # Errors
///
/// Returns `serde_json::Error` if the input is malformed, truncated, or not an
/// array, ensuring callers can distinguish "no data" from "parse failure"
/// (fail-closed).
pub fn parse_paginated_json(text: &str) -> Result<Vec<serde_json::Value>, serde_json::Error> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(Vec::new());
    }
    match serde_json::from_str::<serde_json::Value>(text) {
        Ok(serde_json::Value::Array(arr)) => Ok(arr),
        Ok(other) => {
            // Non-array top-level value (object or scalar) — fail-closed.
            // Attempt to deserialize the already-parsed value as Vec to get a
            // typed serde error (e.g., "expected a sequence").
            serde_json::from_value::<Vec<serde_json::Value>>(other)?;
            // from_value will always fail for non-array values, but if it
            // somehow succeeds (unreachable), return the result.
            Ok(Vec::new())
        }
        Err(_) => {
            // Concatenated JSON values (e.g., `[...]\n[...]`): use StreamDeserializer.
            // Fail-closed: if any item fails to decode, propagate the error.
            let mut results = Vec::new();
            let stream = serde_json::Deserializer::from_str(text).into_iter::<serde_json::Value>();
            for item in stream {
                match item {
                    Ok(serde_json::Value::Array(arr)) => results.extend(arr),
                    Ok(other) => {
                        // Non-array item in stream — fail-closed.
                        // Use from_value to produce a typed "expected a sequence" error.
                        serde_json::from_value::<Vec<serde_json::Value>>(other)?;
                    }
                    Err(e) => return Err(e),
                }
            }
            Ok(results)
        }
    }
}

/// Validates that the given reviewer provider supports structured output.
///
/// The caller is responsible for resolving the provider from the agent-profiles
/// configuration (via `AgentProfiles::resolve_execution` in the infrastructure layer).
/// This function only validates the provider against `STRUCTURED_PROVIDERS`.
///
/// # Errors
///
/// Returns [`PrReviewError::UnsupportedProvider`] if the provider is not in
/// `STRUCTURED_PROVIDERS`.
pub fn validate_reviewer_provider(provider: &str) -> Result<(), PrReviewError> {
    if !STRUCTURED_PROVIDERS.contains(&provider) {
        return Err(PrReviewError::UnsupportedProvider {
            provider: provider.to_owned(),
            allowed: STRUCTURED_PROVIDERS.join(", "),
        });
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::{parse_paginated_json, sanitize_text, validate_reviewer_provider};

    // -----------------------------------------------------------------------
    // sanitize_text — 16 tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_sanitize_removes_absolute_paths() {
        let result = sanitize_text("Error in /home/user/project/src/main.rs");
        assert!(!result.contains("/home/user"), "path not redacted: {result}");
        assert!(result.contains("[PATH]"), "expected [PATH]: {result}");
    }

    #[test]
    fn test_sanitize_removes_secrets_github_token() {
        let result = sanitize_text("Token: ghp_abcdefghijklmnopqrstuvwxyz0123456789");
        assert!(!result.contains("ghp_"), "token not redacted: {result}");
        assert!(result.contains("[REDACTED]"), "expected [REDACTED]: {result}");
    }

    #[test]
    fn test_sanitize_removes_secrets_sk_key() {
        let result = sanitize_text("API key: sk-abcdefghijklmnopqrstuvwx");
        assert!(!result.contains("sk-"), "key not redacted: {result}");
        assert!(result.contains("[REDACTED]"), "expected [REDACTED]: {result}");
    }

    #[test]
    fn test_sanitize_removes_localhost_urls() {
        let result = sanitize_text("Server at http://localhost:3000/api");
        assert!(!result.contains("localhost"), "localhost not redacted: {result}");
        assert!(result.contains("[INTERNAL]"), "expected [INTERNAL]: {result}");
    }

    #[test]
    fn test_sanitize_removes_internal_ip() {
        let result = sanitize_text("Listening on 127.0.0.1:8080");
        assert!(!result.contains("127.0.0.1"), "127.0.0.1 not redacted: {result}");
    }

    #[test]
    fn test_sanitize_preserves_normal_text() {
        let text = "This function has a logic error in the loop condition";
        assert_eq!(sanitize_text(text), text);
    }

    #[test]
    fn test_sanitize_removes_aws_key() {
        let result = sanitize_text("Key: AKIAIOSFODNN7EXAMPLE");
        assert!(!result.contains("AKIA"), "AWS key not redacted: {result}");
        assert!(result.contains("[REDACTED]"), "expected [REDACTED]: {result}");
    }

    #[test]
    fn test_sanitize_removes_github_pat_token() {
        let result = sanitize_text("Token: github_pat_abcdefghijklmnopqrstuvwx");
        assert!(!result.contains("github_pat_"), "PAT not redacted: {result}");
        assert!(result.contains("[REDACTED]"), "expected [REDACTED]: {result}");
    }

    #[test]
    fn test_sanitize_removes_gitlab_token() {
        let result = sanitize_text("Token: glpat-abcdefghijklmnopqrstuvwx");
        assert!(!result.contains("glpat-"), "gitlab token not redacted: {result}");
        assert!(result.contains("[REDACTED]"), "expected [REDACTED]: {result}");
    }

    #[test]
    fn test_sanitize_removes_rfc1918_addresses() {
        let result = sanitize_text("Server at 10.0.1.5:8080 and 192.168.1.100 and 172.16.0.1");
        assert!(!result.contains("10.0.1.5"), "10.0.1.5 not redacted: {result}");
        assert!(!result.contains("192.168.1.100"), "192.168.1.100 not redacted: {result}");
        assert!(!result.contains("172.16.0.1"), "172.16.0.1 not redacted: {result}");
        assert!(result.contains("[INTERNAL_IP]"), "expected [INTERNAL_IP]: {result}");
    }

    #[test]
    fn test_sanitize_removes_rfc1918_in_url() {
        let result = sanitize_text("URL http://10.0.1.5:8080/api");
        assert!(!result.contains("10.0.1.5"), "IP in URL not redacted: {result}");
    }

    #[test]
    fn test_sanitize_removes_rfc1918_in_parens() {
        let result = sanitize_text("(10.0.1.5:8080)");
        assert!(!result.contains("10.0.1.5"), "IP in parens not redacted: {result}");
    }

    #[test]
    fn test_sanitize_removes_workspace_path() {
        let result = sanitize_text("Error in /workspace/src/main.rs");
        assert!(!result.contains("/workspace/"), "workspace path not redacted: {result}");
        assert!(result.contains("[PATH]"), "expected [PATH]: {result}");
    }

    #[test]
    fn test_sanitize_removes_etc_path() {
        let result = sanitize_text("Config at /etc/ssl/certs/ca.pem");
        assert!(!result.contains("/etc/"), "/etc/ path not redacted: {result}");
        assert!(result.contains("[PATH]"), "expected [PATH]: {result}");
    }

    #[test]
    fn test_sanitize_no_false_positive_rfc1918_substring() {
        // 110.0.1.5 starts with 1, not 10. Must NOT be redacted.
        let result = sanitize_text("IP 110.0.1.5 is public");
        assert!(result.contains("110.0.1.5"), "public IP wrongly redacted: {result}");
    }

    // -----------------------------------------------------------------------
    // parse_paginated_json — 5 tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_paginated_json_single_array() {
        let result = parse_paginated_json(r#"[{"id": 1}, {"id": 2}]"#).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0]["id"], 1);
    }

    #[test]
    fn test_parse_paginated_json_empty_string() {
        assert!(parse_paginated_json("").unwrap().is_empty());
    }

    #[test]
    fn test_parse_paginated_json_single_object_returns_error() {
        // Non-array top-level value is rejected (fail-closed: could be API error).
        let result = parse_paginated_json(r#"{"id": 1}"#);
        assert!(result.is_err(), "single object should be rejected as non-array");
    }

    #[test]
    fn test_parse_paginated_json_api_error_returns_error() {
        // GitHub API error payload must not be silently accepted.
        let result = parse_paginated_json(r#"{"message": "Not Found"}"#);
        assert!(result.is_err(), "API error object should be rejected");
    }

    #[test]
    fn test_parse_paginated_json_concatenated_arrays() {
        let result = parse_paginated_json("[{\"id\": 1}]\n[{\"id\": 2}]").unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0]["id"], 1);
        assert_eq!(result[1]["id"], 2);
    }

    #[test]
    fn test_parse_paginated_json_truncated_returns_error() {
        // Fail-closed: corrupted mid-stream data should return Err, not partial results.
        let result = parse_paginated_json("[{\"id\": 1}]\n{truncated");
        assert!(result.is_err(), "expected Err for corrupted stream");
    }

    // -----------------------------------------------------------------------
    // validate_reviewer_provider — tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_validate_reviewer_provider_codex_succeeds() {
        assert!(validate_reviewer_provider("codex").is_ok());
    }

    #[test]
    fn test_validate_reviewer_provider_claude_fails_closed() {
        let result = validate_reviewer_provider("claude");
        assert!(result.is_err(), "expected Err for unsupported provider");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("claude") && msg.contains("structured"),
            "unexpected error message: {msg}"
        );
    }

    #[test]
    fn test_validate_reviewer_provider_gemini_fails() {
        let result = validate_reviewer_provider("gemini");
        assert!(result.is_err(), "expected Err for unsupported provider");
    }

    // -----------------------------------------------------------------------
    // PrReviewPollingInteractor — T008 tests
    // -----------------------------------------------------------------------

    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use crate::pr_review_polling::{
        PrGhApiError, PrListIssueCommentsPort, PrListReactionsPort, PrListReviewsPort,
        PrReviewPollingCommand, PrReviewPollingInteractor, PrReviewPollingOutput,
        PrReviewPollingService, SleepPort,
    };

    // ── Test doubles ─────────────────────────────────────────────────────────

    struct StubReviews(String);
    impl PrListReviewsPort for StubReviews {
        fn list_reviews(&self, _repo: &str, _pr: &str) -> Result<String, PrGhApiError> {
            Ok(self.0.clone())
        }
    }

    struct SequencedReviews(Mutex<Vec<String>>);
    impl PrListReviewsPort for SequencedReviews {
        fn list_reviews(&self, _repo: &str, _pr: &str) -> Result<String, PrGhApiError> {
            let mut responses = self.0.lock().unwrap();
            if responses.is_empty() {
                return Ok("[]".to_owned());
            }
            Ok(responses.remove(0))
        }
    }

    struct FailingReviews;
    impl PrListReviewsPort for FailingReviews {
        fn list_reviews(&self, _repo: &str, _pr: &str) -> Result<String, PrGhApiError> {
            Err(PrGhApiError::ApiFailure("reviews unavailable".to_owned()))
        }
    }

    struct StubReactions(String);
    impl PrListReactionsPort for StubReactions {
        fn list_reactions(&self, _repo: &str, _pr: &str) -> Result<String, PrGhApiError> {
            Ok(self.0.clone())
        }
    }

    struct FailingReactions;
    impl PrListReactionsPort for FailingReactions {
        fn list_reactions(&self, _repo: &str, _pr: &str) -> Result<String, PrGhApiError> {
            Err(PrGhApiError::ApiFailure("reactions unavailable".to_owned()))
        }
    }

    struct StubComments(String);
    impl PrListIssueCommentsPort for StubComments {
        fn list_issue_comments(&self, _repo: &str, _pr: &str) -> Result<String, PrGhApiError> {
            Ok(self.0.clone())
        }
    }

    struct FailingComments;
    impl PrListIssueCommentsPort for FailingComments {
        fn list_issue_comments(&self, _repo: &str, _pr: &str) -> Result<String, PrGhApiError> {
            Err(PrGhApiError::ApiFailure("comments unavailable".to_owned()))
        }
    }

    struct RecordingSleep(Mutex<Vec<Duration>>);

    impl RecordingSleep {
        fn new() -> Arc<Self> {
            Arc::new(Self(Mutex::new(Vec::new())))
        }
        fn recorded(&self) -> Vec<Duration> {
            self.0.lock().unwrap().clone()
        }
    }

    impl SleepPort for RecordingSleep {
        fn sleep(&self, duration: Duration) {
            self.0.lock().unwrap().push(duration);
        }
    }

    fn make_interactor(
        reviews: &str,
        reactions: &str,
        comments: &str,
        sleep: Arc<dyn SleepPort>,
    ) -> PrReviewPollingInteractor {
        PrReviewPollingInteractor::new(
            Arc::new(StubReviews(reviews.to_owned())),
            Arc::new(StubReactions(reactions.to_owned())),
            Arc::new(StubComments(comments.to_owned())),
            sleep,
        )
    }

    fn make_cmd(max_iterations: u64) -> PrReviewPollingCommand {
        PrReviewPollingCommand {
            pr: "42".to_owned(),
            repo_nwo: "owner/repo".to_owned(),
            trigger_timestamp: "2026-01-01T00:00:00Z".to_owned(),
            interval_secs: 1,
            max_iterations,
            head_commit: None,
        }
    }

    fn make_cmd_with_head(max_iterations: u64) -> PrReviewPollingCommand {
        PrReviewPollingCommand {
            head_commit: Some("abc123".to_owned()),
            ..make_cmd(max_iterations)
        }
    }

    fn assert_pr_polling_error(
        result: Result<PrReviewPollingOutput, crate::d4_orchestration::D4OrchestrationError>,
    ) {
        assert!(
            matches!(result, Err(crate::d4_orchestration::D4OrchestrationError::PrPolling(_))),
            "expected PrPolling error, got {result:?}"
        );
    }

    // A qualifying Codex bot review JSON (submitted after trigger).
    fn codex_review_json(state: &str) -> serde_json::Value {
        serde_json::json!({
            "id": 1,
            "user": { "login": "openai-codex[bot]" },
            "state": state,
            "submitted_at": "2026-01-01T01:00:00Z",
            "body": "review body",
            "commit_id": "abc123"
        })
    }

    fn codex_reaction_json(created_at: &str) -> serde_json::Value {
        serde_json::json!({
            "content": "+1",
            "user": { "login": "openai-codex[bot]" },
            "created_at": created_at
        })
    }

    fn codex_zero_findings_comment_json(created_at: &str) -> serde_json::Value {
        serde_json::json!({
            "user": { "login": "openai-codex[bot]" },
            "created_at": created_at,
            "body": "Didn't find any major issues."
        })
    }

    /// When a qualifying review is found immediately, poll returns ReviewFound.
    #[test]
    fn polling_exits_immediately_when_review_found() {
        let review_json =
            serde_json::to_string(&serde_json::json!([codex_review_json("APPROVED")])).unwrap();
        let sleep = RecordingSleep::new();
        let interactor = make_interactor(&review_json, "[]", "[]", Arc::clone(&sleep) as _);
        let result = interactor.poll(make_cmd(10)).unwrap();
        assert!(
            matches!(result, PrReviewPollingOutput::ReviewFound(_)),
            "expected ReviewFound, got {result:?}"
        );
        // Should not sleep at all since review found on first iteration.
        assert!(sleep.recorded().is_empty(), "should not sleep when review found on first poll");
    }

    /// A zero interval with one allowed iteration still performs the initial poll.
    #[test]
    fn polling_with_zero_interval_still_polls_once() {
        let review_json =
            serde_json::to_string(&serde_json::json!([codex_review_json("APPROVED")])).unwrap();
        let sleep = RecordingSleep::new();
        let interactor = make_interactor(&review_json, "[]", "[]", Arc::clone(&sleep) as _);
        let cmd = PrReviewPollingCommand { interval_secs: 0, max_iterations: 1, ..make_cmd(1) };

        let result = interactor.poll(cmd).unwrap();

        assert!(
            matches!(result, PrReviewPollingOutput::ReviewFound(_)),
            "expected ReviewFound from the initial zero-interval poll, got {result:?}"
        );
        assert!(sleep.recorded().is_empty(), "review found on initial poll should not sleep");
    }

    /// When no review is found within max_iterations, poll returns Timeout.
    #[test]
    fn polling_returns_timeout_when_no_review_found() {
        let sleep = RecordingSleep::new();
        let interactor = make_interactor("[]", "[]", "[]", Arc::clone(&sleep) as _);
        let result = interactor.poll(make_cmd(3)).unwrap();
        assert!(
            matches!(result, PrReviewPollingOutput::Timeout),
            "expected Timeout when no review found, got {result:?}"
        );
        // Should have slept max_iterations times.
        assert_eq!(
            sleep.recorded().len(),
            3,
            "should sleep exactly max_iterations=3 times, got {}",
            sleep.recorded().len()
        );
    }

    /// The deterministic interval budget can stop polling before max_iterations.
    #[test]
    fn polling_returns_timeout_when_interval_budget_reaches_deadline() {
        let sleep = RecordingSleep::new();
        let interactor = make_interactor("[]", "[]", "[]", Arc::clone(&sleep) as _);
        let cmd =
            PrReviewPollingCommand { interval_secs: 86_401, max_iterations: 2, ..make_cmd(2) };

        let result = interactor.poll(cmd).unwrap();

        assert!(
            matches!(result, PrReviewPollingOutput::Timeout),
            "expected Timeout when interval budget reaches deadline, got {result:?}"
        );
        assert_eq!(
            sleep.recorded(),
            vec![Duration::from_secs(86_401)],
            "deadline budget should stop before the second max-iteration sleep"
        );
    }

    /// When max_iterations is 0, poll returns Timeout immediately without sleeping.
    #[test]
    fn polling_returns_timeout_immediately_when_zero_max_iterations() {
        let sleep = RecordingSleep::new();
        let interactor = make_interactor("[]", "[]", "[]", Arc::clone(&sleep) as _);
        let result = interactor.poll(make_cmd(0)).unwrap();
        assert!(
            matches!(result, PrReviewPollingOutput::Timeout),
            "expected Timeout for max_iterations=0"
        );
        assert!(sleep.recorded().is_empty(), "no sleep for max_iterations=0");
    }

    /// Invalid trigger timestamps are mapped to PrPolling errors.
    #[test]
    fn polling_invalid_trigger_timestamp_returns_pr_polling_error() {
        let sleep = RecordingSleep::new();
        let interactor = make_interactor("[]", "[]", "[]", Arc::clone(&sleep) as _);
        let mut cmd = make_cmd(1);
        cmd.trigger_timestamp = "not-a-timestamp".to_owned();

        assert_pr_polling_error(interactor.poll(cmd));
        assert!(sleep.recorded().is_empty(), "timestamp errors should occur before polling");
    }

    /// Review-list port failures are mapped to PrPolling errors.
    #[test]
    fn polling_review_list_failure_returns_pr_polling_error() {
        let sleep = RecordingSleep::new();
        let interactor = PrReviewPollingInteractor::new(
            Arc::new(FailingReviews),
            Arc::new(StubReactions("[]".to_owned())),
            Arc::new(StubComments("[]".to_owned())),
            Arc::clone(&sleep) as Arc<dyn SleepPort>,
        );

        assert_pr_polling_error(interactor.poll(make_cmd(1)));
        assert!(sleep.recorded().is_empty(), "review-list errors should stop before sleeping");
    }

    /// Malformed review JSON is mapped to a PrPolling error.
    #[test]
    fn polling_review_json_parse_failure_returns_pr_polling_error() {
        let sleep = RecordingSleep::new();
        let interactor = make_interactor("{", "[]", "[]", Arc::clone(&sleep) as _);

        assert_pr_polling_error(interactor.poll(make_cmd(1)));
        assert!(sleep.recorded().is_empty(), "review JSON errors should stop before sleeping");
    }

    /// Reaction-list port failures are mapped to PrPolling errors.
    #[test]
    fn polling_reaction_list_failure_returns_pr_polling_error() {
        let sleep = RecordingSleep::new();
        let interactor = PrReviewPollingInteractor::new(
            Arc::new(StubReviews("[]".to_owned())),
            Arc::new(FailingReactions),
            Arc::new(StubComments("[]".to_owned())),
            Arc::clone(&sleep) as Arc<dyn SleepPort>,
        );

        assert_pr_polling_error(interactor.poll(make_cmd_with_head(1)));
        assert!(sleep.recorded().is_empty(), "reaction-list errors should stop before sleeping");
    }

    /// Malformed reaction JSON is mapped to a PrPolling error.
    #[test]
    fn polling_reaction_json_parse_failure_returns_pr_polling_error() {
        let sleep = RecordingSleep::new();
        let interactor = make_interactor("[]", "{", "[]", Arc::clone(&sleep) as _);

        assert_pr_polling_error(interactor.poll(make_cmd_with_head(1)));
        assert!(sleep.recorded().is_empty(), "reaction JSON errors should stop before sleeping");
    }

    /// Comment-list port failures are mapped to PrPolling errors.
    #[test]
    fn polling_comment_list_failure_returns_pr_polling_error() {
        let sleep = RecordingSleep::new();
        let interactor = PrReviewPollingInteractor::new(
            Arc::new(StubReviews("[]".to_owned())),
            Arc::new(StubReactions("[]".to_owned())),
            Arc::new(FailingComments),
            Arc::clone(&sleep) as Arc<dyn SleepPort>,
        );

        assert_pr_polling_error(interactor.poll(make_cmd(1)));
        assert!(sleep.recorded().is_empty(), "comment-list errors should stop before sleeping");
    }

    /// Malformed comment JSON is mapped to a PrPolling error.
    #[test]
    fn polling_comment_json_parse_failure_returns_pr_polling_error() {
        let sleep = RecordingSleep::new();
        let interactor = make_interactor("[]", "[]", "{", Arc::clone(&sleep) as _);

        assert_pr_polling_error(interactor.poll(make_cmd(1)));
        assert!(sleep.recorded().is_empty(), "comment JSON errors should stop before sleeping");
    }

    /// A post-trigger Codex +1 reaction is the primary zero-findings signal.
    #[test]
    fn polling_returns_zero_findings_when_codex_reaction_after_trigger() {
        let reactions = serde_json::to_string(&serde_json::json!([codex_reaction_json(
            "2026-01-01T00:00:01Z"
        )]))
        .unwrap();
        let sleep = RecordingSleep::new();
        let interactor = make_interactor("[]", &reactions, "[]", Arc::clone(&sleep) as _);

        let result = interactor.poll(make_cmd_with_head(10)).unwrap();

        assert!(
            matches!(result, PrReviewPollingOutput::ZeroFindings),
            "expected ZeroFindings from post-trigger reaction, got {result:?}"
        );
        assert!(
            sleep.recorded().is_empty(),
            "zero-findings reaction should stop polling before sleeping"
        );
    }

    /// A stale Codex +1 reaction plus a post-trigger zero-findings comment is the fallback signal.
    #[test]
    fn polling_returns_zero_findings_when_stale_reaction_has_comment_fallback() {
        let reactions = serde_json::to_string(&serde_json::json!([codex_reaction_json(
            "2025-12-31T23:59:00Z"
        )]))
        .unwrap();
        let comments =
            serde_json::to_string(&serde_json::json!([codex_zero_findings_comment_json(
                "2026-01-01T00:00:01Z"
            )]))
            .unwrap();
        let sleep = RecordingSleep::new();
        let interactor = make_interactor("[]", &reactions, &comments, Arc::clone(&sleep) as _);

        let result = interactor.poll(make_cmd_with_head(10)).unwrap();

        assert!(
            matches!(result, PrReviewPollingOutput::ZeroFindings),
            "expected ZeroFindings from comment fallback, got {result:?}"
        );
        assert!(
            sleep.recorded().is_empty(),
            "zero-findings comment fallback should stop polling before sleeping"
        );
    }

    /// Timeout recovery accepts a qualifying Codex review on the exact expected HEAD commit.
    #[test]
    fn polling_recovers_exact_head_review_after_timeout() {
        let recovery_review =
            serde_json::to_string(&serde_json::json!([codex_review_json("COMMENTED")])).unwrap();
        let sleep = RecordingSleep::new();
        let interactor = PrReviewPollingInteractor::new(
            Arc::new(SequencedReviews(Mutex::new(vec!["[]".to_owned(), recovery_review]))),
            Arc::new(StubReactions("[]".to_owned())),
            Arc::new(StubComments("[]".to_owned())),
            Arc::clone(&sleep) as Arc<dyn SleepPort>,
        );

        let result = interactor.poll(make_cmd_with_head(1)).unwrap();

        assert!(
            matches!(result, PrReviewPollingOutput::ReviewFound(_)),
            "expected timeout recovery to return ReviewFound, got {result:?}"
        );
        assert_eq!(
            sleep.recorded(),
            vec![Duration::from_secs(1)],
            "one poll iteration should sleep once before recovery"
        );
    }

    /// Sleep is recorded with the correct interval duration.
    #[test]
    fn polling_records_correct_sleep_duration() {
        let sleep = RecordingSleep::new();
        let interactor = make_interactor("[]", "[]", "[]", Arc::clone(&sleep) as _);
        let cmd = PrReviewPollingCommand {
            pr: "42".to_owned(),
            repo_nwo: "owner/repo".to_owned(),
            trigger_timestamp: "2026-01-01T00:00:00Z".to_owned(),
            interval_secs: 5,
            max_iterations: 2,
            head_commit: None,
        };
        let _ = interactor.poll(cmd);
        let sleeps = sleep.recorded();
        assert_eq!(sleeps.len(), 2);
        assert!(sleeps.iter().all(|d| *d == Duration::from_secs(5)));
    }
}
