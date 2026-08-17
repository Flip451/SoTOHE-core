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
    ///
    /// Only constructs and injects the fixpoint-resolve adapter chain — never
    /// calls `FixpointResolveDriverService::fixpoint_resolve` itself (ADR
    /// 2026-06-21-1328 D2: composition root is wire-only).
    pub fn track_driver(&self) -> cli_driver::track::TrackDriver {
        build_track_driver()
    }

    /// Build the focused TDDD baseline-capture driver.
    pub fn track_tddd_driver(&self) -> cli_driver::track_tddd::TrackTdddDriver {
        build_track_tddd_driver()
    }

    /// Build the compatibility resolution driver used by foreign command
    /// contexts while the legacy resolution wrappers remain in place.
    pub fn track_resolution_driver(&self) -> cli_driver::track_resolution::TrackResolutionDriver {
        use std::sync::Arc;

        let adapter = Arc::new(
            infrastructure::track_lifecycle::resolution_compat::SystemTrackResolutionAdapter,
        );
        let service = Arc::new(
            usecase::track_lifecycle::resolution_compat::TrackResolutionInteractor::new(adapter),
        );
        cli_driver::track_resolution::TrackResolutionDriver::new(service)
    }
}

pub(crate) fn build_track_driver() -> cli_driver::track::TrackDriver {
    use std::sync::Arc;

    use super::service_impl::TrackServiceImpl;

    let track_init_service =
        Arc::new(usecase::track_lifecycle::track_init::TrackInitInteractor::new(
            Arc::new(infrastructure::track::FsTrackMetadataAdapter::new()),
            Arc::new(infrastructure::track::FsTrackBranchStrategyAdapter),
            Arc::new(infrastructure::track::FsTrackViewsAdapter::new()),
        ));
    let service = Arc::new(TrackServiceImpl);
    let fixpoint_resolve_service =
        Arc::new(usecase::fixpoint_resolve_driver::FixpointResolveDriverInteractor::new(
            Arc::new(
                infrastructure::track::fixpoint_resolve_driver::FsFixpointWorkspaceContextAdapter,
            ),
            Arc::new(infrastructure::track::fixpoint_resolve_driver::FsDryCheckConfigLoaderAdapter),
            Arc::new(
                infrastructure::track::fixpoint_resolve_driver::FsFixpointDryGateFactoryAdapter,
            ),
            Arc::new(
                infrastructure::track::fixpoint_resolve_driver::FsFixpointGateStateFactoryAdapter,
            ),
        ));
    let base_merge_cleanup: Arc<dyn usecase::base_merge::BaseMergeCleanupPort> =
        Arc::new(infrastructure::base_merge::FsBaseMergeCleanupAdapter::new());
    let base_merge_service = Arc::new(usecase::base_merge::BaseMergeInteractor::new(
        Arc::new(infrastructure::base_merge::FsBaseMergeContextAdapter::new()),
        Arc::new(infrastructure::base_merge::FsBaseMergeGitAdapter::new()),
        base_merge_cleanup,
    ));
    cli_driver::track::TrackDriver::new(
        track_init_service,
        service,
        fixpoint_resolve_service,
        base_merge_service,
    )
}

pub(crate) fn build_track_tddd_driver() -> cli_driver::track_tddd::TrackTdddDriver {
    use std::sync::Arc;

    let operation = Arc::new(
        infrastructure::track_lifecycle::tddd::baseline_capture::SystemTrackBaselineCaptureAdapter,
    );
    let resolver = Arc::new(infrastructure::track::GitTrackSelectionAdapter);
    let service = Arc::new(
        usecase::track_lifecycle::tddd::baseline_capture::TrackBaselineCaptureInteractor::new(
            operation, resolver,
        ),
    );
    cli_driver::track_tddd::TrackTdddDriver::new(service)
}
