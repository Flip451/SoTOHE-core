//! Verdict parsing, classification, and validation for local reviewer workflow.

use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

use domain::StoredFinding;
pub use domain::review::{ModelProfile, resolve_full_auto};

/// Errors returned by review workflow functions.
#[derive(Debug, Error)]
pub enum ReviewWorkflowError {
    #[error("failed to serialize reviewer final payload: {0}")]
    Serialize(#[from] serde_json::Error),
}

pub const REVIEW_OUTPUT_SCHEMA_JSON: &str = r##"{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "properties": {
    "verdict": {
      "type": "string",
      "enum": ["zero_findings", "findings_remain"]
    },
    "findings": {
      "type": "array",
      "items": { "$ref": "#/$defs/finding" }
    }
  },
  "required": ["verdict", "findings"],
  "additionalProperties": false,
  "$defs": {
    "non_blank_string": {
      "type": "string",
      "pattern": ".*\\S.*"
    },
    "nullable_non_blank_string": {
      "anyOf": [
        { "$ref": "#/$defs/non_blank_string" },
        { "type": "null" }
      ]
    },
    "finding": {
      "type": "object",
      "properties": {
        "message": { "$ref": "#/$defs/non_blank_string" },
        "severity": { "$ref": "#/$defs/nullable_non_blank_string" },
        "file": { "$ref": "#/$defs/nullable_non_blank_string" },
        "line": { "type": ["integer", "null"], "minimum": 1 },
        "category": { "$ref": "#/$defs/nullable_non_blank_string" }
      },
      "required": ["message", "severity", "file", "line", "category"],
      "additionalProperties": false
    }
  }
}"##;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewVerdict {
    ZeroFindings,
    FindingsRemain,
    Timeout,
    ProcessFailed,
    LastMessageMissing,
}

impl ReviewVerdict {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ZeroFindings => "zero_findings",
            Self::FindingsRemain => "findings_remain",
            Self::Timeout => "timeout",
            Self::ProcessFailed => "process_failed",
            Self::LastMessageMissing => "last_message_missing",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewFinalMessageState {
    Missing,
    Invalid { reason: String },
    Parsed(ReviewFinalPayload),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReviewFinalPayload {
    pub verdict: ReviewPayloadVerdict,
    pub findings: Vec<ReviewFinding>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewPayloadVerdict {
    ZeroFindings,
    FindingsRemain,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReviewFinding {
    pub message: String,
    pub severity: Option<String>,
    pub file: Option<String>,
    pub line: Option<u64>,
    pub category: Option<String>,
}

impl<'de> Deserialize<'de> for ReviewFinding {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_map(StrictReviewFindingVisitor)
    }
}

impl<'de> Deserialize<'de> for ReviewFinalPayload {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_map(StrictReviewFinalPayloadVisitor)
    }
}

struct StrictReviewFinalPayloadVisitor;
struct StrictReviewFindingVisitor;

impl<'de> Visitor<'de> for StrictReviewFinalPayloadVisitor {
    type Value = ReviewFinalPayload;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a review payload object with exact required fields")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut verdict: Option<ReviewPayloadVerdict> = None;
        let mut findings: Option<Vec<ReviewFinding>> = None;

        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "verdict" => {
                    if verdict.is_some() {
                        return Err(de::Error::duplicate_field("verdict"));
                    }
                    verdict = Some(map.next_value()?);
                }
                "findings" => {
                    if findings.is_some() {
                        return Err(de::Error::duplicate_field("findings"));
                    }
                    findings = Some(map.next_value()?);
                }
                other => return Err(de::Error::unknown_field(other, &["verdict", "findings"])),
            }
        }

        Ok(ReviewFinalPayload {
            verdict: verdict.ok_or_else(|| de::Error::missing_field("verdict"))?,
            findings: findings.ok_or_else(|| de::Error::missing_field("findings"))?,
        })
    }
}

impl<'de> Visitor<'de> for StrictReviewFindingVisitor {
    type Value = ReviewFinding;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a review finding object with exact required fields")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut message: Option<String> = None;
        let mut severity: Option<Option<String>> = None;
        let mut file: Option<Option<String>> = None;
        let mut line: Option<Option<u64>> = None;
        let mut category: Option<Option<String>> = None;

        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "message" => {
                    if message.is_some() {
                        return Err(de::Error::duplicate_field("message"));
                    }
                    message = Some(map.next_value()?);
                }
                "severity" => {
                    if severity.is_some() {
                        return Err(de::Error::duplicate_field("severity"));
                    }
                    severity = Some(map.next_value()?);
                }
                "file" => {
                    if file.is_some() {
                        return Err(de::Error::duplicate_field("file"));
                    }
                    file = Some(map.next_value()?);
                }
                "line" => {
                    if line.is_some() {
                        return Err(de::Error::duplicate_field("line"));
                    }
                    line = Some(map.next_value()?);
                }
                "category" => {
                    if category.is_some() {
                        return Err(de::Error::duplicate_field("category"));
                    }
                    category = Some(map.next_value()?);
                }
                other => {
                    return Err(de::Error::unknown_field(
                        other,
                        &["message", "severity", "file", "line", "category"],
                    ));
                }
            }
        }

        Ok(ReviewFinding {
            message: message.ok_or_else(|| de::Error::missing_field("message"))?,
            severity: severity.ok_or_else(|| de::Error::missing_field("severity"))?,
            file: file.ok_or_else(|| de::Error::missing_field("file"))?,
            line: line.ok_or_else(|| de::Error::missing_field("line"))?,
            category: category.ok_or_else(|| de::Error::missing_field("category"))?,
        })
    }
}

#[must_use]
pub fn review_findings_to_stored(findings: &[ReviewFinding]) -> Vec<StoredFinding> {
    findings
        .iter()
        .map(|finding| {
            StoredFinding::new(
                finding.message.clone(),
                finding.severity.clone(),
                finding.file.clone(),
                finding.line,
            )
            .with_category(finding.category.clone())
        })
        .collect()
}

#[must_use]
pub fn normalize_final_message(content: &str) -> Option<String> {
    let trimmed = content.trim();
    if trimmed.is_empty() { None } else { Some(trimmed.to_owned()) }
}

#[must_use]
pub fn parse_review_final_message(content: Option<&str>) -> ReviewFinalMessageState {
    parse_review_final_message_with(content, validate_review_json_shape)
}

#[must_use]
pub fn parse_review_final_message_compatible(content: Option<&str>) -> ReviewFinalMessageState {
    let Some(content) = content else {
        return ReviewFinalMessageState::Missing;
    };

    parse_review_final_message_bridge_missing_category(content).unwrap_or_else(|| {
        parse_review_final_message_with(Some(content), validate_review_json_shape)
    })
}

fn parse_review_final_message_with(
    content: Option<&str>,
    validator: fn(&str) -> Result<(), serde_json::Error>,
) -> ReviewFinalMessageState {
    let Some(content) = content else {
        return ReviewFinalMessageState::Missing;
    };

    match validator(content) {
        Ok(()) => match serde_json::from_str::<ReviewFinalPayload>(content) {
            Ok(payload) => match validate_review_payload(payload) {
                Ok(payload) => ReviewFinalMessageState::Parsed(payload),
                Err(reason) => ReviewFinalMessageState::Invalid { reason },
            },
            Err(err) => ReviewFinalMessageState::Invalid {
                reason: format!("expected review JSON object: {err}"),
            },
        },
        Err(err) => ReviewFinalMessageState::Invalid {
            reason: format!("expected review JSON object: {err}"),
        },
    }
}

fn parse_review_final_message_bridge_missing_category(
    content: &str,
) -> Option<ReviewFinalMessageState> {
    if validate_review_json_shape_bridge_missing_category(content).is_err() {
        return None;
    }

    match serde_json::from_str::<CategoryBridgeReviewFinalPayload>(content) {
        Ok(payload) => {
            let payload = ReviewFinalPayload {
                verdict: payload.verdict,
                findings: payload
                    .findings
                    .into_iter()
                    .map(|finding| ReviewFinding {
                        message: finding.message,
                        severity: finding.severity,
                        file: finding.file,
                        line: finding.line,
                        category: finding.category,
                    })
                    .collect(),
            };
            Some(match validate_review_payload(payload) {
                Ok(payload) => ReviewFinalMessageState::Parsed(payload),
                Err(reason) => ReviewFinalMessageState::Invalid { reason },
            })
        }
        Err(_) => None,
    }
}

#[must_use]
pub fn classify_review_verdict(
    timed_out: bool,
    exit_success: bool,
    final_message: &ReviewFinalMessageState,
) -> ReviewVerdict {
    if timed_out {
        ReviewVerdict::Timeout
    } else {
        match final_message {
            ReviewFinalMessageState::Parsed(ReviewFinalPayload {
                verdict: ReviewPayloadVerdict::ZeroFindings,
                ..
            }) if exit_success => ReviewVerdict::ZeroFindings,
            ReviewFinalMessageState::Parsed(ReviewFinalPayload {
                verdict: ReviewPayloadVerdict::ZeroFindings,
                ..
            }) => ReviewVerdict::ProcessFailed,
            ReviewFinalMessageState::Parsed(ReviewFinalPayload {
                verdict: ReviewPayloadVerdict::FindingsRemain,
                ..
            }) if exit_success => ReviewVerdict::FindingsRemain,
            ReviewFinalMessageState::Parsed(ReviewFinalPayload {
                verdict: ReviewPayloadVerdict::FindingsRemain,
                ..
            }) => ReviewVerdict::ProcessFailed,
            ReviewFinalMessageState::Missing if exit_success => ReviewVerdict::LastMessageMissing,
            ReviewFinalMessageState::Missing | ReviewFinalMessageState::Invalid { .. } => {
                ReviewVerdict::ProcessFailed
            }
        }
    }
}

/// Attempts to extract a valid verdict from session log content.
///
/// Delegates candidate scanning to domain, validates each candidate.
/// The caller is responsible for reading the file content.
///
/// Returns `None` if no valid verdict JSON is found.
#[must_use]
pub fn extract_verdict_from_content(content: &str) -> Option<ReviewFinalMessageState> {
    // Strategy 1: compact single-line JSON (bottom-up scan).
    for candidate in domain::review::extract_verdict_json_candidates_compact(content) {
        let state = parse_review_final_message_compatible(Some(&candidate));
        if matches!(state, ReviewFinalMessageState::Parsed(_)) {
            return Some(state);
        }
    }

    // Strategy 2: pretty-printed multi-line JSON (bottom-up scan).
    for candidate in domain::review::extract_verdict_json_candidates_multiline(content) {
        let state = parse_review_final_message_compatible(Some(&candidate));
        if matches!(state, ReviewFinalMessageState::Parsed(_)) {
            return Some(state);
        }
    }

    None
}

/// Serializes a `ReviewFinalPayload` to a JSON string.
///
/// # Errors
/// Returns `ReviewWorkflowError::Serialize` on serialization failure.
pub fn render_review_payload(payload: &ReviewFinalPayload) -> Result<String, ReviewWorkflowError> {
    Ok(serde_json::to_string(payload)?)
}

pub(crate) fn validate_review_payload(
    payload: ReviewFinalPayload,
) -> Result<ReviewFinalPayload, String> {
    if payload.findings.iter().any(|finding| finding.message.trim().is_empty()) {
        return Err("findings entries must include a non-empty `message`".to_owned());
    }
    if payload
        .findings
        .iter()
        .any(|finding| finding.severity.as_deref().is_some_and(|value| value.trim().is_empty()))
    {
        return Err("findings entries must use `severity: null` or a non-empty string".to_owned());
    }
    if payload
        .findings
        .iter()
        .any(|finding| finding.file.as_deref().is_some_and(|value| value.trim().is_empty()))
    {
        return Err("findings entries must use `file: null` or a non-empty string".to_owned());
    }
    if payload.findings.iter().any(|finding| finding.line == Some(0)) {
        return Err("findings entries must use `line: null` or a 1-based line number".to_owned());
    }
    if payload
        .findings
        .iter()
        .any(|finding| finding.category.as_deref().is_some_and(|value| value.trim().is_empty()))
    {
        return Err("findings entries must use `category: null` or a non-empty string".to_owned());
    }

    match payload.verdict {
        ReviewPayloadVerdict::ZeroFindings if !payload.findings.is_empty() => {
            Err("`zero_findings` payload must use an empty `findings` array".to_owned())
        }
        ReviewPayloadVerdict::FindingsRemain if payload.findings.is_empty() => {
            Err("`findings_remain` payload must include at least one finding".to_owned())
        }
        _ => Ok(payload),
    }
}

pub(crate) fn validate_review_json_shape(content: &str) -> Result<(), serde_json::Error> {
    let mut deserializer = serde_json::Deserializer::from_str(content);
    ReviewPayloadShape::deserialize(&mut deserializer)?;
    deserializer.end()
}

pub(crate) fn validate_review_json_shape_bridge_missing_category(
    content: &str,
) -> Result<(), serde_json::Error> {
    let mut deserializer = serde_json::Deserializer::from_str(content);
    CategoryBridgeReviewPayloadShape::deserialize(&mut deserializer)?;
    deserializer.end()
}

struct ReviewPayloadShape;
struct CategoryBridgeReviewPayloadShape;

impl<'de> Deserialize<'de> for ReviewPayloadShape {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_map(ReviewPayloadShapeVisitor)
    }
}

struct ReviewPayloadShapeVisitor;
struct CategoryBridgeReviewPayloadShapeVisitor;

impl<'de> Visitor<'de> for ReviewPayloadShapeVisitor {
    type Value = ReviewPayloadShape;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a review payload object with `verdict` and `findings`")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut seen_verdict = false;
        let mut seen_findings = false;

        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "verdict" => {
                    if seen_verdict {
                        return Err(de::Error::duplicate_field("verdict"));
                    }
                    let _: ReviewPayloadVerdict = map.next_value()?;
                    seen_verdict = true;
                }
                "findings" => {
                    if seen_findings {
                        return Err(de::Error::duplicate_field("findings"));
                    }
                    let _: Vec<ReviewFindingShape> = map.next_value()?;
                    seen_findings = true;
                }
                other => return Err(de::Error::unknown_field(other, &["verdict", "findings"])),
            }
        }

        if !seen_verdict {
            return Err(de::Error::missing_field("verdict"));
        }
        if !seen_findings {
            return Err(de::Error::missing_field("findings"));
        }

        Ok(ReviewPayloadShape)
    }
}

impl<'de> Deserialize<'de> for CategoryBridgeReviewPayloadShape {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_map(CategoryBridgeReviewPayloadShapeVisitor)
    }
}

impl<'de> Visitor<'de> for CategoryBridgeReviewPayloadShapeVisitor {
    type Value = CategoryBridgeReviewPayloadShape;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a review payload object with `verdict` and `findings`")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut seen_verdict = false;
        let mut seen_findings = false;

        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "verdict" => {
                    if seen_verdict {
                        return Err(de::Error::duplicate_field("verdict"));
                    }
                    let _: ReviewPayloadVerdict = map.next_value()?;
                    seen_verdict = true;
                }
                "findings" => {
                    if seen_findings {
                        return Err(de::Error::duplicate_field("findings"));
                    }
                    let _: Vec<CategoryBridgeReviewFindingShape> = map.next_value()?;
                    seen_findings = true;
                }
                other => return Err(de::Error::unknown_field(other, &["verdict", "findings"])),
            }
        }

        if !seen_verdict {
            return Err(de::Error::missing_field("verdict"));
        }
        if !seen_findings {
            return Err(de::Error::missing_field("findings"));
        }

        Ok(CategoryBridgeReviewPayloadShape)
    }
}

struct ReviewFindingShape;
struct CategoryBridgeReviewFindingShape;

impl<'de> Deserialize<'de> for ReviewFindingShape {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_map(ReviewFindingShapeVisitor)
    }
}

struct ReviewFindingShapeVisitor;
struct CategoryBridgeReviewFindingShapeVisitor;

impl<'de> Visitor<'de> for ReviewFindingShapeVisitor {
    type Value = ReviewFindingShape;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a review finding object with exact required fields")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut seen_message = false;
        let mut seen_severity = false;
        let mut seen_file = false;
        let mut seen_line = false;
        let mut seen_category = false;
        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "message" => {
                    if seen_message {
                        return Err(de::Error::duplicate_field("message"));
                    }
                    let _: String = map.next_value()?;
                    seen_message = true;
                }
                "severity" => {
                    if seen_severity {
                        return Err(de::Error::duplicate_field("severity"));
                    }
                    let _: Option<String> = map.next_value()?;
                    seen_severity = true;
                }
                "file" => {
                    if seen_file {
                        return Err(de::Error::duplicate_field("file"));
                    }
                    let _: Option<String> = map.next_value()?;
                    seen_file = true;
                }
                "line" => {
                    if seen_line {
                        return Err(de::Error::duplicate_field("line"));
                    }
                    let _: Option<u64> = map.next_value()?;
                    seen_line = true;
                }
                "category" => {
                    if seen_category {
                        return Err(de::Error::duplicate_field("category"));
                    }
                    let _: Option<String> = map.next_value()?;
                    seen_category = true;
                }
                other => {
                    return Err(de::Error::unknown_field(
                        other,
                        &["message", "severity", "file", "line", "category"],
                    ));
                }
            }
        }

        if !seen_message {
            return Err(de::Error::missing_field("message"));
        }
        if !seen_severity {
            return Err(de::Error::missing_field("severity"));
        }
        if !seen_file {
            return Err(de::Error::missing_field("file"));
        }
        if !seen_line {
            return Err(de::Error::missing_field("line"));
        }
        if !seen_category {
            return Err(de::Error::missing_field("category"));
        }
        Ok(ReviewFindingShape)
    }
}

impl<'de> Deserialize<'de> for CategoryBridgeReviewFindingShape {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_map(CategoryBridgeReviewFindingShapeVisitor)
    }
}

impl<'de> Visitor<'de> for CategoryBridgeReviewFindingShapeVisitor {
    type Value = CategoryBridgeReviewFindingShape;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(
            "a review finding object with `message`, `severity`, `file`, `line`, and optional `category`",
        )
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut seen_message = false;
        let mut seen_severity = false;
        let mut seen_file = false;
        let mut seen_line = false;
        let mut seen_category = false;

        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "message" => {
                    if seen_message {
                        return Err(de::Error::duplicate_field("message"));
                    }
                    let _: String = map.next_value()?;
                    seen_message = true;
                }
                "severity" => {
                    if seen_severity {
                        return Err(de::Error::duplicate_field("severity"));
                    }
                    let _: Option<String> = map.next_value()?;
                    seen_severity = true;
                }
                "file" => {
                    if seen_file {
                        return Err(de::Error::duplicate_field("file"));
                    }
                    let _: Option<String> = map.next_value()?;
                    seen_file = true;
                }
                "line" => {
                    if seen_line {
                        return Err(de::Error::duplicate_field("line"));
                    }
                    let _: Option<u64> = map.next_value()?;
                    seen_line = true;
                }
                "category" => {
                    if seen_category {
                        return Err(de::Error::duplicate_field("category"));
                    }
                    let _: Option<String> = map.next_value()?;
                    seen_category = true;
                }
                other => {
                    return Err(de::Error::unknown_field(
                        other,
                        &["message", "severity", "file", "line", "category"],
                    ));
                }
            }
        }

        if !seen_message {
            return Err(de::Error::missing_field("message"));
        }
        if !seen_severity {
            return Err(de::Error::missing_field("severity"));
        }
        if !seen_file {
            return Err(de::Error::missing_field("file"));
        }
        if !seen_line {
            return Err(de::Error::missing_field("line"));
        }

        Ok(CategoryBridgeReviewFindingShape)
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CategoryBridgeReviewFinalPayload {
    verdict: ReviewPayloadVerdict,
    #[serde(default)]
    findings: Vec<CategoryBridgeReviewFinding>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CategoryBridgeReviewFinding {
    message: String,
    severity: Option<String>,
    file: Option<String>,
    line: Option<u64>,
    #[serde(default)]
    category: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::{
        ReviewFinalMessageState, ReviewFinalPayload, ReviewFinding, ReviewPayloadVerdict,
        extract_verdict_from_content, parse_review_final_message,
        parse_review_final_message_compatible, render_review_payload,
    };

    #[test]
    fn test_parse_review_final_message_rejects_missing_category_field() {
        let state = parse_review_final_message(Some(
            r#"{"verdict":"findings_remain","findings":[{"message":"P1","severity":"P1","file":null,"line":1}]}"#,
        ));

        assert!(matches!(state, ReviewFinalMessageState::Invalid { .. }));
    }

    #[test]
    fn test_parse_review_final_message_rejects_missing_severity_field() {
        let state = parse_review_final_message(Some(
            r#"{"verdict":"findings_remain","findings":[{"message":"P1","file":null,"line":1,"category":null}]}"#,
        ));

        assert!(matches!(state, ReviewFinalMessageState::Invalid { .. }));
    }

    #[test]
    fn test_parse_review_final_message_rejects_missing_file_field() {
        let state = parse_review_final_message(Some(
            r#"{"verdict":"findings_remain","findings":[{"message":"P1","severity":"P1","line":1,"category":null}]}"#,
        ));

        assert!(matches!(state, ReviewFinalMessageState::Invalid { .. }));
    }

    #[test]
    fn test_parse_review_final_message_rejects_missing_line_field() {
        let state = parse_review_final_message(Some(
            r#"{"verdict":"findings_remain","findings":[{"message":"P1","severity":"P1","file":null,"category":null}]}"#,
        ));

        assert!(matches!(state, ReviewFinalMessageState::Invalid { .. }));
    }

    #[test]
    fn test_parse_review_final_message_compatible_accepts_missing_category_field_as_none() {
        let state = parse_review_final_message_compatible(Some(
            r#"{"verdict":"findings_remain","findings":[{"message":"P1","severity":"P1","file":null,"line":1}]}"#,
        ));

        assert!(matches!(state, ReviewFinalMessageState::Parsed(_)));
    }

    #[test]
    fn test_parse_review_final_message_compatible_rejects_missing_severity_field() {
        let state = parse_review_final_message_compatible(Some(
            r#"{"verdict":"findings_remain","findings":[{"message":"P1","file":null,"line":1,"category":null}]}"#,
        ));

        assert!(matches!(state, ReviewFinalMessageState::Invalid { .. }));
    }

    #[test]
    fn test_parse_review_final_message_compatible_rejects_missing_file_field() {
        let state = parse_review_final_message_compatible(Some(
            r#"{"verdict":"findings_remain","findings":[{"message":"P1","severity":"P1","line":1,"category":null}]}"#,
        ));

        assert!(matches!(state, ReviewFinalMessageState::Invalid { .. }));
    }

    #[test]
    fn test_parse_review_final_message_compatible_rejects_missing_line_field() {
        let state = parse_review_final_message_compatible(Some(
            r#"{"verdict":"findings_remain","findings":[{"message":"P1","severity":"P1","file":null,"category":null}]}"#,
        ));

        assert!(matches!(state, ReviewFinalMessageState::Invalid { .. }));
    }

    #[test]
    fn test_parse_review_final_message_compatible_rejects_missing_findings_field() {
        let state = parse_review_final_message_compatible(Some(r#"{"verdict":"zero_findings"}"#));

        assert!(matches!(state, ReviewFinalMessageState::Invalid { .. }));
    }

    #[test]
    fn test_parse_review_final_message_accepts_null_category_field() {
        let state = parse_review_final_message(Some(
            r#"{"verdict":"findings_remain","findings":[{"message":"P1","severity":"P1","file":null,"line":1,"category":null}]}"#,
        ));

        assert!(matches!(state, ReviewFinalMessageState::Parsed(_)));
    }

    #[test]
    fn test_parse_review_final_message_compatible_rejects_duplicate_category_field() {
        let state = parse_review_final_message_compatible(Some(
            r#"{"verdict":"findings_remain","findings":[{"message":"P1","severity":"P1","file":null,"line":1,"category":null,"category":"workflow"}]}"#,
        ));

        assert!(matches!(state, ReviewFinalMessageState::Invalid { .. }));
    }

    #[test]
    fn test_extract_verdict_from_content_accepts_missing_category_payload() {
        let state = extract_verdict_from_content(
            r#"noise
{"verdict":"findings_remain","findings":[{"message":"P1","severity":"P1","file":null,"line":1}]}
"#,
        );

        assert!(matches!(state, Some(ReviewFinalMessageState::Parsed(_))));
    }

    #[test]
    fn test_render_review_payload_preserves_null_category_field() {
        let payload = ReviewFinalPayload {
            verdict: ReviewPayloadVerdict::FindingsRemain,
            findings: vec![ReviewFinding {
                message: "P1".to_owned(),
                severity: Some("P1".to_owned()),
                file: None,
                line: Some(1),
                category: None,
            }],
        };

        let json = render_review_payload(&payload);
        assert!(json.is_ok(), "render_review_payload should serialize null category: {json:?}");
        let json = json.unwrap_or_default();

        assert!(
            json.contains(r#""category":null"#),
            "rendered payload must preserve a required null category field: {json}"
        );
    }

    #[test]
    fn test_review_final_payload_deserialize_rejects_missing_category_field() {
        let parsed = serde_json::from_str::<ReviewFinalPayload>(
            r#"{"verdict":"findings_remain","findings":[{"message":"P1","severity":"P1","file":null,"line":1}]}"#,
        );

        assert!(parsed.is_err(), "strict payload type must reject missing category");
    }

    #[test]
    fn test_review_final_payload_deserialize_rejects_missing_severity_field() {
        let parsed = serde_json::from_str::<ReviewFinalPayload>(
            r#"{"verdict":"findings_remain","findings":[{"message":"P1","file":null,"line":1,"category":null}]}"#,
        );

        assert!(parsed.is_err(), "strict payload type must reject missing severity");
    }

    #[test]
    fn test_review_final_payload_deserialize_rejects_missing_file_field() {
        let parsed = serde_json::from_str::<ReviewFinalPayload>(
            r#"{"verdict":"findings_remain","findings":[{"message":"P1","severity":"P1","line":1,"category":null}]}"#,
        );

        assert!(parsed.is_err(), "strict payload type must reject missing file");
    }

    #[test]
    fn test_review_final_payload_deserialize_rejects_missing_line_field() {
        let parsed = serde_json::from_str::<ReviewFinalPayload>(
            r#"{"verdict":"findings_remain","findings":[{"message":"P1","severity":"P1","file":null,"category":null}]}"#,
        );

        assert!(parsed.is_err(), "strict payload type must reject missing line");
    }

    #[test]
    fn test_review_final_payload_deserialize_rejects_missing_findings_field() {
        let parsed = serde_json::from_str::<ReviewFinalPayload>(r#"{"verdict":"zero_findings"}"#);

        assert!(parsed.is_err(), "strict payload type must reject missing findings");
    }

    #[test]
    fn test_review_final_payload_deserialize_accepts_required_nullable_fields_as_null() {
        let parsed = serde_json::from_str::<ReviewFinalPayload>(
            r#"{"verdict":"findings_remain","findings":[{"message":"P1","severity":null,"file":null,"line":null,"category":null}]}"#,
        );

        assert!(
            matches!(
                parsed,
                Ok(ReviewFinalPayload { verdict: ReviewPayloadVerdict::FindingsRemain, .. })
            ),
            "explicit nulls must remain accepted for required nullable fields: {parsed:?}"
        );
        if let Ok(payload) = parsed {
            assert_eq!(payload.findings.len(), 1);
            assert_eq!(
                payload.findings.first().map(|finding| {
                    (
                        finding.severity.clone(),
                        finding.file.clone(),
                        finding.line,
                        finding.category.clone(),
                    )
                }),
                Some((None, None, None, None))
            );
        }
    }

    #[test]
    fn test_parse_review_final_message_rejects_blank_category_field() {
        let state = parse_review_final_message(Some(
            r#"{"verdict":"findings_remain","findings":[{"message":"P1","severity":"P1","file":"src/lib.rs","line":1,"category":" "}]} "#.trim(),
        ));

        assert!(matches!(state, ReviewFinalMessageState::Invalid { .. }));
    }
}
