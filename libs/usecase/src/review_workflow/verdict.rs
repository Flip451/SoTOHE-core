//! Verdict parsing, classification, and validation for local reviewer workflow.

use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

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
    "finding": {
      "type": "object",
      "properties": {
        "message": { "type": "string" },
        "severity": { "type": ["string", "null"] },
        "file": { "type": ["string", "null"] },
        "line": { "type": ["integer", "null"], "minimum": 1 },
        "category": {
          "type": ["string", "null"],
          "description": "Optional concern category for escalation tracking"
        }
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

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewFinalPayload {
    pub verdict: ReviewPayloadVerdict,
    #[serde(default)]
    pub findings: Vec<ReviewFinding>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewPayloadVerdict {
    ZeroFindings,
    FindingsRemain,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewFinding {
    pub message: String,
    #[serde(default)]
    pub severity: Option<String>,
    #[serde(default)]
    pub file: Option<String>,
    #[serde(default)]
    pub line: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
}

#[must_use]
pub fn normalize_final_message(content: &str) -> Option<String> {
    let trimmed = content.trim();
    if trimmed.is_empty() { None } else { Some(trimmed.to_owned()) }
}

#[must_use]
pub fn parse_review_final_message(content: Option<&str>) -> ReviewFinalMessageState {
    let Some(content) = content else {
        return ReviewFinalMessageState::Missing;
    };

    match validate_review_json_shape(content) {
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
        let state = parse_review_final_message(Some(&candidate));
        if matches!(state, ReviewFinalMessageState::Parsed(_)) {
            return Some(state);
        }
    }

    // Strategy 2: pretty-printed multi-line JSON (bottom-up scan).
    for candidate in domain::review::extract_verdict_json_candidates_multiline(content) {
        let state = parse_review_final_message(Some(&candidate));
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

struct ReviewPayloadShape;

impl<'de> Deserialize<'de> for ReviewPayloadShape {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_map(ReviewPayloadShapeVisitor)
    }
}

struct ReviewPayloadShapeVisitor;

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

struct ReviewFindingShape;

impl<'de> Deserialize<'de> for ReviewFindingShape {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_map(ReviewFindingShapeVisitor)
    }
}

struct ReviewFindingShapeVisitor;

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

        Ok(ReviewFindingShape)
    }
}
