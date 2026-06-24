//! Per-context composition root for the `track` command family.
//!
//! `TrackCompositionRoot` replaces the `CliApp` god-facade for all `track`
//! subcommands.  The struct is a unit struct because no adapter dependencies
//! are injected at construction time — each method constructs its own adapters
//! inline from the arguments it receives (hexagonal composition pattern).
//!
//! `CliApp` keeps backward-compatible shim methods in `track/shim.rs` that
//! construct a `TrackCompositionRoot` and delegate, so all existing call-sites
//! in `apps/cli` continue to compile without change.

/// Composition root for the `track` command family.
///
/// This is a unit struct: no adapter dependencies are injected at construction
/// time.  All port adapters are wired inside individual methods from the
/// runtime arguments they receive (in-method composition).
pub struct TrackCompositionRoot;

impl TrackCompositionRoot {
    /// Create a new `TrackCompositionRoot`.
    pub fn new() -> Self {
        Self
    }
}

impl Default for TrackCompositionRoot {
    fn default() -> Self {
        Self::new()
    }
}

impl TrackCompositionRoot {
    /// Build a wired [`cli_driver::track::TrackDriver`] for the track family.
    pub fn track_driver(&self) -> cli_driver::track::TrackDriver {
        use std::sync::Arc;

        use super::service_impl::TrackServiceImpl;

        let service = Arc::new(TrackServiceImpl);
        cli_driver::track::TrackDriver::new(service)
    }
}
