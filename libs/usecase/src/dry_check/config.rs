//! D3 (T009) usecase-layer config types: validated newtypes + the usecase
//! `DryCheckConfig` consumed by `DryCheckInteractor` (T010 / T012).
//!
//! Mirrors the precedent set by the `ref-verify` capability
//! (`libs/usecase/src/ref_verify/config.rs`):
//! - `DryCheckParallelism(usize)` newtype: validated nonzero parallelism.
//! - `DryCheckPercent(u8)` newtype: validated `1..=100` percent.
//! - `DryCheckConfig`: the usecase config struct, populated by composition
//!   from the infrastructure-layer DTO at adapter-construction time.

use super::errors::DryCheckCycleError;

// ── DryCheckParallelism ───────────────────────────────────────────────────────

/// Validated nonzero parallelism degree for the D3 judge fan-out (T010).
///
/// `try_new(0)` returns [`DryCheckCycleError::InvalidParallelism`] — composition
/// is responsible for sourcing the raw value from
/// `.harness/config/dry-check.json` via the infrastructure `DryCheckConfig`
/// loader and lifting it into this newtype.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DryCheckParallelism(usize);

impl DryCheckParallelism {
    /// Construct a [`DryCheckParallelism`] from a raw `usize`.
    ///
    /// # Errors
    ///
    /// Returns [`DryCheckCycleError::InvalidParallelism`] when `value == 0`.
    pub fn try_new(value: usize) -> Result<DryCheckParallelism, DryCheckCycleError> {
        if value == 0 {
            return Err(DryCheckCycleError::InvalidParallelism);
        }
        Ok(DryCheckParallelism(value))
    }

    /// Return the wrapped `usize`.
    #[must_use]
    pub fn as_usize(&self) -> usize {
        self.0
    }
}

// ── DryCheckPercent ───────────────────────────────────────────────────────────

/// Validated percentage in the inclusive range `1..=100`, used by D4 calibration
/// settings (`known_bad_injection_rate_percent`,
/// `known_bad_detection_threshold_percent`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DryCheckPercent(u8);

impl DryCheckPercent {
    /// Construct a [`DryCheckPercent`] from a raw `u8`.
    ///
    /// # Errors
    ///
    /// Returns [`DryCheckCycleError::InvalidPercent`] when `value` is `0` or
    /// greater than `100`.
    pub fn try_new(value: u8) -> Result<DryCheckPercent, DryCheckCycleError> {
        if !(1..=100).contains(&value) {
            return Err(DryCheckCycleError::InvalidPercent(value));
        }
        Ok(DryCheckPercent(value))
    }

    /// Return the wrapped `u8` (always in `1..=100`).
    #[must_use]
    pub fn as_u8(&self) -> u8 {
        self.0
    }
}

// ── DryCheckConfig (usecase) ──────────────────────────────────────────────────

/// Usecase-layer configuration for the dry-check capability.
///
/// Populated by the composition layer from the infrastructure DTO at
/// adapter-construction time (T010 / T012). All fields are domain-validated
/// newtypes — composition is the only place where raw integers are seen.
/// `enabled` governs whether the DRY gate runs at all (D2 / IN-05 / CN-06);
/// it is a plain boolean passed through from the infrastructure config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DryCheckConfig {
    /// D2 (IN-05 / CN-06): whether the DRY gate is active. When `false`, the
    /// gate short-circuits to Approved without running any checks.
    pub enabled: bool,
    /// D4 (CN-06): how often known-bad probes are mixed into the fast tier.
    pub known_bad_injection_rate_percent: DryCheckPercent,
    /// D4 (CN-06): minimum probe detection rate for fast tier to be trusted.
    pub known_bad_detection_threshold_percent: DryCheckPercent,
    /// D3 (CN-04): bounded judge fan-out parallelism.
    pub max_parallelism: DryCheckParallelism,
}

impl DryCheckConfig {
    /// Construct a [`DryCheckConfig`] from validated newtype components plus the enabled flag.
    ///
    /// Construction is total (each field is already validated by its
    /// newtype), so this is a plain constructor — composition uses it after
    /// lifting the infrastructure DTO values through `try_new`.
    #[must_use]
    pub fn new(
        known_bad_injection_rate_percent: DryCheckPercent,
        known_bad_detection_threshold_percent: DryCheckPercent,
        max_parallelism: DryCheckParallelism,
        enabled: bool,
    ) -> DryCheckConfig {
        DryCheckConfig {
            enabled,
            known_bad_injection_rate_percent,
            known_bad_detection_threshold_percent,
            max_parallelism,
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    // ── DryCheckParallelism ───────────────────────────────────────────────────

    #[test]
    fn test_dry_check_parallelism_try_new_with_zero_returns_invalid_parallelism() {
        let result = DryCheckParallelism::try_new(0);
        assert!(matches!(result, Err(DryCheckCycleError::InvalidParallelism)));
    }

    #[test]
    fn test_dry_check_parallelism_try_new_with_one_succeeds() {
        let value = DryCheckParallelism::try_new(1).unwrap();
        assert_eq!(value.as_usize(), 1);
    }

    #[test]
    fn test_dry_check_parallelism_try_new_with_large_value_succeeds() {
        let value = DryCheckParallelism::try_new(64).unwrap();
        assert_eq!(value.as_usize(), 64);
    }

    // ── DryCheckPercent ───────────────────────────────────────────────────────

    #[test]
    fn test_dry_check_percent_try_new_with_zero_returns_invalid_percent() {
        let result = DryCheckPercent::try_new(0);
        assert!(matches!(result, Err(DryCheckCycleError::InvalidPercent(0))));
    }

    #[test]
    fn test_dry_check_percent_try_new_with_one_succeeds() {
        let value = DryCheckPercent::try_new(1).unwrap();
        assert_eq!(value.as_u8(), 1);
    }

    #[test]
    fn test_dry_check_percent_try_new_with_one_hundred_succeeds() {
        let value = DryCheckPercent::try_new(100).unwrap();
        assert_eq!(value.as_u8(), 100);
    }

    #[test]
    fn test_dry_check_percent_try_new_with_one_hundred_one_returns_invalid_percent() {
        let result = DryCheckPercent::try_new(101);
        assert!(matches!(result, Err(DryCheckCycleError::InvalidPercent(101))));
    }

    // ── DryCheckConfig::new ───────────────────────────────────────────────────

    #[test]
    fn test_dry_check_config_new_preserves_all_fields() {
        let injection = DryCheckPercent::try_new(10).unwrap();
        let threshold = DryCheckPercent::try_new(90).unwrap();
        let parallelism = DryCheckParallelism::try_new(4).unwrap();
        let config = DryCheckConfig::new(injection, threshold, parallelism, false);
        assert!(!config.enabled);
        assert_eq!(config.known_bad_injection_rate_percent, injection);
        assert_eq!(config.known_bad_detection_threshold_percent, threshold);
        assert_eq!(config.max_parallelism, parallelism);
    }
}
