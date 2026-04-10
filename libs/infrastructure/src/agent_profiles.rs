//! I/O for `.harness/config/agent-profiles.json` — capability-centric agent routing.
//!
//! Reads the v2 schema (`schema_version: 1`) where each capability directly
//! specifies its provider and model. Resolution follows:
//! - `RoundType::Final` → `(provider, model)`
//! - `RoundType::Fast`  → `(fast_provider ?? provider, fast_model ?? model)`

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;

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
    Io { path: String, source: std::io::Error },

    /// The configuration file contains invalid JSON.
    #[error("failed to parse agent profiles: {0}")]
    Parse(#[from] serde_json::Error),

    /// The configuration file uses an unsupported schema version.
    #[error("unsupported agent profiles schema version {found}; expected {expected}")]
    UnsupportedSchemaVersion { found: u32, expected: u32 },

    /// A capability entry has an invalid configuration (e.g., empty provider name).
    #[error("invalid capability '{capability}': {reason}")]
    InvalidCapability { capability: String, reason: String },
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
    provider: String,
    model: Option<String>,
    fast_provider: Option<String>,
    fast_model: Option<String>,
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
    /// Loads agent profiles from a JSON file.
    ///
    /// # Errors
    ///
    /// Returns [`AgentProfilesError::Io`] if the file cannot be read, or
    /// [`AgentProfilesError::Parse`] if the JSON is invalid.
    pub fn load(path: &Path) -> Result<Self, AgentProfilesError> {
        const SUPPORTED_SCHEMA_VERSION: u32 = 1;

        let content = std::fs::read_to_string(path)
            .map_err(|e| AgentProfilesError::Io { path: path.display().to_string(), source: e })?;
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
            if config.provider.trim().is_empty() {
                return Err(AgentProfilesError::InvalidCapability {
                    capability: name.clone(),
                    reason: "provider must not be empty".to_owned(),
                });
            }
            if let Some(fp) = &config.fast_provider {
                if fp.trim().is_empty() {
                    return Err(AgentProfilesError::InvalidCapability {
                        capability: name.clone(),
                        reason: "fast_provider must not be empty when specified".to_owned(),
                    });
                }
            }
            // Reject empty model/fast_model strings (must be non-empty when specified).
            if config.model.as_deref().is_some_and(|s| s.trim().is_empty()) {
                return Err(AgentProfilesError::InvalidCapability {
                    capability: name.clone(),
                    reason: "model must not be empty when specified".to_owned(),
                });
            }
            if config.fast_model.as_deref().is_some_and(|s| s.trim().is_empty()) {
                return Err(AgentProfilesError::InvalidCapability {
                    capability: name.clone(),
                    reason: "fast_model must not be empty when specified".to_owned(),
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
                provider: config.provider.clone(),
                model: config.model.clone(),
            }),
            RoundType::Fast => Some(ResolvedExecution {
                provider: config.fast_provider.clone().unwrap_or_else(|| config.provider.clone()),
                model: Some(
                    config.fast_model.clone().or_else(|| config.model.clone()).unwrap_or_default(),
                )
                .filter(|s| !s.is_empty()),
            }),
        }
    }

    /// Shortcut: resolve just the model name for a capability and round type.
    #[must_use]
    pub fn resolve_model(&self, capability: &str, round_type: RoundType) -> Option<String> {
        self.resolve_execution(capability, round_type).and_then(|r| r.model)
    }

    /// Shortcut: resolve just the provider name for a capability and round type.
    #[must_use]
    pub fn resolve_provider(&self, capability: &str, round_type: RoundType) -> Option<String> {
        self.resolve_execution(capability, round_type).map(|r| r.provider)
    }

    /// Returns the provider label (human-readable name) for a provider key.
    #[must_use]
    pub fn provider_label(&self, provider: &str) -> Option<&str> {
        self.providers.get(provider).and_then(|p| p.label.as_deref())
    }
}

// Re-export CapabilityConfigDto fields for callers that need raw access.
impl CapabilityConfigDto {
    /// The provider name.
    #[must_use]
    pub fn provider(&self) -> &str {
        &self.provider
    }

    /// The default model name, if set.
    #[must_use]
    pub fn model(&self) -> Option<&str> {
        self.model.as_deref()
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
            "orchestrator": { "provider": "claude", "model": "claude-opus-4-6" },
            "planner": { "provider": "claude", "model": "claude-opus-4-6" },
            "reviewer": { "provider": "codex", "model": "gpt-5.4", "fast_model": "gpt-5.4-mini" },
            "researcher": { "provider": "gemini" }
        }
    }"#;

    #[test]
    fn test_load_and_parse_valid_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_json(dir.path(), FULL_CONFIG);
        let profiles = AgentProfiles::load(&path).unwrap();
        assert_eq!(profiles.capabilities.len(), 4);
        assert_eq!(profiles.providers.len(), 3);
        assert_eq!(profiles.provider_label("claude"), Some("Claude Code"));
    }

    #[test]
    fn test_resolve_final_returns_provider_and_model() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_json(dir.path(), FULL_CONFIG);
        let profiles = AgentProfiles::load(&path).unwrap();

        let resolved = profiles.resolve_execution("orchestrator", RoundType::Final).unwrap();
        assert_eq!(resolved.provider, "claude");
        assert_eq!(resolved.model.as_deref(), Some("claude-opus-4-6"));
    }

    #[test]
    fn test_resolve_fast_with_fast_model_only() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_json(dir.path(), FULL_CONFIG);
        let profiles = AgentProfiles::load(&path).unwrap();

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
                    "model": "claude-opus-4-6",
                    "fast_provider": "codex",
                    "fast_model": "gpt-5.4-mini"
                }
            }
        }"#;
        let dir = tempfile::tempdir().unwrap();
        let path = write_json(dir.path(), json);
        let profiles = AgentProfiles::load(&path).unwrap();

        let final_exec = profiles.resolve_execution("reviewer", RoundType::Final).unwrap();
        assert_eq!(final_exec.provider, "claude");
        assert_eq!(final_exec.model.as_deref(), Some("claude-opus-4-6"));

        let fast_exec = profiles.resolve_execution("reviewer", RoundType::Fast).unwrap();
        assert_eq!(fast_exec.provider, "codex");
        assert_eq!(fast_exec.model.as_deref(), Some("gpt-5.4-mini"));
    }

    #[test]
    fn test_resolve_unknown_capability_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_json(dir.path(), FULL_CONFIG);
        let profiles = AgentProfiles::load(&path).unwrap();

        assert!(profiles.resolve_execution("nonexistent", RoundType::Final).is_none());
    }

    #[test]
    fn test_load_invalid_json_returns_parse_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_json(dir.path(), "not valid json");
        let err = AgentProfiles::load(&path).unwrap_err();
        assert!(matches!(err, AgentProfilesError::Parse(_)));
    }

    #[test]
    fn test_load_missing_file_returns_io_error() {
        let path = std::path::Path::new("/nonexistent/agent-profiles.json");
        let err = AgentProfiles::load(path).unwrap_err();
        assert!(matches!(err, AgentProfilesError::Io { .. }));
    }

    #[test]
    fn test_resolve_fast_without_fast_fields_falls_back() {
        // orchestrator has no fast_model or fast_provider → fallback to provider + model
        let dir = tempfile::tempdir().unwrap();
        let path = write_json(dir.path(), FULL_CONFIG);
        let profiles = AgentProfiles::load(&path).unwrap();

        let resolved = profiles.resolve_execution("orchestrator", RoundType::Fast).unwrap();
        assert_eq!(resolved.provider, "claude");
        assert_eq!(resolved.model.as_deref(), Some("claude-opus-4-6"));
    }

    #[test]
    fn test_resolve_model_none_when_not_specified() {
        // researcher has provider=gemini but no model
        let dir = tempfile::tempdir().unwrap();
        let path = write_json(dir.path(), FULL_CONFIG);
        let profiles = AgentProfiles::load(&path).unwrap();

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
        let err = AgentProfiles::load(&path).unwrap_err();
        assert!(
            matches!(err, AgentProfilesError::UnsupportedSchemaVersion { found: 2, expected: 1 }),
            "unexpected error variant: {err}"
        );
    }

    #[test]
    fn test_shortcut_resolve_model_and_provider() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_json(dir.path(), FULL_CONFIG);
        let profiles = AgentProfiles::load(&path).unwrap();

        assert_eq!(
            profiles.resolve_model("reviewer", RoundType::Fast).as_deref(),
            Some("gpt-5.4-mini")
        );
        assert_eq!(
            profiles.resolve_provider("reviewer", RoundType::Final).as_deref(),
            Some("codex")
        );
        assert!(profiles.resolve_model("researcher", RoundType::Final).is_none());
    }

    #[test]
    fn test_load_empty_provider_returns_invalid_capability() {
        let json = r#"{
            "schema_version": 1,
            "providers": {},
            "capabilities": {
                "reviewer": { "provider": "", "model": "gpt-5.4" }
            }
        }"#;
        let dir = tempfile::tempdir().unwrap();
        let path = write_json(dir.path(), json);
        let err = AgentProfiles::load(&path).unwrap_err();
        assert!(
            matches!(err, AgentProfilesError::InvalidCapability { .. }),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_load_empty_model_returns_invalid_capability() {
        let json = r#"{
            "schema_version": 1,
            "providers": {},
            "capabilities": {
                "reviewer": { "provider": "codex", "model": "" }
            }
        }"#;
        let dir = tempfile::tempdir().unwrap();
        let path = write_json(dir.path(), json);
        let err = AgentProfiles::load(&path).unwrap_err();
        assert!(
            matches!(err, AgentProfilesError::InvalidCapability { .. }),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_load_empty_fast_model_returns_invalid_capability() {
        let json = r#"{
            "schema_version": 1,
            "providers": {},
            "capabilities": {
                "reviewer": { "provider": "codex", "fast_model": " " }
            }
        }"#;
        let dir = tempfile::tempdir().unwrap();
        let path = write_json(dir.path(), json);
        let err = AgentProfiles::load(&path).unwrap_err();
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
        let err = AgentProfiles::load(&path).unwrap_err();
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
                "reviewer": { "provider": "codex", "fast_provider": " " }
            }
        }"#;
        let dir = tempfile::tempdir().unwrap();
        let path = write_json(dir.path(), json);
        let err = AgentProfiles::load(&path).unwrap_err();
        assert!(
            matches!(err, AgentProfilesError::InvalidCapability { .. }),
            "unexpected error: {err}"
        );
    }
}
