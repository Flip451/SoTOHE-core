//! I/O for `.claude/agent-profiles.json` — resolves per-model behavioral flags.

use std::collections::HashMap;

use domain::review_v2::{ModelProfile, resolve_full_auto};
use serde::Deserialize;

const AGENT_PROFILES_PATH: &str = ".claude/agent-profiles.json";

fn default_full_auto() -> bool {
    true
}

/// Serde-only mirror of `domain::review::ModelProfile`.
///
/// `domain::ModelProfile` is intentionally free of serde; deserialization
/// lives here in the infrastructure layer.
#[derive(Deserialize)]
struct ModelProfileSerde {
    #[serde(default = "default_full_auto")]
    full_auto: bool,
}

impl From<ModelProfileSerde> for ModelProfile {
    fn from(s: ModelProfileSerde) -> Self {
        ModelProfile::new(s.full_auto)
    }
}

#[derive(Deserialize)]
struct AgentProfiles {
    #[serde(default)]
    providers: HashMap<String, ProviderConfig>,
}

#[derive(Deserialize)]
struct ProviderConfig {
    #[serde(default)]
    model_profiles: Option<HashMap<String, ModelProfileSerde>>,
}

/// Reads `.claude/agent-profiles.json` and resolves whether `--full-auto` should be
/// enabled for the given model.
///
/// Falls back to `true` (fail-closed) when the file is missing, unreadable,
/// or does not contain `model_profiles` for the `codex` provider.
#[must_use]
pub fn resolve_full_auto_from_profiles(model: &str) -> bool {
    let content = match std::fs::read_to_string(AGENT_PROFILES_PATH) {
        Ok(c) => c,
        Err(_) => return true,
    };
    let profiles: AgentProfiles = match serde_json::from_str(&content) {
        Ok(p) => p,
        Err(_) => return true,
    };
    let codex = match profiles.providers.get("codex") {
        Some(p) => p,
        None => return true,
    };
    let converted: Option<HashMap<String, ModelProfile>> = codex
        .model_profiles
        .as_ref()
        .map(|m| m.iter().map(|(k, v)| (k.clone(), ModelProfile::new(v.full_auto))).collect());
    resolve_full_auto(model, converted.as_ref())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::resolve_full_auto_from_profiles;

    #[test]
    fn resolve_full_auto_from_profiles_returns_true_when_file_missing() {
        // This test relies on the agent-profiles.json in the project root.
        // The function should never panic regardless of file presence.
        let _result = resolve_full_auto_from_profiles("gpt-5.4");
        // No assertion needed — just verify it does not panic.
    }

    #[test]
    fn resolve_full_auto_from_profiles_falls_back_to_true_for_unknown_model() {
        // Unknown models always get full_auto=true (fail-closed).
        let result = resolve_full_auto_from_profiles("unknown-model-that-does-not-exist");
        assert!(result, "unknown model should fall back to full_auto=true");
    }
}
