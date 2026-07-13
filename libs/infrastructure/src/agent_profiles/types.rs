//! DTOs, validated configuration values, and errors for agent profiles.

use std::collections::HashMap;
use std::path::PathBuf;

use domain::FreeText;
use serde::{Deserialize, Deserializer};
use usecase::capability_exec::{
    CapabilityInputValidationError, ExecutionMode, ModelName, ProviderName, ReasoningEffort,
};
use usecase::dry_write_driver::CapabilityName;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can occur when loading or querying agent profiles.
#[derive(Debug, thiserror::Error)]
pub enum AgentProfilesError {
    /// The configuration file could not be read.
    #[error("failed to read agent profiles at {path}: {source}")]
    Io { path: FreeText, source: std::io::Error },

    /// The configuration file is a symbolic link and cannot be trusted.
    #[error("refusing to load agent profiles through a symlink: {path}")]
    Symlink { path: FreeText },

    /// The configuration path resolves outside its trusted workspace root.
    #[error("agent profiles path {path} escapes trusted root {root}")]
    PathOutsideTrustedRoot { path: FreeText, root: FreeText },

    /// The configuration file contains invalid JSON.
    #[error("failed to parse agent profiles: {0}")]
    Parse(#[from] serde_json::Error),

    /// The configuration file uses an unsupported schema version.
    #[error("unsupported agent profiles schema version {found}; expected {expected}")]
    UnsupportedSchemaVersion { found: u32, expected: u32 },

    /// A capability entry has an invalid configuration (e.g., empty provider name).
    #[error("invalid capability '{capability}': {reason}")]
    InvalidCapability { capability: FreeText, reason: FreeText },

    /// The requested capability does not have a routing row.
    #[error("capability '{0}' is not declared in agent-profiles.json")]
    CapabilityNotFound(CapabilityName),

    /// A provider-CLI capability omitted its required model.
    #[error("capability '{0}' has no model configured")]
    ModelMissing(CapabilityName),

    /// A provider-CLI capability omitted effort for the resolved round.
    #[error("capability '{0}' has no reasoning effort configured for {1:?}")]
    EffortMissing(CapabilityName, RoundType),

    /// A configured effort is not accepted by the selected provider.
    #[error("reasoning effort {1:?} is unsupported by provider '{0}'")]
    UnsupportedEffort(ProviderName, ReasoningEffort),
}

// ---------------------------------------------------------------------------
// RoundType enum
// ---------------------------------------------------------------------------

/// Selects which model tier to resolve for a capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoundType {
    /// Use the primary `model` and `provider`.
    Final,
    /// Use `fast_model` (fallback: `model`) and `fast_provider` (fallback: `provider`).
    Fast,
}

impl std::str::FromStr for RoundType {
    type Err = String;

    /// Parses `"fast"` → `Fast` and `"final"` → `Final`.
    ///
    /// # Errors
    /// Returns an error message for any unrecognised value.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "fast" => Ok(RoundType::Fast),
            "final" => Ok(RoundType::Final),
            other => {
                Err(format!("[ERROR] unknown round type '{other}' (expected 'fast' or 'final')"))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// ResolvedExecution
// ---------------------------------------------------------------------------

/// Fully resolved execution data for a capability and round.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedExecution {
    /// A provider CLI invocation with every execution setting explicit.
    ProviderCli {
        /// Provider that owns the CLI process.
        provider: ProviderName,
        /// Model supplied to the provider CLI.
        model: ModelName,
        /// Reasoning effort supplied to the provider CLI.
        effort: ReasoningEffort,
    },
    /// A hosted-service execution which cannot receive CLI effort settings.
    HostedService {
        /// Provider that owns the hosted service.
        provider: ProviderName,
    },
}

// ---------------------------------------------------------------------------
// Serde DTOs
// ---------------------------------------------------------------------------

/// Minimal envelope to extract `schema_version` before full deserialization.
/// This avoids `deny_unknown_fields` masking future-schema errors as parse errors.
#[derive(Debug, Deserialize)]
pub(super) struct SchemaVersionEnvelope {
    pub(super) schema_version: u32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AgentProfilesDto {
    #[allow(dead_code)]
    schema_version: u32,
    pub(super) providers: HashMap<String, ProviderMetadataDto>,
    pub(super) capabilities: HashMap<String, CapabilityConfigDto>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ProviderMetadataDto {
    #[allow(dead_code)]
    pub(super) label: Option<String>,
}

/// Configuration for a single capability entry.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityConfigDto {
    pub(super) provider: ProviderNameDto,
    pub(super) model: Option<ModelNameDto>,
    pub(super) fast_provider: Option<ProviderNameDto>,
    pub(super) fast_model: Option<ModelNameDto>,
    /// Optional path to the prompt template for this capability.
    pub(super) prompt_template_path: Option<PathBuf>,
    /// Explicit final/default reasoning effort for provider-CLI dispatch.
    #[serde(rename = "reasoning_effort", alias = "final_reasoning_effort")]
    pub(super) effort: Option<ReasoningEffortDto>,
    /// Explicit fast-round reasoning effort for reviewer-like capabilities.
    #[serde(rename = "fast_reasoning_effort")]
    pub(super) fast_effort: Option<ReasoningEffortDto>,
    /// Required dispatch category used by the generic capability-exec entrypoint.
    pub(super) execution_mode: ExecutionModeDto,
}

/// Serde-boundary mirror of a generic capability dispatch execution category.
#[derive(Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExecutionModeDto {
    /// The orchestrator consumes the capability's free-form output.
    OrchestratorOutput,
    /// A dedicated pipeline machine-consumes the capability's output.
    TypedPipeline,
}

impl Copy for ExecutionModeDto {}

impl Clone for ExecutionModeDto {
    fn clone(&self) -> Self {
        *self
    }
}

impl ExecutionModeDto {
    /// Converts this serde DTO into the usecase execution category.
    #[must_use]
    pub fn into_domain(self) -> ExecutionMode {
        match self {
            Self::OrchestratorOutput => ExecutionMode::OrchestratorOutput,
            Self::TypedPipeline => ExecutionMode::TypedPipeline,
        }
    }
}

/// Serde-boundary mirror of a validated capability provider identifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderNameDto(ProviderName);

impl ProviderNameDto {
    /// Validates a provider identifier at the infrastructure boundary.
    ///
    /// # Errors
    ///
    /// Returns the usecase validation error when `value` is blank.
    pub fn try_new(value: String) -> Result<Self, CapabilityInputValidationError> {
        ProviderName::try_new(value).map(Self)
    }

    /// Converts this DTO into its validated usecase value.
    #[must_use]
    pub fn into_domain(self) -> ProviderName {
        self.0
    }
}

impl<'de> Deserialize<'de> for ProviderNameDto {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::try_new(value).map_err(serde::de::Error::custom)
    }
}

/// Serde-boundary mirror of a validated capability model identifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelNameDto(ModelName);

impl ModelNameDto {
    /// Validates a model identifier at the infrastructure boundary.
    ///
    /// # Errors
    ///
    /// Returns the usecase validation error when `value` is blank.
    pub fn try_new(value: String) -> Result<Self, CapabilityInputValidationError> {
        ModelName::try_new(value).map(Self)
    }

    /// Converts this DTO into its validated usecase value.
    #[must_use]
    pub fn into_domain(self) -> ModelName {
        self.0
    }
}

impl<'de> Deserialize<'de> for ModelNameDto {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::try_new(value).map_err(serde::de::Error::custom)
    }
}

/// Serde-boundary vocabulary for a provider-independent reasoning effort.
#[derive(Debug, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReasoningEffortDto {
    /// Lowest supported reasoning effort.
    Low,
    /// Medium reasoning effort.
    Medium,
    /// High reasoning effort.
    High,
    /// Codex's maximum reasoning effort vocabulary.
    #[serde(rename = "xhigh")]
    XHigh,
    /// Claude's maximum reasoning effort vocabulary.
    Max,
}

impl Clone for ReasoningEffortDto {
    fn clone(&self) -> Self {
        *self
    }
}

impl ReasoningEffortDto {
    /// Converts this configuration vocabulary to the usecase value.
    #[must_use]
    pub fn into_domain(self) -> ReasoningEffort {
        match self {
            Self::Low => ReasoningEffort::Low,
            Self::Medium => ReasoningEffort::Medium,
            Self::High => ReasoningEffort::High,
            Self::XHigh => ReasoningEffort::XHigh,
            Self::Max => ReasoningEffort::Max,
        }
    }
}
