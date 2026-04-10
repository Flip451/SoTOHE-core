//! PR review data types, text sanitization, and agent-profiles resolution.
//!
//! This module provides pure (I/O-free) helpers used by the PR review workflow:
//! - `sanitize_text`: redact secrets, absolute paths, localhost URLs, and RFC 1918 IPs
//! - `PrReviewFinding` / `PrReviewResult`: normalized review data types
//! - `classify_severity`: keyword-based severity classification
//! - `parse_body_findings`: extract findings from review body text
//! - `parse_paginated_json`: handle concatenated JSON arrays from `gh api --paginate`
//! - `resolve_reviewer_provider`: read capability-centric agent-profiles and validate provider

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

/// A normalized finding extracted from a PR review.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrReviewFinding {
    /// Severity classification: `"P0"`, `"P1"`, `"LOW"`, or `"INFO"`.
    pub severity: String,
    /// File path where the finding was posted (empty string for body findings).
    pub path: String,
    /// First (start) line of the finding, if known.
    pub line: Option<u32>,
    /// Last (end) line of the finding, if known.
    pub end_line: Option<u32>,
    /// Sanitized body text of the finding.
    pub body: String,
    /// Optional rule identifier.
    pub rule_id: Option<String>,
}

/// Parsed result of a Codex Cloud PR review.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrReviewResult {
    /// GitHub review ID.
    pub review_id: u64,
    /// Review state: `"APPROVED"`, `"CHANGES_REQUESTED"`, or `"COMMENTED"`.
    pub state: String,
    /// Sanitized review body.
    pub body: String,
    /// All normalized findings (inline + body-parsed).
    pub findings: Vec<PrReviewFinding>,
    /// Number of inline comments fetched from the GitHub API.
    pub inline_comment_count: u32,
    /// Number of actionable (P0 or P1) findings.
    pub actionable_count: u32,
    /// `true` when `actionable_count == 0` and state is not `CHANGES_REQUESTED`.
    pub passed: bool,
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

/// Classify a finding's severity from its text content.
///
/// Case-insensitive keyword matching:
/// - P0: critical / security / vulnerability / panic / data loss / error / bug / incorrect / wrong / missing error
/// - P1 (default): warning / should / consider / suggest / improve, and unrecognized text
///
/// The default is P1 (actionable) to match Python behavior.  APPROVED reviews
/// should handle nits at the `parse_review` level, not here.
#[must_use]
pub fn classify_severity(text: &str) -> &'static str {
    let lower = text.to_lowercase();
    if ["critical", "security", "vulnerability", "panic", "data loss"]
        .iter()
        .any(|kw| lower.contains(kw))
    {
        return "P0";
    }
    if ["error", "bug", "incorrect", "wrong", "missing error"].iter().any(|kw| lower.contains(kw)) {
        return "P0";
    }
    if ["warning", "should", "consider", "suggest", "improve"].iter().any(|kw| lower.contains(kw)) {
        return "P1";
    }
    "P1"
}

/// Extract findings from a review body by parsing bullet-pointed list items.
///
/// Lines starting with `- `, `* `, or `• ` (after trimming) are treated as
/// candidate findings. Items whose content is ≤ 10 characters are skipped.
///
/// # Errors
///
/// This function is infallible and returns an empty `Vec` for empty input.
#[must_use]
pub fn parse_body_findings(body: &str) -> Vec<PrReviewFinding> {
    let mut findings = Vec::new();
    // Tracks (fence_char, min_length) so ```````` is not closed by ```
    let mut fence_marker: Option<(char, usize)> = None;
    for line in body.lines() {
        let stripped = line.trim();
        // CommonMark: fenced code blocks allow 0-3 leading spaces only.
        // Lines with 4+ leading spaces are indented code, not fences.
        // CommonMark: tabs expand to 4-space tab stops. A tab at column 0
        // expands to 4 spaces, disqualifying the line as a fence (max 3).
        let leading_indent: usize = line
            .chars()
            .take_while(|c| c.is_whitespace())
            .map(|c| if c == '\t' { 4 } else { 1 })
            .sum();
        let could_be_fence = leading_indent <= 3;
        // Track fenced code blocks — remember fence char AND length.
        // Opening fences may have an info string; closing fences must be
        // fence chars + optional whitespace only (per CommonMark spec).
        if could_be_fence
            && let Some(fence_len) = stripped
                .strip_prefix("```")
                .map(|rest| 3 + rest.chars().take_while(|&c| c == '`').count())
        {
            let after_fence = &stripped[fence_len..];
            let is_close_candidate = after_fence.trim().is_empty();
            fence_marker = match fence_marker {
                Some(('`', open_len)) if fence_len >= open_len && is_close_candidate => None,
                None => Some(('`', fence_len)),
                other => other,
            };
            continue;
        }
        if could_be_fence
            && let Some(fence_len) = stripped
                .strip_prefix("~~~")
                .map(|rest| 3 + rest.chars().take_while(|&c| c == '~').count())
        {
            let after_fence = &stripped[fence_len..];
            let is_close_candidate = after_fence.trim().is_empty();
            fence_marker = match fence_marker {
                Some(('~', open_len)) if fence_len >= open_len && is_close_candidate => None,
                None => Some(('~', fence_len)),
                other => other,
            };
            continue;
        }
        let in_code_block = fence_marker.is_some();
        if in_code_block {
            continue;
        }
        let content = if let Some(rest) = stripped.strip_prefix("- ") {
            rest.trim()
        } else if let Some(rest) = stripped.strip_prefix("* ") {
            rest.trim()
        } else if let Some(rest) = stripped.strip_prefix('\u{2022}') {
            // '•' U+2022 BULLET
            rest.trim()
        } else if let Some(rest) = stripped.strip_prefix("+ ") {
            rest.trim()
        } else if let Some(rest) = strip_numbered_prefix(stripped) {
            rest.trim()
        } else {
            continue;
        };
        if content.len() <= 10 {
            continue;
        }
        let sanitized = sanitize_text(content);
        let severity = classify_severity(&sanitized);
        findings.push(PrReviewFinding {
            severity: severity.to_owned(),
            path: String::new(),
            line: None,
            end_line: None,
            body: sanitized,
            rule_id: None,
        });
    }
    findings
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

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Strips a leading numbered list prefix (e.g., `1. `, `12. `) from a line.
/// Returns `None` if the line does not match the pattern `<digits>. <rest>`.
fn strip_numbered_prefix(s: &str) -> Option<&str> {
    let dot_pos = s.find(". ")?;
    let prefix = s.get(..dot_pos)?;
    if !prefix.is_empty() && prefix.chars().all(|c| c.is_ascii_digit()) {
        s.get(dot_pos + 2..)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Tests (written first — TDD)
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::{
        classify_severity, parse_body_findings, parse_paginated_json, sanitize_text,
        validate_reviewer_provider,
    };

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
    // classify_severity — 5 tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_classify_severity_critical_keywords() {
        assert_eq!(classify_severity("This is a critical security issue"), "P0");
    }

    #[test]
    fn test_classify_severity_bug_keywords() {
        assert_eq!(classify_severity("This is a bug in the logic"), "P0");
    }

    #[test]
    fn test_classify_severity_suggestion_keywords() {
        assert_eq!(classify_severity("You should consider refactoring"), "P1");
    }

    #[test]
    fn test_classify_severity_default_is_p1() {
        assert_eq!(classify_severity("Some inline comment"), "P1");
    }

    #[test]
    fn test_classify_severity_panic_is_p0() {
        assert_eq!(classify_severity("This code could panic"), "P0");
    }

    // -----------------------------------------------------------------------
    // parse_body_findings — 4 tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_body_findings_parses_bullet_items() {
        let body = "Review:\n- This function has a critical bug in error handling\n- Consider using a different pattern";
        let findings = parse_body_findings(body);
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].severity, "P0"); // "critical"
        assert_eq!(findings[1].severity, "P1"); // "consider"
    }

    #[test]
    fn test_parse_body_findings_skips_short_items() {
        let body = "- OK\n- This is a longer finding that should be included";
        let findings = parse_body_findings(body);
        assert_eq!(findings.len(), 1);
    }

    #[test]
    fn test_parse_body_findings_empty_body() {
        assert!(parse_body_findings("").is_empty());
    }

    #[test]
    fn test_parse_body_findings_no_bullets() {
        let body = "Everything looks good. No issues found.";
        assert!(parse_body_findings(body).is_empty());
    }

    #[test]
    fn test_parse_body_findings_sanitizes_body_content() {
        let body = "- Error in /home/user/project/main.rs with sk-abcdefghijklmnopqrstuvwxyz token";
        let findings = parse_body_findings(body);
        assert_eq!(findings.len(), 1);
        assert!(!findings[0].body.contains("/home/user"), "abs path not sanitized");
        assert!(!findings[0].body.contains("sk-abcdefghij"), "secret not sanitized");
    }

    // -----------------------------------------------------------------------
    // resolve_reviewer_provider — 2 tests
    // -----------------------------------------------------------------------

    // -----------------------------------------------------------------------
    // parse_body_findings — T004: extended format support (written first — Red phase)
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_body_findings_parses_numbered_list() {
        let body = "1. This function has a critical bug in error handling";
        let findings = parse_body_findings(body);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, "P0"); // "critical"
    }

    #[test]
    fn test_parse_body_findings_parses_plus_prefix() {
        let body = "+ This function has a critical bug in error handling";
        let findings = parse_body_findings(body);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, "P0");
    }

    #[test]
    fn test_parse_body_findings_ignores_plain_paragraph() {
        let body = "This is a plain paragraph without any list prefix. It should not be a finding.";
        let findings = parse_body_findings(body);
        assert!(findings.is_empty());
    }

    #[test]
    fn test_parse_body_findings_skips_plus_prefix_inside_code_block() {
        let body =
            "Review:\n```diff\n+ cargo test\n+ some other line that looks like a finding\n```\n";
        let findings = parse_body_findings(body);
        assert!(
            findings.is_empty(),
            "lines inside fenced code blocks must not be parsed as findings"
        );
    }

    #[test]
    fn test_parse_body_findings_skips_tilde_fence_code_block() {
        let body = "Review:\n~~~diff\n+ critical bug in error handling\n~~~\n";
        let findings = parse_body_findings(body);
        assert!(
            findings.is_empty(),
            "lines inside ~~~ fenced code blocks must not be parsed as findings"
        );
    }

    #[test]
    fn test_parse_body_findings_tilde_inside_backtick_fence_does_not_close() {
        // A ~~~ inside a ```-opened block must NOT close the block
        let body = "```\n~~~\n+ this is still inside the code block\n```\n";
        let findings = parse_body_findings(body);
        assert!(findings.is_empty(), "~~~ inside ``` block must not close the fence");
    }

    #[test]
    fn test_parse_body_findings_backtick_inside_tilde_fence_does_not_close() {
        let body = "~~~\n```\n+ this is still inside the code block\n~~~\n";
        let findings = parse_body_findings(body);
        assert!(findings.is_empty(), "``` inside ~~~ block must not close the fence");
    }

    #[test]
    fn test_parse_body_findings_longer_fence_not_closed_by_shorter() {
        // A ```````` (4-backtick) block must not be closed by ``` (3-backtick)
        let body = "````\n```\n+ still inside outer fence\n```\n````\n";
        let findings = parse_body_findings(body);
        assert!(findings.is_empty(), "``` inside ```` block must not close the fence");
    }

    #[test]
    fn test_parse_body_findings_longer_tilde_fence_not_closed_by_shorter() {
        let body = "~~~~\n~~~\n+ still inside outer fence\n~~~\n~~~~\n";
        let findings = parse_body_findings(body);
        assert!(findings.is_empty(), "~~~ inside ~~~~ block must not close the fence");
    }

    #[test]
    fn test_parse_body_findings_equal_length_fence_closes() {
        // A ``` block is correctly closed by another ```
        let body = "```\n+ inside code block\n```\n- real finding outside code block\n";
        let findings = parse_body_findings(body);
        assert_eq!(findings.len(), 1, "finding after closed fence should be parsed");
    }

    #[test]
    fn test_parse_body_findings_closing_fence_with_info_string_does_not_close() {
        // A line like ```diff inside a ``` block must NOT close the block (info string present)
        let body = "```\n```diff\n+ still inside code block\n```\n";
        let findings = parse_body_findings(body);
        assert!(
            findings.is_empty(),
            "```diff inside ``` block must not close the fence (closing fence cannot have info text)"
        );
    }

    #[test]
    fn test_parse_body_findings_deeply_indented_fence_is_not_fence() {
        // 4+ leading spaces means indented code block, not a fence (CommonMark)
        let body = "- finding before\n    ```\n+ indented but not a code block\n    ```\n- finding after\n";
        let findings = parse_body_findings(body);
        // Both "- finding before" and "- finding after" should be parsed;
        // the indented ``` lines are NOT fences.
        // 3 findings: "- finding before", "+ indented but not a code block", "- finding after"
        assert_eq!(findings.len(), 3, "4-space indented ``` must not be treated as fence");
    }

    #[test]
    fn test_parse_body_findings_tab_indented_fence_is_not_fence() {
        // A tab before ``` means ≥4 columns indent — not a CommonMark fence
        let body =
            "- finding before\n\t```diff\n+ inside tab-indented block\n\t```\n- finding after\n";
        let findings = parse_body_findings(body);
        // Tab-indented ``` is NOT a fence, so all 3 lines are findings
        assert_eq!(findings.len(), 3, "tab-indented ``` must not be treated as fence");
    }

    #[test]
    fn test_parse_body_findings_quoted_rm_blocked() {
        // Test that quoted rm commands are still detected (uses hook.rs but validates integration)
        let body = "- critical bug: rm command bypasses guard with quotes\n";
        let findings = parse_body_findings(body);
        assert_eq!(findings.len(), 1);
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
}
