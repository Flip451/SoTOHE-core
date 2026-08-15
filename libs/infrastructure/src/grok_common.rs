//! Shared types for the Grok provider output envelope.

use serde::{Deserialize, Deserializer};
use usecase::capability_exec::CapabilityFailureDetail;

const MISSING_STRUCTURED_OUTPUT: &str = "structured output is missing from the Grok envelope";

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
