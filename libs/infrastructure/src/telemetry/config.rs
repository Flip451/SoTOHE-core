//! `TelemetryConfig` — resolved telemetry configuration from environment variables.
//!
//! Loaded once at CLI startup by the composition root and passed to
//! `TelemetryWriter::new`.  Private fields ensure the config is only
//! constructed via `from_env`.

use std::path::PathBuf;

// ---------------------------------------------------------------------------
// TelemetryConfig
// ---------------------------------------------------------------------------

/// Resolved telemetry configuration loaded from environment variables at
/// `cli-composition` startup.
///
/// Private fields:
/// - `enabled`: `false` when `SOTP_TELEMETRY=0` (kill switch, IN-05 / AC-05).
/// - `output_dir_override`: `Some` when `SOTP_TELEMETRY_DIR` is set
///   (directory override, IN-05 / AC-05).
///
/// Role is `Dto` rather than `ValueObject` because it carries
/// serde/env-boundary resolution state, not a validated domain value.
#[derive(Debug, Clone)]
pub struct TelemetryConfig {
    enabled: bool,
    output_dir_override: Option<PathBuf>,
}

impl TelemetryConfig {
    /// Constructs a `TelemetryConfig` by reading environment variables.
    ///
    /// Rules:
    /// - `SOTP_TELEMETRY=0` → `enabled = false` (kill switch).
    /// - `SOTP_TELEMETRY_DIR=<path>` → `output_dir_override = Some(path)`.
    /// - Any other value of `SOTP_TELEMETRY` (or absent) → `enabled = true`.
    #[must_use]
    pub fn from_env() -> Self {
        let enabled = std::env::var("SOTP_TELEMETRY").map(|v| v.trim() != "0").unwrap_or(true);

        let output_dir_override =
            std::env::var("SOTP_TELEMETRY_DIR").ok().filter(|s| !s.is_empty()).map(PathBuf::from);

        Self { enabled, output_dir_override }
    }

    /// Returns `true` when telemetry recording is enabled.
    ///
    /// `false` when `SOTP_TELEMETRY=0` was set at construction time.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Returns the output directory override if `SOTP_TELEMETRY_DIR` was set.
    #[must_use]
    pub(crate) fn output_dir_override(&self) -> Option<&std::path::Path> {
        self.output_dir_override.as_deref()
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    /// Helper: run a closure with an env var set, then restore the original.
    /// Uses a thread-local mutex to serialise env-mutation across tests that
    /// run in the same process (parallel test threads share the env).
    #[test]
    fn test_from_env_with_kill_switch_zero_returns_disabled() {
        temp_env::with_var("SOTP_TELEMETRY", Some("0"), || {
            let cfg = TelemetryConfig::from_env();
            assert!(!cfg.is_enabled(), "SOTP_TELEMETRY=0 must disable telemetry");
        });
    }

    #[test]
    fn test_from_env_with_no_kill_switch_returns_enabled() {
        temp_env::with_var("SOTP_TELEMETRY", None::<&str>, || {
            let cfg = TelemetryConfig::from_env();
            assert!(cfg.is_enabled(), "absent SOTP_TELEMETRY must leave telemetry enabled");
        });
    }

    #[test]
    fn test_from_env_with_sotp_telemetry_one_returns_enabled() {
        temp_env::with_var("SOTP_TELEMETRY", Some("1"), || {
            let cfg = TelemetryConfig::from_env();
            assert!(cfg.is_enabled(), "SOTP_TELEMETRY=1 must leave telemetry enabled");
        });
    }

    #[test]
    fn test_from_env_with_dir_override_sets_output_dir() {
        temp_env::with_var("SOTP_TELEMETRY_DIR", Some("/tmp/test-telemetry"), || {
            let cfg = TelemetryConfig::from_env();
            assert_eq!(
                cfg.output_dir_override(),
                Some(std::path::Path::new("/tmp/test-telemetry")),
                "SOTP_TELEMETRY_DIR must populate output_dir_override"
            );
        });
    }

    #[test]
    fn test_from_env_without_dir_override_has_none() {
        temp_env::with_var("SOTP_TELEMETRY_DIR", None::<&str>, || {
            let cfg = TelemetryConfig::from_env();
            assert!(
                cfg.output_dir_override().is_none(),
                "absent SOTP_TELEMETRY_DIR must leave output_dir_override as None"
            );
        });
    }

    #[test]
    fn test_config_is_clone() {
        temp_env::with_var("SOTP_TELEMETRY", None::<&str>, || {
            let cfg = TelemetryConfig::from_env();
            let _cloned = cfg.clone();
        });
    }
}
