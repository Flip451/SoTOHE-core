//! `SignalServiceImpl` — implementation of `usecase::signal_service::SignalService`
//! for use in the `cli_composition` factory.
//!
//! Delegates each method to the corresponding [`SignalCompositionRoot`] method,
//! converting `Result<CommandOutcome, CompositionError>` → `SignalCommandOutput`.
//! This adapter is the bridge between the usecase port and the existing
//! composition-root logic, so no business logic is duplicated.

use std::path::PathBuf;

use usecase::signal_service::{SignalCommandOutput, SignalGateName, SignalService};

use super::SignalCompositionRoot;
use crate::signal::SignalGateName as CompositionSignalGateName;

#[cfg(feature = "test-support")]
#[derive(Clone)]
pub(crate) struct ImplCatalogTestContext {
    workspace_root: PathBuf,
    track_id: domain::TrackId,
    launch_observer: infrastructure::tddd::type_signals_evaluator::RustdocLaunchObserver,
}

fn to_service_gate(gate: SignalGateName) -> CompositionSignalGateName {
    match gate {
        SignalGateName::Commit => CompositionSignalGateName::Commit,
        SignalGateName::Merge => CompositionSignalGateName::Merge,
    }
}

fn composition_to_output(
    result: Result<crate::CommandOutcome, crate::error::CompositionError>,
) -> SignalCommandOutput {
    match result {
        Ok(outcome) => SignalCommandOutput {
            stdout: outcome.stdout,
            stderr: outcome.stderr,
            exit_code: outcome.exit_code,
        },
        Err(e) => SignalCommandOutput::failure(Some(format!("[ERROR] {e}"))),
    }
}

/// Implementation of [`SignalService`] that delegates to [`SignalCompositionRoot`].
///
/// Constructed by the `signal_driver()` factory in the composition root.
pub struct SignalServiceImpl {
    #[cfg(feature = "test-support")]
    impl_catalog_test_context: Option<ImplCatalogTestContext>,
}

impl SignalServiceImpl {
    pub(crate) fn new() -> Self {
        Self {
            #[cfg(feature = "test-support")]
            impl_catalog_test_context: None,
        }
    }

    #[cfg(feature = "test-support")]
    pub(crate) fn for_test_workspace(
        workspace_root: PathBuf,
        track_id: domain::TrackId,
        launch_observer: infrastructure::tddd::type_signals_evaluator::RustdocLaunchObserver,
    ) -> Self {
        Self {
            impl_catalog_test_context: Some(ImplCatalogTestContext {
                workspace_root,
                track_id,
                launch_observer,
            }),
        }
    }
}

impl SignalService for SignalServiceImpl {
    fn calc_adr_user(&self, project_root: PathBuf) -> SignalCommandOutput {
        composition_to_output(SignalCompositionRoot::new().signal_calc_adr_user(project_root))
    }

    fn check_adr_user(
        &self,
        project_root: PathBuf,
        strict_override: bool,
        gate: Option<SignalGateName>,
        workspace_root: Option<PathBuf>,
    ) -> SignalCommandOutput {
        composition_to_output(SignalCompositionRoot::new().signal_check_adr_user(
            project_root,
            strict_override.then_some(domain::Strictness::Strict),
            gate.map(to_service_gate),
            workspace_root,
        ))
    }

    fn calc_spec_adr(
        &self,
        spec_json_path: Option<PathBuf>,
        workspace_root: Option<PathBuf>,
    ) -> SignalCommandOutput {
        composition_to_output(
            SignalCompositionRoot::new().signal_calc_spec_adr(spec_json_path, workspace_root),
        )
    }

    fn check_spec_adr(
        &self,
        spec_json_path: Option<PathBuf>,
        strict_override: bool,
        gate: Option<SignalGateName>,
        workspace_root: Option<PathBuf>,
    ) -> SignalCommandOutput {
        composition_to_output(SignalCompositionRoot::new().signal_check_spec_adr(
            spec_json_path,
            strict_override.then_some(domain::Strictness::Strict),
            gate.map(to_service_gate),
            workspace_root,
        ))
    }

    fn calc_catalog_spec(&self) -> SignalCommandOutput {
        composition_to_output(SignalCompositionRoot::new().signal_calc_catalog_spec())
    }

    fn check_catalog_spec(
        &self,
        strict_override: bool,
        gate: Option<SignalGateName>,
        workspace_root: Option<PathBuf>,
    ) -> SignalCommandOutput {
        composition_to_output(SignalCompositionRoot::new().signal_check_catalog_spec(
            strict_override.then_some(domain::Strictness::Strict),
            gate.map(to_service_gate),
            workspace_root,
        ))
    }

    fn calc_impl_catalog(&self) -> SignalCommandOutput {
        #[cfg(feature = "test-support")]
        if let Some(context) = &self.impl_catalog_test_context {
            return composition_to_output(
                SignalCompositionRoot::new().signal_calc_impl_catalog_for_test_workspace(
                    context.workspace_root.clone(),
                    context.track_id.clone(),
                    context.launch_observer.clone(),
                ),
            );
        }
        composition_to_output(SignalCompositionRoot::new().signal_calc_impl_catalog())
    }

    fn check_impl_catalog(
        &self,
        strict_override: bool,
        gate: Option<SignalGateName>,
        workspace_root: Option<PathBuf>,
    ) -> SignalCommandOutput {
        composition_to_output(SignalCompositionRoot::new().signal_check_impl_catalog(
            strict_override.then_some(domain::Strictness::Strict),
            gate.map(to_service_gate),
            workspace_root,
        ))
    }

    fn check_gate(
        &self,
        project_root: Option<PathBuf>,
        spec_json_path: Option<PathBuf>,
        gate: SignalGateName,
        workspace_root: Option<PathBuf>,
    ) -> SignalCommandOutput {
        composition_to_output(SignalCompositionRoot::new().signal_check_gate(
            project_root,
            spec_json_path,
            to_service_gate(gate),
            workspace_root,
        ))
    }
}
