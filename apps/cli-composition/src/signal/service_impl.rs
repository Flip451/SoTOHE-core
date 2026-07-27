#![allow(dead_code)]
//! Legacy `SignalService` implementation retained for the final convergence track.
//!
//! It is not used by production wiring. Its delegation stays on the typed
//! interactor path so the retained compatibility type cannot reintroduce a
//! composition-root execution path.

use std::path::PathBuf;
use std::sync::Arc;

use usecase::signal_service::{
    SignalCommandInteractor, SignalCommandOutput, SignalGateName, SignalService,
};

/// Compatibility implementation retained without runtime use by the Signal root.
pub struct SignalServiceImpl {
    interactor: SignalCommandInteractor,
}

impl SignalServiceImpl {
    pub(crate) fn new() -> Self {
        let adapter = Arc::new(infrastructure::signal::SystemSignalCommandAdapter::new());
        let gate_config = Arc::new(infrastructure::signal::SystemSignalGateConfigAdapter::new());
        Self {
            interactor: SignalCommandInteractor::new(
                adapter.clone(),
                adapter.clone(),
                adapter,
                gate_config,
            ),
        }
    }

    #[cfg(feature = "test-support")]
    pub(crate) fn for_test_workspace(
        workspace_root: PathBuf,
        track_id: domain::TrackId,
        launch_observer: infrastructure::tddd::type_signals_evaluator::RustdocLaunchObserver,
    ) -> Self {
        let adapter = Arc::new(infrastructure::signal::SystemSignalCommandAdapter::with_test_context(
            workspace_root,
            track_id,
            launch_observer,
        ));
        let gate_config = Arc::new(infrastructure::signal::SystemSignalGateConfigAdapter::new());
        Self {
            interactor: SignalCommandInteractor::new(
                adapter.clone(),
                adapter.clone(),
                adapter,
                gate_config,
            ),
        }
    }
}

impl SignalService for SignalServiceImpl {
    fn calc_adr_user(&self, project_root: PathBuf) -> SignalCommandOutput {
        self.interactor.calc_adr_user(project_root)
    }

    fn check_adr_user(
        &self,
        project_root: PathBuf,
        strict_override: bool,
        gate: Option<SignalGateName>,
        workspace_root: Option<PathBuf>,
    ) -> SignalCommandOutput {
        self.interactor.check_adr_user(project_root, strict_override, gate, workspace_root)
    }

    fn calc_spec_adr(
        &self,
        spec_json_path: Option<PathBuf>,
        workspace_root: Option<PathBuf>,
    ) -> SignalCommandOutput {
        self.interactor.calc_spec_adr(spec_json_path, workspace_root)
    }

    fn check_spec_adr(
        &self,
        spec_json_path: Option<PathBuf>,
        strict_override: bool,
        gate: Option<SignalGateName>,
        workspace_root: Option<PathBuf>,
    ) -> SignalCommandOutput {
        self.interactor.check_spec_adr(spec_json_path, strict_override, gate, workspace_root)
    }

    fn calc_catalog_spec(&self) -> SignalCommandOutput {
        self.interactor.calc_catalog_spec()
    }

    fn check_catalog_spec(
        &self,
        strict_override: bool,
        gate: Option<SignalGateName>,
        workspace_root: Option<PathBuf>,
    ) -> SignalCommandOutput {
        self.interactor.check_catalog_spec(strict_override, gate, workspace_root)
    }

    fn calc_impl_catalog(&self) -> SignalCommandOutput {
        self.interactor.calc_impl_catalog()
    }

    fn check_impl_catalog(
        &self,
        strict_override: bool,
        gate: Option<SignalGateName>,
        workspace_root: Option<PathBuf>,
    ) -> SignalCommandOutput {
        self.interactor.check_impl_catalog(strict_override, gate, workspace_root)
    }

    fn check_gate(
        &self,
        project_root: Option<PathBuf>,
        spec_json_path: Option<PathBuf>,
        gate: SignalGateName,
        workspace_root: Option<PathBuf>,
    ) -> SignalCommandOutput {
        self.interactor.check_gate(project_root, spec_json_path, gate, workspace_root)
    }
}
