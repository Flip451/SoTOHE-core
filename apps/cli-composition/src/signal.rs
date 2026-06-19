//! `signal` command family — CliApp impl methods.
//!
//! Each method corresponds to one `signal calc-*` or `signal check-*` CLI
//! subcommand.  The aggregate `signal check --gate commit|merge` is handled
//! by [`CliApp::signal_check_gate`], which runs all four chains in declared
//! order and aggregates their findings.
//!
//! # Gate name enum
//!
//! Callers pass [`SignalGateName`] (a pure value type defined in this module)
//! to select the gate context.  This keeps `domain` types off the `cli` →
//! `cli_composition` public boundary (CN-02).

use std::path::{Path, PathBuf};

use crate::{CliApp, CommandOutcome, cmd_outcome::render_outcome};

// ── Public gate-name DTO (crosses the cli_composition boundary) ───────────────

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

// ── Signal-local helpers (private) ───────────────────────────────────────────

/// Parse a hex-encoded catalogue hash, returning a failure `CommandOutcome` on error.
///
/// Used by both calc and check catalogue commands to avoid duplicating the
/// `ContentHash::try_from_hex` + error-format pattern.
fn parse_catalogue_hash(
    command_label: &str,
    catalogue_hash: &str,
) -> Result<domain::ContentHash, CommandOutcome> {
    domain::ContentHash::try_from_hex(catalogue_hash).map_err(|e| {
        CommandOutcome::failure(Some(format!(
            "--- {command_label} ---\n\
             [ERROR] invalid catalogue-hash: {e}\n\
             --- {command_label} FAILED ---"
        )))
    })
}

/// Merge multiple `CommandOutcome` values (all run in declared order).
///
/// Collects stdout lines from each outcome; exit code is non-zero if any
/// outcome has `exit_code != 0`.
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

/// Resolve the `signal-gates.json` path from the workspace root (discovered
/// from the CWD via git).
///
/// Returns `Err` with an actionable message when git discovery fails or when
/// the config cannot be loaded.
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

/// Resolve the effective `strict: bool` from a `--strict` override or
/// a `--gate commit|merge` + config lookup.
///
/// Returns `Err(CommandOutcome)` when the config cannot be loaded.
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

// ── Path-parsing helpers for calc commands ────────────────────────────────────

/// Parse `<track-id>` and `<layer>` from a signals path that follows the
/// convention `track/items/<track-id>/<layer><suffix>`.
///
/// Returns `None` when the path does not match the expected structure.
fn parse_signals_path_with_suffix(signals_path: &Path, suffix: &str) -> Option<(String, String)> {
    // We look for the `.../track/items/<track-id>/<layer><suffix>` convention.
    // The path may be absolute or relative; we walk the components from the end.
    let file_name = signals_path.file_name()?.to_str()?;
    let layer = file_name.strip_suffix(suffix)?;
    let track_id_component = signals_path.parent()?.file_name()?.to_str()?;
    // Sanity: grandparent should be "items".
    let maybe_items = signals_path.parent()?.parent()?.file_name()?.to_str()?;
    if maybe_items != "items" {
        return None;
    }
    Some((track_id_component.to_owned(), layer.to_owned()))
}

/// Parse `<track-id>` and `<layer>` from a signals path that follows the
/// convention `track/items/<track-id>/<layer>-type-signals.json`.
///
/// Returns `None` when the path does not match the expected structure.
fn parse_signals_path_for_impl_catalog(signals_path: &Path) -> Option<(String, String)> {
    parse_signals_path_with_suffix(signals_path, "-type-signals.json")
}

/// Parse `<track-id>` and `<layer>` from a signals path that follows the
/// convention `track/items/<track-id>/<layer>-catalogue-spec-signals.json`.
///
/// Returns `None` when the path does not match the expected structure.
fn parse_signals_path_for_catalog_spec(signals_path: &Path) -> Option<(String, String)> {
    parse_signals_path_with_suffix(signals_path, "-catalogue-spec-signals.json")
}

/// Shared orchestration for catalogue-hash-based signal check commands.
///
/// Resolves strictness, validates the catalogue hash, delegates to the
/// chain-specific check function via `check_fn`, and renders the outcome.
///
/// `check_fn` receives `(signals_path, hex_hash, strict)` and returns a
/// `infrastructure::verify::VerifyOutcome`.
#[allow(clippy::too_many_arguments)]
fn run_catalogue_hash_check(
    command_label: &str,
    signals_path: &Path,
    catalogue_hash: &str,
    strict_override: bool,
    gate: Option<SignalGateName>,
    chain_id: domain::ChainId,
    workspace_root: Option<&Path>,
    check_fn: impl FnOnce(&Path, &str, bool) -> infrastructure::verify::VerifyOutcome,
) -> Result<CommandOutcome, String> {
    let strict = match resolve_strict(strict_override, gate, chain_id, workspace_root) {
        Ok(s) => s,
        Err(outcome) => return Ok(outcome),
    };
    let hash = match parse_catalogue_hash(command_label, catalogue_hash) {
        Ok(h) => h,
        Err(outcome) => return Ok(outcome),
    };
    let outcome = check_fn(signals_path, &hash.to_hex(), strict);
    Ok(render_outcome(command_label, &outcome))
}

// ── CliApp impl ───────────────────────────────────────────────────────────────

impl CliApp {
    // ── calc-adr-user ─────────────────────────────────────────────────────────

    /// Compute ADR signal grounding live from `project_root/knowledge/adr/`.
    ///
    /// Compute ADR signal grounding live from `project_root/knowledge/adr/`.
    ///
    /// Wired at the composition root via
    /// `infrastructure::verify::adr_signals::execute_verify_adr_signals_with_strict`
    /// (same path as `signal check-adr-user` but with `strict = false` — no
    /// gate enforcement, result is displayed only).
    /// `usecase::chain::AdrUserChain::calc_live` remains a placeholder until T006.
    ///
    /// # Errors
    ///
    /// Returns `Err` when the infrastructure ADR scanner fails (I/O errors).
    pub fn signal_calc_adr_user(&self, project_root: PathBuf) -> Result<CommandOutcome, String> {
        // Wire chain ⓪ calc at the composition root.
        // `usecase::chain::AdrUserChain::calc_live` carries a T006 placeholder; we bypass
        // it and call `execute_verify_adr_signals_with_strict` directly with `strict=false`
        // (no gate enforcement — calc only displays the current signal distribution).
        let outcome = infrastructure::verify::adr_signals::execute_verify_adr_signals_with_strict(
            &project_root,
            false,
        );
        Ok(render_outcome("signal calc-adr-user", &outcome))
    }

    // ── check-adr-user ────────────────────────────────────────────────────────

    /// Evaluate chain ⓪ (ADR→user) gate.
    ///
    /// Strictness is resolved from `strict_override` (takes precedence) or
    /// from `signal-gates.json` using `gate` context.  The `--strict` and
    /// `--gate` flags are mutually exclusive (enforced by the CLI parser).
    ///
    /// # Errors
    ///
    /// Returns `Err` when gate config cannot be loaded and no override is given.
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

        // Wire AdrUserChain at the composition root using FsAdrFileAdapter.
        // `usecase::chain::AdrUserChain::calc_live` carries a placeholder until the
        // usecase-layer wiring (T006 follow-up) lands; we bypass it here and call
        // `execute_verify_adr_signals_with_strict` directly, which uses
        // `infrastructure::adr_decision::FsAdrFileAdapter` to perform the live scan.
        let outcome = infrastructure::verify::adr_signals::execute_verify_adr_signals_with_strict(
            &project_root,
            strict,
        );
        Ok(render_outcome("signal check-adr-user", &outcome))
    }

    // ── calc-spec-adr ─────────────────────────────────────────────────────────

    /// Compute and persist chain ① (spec→ADR) signals to `spec.json`.
    ///
    /// Wired at the composition root: reads `spec.json`, evaluates signals via
    /// `SpecDocument::evaluate_signals`, writes them back with `set_signals`, then
    /// atomically overwrites `spec.json`.  This is the same path as `sotp track
    /// signals` but scoped to a single `spec.json` path.
    /// `usecase::chain::SpecAdrChain::calc` remains a placeholder until T007.
    ///
    /// # Errors
    ///
    /// Returns `Err` when `spec.json` cannot be read, decoded, or written.
    pub fn signal_calc_spec_adr(&self, spec_json_path: PathBuf) -> Result<CommandOutcome, String> {
        use infrastructure::spec::codec as spec_codec;
        use infrastructure::track::atomic_write::atomic_write_file;

        // Wire chain ① calc at the composition root.
        // `usecase::chain::SpecAdrChain::calc` carries a T007 placeholder; we bypass
        // it and call the spec read/evaluate/write path directly.
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

    // ── check-spec-adr ────────────────────────────────────────────────────────

    /// Evaluate chain ① (spec→ADR) gate.
    ///
    /// # Errors
    ///
    /// Returns `Err` when gate config cannot be loaded.
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

        // Wire chain ① at the composition root.
        // `usecase::chain::SpecAdrChain::calc`/`load` carry T007 placeholders; we bypass
        // them and call `verify_from_spec_json` (Stage 1 only) directly.
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

    // ── calc-catalog-spec ─────────────────────────────────────────────────────

    /// Compute and persist chain ② (catalog→spec) signals for a single layer.
    ///
    /// Wired at the composition root via
    /// `infrastructure::tddd::catalogue_spec_signals_refresher::refresh_one_layer`
    /// (same path as `track catalogue-spec-signals`, but scoped to one layer).
    ///
    /// `signals_path` must follow the convention
    /// `track/items/<track-id>/<layer>-catalogue-spec-signals.json` so the
    /// track-id and layer can be inferred.
    ///
    /// `_catalogue_hash` is accepted but ignored — `refresh_one_layer` computes
    /// the hash itself from the catalogue file bytes.
    ///
    /// # Errors
    ///
    /// Returns `Err` on workspace-discovery, path-parse, layer-lookup, or
    /// execution failures.
    pub fn signal_calc_catalog_spec(
        &self,
        signals_path: PathBuf,
        _catalogue_hash: String,
    ) -> Result<CommandOutcome, String> {
        use infrastructure::git_cli::{GitRepository as _, SystemGitRepo};
        use infrastructure::tddd::fs_catalogue_spec_signals_store::FsCatalogueSpecSignalsStore;
        use infrastructure::verify::tddd_layers::{LoadTdddLayersError, load_tddd_layers};

        // Discover workspace root from git.
        let repo = SystemGitRepo::discover()
            .map_err(|e| format!("signal calc-catalog-spec: cannot discover git repo: {e}"))?;
        let workspace_root = repo.root().to_path_buf();

        // Parse track_id and layer from signals_path convention:
        // track/items/<track-id>/<layer>-catalogue-spec-signals.json
        let (track_id, layer_id) =
            parse_signals_path_for_catalog_spec(&signals_path).ok_or_else(|| {
                format!(
                    "signal calc-catalog-spec: cannot infer track-id/layer from signals path '{}'. \
                     Expected convention: track/items/<track-id>/<layer>-catalogue-spec-signals.json",
                    signals_path.display()
                )
            })?;

        // Resolve the TdddLayerBinding for this layer.
        let rules_path = workspace_root.join("architecture-rules.json");
        let bindings = load_tddd_layers(&rules_path, &workspace_root).map_err(|e| match e {
            LoadTdddLayersError::Io { path, source } => {
                format!("{}: {source}", path.display())
            }
            LoadTdddLayersError::Parse(err) => {
                format!("{}: {err}", rules_path.display())
            }
        })?;
        let binding = bindings.into_iter().find(|b| b.layer_id() == layer_id).ok_or_else(|| {
            format!(
                "signal calc-catalog-spec: layer '{layer_id}' is not tddd.enabled in \
                     architecture-rules.json"
            )
        })?;

        let items_dir = workspace_root.join("track").join("items");
        let track_dir = items_dir.join(&track_id);
        let writer = FsCatalogueSpecSignalsStore::new(items_dir.clone());

        infrastructure::tddd::catalogue_spec_signals_refresher::refresh_one_layer(
            &items_dir, &track_dir, &track_id, &binding, &writer,
        )?;

        Ok(CommandOutcome::success(None))
    }

    // ── check-catalog-spec ────────────────────────────────────────────────────

    /// Evaluate chain ② (catalog→spec) gate.
    ///
    /// # Errors
    ///
    /// Returns `Err` when gate config cannot be loaded.
    pub fn signal_check_catalog_spec(
        &self,
        signals_path: PathBuf,
        catalogue_hash: String,
        strict_override: bool,
        gate: Option<SignalGateName>,
        workspace_root: Option<PathBuf>,
    ) -> Result<CommandOutcome, String> {
        // Wire chain ② at the composition root.
        // `usecase::chain::CatalogSpecChain::load`/`calc` carry T008 placeholders; we bypass
        // them and call `check_catalog_spec_from_signals_file` directly.
        run_catalogue_hash_check(
            "signal check-catalog-spec",
            &signals_path,
            &catalogue_hash,
            strict_override,
            gate,
            domain::ChainId::CatalogSpec,
            workspace_root.as_deref(),
            infrastructure::verify::catalogue_spec_signals::check_catalog_spec_from_signals_file,
        )
    }

    // ── calc-impl-catalog ─────────────────────────────────────────────────────

    /// Compute and persist chain ③ (impl↔catalog) signals for a single layer.
    ///
    /// Wired at the composition root via `TypeSignalsInteractor` (same path as
    /// `track type-signals`, but scoped to one layer).
    ///
    /// `signals_path` must follow the convention
    /// `track/items/<track-id>/<layer>-type-signals.json` so the track-id and
    /// layer can be inferred without requiring the caller to pass them separately.
    ///
    /// # Errors
    ///
    /// Returns `Err` on workspace-discovery, path-parse, or execution failures.
    pub fn signal_calc_impl_catalog(
        &self,
        signals_path: PathBuf,
        _catalogue_hash: String,
    ) -> Result<CommandOutcome, String> {
        use infrastructure::git_cli::{GitRepository as _, SystemGitRepo};
        use infrastructure::tddd::tddd_layer_bindings_adapter::FsTdddLayerBindingsAdapter;
        use infrastructure::tddd::type_signals_executor_adapter::TypeSignalsExecutorAdapter;
        use usecase::type_signals::{
            TypeSignalsInteractor, TypeSignalsRequest, TypeSignalsService,
        };

        // Discover workspace root from git.
        let repo = SystemGitRepo::discover()
            .map_err(|e| format!("signal calc-impl-catalog: cannot discover git repo: {e}"))?;
        let workspace_root = repo.root().to_path_buf();
        let branch = repo
            .current_branch()
            .map_err(|e| format!("signal calc-impl-catalog: cannot read current branch: {e}"))?
            .ok_or_else(|| "signal calc-impl-catalog: cannot read current branch".to_owned())?;

        // Parse track_id and layer from signals_path convention:
        // track/items/<track-id>/<layer>-type-signals.json
        let (track_id, layer) =
            parse_signals_path_for_impl_catalog(&signals_path).ok_or_else(|| {
                format!(
                    "signal calc-impl-catalog: cannot infer track-id/layer from signals path '{}'. \
                     Expected convention: track/items/<track-id>/<layer>-type-signals.json",
                    signals_path.display()
                )
            })?;

        let items_dir = workspace_root.join("track").join("items");
        let layer_bindings = std::sync::Arc::new(FsTdddLayerBindingsAdapter::new());
        let executor = std::sync::Arc::new(TypeSignalsExecutorAdapter::new());
        let interactor = TypeSignalsInteractor::new(layer_bindings, executor);

        interactor
            .run(TypeSignalsRequest {
                items_dir,
                track_id,
                branch,
                workspace_root,
                layer: Some(layer),
            })
            .map_err(|e| format!("signal calc-impl-catalog: {e}"))?;

        Ok(CommandOutcome::success(None))
    }

    // ── check-impl-catalog ────────────────────────────────────────────────────

    /// Evaluate chain ③ (impl↔catalog) gate.
    ///
    /// # Errors
    ///
    /// Returns `Err` when gate config cannot be loaded.
    pub fn signal_check_impl_catalog(
        &self,
        signals_path: PathBuf,
        catalogue_hash: String,
        strict_override: bool,
        gate: Option<SignalGateName>,
        workspace_root: Option<PathBuf>,
    ) -> Result<CommandOutcome, String> {
        // Wire chain ③ at the composition root.
        // `usecase::chain::ImplCatalogChain::load`/`calc` carry T007 placeholders; we bypass
        // them and call `check_impl_catalog_from_signals_file` directly.
        run_catalogue_hash_check(
            "signal check-impl-catalog",
            &signals_path,
            &catalogue_hash,
            strict_override,
            gate,
            domain::ChainId::ImplCatalog,
            workspace_root.as_deref(),
            infrastructure::verify::spec_states::check_impl_catalog_from_signals_file,
        )
    }

    // ── aggregate: signal check --gate ────────────────────────────────────────

    /// Aggregate gate check: runs all four chains in declared order.
    ///
    /// Chains: ⓪ `adr-user`, ① `spec-adr`, ② `catalog-spec`, ③ `impl-catalog`.
    /// Strictness for each chain is resolved from `signal-gates.json` using
    /// the given `gate` context.
    ///
    /// Returns non-zero exit if any chain reports a blocking finding.
    ///
    /// # Parameters
    ///
    /// - `project_root`: used for chain ⓪ (`knowledge/adr/` scan).
    /// - `spec_json_path`: used for chain ① (`spec.json`).
    /// - `catalog_spec_signals_path` + `catalog_spec_hash`: used for chain ②.
    /// - `impl_catalog_signals_path` + `impl_catalog_hash`: used for chain ③.
    /// - `gate`: selects commit vs merge gate strictness from config.
    /// - `workspace_root`: when provided, overrides git discovery for config path.
    ///
    /// # Errors
    ///
    /// Returns `Err` when gate config cannot be loaded.
    #[allow(clippy::too_many_arguments)]
    pub fn signal_check_gate(
        &self,
        project_root: PathBuf,
        spec_json_path: PathBuf,
        catalog_spec_signals_path: PathBuf,
        catalog_spec_hash: String,
        impl_catalog_signals_path: PathBuf,
        impl_catalog_hash: String,
        gate: SignalGateName,
        workspace_root: Option<PathBuf>,
    ) -> Result<CommandOutcome, String> {
        // Load the gate matrix once — all four chains use the same config.
        let matrix = match load_gate_matrix(workspace_root.as_deref()) {
            Ok(m) => m,
            Err(outcome) => return Ok(outcome),
        };
        let gate_kind = gate_name_to_kind(gate);

        let gate_label = match gate {
            SignalGateName::Commit => "signal check --gate commit",
            SignalGateName::Merge => "signal check --gate merge",
        };

        // All four chains use the already-loaded matrix.  Call chain-specific infrastructure
        // functions directly — the usecase-layer chain structs carry T006/T007/T008
        // placeholders in `calc_live` / `calc` / `load`; bypassing them keeps each gate
        // functional until the usecase-layer wiring is completed.

        // Chain ⓪: adr-user — live scan via FsAdrFileAdapter (T006 follow-up wiring).
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

        // Chain ①: spec-adr — Stage 1 of verify_from_spec_json (T007 wiring).
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

        // Chain ②: catalog-spec — explicit per-layer check (T008 wiring).
        let catalog_strict =
            matrix.resolve(domain::ChainId::CatalogSpec, gate_kind) == domain::Strictness::Strict;
        let chain2 = match parse_catalogue_hash("signal check-catalog-spec", &catalog_spec_hash) {
            Err(err_outcome) => err_outcome,
            Ok(hash) => {
                let outcome =
                    infrastructure::verify::catalogue_spec_signals::check_catalog_spec_from_signals_file(
                        &catalog_spec_signals_path,
                        &hash.to_hex(),
                        catalog_strict,
                    );
                render_outcome("signal check-catalog-spec", &outcome)
            }
        };

        // Chain ③: impl-catalog — explicit per-layer check (T007 wiring).
        let impl_strict =
            matrix.resolve(domain::ChainId::ImplCatalog, gate_kind) == domain::Strictness::Strict;
        let chain3 = match parse_catalogue_hash("signal check-impl-catalog", &impl_catalog_hash) {
            Err(err_outcome) => err_outcome,
            Ok(hash) => {
                let outcome =
                    infrastructure::verify::spec_states::check_impl_catalog_from_signals_file(
                        &impl_catalog_signals_path,
                        &hash.to_hex(),
                        impl_strict,
                    );
                render_outcome("signal check-impl-catalog", &outcome)
            }
        };

        Ok(merge_outcomes(gate_label, vec![chain0, chain1, chain2, chain3]))
    }
}
