//! Shared types for the Grok provider adapter.

use serde::{Deserialize, Deserializer};
use usecase::capability_exec::CapabilityFailureDetail;

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
    value.get("structured_output").or_else(|| value.get("structuredOutput")).cloned()
}

/// Selects the Grok envelope bytes from provider stdout.
///
/// Prefers the last NDJSON line that carries structured output or a terminal
/// failure. Otherwise the whole buffer is treated as one JSON document so a
/// pretty-printed `--output-format json` object is still admitted. Envelope
/// text is not used as a result channel; callers still decode through
/// [`GrokOutputEnvelope`].
pub(crate) fn grok_envelope_bytes_from_stdout(stdout: &[u8]) -> Option<Vec<u8>> {
    let text = std::str::from_utf8(stdout).ok()?;
    let mut last_stream = None;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if is_grok_stream_terminal_envelope(line) {
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

fn is_grok_stream_terminal_envelope(line: &str) -> bool {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
        return false;
    };
    structured_output_value(&value).is_some()
        || value.get("failure_reason").is_some()
        || matches!(
            value.get("type").and_then(serde_json::Value::as_str),
            Some("result" | "error" | "end")
        )
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
}
