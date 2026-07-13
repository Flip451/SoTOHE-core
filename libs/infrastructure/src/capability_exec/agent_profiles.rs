//! Runtime profile adapter for generic capability dispatch.

use std::path::PathBuf;

use usecase::capability_exec::{
    CapabilityExecError, CapabilityFailureDetail, CapabilityProfile, CapabilityProfilePort,
    ExecutionMode, ModelName, ProviderName,
};
use usecase::dry_write_driver::CapabilityName;

use crate::agent_profiles::AgentProfiles;

/// Resolves capability execution profiles from `agent-profiles.json`.
pub struct AgentProfilesCapabilityAdapter {
    trusted_root: PathBuf,
    path: PathBuf,
}

impl AgentProfilesCapabilityAdapter {
    /// Creates an adapter that reads `path` within `trusted_root` on resolution.
    #[must_use]
    pub fn new(trusted_root: PathBuf, path: PathBuf) -> Self {
        Self { trusted_root, path }
    }
}

impl CapabilityProfilePort for AgentProfilesCapabilityAdapter {
    fn resolve(
        &self,
        capability: &CapabilityName,
    ) -> Result<CapabilityProfile, CapabilityExecError> {
        let profiles = AgentProfiles::load(&self.trusted_root, &self.path)
            .map_err(|error| profile_error(capability, error))?;
        let config = profiles.resolve_capability(capability.as_str()).ok_or_else(|| {
            profile_error_message(capability, "capability is not declared in agent-profiles.json")
        })?;
        let execution_mode = config.execution_mode().into_domain();
        if execution_mode != ExecutionMode::OrchestratorOutput {
            return Err(profile_error_message(
                capability,
                "only orchestrator-output capabilities are eligible for generic dispatch",
            ));
        }
        let provider = ProviderName::try_new(config.provider().to_owned())
            .map_err(|error| profile_error(capability, error))?;
        let model = config
            .model()
            .ok_or_else(|| CapabilityExecError::ModelMissing { capability: capability.clone() })?;
        let model = ModelName::try_new(model.to_owned())
            .map_err(|error| profile_error(capability, error))?;

        Ok(CapabilityProfile { provider, model, execution_mode })
    }
}

fn profile_error(
    capability: &CapabilityName,
    error: impl std::fmt::Display,
) -> CapabilityExecError {
    profile_error_message(capability, error.to_string())
}

fn profile_error_message(
    capability: &CapabilityName,
    detail: impl Into<String>,
) -> CapabilityExecError {
    CapabilityExecError::ProfileResolution {
        capability: capability.clone(),
        detail: CapabilityFailureDetail::new(detail),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use std::{fs, path::PathBuf, sync::Arc};

    use super::AgentProfilesCapabilityAdapter;
    use crate::agent_profiles::{AgentProfiles, ExecutionModeDto};
    use usecase::capability_exec::{
        BriefingText, CapabilityExecError, CapabilityExecInteractor, CapabilityExecRequest,
        CapabilityExecService, CapabilityFilePath, CapabilityProfilePort, CapabilitySourcePort,
        DisciplineText, ExecutionMode, ProviderName,
    };
    use usecase::dry_write_driver::CapabilityName;

    struct StaticCapabilitySource;

    impl CapabilitySourcePort for StaticCapabilitySource {
        fn load_briefing(
            &self,
            _: &CapabilityFilePath,
        ) -> Result<BriefingText, CapabilityExecError> {
            Ok(BriefingText::try_new("perform the task".to_owned())
                .expect("fixed test briefing is valid"))
        }

        fn load_discipline(&self) -> Result<DisciplineText, CapabilityExecError> {
            Ok(DisciplineText::try_new("do not stage changes".to_owned())
                .expect("fixed test discipline is valid"))
        }
    }

    fn write_profiles(content: &str) -> tempfile::TempDir {
        let directory = tempfile::tempdir().expect("test directory is created");
        fs::write(directory.path().join("agent-profiles.json"), content)
            .expect("test profile file is written");
        directory
    }

    #[test]
    fn test_agent_profiles_adapter_orchestrator_output_resolves_profile() {
        let directory = write_profiles(
            r#"{
                "schema_version": 1,
                "providers": {},
                "capabilities": {
                    "implementer": {
                        "provider": "codex",
                        "model": "gpt-5",
                        "execution_mode": "orchestrator-output"
                    }
                }
            }"#,
        );
        let adapter = AgentProfilesCapabilityAdapter::new(
            directory.path().to_path_buf(),
            directory.path().join("agent-profiles.json"),
        );
        let capability = CapabilityName::try_new("implementer").expect("valid test capability");

        let profile = adapter.resolve(&capability).expect("profile resolves");

        assert_eq!(profile.provider.as_str(), "codex");
        assert_eq!(profile.model.as_str(), "gpt-5");
        assert_eq!(profile.execution_mode, ExecutionMode::OrchestratorOutput);
    }

    #[test]
    fn test_agent_profiles_adapter_unsupported_provider_rejected_before_adapter_dispatch()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = write_profiles(
            r#"{
                "schema_version": 1,
                "providers": {
                    "gemini": {"label": "Gemini"}
                },
                "capabilities": {
                    "implementer": {
                        "provider": "gemini",
                        "model": "gemini-2.5-pro",
                        "execution_mode": "orchestrator-output"
                    }
                }
            }"#,
        );
        let interactor = CapabilityExecInteractor::new(
            Arc::new(AgentProfilesCapabilityAdapter::new(
                directory.path().to_path_buf(),
                directory.path().join("agent-profiles.json"),
            )),
            Arc::new(StaticCapabilitySource),
            Vec::new(),
        );
        let request = CapabilityExecRequest {
            capability: CapabilityName::try_new("implementer")?,
            host: ProviderName::try_new("codex")?,
            briefing_file: CapabilityFilePath::try_new(PathBuf::from("tmp/briefing.md"))?,
            timeout: None,
        };

        assert!(matches!(
            interactor.execute(request),
            Err(CapabilityExecError::UnsupportedProvider { provider })
                if provider.as_str() == "gemini"
        ));
        Ok(())
    }

    #[test]
    fn test_agent_profiles_adapter_missing_capability_returns_profile_resolution() {
        let directory = write_profiles(
            r#"{
                "schema_version": 1,
                "providers": {},
                "capabilities": {}
            }"#,
        );
        let adapter = AgentProfilesCapabilityAdapter::new(
            directory.path().to_path_buf(),
            directory.path().join("agent-profiles.json"),
        );
        let capability = CapabilityName::try_new("missing").expect("valid test capability");

        assert!(matches!(
            adapter.resolve(&capability),
            Err(CapabilityExecError::ProfileResolution { .. })
        ));
    }

    #[test]
    fn test_agent_profiles_adapter_missing_model_returns_model_missing() {
        let directory = write_profiles(
            r#"{
                "schema_version": 1,
                "providers": {},
                "capabilities": {
                    "implementer": {
                        "provider": "codex",
                        "execution_mode": "orchestrator-output"
                    }
                }
            }"#,
        );
        let adapter = AgentProfilesCapabilityAdapter::new(
            directory.path().to_path_buf(),
            directory.path().join("agent-profiles.json"),
        );
        let capability = CapabilityName::try_new("implementer").expect("valid test capability");

        assert!(matches!(
            adapter.resolve(&capability),
            Err(CapabilityExecError::ModelMissing { .. })
        ));
    }

    #[test]
    fn test_agent_profiles_adapter_typed_pipeline_returns_profile_resolution() {
        let directory = write_profiles(
            r#"{
                "schema_version": 1,
                "providers": {},
                "capabilities": {
                    "reviewer": {
                        "provider": "claude",
                        "model": "claude-opus",
                        "execution_mode": "typed-pipeline"
                    }
                }
            }"#,
        );
        let adapter = AgentProfilesCapabilityAdapter::new(
            directory.path().to_path_buf(),
            directory.path().join("agent-profiles.json"),
        );
        let capability = CapabilityName::try_new("reviewer").expect("valid test capability");

        assert!(matches!(
            adapter.resolve(&capability),
            Err(CapabilityExecError::ProfileResolution { .. })
        ));
    }

    #[test]
    fn test_agent_profiles_adapter_missing_provider_returns_profile_resolution() {
        let directory = write_profiles(
            r#"{
                "schema_version": 1,
                "providers": {},
                "capabilities": {
                    "implementer": {
                        "model": "gpt-5",
                        "execution_mode": "orchestrator-output"
                    }
                }
            }"#,
        );
        let adapter = AgentProfilesCapabilityAdapter::new(
            directory.path().to_path_buf(),
            directory.path().join("agent-profiles.json"),
        );
        let capability = CapabilityName::try_new("implementer").expect("valid test capability");

        assert!(matches!(
            adapter.resolve(&capability),
            Err(CapabilityExecError::ProfileResolution { .. })
        ));
    }

    #[test]
    fn test_agent_profiles_adapter_unknown_execution_mode_returns_profile_resolution() {
        let directory = write_profiles(
            r#"{
                "schema_version": 1,
                "providers": {},
                "capabilities": {
                    "implementer": {
                        "provider": "codex",
                        "model": "gpt-5",
                        "execution_mode": "future-mode"
                    }
                }
            }"#,
        );
        let adapter = AgentProfilesCapabilityAdapter::new(
            directory.path().to_path_buf(),
            directory.path().join("agent-profiles.json"),
        );
        let capability = CapabilityName::try_new("implementer").expect("valid test capability");

        assert!(matches!(
            adapter.resolve(&capability),
            Err(CapabilityExecError::ProfileResolution { .. })
        ));
    }

    #[test]
    fn test_capability_config_dto_missing_execution_mode_is_rejected() {
        let directory = write_profiles(
            r#"{
                "schema_version": 1,
                "providers": {},
                "capabilities": {
                    "reviewer": {
                        "provider": "claude",
                        "model": "claude-opus"
                    }
                }
            }"#,
        );
        assert!(matches!(
            AgentProfiles::load(directory.path(), &directory.path().join("agent-profiles.json"),),
            Err(crate::agent_profiles::AgentProfilesError::Parse(_))
        ));
    }

    #[test]
    fn test_capability_config_dto_orchestrator_output_is_distinct_from_typed_pipeline() {
        let directory = write_profiles(
            r#"{
                "schema_version": 1,
                "providers": {},
                "capabilities": {
                    "implementer": {
                        "provider": "codex",
                        "model": "gpt-5",
                        "execution_mode": "orchestrator-output"
                    },
                    "reviewer": {
                        "provider": "claude",
                        "model": "claude-opus",
                        "execution_mode": "typed-pipeline"
                    }
                }
            }"#,
        );
        let profiles =
            AgentProfiles::load(directory.path(), &directory.path().join("agent-profiles.json"))
                .expect("profiles load");

        let implementer =
            profiles.resolve_capability("implementer").expect("implementer config exists");
        let reviewer = profiles.resolve_capability("reviewer").expect("reviewer config exists");

        assert_eq!(implementer.execution_mode(), ExecutionModeDto::OrchestratorOutput);
        assert_eq!(reviewer.execution_mode(), ExecutionModeDto::TypedPipeline);
    }

    #[test]
    fn test_agent_profiles_adapter_out_of_root_config_returns_profile_resolution() {
        let trusted_workspace = tempfile::tempdir().expect("trusted workspace is created");
        let outside = write_profiles(
            r#"{
                "schema_version": 1,
                "providers": {},
                "capabilities": {
                    "implementer": {
                        "provider": "codex",
                        "model": "gpt-5",
                        "execution_mode": "orchestrator-output"
                    }
                }
            }"#,
        );
        let adapter = AgentProfilesCapabilityAdapter::new(
            trusted_workspace.path().to_path_buf(),
            outside.path().join("agent-profiles.json"),
        );
        let capability = CapabilityName::try_new("implementer").expect("valid test capability");

        assert!(matches!(
            adapter.resolve(&capability),
            Err(CapabilityExecError::ProfileResolution { .. })
        ));
    }
}
