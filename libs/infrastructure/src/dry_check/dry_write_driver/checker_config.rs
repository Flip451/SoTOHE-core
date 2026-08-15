//! Helpers for lifting infra `DryCheckConfig` into the usecase newtypes and
//! resolving `CodexDryChecker` construction parameters (model + reasoning
//! effort) from `AgentProfiles`.
//!
//! Relocated from `apps/cli-composition/src/dry/dry_checker_config.rs` (T028)
//! — the logic is fully owned by [`super::DryCheckServiceFactoryAdapter`] /
//! [`super::FsDryWriteConfigLoaderAdapter`] now, so it no longer needs to
//! live in `cli_composition`.

use std::path::Path;

use crate::agent_profiles::{AGENT_PROFILES_PATH, AgentProfiles, ResolvedExecution};
use crate::dry_check::DryCheckConfig as InfraDryCheckConfig;
use crate::track::symlink_guard::reject_symlinks_below;

/// Distinguishes which validation step failed inside
/// [`build_usecase_dry_check_config`], so the caller
/// ([`super::FsDryWriteConfigLoaderAdapter::load`]) can map each failure to
/// its own `usecase::fixpoint_resolve_driver::DryCheckConfigLoaderError`
/// variant, carrying the raw rejected input value (not the validation
/// error's rendered text).
#[derive(Debug)]
pub(super) enum BuildDryCheckConfigError {
    /// A `known_bad_*_percent` value failed `DryCheckPercent::try_new`.
    InvalidKnownBadPercent(u8),
    /// `max_parallelism` failed `DryCheckParallelism::try_new`.
    InvalidMaxParallelism(usize),
}

/// Lift infra [`InfraDryCheckConfig`] fields (enabled + max_parallelism +
/// known-bad percents) into the validated usecase newtypes. All values come
/// from `.harness/config/dry-check.json` v4.
pub(super) fn build_usecase_dry_check_config(
    infra_config: &InfraDryCheckConfig,
) -> Result<usecase::dry_check::DryCheckConfig, BuildDryCheckConfigError> {
    use usecase::dry_check::{DryCheckConfig, DryCheckParallelism, DryCheckPercent};
    let percent = |v: u8| {
        DryCheckPercent::try_new(v).map_err(|_| BuildDryCheckConfigError::InvalidKnownBadPercent(v))
    };
    Ok(DryCheckConfig::new(
        percent(infra_config.known_bad_injection_rate_percent())?,
        percent(infra_config.known_bad_detection_threshold_percent())?,
        DryCheckParallelism::try_new(infra_config.max_parallelism()).map_err(|_| {
            BuildDryCheckConfigError::InvalidMaxParallelism(infra_config.max_parallelism())
        })?,
        infra_config.enabled(),
    ))
}

/// Resolve `(provider, fast_model, final_model, fast_reasoning_effort,
/// final_reasoning_effort)` for the `dry-checker` capability. Explicit
/// `--model` overrides both model fields. Both rounds resolve through the
/// fail-closed profile API so the process never falls back to provider effort
/// defaults.
pub(super) fn resolve_dry_checker_config(
    root: &Path,
    capability_name: &str,
    explicit_model: Option<String>,
) -> Result<(String, String, String, String, String), String> {
    use crate::agent_profiles::RoundType;
    use usecase::capability_exec::ReasoningEffort;
    use usecase::dry_write_driver::CapabilityName;
    let profiles = load_agent_profiles_under_root(root)?;
    let capability = CapabilityName::try_new(capability_name)
        .map_err(|error| format!("[ERROR] invalid dry-checker capability name: {error}"))?;
    let resolve_cli = |round_type| {
        let resolved = profiles
            .resolve_execution(&capability, round_type)
            .map_err(|error| format!("[ERROR] failed to resolve '{capability_name}': {error}"))?;
        match resolved {
            ResolvedExecution::ProviderCli { provider, model, effort } => {
                Ok((provider, model, effort))
            }
            ResolvedExecution::HostedService { .. } => {
                Err(format!("[ERROR] '{capability_name}' must resolve to a provider CLI execution"))
            }
        }
    };
    let (fast_provider, fast_profile_model, fast_effort) = resolve_cli(RoundType::Fast)?;
    let (final_provider, final_profile_model, final_effort) = resolve_cli(RoundType::Final)?;
    if fast_provider.as_str() != final_provider.as_str() {
        return Err(format!(
            "[ERROR] '{capability_name}' fast provider '{}' does not match final provider '{}'",
            fast_provider.as_str(),
            final_provider.as_str()
        ));
    }
    if final_provider.as_str() == "grok"
        && let Some(model) = explicit_model.as_ref()
        && (model != fast_profile_model.as_str() || model != final_profile_model.as_str())
    {
        return Err(format!(
            "[ERROR] Grok dry-checker model override '{model}' does not match profile models '{}/{}'",
            fast_profile_model.as_str(),
            final_profile_model.as_str()
        ));
    }
    let (fast_model, final_model) = explicit_model.map_or_else(
        || (fast_profile_model.as_str().to_owned(), final_profile_model.as_str().to_owned()),
        |model| (model.clone(), model),
    );
    let effort_text = |effort| match effort {
        ReasoningEffort::Low => "low",
        ReasoningEffort::Medium => "medium",
        ReasoningEffort::High => "high",
        ReasoningEffort::XHigh => "xhigh",
        ReasoningEffort::Max => "max",
    };
    let fast_reasoning_effort = effort_text(fast_effort).to_owned();
    let final_reasoning_effort = effort_text(final_effort).to_owned();
    Ok((
        final_provider.as_str().to_owned(),
        fast_model,
        final_model,
        fast_reasoning_effort,
        final_reasoning_effort,
    ))
}

fn load_agent_profiles_under_root(root: &Path) -> Result<AgentProfiles, String> {
    let canonical_root = root.canonicalize().map_err(|e| {
        format!("[ERROR] failed to canonicalize repo root '{}': {e}", root.display())
    })?;
    let profiles_path = canonical_root.join(AGENT_PROFILES_PATH);
    reject_symlinks_below(&profiles_path, &canonical_root).map_err(|e| {
        format!("symlink guard agent-profiles.json '{}': {e}", profiles_path.display())
    })?;
    AgentProfiles::load(&canonical_root, &profiles_path)
        .map_err(|e| format!("[ERROR] failed to load agent-profiles.json: {e}"))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn load_infra_dry_check_config_from_json(json: &str) -> InfraDryCheckConfig {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("dry-check.json");
        std::fs::write(&path, json).unwrap();
        let config = InfraDryCheckConfig::load(&path).unwrap();
        drop(dir);
        config
    }

    #[test]
    fn test_dry_write_passes_max_parallelism_to_usecase_config() {
        let infra_config = load_infra_dry_check_config_from_json(
            r#"{
                "schema_version": 4,
                "threshold": 0.85,
                "max_parallelism": 7,
                "known_bad_injection_rate_percent": 10,
                "known_bad_detection_threshold_percent": 90
            }"#,
        );
        let usecase_config = build_usecase_dry_check_config(&infra_config).unwrap();
        assert_eq!(usecase_config.max_parallelism.as_usize(), 7);
    }

    #[test]
    fn test_dry_write_passes_known_bad_calibration_to_usecase_config() {
        let infra_config = load_infra_dry_check_config_from_json(
            r#"{
                "schema_version": 4,
                "threshold": 0.85,
                "max_parallelism": 4,
                "known_bad_injection_rate_percent": 20,
                "known_bad_detection_threshold_percent": 80
            }"#,
        );
        let usecase_config = build_usecase_dry_check_config(&infra_config).unwrap();
        assert_eq!(usecase_config.known_bad_injection_rate_percent.as_u8(), 20);
        assert_eq!(usecase_config.known_bad_detection_threshold_percent.as_u8(), 80);
    }

    #[test]
    fn test_resolve_dry_checker_config_returns_fast_and_final_from_agent_profiles() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".harness/config")).unwrap();
        std::fs::write(
            dir.path().join(".harness/config/agent-profiles.json"),
            r#"{
  "schema_version": 1,
  "providers": {
    "codex": {
      "label": "Codex",
      "supported_reasoning_efforts": ["low", "high"]
    }
  },
  "capabilities": {
    "dry-checker": {
      "provider": "codex",
      "model": "final-model-v1",
      "fast_model": "fast-model-v1",
      "fast_reasoning_effort": "low",
      "final_reasoning_effort": "high",
      "execution_mode": "typed-pipeline"
    }
  }
}"#,
        )
        .unwrap();

        let (provider, fast_model, final_model, fast_effort, final_effort) =
            resolve_dry_checker_config(dir.path(), "dry-checker", None).unwrap();

        assert_eq!(provider, "codex");
        assert_eq!(fast_model, "fast-model-v1");
        assert_eq!(final_model, "final-model-v1");
        assert_eq!(fast_effort, "low");
        assert_eq!(final_effort, "high");
    }

    #[test]
    fn test_resolve_dry_checker_config_explicit_model_uses_agent_profile_reasoning_effort() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".harness/config")).unwrap();
        std::fs::write(
            dir.path().join(".harness/config/agent-profiles.json"),
            r#"{
  "schema_version": 1,
  "providers": {
    "codex": {
      "label": "Codex",
      "supported_reasoning_efforts": ["low", "high"]
    }
  },
  "capabilities": {
    "dry-checker": {
      "provider": "codex",
      "model": "profile-final-model-v1",
      "fast_model": "profile-fast-model-v1",
      "fast_reasoning_effort": "low",
      "reasoning_effort": "high",
      "execution_mode": "typed-pipeline"
    }
  }
}"#,
        )
        .unwrap();

        let (_provider, fast_model, final_model, fast_effort, final_effort) =
            resolve_dry_checker_config(
                dir.path(),
                "dry-checker",
                Some("explicit-model-v1".to_owned()),
            )
            .unwrap();

        assert_eq!(fast_model, "explicit-model-v1");
        assert_eq!(final_model, "explicit-model-v1");
        assert_eq!(fast_effort, "low");
        assert_eq!(final_effort, "high");
    }

    #[test]
    fn test_resolve_dry_checker_config_fast_uses_generic_effort_when_not_overridden() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".harness/config")).unwrap();
        std::fs::write(
            dir.path().join(".harness/config/agent-profiles.json"),
            r#"{
  "schema_version": 1,
  "providers": {
    "codex": {
      "label": "Codex",
      "supported_reasoning_efforts": ["high"]
    }
  },
  "capabilities": {
    "dry-checker": {
      "provider": "codex",
      "model": "only-final-model-v1",
      "reasoning_effort": "high",
      "execution_mode": "typed-pipeline"
    }
  }
}"#,
        )
        .unwrap();

        let (_provider, fast_model, final_model, fast_effort, final_effort) =
            resolve_dry_checker_config(dir.path(), "dry-checker", None).unwrap();

        assert_eq!(fast_model, "only-final-model-v1");
        assert_eq!(final_model, "only-final-model-v1");
        assert_eq!(fast_effort, "high");
        assert_eq!(final_effort, "high");
    }

    #[cfg(unix)]
    #[test]
    fn test_resolve_dry_checker_config_symlinked_agent_profiles_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let config_dir = dir.path().join(".harness/config");
        std::fs::create_dir_all(&config_dir).unwrap();
        let real_profiles = config_dir.join("real-agent-profiles.json");
        std::fs::write(
            &real_profiles,
            r#"{
  "schema_version": 1,
  "providers": {
    "codex": {
      "label": "Codex",
      "supported_reasoning_efforts": ["high"]
    }
  },
  "capabilities": {
    "dry-checker": {
      "provider": "codex",
      "model": "final-model-v1",
      "execution_mode": "typed-pipeline"
    }
  }
}"#,
        )
        .unwrap();
        std::os::unix::fs::symlink(&real_profiles, config_dir.join("agent-profiles.json")).unwrap();

        let err = resolve_dry_checker_config(dir.path(), "dry-checker", None).unwrap_err();

        assert!(err.contains("symlink guard agent-profiles.json"), "got: {err}");
        assert!(err.contains("symlink"), "got: {err}");
    }

    #[test]
    fn test_resolve_dry_checker_config_invalid_reasoning_effort_returns_error() {
        let cases = [("turbo", "high", "turbo"), ("medium", "ultra", "ultra")];

        for (fast_effort, final_effort, expected_value) in cases {
            let dir = tempfile::tempdir().unwrap();
            std::fs::create_dir_all(dir.path().join(".harness/config")).unwrap();
            std::fs::write(
                dir.path().join(".harness/config/agent-profiles.json"),
                format!(
                    r#"{{
  "schema_version": 1,
  "providers": {{
    "codex": {{
      "label": "Codex",
      "supported_reasoning_efforts": ["low", "medium", "high"]
    }}
  }},
  "capabilities": {{
    "dry-checker": {{
      "provider": "codex",
      "model": "final-model-v1",
      "fast_reasoning_effort": "{fast_effort}",
      "final_reasoning_effort": "{final_effort}",
      "execution_mode": "typed-pipeline"
    }}
  }}
}}"#
                ),
            )
            .unwrap();

            let err = resolve_dry_checker_config(dir.path(), "dry-checker", None).unwrap_err();
            assert!(err.contains(expected_value), "got: {err}");
        }
    }

    #[test]
    fn test_resolve_dry_checker_config_returns_grok_provider() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".harness/config")).unwrap();
        std::fs::write(
            dir.path().join(".harness/config/agent-profiles.json"),
            r#"{
  "schema_version": 1,
  "providers": {
    "grok": {
      "label": "Grok",
      "supported_reasoning_efforts": ["low", "high"]
    }
  },
  "capabilities": {
    "dry-checker": {
      "provider": "grok",
      "model": "grok-final",
      "fast_model": "grok-fast",
      "fast_reasoning_effort": "low",
      "final_reasoning_effort": "high",
      "execution_mode": "typed-pipeline"
    }
  }
}"#,
        )
        .unwrap();

        let (provider, fast_model, final_model, fast_effort, final_effort) =
            resolve_dry_checker_config(dir.path(), "dry-checker", None).unwrap();

        assert_eq!(provider, "grok");
        assert_eq!(fast_model, "grok-fast");
        assert_eq!(final_model, "grok-final");
        assert_eq!(fast_effort, "low");
        assert_eq!(final_effort, "high");
    }

    #[test]
    fn test_resolve_dry_checker_config_rejects_grok_model_override_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".harness/config")).unwrap();
        std::fs::write(
            dir.path().join(".harness/config/agent-profiles.json"),
            r#"{
  "schema_version": 1,
  "providers": {
    "grok": {
      "label": "Grok",
      "supported_reasoning_efforts": ["low", "high"]
    }
  },
  "capabilities": {
    "dry-checker": {
      "provider": "grok",
      "model": "grok-final",
      "fast_model": "grok-fast",
      "fast_reasoning_effort": "low",
      "final_reasoning_effort": "high",
      "execution_mode": "typed-pipeline"
    }
  }
}"#,
        )
        .unwrap();

        let err =
            resolve_dry_checker_config(dir.path(), "dry-checker", Some("gpt-fast".to_owned()))
                .unwrap_err();

        assert!(err.contains("does not match profile models"), "got: {err}");
        assert!(err.contains("gpt-fast"), "got: {err}");
    }
}
