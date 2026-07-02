//! Helpers for building usecase `DryCheckConfig` from infra config and
//! resolving `CodexDryChecker` construction parameters (model + reasoning effort)
//! from `AgentProfiles`.

/// Lift infra `DryCheckConfig` fields (enabled + max_parallelism + known-bad percents) into the
/// validated usecase newtypes (D2 / D3 / D4 / T004 / T011). All values come from
/// `.harness/config/dry-check.json` v4.
pub(super) fn build_usecase_dry_check_config(
    infra_config: &infrastructure::dry_check::DryCheckConfig,
) -> Result<usecase::dry_check::DryCheckConfig, String> {
    use usecase::dry_check::{DryCheckConfig, DryCheckParallelism, DryCheckPercent};
    let percent =
        |v: u8| DryCheckPercent::try_new(v).map_err(|e| format!("invalid known-bad percent: {e}"));
    Ok(DryCheckConfig::new(
        percent(infra_config.known_bad_injection_rate_percent())?,
        percent(infra_config.known_bad_detection_threshold_percent())?,
        DryCheckParallelism::try_new(infra_config.max_parallelism())
            .map_err(|e| format!("invalid max_parallelism: {e}"))?,
        infra_config.enabled(),
    ))
}

pub(super) const DEFAULT_FAST_REASONING_EFFORT: &str = "medium";
pub(super) const DEFAULT_FINAL_REASONING_EFFORT: &str = "high";
const ALLOWED_DRY_CHECKER_REASONING_EFFORTS: &[&str] = &["low", "medium", "high", "minimal"];

pub(super) fn resolve_dry_checker_reasoning_effort(
    capability_name: &str,
    field: &str,
    configured: Option<&str>,
    default_value: &str,
) -> Result<String, String> {
    let value = configured.unwrap_or(default_value);
    if ALLOWED_DRY_CHECKER_REASONING_EFFORTS.contains(&value) {
        Ok(value.to_owned())
    } else {
        Err(format!(
            "[ERROR] invalid reasoning_effort in agent-profiles.json capability \
             '{capability_name}' field '{field}': '{value}' (allowed: low, medium, high, minimal)"
        ))
    }
}

/// Resolve `(fast_model, final_model, fast_reasoning_effort, final_reasoning_effort)` for the
/// `dry-checker` capability (D4 / T012 / T013). Explicit `--model` overrides both model fields.
/// Reasoning effort comes from `CapabilityConfigDto` accessors, is validated against the Codex
/// allowed values, and absent fields fall back to `"medium"` (fast) / `"high"` (final).
pub(super) fn resolve_dry_checker_config(
    root: &std::path::Path,
    capability_name: &str,
    explicit_model: Option<String>,
) -> Result<(String, String, String, String), String> {
    use infrastructure::agent_profiles::{AGENT_PROFILES_PATH, AgentProfiles, RoundType};
    let profiles = AgentProfiles::load(&root.join(AGENT_PROFILES_PATH))
        .map_err(|e| format!("[ERROR] failed to load agent-profiles.json: {e}"))?;
    let (fast_model, final_model) = if let Some(m) = explicit_model {
        (m.clone(), m)
    } else {
        let resolve_model =
            |rt| profiles.resolve_execution(capability_name, rt).and_then(|r| r.model);
        let final_model = resolve_model(RoundType::Final).ok_or_else(|| {
            format!(
                "[ERROR] no model specified: pass --model or set model in \
                 agent-profiles.json '{capability_name}' capability"
            )
        })?;
        (resolve_model(RoundType::Fast).unwrap_or_else(|| final_model.clone()), final_model)
    };
    let cap = profiles.resolve_capability(capability_name);
    let fast_reasoning_effort = resolve_dry_checker_reasoning_effort(
        capability_name,
        "fast_reasoning_effort",
        cap.and_then(|c| c.fast_reasoning_effort()),
        DEFAULT_FAST_REASONING_EFFORT,
    )?;
    let final_reasoning_effort = resolve_dry_checker_reasoning_effort(
        capability_name,
        "final_reasoning_effort",
        cap.and_then(|c| c.final_reasoning_effort()),
        DEFAULT_FINAL_REASONING_EFFORT,
    )?;
    Ok((fast_model, final_model, fast_reasoning_effort, final_reasoning_effort))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;

    // ── T016 smoke tests: build_usecase_dry_check_config + resolve_dry_checker_config ──

    /// Builds an infra `DryCheckConfig` from JSON content and a temp file.
    fn load_infra_dry_check_config_from_json(
        json: &str,
    ) -> infrastructure::dry_check::DryCheckConfig {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("dry-check.json");
        std::fs::write(&path, json).unwrap();
        let config = infrastructure::dry_check::DryCheckConfig::load(&path).unwrap();
        // Keep dir alive through the function — config has no reference to the file.
        drop(dir);
        config
    }

    /// Verify that `build_usecase_dry_check_config` propagates `max_parallelism`
    /// from the infra config to the usecase config newtype.
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
        assert_eq!(infra_config.max_parallelism(), 7, "infra config must expose max_parallelism=7");

        let usecase_config = build_usecase_dry_check_config(&infra_config).unwrap();
        assert_eq!(
            usecase_config.max_parallelism.as_usize(),
            7,
            "build_usecase_dry_check_config must propagate max_parallelism to the usecase newtype"
        );
    }

    /// Verify that `build_usecase_dry_check_config` propagates the known-bad calibration
    /// percent fields from the infra config to the usecase config newtypes.
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
        assert_eq!(infra_config.known_bad_injection_rate_percent(), 20);
        assert_eq!(infra_config.known_bad_detection_threshold_percent(), 80);

        let usecase_config = build_usecase_dry_check_config(&infra_config).unwrap();
        assert_eq!(
            usecase_config.known_bad_injection_rate_percent.as_u8(),
            20,
            "build_usecase_dry_check_config must propagate known_bad_injection_rate_percent"
        );
        assert_eq!(
            usecase_config.known_bad_detection_threshold_percent.as_u8(),
            80,
            "build_usecase_dry_check_config must propagate known_bad_detection_threshold_percent"
        );
    }

    /// Verify that `resolve_dry_checker_config` returns fast and final models from a
    /// test `agent-profiles.json` with both `fast_model` and `model` defined.
    #[test]
    fn test_resolve_dry_checker_config_returns_fast_and_final_from_agent_profiles() {
        let dir = tempfile::tempdir().unwrap();

        // Write a minimal agent-profiles.json with separate fast_model / model and
        // reasoning_effort fields for dry-checker.
        std::fs::create_dir_all(dir.path().join(".harness/config")).unwrap();
        std::fs::write(
            dir.path().join(".harness/config/agent-profiles.json"),
            r#"{
  "schema_version": 1,
  "providers": { "codex": { "label": "Codex" } },
  "capabilities": {
    "dry-checker": {
      "provider": "codex",
      "model": "final-model-v1",
      "fast_model": "fast-model-v1",
      "fast_reasoning_effort": "low",
      "final_reasoning_effort": "high"
    }
  }
}"#,
        )
        .unwrap();

        let (fast_model, final_model, fast_effort, final_effort) =
            resolve_dry_checker_config(dir.path(), "dry-checker", None).unwrap();

        assert_eq!(
            fast_model, "fast-model-v1",
            "fast_model must come from the fast_model field in agent-profiles.json"
        );
        assert_eq!(
            final_model, "final-model-v1",
            "final_model must come from the model field in agent-profiles.json"
        );
        assert_eq!(
            fast_effort, "low",
            "fast_reasoning_effort must come from agent-profiles.json dry-checker capability"
        );
        assert_eq!(
            final_effort, "high",
            "final_reasoning_effort must come from agent-profiles.json dry-checker capability"
        );
    }

    /// Verify that an explicit model override still uses reasoning effort from agent-profiles.json.
    #[test]
    fn test_resolve_dry_checker_config_explicit_model_uses_agent_profile_reasoning_effort() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".harness/config")).unwrap();
        std::fs::write(
            dir.path().join(".harness/config/agent-profiles.json"),
            r#"{
  "schema_version": 1,
  "providers": { "codex": { "label": "Codex" } },
  "capabilities": {
    "dry-checker": {
      "provider": "codex",
      "model": "profile-final-model-v1",
      "fast_model": "profile-fast-model-v1",
      "fast_reasoning_effort": "low",
      "final_reasoning_effort": "minimal"
    }
  }
}"#,
        )
        .unwrap();

        let (fast_model, final_model, fast_effort, final_effort) = resolve_dry_checker_config(
            dir.path(),
            "dry-checker",
            Some("explicit-model-v1".to_owned()),
        )
        .unwrap();

        assert_eq!(fast_model, "explicit-model-v1");
        assert_eq!(final_model, "explicit-model-v1");
        assert_eq!(fast_effort, "low");
        assert_eq!(final_effort, "minimal");
    }

    /// Verify that `resolve_dry_checker_config` falls back fast_model → final_model
    /// when no separate `fast_model` field is configured, and uses built-in defaults
    /// for reasoning effort when the fields are absent.
    #[test]
    fn test_resolve_dry_checker_config_fast_falls_back_to_final_when_not_set() {
        let dir = tempfile::tempdir().unwrap();

        std::fs::create_dir_all(dir.path().join(".harness/config")).unwrap();
        std::fs::write(
            dir.path().join(".harness/config/agent-profiles.json"),
            r#"{
  "schema_version": 1,
  "providers": { "codex": { "label": "Codex" } },
  "capabilities": {
    "dry-checker": {
      "provider": "codex",
      "model": "only-final-model-v1"
    }
  }
}"#,
        )
        .unwrap();

        let (fast_model, final_model, fast_effort, final_effort) =
            resolve_dry_checker_config(dir.path(), "dry-checker", None).unwrap();

        assert_eq!(
            fast_model, "only-final-model-v1",
            "fast_model must fall back to final_model when fast_model is not configured"
        );
        assert_eq!(final_model, "only-final-model-v1", "final_model must be the model field");
        assert_eq!(
            fast_effort, "medium",
            "fast_reasoning_effort must use built-in default 'medium' when absent from profiles"
        );
        assert_eq!(
            final_effort, "high",
            "final_reasoning_effort must use built-in default 'high' when absent from profiles"
        );
    }

    /// Verify that `resolve_dry_checker_config` remains the allowed-values guard
    /// for reasoning effort values loaded from agent-profiles.json.
    #[test]
    fn test_resolve_dry_checker_config_invalid_reasoning_effort_returns_error() {
        let cases = [
            ("turbo", "high", "fast_reasoning_effort", "turbo"),
            ("medium", "ultra", "final_reasoning_effort", "ultra"),
        ];

        for (fast_effort, final_effort, expected_field, expected_value) in cases {
            let dir = tempfile::tempdir().unwrap();
            std::fs::create_dir_all(dir.path().join(".harness/config")).unwrap();
            std::fs::write(
                dir.path().join(".harness/config/agent-profiles.json"),
                format!(
                    r#"{{
  "schema_version": 1,
  "providers": {{ "codex": {{ "label": "Codex" }} }},
  "capabilities": {{
    "dry-checker": {{
      "provider": "codex",
      "model": "final-model-v1",
      "fast_reasoning_effort": "{fast_effort}",
      "final_reasoning_effort": "{final_effort}"
    }}
  }}
}}"#
                ),
            )
            .unwrap();

            let err = resolve_dry_checker_config(dir.path(), "dry-checker", None).unwrap_err();
            assert!(
                err.contains(expected_field),
                "error must name invalid field {expected_field}; got: {err}"
            );
            assert!(
                err.contains(expected_value),
                "error must include invalid value {expected_value}; got: {err}"
            );
            assert!(
                err.contains("allowed: low, medium, high, minimal"),
                "error must show allowed values; got: {err}"
            );
        }
    }
}
