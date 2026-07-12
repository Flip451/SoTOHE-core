//! Validated values and error vocabulary for generic capability dispatch.
//!
//! This module deliberately contains no I/O, profile resolution, or provider
//! dispatch. Those concerns are introduced through ports and the interactor in
//! the follow-up task.

use std::path::{Path, PathBuf};

use crate::dry_write_driver::CapabilityName;

/// Validation error for a capability-dispatch input value.
#[derive(Debug, thiserror::Error)]
pub enum CapabilityInputValidationError {
    /// The supplied capability name was empty.
    #[error("capability name must not be empty")]
    EmptyCapabilityName,
    /// The supplied provider name was empty.
    #[error("provider name must not be empty")]
    EmptyProviderName,
    /// The supplied model name was empty.
    #[error("model name must not be empty")]
    EmptyModelName,
    /// The supplied file path was empty.
    #[error("file path must not be empty")]
    EmptyFilePath,
    /// The supplied technical text was empty or whitespace-only.
    #[error("content must not be empty")]
    EmptyContent,
}

/// Validated technical provider identifier for capability dispatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderName(String);

impl ProviderName {
    /// Validates and wraps a provider identifier.
    ///
    /// # Errors
    ///
    /// Returns [`CapabilityInputValidationError::EmptyProviderName`] when
    /// `value` is empty or whitespace-only.
    pub fn try_new(value: impl Into<String>) -> Result<Self, CapabilityInputValidationError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(CapabilityInputValidationError::EmptyProviderName);
        }
        Ok(Self(value))
    }

    /// Returns the validated provider identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ProviderName {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Validated technical model identifier for capability dispatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelName(String);

impl ModelName {
    /// Validates and wraps a model identifier.
    ///
    /// # Errors
    ///
    /// Returns [`CapabilityInputValidationError::EmptyModelName`] when `value`
    /// is empty or whitespace-only.
    pub fn try_new(value: impl Into<String>) -> Result<Self, CapabilityInputValidationError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(CapabilityInputValidationError::EmptyModelName);
        }
        Ok(Self(value))
    }

    /// Returns the validated model identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ModelName {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Validated technical file path for a capability briefing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityFilePath(PathBuf);

impl CapabilityFilePath {
    /// Validates and wraps a briefing-file path.
    ///
    /// # Errors
    ///
    /// Returns [`CapabilityInputValidationError::EmptyFilePath`] when `path`
    /// has no path components.
    pub fn try_new(path: PathBuf) -> Result<Self, CapabilityInputValidationError> {
        if path.as_os_str().is_empty() {
            return Err(CapabilityInputValidationError::EmptyFilePath);
        }
        Ok(Self(path))
    }

    /// Returns the validated briefing-file path.
    #[must_use]
    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

impl std::fmt::Display for CapabilityFilePath {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.as_path().display())
    }
}

/// Validated technical briefing text loaded by a source adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BriefingText(String);

impl BriefingText {
    /// Validates and wraps briefing content.
    ///
    /// # Errors
    ///
    /// Returns [`CapabilityInputValidationError::EmptyContent`] when `value`
    /// is empty or whitespace-only.
    pub fn try_new(value: String) -> Result<Self, CapabilityInputValidationError> {
        if value.trim().is_empty() {
            return Err(CapabilityInputValidationError::EmptyContent);
        }
        Ok(Self(value))
    }

    /// Returns the validated briefing text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Validated technical discipline text loaded by a source adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisciplineText(String);

impl DisciplineText {
    /// Validates and wraps discipline content.
    ///
    /// # Errors
    ///
    /// Returns [`CapabilityInputValidationError::EmptyContent`] when `value`
    /// is empty or whitespace-only.
    pub fn try_new(value: String) -> Result<Self, CapabilityInputValidationError> {
        if value.trim().is_empty() {
            return Err(CapabilityInputValidationError::EmptyContent);
        }
        Ok(Self(value))
    }

    /// Returns the validated discipline text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Opaque diagnostic detail carried by capability-dispatch errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityFailureDetail(String);

impl CapabilityFailureDetail {
    /// Wraps presentation-only diagnostic text.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the diagnostic text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for CapabilityFailureDetail {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Execution category declared by a capability profile.
#[derive(Debug, PartialEq, Eq)]
pub enum ExecutionMode {
    /// The capability returns free-form output for an orchestrator to consume.
    OrchestratorOutput,
    /// The capability has a fixed, machine-consumed output contract.
    TypedPipeline,
}

impl Copy for ExecutionMode {}

impl Clone for ExecutionMode {
    fn clone(&self) -> Self {
        *self
    }
}

/// Request values supplied to generic capability dispatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityExecRequest {
    /// Capability selected from the runtime profile.
    pub capability: CapabilityName,
    /// Provider of the host that invoked this command.
    pub host: ProviderName,
    /// Validated path to the capability briefing.
    pub briefing_file: CapabilityFilePath,
}

/// Typed routing data resolved from the capability profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityProfile {
    /// Provider selected by the profile.
    pub provider: ProviderName,
    /// Single model selected by the profile.
    pub model: ModelName,
    /// Execution category selected by the profile.
    pub execution_mode: ExecutionMode,
}

/// Errors that stop capability dispatch before a successful outcome.
#[derive(Debug, thiserror::Error)]
pub enum CapabilityExecError {
    /// Profile lookup or decoding failed for a capability.
    #[error("capability profile resolution failed for '{capability}': {detail}")]
    ProfileResolution {
        /// Capability whose profile could not be resolved.
        capability: CapabilityName,
        /// Opaque adapter diagnostic.
        detail: CapabilityFailureDetail,
    },
    /// A profile's execution category is not eligible for generic dispatch.
    #[error("execution mode rejected for capability '{capability}': {mode:?}")]
    ExecutionModeRejected {
        /// Capability whose execution category was rejected.
        capability: CapabilityName,
        /// Rejected execution category.
        mode: ExecutionMode,
    },
    /// A profile omitted the single model required for dispatch.
    #[error("capability profile has no model for '{capability}'")]
    ModelMissing {
        /// Capability whose profile omitted its model.
        capability: CapabilityName,
    },
    /// A provider has no supported generic-dispatch adapter.
    #[error("unsupported capability provider '{provider}'")]
    UnsupportedProvider {
        /// Unsupported provider identifier.
        provider: ProviderName,
    },
    /// Briefing or discipline source validation failed.
    #[error("capability source validation failed for '{path}': {detail}")]
    SourceValidation {
        /// Source path associated with the failed validation.
        path: CapabilityFilePath,
        /// Opaque adapter diagnostic.
        detail: CapabilityFailureDetail,
    },
    /// A provider-native adapter did not meet its preflight requirements.
    #[error(
        "capability adapter preflight failed for '{capability}' on provider '{provider}': {detail}"
    )]
    AdapterPreflight {
        /// Capability whose adapter could not pass preflight.
        capability: CapabilityName,
        /// Provider whose adapter failed preflight.
        provider: ProviderName,
        /// Opaque adapter diagnostic.
        detail: CapabilityFailureDetail,
    },
    /// A provider-native dispatch attempt failed.
    #[error("capability dispatch failed for provider '{provider}': {detail}")]
    DispatchFailed {
        /// Provider whose dispatch attempt failed.
        provider: ProviderName,
        /// Opaque adapter diagnostic.
        detail: CapabilityFailureDetail,
    },
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{
        BriefingText, CapabilityExecError, CapabilityExecRequest, CapabilityFailureDetail,
        CapabilityFilePath, CapabilityInputValidationError, CapabilityProfile, DisciplineText,
        ExecutionMode, ModelName, ProviderName,
    };
    use crate::dry_write_driver::CapabilityName;

    #[test]
    fn test_provider_name_whitespace_rejected() {
        assert!(matches!(
            ProviderName::try_new(" \t\n "),
            Err(CapabilityInputValidationError::EmptyProviderName)
        ));
    }

    #[test]
    fn test_provider_name_valid_value_preserved() {
        assert!(matches!(
            ProviderName::try_new("codex"),
            Ok(provider) if provider.as_str() == "codex" && provider.to_string() == "codex"
        ));
    }

    #[test]
    fn test_model_name_whitespace_rejected() {
        assert!(matches!(
            ModelName::try_new(" "),
            Err(CapabilityInputValidationError::EmptyModelName)
        ));
    }

    #[test]
    fn test_model_name_valid_value_preserved() {
        assert!(matches!(
            ModelName::try_new("gpt-5"),
            Ok(model) if model.as_str() == "gpt-5" && model.to_string() == "gpt-5"
        ));
    }

    #[test]
    fn test_capability_file_path_empty_path_rejected() {
        assert!(matches!(
            CapabilityFilePath::try_new(PathBuf::new()),
            Err(CapabilityInputValidationError::EmptyFilePath)
        ));
    }

    #[test]
    fn test_capability_file_path_valid_path_preserved() {
        assert!(matches!(
            CapabilityFilePath::try_new(PathBuf::from("tmp/briefing.md")),
            Ok(path) if path.as_path() == "tmp/briefing.md"
                && path.to_string() == "tmp/briefing.md"
        ));
    }

    #[test]
    fn test_briefing_text_whitespace_rejected() {
        assert!(matches!(
            BriefingText::try_new("\n  \t".to_owned()),
            Err(CapabilityInputValidationError::EmptyContent)
        ));
    }

    #[test]
    fn test_briefing_text_valid_content_preserved() {
        assert!(matches!(
            BriefingText::try_new("Implement T001.".to_owned()),
            Ok(briefing) if briefing.as_str() == "Implement T001."
        ));
    }

    #[test]
    fn test_discipline_text_whitespace_rejected() {
        assert!(matches!(
            DisciplineText::try_new("\n  \t".to_owned()),
            Err(CapabilityInputValidationError::EmptyContent)
        ));
    }

    #[test]
    fn test_discipline_text_valid_content_preserved() {
        assert!(matches!(
            DisciplineText::try_new("Do not stage changes.".to_owned()),
            Ok(discipline) if discipline.as_str() == "Do not stage changes."
        ));
    }

    #[test]
    fn test_capability_profile_valid_values_retained() -> Result<(), Box<dyn std::error::Error>> {
        let profile = CapabilityProfile {
            provider: ProviderName::try_new("codex")?,
            model: ModelName::try_new("gpt-5")?,
            execution_mode: ExecutionMode::OrchestratorOutput,
        };

        assert_eq!(profile.provider.as_str(), "codex");
        assert_eq!(profile.model.as_str(), "gpt-5");
        assert_eq!(profile.execution_mode, ExecutionMode::OrchestratorOutput);
        Ok(())
    }

    #[test]
    fn test_capability_exec_request_shared_capability_name_retained()
    -> Result<(), Box<dyn std::error::Error>> {
        let request = CapabilityExecRequest {
            capability: CapabilityName::try_new("implementer")?,
            host: ProviderName::try_new("codex")?,
            briefing_file: CapabilityFilePath::try_new(PathBuf::from("tmp/briefing.md"))?,
        };

        assert_eq!(request.capability.as_str(), "implementer");
        assert_eq!(request.host.as_str(), "codex");
        assert_eq!(request.briefing_file.as_path(), PathBuf::from("tmp/briefing.md"));
        Ok(())
    }

    #[test]
    fn test_execution_mode_distinct_variants_retained() {
        assert_ne!(ExecutionMode::OrchestratorOutput, ExecutionMode::TypedPipeline);
    }

    #[test]
    fn test_capability_failure_detail_input_preserved() {
        let detail = CapabilityFailureDetail::new("definition is missing");

        assert_eq!(detail.as_str(), "definition is missing");
        assert_eq!(detail.to_string(), "definition is missing");
    }

    #[test]
    fn test_capability_exec_error_all_variants_render_diagnostics()
    -> Result<(), Box<dyn std::error::Error>> {
        let detail = CapabilityFailureDetail::new("definition is missing");
        let variants = [
            CapabilityExecError::ProfileResolution {
                capability: CapabilityName::try_new("implementer")?,
                detail: detail.clone(),
            },
            CapabilityExecError::ExecutionModeRejected {
                capability: CapabilityName::try_new("implementer")?,
                mode: ExecutionMode::TypedPipeline,
            },
            CapabilityExecError::ModelMissing {
                capability: CapabilityName::try_new("implementer")?,
            },
            CapabilityExecError::UnsupportedProvider { provider: ProviderName::try_new("codex")? },
            CapabilityExecError::SourceValidation {
                path: CapabilityFilePath::try_new(PathBuf::from("tmp/briefing.md"))?,
                detail: detail.clone(),
            },
            CapabilityExecError::AdapterPreflight {
                capability: CapabilityName::try_new("implementer")?,
                provider: ProviderName::try_new("codex")?,
                detail: detail.clone(),
            },
            CapabilityExecError::DispatchFailed {
                provider: ProviderName::try_new("codex")?,
                detail,
            },
        ];

        for error in variants {
            assert!(!error.to_string().is_empty());
        }
        Ok(())
    }
}
