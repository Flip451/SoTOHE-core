use std::{fs, path::Path};

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

fn workspace_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("infrastructure crate is nested under the workspace")
}

fn load_shipped_profile(relative_path: &str) -> AgentProfiles {
    let root = workspace_root();
    AgentProfiles::load(root, &root.join(relative_path)).expect("shipped profile loads")
}

const SHIPPED_PROFILE_PATHS: &[&str] = &[
    AGENT_PROFILES_PATH,
    ".harness/config/samples/agent-profiles.default.json",
    ".harness/config/samples/agent-profiles.claude-heavy.json",
    ".harness/config/samples/agent-profiles.codex-heavy.json",
    ".harness/config/samples/agent-profiles.grok-heavy.json",
];

const TYPED_PIPELINE_VERIFIER_CAPABILITIES: &[&str] = &[
    "ref-verifier-chain1",
    "ref-verifier-chain2",
    "obligation-fulfillment-verifier",
    "waiver-verifier",
];

#[derive(Debug, Clone, Copy)]
struct ShippedDefaultBaseline {
    capability: &'static str,
    provider: &'static str,
    model: Option<&'static str>,
    fast_provider: Option<&'static str>,
    fast_model: Option<&'static str>,
}

// These are the provider/model fields from the previous committed base. The
// current branch is allowed to change only reviewer.provider and reviewer.model.
const PREVIOUS_COMMITTED_SHIPPED_DEFAULTS: &[ShippedDefaultBaseline] = &[
    ShippedDefaultBaseline {
        capability: "orchestrator",
        provider: "grok",
        model: Some("grok-4.6"),
        fast_provider: None,
        fast_model: None,
    },
    ShippedDefaultBaseline {
        capability: "spec-designer",
        provider: "grok",
        model: Some("grok-4.6"),
        fast_provider: None,
        fast_model: None,
    },
    ShippedDefaultBaseline {
        capability: "impl-planner",
        provider: "grok",
        model: Some("grok-4.6"),
        fast_provider: None,
        fast_model: None,
    },
    ShippedDefaultBaseline {
        capability: "type-designer",
        provider: "grok",
        model: Some("grok-4.6"),
        fast_provider: None,
        fast_model: None,
    },
    ShippedDefaultBaseline {
        capability: "adr-editor",
        provider: "grok",
        model: Some("grok-4.6"),
        fast_provider: None,
        fast_model: None,
    },
    ShippedDefaultBaseline {
        capability: "rollback-diagnoser",
        provider: "grok",
        model: Some("grok-4.6"),
        fast_provider: None,
        fast_model: None,
    },
    ShippedDefaultBaseline {
        capability: "adr-diagnoser",
        provider: "grok",
        model: Some("grok-4.6"),
        fast_provider: None,
        fast_model: None,
    },
    ShippedDefaultBaseline {
        capability: "implementer",
        provider: "codex",
        model: Some("gpt-5.6-luna"),
        fast_provider: None,
        fast_model: None,
    },
    ShippedDefaultBaseline {
        capability: "reviewer",
        provider: "claude",
        model: Some("claude-fable-5"),
        fast_provider: Some("codex"),
        fast_model: Some("gpt-5.6-luna"),
    },
    ShippedDefaultBaseline {
        capability: "researcher",
        provider: "grok",
        model: Some("grok-4.6"),
        fast_provider: None,
        fast_model: None,
    },
    ShippedDefaultBaseline {
        capability: "review-fix-lead",
        provider: "grok",
        model: Some("grok-4.6"),
        fast_provider: None,
        fast_model: None,
    },
    ShippedDefaultBaseline {
        capability: "dry-checker",
        provider: "grok",
        model: Some("grok-4.6"),
        fast_provider: Some("codex"),
        fast_model: Some("gpt-5.6-luna"),
    },
    ShippedDefaultBaseline {
        capability: "dry-fix-lead",
        provider: "codex",
        model: Some("gpt-5.6-luna"),
        fast_provider: None,
        fast_model: None,
    },
    ShippedDefaultBaseline {
        capability: "pr-reviewer",
        provider: "codex",
        model: None,
        fast_provider: None,
        fast_model: None,
    },
    ShippedDefaultBaseline {
        capability: "ref-verifier-chain1",
        provider: "codex",
        model: Some("gpt-5.6-sol"),
        fast_provider: Some("codex"),
        fast_model: Some("gpt-5.6-luna"),
    },
    ShippedDefaultBaseline {
        capability: "ref-verifier-chain2",
        provider: "codex",
        model: Some("gpt-5.6-sol"),
        fast_provider: Some("codex"),
        fast_model: Some("gpt-5.6-luna"),
    },
    ShippedDefaultBaseline {
        capability: "obligation-fulfillment-verifier",
        provider: "codex",
        model: Some("gpt-5.6-terra"),
        fast_provider: Some("codex"),
        fast_model: Some("gpt-5.6-luna"),
    },
    ShippedDefaultBaseline {
        capability: "waiver-verifier",
        provider: "codex",
        model: Some("gpt-5.6-terra"),
        fast_provider: Some("codex"),
        fast_model: Some("gpt-5.6-luna"),
    },
];

fn configured_provider(config: &CapabilityConfigDto) -> String {
    match config.provider_binding() {
        CapabilityProviderBindingDto::Standard(provider) => {
            provider.clone().into_domain().as_str().to_owned()
        }
        CapabilityProviderBindingDto::CodexCustom(_) => "codex".to_owned(),
    }
}

fn configured_model(config: &CapabilityConfigDto) -> Option<String> {
    config.model().map(|model| model.clone().into_domain().as_str().to_owned())
}

fn configured_fast_provider(config: &CapabilityConfigDto) -> Option<String> {
    config.fast_provider().map(|provider| provider.clone().into_domain().as_str().to_owned())
}

fn configured_fast_model(config: &CapabilityConfigDto) -> Option<String> {
    config.fast_model().map(|model| model.clone().into_domain().as_str().to_owned())
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

// The structural tests above keep the general consumer-owned profile contract
// flexible. The T002 tests below intentionally pin the shipped defaults whose
// values are part of this track's acceptance contract.

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
        // This remains a structural contract check; the explicit T002 locks
        // below pin only the shipped values required by this track.
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
fn test_committed_profile_reviewer_uses_grok_with_codex_fast_round() {
    let profiles = load_shipped_profile(AGENT_PROFILES_PATH);
    let config = profiles
        .resolve_capability(&capability("reviewer"))
        .expect("committed reviewer profile exists");

    assert_eq!(configured_provider(config), "grok");
    assert_eq!(configured_model(config).as_deref(), Some("grok-4.6"));
    assert_eq!(configured_fast_provider(config).as_deref(), Some("codex"));
    assert_eq!(configured_fast_model(config).as_deref(), Some("gpt-5.6-luna"));
}

#[test]
fn test_committed_profile_other_shipped_defaults_match_previous_base() {
    let profiles = load_shipped_profile(AGENT_PROFILES_PATH);
    let mut actual_capabilities =
        profiles.capabilities.keys().map(|name| name.as_str()).collect::<Vec<_>>();
    actual_capabilities.sort_unstable();
    let mut expected_capabilities = PREVIOUS_COMMITTED_SHIPPED_DEFAULTS
        .iter()
        .map(|entry| entry.capability)
        .collect::<Vec<_>>();
    expected_capabilities.sort_unstable();
    assert_eq!(actual_capabilities, expected_capabilities);

    for expected in PREVIOUS_COMMITTED_SHIPPED_DEFAULTS {
        let config = profiles
            .resolve_capability(&capability(expected.capability))
            .expect("baseline capability exists in committed profile");

        if expected.capability != "reviewer" {
            assert_eq!(
                configured_provider(config),
                expected.provider,
                "provider changed for {}",
                expected.capability
            );
            assert_eq!(
                configured_model(config).as_deref(),
                expected.model,
                "model changed for {}",
                expected.capability
            );
        }
        assert_eq!(
            configured_fast_provider(config).as_deref(),
            expected.fast_provider,
            "fast_provider changed for {}",
            expected.capability
        );
        assert_eq!(
            configured_fast_model(config).as_deref(),
            expected.fast_model,
            "fast_model changed for {}",
            expected.capability
        );
    }
}

#[test]
fn test_shipped_profiles_keep_verifier_provider_values_non_grok() {
    for profile_path in SHIPPED_PROFILE_PATHS {
        let profiles = load_shipped_profile(profile_path);
        for capability_name in TYPED_PIPELINE_VERIFIER_CAPABILITIES {
            let config = profiles
                .resolve_capability(&capability(capability_name))
                .expect("shipped profile contains every verifier capability");

            assert_ne!(
                configured_provider(config),
                "grok",
                "{profile_path} selects grok for {capability_name}"
            );
            assert_ne!(
                configured_fast_provider(config).as_deref(),
                Some("grok"),
                "{profile_path} selects grok fast_provider for {capability_name}"
            );
        }
    }
}

#[test]
fn test_shipped_profiles_resolve_pr_reviewer_to_codex_hosted_service() {
    for profile_path in SHIPPED_PROFILE_PATHS {
        let profiles = load_shipped_profile(profile_path);
        let resolved = profiles
            .resolve_execution(&capability("pr-reviewer"), RoundType::Final)
            .expect("shipped pr-reviewer resolves");

        assert!(matches!(
            resolved,
            ResolvedExecution::HostedService { provider } if provider.as_str() == "codex"
        ));
    }
}

#[test]
fn test_pr_reviewer_grok_hosted_resolution_is_rejected_by_reviewer_policy() {
    let profiles = load_profiles(
        r#"{
            "schema_version": 1,
            "providers": {
                "codex": { "label": "Codex CLI", "supported_reasoning_efforts": ["low"] },
                "grok": { "label": "Grok CLI", "supported_reasoning_efforts": ["low"] }
            },
            "capabilities": {
                "pr-reviewer": {
                    "provider": "grok",
                    "execution_mode": "typed-pipeline"
                }
            }
        }"#,
    );
    let resolved = profiles
        .resolve_execution(&capability("pr-reviewer"), RoundType::Final)
        .expect("pr-reviewer remains a hosted execution");
    let provider = match resolved {
        ResolvedExecution::HostedService { provider } => provider,
        other => {
            assert!(
                matches!(other, ResolvedExecution::HostedService { .. }),
                "pr-reviewer must not resolve to a provider CLI execution"
            );
            return;
        }
    };

    assert_eq!(provider.as_str(), "grok");
    assert!(usecase::pr_review::validate_reviewer_provider(provider.as_str()).is_err());
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
