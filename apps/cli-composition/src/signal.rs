//! `signal` command family for [`CliApp`].

use std::path::{Path, PathBuf};

use crate::{CliApp, CommandOutcome, cmd_outcome::render_outcome};
use infrastructure::verify::tddd_layers::TdddLayerBinding;
use usecase::signal::{SignalLayerReader as _, SignalLayerReaderError};

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

struct BindingSignalLayerReader {
    inner: infrastructure::signal_layer_reader::LocalSignalLayerReaderAdapter,
    bindings: Vec<TdddLayerBinding>,
}

impl usecase::signal::SignalLayerReader for BindingSignalLayerReader {
    fn active_track_id(&self) -> Result<domain::TrackId, SignalLayerReaderError> {
        self.inner.active_track_id()
    }

    fn enabled_layers(
        &self,
        _track_id: domain::TrackId,
    ) -> Result<Vec<domain::tddd::LayerId>, SignalLayerReaderError> {
        self.bindings
            .iter()
            .map(|b| {
                domain::tddd::LayerId::try_new(b.layer_id().to_owned())
                    .map_err(|_| SignalLayerReaderError::Io)
            })
            .collect()
    }

    fn catalogue_bytes(
        &self,
        track_id: domain::TrackId,
        layer: domain::tddd::LayerId,
    ) -> Result<Option<Vec<u8>>, SignalLayerReaderError> {
        self.inner.catalogue_bytes(track_id, layer)
    }
}

/// Shared body for `signal_check_catalog_spec` and `signal_check_impl_catalog`.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn signal_check_layer_chain(
    strict_override: bool,
    gate: Option<SignalGateName>,
    workspace_root: Option<PathBuf>,
    chain_id: domain::ChainId,
    command_label: &str,
    signal_file_name: impl Fn(&TdddLayerBinding) -> String + 'static,
    include_binding: impl Fn(&TdddLayerBinding) -> bool + 'static,
    fail_on_empty_bindings: bool,
    verifier: impl Fn(&std::path::Path, &str, bool) -> infrastructure::verify::VerifyOutcome + 'static,
    run_usecase: impl Fn(
        &BindingSignalLayerReader,
        Box<dyn Fn(domain::tddd::LayerId, &str) -> infrastructure::verify::VerifyOutcome>,
    ) -> infrastructure::verify::VerifyOutcome,
) -> Result<CommandOutcome, String> {
    let strict = match resolve_strict(strict_override, gate, chain_id, workspace_root.as_deref()) {
        Ok(s) => s,
        Err(outcome) => return Ok(outcome),
    };

    signal_check_layer_chain_with_strict(
        strict,
        workspace_root,
        command_label,
        signal_file_name,
        include_binding,
        fail_on_empty_bindings,
        verifier,
        run_usecase,
    )
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn signal_check_layer_chain_with_strict(
    strict: bool,
    workspace_root: Option<PathBuf>,
    command_label: &str,
    signal_file_name: impl Fn(&TdddLayerBinding) -> String + 'static,
    include_binding: impl Fn(&TdddLayerBinding) -> bool + 'static,
    fail_on_empty_bindings: bool,
    verifier: impl Fn(&std::path::Path, &str, bool) -> infrastructure::verify::VerifyOutcome + 'static,
    run_usecase: impl Fn(
        &BindingSignalLayerReader,
        Box<dyn Fn(domain::tddd::LayerId, &str) -> infrastructure::verify::VerifyOutcome>,
    ) -> infrastructure::verify::VerifyOutcome,
) -> Result<CommandOutcome, String> {
    use infrastructure::git_cli::{GitRepository as _, SystemGitRepo};
    use infrastructure::signal_layer_reader::LocalSignalLayerReaderAdapter;
    use infrastructure::verify::tddd_layers::{
        LoadTdddLayersError, find_binding, load_tddd_layers,
    };

    let root = match workspace_root {
        Some(ref r) => r.clone(),
        None => {
            let repo = SystemGitRepo::discover()
                .map_err(|e| format!("{command_label}: cannot discover git repo: {e}"))?;
            repo.root().to_path_buf()
        }
    };
    let items_dir = root.join("track").join("items");
    let rules_path = root.join("architecture-rules.json");
    let bindings = load_tddd_layers(&rules_path, &root).map_err(|e| match e {
        LoadTdddLayersError::Io { path, source } => format!("{}: {source}", path.display()),
        LoadTdddLayersError::Parse(err) => format!("{}: {err}", rules_path.display()),
    })?;
    let bindings: Vec<_> = bindings.into_iter().filter(|b| include_binding(b)).collect();

    // Handle empty filtered binding list.
    if bindings.is_empty() {
        if fail_on_empty_bindings {
            // Fail-closed for chain ③ `check-impl-catalog`: silently passing an empty
            // layer set would allow CI to succeed on a repo where every layer has
            // `tddd.enabled = false`, violating the same contract enforced by
            // `verify_type_signals_from_spec_json`.
            let outcome = infrastructure::verify::VerifyOutcome::from_findings(vec![
                infrastructure::verify::VerifyFinding::error(format!(
                    "[BLOCKED] {command_label}: no TDDD-enabled layers for chain ③ check — \
                     set `tddd.enabled: true` for at least one layer in architecture-rules.json"
                )),
            ]);
            return Ok(crate::cmd_outcome::render_outcome(command_label, &outcome));
        }
        // No layers opted in (e.g. chain ② with no catalogue-spec-signal-enabled layers)
        // — nothing to check; return a clean pass without attempting to resolve the
        // active track id (which would fail outside a real track branch).
        return Ok(crate::cmd_outcome::render_outcome(
            command_label,
            &infrastructure::verify::VerifyOutcome::pass(),
        ));
    }
    let reader = BindingSignalLayerReader {
        inner: LocalSignalLayerReaderAdapter::new(root.clone()),
        bindings: bindings.clone(),
    };

    let track_id_str = {
        reader
            .active_track_id()
            .map_err(|e| format!("{command_label}: cannot resolve active track id: {e}"))?
            .to_string()
    };

    let per_layer_fn: Box<
        dyn Fn(domain::tddd::LayerId, &str) -> infrastructure::verify::VerifyOutcome,
    > = {
        let items_dir = items_dir.clone();
        let track_id_str = track_id_str.clone();
        Box::new(move |layer: domain::tddd::LayerId, hash_hex: &str| {
            let layer_str = layer.as_ref();
            let Some(binding) = find_binding(&bindings, layer_str) else {
                return infrastructure::verify::VerifyOutcome::from_findings(vec![
                    infrastructure::verify::VerifyFinding::error(format!(
                        "TDDD layer binding for '{layer_str}' not found"
                    )),
                ]);
            };
            let signals_path = items_dir.join(&track_id_str).join(signal_file_name(binding));
            verifier(&signals_path, hash_hex, strict)
        })
    };

    let outcome = run_usecase(&reader, per_layer_fn);
    Ok(render_outcome(command_label, &outcome))
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

fn resolve_strict(
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
    pub fn signal_calc_spec_adr(&self, spec_json_path: PathBuf) -> Result<CommandOutcome, String> {
        use infrastructure::spec::codec as spec_codec;
        use infrastructure::track::atomic_write::atomic_write_file;

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
    pub fn signal_check_spec_adr(
        &self,
        spec_json_path: PathBuf,
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
        use infrastructure::verify::tddd_layers::{LoadTdddLayersError, load_tddd_layers};

        let repo = SystemGitRepo::discover()
            .map_err(|e| format!("signal calc-catalog-spec: cannot discover git repo: {e}"))?;
        let workspace_root = repo.root().to_path_buf();
        let items_dir = workspace_root.join("track").join("items");
        let rules_path = workspace_root.join("architecture-rules.json");

        let bindings: Vec<_> = load_tddd_layers(&rules_path, &workspace_root)
            .map_err(|e| match e {
                LoadTdddLayersError::Io { path, source } => format!("{}: {source}", path.display()),
                LoadTdddLayersError::Parse(err) => format!("{}: {err}", rules_path.display()),
            })?
            .into_iter()
            .filter(TdddLayerBinding::catalogue_spec_signal_enabled)
            .collect();
        let writer = FsCatalogueSpecSignalsStore::new(items_dir.clone());

        let reader = BindingSignalLayerReader {
            inner: LocalSignalLayerReaderAdapter::new(workspace_root.clone()),
            bindings: bindings.clone(),
        };

        let track_id_str = {
            reader
                .active_track_id()
                .map_err(|e| {
                    format!("signal calc-catalog-spec: cannot resolve active track id: {e}")
                })?
                .to_string()
        };

        let per_layer_fn = {
            let items_dir = items_dir.clone();
            let track_id_str = track_id_str.clone();
            move |layer: domain::tddd::LayerId, _hash_hex: &str| {
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
                let track_dir = items_dir.join(&track_id_str);
                match infrastructure::tddd::catalogue_spec_signals_refresher::refresh_one_layer(
                    &items_dir,
                    &track_dir,
                    &track_id_str,
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
            TdddLayerBinding::catalogue_spec_signal_file,
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

        let track_id_str = {
            use usecase::signal::SignalLayerReader as _;
            reader
                .active_track_id()
                .map_err(|e| {
                    format!("signal calc-impl-catalog: cannot resolve active track id: {e}")
                })?
                .to_string()
        };

        let per_layer_fn = {
            let items_dir = items_dir.clone();
            let workspace_root = workspace_root.clone();
            let branch = branch.clone();
            let track_id_str = track_id_str.clone();
            move |layer: domain::tddd::LayerId, _hash_hex: &str| {
                let layer_str = layer.as_ref().to_owned();
                match interactor.run(TypeSignalsRequest {
                    items_dir: items_dir.clone(),
                    track_id: track_id_str.clone(),
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
            TdddLayerBinding::signal_file,
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
        let matrix = match load_gate_matrix(workspace_root.as_deref()) {
            Ok(m) => m,
            Err(outcome) => return Ok(outcome),
        };
        let gate_kind = gate_name_to_kind(gate);

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

        // Resolve `spec_json_path` from the active track when not supplied.
        let spec_json_path = match spec_json_path {
            Some(p) => p,
            None => {
                use infrastructure::signal_layer_reader::LocalSignalLayerReaderAdapter;
                use usecase::signal::SignalLayerReader as _;
                let reader = LocalSignalLayerReaderAdapter::new(resolved_root.clone());
                match reader.active_track_id() {
                    Ok(track_id) => {
                        resolved_root.join("track/items").join(track_id.as_ref()).join("spec.json")
                    }
                    Err(e) => {
                        return Ok(CommandOutcome::failure(Some(format!(
                            "[BLOCKED] signal check --gate {gate:?}: cannot resolve spec.json from \
                             active track: {e}; pass --spec-json explicitly"
                        ))));
                    }
                }
            }
        };

        let gate_label = match gate {
            SignalGateName::Commit => "signal check --gate commit",
            SignalGateName::Merge => "signal check --gate merge",
        };

        // Chain ⓪: adr-user — live scan via FsAdrFileAdapter.
        let adr_strict =
            matrix.resolve(domain::ChainId::AdrUser, gate_kind) == domain::Strictness::Strict;
        let chain0 = {
            let outcome =
                infrastructure::verify::adr_signals::execute_verify_adr_signals_with_strict(
                    &project_root,
                    adr_strict,
                );
            render_outcome("signal check-adr-user", &outcome)
        };

        // Chain ①: spec-adr — Stage 1 of verify_from_spec_json.
        let spec_strict =
            matrix.resolve(domain::ChainId::SpecAdr, gate_kind) == domain::Strictness::Strict;
        let chain1 = {
            let outcome =
                match infrastructure::verify::trusted_root::resolve_trusted_root(&spec_json_path) {
                    Ok(trusted_root) => infrastructure::verify::spec_states::verify_from_spec_json(
                        spec_json_path.clone(),
                        spec_strict,
                        trusted_root,
                    ),
                    Err(e) => infrastructure::verify::VerifyOutcome::from_findings(vec![
                        infrastructure::verify::VerifyFinding::error(format!(
                            "cannot resolve trusted_root for {}: {e}",
                            spec_json_path.display()
                        )),
                    ]),
                };
            render_outcome("signal check-spec-adr", &outcome)
        };

        // Chains ②③: use the argless orchestrators with per-layer closures.
        let catalog_strict =
            matrix.resolve(domain::ChainId::CatalogSpec, gate_kind) == domain::Strictness::Strict;
        let impl_strict =
            matrix.resolve(domain::ChainId::ImplCatalog, gate_kind) == domain::Strictness::Strict;

        let chain2 = match signal_check_layer_chain_with_strict(
            catalog_strict,
            workspace_root.clone(),
            "signal check-catalog-spec",
            TdddLayerBinding::catalogue_spec_signal_file,
            TdddLayerBinding::catalogue_spec_signal_enabled,
            false,
            |signals_path, hash_hex, strict| {
                infrastructure::verify::catalogue_spec_signals::check_catalog_spec_from_signals_file(
                    signals_path,
                    hash_hex,
                    strict,
                )
            },
            |reader, per_layer_fn| usecase::signal::check_catalog_spec(reader, per_layer_fn),
        ) {
            Ok(outcome) => outcome,
            Err(e) => CommandOutcome::failure(Some(e)),
        };

        let chain3 = match signal_check_layer_chain_with_strict(
            impl_strict,
            workspace_root,
            "signal check-impl-catalog",
            TdddLayerBinding::signal_file,
            |_| true,
            true,
            |signals_path, hash_hex, strict| {
                infrastructure::verify::spec_states::check_impl_catalog_from_signals_file(
                    signals_path,
                    hash_hex,
                    strict,
                )
            },
            |reader, per_layer_fn| usecase::signal::check_impl_catalog(reader, per_layer_fn),
        ) {
            Ok(outcome) => outcome,
            Err(e) => CommandOutcome::failure(Some(e)),
        };

        Ok(merge_outcomes(gate_label, vec![chain0, chain1, chain2, chain3]))
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
