//! Pure workflow rules for local reviewer verdict normalization.

use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

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
        "line": { "type": ["integer", "null"], "minimum": 1 }
      },
      "required": ["message", "severity", "file", "line"],
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

fn default_full_auto() -> bool {
    true
}

/// Per-model behavioral profile loaded from `agent-profiles.json`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ModelProfile {
    /// Whether `--full-auto` should be passed to `codex exec`.
    /// Defaults to `true` (fail-closed) when omitted from JSON.
    #[serde(default = "default_full_auto")]
    pub full_auto: bool,
}

/// Resolves whether `--full-auto` should be enabled for the given model.
///
/// Looks up `model` in the provided `model_profiles` map.
/// Falls back to `true` (fail-closed) when the model is not found
/// or when `model_profiles` is `None`.
///
/// # Errors
///
/// This function does not return errors — unknown models default to `true`.
#[must_use]
pub fn resolve_full_auto(
    model: &str,
    model_profiles: Option<&std::collections::HashMap<String, ModelProfile>>,
) -> bool {
    match model_profiles {
        Some(profiles) => profiles.get(model).is_none_or(|profile| profile.full_auto),
        None => true,
    }
}

pub fn render_review_payload(payload: &ReviewFinalPayload) -> Result<String, ReviewWorkflowError> {
    Ok(serde_json::to_string(payload)?)
}

fn validate_review_payload(payload: ReviewFinalPayload) -> Result<ReviewFinalPayload, String> {
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

fn validate_review_json_shape(content: &str) -> Result<(), serde_json::Error> {
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
                other => {
                    return Err(de::Error::unknown_field(
                        other,
                        &["message", "severity", "file", "line"],
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::{
        ModelProfile, REVIEW_OUTPUT_SCHEMA_JSON, ReviewFinalMessageState, ReviewFinalPayload,
        ReviewFinding, ReviewPayloadVerdict, ReviewVerdict, classify_review_verdict,
        normalize_final_message, parse_review_final_message, resolve_full_auto,
    };
    use serde_json::Value;
    use std::collections::HashMap;

    #[test]
    fn review_output_schema_json_contains_expected_verdict_literals() {
        let schema: Value = serde_json::from_str(REVIEW_OUTPUT_SCHEMA_JSON).unwrap();
        let required =
            schema.get("required").and_then(Value::as_array).cloned().unwrap_or_default();
        let finding_required = schema
            .get("$defs")
            .and_then(|defs| defs.get("finding"))
            .and_then(|finding| finding.get("required"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        assert!(REVIEW_OUTPUT_SCHEMA_JSON.contains("\"zero_findings\""));
        assert!(REVIEW_OUTPUT_SCHEMA_JSON.contains("\"findings_remain\""));
        assert!(
            REVIEW_OUTPUT_SCHEMA_JSON
                .contains("\"enum\": [\"zero_findings\", \"findings_remain\"]")
        );
        assert!(REVIEW_OUTPUT_SCHEMA_JSON.contains("\"minimum\": 1"));
        assert_eq!(schema.get("type").and_then(Value::as_str), Some("object"));
        assert_eq!(
            required,
            vec![Value::String("verdict".to_owned()), Value::String("findings".to_owned())]
        );
        assert_eq!(
            finding_required,
            vec![
                Value::String("message".to_owned()),
                Value::String("severity".to_owned()),
                Value::String("file".to_owned()),
                Value::String("line".to_owned())
            ]
        );
    }

    #[test]
    fn normalize_final_message_trims_and_rejects_empty_content() {
        assert_eq!(
            normalize_final_message("  {\"verdict\":\"zero_findings\"}\n"),
            Some("{\"verdict\":\"zero_findings\"}".to_owned())
        );
        assert_eq!(normalize_final_message(" \n\t "), None);
    }

    #[test]
    fn classify_review_verdict_prioritizes_timeout() {
        let verdict = classify_review_verdict(
            true,
            false,
            &ReviewFinalMessageState::Parsed(ReviewFinalPayload {
                verdict: ReviewPayloadVerdict::ZeroFindings,
                findings: Vec::new(),
            }),
        );

        assert_eq!(verdict, ReviewVerdict::Timeout);
    }

    #[test]
    fn parse_review_final_message_accepts_zero_findings_payload() {
        let payload =
            parse_review_final_message(Some("{\"verdict\":\"zero_findings\",\"findings\":[]}"));

        assert_eq!(
            payload,
            ReviewFinalMessageState::Parsed(ReviewFinalPayload {
                verdict: ReviewPayloadVerdict::ZeroFindings,
                findings: Vec::new(),
            })
        );
    }

    #[test]
    fn parse_review_final_message_accepts_findings_payload() {
        let payload = parse_review_final_message(Some(
            "{\"verdict\":\"findings_remain\",\"findings\":[{\"message\":\"P1: finding\",\"severity\":\"P1\",\"file\":null,\"line\":null}]}",
        ));

        assert_eq!(
            payload,
            ReviewFinalMessageState::Parsed(ReviewFinalPayload {
                verdict: ReviewPayloadVerdict::FindingsRemain,
                findings: vec![ReviewFinding {
                    message: "P1: finding".to_owned(),
                    severity: Some("P1".to_owned()),
                    file: None,
                    line: None,
                }],
            })
        );
    }

    #[test]
    fn parse_review_final_message_rejects_malformed_or_inconsistent_payloads() {
        let malformed = parse_review_final_message(Some("NO_FINDINGS"));
        let extra_field = parse_review_final_message(Some(
            "{\"verdict\":\"zero_findings\",\"findings\":[],\"extra\":\"oops\"}",
        ));
        let missing_nested_required = parse_review_final_message(Some(
            "{\"verdict\":\"findings_remain\",\"findings\":[{\"message\":\"P1: finding\"}]}",
        ));
        let zero_line = parse_review_final_message(Some(
            "{\"verdict\":\"findings_remain\",\"findings\":[{\"message\":\"P1: finding\",\"severity\":\"P1\",\"file\":\"src/lib.rs\",\"line\":0}]}",
        ));
        let empty_severity = parse_review_final_message(Some(
            "{\"verdict\":\"findings_remain\",\"findings\":[{\"message\":\"P1: finding\",\"severity\":\"\",\"file\":\"src/lib.rs\",\"line\":1}]}",
        ));
        let empty_file = parse_review_final_message(Some(
            "{\"verdict\":\"findings_remain\",\"findings\":[{\"message\":\"P1: finding\",\"severity\":\"P1\",\"file\":\"\",\"line\":1}]}",
        ));

        assert!(matches!(malformed, ReviewFinalMessageState::Invalid { .. }));
        assert!(matches!(extra_field, ReviewFinalMessageState::Invalid { .. }));
        assert!(matches!(missing_nested_required, ReviewFinalMessageState::Invalid { .. }));
        assert!(matches!(zero_line, ReviewFinalMessageState::Invalid { .. }));
        assert!(matches!(empty_severity, ReviewFinalMessageState::Invalid { .. }));
        assert!(matches!(empty_file, ReviewFinalMessageState::Invalid { .. }));
    }

    #[test]
    fn parse_review_final_message_rejects_duplicate_root_keys() {
        let duplicate_verdict = parse_review_final_message(Some(
            "{\"verdict\":\"findings_remain\",\"verdict\":\"zero_findings\",\"findings\":[]}",
        ));

        assert!(matches!(duplicate_verdict, ReviewFinalMessageState::Invalid { .. }));
    }

    #[test]
    fn parse_review_final_message_rejects_inconsistent_verdict_payloads() {
        // Real finding (severity set) with zero_findings verdict — still rejected.
        let zero_with_real_findings = parse_review_final_message(Some(
            "{\"verdict\":\"zero_findings\",\"findings\":[{\"message\":\"P1: finding\",\"severity\":\"P1\",\"file\":null,\"line\":null}]}",
        ));
        let findings_without_entries =
            parse_review_final_message(Some("{\"verdict\":\"findings_remain\",\"findings\":[]}"));

        assert!(matches!(zero_with_real_findings, ReviewFinalMessageState::Invalid { .. }));
        assert!(matches!(findings_without_entries, ReviewFinalMessageState::Invalid { .. }));
    }

    #[test]
    fn parse_review_final_message_rejects_zero_findings_with_null_locator_findings() {
        // zero_findings + non-empty findings is always rejected (fail-closed),
        // regardless of locator values or message content.
        let payload = parse_review_final_message(Some(
            "{\"verdict\":\"zero_findings\",\"findings\":[{\"message\":\"Schema-level checks passed; running validators...\",\"severity\":null,\"file\":null,\"line\":null}]}",
        ));

        assert!(matches!(payload, ReviewFinalMessageState::Invalid { .. }));
    }

    #[test]
    fn parse_review_final_message_accepts_findings_remain_with_null_locators() {
        // findings_remain with findings is valid, even if locators are null.
        let payload = parse_review_final_message(Some(
            "{\"verdict\":\"findings_remain\",\"findings\":[{\"message\":\"thinking...\",\"severity\":null,\"file\":null,\"line\":null}]}",
        ));

        assert_eq!(
            payload,
            ReviewFinalMessageState::Parsed(ReviewFinalPayload {
                verdict: ReviewPayloadVerdict::FindingsRemain,
                findings: vec![ReviewFinding {
                    message: "thinking...".to_owned(),
                    severity: None,
                    file: None,
                    line: None,
                }],
            })
        );
    }

    #[test]
    fn classify_review_verdict_reports_zero_findings_for_json_payload() {
        let verdict = classify_review_verdict(
            false,
            true,
            &ReviewFinalMessageState::Parsed(ReviewFinalPayload {
                verdict: ReviewPayloadVerdict::ZeroFindings,
                findings: Vec::new(),
            }),
        );

        assert_eq!(verdict, ReviewVerdict::ZeroFindings);
    }

    #[test]
    fn classify_review_verdict_rejects_zero_findings_when_process_failed() {
        let verdict = classify_review_verdict(
            false,
            false,
            &ReviewFinalMessageState::Parsed(ReviewFinalPayload {
                verdict: ReviewPayloadVerdict::ZeroFindings,
                findings: Vec::new(),
            }),
        );

        assert_eq!(verdict, ReviewVerdict::ProcessFailed);
    }

    #[test]
    fn classify_review_verdict_reports_findings_for_json_payload() {
        let verdict = classify_review_verdict(
            false,
            true,
            &ReviewFinalMessageState::Parsed(ReviewFinalPayload {
                verdict: ReviewPayloadVerdict::FindingsRemain,
                findings: vec![ReviewFinding {
                    message: "P1: finding".to_owned(),
                    severity: Some("P1".to_owned()),
                    file: None,
                    line: None,
                }],
            }),
        );

        assert_eq!(verdict, ReviewVerdict::FindingsRemain);
    }

    #[test]
    fn classify_review_verdict_rejects_findings_when_process_failed() {
        let verdict = classify_review_verdict(
            false,
            false,
            &ReviewFinalMessageState::Parsed(ReviewFinalPayload {
                verdict: ReviewPayloadVerdict::FindingsRemain,
                findings: vec![ReviewFinding {
                    message: "P1: finding".to_owned(),
                    severity: Some("P1".to_owned()),
                    file: None,
                    line: None,
                }],
            }),
        );

        assert_eq!(verdict, ReviewVerdict::ProcessFailed);
    }

    #[test]
    fn classify_review_verdict_reports_process_failed_for_invalid_payload() {
        let verdict = classify_review_verdict(
            false,
            true,
            &ReviewFinalMessageState::Invalid { reason: "bad json".to_owned() },
        );

        assert_eq!(verdict, ReviewVerdict::ProcessFailed);
    }

    #[test]
    fn classify_review_verdict_reports_missing_message_only_on_success() {
        assert_eq!(
            classify_review_verdict(false, true, &ReviewFinalMessageState::Missing),
            ReviewVerdict::LastMessageMissing
        );
        assert_eq!(
            classify_review_verdict(false, false, &ReviewFinalMessageState::Missing),
            ReviewVerdict::ProcessFailed
        );
    }

    #[test]
    fn resolve_full_auto_returns_true_for_full_model() {
        let mut profiles = HashMap::new();
        profiles.insert("gpt-5.4".to_owned(), ModelProfile { full_auto: true });
        profiles.insert("gpt-5.3-codex-spark".to_owned(), ModelProfile { full_auto: false });

        assert!(resolve_full_auto("gpt-5.4", Some(&profiles)));
    }

    #[test]
    fn resolve_full_auto_returns_false_for_spark_model() {
        let mut profiles = HashMap::new();
        profiles.insert("gpt-5.4".to_owned(), ModelProfile { full_auto: true });
        profiles.insert("gpt-5.3-codex-spark".to_owned(), ModelProfile { full_auto: false });

        assert!(!resolve_full_auto("gpt-5.3-codex-spark", Some(&profiles)));
    }

    #[test]
    fn resolve_full_auto_returns_true_for_explicit_gpt53_codex_entry() {
        let mut profiles = HashMap::new();
        profiles.insert("gpt-5.3-codex".to_owned(), ModelProfile { full_auto: true });

        assert!(resolve_full_auto("gpt-5.3-codex", Some(&profiles)));
    }

    #[test]
    fn resolve_full_auto_returns_true_for_unknown_model_fail_closed() {
        let mut profiles = HashMap::new();
        profiles.insert("gpt-5.3-codex-spark".to_owned(), ModelProfile { full_auto: false });

        assert!(resolve_full_auto("unknown-model-xyz", Some(&profiles)));
    }

    #[test]
    fn resolve_full_auto_returns_true_when_model_profiles_is_none() {
        assert!(resolve_full_auto("gpt-5.4", None));
        assert!(resolve_full_auto("gpt-5.3-codex-spark", None));
    }

    #[test]
    fn resolve_full_auto_returns_true_when_model_profiles_is_empty() {
        let profiles = HashMap::new();

        assert!(resolve_full_auto("gpt-5.4", Some(&profiles)));
    }

    #[test]
    fn model_profile_defaults_full_auto_to_true_when_omitted() {
        let profile: ModelProfile = serde_json::from_str("{}").unwrap();

        assert!(profile.full_auto, "omitted full_auto should default to true (fail-closed)");
    }
}
