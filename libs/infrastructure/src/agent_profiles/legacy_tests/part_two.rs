#[test]
fn test_shortcut_resolution_fast_and_final_returns_borrowed_config_values() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_json(dir.path(), FULL_CONFIG);
    let profiles = AgentProfiles::load(dir.path(), &path).unwrap();

    assert_eq!(profiles.resolve_model("reviewer", RoundType::Fast), Some("gpt-5.4-mini"));
    assert_eq!(profiles.resolve_model("orchestrator", RoundType::Fast), Some("claude-opus-4-7"));
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
    assert!(matches!(err, AgentProfilesError::InvalidCapability { .. }), "unexpected error: {err}");
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
    assert!(matches!(err, AgentProfilesError::InvalidCapability { .. }), "unexpected error: {err}");
}

// -----------------------------------------------------------------------
// T006 tests: ref-verifier-chain1/chain2 capabilities with
// prompt_template_path fields; independent resolution from reviewer (D11).
// -----------------------------------------------------------------------

const REF_VERIFIER_CONFIG: &str = r#"{
        "schema_version": 1,
        "providers": {
            "claude": { "label": "Claude Code", "supported_reasoning_efforts": ["low", "medium", "high", "xhigh", "max"] },
            "codex": { "label": "Codex CLI", "supported_reasoning_efforts": ["low", "medium", "high", "xhigh", "max"] }
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

    let final_exec = profiles.resolve_execution("ref-verifier-chain1", RoundType::Final).unwrap();
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
            "providers": { "claude": { "label": "Claude", "supported_reasoning_efforts": ["low", "medium", "high", "xhigh", "max"] } },
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
            "codex": { "label": "Codex CLI", "supported_reasoning_efforts": ["low", "medium", "high", "xhigh", "max"] }
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
            "codex": { "label": "Codex CLI", "supported_reasoning_efforts": ["low", "medium", "high", "xhigh", "max"] }
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
            "providers": { "codex": { "label": "Codex CLI", "supported_reasoning_efforts": ["low", "medium", "high", "xhigh", "max"] } },
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
