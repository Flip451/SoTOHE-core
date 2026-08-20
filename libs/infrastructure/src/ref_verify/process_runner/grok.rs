//! Grok arm for the shared typed-pipeline process runner.

use std::ffi::{OsStr, OsString};
use std::path::Path;

use usecase::capability_exec::{ModelName, ReasoningEffort};
use usecase::provider_session::ProviderSessionId;
use usecase::ref_verify::RefVerifyError;

use crate::capability_exec::grok::{build_grok_args, resolve_grok_capability_definition};
use crate::grok_common::{GrokOutputEnvelope, GrokSandbox, grok_envelope_bytes_from_stdout};

use super::{nonempty_trimmed, ref_verify_runner_error, run_process};

/// Builds the Grok argv for a ref-verifier invocation using the canonical
/// model / effort / resume / sandbox mapping shared by Grok adapters.
///
/// The Grok structured-output schema is the existing generic `result` string
/// envelope. The verifier's JSON response is transported in that structured
/// result and decoded by the shared grok ref-verifier runner.
#[must_use]
pub(super) fn build_grok_ref_verifier_args(
    model: &ModelName,
    effort: ReasoningEffort,
    sandbox: &GrokSandbox,
    resume_id: Option<&ProviderSessionId>,
    prompt: &str,
) -> Vec<OsString> {
    build_grok_args(
        model.as_str(),
        effort,
        sandbox,
        resume_id.map(ProviderSessionId::as_str),
        prompt,
    )
}

/// Runs the shared Grok ref-verifier arm.
pub(super) fn run_grok_ref_verifier(
    project_root: &Path,
    model: &ModelName,
    effort: ReasoningEffort,
    prompt: &str,
    capability: &str,
) -> Result<String, RefVerifyError> {
    super::super::retry::retry_transient(
        super::super::retry::DEFAULT_TRANSIENT_BACKOFFS,
        || run_grok_ref_verifier_once(project_root, model, effort, prompt, capability),
        std::thread::sleep,
    )
}

fn run_grok_ref_verifier_once(
    project_root: &Path,
    model: &ModelName,
    effort: ReasoningEffort,
    prompt: &str,
    capability: &str,
) -> Result<String, RefVerifyError> {
    let definition = resolve_grok_capability_definition(project_root, capability, model)
        .map_err(ref_verify_runner_error)?;
    let sandbox = definition.sandbox().cloned().ok_or_else(|| {
        ref_verify_runner_error(format!(
            "Grok capability '{capability}' did not resolve a grok-sandbox permission"
        ))
    })?;
    let args = build_grok_ref_verifier_args(model, effort, &sandbox, None, prompt);
    let outcome = run_process(OsStr::new("grok"), &args, project_root, "grok ref-verifier", None)?;
    let envelope_bytes = grok_envelope_bytes_from_stdout(outcome.stdout.as_bytes())
        .ok_or_else(|| ref_verify_runner_error("grok ref-verifier produced no output envelope"))?;
    let envelope =
        serde_json::from_slice::<GrokOutputEnvelope>(&envelope_bytes).map_err(|error| {
            ref_verify_runner_error(format!("cannot decode Grok ref-verifier envelope: {error}"))
        })?;
    let structured_output = envelope
        .into_structured_output()
        .map_err(|error| ref_verify_runner_error(error.to_string()))?;
    structured_output
        .get("result")
        .and_then(serde_json::Value::as_str)
        .and_then(nonempty_trimmed)
        .ok_or_else(|| {
            ref_verify_runner_error(
                "grok ref-verifier structured output is missing a non-empty string result",
            )
        })
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use std::fs;

    use super::*;
    use crate::agent_profiles::{AgentProfiles, ResolvedExecution, RoundType};
    use usecase::capability_exec::ProviderName;
    use usecase::dry_write_driver::CapabilityName;

    #[test]
    fn test_build_grok_ref_verifier_args_reuses_provider_mapping() {
        let cases = vec![
            ("grok-4.6", ReasoningEffort::Low, GrokSandbox::ReadOnly, None, "read-only prompt"),
            (
                "grok-4.6",
                ReasoningEffort::High,
                GrokSandbox::Workspace,
                Some("session-workspace"),
                "workspace prompt",
            ),
            (
                "grok-4.5",
                ReasoningEffort::XHigh,
                GrokSandbox::Strict,
                Some("session-strict"),
                "strict prompt",
            ),
            ("grok-reasoner", ReasoningEffort::Max, GrokSandbox::ReadOnly, None, "max prompt"),
        ];

        for (model_value, effort, sandbox, resume_value, prompt) in cases {
            let model = ModelName::try_new(model_value.to_owned()).expect("model is valid");
            let resume_id = resume_value.map(|value| {
                ProviderSessionId::try_new(value.to_owned()).expect("resume is valid")
            });
            let actual =
                build_grok_ref_verifier_args(&model, effort, &sandbox, resume_id.as_ref(), prompt);
            let expected = build_grok_args(
                model.as_str(),
                effort,
                &sandbox,
                resume_id.as_ref().map(ProviderSessionId::as_str),
                prompt,
            );

            assert_eq!(actual, expected, "Grok mapping drifted for model {model_value}");
        }
    }

    #[cfg(unix)]
    fn install_fake_grok(bin_dir: &Path, envelope: &str) {
        use std::os::unix::fs::PermissionsExt;

        let executable = bin_dir.join("grok");
        fs::write(&executable, format!("#!/bin/sh\nprintf '%s\\n' '{envelope}'\n"))
            .expect("fake grok is written");
        let mut permissions = fs::metadata(&executable).expect("fake grok metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&executable, permissions).expect("fake grok is executable");
    }

    #[cfg(unix)]
    fn install_recording_fake_grok(bin_dir: &Path, envelope: &str) {
        use std::os::unix::fs::PermissionsExt;

        let executable = bin_dir.join("grok");
        fs::write(
            &executable,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$GROK_REF_VERIFIER_MARKER\"\nprintf '%s\\n' '{envelope}'\n"
            ),
        )
        .expect("recording fake grok is written");
        let mut permissions =
            fs::metadata(&executable).expect("recording fake grok metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&executable, permissions).expect("recording fake grok is executable");
    }

    fn write_grok_skill(root: &Path, capability: &str, sandbox: &str) {
        let directory = root.join(".agents").join("skills").join(capability);
        fs::create_dir_all(&directory).expect("skill directory is created");
        fs::write(
            directory.join("SKILL.md"),
            format!(
                "---\nname: {capability}\ndescription: test grok verifier skill\ngrok-sandbox: {sandbox}\n---\n"
            ),
        )
        .expect("skill definition is written");
    }

    #[cfg(unix)]
    fn path_with_fake_grok(bin_dir: &Path) -> OsString {
        let mut entries = vec![bin_dir.to_path_buf()];
        if let Some(existing) = std::env::var_os("PATH") {
            entries.extend(std::env::split_paths(&existing));
        }
        std::env::join_paths(entries).expect("fake grok PATH is valid")
    }

    #[cfg(unix)]
    #[test]
    fn test_run_ref_verifier_agent_grok_returns_structured_result_and_ignores_text() {
        let directory = tempfile::tempdir().expect("temporary directory is created");
        let bin_dir = directory.path().join("bin");
        fs::create_dir(&bin_dir).expect("fake bin directory is created");
        install_fake_grok(
            &bin_dir,
            r#"{"text":"ignore this text","structured_output":{"result":"{\"kind\":\"pass\",\"citation\":\"AC-01\",\"reason\":null}"}}"#,
        );
        write_grok_skill(directory.path(), "ref-verifier-chain1", "read-only");
        let path = path_with_fake_grok(&bin_dir);
        let resolved = ResolvedExecution::ProviderCli {
            provider: ProviderName::try_new("grok".to_owned()).expect("provider is valid"),
            model: ModelName::try_new("grok-4.6".to_owned()).expect("model is valid"),
            effort: ReasoningEffort::High,
        };

        let result = temp_env::with_var("PATH", Some(path.as_os_str()), || {
            super::super::run_ref_verifier_agent(
                directory.path(),
                resolved,
                "prompt".to_owned(),
                super::super::CODEX_OUTPUT_SCHEMA,
                "ref-verifier-chain1",
            )
        })
        .expect("Grok structured result is returned");

        assert_eq!(result, r#"{"kind":"pass","citation":"AC-01","reason":null}"#);
    }

    #[cfg(unix)]
    #[test]
    fn test_grok_provider_and_fast_provider_launch_shared_ref_verifier_arm() {
        let directory = tempfile::tempdir().expect("temporary directory is created");
        let profile_path = directory.path().join("agent-profiles.json");
        let capability_entries = [
            "ref-verifier-chain1",
            "ref-verifier-chain2",
            "obligation-fulfillment-verifier",
            "waiver-verifier",
        ]
        .into_iter()
        .map(|name| {
            format!(
                r#""{name}": {{
                    "provider": "grok",
                    "model": "grok-final",
                    "fast_provider": "grok",
                    "fast_model": "grok-fast",
                    "reasoning_effort": "high",
                    "fast_reasoning_effort": "low",
                    "execution_mode": "typed-pipeline"
                }}"#,
                name = name
            )
        })
        .collect::<Vec<_>>()
        .join(",");
        let profile = format!(
            r#"{{
                "schema_version": 1,
                "providers": {{
                    "grok": {{
                        "label": "Grok CLI",
                        "supported_reasoning_efforts": ["low", "high"]
                    }}
                }},
                "capabilities": {{
                    {capability_entries}
                }}
            }}"#,
            capability_entries = capability_entries
        );
        fs::write(&profile_path, profile).expect("Grok profile is written");
        for capability_name in [
            "ref-verifier-chain1",
            "ref-verifier-chain2",
            "obligation-fulfillment-verifier",
            "waiver-verifier",
        ] {
            write_grok_skill(directory.path(), capability_name, "read-only");
        }
        let profiles =
            AgentProfiles::load(directory.path(), &profile_path).expect("Grok profile loads");

        let bin_dir = directory.path().join("bin");
        fs::create_dir(&bin_dir).expect("fake bin directory is created");
        install_recording_fake_grok(
            &bin_dir,
            r#"{"text":"ignore this text","structured_output":{"result":"{\"kind\":\"pass\",\"citation\":\"fake Grok\",\"reason\":null}"}}"#,
        );
        let path = path_with_fake_grok(&bin_dir);
        let marker = directory.path().join("grok-invocations.log");

        let outcomes = temp_env::with_var("PATH", Some(path.as_os_str()), || {
            temp_env::with_var("GROK_REF_VERIFIER_MARKER", Some(marker.as_os_str()), || {
                let mut outcomes = Vec::new();
                for capability_name in [
                    "ref-verifier-chain1",
                    "ref-verifier-chain2",
                    "obligation-fulfillment-verifier",
                    "waiver-verifier",
                ] {
                    let capability =
                        CapabilityName::try_new(capability_name.to_owned()).expect("capability");
                    for round_type in [RoundType::Final, RoundType::Fast] {
                        let resolved = profiles
                            .resolve_execution(&capability, round_type)
                            .expect("Grok provider resolves for both rounds");
                        assert!(matches!(
                            &resolved,
                            ResolvedExecution::ProviderCli { provider, .. }
                                if provider.as_str() == "grok"
                        ));
                        let result = super::super::run_ref_verifier_agent(
                            directory.path(),
                            resolved,
                            format!("{capability_name} {round_type:?}"),
                            super::super::CODEX_OUTPUT_SCHEMA,
                            capability_name,
                        );
                        outcomes.push((capability_name, round_type, result));
                    }
                }
                outcomes
            })
        });

        for (capability_name, round_type, result) in outcomes {
            let result = result.unwrap_or_else(|error| {
                panic!("Grok {round_type:?} execution failed for {capability_name}: {error:?}")
            });
            assert_eq!(result, r#"{"kind":"pass","citation":"fake Grok","reason":null}"#);
        }

        let invocation_count =
            fs::read_to_string(&marker).expect("fake Grok was launched").lines().count();
        assert_eq!(invocation_count, 8, "each provider/round capability pair launched Grok");
    }

    #[cfg(unix)]
    #[test]
    fn test_run_ref_verifier_agent_grok_reports_envelope_failure_without_text_fallback() {
        let directory = tempfile::tempdir().expect("temporary directory is created");
        let bin_dir = directory.path().join("bin");
        fs::create_dir(&bin_dir).expect("fake bin directory is created");
        install_fake_grok(
            &bin_dir,
            r#"{"text":"do not use me","failure_reason":"provider rejected the request"}"#,
        );
        write_grok_skill(directory.path(), "ref-verifier-chain1", "read-only");
        let path = path_with_fake_grok(&bin_dir);
        let resolved = ResolvedExecution::ProviderCli {
            provider: ProviderName::try_new("grok".to_owned()).expect("provider is valid"),
            model: ModelName::try_new("grok-4.6".to_owned()).expect("model is valid"),
            effort: ReasoningEffort::High,
        };

        let error = temp_env::with_var("PATH", Some(path.as_os_str()), || {
            super::super::run_ref_verifier_agent(
                directory.path(),
                resolved,
                "prompt".to_owned(),
                super::super::CODEX_OUTPUT_SCHEMA,
                "ref-verifier-chain1",
            )
        })
        .expect_err("Grok failure envelope must fail closed");
        let RefVerifyError::VerifierPort { message } = error else {
            panic!("expected VerifierPort, got {error:?}");
        };

        assert!(message.contains("provider rejected the request"), "got: {message}");
        assert!(!message.contains("do not use me"), "envelope text must not be a result");
    }

    #[cfg(unix)]
    #[test]
    fn test_run_ref_verifier_agent_grok_uses_capability_sandbox() {
        let directory = tempfile::tempdir().expect("temporary directory is created");
        let bin_dir = directory.path().join("bin");
        fs::create_dir(&bin_dir).expect("fake bin directory is created");
        write_grok_skill(directory.path(), "ref-verifier-chain2", "workspace");
        install_recording_fake_grok(
            &bin_dir,
            r#"{"structured_output":{"result":"{\"kind\":\"pass\",\"citation\":\"sandbox\",\"reason\":null}"}}"#,
        );
        let path = path_with_fake_grok(&bin_dir);
        let marker = directory.path().join("grok-invocations.log");
        let resolved = ResolvedExecution::ProviderCli {
            provider: ProviderName::try_new("grok".to_owned()).expect("provider is valid"),
            model: ModelName::try_new("grok-4.6".to_owned()).expect("model is valid"),
            effort: ReasoningEffort::High,
        };

        let result = temp_env::with_var("PATH", Some(path.as_os_str()), || {
            temp_env::with_var("GROK_REF_VERIFIER_MARKER", Some(marker.as_os_str()), || {
                super::super::run_ref_verifier_agent(
                    directory.path(),
                    resolved,
                    "prompt".to_owned(),
                    super::super::CODEX_OUTPUT_SCHEMA,
                    "ref-verifier-chain2",
                )
            })
        })
        .expect("workspace sandbox launch succeeds");
        assert_eq!(result, r#"{"kind":"pass","citation":"sandbox","reason":null}"#);
        let recorded = fs::read_to_string(&marker).expect("argv is recorded");
        assert!(recorded.contains("--sandbox workspace"), "got: {recorded}");
    }

    #[cfg(unix)]
    #[test]
    fn test_run_ref_verifier_agent_grok_retries_transient_envelope_failure() {
        let directory = tempfile::tempdir().expect("temporary directory is created");
        let bin_dir = directory.path().join("bin");
        fs::create_dir(&bin_dir).expect("fake bin directory is created");
        write_grok_skill(directory.path(), "waiver-verifier", "read-only");
        let executable = bin_dir.join("grok");
        fs::write(
            &executable,
            r#"#!/bin/sh
if [ -f "$GROK_REF_VERIFIER_MARKER" ]; then
  printf '%s\n' '{"structured_output":{"result":"{\"kind\":\"pass\",\"citation\":\"retried\",\"reason\":null}"}}'
else
  printf 'attempted\n' > "$GROK_REF_VERIFIER_MARKER"
  printf '%s\n' '{"failure_reason":"rate limit exceeded"}'
fi
"#,
        )
        .expect("retry fake grok is written");
        let mut permissions = fs::metadata(&executable).expect("fake grok metadata").permissions();
        use std::os::unix::fs::PermissionsExt;
        permissions.set_mode(0o755);
        fs::set_permissions(&executable, permissions).expect("fake grok is executable");
        let path = path_with_fake_grok(&bin_dir);
        let marker = directory.path().join("grok-retry.log");
        let resolved = ResolvedExecution::ProviderCli {
            provider: ProviderName::try_new("grok".to_owned()).expect("provider is valid"),
            model: ModelName::try_new("grok-4.6".to_owned()).expect("model is valid"),
            effort: ReasoningEffort::High,
        };

        let result = temp_env::with_var("PATH", Some(path.as_os_str()), || {
            temp_env::with_var("GROK_REF_VERIFIER_MARKER", Some(marker.as_os_str()), || {
                super::super::run_ref_verifier_agent(
                    directory.path(),
                    resolved,
                    "prompt".to_owned(),
                    super::super::CODEX_OUTPUT_SCHEMA,
                    "waiver-verifier",
                )
            })
        })
        .expect("transient envelope failure is retried");
        assert_eq!(result, r#"{"kind":"pass","citation":"retried","reason":null}"#);
    }

    #[cfg(unix)]
    #[test]
    fn test_public_adapter_new_uses_invoked_capability_sandbox() {
        let directory = tempfile::tempdir().expect("temporary directory is created");
        let prompt_dir = directory.path().join(".harness").join("prompts");
        fs::create_dir_all(&prompt_dir).expect("prompt directory is created");
        fs::write(
            prompt_dir.join("ref-verifier-chain2.md"),
            "Claim: {{claim}}\nEvidence: {{evidence}}\nTier: {{tier}}",
        )
        .expect("prompt template is written");
        write_grok_skill(directory.path(), "ref-verifier-chain2", "workspace");

        let profile_path = directory.path().join("agent-profiles.json");
        fs::write(
            &profile_path,
            r#"{
                "schema_version": 1,
                "providers": {
                    "grok": {
                        "label": "Grok CLI",
                        "supported_reasoning_efforts": ["low", "high"]
                    }
                },
                "capabilities": {
                    "ref-verifier-chain2": {
                        "provider": "grok",
                        "model": "grok-4.6",
                        "fast_provider": "grok",
                        "fast_model": "grok-4.6",
                        "reasoning_effort": "high",
                        "fast_reasoning_effort": "low",
                        "prompt_template_path": ".harness/prompts/ref-verifier-chain2.md",
                        "execution_mode": "typed-pipeline"
                    }
                }
            }"#,
        )
        .expect("profile is written");
        let profiles = std::sync::Arc::new(
            AgentProfiles::load(directory.path(), &profile_path).expect("profile loads"),
        );

        let bin_dir = directory.path().join("bin");
        fs::create_dir(&bin_dir).expect("fake bin directory is created");
        install_recording_fake_grok(
            &bin_dir,
            r#"{"structured_output":{"result":"{\"kind\":\"pass\",\"citation\":\"chain2 sandbox\",\"reason\":null}"}}"#,
        );
        let path = path_with_fake_grok(&bin_dir);
        let marker = directory.path().join("grok-invocations.log");
        let runner = super::super::make_ref_verifier_process_runner(directory.path().to_path_buf());
        let adapter = crate::ref_verify::AgentRefVerifierAdapter::new(
            profiles,
            runner,
            directory.path().to_path_buf(),
        );

        let result = temp_env::with_var("PATH", Some(path.as_os_str()), || {
            temp_env::with_var("GROK_REF_VERIFIER_MARKER", Some(marker.as_os_str()), || {
                usecase::ref_verify::RefVerifierPort::verify_pair(
                    &adapter,
                    "claim".to_owned(),
                    "evidence".to_owned(),
                    &usecase::ref_verify::RefVerifyCacheScope::CatalogueSpec {
                        layer: domain::tddd::LayerId::try_new("infrastructure".to_owned())
                            .expect("layer"),
                    },
                    domain::ModelTier::Final,
                )
            })
        })
        .expect("public adapter pairing launches Grok");
        assert!(matches!(result, domain::tddd::semantic_verify::SemanticVerdict::Pass { .. }));
        let recorded = fs::read_to_string(&marker).expect("argv is recorded");
        assert!(recorded.contains("--sandbox workspace"), "got: {recorded}");
    }
}
