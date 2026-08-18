//! Shared types for the Grok provider adapter.

use std::io::{BufRead, BufReader, Read};

use serde::{Deserialize, Deserializer};
use usecase::capability_exec::CapabilityFailureDetail;

/// Per-event retention for Grok NDJSON while draining a verbose stdout stream.
const MAX_GROK_STREAM_EVENT_BYTES: usize = 256 * 1024;

const MISSING_STRUCTURED_OUTPUT: &str = "structured output is missing from the Grok envelope";

/// A validated Grok project-profile name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrokSandboxProfileName(String);

impl GrokSandboxProfileName {
    /// Creates a validated Grok project-profile name.
    ///
    /// # Errors
    ///
    /// Returns [`GrokSandboxProfileNameError::Empty`] for an empty or
    /// whitespace-only name and [`GrokSandboxProfileNameError::Reserved`] for
    /// unrestricted, built-in, or Codex-specific sandbox values.
    pub fn try_new(value: String) -> Result<Self, GrokSandboxProfileNameError> {
        if value.trim().is_empty() {
            return Err(GrokSandboxProfileNameError::Empty);
        }
        if is_reserved_sandbox_value(&value) {
            return Err(GrokSandboxProfileNameError::Reserved);
        }
        Ok(Self(value))
    }

    /// Returns the profile name as declared for Grok.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Values that cannot be interpreted as Grok project-profile names.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GrokSandboxProfileNameError {
    /// The profile name is empty or whitespace-only.
    #[error("Grok sandbox profile name must not be empty")]
    Empty,
    /// The value is reserved for unrestricted, built-in, or another provider's sandbox.
    #[error("Grok sandbox profile name uses a reserved sandbox value")]
    Reserved,
}

fn is_reserved_sandbox_value(value: &str) -> bool {
    matches!(
        value,
        "read-only"
            | "workspace"
            | "strict"
            | "off"
            | "devbox"
            | "workspace-write"
            | "danger-full-access"
            | "dangerously-bypass-approvals-and-sandbox"
    )
}

impl<'de> Deserialize<'de> for GrokSandboxProfileName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::try_new(value).map_err(serde::de::Error::custom)
    }
}

/// A sandbox permission accepted by the Grok CLI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GrokSandbox {
    /// Permit read-only access.
    ReadOnly,
    /// Permit workspace access.
    Workspace,
    /// Apply Grok's strict sandbox.
    Strict,
    /// Use a named Grok project profile.
    ProjectProfile(GrokSandboxProfileName),
}

impl<'de> Deserialize<'de> for GrokSandbox {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        match value.as_str() {
            "read-only" => Ok(Self::ReadOnly),
            "workspace" => Ok(Self::Workspace),
            "strict" => Ok(Self::Strict),
            _ => GrokSandboxProfileName::try_new(value)
                .map(Self::ProjectProfile)
                .map_err(serde::de::Error::custom),
        }
    }
}

/// Output returned by the Grok provider's JSON envelope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GrokOutputEnvelope {
    /// The provider returned the schema-owned structured value.
    Succeeded {
        /// Opaque JSON selected by the requested output schema.
        structured_output: serde_json::Value,
    },
    /// The provider did not return a structured value.
    Failed {
        /// Diagnostic detail supplied by the provider or the envelope boundary.
        failure_reason: CapabilityFailureDetail,
    },
}

impl<'de> Deserialize<'de> for GrokOutputEnvelope {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        if let Some(structured_output) = structured_output_value(&value) {
            return Ok(Self::Succeeded { structured_output });
        }
        Ok(Self::Failed { failure_reason: failure_detail_from_envelope(&value) })
    }
}

fn structured_output_value(value: &serde_json::Value) -> Option<serde_json::Value> {
    value
        .get("structured_output")
        .or_else(|| value.get("structuredOutput"))
        .filter(|candidate| !candidate.is_null())
        .cloned()
}

/// Session metadata can arrive on an earlier NDJSON event than the selected
/// structured-result envelope. Scan the full stdout independently.
#[must_use]
pub(crate) fn session_id_from_grok_stdout(stdout: &[u8]) -> Option<String> {
    let mut session_id = None;
    for line in stdout.split(|byte| *byte == b'\n') {
        if session_id.is_none() {
            session_id = extract_grok_session_id(line);
        }
    }
    session_id.or_else(|| extract_grok_session_id(stdout))
}

fn extract_grok_session_id(event: &[u8]) -> Option<String> {
    let event = std::str::from_utf8(event).ok()?;
    let value = serde_json::from_str::<serde_json::Value>(event.trim()).ok()?;
    value
        .get("session_id")
        .or_else(|| value.get("thread_id"))
        .or_else(|| value.get("sessionId"))?
        .as_str()
        .filter(|id| !id.trim().is_empty())
        .map(str::to_owned)
}

/// Selects the Grok envelope bytes from provider stdout.
///
/// Prefers the last NDJSON line that carries structured output. A later
/// failure line may replace that result; a later `type=end` (or any other
/// event without structured output or a failure) must not. Otherwise the
/// whole buffer is treated as one JSON document so a pretty-printed
/// `--output-format json` object is still admitted. Envelope text is not
/// used as a result channel; callers still decode through
/// [`GrokOutputEnvelope`].
pub(crate) fn grok_envelope_bytes_from_stdout(stdout: &[u8]) -> Option<Vec<u8>> {
    let text = std::str::from_utf8(stdout).ok()?;
    let mut last_stream = None;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if is_grok_stream_result_or_failure(line) {
            last_stream = Some(line.as_bytes().to_vec());
        }
    }
    if last_stream.is_some() {
        return last_stream;
    }
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    serde_json::from_str::<serde_json::Value>(trimmed)
        .ok()
        .filter(serde_json::Value::is_object)
        .map(|_| trimmed.as_bytes().to_vec())
}

fn is_grok_stream_result_or_failure(line: &str) -> bool {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
        return false;
    };
    structured_output_value(&value).is_some()
        || value.get("failure_reason").is_some_and(|reason| !reason.is_null())
        || value.get("type").and_then(serde_json::Value::as_str) == Some("error")
}

/// Session metadata and the last structured-result or failure envelope retained
/// while draining Grok stdout. Cumulative volume is not a failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GrokStreamedStdout {
    pub(crate) session_id: Option<String>,
    pub(crate) envelope_bytes: Option<Vec<u8>>,
}

/// Drains Grok stdout while keeping only session metadata and a bounded final
/// envelope candidate. Earlier tool/text events are not retained.
///
/// # Errors
///
/// Returns an I/O error when reading the pipe fails.
pub(crate) fn collect_grok_stdout_stream<R: Read>(
    pipe: R,
) -> Result<GrokStreamedStdout, std::io::Error> {
    let mut reader = BufReader::new(pipe);
    let mut session_id = None;
    let mut envelope_bytes = None;
    let mut document = Vec::new();
    let mut event = Vec::new();
    let mut discarding_event = false;
    let mut document_overflow = false;

    loop {
        let buffer = reader.fill_buf()?;
        if buffer.is_empty() {
            break;
        }
        let newline = buffer.iter().position(|byte| *byte == b'\n');
        let event_bytes = newline.unwrap_or(buffer.len());
        let consumed = newline.map_or(buffer.len(), |index| index.saturating_add(1));
        if !discarding_event {
            let remaining = MAX_GROK_STREAM_EVENT_BYTES.saturating_sub(event.len());
            if event_bytes <= remaining {
                if let Some(part) = buffer.get(..event_bytes) {
                    event.extend_from_slice(part);
                }
            } else {
                discarding_event = true;
            }
        }
        reader.consume(consumed);
        if newline.is_some() {
            admit_grok_stream_event(
                &event,
                discarding_event,
                &mut session_id,
                &mut envelope_bytes,
                &mut document,
                &mut document_overflow,
            );
            event.clear();
            discarding_event = false;
        }
    }
    admit_grok_stream_event(
        &event,
        discarding_event,
        &mut session_id,
        &mut envelope_bytes,
        &mut document,
        &mut document_overflow,
    );
    if envelope_bytes.is_none() && !document_overflow {
        if session_id.is_none() {
            session_id = extract_grok_session_id(&document);
        }
        envelope_bytes = pretty_printed_grok_envelope(&document);
    }
    Ok(GrokStreamedStdout { session_id, envelope_bytes })
}

fn admit_grok_stream_event(
    event: &[u8],
    discarding_event: bool,
    session_id: &mut Option<String>,
    envelope_bytes: &mut Option<Vec<u8>>,
    document: &mut Vec<u8>,
    document_overflow: &mut bool,
) {
    if discarding_event {
        *document_overflow = true;
        return;
    }
    if event.is_empty() {
        return;
    }
    if session_id.is_none() {
        *session_id = extract_grok_session_id(event);
    }
    if let Ok(line) = std::str::from_utf8(event) {
        let line = line.trim();
        if is_grok_stream_result_or_failure(line) {
            *envelope_bytes = Some(line.as_bytes().to_vec());
            return;
        }
    }
    if envelope_bytes.is_some() || *document_overflow {
        return;
    }
    let extra = if document.is_empty() { 0 } else { 1 };
    if document.len().saturating_add(extra).saturating_add(event.len())
        > MAX_GROK_STREAM_EVENT_BYTES
    {
        *document_overflow = true;
        document.clear();
        return;
    }
    if extra == 1 {
        document.push(b'\n');
    }
    document.extend_from_slice(event);
}

fn pretty_printed_grok_envelope(event: &[u8]) -> Option<Vec<u8>> {
    let text = std::str::from_utf8(event).ok()?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    serde_json::from_str::<serde_json::Value>(trimmed)
        .ok()
        .filter(serde_json::Value::is_object)
        .map(|_| trimmed.as_bytes().to_vec())
}

fn missing_structured_output_failure() -> CapabilityFailureDetail {
    CapabilityFailureDetail::new(MISSING_STRUCTURED_OUTPUT)
}

fn failure_detail_from_envelope(value: &serde_json::Value) -> CapabilityFailureDetail {
    ["failure_reason", "error", "message"]
        .into_iter()
        .find_map(|key| value.get(key).and_then(serde_json::Value::as_str))
        .map(CapabilityFailureDetail::new)
        .unwrap_or_else(missing_structured_output_failure)
}

impl GrokOutputEnvelope {
    /// Extracts the envelope's structured output.
    ///
    /// # Errors
    ///
    /// Returns [`GrokEnvelopeError::ProviderFailure`] when the envelope did not
    /// contain structured output. Envelope text is never used as a fallback.
    pub fn into_structured_output(self) -> Result<serde_json::Value, GrokEnvelopeError> {
        match self {
            Self::Succeeded { structured_output } => Ok(structured_output),
            Self::Failed { failure_reason } => {
                Err(GrokEnvelopeError::ProviderFailure { failure_reason })
            }
        }
    }
}

/// Error produced when a Grok envelope cannot yield structured output.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GrokEnvelopeError {
    /// The provider returned a failure envelope instead of structured output.
    #[error("Grok provider failed: {failure_reason}")]
    ProviderFailure {
        /// Diagnostic detail supplied by the provider or the envelope boundary.
        failure_reason: CapabilityFailureDetail,
    },
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_grok_sandbox_read_only_value_deserializes() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(serde_json::from_str::<GrokSandbox>(r#""read-only""#)?, GrokSandbox::ReadOnly,);
        Ok(())
    }

    #[test]
    fn test_grok_sandbox_workspace_value_deserializes() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(serde_json::from_str::<GrokSandbox>(r#""workspace""#)?, GrokSandbox::Workspace,);
        Ok(())
    }

    #[test]
    fn test_grok_sandbox_strict_value_deserializes() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(serde_json::from_str::<GrokSandbox>(r#""strict""#)?, GrokSandbox::Strict,);
        Ok(())
    }

    #[test]
    fn test_grok_sandbox_project_profile_value_deserializes()
    -> Result<(), Box<dyn std::error::Error>> {
        let sandbox = serde_json::from_str::<GrokSandbox>(r#""repository-profile""#)?;

        assert_eq!(
            sandbox,
            GrokSandbox::ProjectProfile(GrokSandboxProfileName::try_new(
                "repository-profile".to_owned(),
            )?),
        );
        Ok(())
    }

    #[test]
    fn test_grok_sandbox_profile_name_empty_value_is_rejected() {
        assert_eq!(
            GrokSandboxProfileName::try_new(String::new()),
            Err(GrokSandboxProfileNameError::Empty),
        );
    }

    #[test]
    fn test_grok_sandbox_profile_name_off_value_is_rejected() {
        assert_eq!(
            GrokSandboxProfileName::try_new("off".to_owned()),
            Err(GrokSandboxProfileNameError::Reserved),
        );
    }

    #[test]
    fn test_grok_sandbox_profile_name_devbox_value_is_rejected() {
        assert_eq!(
            GrokSandboxProfileName::try_new("devbox".to_owned()),
            Err(GrokSandboxProfileNameError::Reserved),
        );
    }

    #[test]
    fn test_grok_sandbox_profile_name_builtin_values_are_rejected() {
        for value in ["read-only", "workspace", "strict"] {
            assert_eq!(
                GrokSandboxProfileName::try_new(value.to_owned()),
                Err(GrokSandboxProfileNameError::Reserved),
            );
        }
    }

    #[test]
    fn test_grok_sandbox_off_value_is_rejected() {
        assert!(serde_json::from_str::<GrokSandbox>(r#""off""#).is_err());
    }

    #[test]
    fn test_grok_sandbox_codex_workspace_write_value_is_rejected() {
        assert!(serde_json::from_str::<GrokSandbox>(r#""workspace-write""#).is_err());
    }

    #[test]
    fn test_grok_sandbox_devbox_value_is_rejected() {
        assert!(serde_json::from_str::<GrokSandbox>(r#""devbox""#).is_err());
    }

    #[test]
    fn test_grok_output_envelope_success_structured_output_returns_value()
    -> Result<(), Box<dyn std::error::Error>> {
        let envelope: GrokOutputEnvelope = serde_json::from_str(
            r#"{"structured_output":{"verdict":"zero_findings","findings":[]}}"#,
        )?;

        assert_eq!(
            envelope.into_structured_output()?,
            serde_json::json!({"verdict": "zero_findings", "findings": []}),
        );
        Ok(())
    }

    #[test]
    fn test_grok_output_envelope_missing_structured_output_returns_provider_failure()
    -> Result<(), Box<dyn std::error::Error>> {
        let envelope: GrokOutputEnvelope =
            serde_json::from_str(r#"{"failure_reason":"provider timed out"}"#)?;

        assert_eq!(
            envelope.into_structured_output(),
            Err(GrokEnvelopeError::ProviderFailure {
                failure_reason: CapabilityFailureDetail::new("provider timed out"),
            }),
        );
        Ok(())
    }

    #[test]
    fn test_grok_output_envelope_error_field_is_used_as_failure_reason()
    -> Result<(), Box<dyn std::error::Error>> {
        let envelope: GrokOutputEnvelope =
            serde_json::from_str(r#"{"error":"provider timed out"}"#)?;

        assert_eq!(
            envelope.into_structured_output(),
            Err(GrokEnvelopeError::ProviderFailure {
                failure_reason: CapabilityFailureDetail::new("provider timed out"),
            }),
        );
        Ok(())
    }

    #[test]
    fn test_grok_output_envelope_typed_error_message_is_used_as_failure_reason()
    -> Result<(), Box<dyn std::error::Error>> {
        let envelope: GrokOutputEnvelope =
            serde_json::from_str(r#"{"type":"error","message":"authentication failed"}"#)?;

        assert_eq!(
            envelope.into_structured_output(),
            Err(GrokEnvelopeError::ProviderFailure {
                failure_reason: CapabilityFailureDetail::new("authentication failed"),
            }),
        );
        Ok(())
    }

    #[test]
    fn test_grok_output_envelope_failure_reason_precedes_error_and_message()
    -> Result<(), Box<dyn std::error::Error>> {
        let envelope: GrokOutputEnvelope = serde_json::from_str(
            r#"{"failure_reason":"canonical","error":"ignored-error","message":"ignored-message"}"#,
        )?;

        assert_eq!(
            envelope.into_structured_output(),
            Err(GrokEnvelopeError::ProviderFailure {
                failure_reason: CapabilityFailureDetail::new("canonical"),
            }),
        );
        Ok(())
    }

    #[test]
    fn test_grok_output_envelope_text_without_structured_output_is_rejected()
    -> Result<(), Box<dyn std::error::Error>> {
        let envelope: GrokOutputEnvelope =
            serde_json::from_str(r#"{"text":"do not use this as the result"}"#)?;

        assert_eq!(
            envelope.into_structured_output(),
            Err(GrokEnvelopeError::ProviderFailure {
                failure_reason: CapabilityFailureDetail::new(MISSING_STRUCTURED_OUTPUT),
            }),
        );
        Ok(())
    }

    #[test]
    fn test_grok_output_envelope_camel_case_structured_output_returns_value()
    -> Result<(), Box<dyn std::error::Error>> {
        let envelope: GrokOutputEnvelope =
            serde_json::from_str(r#"{"structuredOutput":{"result":"OK"},"text":"ignore-me"}"#)?;

        assert_eq!(envelope.into_structured_output()?, serde_json::json!({"result": "OK"}),);
        Ok(())
    }

    #[test]
    fn test_grok_envelope_bytes_from_pretty_printed_json_document() {
        let stdout = r#"{
  "text": "{\"result\": \"OK\"}",
  "sessionId": "grok-session",
  "structuredOutput": {
    "result": "OK"
  }
}
"#;
        let bytes = grok_envelope_bytes_from_stdout(stdout.as_bytes()).expect("envelope");
        let envelope: GrokOutputEnvelope =
            serde_json::from_slice(&bytes).expect("pretty-printed envelope decodes");
        assert_eq!(
            envelope.into_structured_output().expect("structured output"),
            serde_json::json!({"result": "OK"}),
        );
    }

    #[test]
    fn test_session_id_from_grok_stdout_reads_earlier_ndjson_event() {
        let stdout = concat!(
            "{\"type\":\"start\",\"sessionId\":\"early-session\"}\n",
            "{\"structured_output\":{\"status\":\"ok\"}}\n",
        );
        assert_eq!(
            session_id_from_grok_stdout(stdout.as_bytes()).as_deref(),
            Some("early-session"),
        );
    }

    #[test]
    fn test_grok_envelope_bytes_from_ndjson_prefers_last_structured_output() {
        let stdout = concat!(
            r#"{"type":"text","data":"ignore"}"#,
            "\n",
            r#"{"type":"result","structured_output":{"result":"first"}}"#,
            "\n",
            r#"{"type":"result","structuredOutput":{"result":"second"}}"#,
            "\n",
        );
        let bytes = grok_envelope_bytes_from_stdout(stdout.as_bytes()).expect("envelope");
        assert_eq!(
            bytes.as_slice(),
            br#"{"type":"result","structuredOutput":{"result":"second"}}"#,
        );
    }

    #[test]
    fn test_grok_envelope_bytes_from_ndjson_keeps_structured_output_after_end_event() {
        let stdout = concat!(
            r#"{"type":"result","structured_output":{"result":"keep"}}"#,
            "\n",
            r#"{"type":"end"}"#,
            "\n",
        );
        let bytes = grok_envelope_bytes_from_stdout(stdout.as_bytes()).expect("envelope");
        assert_eq!(bytes.as_slice(), br#"{"type":"result","structured_output":{"result":"keep"}}"#,);
    }

    #[test]
    fn test_grok_envelope_bytes_from_ndjson_ignores_null_failure_reason_on_end_event() {
        let stdout = concat!(
            r#"{"type":"result","structured_output":{"result":"keep"}}"#,
            "\n",
            r#"{"type":"end","failure_reason":null}"#,
            "\n",
        );
        let bytes = grok_envelope_bytes_from_stdout(stdout.as_bytes()).expect("envelope");
        assert_eq!(bytes.as_slice(), br#"{"type":"result","structured_output":{"result":"keep"}}"#,);
    }

    #[test]
    fn test_grok_envelope_bytes_from_ndjson_ignores_null_structured_output_after_result() {
        let stdout = concat!(
            r#"{"type":"result","structured_output":{"result":"keep"}}"#,
            "\n",
            r#"{"type":"end","structured_output":null}"#,
            "\n",
        );
        let bytes = grok_envelope_bytes_from_stdout(stdout.as_bytes()).expect("envelope");
        assert_eq!(bytes.as_slice(), br#"{"type":"result","structured_output":{"result":"keep"}}"#,);
    }

    #[test]
    fn test_grok_envelope_bytes_from_ndjson_later_failure_reason_replaces_result() {
        let stdout = concat!(
            r#"{"type":"result","structured_output":{"result":"first"}}"#,
            "\n",
            r#"{"failure_reason":"provider timed out"}"#,
            "\n",
        );
        let bytes = grok_envelope_bytes_from_stdout(stdout.as_bytes()).expect("envelope");
        assert_eq!(bytes.as_slice(), br#"{"failure_reason":"provider timed out"}"#,);
    }

    #[test]
    fn test_grok_output_envelope_null_structured_output_is_provider_failure()
    -> Result<(), Box<dyn std::error::Error>> {
        let envelope: GrokOutputEnvelope = serde_json::from_str(r#"{"structured_output":null}"#)?;

        assert_eq!(
            envelope.into_structured_output(),
            Err(GrokEnvelopeError::ProviderFailure {
                failure_reason: CapabilityFailureDetail::new(MISSING_STRUCTURED_OUTPUT),
            }),
        );
        Ok(())
    }

    #[test]
    fn test_collect_grok_stdout_stream_keeps_early_session_and_late_envelope()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut stdout = b"{\"type\":\"start\",\"sessionId\":\"early-session\"}\n".to_vec();
        stdout.extend(std::iter::repeat_n(b'x', MAX_GROK_STREAM_EVENT_BYTES.saturating_add(64)));
        stdout.extend_from_slice(b"\n{\"structured_output\":{\"result\":\"ok\"}}\n");

        let collected = collect_grok_stdout_stream(stdout.as_slice())?;
        assert_eq!(collected.session_id.as_deref(), Some("early-session"));
        assert_eq!(
            collected.envelope_bytes.as_deref(),
            Some(br#"{"structured_output":{"result":"ok"}}"#.as_slice()),
        );
        Ok(())
    }

    #[test]
    fn test_collect_grok_stdout_stream_keeps_pretty_printed_envelope()
    -> Result<(), Box<dyn std::error::Error>> {
        let stdout = br#"{
  "sessionId": "pretty-session",
  "structuredOutput": {
    "result": "OK"
  }
}"#;
        let collected = collect_grok_stdout_stream(stdout.as_slice())?;
        assert_eq!(collected.session_id.as_deref(), Some("pretty-session"));
        let envelope: GrokOutputEnvelope =
            serde_json::from_slice(collected.envelope_bytes.as_deref().ok_or("envelope")?)?;
        assert_eq!(envelope.into_structured_output()?, serde_json::json!({"result": "OK"}));
        Ok(())
    }
}
