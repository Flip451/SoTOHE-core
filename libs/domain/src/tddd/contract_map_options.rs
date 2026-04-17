//! Render-configuration value object for the Contract Map (ADR
//! 2026-04-17-1528 §D1).
//!
//! The 5 fields are part of the Phase 1 public API so that Phase 2 and
//! Phase 3 extensions can be wired in without breaking callers. Three of
//! them (`signal_overlay`, `action_overlay`, `include_spec_source_edges`)
//! are **stubs in Phase 1** — the render function ignores their value and
//! the output is identical regardless of how they are set. This keeps the
//! type's shape stable across phases.

use crate::tddd::LayerId;
use crate::tddd::catalogue::TypeDefinitionKind;

/// Options that drive `render_contract_map`.
///
/// * `layers` — if non-empty, restricts the rendered subgraphs to this
///   layer subset (preserving `layer_order` ordering). An empty `layers`
///   list means "render every entry in `layer_order`".
/// * `kind_filter` — if `Some`, restricts rendered entries to the listed
///   [`TypeDefinitionKind`]s (compared by `kind_tag`). `None` means "render
///   every kind". An explicit `Some(vec![])` intentionally filters every
///   entry out and produces an empty-subgraph diagram (not an error), so
///   CLI callers can surface that as a warning instead of failing.
/// * `signal_overlay` (Phase 2 stub) — will paint nodes by
///   Blue/Yellow/Red signal.
/// * `action_overlay` (Phase 2 stub) — will visualise add / modify /
///   delete / reference actions via stroke styles.
/// * `include_spec_source_edges` (Phase 3 stub) — will emit outbound edges
///   to spec sections once `spec_source` becomes a catalogue-level field.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ContractMapRenderOptions {
    pub layers: Vec<LayerId>,
    pub kind_filter: Option<Vec<TypeDefinitionKind>>,
    pub signal_overlay: bool,
    pub action_overlay: bool,
    pub include_spec_source_edges: bool,
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
    fn test_contract_map_render_options_default_is_empty_unfiltered() {
        let opts = ContractMapRenderOptions::default();
        assert!(opts.layers.is_empty());
        assert!(opts.kind_filter.is_none());
        assert!(!opts.signal_overlay);
        assert!(!opts.action_overlay);
        assert!(!opts.include_spec_source_edges);
    }

    #[test]
    fn test_contract_map_render_options_empty_matches_default() {
        assert_eq!(ContractMapRenderOptions::empty(), ContractMapRenderOptions::default());
    }
}
