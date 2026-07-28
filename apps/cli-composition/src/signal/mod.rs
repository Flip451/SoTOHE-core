//! Pure DI composition root for the `signal` command family.

mod shim;
#[cfg(test)]
mod tests;

use std::sync::Arc;

#[cfg(all(test, feature = "test-support"))]
use std::path::PathBuf;

/// Legacy gate-name DTO retained for public compatibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalGateName {
    /// CI commit gate.
    Commit,
    /// PR merge gate.
    Merge,
}

/// Composition root for the `signal` command family.
pub struct SignalCompositionRoot;

impl SignalCompositionRoot {
    /// Create a new `SignalCompositionRoot`.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Build the fully wired Signal driver.
    #[must_use]
    pub fn signal_driver(&self) -> cli_driver::signal::SignalDriver {
        let adapter = Arc::new(infrastructure::signal::SystemSignalCommandAdapter::new());
        let gate_config = Arc::new(infrastructure::signal::SystemSignalGateConfigAdapter::new());
        self.signal_driver_with_ports(adapter.clone(), adapter.clone(), adapter, gate_config)
    }

    fn signal_driver_with_ports(
        &self,
        port: Arc<dyn usecase::signal_service::SignalCommandPort>,
        active_track_resolver: Arc<dyn usecase::signal_service::SignalActiveTrackResolverPort>,
        spec_path_resolver: Arc<dyn usecase::signal_service::SignalSpecPathResolverPort>,
        gate_config: Arc<dyn usecase::signal_service::SignalGateConfigPort>,
    ) -> cli_driver::signal::SignalDriver {
        let service = Arc::new(usecase::signal_service::SignalCommandInteractor::new(
            port,
            active_track_resolver,
            spec_path_resolver,
            gate_config,
        ));
        cli_driver::signal::SignalDriver::new(service)
    }

    #[cfg(all(test, feature = "test-support"))]
    pub(crate) fn signal_driver_for_test_workspace(
        &self,
        workspace_root: PathBuf,
        track_id: domain::TrackId,
        launch_observer: infrastructure::tddd::type_signals_evaluator::RustdocLaunchObserver,
    ) -> cli_driver::signal::SignalDriver {
        let adapter =
            Arc::new(infrastructure::signal::SystemSignalCommandAdapter::with_test_context(
                workspace_root,
                track_id,
                launch_observer,
            ));
        let gate_config = Arc::new(infrastructure::signal::SystemSignalGateConfigAdapter::new());
        self.signal_driver_with_ports(adapter.clone(), adapter.clone(), adapter, gate_config)
    }
}

impl Default for SignalCompositionRoot {
    fn default() -> Self {
        Self::new()
    }
}
