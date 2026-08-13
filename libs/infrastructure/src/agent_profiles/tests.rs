use std::fs;

use super::*;
use usecase::capability_exec::{
    CapabilityInputValidationError, CapabilityProviderBinding, ReasoningEffort,
};
use usecase::dry_write_driver::CapabilityName;

fn capability(value: &str) -> CapabilityName {
    CapabilityName::try_new(value).expect("test capability names are valid")
}

fn load_profiles(contents: &str) -> AgentProfiles {
    load_profiles_result(contents).expect("test profile loads")
}

fn load_profiles_result(contents: &str) -> Result<AgentProfiles, AgentProfilesError> {
    let directory = tempfile::tempdir().expect("test directory is created");
    let path = directory.path().join("agent-profiles.json");
    fs::write(&path, contents).expect("test profile is written");
    AgentProfiles::load(directory.path(), &path)
}

const CODEX_PROFILE: &str = r#"{
    "schema_version": 1,
    "providers": {
        "codex": {
            "label": "Codex CLI",
            "supported_reasoning_efforts": ["low", "medium", "high", "xhigh", "max"]
        }
    },
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
fn test_capability_config_codex_model_provider_decodes_to_custom_binding() {
    let profiles = load_profiles(
        r#"{
            "schema_version": 1,
            "providers": { "codex": { "label": "Codex CLI", "supported_reasoning_efforts": ["high"] } },
            "capabilities": {
                "implementer": {
                    "provider": "codex",
                    "model_provider": "deepseek",
                    "model": "deepseek-chat",
                    "reasoning_effort": "high",
                    "execution_mode": "orchestrator-output"
                }
            }
        }"#,
    );
    let config = profiles
        .resolve_capability(&capability("implementer"))
        .expect("implementer configuration exists");

    assert!(matches!(
        config.provider_binding(),
        CapabilityProviderBindingDto::CodexCustom(model_provider)
            if model_provider.clone().into_domain().as_str() == "deepseek"
    ));
}

#[test]
fn test_capability_config_without_model_provider_decodes_to_standard_binding() {
    let profiles = load_profiles(CODEX_PROFILE);
    let config = profiles
        .resolve_capability(&capability("reviewer"))
        .expect("reviewer configuration exists");

    assert!(matches!(
        config.provider_binding(),
        CapabilityProviderBindingDto::Standard(provider)
            if provider.clone().into_domain().as_str() == "codex"
    ));
}

#[test]
fn test_model_provider_name_dto_accepts_arbitrary_non_empty_value_without_interpretation() {
    let dto: ModelProviderNameDto = serde_json::from_str(r#""consumer-defined-provider-id""#)
        .expect("model provider DTO deserializes");

    assert_eq!(dto.into_domain().as_str(), "consumer-defined-provider-id");
}

#[test]
fn test_model_provider_name_dto_deserialization_rejects_empty_value() {
    let result = serde_json::from_str::<ModelProviderNameDto>(r#"""#);

    assert!(result.is_err(), "empty model_provider values must fail DTO deserialization");
}

#[test]
fn test_model_provider_name_dto_try_new_rejects_whitespace() {
    assert!(matches!(
        ModelProviderNameDto::try_new(" \t\n ".to_owned()),
        Err(CapabilityInputValidationError::EmptyModelProviderName)
    ));
}

#[test]
fn test_capability_provider_binding_dto_converts_to_codex_custom_binding() {
    let dto = CapabilityProviderBindingDto::CodexCustom(
        ModelProviderNameDto::try_new("glm".to_owned()).expect("model provider is valid"),
    );

    assert!(matches!(
        dto.into_domain(),
        CapabilityProviderBinding::CodexCustom(model_provider)
            if model_provider.as_str() == "glm"
    ));
}

#[test]
fn test_capability_config_model_provider_with_non_codex_provider_is_rejected() {
    let result = load_profiles_result(
        r#"{
            "schema_version": 1,
            "providers": { "claude": { "label": "Claude Code", "supported_reasoning_efforts": ["high"] } },
            "capabilities": {
                "implementer": {
                    "provider": "claude",
                    "model_provider": "deepseek",
                    "model": "deepseek-chat",
                    "reasoning_effort": "high",
                    "execution_mode": "orchestrator-output"
                }
            }
        }"#,
    );

    assert!(matches!(result, Err(AgentProfilesError::Parse(_))));
}

#[test]
fn test_capability_config_empty_model_provider_is_rejected() {
    let result = load_profiles_result(
        r#"{
            "schema_version": 1,
            "providers": { "codex": { "label": "Codex CLI", "supported_reasoning_efforts": ["high"] } },
            "capabilities": {
                "implementer": {
                    "provider": "codex",
                    "model_provider": " \t ",
                    "model": "deepseek-chat",
                    "reasoning_effort": "high",
                    "execution_mode": "orchestrator-output"
                }
            }
        }"#,
    );

    assert!(matches!(result, Err(AgentProfilesError::Parse(_))));
}

#[test]
fn test_resolve_execution_codex_custom_binding_keeps_codex_provider() {
    let profiles = load_profiles(
        r#"{
            "schema_version": 1,
            "providers": { "codex": { "label": "Codex CLI", "supported_reasoning_efforts": ["high"] } },
            "capabilities": {
                "implementer": {
                    "provider": "codex",
                    "model_provider": "qwen",
                    "model": "qwen-max",
                    "reasoning_effort": "high",
                    "execution_mode": "orchestrator-output"
                }
            }
        }"#,
    );

    assert!(matches!(
        profiles.resolve_execution(&capability("implementer"), RoundType::Final),
        Ok(ResolvedExecution::ProviderCli { provider, .. }) if provider.as_str() == "codex"
    ));
}

#[test]
fn test_resolve_execution_fast_round_selects_the_fast_model_and_effort() {
    // `CODEX_PROFILE` gives the two round types distinct models and distinct
    // efforts, so an implementation that requires `fast_reasoning_effort` to be
    // present but then hands back the final round's `reasoning_effort` — or the
    // final model — fails here. The absence test below cannot see that: it only
    // establishes that a missing fast effort is refused, which such an
    // implementation would still do.
    let profiles = load_profiles(CODEX_PROFILE);
    let execution = profiles
        .resolve_execution(&capability("reviewer"), RoundType::Fast)
        .expect("fast profile resolves");

    assert!(matches!(
        execution,
        ResolvedExecution::ProviderCli { provider, model, effort }
            if provider.as_str() == "codex"
                && model.as_str() == "gpt-5.6-luna"
                && effort == ReasoningEffort::Low
    ));
}

#[test]
fn test_resolve_execution_missing_fast_effort_returns_error() {
    let profiles = load_profiles(
        r#"{
            "schema_version": 1,
            "providers": { "codex": { "label": "Codex CLI", "supported_reasoning_efforts": ["low", "medium", "high", "xhigh", "max"] } },
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
            "providers": { "codex": { "label": "Codex CLI", "supported_reasoning_efforts": ["low", "medium", "high", "xhigh", "max"] } },
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
fn test_provider_without_effort_list_is_rejected() {
    let result = load_profiles_result(
        r#"{
            "schema_version": 1,
            "providers": { "codex": { "label": "Codex CLI" } },
            "capabilities": {
                "implementer": {
                    "provider": "codex",
                    "model": "gpt-5.6-terra",
                    "reasoning_effort": "high",
                    "execution_mode": "orchestrator-output"
                }
            }
        }"#,
    );

    assert!(matches!(result, Err(AgentProfilesError::Parse(_))));
}

#[test]
fn test_resolve_execution_unsupported_provider_effort_returns_error() {
    let profiles = load_profiles(
        r#"{
            "schema_version": 1,
            "providers": {
                "gemini": {
                    "label": "Gemini CLI",
                    "supported_reasoning_efforts": ["low", "medium", "high"]
                }
            },
            "capabilities": {
                "researcher": {
                    "provider": "gemini",
                    "model": "gemini-3-pro",
                    "reasoning_effort": "xhigh",
                    "execution_mode": "orchestrator-output"
                }
            }
        }"#,
    );

    assert!(matches!(
        profiles.resolve_execution(&capability("researcher"), RoundType::Final),
        Err(AgentProfilesError::UnsupportedEffort(provider, ReasoningEffort::XHigh))
            if provider.as_str() == "gemini"
    ));
}

#[test]
fn test_resolve_execution_codex_max_returns_provider_cli() {
    let profiles = load_profiles(
        r#"{
            "schema_version": 1,
            "providers": {
                "codex": {
                    "label": "Codex CLI",
                    "supported_reasoning_efforts": ["max"]
                }
            },
            "capabilities": {
                "implementer": {
                    "provider": "codex",
                    "model": "gpt-5.6-luna",
                    "reasoning_effort": "max",
                    "execution_mode": "orchestrator-output"
                }
            }
        }"#,
    );

    assert!(matches!(
        profiles.resolve_execution(&capability("implementer"), RoundType::Final),
        Ok(ResolvedExecution::ProviderCli { provider, effort, .. })
            if provider.as_str() == "codex" && effort == ReasoningEffort::Max
    ));
}

#[test]
fn test_resolve_execution_claude_accepts_every_effort_level() {
    for (encoded, expected) in [
        ("low", ReasoningEffort::Low),
        ("medium", ReasoningEffort::Medium),
        ("high", ReasoningEffort::High),
        ("xhigh", ReasoningEffort::XHigh),
        ("max", ReasoningEffort::Max),
    ] {
        let profiles = load_profiles(&format!(
            r#"{{
                "schema_version": 1,
                "providers": {{
                    "claude": {{
                        "label": "Claude Code",
                        "supported_reasoning_efforts": ["low", "medium", "high", "xhigh", "max"]
                    }}
                }},
                "capabilities": {{
                    "implementer": {{
                        "provider": "claude",
                        "model": "claude-opus-5",
                        "reasoning_effort": "{encoded}",
                        "execution_mode": "orchestrator-output"
                    }}
                }}
            }}"#
        ));

        assert!(matches!(
            profiles.resolve_execution(&capability("implementer"), RoundType::Final),
            Ok(ResolvedExecution::ProviderCli { provider, effort, .. })
                if provider.as_str() == "claude" && effort == expected
        ));
    }
}

#[test]
fn test_resolve_execution_provider_declaration_rejects_undeclared_effort() {
    let profiles = load_profiles(
        r#"{
            "schema_version": 1,
            "providers": {
                "claude": {
                    "label": "Claude Code",
                    "supported_reasoning_efforts": ["low"]
                }
            },
            "capabilities": {
                "implementer": {
                    "provider": "claude",
                    "model": "claude-opus-5",
                    "reasoning_effort": "high",
                    "execution_mode": "orchestrator-output"
                }
            }
        }"#,
    );

    assert!(matches!(
        profiles.resolve_execution(&capability("implementer"), RoundType::Final),
        Err(AgentProfilesError::UnsupportedEffort(provider, ReasoningEffort::High))
            if provider.as_str() == "claude"
    ));
}

#[test]
fn test_resolve_execution_explicit_empty_provider_effort_list_rejects_all_efforts() {
    let profiles = load_profiles(
        r#"{
            "schema_version": 1,
            "providers": {
                "codex": {
                    "label": "Codex CLI",
                    "supported_reasoning_efforts": []
                }
            },
            "capabilities": {
                "implementer": {
                    "provider": "codex",
                    "model": "gpt-5.6-luna",
                    "reasoning_effort": "max",
                    "execution_mode": "orchestrator-output"
                }
            }
        }"#,
    );

    assert!(matches!(
        profiles.resolve_execution(&capability("implementer"), RoundType::Final),
        Err(AgentProfilesError::UnsupportedEffort(provider, ReasoningEffort::Max))
            if provider.as_str() == "codex"
    ));
}

#[test]
fn test_resolve_execution_custom_provider_declares_max_returns_provider_cli() {
    let profiles = load_profiles(
        r#"{
            "schema_version": 1,
            "providers": {
                "custom": {
                    "label": "Custom CLI",
                    "supported_reasoning_efforts": ["max"]
                }
            },
            "capabilities": {
                "implementer": {
                    "provider": "custom",
                    "model": "custom-reasoner",
                    "reasoning_effort": "max",
                    "execution_mode": "orchestrator-output"
                }
            }
        }"#,
    );

    assert!(matches!(
        profiles.resolve_execution(&capability("implementer"), RoundType::Final),
        Ok(ResolvedExecution::ProviderCli { provider, effort, .. })
            if provider.as_str() == "custom" && effort == ReasoningEffort::Max
    ));
}

#[test]
fn test_resolve_execution_pr_reviewer_is_hosted_service_exempt() {
    let profiles = load_profiles(
        r#"{
            "schema_version": 1,
            "providers": { "codex": { "label": "Codex CLI", "supported_reasoning_efforts": ["low", "medium", "high", "xhigh", "max"] } },
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
            "providers": { "codex": { "label": "Codex CLI", "supported_reasoning_efforts": ["low", "medium", "high", "xhigh", "max"] } },
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
    // Both round types resolve an explicit effort. The values themselves are
    // tunable configuration, so pinning them here would make every change to
    // the committed profile a test edit; that the fast round reads the `fast_*`
    // fields at all is established by the fixture tests above, which observe
    // `EffortMissing(_, Fast)` when they are absent.
    for round_type in [RoundType::Fast, RoundType::Final] {
        assert!(matches!(
            profiles.resolve_execution(&capability("reviewer"), round_type),
            Ok(ResolvedExecution::ProviderCli { .. })
        ));
    }
    assert!(matches!(
        profiles.resolve_execution(&capability("pr-reviewer"), RoundType::Final),
        Ok(ResolvedExecution::HostedService { .. })
    ));
}

// NOTE: no test may pin the committed profile's tunable values (provider /
// model / effort). `.harness/config/agent-profiles.json` is consumer-owned
// configuration that must be changeable without touching Rust code; tests
// against the committed file assert only the structural contract (every
// capability resolves, efforts are valid), as
// `test_committed_profiles_subprocess_entries_resolve_explicit_effort` does.

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
        // Contract only, not the tunable values — see the NOTE above this
        // test about never pinning committed-profile configuration.
        for round_type in [RoundType::Fast, RoundType::Final] {
            assert!(matches!(
                profiles.resolve_execution(&capability("reviewer"), round_type),
                Ok(ResolvedExecution::ProviderCli { .. })
            ));
        }
    }

    // Both round types must resolve for the chain verifiers; the effort values
    // themselves are tunable configuration and are deliberately not pinned.
    for capability_name in ["ref-verifier-chain1", "ref-verifier-chain2"] {
        for round_type in [RoundType::Fast, RoundType::Final] {
            assert!(matches!(
                committed.resolve_execution(&capability(capability_name), round_type),
                Ok(ResolvedExecution::ProviderCli { .. })
            ));
        }
    }
}

#[test]
fn test_resolve_execution_non_pr_reviewer_missing_final_effort_returns_error() {
    let profiles = load_profiles(
        r#"{
            "schema_version": 1,
            "providers": { "codex": { "label": "Codex CLI", "supported_reasoning_efforts": ["low", "medium", "high", "xhigh", "max"] } },
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
