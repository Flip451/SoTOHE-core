//! `signal_check_gate` — chain ⓪①②③ aggregate evaluation for commit/merge gates.

use std::path::PathBuf;

use super::{
    SignalCompositionRoot, SignalGateName, load_gate_matrix, merge_outcomes, resolve_spec_json_path,
};
use crate::signal_layer_chain::signal_check_layer_chain_with_strict;
use crate::{CommandOutcome, cmd_outcome::render_outcome, error::CompositionError};
use infrastructure::verify::tddd_layers::TdddLayerBinding;

impl SignalCompositionRoot {
    /// Evaluate the commit-gate or merge-gate (chains ⓪①②③) and return a merged outcome.
    pub fn signal_check_gate(
        &self,
        project_root: Option<PathBuf>,
        spec_json_path: Option<PathBuf>,
        gate: SignalGateName,
        workspace_root: Option<PathBuf>,
    ) -> Result<CommandOutcome, CompositionError> {
        use std::sync::Arc;

        use infrastructure::signal_layer_reader::LocalSignalLayerReaderAdapter;
        use usecase::signal::SignalLayerReader as _;
        use usecase::signal_gate::{
            AdrChainRunnerPort, ChainRunnerError, LayerChainRunnerPort, SignalChainOutput,
            SignalGateCommand, SignalGateInteractor, SignalGateService, SpecAdrChainRunnerPort,
        };

        let matrix = match load_gate_matrix(workspace_root.as_deref()) {
            Ok(m) => m,
            Err(outcome) => return Ok(outcome),
        };

        let resolved_root: PathBuf = match workspace_root.clone() {
            Some(root) => root,
            None => {
                use infrastructure::git_cli::{GitRepository as _, SystemGitRepo};
                match SystemGitRepo::discover() {
                    Ok(repo) => repo.root().to_path_buf(),
                    Err(e) => {
                        return Ok(CommandOutcome::failure(Some(format!(
                            "[BLOCKED] signal check --gate {gate:?}: cannot discover git \
                             repository: {e}; pass --workspace-root explicitly"
                        ))));
                    }
                }
            }
        };

        let project_root = project_root.unwrap_or_else(|| resolved_root.clone());

        let spec_json_path = match resolve_spec_json_path(workspace_root.as_deref(), spec_json_path)
        {
            Ok(p) => p,
            Err(outcome) => return Ok(outcome),
        };

        let signal_layer_reader =
            Arc::new(LocalSignalLayerReaderAdapter::new(resolved_root.clone()));
        let items_dir = resolved_root.join("track/items");
        let track_id = match signal_layer_reader.active_track_id() {
            Ok(id) => id,
            Err(e) => {
                return Ok(CommandOutcome::failure(Some(format!(
                    "[BLOCKED] signal check --gate {gate:?}: cannot resolve active track ID: {e}"
                ))));
            }
        };

        let gate_label = match gate {
            SignalGateName::Commit => "signal check --gate commit",
            SignalGateName::Merge => "signal check --gate merge",
        };

        struct AdrChainAdapter {
            project_root: PathBuf,
        }

        impl AdrChainRunnerPort for AdrChainAdapter {
            fn run_adr_chain(
                &self,
                _project_root: PathBuf,
                strict: bool,
            ) -> Result<SignalChainOutput, ChainRunnerError> {
                let outcome =
                    infrastructure::verify::adr_signals::execute_verify_adr_signals_with_strict(
                        &self.project_root,
                        strict,
                    );
                let cmd_outcome = render_outcome("signal check-adr-user", &outcome);
                Ok(SignalChainOutput {
                    chain_label: "signal check-adr-user".to_owned(),
                    passed: cmd_outcome.exit_code == 0,
                    stdout: cmd_outcome.stdout,
                    stderr: cmd_outcome.stderr,
                })
            }
        }

        struct SpecAdrChainAdapter {
            spec_json_path: PathBuf,
        }

        impl SpecAdrChainRunnerPort for SpecAdrChainAdapter {
            fn run_spec_adr_chain(
                &self,
                _spec_json_path: PathBuf,
                strict: bool,
            ) -> Result<SignalChainOutput, ChainRunnerError> {
                let spec_json_path = self.spec_json_path.clone();
                let outcome = match infrastructure::verify::trusted_root::resolve_trusted_root(
                    &spec_json_path,
                ) {
                    Ok(trusted_root) => infrastructure::verify::spec_states::verify_from_spec_json(
                        spec_json_path.clone(),
                        strict,
                        trusted_root,
                    ),
                    Err(e) => infrastructure::verify::VerifyOutcome::from_findings(vec![
                        infrastructure::verify::VerifyFinding::error(format!(
                            "cannot resolve trusted_root for {}: {e}",
                            spec_json_path.display()
                        )),
                    ]),
                };
                let cmd_outcome = render_outcome("signal check-spec-adr", &outcome);
                Ok(SignalChainOutput {
                    chain_label: "signal check-spec-adr".to_owned(),
                    passed: cmd_outcome.exit_code == 0,
                    stdout: cmd_outcome.stdout,
                    stderr: cmd_outcome.stderr,
                })
            }
        }

        struct LayerChainAdapter {
            workspace_root: Option<PathBuf>,
        }

        impl LayerChainRunnerPort for LayerChainAdapter {
            fn run_catalog_spec_chain(
                &self,
                strict: bool,
                _signal_reader: &dyn usecase::signal::SignalLayerReader,
            ) -> Result<SignalChainOutput, ChainRunnerError> {
                let cmd_outcome = signal_check_layer_chain_with_strict(
                    strict,
                    self.workspace_root.clone(),
                    "signal check-catalog-spec",
                    infrastructure::verify::tddd_layers::catalogue_spec_signals_path,
                    TdddLayerBinding::catalogue_spec_signal_enabled,
                    false,
                    |signals_path, hash_hex, s| {
                        infrastructure::verify::catalogue_spec_signals::check_catalog_spec_from_signals_file(
                            signals_path, hash_hex, s,
                        )
                    },
                    |reader, per_layer_fn| {
                        usecase::signal::check_catalog_spec(reader, per_layer_fn)
                    },
                )
                .map_err(|e| ChainRunnerError::ExecutionFailed(e.to_string()))?;
                Ok(SignalChainOutput {
                    chain_label: "signal check-catalog-spec".to_owned(),
                    passed: cmd_outcome.exit_code == 0,
                    stdout: cmd_outcome.stdout,
                    stderr: cmd_outcome.stderr,
                })
            }

            fn run_impl_catalog_chain(
                &self,
                strict: bool,
                _signal_reader: &dyn usecase::signal::SignalLayerReader,
            ) -> Result<SignalChainOutput, ChainRunnerError> {
                let cmd_outcome = signal_check_layer_chain_with_strict(
                    strict,
                    self.workspace_root.clone(),
                    "signal check-impl-catalog",
                    infrastructure::verify::tddd_layers::impl_catalog_signals_path,
                    |_| true,
                    true,
                    |signals_path, hash_hex, s| {
                        infrastructure::verify::spec_states::check_impl_catalog_from_signals_file(
                            signals_path,
                            hash_hex,
                            s,
                        )
                    },
                    |reader, per_layer_fn| {
                        usecase::signal::check_impl_catalog(reader, per_layer_fn)
                    },
                )
                .map_err(|e| ChainRunnerError::ExecutionFailed(e.to_string()))?;
                Ok(SignalChainOutput {
                    chain_label: "signal check-impl-catalog".to_owned(),
                    passed: cmd_outcome.exit_code == 0,
                    stdout: cmd_outcome.stdout,
                    stderr: cmd_outcome.stderr,
                })
            }
        }

        let adr_adapter = Arc::new(AdrChainAdapter { project_root });
        let spec_adr_adapter = Arc::new(SpecAdrChainAdapter { spec_json_path });
        let layer_adapter = Arc::new(LayerChainAdapter { workspace_root });

        let interactor = SignalGateInteractor::new(
            signal_layer_reader,
            matrix,
            adr_adapter,
            spec_adr_adapter,
            layer_adapter,
        );

        let cmd = SignalGateCommand { gate_label: gate_label.to_owned(), items_dir, track_id };

        let gate_output = match interactor.run_gate(cmd) {
            Ok(o) => o,
            Err(e) => {
                return Ok(CommandOutcome::failure(Some(format!(
                    "[ERROR] signal check --gate {gate:?}: {e}"
                ))));
            }
        };

        let chain_outcomes: Vec<CommandOutcome> = gate_output
            .chain_outputs
            .into_iter()
            .map(|c| CommandOutcome {
                stdout: c.stdout,
                stderr: c.stderr,
                exit_code: if c.passed { 0 } else { 1 },
            })
            .collect();

        Ok(merge_outcomes(gate_label, chain_outcomes))
    }
}
