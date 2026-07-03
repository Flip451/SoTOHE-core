//! Helpers for lifting infra `DryCheckConfig` into the usecase newtypes and
//! resolving `CodexDryChecker` construction parameters (model + reasoning
//! effort) from `AgentProfiles`.
//!
//! Relocated from `apps/cli-composition/src/dry/dry_checker_config.rs` (T028)
//! — the logic is fully owned by [`super::DryCheckServiceFactoryAdapter`] /
//! [`super::FsDryWriteConfigLoaderAdapter`] now, so it no longer needs to
//! live in `cli_composition`.

use std::path::Path;

use crate::agent_profiles::{AGENT_PROFILES_PATH, AgentProfiles};
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

/// Resolve `(fast_model, final_model, fast_reasoning_effort,
/// final_reasoning_effort)` for the `dry-checker` capability. Explicit
/// `--model` overrides both model fields. Reasoning effort comes from
/// `CapabilityConfigDto` accessors, is validated against the Codex allowed
/// values, and absent fields fall back to `"medium"` (fast) / `"high"` (final).
pub(super) fn resolve_dry_checker_config(
    root: &Path,
    capability_name: &str,
    explicit_model: Option<String>,
) -> Result<(String, String, String, String), String> {
    use crate::agent_profiles::RoundType;
    let profiles = load_agent_profiles_under_root(root)?;
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

fn load_agent_profiles_under_root(root: &Path) -> Result<AgentProfiles, String> {
    let canonical_root = root.canonicalize().map_err(|e| {
        format!("[ERROR] failed to canonicalize repo root '{}': {e}", root.display())
    })?;
    let profiles_path = canonical_root.join(AGENT_PROFILES_PATH);
    reject_symlinks_below(&profiles_path, &canonical_root).map_err(|e| {
        format!("symlink guard agent-profiles.json '{}': {e}", profiles_path.display())
    })?;
    AgentProfiles::load(&profiles_path)
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

        assert_eq!(fast_model, "only-final-model-v1");
        assert_eq!(final_model, "only-final-model-v1");
        assert_eq!(fast_effort, "medium");
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
  "providers": { "codex": { "label": "Codex" } },
  "capabilities": {
    "dry-checker": {
      "provider": "codex",
      "model": "final-model-v1"
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
            assert!(err.contains(expected_field), "got: {err}");
            assert!(err.contains(expected_value), "got: {err}");
        }
    }
}
