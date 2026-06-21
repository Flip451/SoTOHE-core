//! Shared per-layer iteration helpers extracted from [`crate::signal`].
//!
//! Hosts the `BindingSignalLayerReader` adapter plus the
//! `signal_check_layer_chain` / `signal_check_layer_chain_with_strict`
//! free functions that the chain ②/③ `check_*` methods on `CliApp` delegate to.
//! Kept in a sibling module to keep `signal.rs` under the 700-line cap.

use std::path::{Path, PathBuf};

use crate::{CommandOutcome, cmd_outcome::render_outcome, signal::SignalGateName};
use infrastructure::verify::tddd_layers::TdddLayerBinding;
use usecase::signal::SignalLayerReaderError;

pub(crate) struct BindingSignalLayerReader {
    pub(crate) inner: infrastructure::signal_layer_reader::LocalSignalLayerReaderAdapter,
    pub(crate) bindings: Vec<TdddLayerBinding>,
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
///
/// `signals_path_fn` is the per-chain path resolver supplied by callers
/// (`TdddLayerBinding::catalogue_spec_signals_path` for chain ②,
/// `TdddLayerBinding::impl_catalog_signals_path` for chain ③). All
/// `<items_dir>/<track_id>/<file>` path assembly happens inside infrastructure
/// (`TdddLayerBinding`), keeping cli-composition wire-up-only per ADR
/// `knowledge/adr/2026-06-16-1030-signal-gate-strictness-config.md` §D8-2.
///
/// The `run_usecase` callback receives a `Box<dyn Fn(LayerId, hash_hex, track_id_str)>`
/// closure whose `track_id_str` argument is supplied by the usecase orchestrator
/// (CN-17 / D8) — cli-composition resolves the track ID from the reader via
/// the orchestrator, not locally.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn signal_check_layer_chain(
    strict_override: bool,
    gate: Option<SignalGateName>,
    workspace_root: Option<PathBuf>,
    chain_id: domain::ChainId,
    command_label: &str,
    signals_path_fn: impl Fn(&TdddLayerBinding, &Path, &str) -> PathBuf + 'static,
    include_binding: impl Fn(&TdddLayerBinding) -> bool + 'static,
    fail_on_empty_bindings: bool,
    verifier: impl Fn(&std::path::Path, &str, bool) -> infrastructure::verify::VerifyOutcome + 'static,
    run_usecase: impl Fn(
        &BindingSignalLayerReader,
        Box<dyn Fn(domain::tddd::LayerId, &str, &str) -> infrastructure::verify::VerifyOutcome>,
    ) -> infrastructure::verify::VerifyOutcome,
) -> Result<CommandOutcome, String> {
    let strict = match crate::signal::resolve_strict(
        strict_override,
        gate,
        chain_id,
        workspace_root.as_deref(),
    ) {
        Ok(s) => s,
        Err(outcome) => return Ok(outcome),
    };

    signal_check_layer_chain_with_strict(
        strict,
        workspace_root,
        command_label,
        signals_path_fn,
        include_binding,
        fail_on_empty_bindings,
        verifier,
        run_usecase,
    )
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(crate) fn signal_check_layer_chain_with_strict(
    strict: bool,
    workspace_root: Option<PathBuf>,
    command_label: &str,
    signals_path_fn: impl Fn(&TdddLayerBinding, &Path, &str) -> PathBuf + 'static,
    include_binding: impl Fn(&TdddLayerBinding) -> bool + 'static,
    fail_on_empty_bindings: bool,
    verifier: impl Fn(&std::path::Path, &str, bool) -> infrastructure::verify::VerifyOutcome + 'static,
    run_usecase: impl Fn(
        &BindingSignalLayerReader,
        Box<dyn Fn(domain::tddd::LayerId, &str, &str) -> infrastructure::verify::VerifyOutcome>,
    ) -> infrastructure::verify::VerifyOutcome,
) -> Result<CommandOutcome, String> {
    use infrastructure::git_cli::{GitRepository as _, SystemGitRepo};
    use infrastructure::signal_layer_reader::LocalSignalLayerReaderAdapter;
    use infrastructure::verify::tddd_layers::{
        LoadTdddLayersError, find_binding, load_tddd_layers_from_workspace,
    };

    let root = match workspace_root {
        Some(ref r) => r.clone(),
        None => {
            let repo = SystemGitRepo::discover()
                .map_err(|e| format!("{command_label}: cannot discover git repo: {e}"))?;
            repo.root().to_path_buf()
        }
    };
    let bindings = load_tddd_layers_from_workspace(&root).map_err(|e| match e {
        LoadTdddLayersError::Io { path, source } => format!("{}: {source}", path.display()),
        LoadTdddLayersError::Parse(err) => format!("architecture-rules.json: {err}"),
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
            return Ok(render_outcome(command_label, &outcome));
        }
        // No layers opted in (e.g. chain ② with no catalogue-spec-signal-enabled layers)
        // — nothing to check; return a clean pass without attempting to resolve the
        // active track id (which would fail outside a real track branch).
        return Ok(render_outcome(command_label, &infrastructure::verify::VerifyOutcome::pass()));
    }
    let reader = BindingSignalLayerReader {
        inner: LocalSignalLayerReaderAdapter::new(root.clone()),
        bindings: bindings.clone(),
    };

    // Active-track ID resolution is owned by the usecase orchestrator (CN-17 / D8).
    // The `per_layer_fn` closure receives `track_id_str` as a third argument supplied
    // by `run_per_layer` inside the orchestrator — cli-composition must not call
    // `reader.active_track_id()` here.
    let per_layer_fn: Box<
        dyn Fn(domain::tddd::LayerId, &str, &str) -> infrastructure::verify::VerifyOutcome,
    > = {
        let root = root.clone();
        Box::new(move |layer: domain::tddd::LayerId, hash_hex: &str, track_id_str: &str| {
            let layer_str = layer.as_ref();
            let Some(binding) = find_binding(&bindings, layer_str) else {
                return infrastructure::verify::VerifyOutcome::from_findings(vec![
                    infrastructure::verify::VerifyFinding::error(format!(
                        "TDDD layer binding for '{layer_str}' not found"
                    )),
                ]);
            };
            let signals_path = signals_path_fn(binding, &root, track_id_str);
            verifier(&signals_path, hash_hex, strict)
        })
    };

    let outcome = run_usecase(&reader, per_layer_fn);
    Ok(render_outcome(command_label, &outcome))
}
