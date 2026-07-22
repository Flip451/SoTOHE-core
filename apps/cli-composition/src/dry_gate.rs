//! Feature-disabled composition for the lightweight `dry check-approved` gate.

use std::sync::Arc;

use cli_driver::dry::FeatureDisabledDryGateDriver;
use infrastructure::dry_check::dry_driver_shared::FsDryRepoRootAdapter;
use infrastructure::track::fixpoint_resolve_driver::FsDryCheckConfigLoaderAdapter;
use usecase::dry_check_approved_driver::FeatureDisabledDryGateInteractor;

/// Wire-only composition root for the feature-disabled DRY gate evaluation.
pub struct FeatureDisabledDryGateCompositionRoot;

impl FeatureDisabledDryGateCompositionRoot {
    /// Construct the feature-disabled gate composition root.
    #[must_use]
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self
    }

    /// Wire the lightweight config-first gate driver.
    #[must_use]
    pub fn driver(&self) -> FeatureDisabledDryGateDriver {
        FeatureDisabledDryGateDriver::new(Arc::new(FeatureDisabledDryGateInteractor::new(
            Arc::new(FsDryRepoRootAdapter),
            Arc::new(FsDryCheckConfigLoaderAdapter),
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature_disabled_dry_gate_composition_root_wires_driver() {
        let _driver = FeatureDisabledDryGateCompositionRoot::new().driver();
    }
}
