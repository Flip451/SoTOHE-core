use std::fs;

use super::*;
use usecase::capability_exec::ReasoningEffort;
use usecase::dry_write_driver::CapabilityName;

fn capability(value: &str) -> CapabilityName {
    CapabilityName::try_new(value).expect("test capability names are valid")
}

fn load_profiles(contents: &str) -> AgentProfiles {
    let directory = tempfile::tempdir().expect("test directory is created");
    let path = directory.path().join("agent-profiles.json");
    fs::write(&path, contents).expect("test profile is written");
    AgentProfiles::load(directory.path(), &path).expect("test profile loads")
}

const CODEX_PROFILE: &str = r#"{
    "schema_version": 1,
    "providers": { "codex": { "label": "Codex CLI" } },
    "capabilities": {
        "reviewer": {
            "provider": "codex",
            "model": "gpt-5.6-sol",
            "fast_model": "gpt-5.6-luna",
            "reasoning_effort": "xhigh",
            "fast_reasoning_effort": "low",
            "execution_mode": "typed-pipeline"
        }
    }
}"#;

#[test]
fn test_resolve_execution_final_provider_cli_has_explicit_effort() {
    let profiles = load_profiles(CODEX_PROFILE);
    let execution = profiles
        .resolve_execution(&capability("reviewer"), RoundType::Final)
        .expect("final profile resolves");

    assert!(matches!(
        execution,
        ResolvedExecution::ProviderCli { provider, model, effort }
            if provider.as_str() == "codex"
                && model.as_str() == "gpt-5.6-sol"
                && effort == ReasoningEffort::XHigh
    ));
}

#[test]
fn test_resolve_execution_missing_fast_effort_returns_error() {
    let profiles = load_profiles(
        r#"{
            "schema_version": 1,
            "providers": { "codex": { "label": "Codex CLI" } },
            "capabilities": {
                "reviewer": {
                    "provider": "codex",
                    "model": "gpt-5.6-sol",
                    "fast_model": "gpt-5.6-luna",
                    "reasoning_effort": "xhigh",
                    "execution_mode": "typed-pipeline"
                }
            }
        }"#,
    );

    assert!(matches!(
        profiles.resolve_execution(&capability("reviewer"), RoundType::Fast),
        Err(AgentProfilesError::EffortMissing(name, RoundType::Fast)) if name.as_str() == "reviewer"
    ));
}

#[test]
fn test_resolve_execution_missing_final_effort_returns_error() {
    let profiles = load_profiles(
        r#"{
            "schema_version": 1,
            "providers": { "codex": { "label": "Codex CLI" } },
            "capabilities": {
                "reviewer": {
                    "provider": "codex",
                    "model": "gpt-5.6-sol",
                    "fast_model": "gpt-5.6-luna",
                    "fast_reasoning_effort": "low",
                    "execution_mode": "typed-pipeline"
                }
            }
        }"#,
    );

    assert!(matches!(
        profiles.resolve_execution(&capability("reviewer"), RoundType::Final),
        Err(AgentProfilesError::EffortMissing(name, RoundType::Final)) if name.as_str() == "reviewer"
    ));
}

#[test]
fn test_resolve_execution_unsupported_provider_effort_returns_error() {
    let profiles = load_profiles(
        r#"{
            "schema_version": 1,
            "providers": { "claude": { "label": "Claude Code" } },
            "capabilities": {
                "implementer": {
                    "provider": "claude",
                    "model": "claude-opus-4-7",
                    "reasoning_effort": "xhigh",
                    "execution_mode": "orchestrator-output"
                }
            }
        }"#,
    );

    assert!(matches!(
        profiles.resolve_execution(&capability("implementer"), RoundType::Final),
        Err(AgentProfilesError::UnsupportedEffort(provider, ReasoningEffort::XHigh))
            if provider.as_str() == "claude"
    ));
}

#[test]
fn test_resolve_execution_pr_reviewer_is_hosted_service_exempt() {
    let profiles = load_profiles(
        r#"{
            "schema_version": 1,
            "providers": { "codex": { "label": "Codex CLI" } },
            "capabilities": {
                "pr-reviewer": {
                    "provider": "codex",
                    "execution_mode": "typed-pipeline"
                }
            }
        }"#,
    );

    assert!(matches!(
        profiles.resolve_execution(&capability("pr-reviewer"), RoundType::Final),
        Ok(ResolvedExecution::HostedService { provider }) if provider.as_str() == "codex"
    ));
}

#[test]
fn test_capability_config_final_effort_alias_maps_to_typed_accessor() {
    let profiles = load_profiles(
        r#"{
            "schema_version": 1,
            "providers": { "codex": { "label": "Codex CLI" } },
            "capabilities": {
                "dry-checker": {
                    "provider": "codex",
                    "model": "gpt-5.6-terra",
                    "fast_model": "gpt-5.6-luna",
                    "final_reasoning_effort": "high",
                    "fast_reasoning_effort": "medium",
                    "execution_mode": "typed-pipeline"
                }
            }
        }"#,
    );
    let config =
        profiles.resolve_capability(&capability("dry-checker")).expect("dry-checker is configured");

    assert_eq!(config.effort(), Some(ReasoningEffortDto::High));
    assert_eq!(config.fast_effort(), Some(ReasoningEffortDto::Medium));
}

#[test]
fn test_committed_profiles_subprocess_entries_resolve_explicit_effort() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent()
        .and_then(std::path::Path::parent)
        .expect("infrastructure crate is nested under the workspace");
    let profiles = AgentProfiles::load(workspace_root, &workspace_root.join(AGENT_PROFILES_PATH))
        .expect("committed profile loads");

    for capability_name in [
        "orchestrator",
        "spec-designer",
        "impl-planner",
        "type-designer",
        "adr-editor",
        "rollback-diagnoser",
        "implementer",
        "researcher",
        "review-fix-lead",
        "dry-checker",
        "dry-fix-lead",
        "ref-verifier-chain1",
        "ref-verifier-chain2",
        "obligation-fulfillment-verifier",
        "waiver-verifier",
    ] {
        assert!(matches!(
            profiles.resolve_execution(&capability(capability_name), RoundType::Final),
            Ok(ResolvedExecution::ProviderCli { .. })
        ));
    }
    assert!(matches!(
        profiles.resolve_execution(&capability("reviewer"), RoundType::Fast),
        Ok(ResolvedExecution::ProviderCli { effort: ReasoningEffort::Low, .. })
    ));
    assert!(matches!(
        profiles.resolve_execution(&capability("reviewer"), RoundType::Final),
        Ok(ResolvedExecution::ProviderCli { effort: ReasoningEffort::XHigh, .. })
    ));
    assert!(matches!(
        profiles.resolve_execution(&capability("pr-reviewer"), RoundType::Final),
        Ok(ResolvedExecution::HostedService { .. })
    ));
}

#[test]
fn test_committed_and_default_profiles_resolve_full_cli_effort_contract() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent()
        .and_then(std::path::Path::parent)
        .expect("infrastructure crate is nested under the workspace");
    let committed = AgentProfiles::load(workspace_root, &workspace_root.join(AGENT_PROFILES_PATH))
        .expect("committed profile loads");
    let default = AgentProfiles::load(
        workspace_root,
        &workspace_root.join(".harness/config/samples/agent-profiles.default.json"),
    )
    .expect("default sample profile loads");

    for profiles in [&committed, &default] {
        for capability_name in profiles.capabilities.keys() {
            let resolved =
                profiles.resolve_execution(&capability(capability_name), RoundType::Final);

            if capability_name == "pr-reviewer" {
                assert!(matches!(resolved, Ok(ResolvedExecution::HostedService { .. })));
                continue;
            }

            assert!(matches!(
                resolved,
                Ok(ResolvedExecution::ProviderCli {
                    effort: ReasoningEffort::Low
                        | ReasoningEffort::Medium
                        | ReasoningEffort::High
                        | ReasoningEffort::XHigh
                        | ReasoningEffort::Max,
                    ..
                })
            ));
        }
        assert!(matches!(
            profiles.resolve_execution(&capability("reviewer"), RoundType::Fast),
            Ok(ResolvedExecution::ProviderCli { effort: ReasoningEffort::Low, .. })
        ));
        assert!(matches!(
            profiles.resolve_execution(&capability("reviewer"), RoundType::Final),
            Ok(ResolvedExecution::ProviderCli { effort: ReasoningEffort::XHigh, .. })
        ));
    }

    for capability_name in ["ref-verifier-chain1", "ref-verifier-chain2"] {
        assert!(matches!(
            committed.resolve_execution(&capability(capability_name), RoundType::Fast),
            Ok(ResolvedExecution::ProviderCli { effort: ReasoningEffort::Low, .. })
        ));
        assert!(matches!(
            committed.resolve_execution(&capability(capability_name), RoundType::Final),
            Ok(ResolvedExecution::ProviderCli { effort: ReasoningEffort::XHigh, .. })
        ));
    }
}

#[test]
fn test_resolve_execution_non_pr_reviewer_missing_final_effort_returns_error() {
    let profiles = load_profiles(
        r#"{
            "schema_version": 1,
            "providers": { "codex": { "label": "Codex CLI" } },
            "capabilities": {
                "implementer": {
                    "provider": "codex",
                    "model": "gpt-5.6-terra",
                    "execution_mode": "orchestrator-output"
                }
            }
        }"#,
    );

    assert!(matches!(
        profiles.resolve_execution(&capability("implementer"), RoundType::Final),
        Err(AgentProfilesError::EffortMissing(name, RoundType::Final))
            if name.as_str() == "implementer"
    ));
}

#[test]
fn test_reasoning_effort_dto_deserializes_closed_vocabulary_including_xhigh() {
    for (encoded, expected) in [
        (r#""low""#, ReasoningEffortDto::Low),
        (r#""medium""#, ReasoningEffortDto::Medium),
        (r#""high""#, ReasoningEffortDto::High),
        (r#""xhigh""#, ReasoningEffortDto::XHigh),
        (r#""max""#, ReasoningEffortDto::Max),
    ] {
        let decoded: ReasoningEffortDto =
            serde_json::from_str(encoded).expect("supported effort vocabulary deserializes");
        assert_eq!(decoded, expected);
    }

    assert!(serde_json::from_str::<ReasoningEffortDto>(r#""provider-default""#).is_err());
}
