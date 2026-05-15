//! Render-configuration value object for the Contract Map.
//!
//! The three stub fields that existed in the previous version
//! (`signal_overlay`, `action_overlay`, `include_spec_source_edges`) have
//! been removed per ADR 2026-05-13-0000 Decision M (YAGNI). Only the
//! `layers` filter field remains.

use crate::tddd::LayerId;

/// Options that drive `ContractMapRenderer::render`.
///
/// * `layers` — if non-empty, restricts the rendered subgraphs to this
///   layer subset (preserving `layer_order` ordering). An empty `layers`
///   list means "render every entry in `layer_order`".
///
/// The three overlay / edge stubs that appeared in the previous version of
/// this struct (`signal_overlay`, `action_overlay`,
/// `include_spec_source_edges`) were removed by ADR 2026-05-13-0000
/// Decision M (YAGNI). They will be re-introduced under a separate ADR if
/// the need becomes concrete.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ContractMapRenderOptions {
    pub layers: Vec<LayerId>,
}

impl ContractMapRenderOptions {
    /// Returns an empty option set — renders every layer and every kind,
    /// with all overlays disabled.
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_contract_map_render_options_default_has_empty_layers() {
        let opts = ContractMapRenderOptions::default();
        assert!(opts.layers.is_empty());
    }

    #[test]
    fn test_contract_map_render_options_empty_matches_default() {
        assert_eq!(ContractMapRenderOptions::empty(), ContractMapRenderOptions::default());
    }
}
