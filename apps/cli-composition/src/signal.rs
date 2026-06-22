//! `signal` command family for [`CliApp`].

use std::path::{Path, PathBuf};

use crate::signal_layer_chain::{
    BindingSignalLayerReader, signal_check_layer_chain, signal_check_layer_chain_with_strict,
};
use crate::{CliApp, CommandOutcome, cmd_outcome::render_outcome};
use infrastructure::verify::tddd_layers::TdddLayerBinding;

/// Selects the gate context when resolving strictness from `signal-gates.json`.
///
/// Maps one-to-one to `domain::GateKind`; defined here so that `apps/cli` can
/// pass the value without importing `domain`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalGateName {
    /// CI commit gate — evaluates using the `commit_gate.*` cells.
    Commit,
    /// PR merge gate — evaluates using the `merge_gate.*` cells.
    Merge,
}

fn merge_outcomes(label: &str, outcomes: Vec<CommandOutcome>) -> CommandOutcome {
    let mut all_lines: Vec<String> = vec![format!("--- {label} ---")];
    let mut any_failure = false;

    for outcome in outcomes {
        if let Some(ref s) = outcome.stdout {
            all_lines.push(s.clone());
        }
        if let Some(ref s) = outcome.stderr {
            all_lines.push(s.clone());
        }
        if outcome.exit_code != 0 {
            any_failure = true;
        }
    }

    let summary = if any_failure {
        format!("--- {label} FAILED ---")
    } else {
        format!("--- {label} PASSED ---")
    };
    all_lines.push(summary);
    let exit_code = if any_failure { 1 } else { 0 };
    CommandOutcome { stdout: Some(all_lines.join("\n")), stderr: None, exit_code }
}

fn load_gate_matrix(
    workspace_root: Option<&Path>,
) -> Result<domain::SignalGateMatrix, CommandOutcome> {
    let config_path = match workspace_root {
        Some(root) => root.join(".harness/config/signal-gates.json"),
        None => {
            use infrastructure::git_cli::GitRepository as _;
            let repo = infrastructure::git_cli::SystemGitRepo::discover().map_err(|e| {
                CommandOutcome::failure(Some(format!("cannot discover git repository: {e}")))
            })?;
            repo.root().join(".harness/config/signal-gates.json")
        }
    };

    infrastructure::verify::signal_gates_config::load_signal_gates_config(config_path.clone())
        .map_err(|e| {
            CommandOutcome::failure(Some(format!(
                "[ERROR] failed to load signal-gates config from {}: {e}",
                config_path.display()
            )))
        })
}

pub(crate) fn resolve_strict(
    strict_override: bool,
    gate: Option<SignalGateName>,
    chain_id: domain::ChainId,
    workspace_root: Option<&Path>,
) -> Result<bool, CommandOutcome> {
    if strict_override {
        return Ok(true);
    }
    let gate_kind = gate_name_to_kind(gate.unwrap_or(SignalGateName::Commit));
    let matrix = load_gate_matrix(workspace_root)?;
    Ok(matrix.resolve(chain_id, gate_kind) == domain::Strictness::Strict)
}

fn gate_name_to_kind(gate: SignalGateName) -> domain::GateKind {
    match gate {
        SignalGateName::Commit => domain::GateKind::Commit,
        SignalGateName::Merge => domain::GateKind::Merge,
    }
}

/// Build the chain ① reader + workspace_root pair and delegate unconditionally
/// to the usecase orchestrator. cli-composition layer is wire-up only (DI) — the
/// `Option<PathBuf>` branching lives in [`usecase::signal::resolve_spec_json_path`];
/// this function never inspects `override_path` to decide its own control flow.
///
/// When `workspace_root` is `None`, `SystemGitRepo::discover()` resolves the repo
/// root from the current working directory.  When `override_path` is `Some`, the
/// usecase short-circuits and returns it verbatim without consulting the reader or
/// `workspace_root`.
fn resolve_spec_json_path(
    workspace_root: Option<&Path>,
    override_path: Option<PathBuf>,
) -> Result<PathBuf, CommandOutcome> {
    use infrastructure::git_cli::{GitRepository as _, SystemGitRepo};
    use infrastructure::signal_layer_reader::LocalSignalLayerReaderAdapter;

    let resolved_root: PathBuf = match workspace_root {
        Some(root) => root.to_path_buf(),
        None => SystemGitRepo::discover().map(|repo| repo.root().to_path_buf()).map_err(|e| {
            CommandOutcome::failure(Some(format!(
                "[BLOCKED] cannot discover git repository: {e}; \
                     pass --workspace-root or --spec-json explicitly"
            )))
        })?,
    };
    let reader = LocalSignalLayerReaderAdapter::new(resolved_root.clone());
    usecase::signal::resolve_spec_json_path(&reader, &resolved_root, override_path).map_err(|e| {
        CommandOutcome::failure(Some(format!(
            "[BLOCKED] cannot resolve spec.json from active track: {e}; \
             pass --workspace-root or --spec-json explicitly"
        )))
    })
}

impl CliApp {
    /// Compute ADR signal grounding live from `project_root/knowledge/adr/`.
    pub fn signal_calc_adr_user(&self, project_root: PathBuf) -> Result<CommandOutcome, String> {
        let outcome = infrastructure::verify::adr_signals::execute_verify_adr_signals_with_strict(
            &project_root,
            false,
        );
        Ok(render_outcome("signal calc-adr-user", &outcome))
    }

    /// Evaluate chain ⓪ (ADR→user) gate.
    pub fn signal_check_adr_user(
        &self,
        project_root: PathBuf,
        strict_override: bool,
        gate: Option<SignalGateName>,
        workspace_root: Option<PathBuf>,
    ) -> Result<CommandOutcome, String> {
        let strict = match resolve_strict(
            strict_override,
            gate,
            domain::ChainId::AdrUser,
            workspace_root.as_deref(),
        ) {
            Ok(s) => s,
            Err(outcome) => return Ok(outcome),
        };

        let outcome = infrastructure::verify::adr_signals::execute_verify_adr_signals_with_strict(
            &project_root,
            strict,
        );
        Ok(render_outcome("signal check-adr-user", &outcome))
    }

    /// Compute and persist chain ① (spec→ADR) signals to `spec.json`.
    ///
    /// When `spec_json_path` is `None`, resolves it from the active track
    /// (current git branch `track/<id>` → `track/items/<id>/spec.json` under
    /// `workspace_root`). Pass `Some(path)` to override.
    pub fn signal_calc_spec_adr(
        &self,
        spec_json_path: Option<PathBuf>,
        workspace_root: Option<PathBuf>,
    ) -> Result<CommandOutcome, String> {
        use infrastructure::spec::codec as spec_codec;
        use infrastructure::track::atomic_write::atomic_write_file;

        let spec_json_path = match resolve_spec_json_path(workspace_root.as_deref(), spec_json_path)
        {
            Ok(p) => p,
            Err(outcome) => return Ok(outcome),
        };

        let json_content = std::fs::read_to_string(&spec_json_path).map_err(|e| {
            format!("signal calc-spec-adr: cannot read {}: {e}", spec_json_path.display())
        })?;
        let mut doc = spec_codec::decode(&json_content)
            .map_err(|e| format!("signal calc-spec-adr: spec.json decode error: {e}"))?;
        let counts = doc.evaluate_signals();
        doc.set_signals(counts);
        let encoded = spec_codec::encode(&doc)
            .map_err(|e| format!("signal calc-spec-adr: spec.json encode error: {e}"))?;
        atomic_write_file(&spec_json_path, format!("{encoded}\n").as_bytes()).map_err(|e| {
            format!("signal calc-spec-adr: cannot write {}: {e}", spec_json_path.display())
        })?;
        let outcome = infrastructure::verify::VerifyOutcome::pass();
        Ok(render_outcome("signal calc-spec-adr", &outcome))
    }

    /// Evaluate chain ① (spec→ADR) gate.
    ///
    /// When `spec_json_path` is `None`, resolves it from the active track
    /// (current git branch `track/<id>` → `track/items/<id>/spec.json` under
    /// `workspace_root`). Pass `Some(path)` to override spec.json resolution.
    ///
    /// Note: `workspace_root` (or a git-discoverable checkout) is **also** required
    /// for gate-matrix resolution (`signal-gates.json`) unless `strict_override` is
    /// `true`.  Passing `--spec-json` alone does not remove the git-discovery
    /// requirement for the check variant; use `--workspace-root` or `--strict` to
    /// satisfy both concerns outside a normal checkout.
    pub fn signal_check_spec_adr(
        &self,
        spec_json_path: Option<PathBuf>,
        strict_override: bool,
        gate: Option<SignalGateName>,
        workspace_root: Option<PathBuf>,
    ) -> Result<CommandOutcome, String> {
        let strict = match resolve_strict(
            strict_override,
            gate,
            domain::ChainId::SpecAdr,
            workspace_root.as_deref(),
        ) {
            Ok(s) => s,
            Err(outcome) => return Ok(outcome),
        };

        let spec_json_path = match resolve_spec_json_path(workspace_root.as_deref(), spec_json_path)
        {
            Ok(p) => p,
            Err(outcome) => return Ok(outcome),
        };

        let trusted_root =
            match infrastructure::verify::trusted_root::resolve_trusted_root(&spec_json_path) {
                Ok(r) => r,
                Err(e) => {
                    return Ok(render_outcome(
                        "signal check-spec-adr",
                        &infrastructure::verify::VerifyOutcome::from_findings(vec![
                            infrastructure::verify::VerifyFinding::error(format!(
                                "cannot resolve trusted_root for {}: {e}",
                                spec_json_path.display()
                            )),
                        ]),
                    ));
                }
            };
        let outcome = infrastructure::verify::spec_states::verify_from_spec_json(
            spec_json_path,
            strict,
            trusted_root,
        );
        Ok(render_outcome("signal check-spec-adr", &outcome))
    }

    /// Compute and persist chain ② (catalog→spec) signals for all TDDD-enabled layers.
    pub fn signal_calc_catalog_spec(&self) -> Result<CommandOutcome, String> {
        use infrastructure::git_cli::{GitRepository as _, SystemGitRepo};
        use infrastructure::signal_layer_reader::LocalSignalLayerReaderAdapter;
        use infrastructure::tddd::fs_catalogue_spec_signals_store::FsCatalogueSpecSignalsStore;
        use infrastructure::verify::tddd_layers::{
            LoadTdddLayersError, load_tddd_layers_from_workspace,
        };

        let repo = SystemGitRepo::discover()
            .map_err(|e| format!("signal calc-catalog-spec: cannot discover git repo: {e}"))?;
        let workspace_root = repo.root().to_path_buf();
        let items_dir = workspace_root.join("track").join("items");

        let bindings: Vec<_> = load_tddd_layers_from_workspace(&workspace_root)
            .map_err(|e| match e {
                LoadTdddLayersError::Io { path, source } => format!("{}: {source}", path.display()),
                LoadTdddLayersError::Parse(err) => format!("architecture-rules.json: {err}"),
            })?
            .into_iter()
            .filter(TdddLayerBinding::catalogue_spec_signal_enabled)
            .collect();
        let writer = FsCatalogueSpecSignalsStore::new(items_dir.clone());

        let reader = BindingSignalLayerReader {
            inner: LocalSignalLayerReaderAdapter::new(workspace_root.clone()),
            bindings: bindings.clone(),
        };

        let per_layer_fn = {
            let items_dir = items_dir.clone();
            move |layer: domain::tddd::LayerId, _hash_hex: &str, track_id_str: &str| {
                let layer_str = layer.as_ref();
                let binding = match bindings.iter().find(|b| b.layer_id() == layer_str) {
                    Some(b) => b,
                    None => {
                        return infrastructure::verify::VerifyOutcome::from_findings(vec![
                            infrastructure::verify::VerifyFinding::error(format!(
                                "signal calc-catalog-spec: layer '{layer_str}' not found in bindings"
                            )),
                        ]);
                    }
                };
                let track_dir = items_dir.join(track_id_str);
                match infrastructure::tddd::catalogue_spec_signals_refresher::refresh_one_layer(
                    &items_dir,
                    &track_dir,
                    track_id_str,
                    binding,
                    &writer,
                ) {
                    Ok(()) => infrastructure::verify::VerifyOutcome::pass(),
                    Err(e) => infrastructure::verify::VerifyOutcome::from_findings(vec![
                        infrastructure::verify::VerifyFinding::error(format!(
                            "signal calc-catalog-spec: layer '{layer_str}': {e}"
                        )),
                    ]),
                }
            }
        };

        let outcome = usecase::signal::calc_catalog_spec(&reader, per_layer_fn);
        Ok(render_outcome("signal calc-catalog-spec", &outcome))
    }

    /// Evaluate chain ② (catalog→spec) gate for all TDDD-enabled layers.
    pub fn signal_check_catalog_spec(
        &self,
        strict_override: bool,
        gate: Option<SignalGateName>,
        workspace_root: Option<PathBuf>,
    ) -> Result<CommandOutcome, String> {
        signal_check_layer_chain(
            strict_override,
            gate,
            workspace_root,
            domain::ChainId::CatalogSpec,
            "signal check-catalog-spec",
            infrastructure::verify::tddd_layers::catalogue_spec_signals_path,
            TdddLayerBinding::catalogue_spec_signal_enabled,
            // Chain ② may have layers where `catalogue_spec_signal_enabled` is false;
            // an empty filtered set is not necessarily a misconfiguration for chain ②.
            false,
            |signals_path, hash_hex, strict| {
                infrastructure::verify::catalogue_spec_signals::check_catalog_spec_from_signals_file(
                    signals_path,
                    hash_hex,
                    strict,
                )
            },
            |reader, per_layer_fn| usecase::signal::check_catalog_spec(reader, per_layer_fn),
        )
    }

    /// Compute and persist chain ③ (impl↔catalog) signals for all TDDD-enabled layers.
    pub fn signal_calc_impl_catalog(&self) -> Result<CommandOutcome, String> {
        use infrastructure::git_cli::{GitRepository as _, SystemGitRepo};
        use infrastructure::signal_layer_reader::LocalSignalLayerReaderAdapter;
        use infrastructure::tddd::tddd_layer_bindings_adapter::FsTdddLayerBindingsAdapter;
        use infrastructure::tddd::type_signals_executor_adapter::TypeSignalsExecutorAdapter;
        use usecase::type_signals::{
            TypeSignalsInteractor, TypeSignalsRequest, TypeSignalsService,
        };

        let repo = SystemGitRepo::discover()
            .map_err(|e| format!("signal calc-impl-catalog: cannot discover git repo: {e}"))?;
        let workspace_root = repo.root().to_path_buf();
        let branch = repo
            .current_branch()
            .map_err(|e| format!("signal calc-impl-catalog: cannot read current branch: {e}"))?
            .ok_or_else(|| "signal calc-impl-catalog: cannot read current branch".to_owned())?;

        let items_dir = workspace_root.join("track").join("items");
        let layer_bindings = std::sync::Arc::new(FsTdddLayerBindingsAdapter::new());
        let executor = std::sync::Arc::new(TypeSignalsExecutorAdapter::new());
        let interactor = TypeSignalsInteractor::new(layer_bindings, executor);

        let reader = LocalSignalLayerReaderAdapter::discover()
            .map_err(|e| format!("signal calc-impl-catalog: {e}"))?;

        let per_layer_fn = {
            let items_dir = items_dir.clone();
            let workspace_root = workspace_root.clone();
            let branch = branch.clone();
            move |layer: domain::tddd::LayerId, _hash_hex: &str, track_id_str: &str| {
                let layer_str = layer.as_ref().to_owned();
                let track_id = track_id_str.to_owned();
                match interactor.run(TypeSignalsRequest {
                    items_dir: items_dir.clone(),
                    track_id,
                    branch: branch.clone(),
                    workspace_root: workspace_root.clone(),
                    layer: Some(layer_str.clone()),
                }) {
                    Ok(()) => infrastructure::verify::VerifyOutcome::pass(),
                    Err(e) => infrastructure::verify::VerifyOutcome::from_findings(vec![
                        infrastructure::verify::VerifyFinding::error(format!(
                            "signal calc-impl-catalog: layer '{layer_str}': {e}"
                        )),
                    ]),
                }
            }
        };

        let outcome = usecase::signal::calc_impl_catalog(&reader, per_layer_fn);
        Ok(render_outcome("signal calc-impl-catalog", &outcome))
    }

    /// Evaluate chain ③ (impl↔catalog) gate for all TDDD-enabled layers.
    pub fn signal_check_impl_catalog(
        &self,
        strict_override: bool,
        gate: Option<SignalGateName>,
        workspace_root: Option<PathBuf>,
    ) -> Result<CommandOutcome, String> {
        signal_check_layer_chain(
            strict_override,
            gate,
            workspace_root,
            domain::ChainId::ImplCatalog,
            "signal check-impl-catalog",
            infrastructure::verify::tddd_layers::impl_catalog_signals_path,
            |_| true,
            // Chain ③ must fail-closed when the TDDD-enabled layer set is empty.
            // A repo where every layer has `tddd.enabled = false` (or no tddd blocks)
            // would otherwise vacuously pass, bypassing the impl-catalog signal gate.
            true,
            |signals_path, hash_hex, strict| {
                infrastructure::verify::spec_states::check_impl_catalog_from_signals_file(
                    signals_path,
                    hash_hex,
                    strict,
                )
            },
            |reader, per_layer_fn| usecase::signal::check_impl_catalog(reader, per_layer_fn),
        )
    }

    /// Aggregate gate check: runs all four chains in declared order.
    ///
    /// When `spec_json_path` is `None`, resolves it from the active track
    /// (current git branch `track/<id>` → `track/items/<id>/spec.json` under
    /// `workspace_root`). Pass `Some(path)` to override.
    pub fn signal_check_gate(
        &self,
        project_root: Option<PathBuf>,
        spec_json_path: Option<PathBuf>,
        gate: SignalGateName,
        workspace_root: Option<PathBuf>,
    ) -> Result<CommandOutcome, String> {
        use std::sync::Arc;

        use infrastructure::signal_layer_reader::LocalSignalLayerReaderAdapter;
        use usecase::signal::SignalLayerReader as _;
        use usecase::signal_gate::{
            AdrChainRunnerPort, LayerChainRunnerPort, SignalChainOutput, SignalGateCommand,
            SignalGateInteractor, SignalGateService, SpecAdrChainRunnerPort,
        };

        let matrix = match load_gate_matrix(workspace_root.as_deref()) {
            Ok(m) => m,
            Err(outcome) => return Ok(outcome),
        };

        // Resolve the workspace / git-repo root once. Used as the default for
        // `project_root` (chain ⓪ ADR scan) AND for spec.json track-id
        // resolution — both must agree on the same root so a single git
        // discovery (or explicit `--workspace-root`) drives every chain.
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

        // Chain ⓪ project root defaults to the resolved workspace root so
        // `sotp signal check` from a subdirectory doesn't scan a stray
        // `<subdir>/knowledge/adr/` tree.
        let project_root = project_root.unwrap_or_else(|| resolved_root.clone());

        // Resolve spec.json path and track_id from the active track.
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

        // ── Adapter: chain ⓪ (ADR → user) ────────────────────────────────────
        struct AdrChainAdapter {
            project_root: PathBuf,
        }

        impl AdrChainRunnerPort for AdrChainAdapter {
            fn run_adr_chain(
                &self,
                _project_root: PathBuf,
                strict: bool,
            ) -> Result<SignalChainOutput, String> {
                // project_root is captured at construction; ignore the argument
                // (the interactor passes workspace_root which equals project_root here).
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

        // ── Adapter: chain ① (spec → ADR) ────────────────────────────────────
        struct SpecAdrChainAdapter {
            spec_json_path: PathBuf,
        }

        impl SpecAdrChainRunnerPort for SpecAdrChainAdapter {
            fn run_spec_adr_chain(
                &self,
                _spec_json_path: PathBuf,
                strict: bool,
            ) -> Result<SignalChainOutput, String> {
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

        // ── Adapter: chains ②③ (catalog-spec / impl-catalog) ─────────────────
        struct LayerChainAdapter {
            workspace_root: Option<PathBuf>,
        }

        impl LayerChainRunnerPort for LayerChainAdapter {
            fn run_catalog_spec_chain(
                &self,
                strict: bool,
                _signal_reader: &dyn usecase::signal::SignalLayerReader,
            ) -> Result<SignalChainOutput, String> {
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
                )?;
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
            ) -> Result<SignalChainOutput, String> {
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
                )?;
                Ok(SignalChainOutput {
                    chain_label: "signal check-impl-catalog".to_owned(),
                    passed: cmd_outcome.exit_code == 0,
                    stdout: cmd_outcome.stdout,
                    stderr: cmd_outcome.stderr,
                })
            }
        }

        // ── Wire up and run ───────────────────────────────────────────────────
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

        // Reconstruct the merged CommandOutcome from SignalGateOutput.
        // Each SignalChainOutput.stdout already contains the per-chain banner
        // produced by render_outcome() inside the adapters above.
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    /// Minimal `architecture-rules.json` with ALL TDDD layers disabled, so that
    /// the filtered binding list is empty when `include_binding: |_| true` is applied.
    const ARCH_RULES_ALL_TDDD_DISABLED: &str = r#"{
  "version": 2,
  "module_limits": { "max_lines": 700, "warn_lines": 400, "exclude": [] },
  "canonical_modules": [],
  "extra_dirs": [],
  "layers": [
    {
      "crate": "domain",
      "path": "libs/domain",
      "may_depend_on": [],
      "deny_reason": "",
      "tddd": { "enabled": false }
    }
  ]
}"#;

    /// Minimal `signal-gates.json` (all strict so tests never vacuously pass).
    const SIGNAL_GATES_ALL_STRICT: &str = r#"{
  "$schema_version": 1,
  "commit_gate": {
    "adr_user": "strict", "spec_adr": "strict",
    "catalog_spec": "strict", "impl_catalog": "strict"
  },
  "merge_gate": {
    "adr_user": "strict", "spec_adr": "strict",
    "catalog_spec": "strict", "impl_catalog": "strict"
  }
}"#;

    /// Set up a minimal workspace directory containing `architecture-rules.json`,
    /// `.harness/config/signal-gates.json`, and the `track/items/<track_id>/` tree.
    ///
    /// Initialises a git repo so `SystemGitRepo::discover()` succeeds, and sets
    /// the current branch to `track/<track_id>` so `active_track_id()` resolves.
    fn setup_workspace(track_id: &str, arch_rules: &str, signal_gates: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        // Git init
        std::process::Command::new("git")
            .args(["init", "--quiet", &format!("--initial-branch=track/{track_id}")])
            .current_dir(root)
            .status()
            .expect("git init failed");
        std::process::Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(root)
            .status()
            .ok();
        std::process::Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(root)
            .status()
            .ok();

        // Write architecture-rules.json and signal-gates.json
        std::fs::write(root.join("architecture-rules.json"), arch_rules).unwrap();
        std::fs::create_dir_all(root.join(".harness/config")).unwrap();
        std::fs::write(root.join(".harness/config/signal-gates.json"), signal_gates).unwrap();

        // Create track/items/<track_id>/ directory
        std::fs::create_dir_all(root.join("track/items").join(track_id)).unwrap();

        // Initial commit so HEAD exists and git discover works
        std::process::Command::new("git").args(["add", "."]).current_dir(root).status().ok();
        std::process::Command::new("git")
            .env("GIT_AUTHOR_NAME", "test")
            .env("GIT_AUTHOR_EMAIL", "test@test.com")
            .env("GIT_COMMITTER_NAME", "test")
            .env("GIT_COMMITTER_EMAIL", "test@test.com")
            .args(["commit", "--quiet", "-m", "initial"])
            .current_dir(root)
            .status()
            .ok();

        dir
    }

    /// When all layers have `tddd.enabled: false`, `signal_check_impl_catalog`
    /// (chain ③) must fail-closed with a `[BLOCKED]` message.
    #[test]
    fn test_signal_check_impl_catalog_empty_bindings_fail_closed() {
        let track_id = "T999";
        let dir = setup_workspace(track_id, ARCH_RULES_ALL_TDDD_DISABLED, SIGNAL_GATES_ALL_STRICT);

        let app = CliApp;
        let result = app.signal_check_impl_catalog(
            false,
            Some(SignalGateName::Commit),
            Some(dir.path().to_path_buf()),
        );

        let outcome = result.expect("signal_check_impl_catalog should not return Err");
        assert_ne!(
            outcome.exit_code, 0,
            "empty TDDD layer set must produce a non-zero exit: {outcome:?}"
        );
        let output = outcome.stdout.as_deref().unwrap_or("").to_owned()
            + outcome.stderr.as_deref().unwrap_or("");
        assert!(
            output.contains("BLOCKED") || output.contains("no TDDD-enabled layers"),
            "output must mention BLOCKED or no TDDD-enabled layers: {output}"
        );
    }

    /// chain ② (`signal_check_catalog_spec`) with all layers disabled passes
    /// without error — it does not enforce the empty-set contract.
    #[test]
    fn test_signal_check_catalog_spec_empty_bindings_passes() {
        let track_id = "T999";
        let dir = setup_workspace(track_id, ARCH_RULES_ALL_TDDD_DISABLED, SIGNAL_GATES_ALL_STRICT);

        let app = CliApp;
        let result = app.signal_check_catalog_spec(
            false,
            Some(SignalGateName::Commit),
            Some(dir.path().to_path_buf()),
        );

        // chain ② does not fail-closed on an empty set; it should succeed (exit 0)
        let outcome = result.expect("signal_check_catalog_spec should not return Err");
        assert_eq!(
            outcome.exit_code, 0,
            "chain ② with empty enabled-layer set should pass vacuously: {outcome:?}"
        );
    }
}
