//! Calibration-probe configuration for the semantic-verdict core.
//!
//! [`SemanticCalibrationProbeConfig`] parametrises the core's calibration probe:
//! how often a known-bad example is injected into a batch, and the detection
//! threshold below which the verifier is treated as degraded (IN-01 / AC-15).

use std::num::NonZeroU8;

/// Configuration for the semantic-verdict calibration probe.
///
/// Both fields are [`NonZeroU8`] so a zero injection rate or zero detection
/// threshold (a probe that can never fire or never fail) is unrepresentable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticCalibrationProbeConfig {
    injection: NonZeroU8,
    threshold: NonZeroU8,
}

impl SemanticCalibrationProbeConfig {
    /// Creates a [`SemanticCalibrationProbeConfig`].
    ///
    /// `injection` is the number of known-bad probes injected per batch and
    /// `threshold` is the minimum detection count required before the verifier
    /// is considered healthy.
    #[must_use]
    pub fn new(injection: NonZeroU8, threshold: NonZeroU8) -> Self {
        Self { injection, threshold }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_probe_config_new_holds_values() {
        let injection = NonZeroU8::new(3).unwrap();
        let threshold = NonZeroU8::new(2).unwrap();
        let config = SemanticCalibrationProbeConfig::new(injection, threshold);
        assert_eq!(config, SemanticCalibrationProbeConfig::new(injection, threshold));
    }

    #[test]
    fn test_probe_config_distinguishes_by_field() {
        let one = NonZeroU8::new(1).unwrap();
        let two = NonZeroU8::new(2).unwrap();
        assert_ne!(
            SemanticCalibrationProbeConfig::new(one, two),
            SemanticCalibrationProbeConfig::new(two, one)
        );
    }
}
