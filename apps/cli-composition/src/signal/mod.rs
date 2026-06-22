//! `signal` command family — per-context composition root and CliApp shim.

mod gate_check;
mod shim;
#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};

use crate::signal_layer_chain::{BindingSignalLayerReader, signal_check_layer_chain};
use crate::{CommandOutcome, cmd_outcome::render_outcome, error::CompositionError};
use infrastructure::verify::tddd_layers::TdddLayerBinding;

// ---------------------------------------------------------------------------
// Per-context composition root
// ---------------------------------------------------------------------------

/// Composition root for the `signal` command family.
///
/// Unit struct: no adapter dependencies are injected at construction time.
pub struct SignalCompositionRoot;

impl SignalCompositionRoot {
    /// Create a new `SignalCompositionRoot`.
    pub fn new() -> Self {
        Self
    }
}

impl Default for SignalCompositionRoot {
    fn default() -> Self {
        Self::new()
    }
}

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

pub(super) fn merge_outcomes(label: &str, outcomes: Vec<CommandOutcome>) -> CommandOutcome {
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

pub(super) fn load_gate_matrix(
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
pub(super) fn resolve_spec_json_path(
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

impl SignalCompositionRoot {
    /// Compute ADR signal grounding live from `project_root/knowledge/adr/`.
    pub fn signal_calc_adr_user(
        &self,
        project_root: PathBuf,
    ) -> Result<CommandOutcome, CompositionError> {
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
    ) -> Result<CommandOutcome, CompositionError> {
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
    ) -> Result<CommandOutcome, CompositionError> {
        use std::sync::Arc;

        use infrastructure::spec::FsSpecFileWriterAdapter;
        use usecase::spec_adr_signal::{
            SpecAdrSignalCommand, SpecAdrSignalInteractor, SpecAdrSignalService,
        };

        let spec_json_path = match resolve_spec_json_path(workspace_root.as_deref(), spec_json_path)
        {
            Ok(p) => p,
            Err(outcome) => return Ok(outcome),
        };

        let adapter = FsSpecFileWriterAdapter::new();
        let interactor = SpecAdrSignalInteractor::new(Arc::new(adapter));
        interactor
            .calc_and_persist(SpecAdrSignalCommand { spec_json_path })
            .map_err(|e| CompositionError::Usecase(e.to_string()))?;
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
    ) -> Result<CommandOutcome, CompositionError> {
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
    pub fn signal_calc_catalog_spec(&self) -> Result<CommandOutcome, CompositionError> {
        use infrastructure::git_cli::{GitRepository as _, SystemGitRepo};
        use infrastructure::signal_layer_reader::LocalSignalLayerReaderAdapter;
        use infrastructure::tddd::fs_catalogue_spec_signals_store::FsCatalogueSpecSignalsStore;
        use infrastructure::verify::tddd_layers::{
            LoadTdddLayersError, load_tddd_layers_from_workspace,
        };

        let repo = SystemGitRepo::discover().map_err(|e| {
            CompositionError::AdapterInit(format!(
                "signal calc-catalog-spec: cannot discover git repo: {e}"
            ))
        })?;
        let workspace_root = repo.root().to_path_buf();
        let items_dir = workspace_root.join("track").join("items");

        let bindings: Vec<_> = load_tddd_layers_from_workspace(&workspace_root)
            .map_err(|e| {
                CompositionError::ConfigLoad(match e {
                    LoadTdddLayersError::Io { path, source } => {
                        format!("{}: {source}", path.display())
                    }
                    LoadTdddLayersError::Parse(err) => format!("architecture-rules.json: {err}"),
                })
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
    ) -> Result<CommandOutcome, CompositionError> {
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
    pub fn signal_calc_impl_catalog(&self) -> Result<CommandOutcome, CompositionError> {
        use infrastructure::git_cli::{GitRepository as _, SystemGitRepo};
        use infrastructure::signal_layer_reader::LocalSignalLayerReaderAdapter;
        use infrastructure::tddd::tddd_layer_bindings_adapter::FsTdddLayerBindingsAdapter;
        use infrastructure::tddd::type_signals_executor_adapter::TypeSignalsExecutorAdapter;
        use usecase::type_signals::{
            TypeSignalsInteractor, TypeSignalsRequest, TypeSignalsService,
        };

        let repo = SystemGitRepo::discover().map_err(|e| {
            CompositionError::AdapterInit(format!(
                "signal calc-impl-catalog: cannot discover git repo: {e}"
            ))
        })?;
        let workspace_root = repo.root().to_path_buf();
        let branch = repo
            .current_branch()
            .map_err(|e| {
                CompositionError::AdapterInit(format!(
                    "signal calc-impl-catalog: cannot read current branch: {e}"
                ))
            })?
            .ok_or_else(|| {
                CompositionError::AdapterInit(
                    "signal calc-impl-catalog: cannot read current branch".to_owned(),
                )
            })?;

        let items_dir = workspace_root.join("track").join("items");
        let layer_bindings = std::sync::Arc::new(FsTdddLayerBindingsAdapter::new());
        let executor = std::sync::Arc::new(TypeSignalsExecutorAdapter::new());
        let interactor = TypeSignalsInteractor::new(layer_bindings, executor);

        let reader = LocalSignalLayerReaderAdapter::discover()
            .map_err(|e| CompositionError::AdapterInit(format!("signal calc-impl-catalog: {e}")))?;

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
    ) -> Result<CommandOutcome, CompositionError> {
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
}
