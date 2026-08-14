//! Profile loading and execution-resolution behavior.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use domain::FreeText;
use usecase::capability_exec::{CODEX_PROVIDER_NAME, ProviderName, ReasoningEffort};
use usecase::dry_write_driver::CapabilityName;

use super::types::{AgentProfilesDto, ProviderMetadataDto, SchemaVersionEnvelope};
use super::{
    AgentProfilesError, CapabilityConfigDto, CapabilityProviderBindingDto, ExecutionModeDto,
    ModelNameDto, ProviderNameDto, ReasoningEffortDto, ResolvedExecution, RoundType,
};
use crate::capability_exec::{
    bounded_read_utf8_file,
    path_guard::{lexically_normalize, normalize_path_rejecting_symlinked_components},
};

/// Loaded agent profiles configuration.
///
/// Provides resolution of capability → (provider, model) pairs.
#[derive(Debug)]
pub struct AgentProfiles {
    pub(super) providers: HashMap<String, ProviderMetadataDto>,
    pub(super) capabilities: HashMap<String, CapabilityConfigDto>,
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
        for name in dto.capabilities.keys() {
            if name.trim().is_empty() {
                return Err(AgentProfilesError::InvalidCapability {
                    capability: FreeText::new(name.clone()),
                    reason: FreeText::new("capability name must not be empty"),
                });
            }
        }
        Ok(Self { providers: dto.providers, capabilities: dto.capabilities })
    }

    /// Returns the raw capability configuration for the given capability name.
    #[must_use]
    pub fn resolve_capability(&self, capability: &CapabilityName) -> Option<&CapabilityConfigDto> {
        self.capabilities.get(capability.as_str())
    }

    /// Resolves all provider execution settings for a capability and round type.
    ///
    /// Resolution rules:
    /// Hosted `pr-reviewer` execution is the sole effort-exempt branch. Every
    /// provider-CLI branch must resolve an explicit model and effort.
    ///
    /// # Errors
    ///
    /// Returns a resolution error when the capability is missing, a provider
    /// CLI model/effort is absent, or that effort is unsupported by the
    /// selected provider.
    pub fn resolve_execution(
        &self,
        capability: &CapabilityName,
        round_type: RoundType,
    ) -> Result<ResolvedExecution, AgentProfilesError> {
        let config = self
            .resolve_capability(capability)
            .ok_or_else(|| AgentProfilesError::CapabilityNotFound(capability.clone()))?;
        let default_provider = match config.provider_binding() {
            CapabilityProviderBindingDto::Standard(provider) => provider.clone().into_domain(),
            CapabilityProviderBindingDto::CodexCustom(_) => CODEX_PROVIDER_NAME.clone(),
        };
        let provider = match round_type {
            RoundType::Final => default_provider,
            RoundType::Fast => config
                .fast_provider
                .clone()
                .map(ProviderNameDto::into_domain)
                .unwrap_or(default_provider),
        };

        if capability.as_str() == "pr-reviewer" {
            return Ok(ResolvedExecution::HostedService { provider });
        }

        let model = match round_type {
            RoundType::Final => config.model.clone(),
            RoundType::Fast => config.fast_model.clone().or_else(|| config.model.clone()),
        }
        .ok_or_else(|| AgentProfilesError::ModelMissing(capability.clone()))?
        .into_domain();
        let has_fast_round_override = config.fast_provider.is_some()
            || config.fast_model.is_some()
            || config.fast_effort.is_some();
        let effort = match round_type {
            RoundType::Final => config.effort,
            RoundType::Fast if has_fast_round_override => config.fast_effort,
            RoundType::Fast => config.effort,
        }
        .ok_or_else(|| AgentProfilesError::EffortMissing(capability.clone(), round_type))?
        .into_domain();
        if !self.supports_effort(&provider, effort) {
            return Err(AgentProfilesError::UnsupportedEffort(provider, effort));
        }

        Ok(ResolvedExecution::ProviderCli { provider, model, effort })
    }

    /// Returns the provider label (human-readable name) for a provider key.
    #[must_use]
    pub fn provider_label(&self, provider: &ProviderName) -> Option<&str> {
        self.providers.get(provider.as_str()).and_then(|p| p.label.as_deref())
    }

    /// Returns the configured prompt template path for a capability, if set.
    ///
    /// Returns `None` if the capability is not defined or has no `prompt_template_path` field.
    #[must_use]
    pub fn resolve_prompt_template_path(&self, capability: &CapabilityName) -> Option<PathBuf> {
        self.capabilities.get(capability.as_str())?.prompt_template_path.clone()
    }
}

impl CapabilityConfigDto {
    /// The provider binding selected by the flat profile fields.
    #[must_use]
    pub fn provider_binding(&self) -> &CapabilityProviderBindingDto {
        &self.provider_binding
    }

    /// The default model name, if set.
    #[must_use]
    pub fn model(&self) -> Option<&ModelNameDto> {
        self.model.as_ref()
    }

    /// The fast-round provider override, if set.
    #[must_use]
    pub fn fast_provider(&self) -> Option<&ProviderNameDto> {
        self.fast_provider.as_ref()
    }

    /// The fast-round model override, if set.
    #[must_use]
    pub fn fast_model(&self) -> Option<&ModelNameDto> {
        self.fast_model.as_ref()
    }

    /// The prompt template path for this capability, if set.
    #[must_use]
    pub fn prompt_template_path(&self) -> Option<&Path> {
        self.prompt_template_path.as_deref()
    }

    /// The final/default reasoning effort for this capability, if configured.
    #[must_use]
    pub fn effort(&self) -> Option<ReasoningEffortDto> {
        self.effort
    }

    /// The fast-round reasoning effort for this capability, if configured.
    #[must_use]
    pub fn fast_effort(&self) -> Option<ReasoningEffortDto> {
        self.fast_effort
    }

    /// The execution category declared for this capability.
    #[must_use]
    pub fn execution_mode(&self) -> ExecutionModeDto {
        self.execution_mode
    }
}

impl AgentProfiles {
    fn supports_effort(&self, provider: &ProviderName, effort: ReasoningEffort) -> bool {
        self.providers.get(provider.as_str()).is_some_and(|metadata| {
            metadata
                .supported_reasoning_efforts
                .iter()
                .copied()
                .any(|declared_effort| declared_effort.into_domain() == effort)
        })
    }
}
