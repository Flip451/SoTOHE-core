//! Compatibility-preserving presentation of Signal command failures.
//!
//! The interactor owns policy and orchestration; this module keeps the legacy
//! command-specific output contract isolated from that control flow.

use super::{
    SignalCommandInteractor, SignalCommandOutput, SignalCommandPortError, SignalGateConfigError,
    SignalGateName,
};

impl SignalCommandInteractor {
    pub(super) fn catalogue_command_error(
        error: SignalCommandPortError,
        command_label: Option<&str>,
    ) -> SignalCommandOutput {
        match (command_label, error) {
            (Some(command_label), SignalCommandPortError::RepositoryDiscovery { reason }) => {
                SignalCommandOutput::failure(Some(format!(
                    "[ERROR] {command_label}: cannot discover git repo: {reason}"
                )))
            }
            (_, error) => Self::command_error(error),
        }
    }

    pub(super) fn command_error(error: SignalCommandPortError) -> SignalCommandOutput {
        let message = match error {
            SignalCommandPortError::RepositoryDiscovery { reason } => format!(
                "[BLOCKED] cannot discover git repository: {reason}; \
                 pass --workspace-root or --spec-json explicitly"
            ),
            SignalCommandPortError::BranchAbsent => {
                "[ERROR] signal calc-impl-catalog: cannot read current branch".to_owned()
            }
            SignalCommandPortError::BranchReadFailure { reason } => {
                format!("[ERROR] signal calc-impl-catalog: cannot read current branch: {reason}")
            }
            SignalCommandPortError::SpecPathResolution { reason } => format!(
                "[BLOCKED] cannot resolve spec.json from active track: {reason}; \
                 pass --workspace-root or --spec-json explicitly"
            ),
            // The adapter owns the command-specific, sanitized presentation for
            // persistence and execution failures. Adding a usecase prefix would
            // change the established CLI contract.
            SignalCommandPortError::Persistence { reason }
            | SignalCommandPortError::Execution { reason } => reason.to_string(),
        };
        SignalCommandOutput::failure(Some(message))
    }

    pub(super) fn gate_config_error(error: SignalGateConfigError) -> SignalCommandOutput {
        let message = match error {
            SignalGateConfigError::RepositoryDiscovery { reason } => {
                format!("cannot discover git repository: {reason}")
            }
            SignalGateConfigError::ConfigurationNotFound { path } => format!(
                "[ERROR] failed to load signal-gates config from {}: \
                 signal-gates.json not found at {}: place the recommended default config at that path and retry",
                path.display(),
                path.display(),
            ),
            SignalGateConfigError::ConfigurationInvalid { path, reason } => format!(
                "[ERROR] failed to load signal-gates config from {}: {reason}",
                path.display(),
            ),
        };
        SignalCommandOutput::failure(Some(message))
    }

    pub(super) fn gate_preflight_error(
        gate: SignalGateName,
        error: SignalCommandPortError,
    ) -> SignalCommandOutput {
        let prefix = format!("[BLOCKED] signal check --gate {gate:?}:");
        let message = match error {
            SignalCommandPortError::RepositoryDiscovery { reason } => format!(
                "{prefix} cannot discover git repository: {reason}; pass --workspace-root explicitly"
            ),
            error => format!("{prefix} {error}"),
        };
        SignalCommandOutput::failure(Some(message))
    }
}
