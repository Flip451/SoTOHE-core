//! `impl CliApp` delegation shims for the signal command family.
//!
//! Each method forwards to `SignalCompositionRoot::new().method(...)`,
//! preserving `apps/cli` call sites unchanged during the per-context dissolution
//! migration (T012). T013 / T021 will remove `CliApp` entirely.

use std::path::PathBuf;

use super::{SignalCompositionRoot, SignalGateName};
use crate::{CliApp, CommandOutcome, error::CompositionError};

impl CliApp {
    /// Delegates to [`SignalCompositionRoot::signal_calc_adr_user`].
    pub fn signal_calc_adr_user(
        &self,
        project_root: PathBuf,
    ) -> Result<CommandOutcome, CompositionError> {
        SignalCompositionRoot::new().signal_calc_adr_user(project_root)
    }

    /// Delegates to [`SignalCompositionRoot::signal_check_adr_user`].
    pub fn signal_check_adr_user(
        &self,
        project_root: PathBuf,
        strict_override: bool,
        gate: Option<SignalGateName>,
        workspace_root: Option<PathBuf>,
    ) -> Result<CommandOutcome, CompositionError> {
        SignalCompositionRoot::new().signal_check_adr_user(
            project_root,
            strict_override,
            gate,
            workspace_root,
        )
    }

    /// Delegates to [`SignalCompositionRoot::signal_calc_spec_adr`].
    pub fn signal_calc_spec_adr(
        &self,
        spec_json_path: Option<PathBuf>,
        workspace_root: Option<PathBuf>,
    ) -> Result<CommandOutcome, CompositionError> {
        SignalCompositionRoot::new().signal_calc_spec_adr(spec_json_path, workspace_root)
    }

    /// Delegates to [`SignalCompositionRoot::signal_check_spec_adr`].
    pub fn signal_check_spec_adr(
        &self,
        spec_json_path: Option<PathBuf>,
        strict_override: bool,
        gate: Option<SignalGateName>,
        workspace_root: Option<PathBuf>,
    ) -> Result<CommandOutcome, CompositionError> {
        SignalCompositionRoot::new().signal_check_spec_adr(
            spec_json_path,
            strict_override,
            gate,
            workspace_root,
        )
    }

    /// Delegates to [`SignalCompositionRoot::signal_calc_catalog_spec`].
    pub fn signal_calc_catalog_spec(&self) -> Result<CommandOutcome, CompositionError> {
        SignalCompositionRoot::new().signal_calc_catalog_spec()
    }

    /// Delegates to [`SignalCompositionRoot::signal_check_catalog_spec`].
    pub fn signal_check_catalog_spec(
        &self,
        strict_override: bool,
        gate: Option<SignalGateName>,
        workspace_root: Option<PathBuf>,
    ) -> Result<CommandOutcome, CompositionError> {
        SignalCompositionRoot::new().signal_check_catalog_spec(
            strict_override,
            gate,
            workspace_root,
        )
    }

    /// Delegates to [`SignalCompositionRoot::signal_calc_impl_catalog`].
    pub fn signal_calc_impl_catalog(&self) -> Result<CommandOutcome, CompositionError> {
        SignalCompositionRoot::new().signal_calc_impl_catalog()
    }

    /// Delegates to [`SignalCompositionRoot::signal_check_impl_catalog`].
    pub fn signal_check_impl_catalog(
        &self,
        strict_override: bool,
        gate: Option<SignalGateName>,
        workspace_root: Option<PathBuf>,
    ) -> Result<CommandOutcome, CompositionError> {
        SignalCompositionRoot::new().signal_check_impl_catalog(
            strict_override,
            gate,
            workspace_root,
        )
    }

    /// Delegates to [`SignalCompositionRoot::signal_check_gate`].
    pub fn signal_check_gate(
        &self,
        project_root: Option<PathBuf>,
        spec_json_path: Option<PathBuf>,
        gate: SignalGateName,
        workspace_root: Option<PathBuf>,
    ) -> Result<CommandOutcome, CompositionError> {
        SignalCompositionRoot::new().signal_check_gate(
            project_root,
            spec_json_path,
            gate,
            workspace_root,
        )
    }
}
