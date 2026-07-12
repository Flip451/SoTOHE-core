//! I/O for `.harness/config/agent-profiles.json` — capability-centric agent routing.
//!
//! Reads the v2 schema (`schema_version: 1`) where each capability directly
//! specifies its provider and model. Resolution follows:
//! - `RoundType::Final` → `(provider, model)`
//! - `RoundType::Fast`  → `(fast_provider ?? provider, fast_model ?? model)`

use std::collections::HashMap;
use std::path::Path;

use domain::FreeText;
use serde::{Deserialize, Deserializer};
use usecase::capability_exec::{
    CapabilityInputValidationError, ExecutionMode, ModelName, ProviderName,
};

use crate::capability_exec::{
    bounded_read_utf8_file,
    path_guard::{lexically_normalize, normalize_path_rejecting_symlinked_components},
};

/// Default path for the agent profiles configuration file.
pub const AGENT_PROFILES_PATH: &str = ".harness/config/agent-profiles.json";

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

/// Fully resolved provider + model pair for a specific round.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedExecution {
    /// The provider to use (e.g., "claude", "codex", "gemini").
    pub provider: String,
    /// The model to use, if specified. `None` when the provider needs no model
    /// (e.g., Gemini CLI).
    pub model: Option<String>,
}

// ---------------------------------------------------------------------------
// Serde DTOs
// ---------------------------------------------------------------------------

/// Minimal envelope to extract `schema_version` before full deserialization.
/// This avoids `deny_unknown_fields` masking future-schema errors as parse errors.
#[derive(Debug, Deserialize)]
struct SchemaVersionEnvelope {
    schema_version: u32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AgentProfilesDto {
    #[allow(dead_code)]
    schema_version: u32,
    providers: HashMap<String, ProviderMetadataDto>,
    capabilities: HashMap<String, CapabilityConfigDto>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProviderMetadataDto {
    #[allow(dead_code)]
    label: Option<String>,
}

/// Configuration for a single capability entry.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityConfigDto {
    provider: ProviderNameDto,
    model: Option<ModelNameDto>,
    fast_provider: Option<String>,
    fast_model: Option<String>,
    /// Optional path to the prompt template for this capability.
    prompt_template_path: Option<String>,
    /// Optional reasoning effort for the fast-round dry-checker (D4 / IN-08).
    /// `None` means no override; the composition root supplies a built-in default.
    fast_reasoning_effort: Option<String>,
    /// Optional reasoning effort for the final-round dry-checker (D4 / IN-08).
    /// `None` means no override; the composition root supplies a built-in default.
    final_reasoning_effort: Option<String>,
    /// Required dispatch category used by the generic capability-exec entrypoint.
    execution_mode: ExecutionModeDto,
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

    fn as_str(&self) -> &str {
        self.0.as_str()
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

    fn as_str(&self) -> &str {
        self.0.as_str()
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

// ---------------------------------------------------------------------------
// AgentProfiles (public API)
// ---------------------------------------------------------------------------

/// Loaded agent profiles configuration.
///
/// Provides resolution of capability → (provider, model) pairs.
#[derive(Debug)]
pub struct AgentProfiles {
    providers: HashMap<String, ProviderMetadataDto>,
    capabilities: HashMap<String, CapabilityConfigDto>,
}

impl AgentProfiles {
    /// Loads agent profiles from a JSON file within a trusted workspace root.
    ///
    /// # Errors
    ///
    /// Returns [`AgentProfilesError::PathOutsideTrustedRoot`] if `path` does
    /// not remain inside `trusted_root`, [`AgentProfilesError::Io`] if the
    /// file cannot be read, or [`AgentProfilesError::Parse`] if the JSON is
    /// invalid.
    pub fn load(trusted_root: &Path, path: &Path) -> Result<Self, AgentProfilesError> {
        const SUPPORTED_SCHEMA_VERSION: u32 = 1;

        let trusted_root = lexically_normalize(trusted_root);
        let normalized_path = normalize_path_rejecting_symlinked_components(path, &trusted_root)
            .map_err(|_| AgentProfilesError::Symlink {
                path: FreeText::new(path.display().to_string()),
            })?;
        if !normalized_path.starts_with(&trusted_root) {
            return Err(AgentProfilesError::PathOutsideTrustedRoot {
                path: FreeText::new(normalized_path.display().to_string()),
                root: FreeText::new(trusted_root.display().to_string()),
            });
        }
        let canonical_root =
            trusted_root.canonicalize().map_err(|source| AgentProfilesError::Io {
                path: FreeText::new(trusted_root.display().to_string()),
                source,
            })?;
        let canonical_path = normalized_path.canonicalize().map_err(|source| {
            AgentProfilesError::Io { path: FreeText::new(path.display().to_string()), source }
        })?;
        if !canonical_path.starts_with(&canonical_root) {
            return Err(AgentProfilesError::PathOutsideTrustedRoot {
                path: FreeText::new(path.display().to_string()),
                root: FreeText::new(canonical_root.display().to_string()),
            });
        }
        let content = bounded_read_utf8_file(&normalized_path).map_err(|e| {
            AgentProfilesError::Io { path: FreeText::new(path.display().to_string()), source: e }
        })?;
        // Parse schema_version first (without deny_unknown_fields) so future
        // schema versions produce UnsupportedSchemaVersion, not a Parse error.
        let envelope: SchemaVersionEnvelope = serde_json::from_str(&content)?;
        if envelope.schema_version != SUPPORTED_SCHEMA_VERSION {
            return Err(AgentProfilesError::UnsupportedSchemaVersion {
                found: envelope.schema_version,
                expected: SUPPORTED_SCHEMA_VERSION,
            });
        }
        let dto: AgentProfilesDto = serde_json::from_str(&content)?;
        // Validate and normalize capability entries.
        let mut capabilities = dto.capabilities;
        for (name, config) in &mut capabilities {
            // Reject empty provider / fast_provider.
            if config.provider.as_str().trim().is_empty() {
                return Err(AgentProfilesError::InvalidCapability {
                    capability: FreeText::new(name.clone()),
                    reason: FreeText::new("provider must not be empty"),
                });
            }
            if let Some(fp) = &config.fast_provider {
                if fp.trim().is_empty() {
                    return Err(AgentProfilesError::InvalidCapability {
                        capability: FreeText::new(name.clone()),
                        reason: FreeText::new("fast_provider must not be empty when specified"),
                    });
                }
            }
            // Reject empty model/fast_model strings (must be non-empty when specified).
            if config.model.as_ref().is_some_and(|model| model.as_str().trim().is_empty()) {
                return Err(AgentProfilesError::InvalidCapability {
                    capability: FreeText::new(name.clone()),
                    reason: FreeText::new("model must not be empty when specified"),
                });
            }
            if config.fast_model.as_deref().is_some_and(|s| s.trim().is_empty()) {
                return Err(AgentProfilesError::InvalidCapability {
                    capability: FreeText::new(name.clone()),
                    reason: FreeText::new("fast_model must not be empty when specified"),
                });
            }
        }
        Ok(Self { providers: dto.providers, capabilities })
    }

    /// Returns the raw capability configuration for the given capability name.
    #[must_use]
    pub fn resolve_capability(&self, capability: &str) -> Option<&CapabilityConfigDto> {
        self.capabilities.get(capability)
    }

    /// Resolves the (provider, model) pair for a capability and round type.
    ///
    /// Resolution rules:
    /// - `Final`: `(config.provider, config.model)`
    /// - `Fast`: `(config.fast_provider ?? config.provider, config.fast_model ?? config.model)`
    ///
    /// Returns `None` if the capability is not defined.
    #[must_use]
    pub fn resolve_execution(
        &self,
        capability: &str,
        round_type: RoundType,
    ) -> Option<ResolvedExecution> {
        let config = self.capabilities.get(capability)?;
        match round_type {
            RoundType::Final => Some(ResolvedExecution {
                provider: config.provider.as_str().to_owned(),
                model: config.model.as_ref().map(|model| model.as_str().to_owned()),
            }),
            RoundType::Fast => Some(ResolvedExecution {
                provider: config
                    .fast_provider
                    .clone()
                    .unwrap_or_else(|| config.provider.as_str().to_owned()),
                model: Some(
                    config
                        .fast_model
                        .clone()
                        .or_else(|| config.model.as_ref().map(|model| model.as_str().to_owned()))
                        .unwrap_or_default(),
                )
                .filter(|s| !s.is_empty()),
            }),
        }
    }

    /// Shortcut: resolve just the model name for a capability and round type.
    #[must_use]
    pub fn resolve_model(&self, capability: &str, round_type: RoundType) -> Option<&str> {
        let config = self.capabilities.get(capability)?;
        match round_type {
            RoundType::Final => config.model.as_ref().map(ModelNameDto::as_str),
            RoundType::Fast => config
                .fast_model
                .as_deref()
                .or_else(|| config.model.as_ref().map(ModelNameDto::as_str)),
        }
    }

    /// Shortcut: resolve just the provider name for a capability and round type.
    #[must_use]
    pub fn resolve_provider(&self, capability: &str, round_type: RoundType) -> Option<&str> {
        let config = self.capabilities.get(capability)?;
        Some(match round_type {
            RoundType::Final => config.provider.as_str(),
            RoundType::Fast => {
                config.fast_provider.as_deref().unwrap_or_else(|| config.provider.as_str())
            }
        })
    }

    /// Returns the provider label (human-readable name) for a provider key.
    #[must_use]
    pub fn provider_label(&self, provider: &str) -> Option<&str> {
        self.providers.get(provider).and_then(|p| p.label.as_deref())
    }

    /// Returns the configured prompt template path for a capability, if set.
    ///
    /// Returns `None` if the capability is not defined or has no `prompt_template_path` field.
    #[must_use]
    pub fn resolve_prompt_template_path(&self, capability: &str) -> Option<std::path::PathBuf> {
        self.capabilities
            .get(capability)?
            .prompt_template_path
            .as_deref()
            .map(std::path::PathBuf::from)
    }
}

// Re-export CapabilityConfigDto fields for callers that need raw access.
impl CapabilityConfigDto {
    /// The provider name.
    #[must_use]
    pub fn provider(&self) -> &str {
        self.provider.as_str()
    }

    /// The default model name, if set.
    #[must_use]
    pub fn model(&self) -> Option<&str> {
        self.model.as_ref().map(ModelNameDto::as_str)
    }

    /// The fast-round provider override, if set.
    #[must_use]
    pub fn fast_provider(&self) -> Option<&str> {
        self.fast_provider.as_deref()
    }

    /// The fast-round model override, if set.
    #[must_use]
    pub fn fast_model(&self) -> Option<&str> {
        self.fast_model.as_deref()
    }

    /// The prompt template path for this capability, if set.
    #[must_use]
    pub fn prompt_template_path(&self) -> Option<&str> {
        self.prompt_template_path.as_deref()
    }

    /// The fast-round reasoning effort for this capability, if set (D4 / IN-08).
    ///
    /// `None` when absent — the composition root supplies a built-in default.
    /// Value validation is the responsibility of the composition root (OS-06).
    #[must_use]
    pub fn fast_reasoning_effort(&self) -> Option<&str> {
        self.fast_reasoning_effort.as_deref()
    }

    /// The final-round reasoning effort for this capability, if set (D4 / IN-08).
    ///
    /// `None` when absent — the composition root supplies a built-in default.
    /// Value validation is the responsibility of the composition root (OS-06).
    #[must_use]
    pub fn final_reasoning_effort(&self) -> Option<&str> {
        self.final_reasoning_effort.as_deref()
    }

    /// The execution category declared for this capability.
    #[must_use]
    pub fn execution_mode(&self) -> ExecutionModeDto {
        self.execution_mode
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use std::io::Write as _;

    fn write_json(dir: &std::path::Path, content: &str) -> std::path::PathBuf {
        let path = dir.join("agent-profiles.json");
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        path
    }

    const FULL_CONFIG: &str = r#"{
        "schema_version": 1,
        "providers": {
            "claude": { "label": "Claude Code" },
            "codex": { "label": "Codex CLI" },
            "gemini": { "label": "Gemini CLI" }
        },
        "capabilities": {
            "orchestrator": { "provider": "claude", "model": "claude-opus-4-7", "execution_mode": "typed-pipeline" },
            "planner": { "provider": "claude", "model": "claude-opus-4-7", "execution_mode": "typed-pipeline" },
            "reviewer": { "provider": "codex", "model": "gpt-5.4", "fast_model": "gpt-5.4-mini", "execution_mode": "typed-pipeline" },
            "researcher": { "provider": "gemini", "execution_mode": "typed-pipeline" }
        }
    }"#;

    #[test]
    fn test_agent_profiles_error_free_text_payloads_preserve_display() {
        let io = AgentProfilesError::Io {
            path: FreeText::new("agent-profiles.json"),
            source: std::io::Error::other("read denied"),
        };
        assert_eq!(
            io.to_string(),
            "failed to read agent profiles at agent-profiles.json: read denied"
        );

        let symlink = AgentProfilesError::Symlink { path: FreeText::new("linked-profiles.json") };
        assert_eq!(
            symlink.to_string(),
            "refusing to load agent profiles through a symlink: linked-profiles.json"
        );

        let outside_root = AgentProfilesError::PathOutsideTrustedRoot {
            path: FreeText::new("/tmp/outside/agent-profiles.json"),
            root: FreeText::new("/workspace"),
        };
        assert_eq!(
            outside_root.to_string(),
            "agent profiles path /tmp/outside/agent-profiles.json escapes trusted root /workspace"
        );

        let invalid_capability = AgentProfilesError::InvalidCapability {
            capability: FreeText::new("reviewer"),
            reason: FreeText::new("provider must not be empty"),
        };
        assert_eq!(
            invalid_capability.to_string(),
            "invalid capability 'reviewer': provider must not be empty"
        );
    }

    #[test]
    fn test_load_and_parse_valid_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_json(dir.path(), FULL_CONFIG);
        let profiles = AgentProfiles::load(dir.path(), &path).unwrap();
        assert_eq!(profiles.capabilities.len(), 4);
        assert_eq!(profiles.providers.len(), 3);
        assert_eq!(profiles.provider_label("claude"), Some("Claude Code"));
    }

    #[test]
    fn test_resolve_final_returns_provider_and_model() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_json(dir.path(), FULL_CONFIG);
        let profiles = AgentProfiles::load(dir.path(), &path).unwrap();

        let resolved = profiles.resolve_execution("orchestrator", RoundType::Final).unwrap();
        assert_eq!(resolved.provider, "claude");
        assert_eq!(resolved.model.as_deref(), Some("claude-opus-4-7"));
    }

    #[test]
    fn test_resolve_fast_with_fast_model_only() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_json(dir.path(), FULL_CONFIG);
        let profiles = AgentProfiles::load(dir.path(), &path).unwrap();

        // reviewer has fast_model but no fast_provider → provider stays "codex"
        let resolved = profiles.resolve_execution("reviewer", RoundType::Fast).unwrap();
        assert_eq!(resolved.provider, "codex");
        assert_eq!(resolved.model.as_deref(), Some("gpt-5.4-mini"));
    }

    #[test]
    fn test_resolve_fast_with_cross_provider() {
        let json = r#"{
            "schema_version": 1,
            "providers": {
                "claude": { "label": "Claude" },
                "codex": { "label": "Codex" }
            },
            "capabilities": {
                "reviewer": {
                    "provider": "claude",
                    "model": "claude-opus-4-7",
                    "fast_provider": "codex",
                    "fast_model": "gpt-5.4-mini",
                    "execution_mode": "typed-pipeline"
                }
            }
        }"#;
        let dir = tempfile::tempdir().unwrap();
        let path = write_json(dir.path(), json);
        let profiles = AgentProfiles::load(dir.path(), &path).unwrap();

        let final_exec = profiles.resolve_execution("reviewer", RoundType::Final).unwrap();
        assert_eq!(final_exec.provider, "claude");
        assert_eq!(final_exec.model.as_deref(), Some("claude-opus-4-7"));

        let fast_exec = profiles.resolve_execution("reviewer", RoundType::Fast).unwrap();
        assert_eq!(fast_exec.provider, "codex");
        assert_eq!(fast_exec.model.as_deref(), Some("gpt-5.4-mini"));

        let fast_model: Option<&str> = profiles.resolve_model("reviewer", RoundType::Fast);
        let fast_provider: Option<&str> = profiles.resolve_provider("reviewer", RoundType::Fast);
        assert_eq!(fast_model, Some("gpt-5.4-mini"));
        assert_eq!(fast_provider, Some("codex"));
    }

    #[test]
    fn test_resolve_unknown_capability_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_json(dir.path(), FULL_CONFIG);
        let profiles = AgentProfiles::load(dir.path(), &path).unwrap();

        assert!(profiles.resolve_execution("nonexistent", RoundType::Final).is_none());
    }

    #[test]
    fn test_load_invalid_json_returns_parse_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_json(dir.path(), "not valid json");
        let err = AgentProfiles::load(dir.path(), &path).unwrap_err();
        assert!(matches!(err, AgentProfilesError::Parse(_)));
    }

    #[test]
    fn test_load_missing_file_returns_io_error() {
        let trusted_root = std::path::Path::new("/nonexistent");
        let path = trusted_root.join("agent-profiles.json");
        let err = AgentProfiles::load(trusted_root, &path).unwrap_err();
        assert!(matches!(err, AgentProfilesError::Io { .. }));
    }

    #[cfg(unix)]
    #[test]
    fn test_load_symlinked_file_returns_symlink_error() {
        let dir = tempfile::tempdir().unwrap();
        let target = write_json(dir.path(), FULL_CONFIG);
        let link = dir.path().join("linked-agent-profiles.json");
        std::os::unix::fs::symlink(&target, &link).unwrap();

        let err = AgentProfiles::load(dir.path(), &link).unwrap_err();

        assert!(matches!(err, AgentProfilesError::Symlink { .. }));
    }

    #[cfg(unix)]
    #[test]
    fn test_load_symlinked_config_parent_returns_symlink_error() {
        let workspace = tempfile::tempdir().unwrap();
        let repository = workspace.path().join("repository");
        let external_config = repository.join("real-config");
        std::fs::create_dir_all(&repository).unwrap();
        std::fs::create_dir_all(&external_config).unwrap();
        write_json(&external_config, FULL_CONFIG);
        std::fs::create_dir_all(repository.join(".harness")).unwrap();
        std::os::unix::fs::symlink(&external_config, repository.join(".harness/config")).unwrap();

        let err = AgentProfiles::load(
            &repository,
            &repository.join(".harness/config/agent-profiles.json"),
        )
        .unwrap_err();

        assert!(matches!(err, AgentProfilesError::Symlink { .. }));
    }

    #[cfg(unix)]
    #[test]
    fn test_load_dot_laden_config_path_rejects_symlinked_config_parent() {
        let workspace = tempfile::tempdir().unwrap();
        let repository = workspace.path().join("repository");
        let external_config = repository.join("real-config");
        std::fs::create_dir_all(&repository).unwrap();
        std::fs::create_dir_all(&external_config).unwrap();
        write_json(&external_config, FULL_CONFIG);
        std::fs::create_dir_all(repository.join(".harness")).unwrap();
        std::os::unix::fs::symlink(&external_config, repository.join(".harness/config")).unwrap();
        let path = repository.join(".harness/config/./agent-profiles.json");

        let err = AgentProfiles::load(&repository, &path).unwrap_err();

        assert!(matches!(err, AgentProfilesError::Symlink { .. }));
    }

    #[cfg(unix)]
    #[test]
    fn test_load_symlinked_component_before_parent_is_rejected() {
        let workspace = tempfile::tempdir().unwrap();
        let repository = workspace.path().join("repository");
        let config_dir = repository.join(".harness/config");
        std::fs::create_dir_all(&config_dir).unwrap();
        write_json(&config_dir, FULL_CONFIG);
        let outside = workspace.path().join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        std::os::unix::fs::symlink(&outside, repository.join("internal-link")).unwrap();
        let path = repository.join("internal-link/../.harness/config/agent-profiles.json");

        let err = AgentProfiles::load(&repository, &path).unwrap_err();

        assert!(matches!(err, AgentProfilesError::Symlink { .. }));
    }

    #[test]
    fn test_load_oversize_config_returns_io_error() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("agent-profiles.json");
        std::fs::File::create(&path)
            .unwrap()
            .set_len(crate::capability_exec::MAX_CAPABILITY_EXEC_TEXT_BYTES + 1)
            .unwrap();

        let error = AgentProfiles::load(directory.path(), &path).unwrap_err();

        assert!(matches!(
            error,
            AgentProfilesError::Io { source, .. }
                if source.kind() == std::io::ErrorKind::InvalidData
        ));
    }

    #[test]
    fn test_load_parent_dir_within_trusted_root_uses_normalized_path() {
        let workspace = tempfile::tempdir().unwrap();
        let repository = workspace.path().join("repository");
        let config_dir = repository.join(".harness/config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::create_dir_all(repository.join("apps/cli-composition")).unwrap();
        write_json(&config_dir, FULL_CONFIG);
        let path =
            repository.join("apps/cli-composition/../../.harness/config/agent-profiles.json");

        let profiles = AgentProfiles::load(&repository, &path).unwrap();

        assert_eq!(profiles.capabilities.len(), 4);
    }

    #[test]
    fn test_load_arbitrary_path_outside_independent_trusted_root_returns_path_outside_error() {
        let workspace = tempfile::tempdir().unwrap();
        let repository = workspace.path().join("repository");
        std::fs::create_dir_all(&repository).unwrap();
        let outside = workspace.path().join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        write_json(&outside, FULL_CONFIG);
        let path = outside.join("agent-profiles.json");

        let err = AgentProfiles::load(&repository, &path).unwrap_err();

        assert!(matches!(err, AgentProfilesError::PathOutsideTrustedRoot { .. }));
    }

    #[cfg(unix)]
    #[test]
    fn test_load_accepts_workspace_root_reached_through_symlink() {
        let workspace = tempfile::tempdir().unwrap();
        let repository = workspace.path().join("repository");
        let config_dir = repository.join(".harness/config");
        std::fs::create_dir_all(&config_dir).unwrap();
        write_json(&config_dir, FULL_CONFIG);
        let workspace_link = workspace.path().join("workspace-link");
        std::os::unix::fs::symlink(&repository, &workspace_link).unwrap();

        let profiles = AgentProfiles::load(
            &workspace_link,
            &workspace_link.join(".harness/config/agent-profiles.json"),
        )
        .unwrap();

        assert_eq!(profiles.capabilities.len(), 4);
    }

    #[test]
    fn test_resolve_fast_without_fast_fields_falls_back() {
        // orchestrator has no fast_model or fast_provider → fallback to provider + model
        let dir = tempfile::tempdir().unwrap();
        let path = write_json(dir.path(), FULL_CONFIG);
        let profiles = AgentProfiles::load(dir.path(), &path).unwrap();

        let resolved = profiles.resolve_execution("orchestrator", RoundType::Fast).unwrap();
        assert_eq!(resolved.provider, "claude");
        assert_eq!(resolved.model.as_deref(), Some("claude-opus-4-7"));
    }

    #[test]
    fn test_resolve_model_none_when_not_specified() {
        // researcher has provider=gemini but no model
        let dir = tempfile::tempdir().unwrap();
        let path = write_json(dir.path(), FULL_CONFIG);
        let profiles = AgentProfiles::load(dir.path(), &path).unwrap();

        let resolved = profiles.resolve_execution("researcher", RoundType::Final).unwrap();
        assert_eq!(resolved.provider, "gemini");
        assert!(resolved.model.is_none());
    }

    #[test]
    fn test_load_unsupported_schema_version_returns_error() {
        let json = r#"{
            "schema_version": 2,
            "providers": {},
            "capabilities": {}
        }"#;
        let dir = tempfile::tempdir().unwrap();
        let path = write_json(dir.path(), json);
        let err = AgentProfiles::load(dir.path(), &path).unwrap_err();
        assert!(
            matches!(err, AgentProfilesError::UnsupportedSchemaVersion { found: 2, expected: 1 }),
            "unexpected error variant: {err}"
        );
    }

    #[test]
    fn test_shortcut_resolution_fast_and_final_returns_borrowed_config_values() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_json(dir.path(), FULL_CONFIG);
        let profiles = AgentProfiles::load(dir.path(), &path).unwrap();

        assert_eq!(profiles.resolve_model("reviewer", RoundType::Fast), Some("gpt-5.4-mini"));
        assert_eq!(
            profiles.resolve_model("orchestrator", RoundType::Fast),
            Some("claude-opus-4-7")
        );
        assert_eq!(profiles.resolve_provider("reviewer", RoundType::Fast), Some("codex"));
        assert_eq!(profiles.resolve_provider("reviewer", RoundType::Final), Some("codex"));
        assert!(profiles.resolve_model("researcher", RoundType::Final).is_none());
    }

    #[test]
    fn test_load_empty_provider_returns_parse_error() {
        let json = r#"{
            "schema_version": 1,
            "providers": {},
            "capabilities": {
                "reviewer": { "provider": "", "model": "gpt-5.4", "execution_mode": "typed-pipeline" }
            }
        }"#;
        let dir = tempfile::tempdir().unwrap();
        let path = write_json(dir.path(), json);
        let err = AgentProfiles::load(dir.path(), &path).unwrap_err();
        assert!(
            matches!(err, AgentProfilesError::Parse(ref error) if error.to_string().contains("provider name must not be empty")),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_load_empty_model_returns_parse_error() {
        let json = r#"{
            "schema_version": 1,
            "providers": {},
            "capabilities": {
                "reviewer": { "provider": "codex", "model": "", "execution_mode": "typed-pipeline" }
            }
        }"#;
        let dir = tempfile::tempdir().unwrap();
        let path = write_json(dir.path(), json);
        let err = AgentProfiles::load(dir.path(), &path).unwrap_err();
        assert!(
            matches!(err, AgentProfilesError::Parse(ref error) if error.to_string().contains("model name must not be empty")),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_load_empty_fast_model_returns_invalid_capability() {
        let json = r#"{
            "schema_version": 1,
            "providers": {},
            "capabilities": {
                "reviewer": { "provider": "codex", "fast_model": " ", "execution_mode": "typed-pipeline" }
            }
        }"#;
        let dir = tempfile::tempdir().unwrap();
        let path = write_json(dir.path(), json);
        let err = AgentProfiles::load(dir.path(), &path).unwrap_err();
        assert!(
            matches!(err, AgentProfilesError::InvalidCapability { .. }),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_load_future_schema_version_returns_unsupported_not_parse() {
        // Even if future schema has new fields, we should get UnsupportedSchemaVersion,
        // not a Parse error from deny_unknown_fields.
        let json = r#"{
            "schema_version": 99,
            "providers": {},
            "capabilities": {},
            "new_future_field": "should not cause parse error"
        }"#;
        let dir = tempfile::tempdir().unwrap();
        let path = write_json(dir.path(), json);
        let err = AgentProfiles::load(dir.path(), &path).unwrap_err();
        assert!(
            matches!(err, AgentProfilesError::UnsupportedSchemaVersion { found: 99, .. }),
            "expected UnsupportedSchemaVersion, got: {err}"
        );
    }

    #[test]
    fn test_load_empty_fast_provider_returns_invalid_capability() {
        let json = r#"{
            "schema_version": 1,
            "providers": {},
            "capabilities": {
                "reviewer": { "provider": "codex", "fast_provider": " ", "execution_mode": "typed-pipeline" }
            }
        }"#;
        let dir = tempfile::tempdir().unwrap();
        let path = write_json(dir.path(), json);
        let err = AgentProfiles::load(dir.path(), &path).unwrap_err();
        assert!(
            matches!(err, AgentProfilesError::InvalidCapability { .. }),
            "unexpected error: {err}"
        );
    }

    // -----------------------------------------------------------------------
    // T006 tests: ref-verifier-chain1/chain2 capabilities with
    // prompt_template_path fields; independent resolution from reviewer (D11).
    // -----------------------------------------------------------------------

    const REF_VERIFIER_CONFIG: &str = r#"{
        "schema_version": 1,
        "providers": {
            "claude": { "label": "Claude Code" },
            "codex": { "label": "Codex CLI" }
        },
        "capabilities": {
            "reviewer": {
                "provider": "codex",
                "model": "gpt-5.5",
                "fast_model": "gpt-5.4-mini",
                "prompt_template_path": ".harness/prompts/reviewer.md",
                "execution_mode": "typed-pipeline"
            },
            "ref-verifier-chain1": {
                "provider": "claude",
                "model": "claude-opus-4-8",
                "fast_provider": "claude",
                "fast_model": "claude-haiku-4-5",
                "prompt_template_path": ".harness/prompts/ref-verifier-chain1.md",
                "execution_mode": "typed-pipeline"
            },
            "ref-verifier-chain2": {
                "provider": "claude",
                "model": "claude-opus-4-8",
                "fast_provider": "claude",
                "fast_model": "claude-haiku-4-5",
                "prompt_template_path": ".harness/prompts/ref-verifier-chain2.md",
                "execution_mode": "typed-pipeline"
            }
        }
    }"#;

    #[test]
    fn test_load_ref_verifier_chain_capabilities_are_loaded() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_json(dir.path(), REF_VERIFIER_CONFIG);
        let profiles = AgentProfiles::load(dir.path(), &path).unwrap();
        assert!(profiles.resolve_capability("ref-verifier-chain1").is_some());
        assert!(profiles.resolve_capability("ref-verifier-chain2").is_some());
    }

    #[test]
    fn test_resolve_ref_verifier_chain1_fast_returns_fast_provider_and_fast_model() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_json(dir.path(), REF_VERIFIER_CONFIG);
        let profiles = AgentProfiles::load(dir.path(), &path).unwrap();

        let fast = profiles.resolve_execution("ref-verifier-chain1", RoundType::Fast).unwrap();
        assert_eq!(fast.provider, "claude");
        assert_eq!(fast.model.as_deref(), Some("claude-haiku-4-5"));
    }

    #[test]
    fn test_resolve_ref_verifier_chain1_final_returns_provider_and_model() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_json(dir.path(), REF_VERIFIER_CONFIG);
        let profiles = AgentProfiles::load(dir.path(), &path).unwrap();

        let final_exec =
            profiles.resolve_execution("ref-verifier-chain1", RoundType::Final).unwrap();
        assert_eq!(final_exec.provider, "claude");
        assert_eq!(final_exec.model.as_deref(), Some("claude-opus-4-8"));
    }

    #[test]
    fn test_ref_verifier_chain_prompt_template_paths_are_independent() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_json(dir.path(), REF_VERIFIER_CONFIG);
        let profiles = AgentProfiles::load(dir.path(), &path).unwrap();

        let chain1_path = profiles.resolve_prompt_template_path("ref-verifier-chain1").unwrap();
        let chain2_path = profiles.resolve_prompt_template_path("ref-verifier-chain2").unwrap();

        assert_eq!(chain1_path.to_str(), Some(".harness/prompts/ref-verifier-chain1.md"));
        assert_eq!(chain2_path.to_str(), Some(".harness/prompts/ref-verifier-chain2.md"));
        assert_ne!(chain1_path, chain2_path);
    }

    #[test]
    fn test_resolve_prompt_template_path_returns_none_for_missing_capability() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_json(dir.path(), REF_VERIFIER_CONFIG);
        let profiles = AgentProfiles::load(dir.path(), &path).unwrap();

        assert!(profiles.resolve_prompt_template_path("nonexistent").is_none());
    }

    #[test]
    fn test_resolve_prompt_template_path_returns_none_when_field_absent() {
        // orchestrator entry has no prompt_template_path field
        let dir = tempfile::tempdir().unwrap();
        let path = write_json(dir.path(), FULL_CONFIG);
        let profiles = AgentProfiles::load(dir.path(), &path).unwrap();

        assert!(profiles.resolve_prompt_template_path("orchestrator").is_none());
    }

    #[test]
    fn test_capability_config_with_timeout_seconds_is_rejected() {
        // timeout_seconds was removed from the schema; deny_unknown_fields
        // must fail-closed on configs that still carry it.
        let json = r#"{
            "schema_version": 1,
            "providers": { "claude": { "label": "Claude" } },
            "capabilities": {
                "ref-verifier-chain1": {
                    "provider": "claude",
                    "model": "claude-opus-4-8",
                    "execution_mode": "typed-pipeline",
                    "timeout_seconds": 120
                }
            }
        }"#;
        let dir = tempfile::tempdir().unwrap();
        let path = write_json(dir.path(), json);
        let err = AgentProfiles::load(dir.path(), &path).unwrap_err();
        assert!(
            err.to_string().contains("timeout_seconds"),
            "expected unknown-field rejection, got: {err}"
        );
    }

    // -----------------------------------------------------------------------
    // T023 tests: test-obligation semantic verifier capabilities.
    // -----------------------------------------------------------------------

    const TEST_OBLIGATION_VERIFIER_CONFIG: &str = r#"{
        "schema_version": 1,
        "providers": {
            "codex": { "label": "Codex CLI" }
        },
        "capabilities": {
            "obligation-fulfillment-verifier": {
                "provider": "codex",
                "model": "gpt-5.5",
                "fast_provider": "codex",
                "fast_model": "gpt-5.4-mini",
                "execution_mode": "typed-pipeline"
            },
            "waiver-verifier": {
                "provider": "codex",
                "model": "gpt-5.5",
                "fast_provider": "codex",
                "fast_model": "gpt-5.4-mini",
                "execution_mode": "typed-pipeline"
            }
        }
    }"#;

    #[test]
    fn test_resolve_test_obligation_verifier_capabilities_fast_and_final() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_json(dir.path(), TEST_OBLIGATION_VERIFIER_CONFIG);
        let profiles = AgentProfiles::load(dir.path(), &path).unwrap();

        for capability in ["obligation-fulfillment-verifier", "waiver-verifier"] {
            let fast = profiles.resolve_execution(capability, RoundType::Fast).unwrap();
            assert_eq!(fast.provider, "codex");
            assert_eq!(fast.model.as_deref(), Some("gpt-5.4-mini"));

            let final_exec = profiles.resolve_execution(capability, RoundType::Final).unwrap();
            assert_eq!(final_exec.provider, "codex");
            assert_eq!(final_exec.model.as_deref(), Some("gpt-5.5"));
        }
    }

    #[test]
    fn test_default_agent_profiles_register_test_obligation_verifiers() {
        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let workspace_root = manifest_dir
            .parent()
            .and_then(std::path::Path::parent)
            .expect("infrastructure crate must live under libs/infrastructure");
        let profiles =
            AgentProfiles::load(workspace_root, &workspace_root.join(AGENT_PROFILES_PATH)).unwrap();

        for capability in ["obligation-fulfillment-verifier", "waiver-verifier"] {
            assert!(profiles.resolve_capability(capability).is_some());
            assert!(profiles.resolve_execution(capability, RoundType::Fast).is_some());
            assert!(profiles.resolve_execution(capability, RoundType::Final).is_some());
        }
    }

    // -----------------------------------------------------------------------
    // T011 / T013 / T015 tests: dry-checker capability with fast_model and
    // reasoning_effort fields (D4 / IN-04 / IN-08).
    // -----------------------------------------------------------------------

    const DRY_CHECKER_CONFIG: &str = r#"{
        "schema_version": 1,
        "providers": {
            "codex": { "label": "Codex CLI" }
        },
        "capabilities": {
            "dry-checker": {
                "provider": "codex",
                "model": "gpt-5.5",
                "fast_model": "gpt-5.4-mini",
                "fast_reasoning_effort": "medium",
                "final_reasoning_effort": "high",
                "execution_mode": "typed-pipeline"
            }
        }
    }"#;

    #[test]
    fn test_dry_checker_fast_round_returns_fast_model() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_json(dir.path(), DRY_CHECKER_CONFIG);
        let profiles = AgentProfiles::load(dir.path(), &path).unwrap();

        let fast = profiles.resolve_execution("dry-checker", RoundType::Fast).unwrap();
        assert_eq!(fast.provider, "codex");
        assert_eq!(fast.model.as_deref(), Some("gpt-5.4-mini"));
    }

    #[test]
    fn test_dry_checker_final_round_returns_primary_model() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_json(dir.path(), DRY_CHECKER_CONFIG);
        let profiles = AgentProfiles::load(dir.path(), &path).unwrap();

        let final_exec = profiles.resolve_execution("dry-checker", RoundType::Final).unwrap();
        assert_eq!(final_exec.provider, "codex");
        assert_eq!(final_exec.model.as_deref(), Some("gpt-5.5"));
    }

    // ── D4 / T013 / T015: CapabilityConfigDto reasoning_effort accessors ──

    #[test]
    fn test_dry_checker_capability_dto_fast_reasoning_effort_accessor() {
        // Verify that the new fast_reasoning_effort accessor returns the configured value.
        let dir = tempfile::tempdir().unwrap();
        let path = write_json(dir.path(), DRY_CHECKER_CONFIG);
        let profiles = AgentProfiles::load(dir.path(), &path).unwrap();

        let capability = profiles.resolve_capability("dry-checker").unwrap();
        assert_eq!(
            capability.fast_reasoning_effort(),
            Some("medium"),
            "fast_reasoning_effort accessor must return the configured value"
        );
    }

    #[test]
    fn test_dry_checker_capability_dto_final_reasoning_effort_accessor() {
        // Verify that the new final_reasoning_effort accessor returns the configured value.
        let dir = tempfile::tempdir().unwrap();
        let path = write_json(dir.path(), DRY_CHECKER_CONFIG);
        let profiles = AgentProfiles::load(dir.path(), &path).unwrap();

        let capability = profiles.resolve_capability("dry-checker").unwrap();
        assert_eq!(
            capability.final_reasoning_effort(),
            Some("high"),
            "final_reasoning_effort accessor must return the configured value"
        );
    }

    #[test]
    fn test_capability_dto_reasoning_effort_returns_none_when_absent() {
        // When reasoning_effort fields are absent from the capability, accessors return None.
        let json = r#"{
            "schema_version": 1,
            "providers": { "codex": { "label": "Codex CLI" } },
            "capabilities": {
                "dry-checker": {
                    "provider": "codex",
                    "model": "gpt-5.5",
                    "fast_model": "gpt-5.4-mini",
                    "execution_mode": "typed-pipeline"
                }
            }
        }"#;
        let dir = tempfile::tempdir().unwrap();
        let path = write_json(dir.path(), json);
        let profiles = AgentProfiles::load(dir.path(), &path).unwrap();

        let capability = profiles.resolve_capability("dry-checker").unwrap();
        assert!(
            capability.fast_reasoning_effort().is_none(),
            "fast_reasoning_effort must be None when the field is absent"
        );
        assert!(
            capability.final_reasoning_effort().is_none(),
            "final_reasoning_effort must be None when the field is absent"
        );
    }

    // -----------------------------------------------------------------------
    // RoundType::from_str tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_round_type_from_str_fast_succeeds() {
        let result: Result<RoundType, _> = "fast".parse();
        assert_eq!(result.unwrap(), RoundType::Fast);
    }

    #[test]
    fn test_round_type_from_str_final_succeeds() {
        let result: Result<RoundType, _> = "final".parse();
        assert_eq!(result.unwrap(), RoundType::Final);
    }

    #[test]
    fn test_round_type_from_str_unknown_returns_error() {
        let result: Result<RoundType, _> = "unknown".parse();
        assert!(result.is_err());
        let msg = result.unwrap_err();
        assert!(
            msg.contains("unknown round type"),
            "expected error message to mention 'unknown round type', got: {msg}"
        );
    }
}
