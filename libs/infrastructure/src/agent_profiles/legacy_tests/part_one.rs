use super::*;
use std::io::Write as _;

use crate::capability_exec::{parse_provider_definition_front_matter, read_front_matter};

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
    assert_eq!(io.to_string(), "failed to read agent profiles at agent-profiles.json: read denied");

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
fn test_shipped_sample_profiles_when_loaded_parse_successfully() {
    let workspace_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let samples_dir = workspace_root.join(".harness/config/samples");

    for sample in [
        "agent-profiles.default.json",
        "agent-profiles.claude-heavy.json",
        "agent-profiles.codex-heavy.json",
    ] {
        let path = samples_dir.join(sample);
        let result = AgentProfiles::load(&workspace_root, &path);

        assert!(result.is_ok(), "shipped sample {sample} must load: {result:?}");
    }
}

#[test]
fn test_shipped_claude_output_profiles_when_preflighted_pass_agent_validation() {
    let workspace_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let samples_dir = workspace_root.join(".harness/config/samples");

    for sample in ["agent-profiles.default.json", "agent-profiles.claude-heavy.json"] {
        let profiles = AgentProfiles::load(&workspace_root, &samples_dir.join(sample))
            .expect("shipped sample profile must load");
        let claude_outputs: Vec<_> = profiles
            .capabilities
            .iter()
            .filter(|(_, config)| {
                config.provider() == "claude"
                    && config.execution_mode() == ExecutionModeDto::OrchestratorOutput
            })
            .collect();

        assert!(
            !claude_outputs.is_empty(),
            "shipped sample {sample} must contain a Claude orchestrator-output capability"
        );

        for (capability, config) in claude_outputs {
            let path =
                workspace_root.join(".claude").join("agents").join(format!("{capability}.md"));
            let definition = std::fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("{sample} {capability} agent must exist: {error}"));
            let yaml = read_front_matter(&definition)
                .unwrap_or_else(|error| {
                    panic!("{sample} {capability} agent front matter must parse: {error}")
                })
                .unwrap_or_else(|| {
                    panic!("{sample} {capability} agent must declare YAML front matter")
                });
            let front_matter =
                parse_provider_definition_front_matter(yaml).unwrap_or_else(|error| {
                    panic!("{sample} {capability} agent YAML must parse: {error}")
                });

            assert!(
                front_matter.has_tools(),
                "{sample} {capability} agent must declare non-empty tools"
            );
            assert_eq!(
                front_matter.model(),
                config.model(),
                "{sample} {capability} agent model must match the profile exactly"
            );
        }
    }
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

    let err =
        AgentProfiles::load(&repository, &repository.join(".harness/config/agent-profiles.json"))
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
    let path = repository.join("apps/cli-composition/../../.harness/config/agent-profiles.json");

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
