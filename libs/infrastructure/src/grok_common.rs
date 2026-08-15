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
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum GrokOutputEnvelope {
    /// The provider returned the schema-owned structured value.
    Succeeded {
        /// Opaque JSON selected by the requested output schema.
        structured_output: serde_json::Value,
    },
    /// The provider did not return a structured value.
    Failed {
        /// Diagnostic detail supplied by the provider or the envelope boundary.
        #[serde(
            default = "missing_structured_output_failure",
            deserialize_with = "deserialize_failure_reason"
        )]
        failure_reason: CapabilityFailureDetail,
    },
}

fn missing_structured_output_failure() -> CapabilityFailureDetail {
    CapabilityFailureDetail::new(MISSING_STRUCTURED_OUTPUT)
}

fn deserialize_failure_reason<'de, D>(deserializer: D) -> Result<CapabilityFailureDetail, D::Error>
where
    D: Deserializer<'de>,
{
    String::deserialize(deserializer).map(CapabilityFailureDetail::new)
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
}
