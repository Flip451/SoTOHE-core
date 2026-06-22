// STAGED FOR T021 — not yet compiled; Cargo.toml + workspace member added atomically in T021 per CN-06.
//
//! `signal` command family — primary adapter driver.
//!
//! `SignalDriver` holds injected use-case interactors and exposes
//! `handle(input) -> CommandOutcome`.  The render helpers here mirror
//! `apps/cli-composition/src/signal/mod.rs` (lines ~49 `merge_outcomes`) and
//! `apps/cli-composition/src/signal_layer_chain.rs`;
//! T021 removes the `cli_composition` duplicates when the live path is flipped.

// TODO(T021): add use-case + infrastructure imports once Cargo.toml is materialized.
// use std::path::{Path, PathBuf};
// use std::sync::Arc;
// use domain::{ChainId, GateKind, SignalGateMatrix, Strictness};
// use infrastructure::git_cli::{GitRepository as _, SystemGitRepo};
// use infrastructure::signal_layer_reader::LocalSignalLayerReaderAdapter;
// use infrastructure::verify::tddd_layers::{
//     LoadTdddLayersError, TdddLayerBinding, find_binding, load_tddd_layers_from_workspace,
// };
// use usecase::signal::{
//     SignalLayerReader, SignalLayerReaderError,
//     calc_catalog_spec, calc_impl_catalog,
//     check_catalog_spec, check_impl_catalog,
//     resolve_spec_json_path as usecase_resolve_spec_json_path,
// };

use std::path::PathBuf;

use crate::render::CommandOutcome;

// ---------------------------------------------------------------------------
// Input types
// ---------------------------------------------------------------------------

/// Selects the gate context when resolving strictness from `signal-gates.json`.
///
/// Maps one-to-one to `domain::GateKind`; defined here so that `apps/cli` can
/// pass the value without importing `domain`.
/// Mirrors `cli_composition::signal::SignalGateName`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalGateName {
    /// CI commit gate — evaluates using the `commit_gate.*` cells.
    Commit,
    /// PR merge gate — evaluates using the `merge_gate.*` cells.
    Merge,
}

/// Typed input for the `signal` command family.
pub enum SignalInput {
    /// Compute ADR signal grounding live from `project_root/knowledge/adr/`.
    CalcAdrUser {
        /// Project root directory.
        project_root: PathBuf,
    },
    /// Evaluate chain ⓪ (ADR→user) gate.
    CheckAdrUser {
        /// Project root directory.
        project_root: PathBuf,
        /// Force strict mode regardless of gate matrix.
        strict_override: bool,
        /// Gate context for strictness resolution.
        gate: Option<SignalGateName>,
        /// Workspace root for git discovery and gate matrix loading.
        workspace_root: Option<PathBuf>,
    },
    /// Compute and persist chain ① (spec→ADR) signals to `spec.json`.
    CalcSpecAdr {
        /// Path to spec.json (resolved from active track if `None`).
        spec_json_path: Option<PathBuf>,
        /// Workspace root for git discovery.
        workspace_root: Option<PathBuf>,
    },
    /// Evaluate chain ① (spec→ADR) gate.
    CheckSpecAdr {
        /// Path to spec.json (resolved from active track if `None`).
        spec_json_path: Option<PathBuf>,
        /// Force strict mode regardless of gate matrix.
        strict_override: bool,
        /// Gate context for strictness resolution.
        gate: Option<SignalGateName>,
        /// Workspace root for git discovery and gate matrix loading.
        workspace_root: Option<PathBuf>,
    },
    /// Compute and persist chain ② (catalog→spec) signals for all TDDD-enabled layers.
    CalcCatalogSpec,
    /// Evaluate chain ② (catalog→spec) gate for all TDDD-enabled layers.
    CheckCatalogSpec {
        /// Force strict mode regardless of gate matrix.
        strict_override: bool,
        /// Gate context for strictness resolution.
        gate: Option<SignalGateName>,
        /// Workspace root for git discovery and gate matrix loading.
        workspace_root: Option<PathBuf>,
    },
    /// Compute and persist chain ③ (impl↔catalog) signals for all TDDD-enabled layers.
    CalcImplCatalog,
    /// Evaluate chain ③ (impl↔catalog) gate for all TDDD-enabled layers.
    CheckImplCatalog {
        /// Force strict mode regardless of gate matrix.
        strict_override: bool,
        /// Gate context for strictness resolution.
        gate: Option<SignalGateName>,
        /// Workspace root for git discovery and gate matrix loading.
        workspace_root: Option<PathBuf>,
    },
    /// Evaluate the commit-gate or merge-gate (chains ⓪①②③) and return a merged outcome.
    CheckGate {
        /// Project root directory (resolved from workspace root if `None`).
        project_root: Option<PathBuf>,
        /// Path to spec.json (resolved from active track if `None`).
        spec_json_path: Option<PathBuf>,
        /// Gate to evaluate.
        gate: SignalGateName,
        /// Workspace root for git discovery and gate matrix loading.
        workspace_root: Option<PathBuf>,
    },
}

// ---------------------------------------------------------------------------
// Driver
// ---------------------------------------------------------------------------

/// Primary adapter driver for the `signal` command family.
///
/// Holds injected use-case interactors; exposes `handle(input) -> CommandOutcome`.
pub struct SignalDriver {
    // TODO(T021): inject use-case interactors here (currently this family has
    // no injectable adapter dependencies — infrastructure functions are called
    // inline, same as cli_composition::SignalCompositionRoot).
}

impl SignalDriver {
    /// Create a new `SignalDriver`.
    ///
    /// TODO(T021): accept injected interactors as parameters once the crate
    /// dependency graph is materialized.
    pub fn new() -> Self {
        Self {}
    }

    /// Handle a signal command.
    ///
    /// TODO(T021): wire real use-case invocation once Cargo.toml is materialized.
    pub fn handle(&self, input: SignalInput) -> CommandOutcome {
        match input {
            SignalInput::CalcAdrUser { project_root } => self.signal_calc_adr_user(project_root),
            SignalInput::CheckAdrUser { project_root, strict_override, gate, workspace_root } => {
                self.signal_check_adr_user(project_root, strict_override, gate, workspace_root)
            }
            SignalInput::CalcSpecAdr { spec_json_path, workspace_root } => {
                self.signal_calc_spec_adr(spec_json_path, workspace_root)
            }
            SignalInput::CheckSpecAdr { spec_json_path, strict_override, gate, workspace_root } => {
                self.signal_check_spec_adr(spec_json_path, strict_override, gate, workspace_root)
            }
            SignalInput::CalcCatalogSpec => self.signal_calc_catalog_spec(),
            SignalInput::CheckCatalogSpec { strict_override, gate, workspace_root } => {
                self.signal_check_catalog_spec(strict_override, gate, workspace_root)
            }
            SignalInput::CalcImplCatalog => self.signal_calc_impl_catalog(),
            SignalInput::CheckImplCatalog { strict_override, gate, workspace_root } => {
                self.signal_check_impl_catalog(strict_override, gate, workspace_root)
            }
            SignalInput::CheckGate { project_root, spec_json_path, gate, workspace_root } => {
                self.signal_check_gate(project_root, spec_json_path, gate, workspace_root)
            }
        }
    }

    // -----------------------------------------------------------------------
    // Render helpers (logic duplicated from cli_composition/src/signal/mod.rs
    // and signal_layer_chain.rs; T021 removes the cli_composition copies).
    // -----------------------------------------------------------------------

    fn signal_calc_adr_user(&self, _project_root: PathBuf) -> CommandOutcome {
        // TODO(T021): invoke infrastructure::verify::adr_signals::execute_verify_adr_signals_with_strict
        // and render_outcome("signal calc-adr-user", &outcome) here.
        // Mirrors cli_composition/src/signal/mod.rs SignalCompositionRoot::signal_calc_adr_user.
        CommandOutcome::failure(Some("cli_driver Driver::handle is not yet wired — apps/cli still routes through cli_composition CompositionRoot dispatch (deferred from T021); call the matching CompositionRoot method instead".to_owned()))
    }

    fn signal_check_adr_user(
        &self,
        _project_root: PathBuf,
        _strict_override: bool,
        _gate: Option<SignalGateName>,
        _workspace_root: Option<PathBuf>,
    ) -> CommandOutcome {
        // TODO(T021): resolve strictness via resolve_strict, then invoke
        // execute_verify_adr_signals_with_strict and render_outcome here.
        // Mirrors cli_composition/src/signal/mod.rs SignalCompositionRoot::signal_check_adr_user.
        CommandOutcome::failure(Some("cli_driver Driver::handle is not yet wired — apps/cli still routes through cli_composition CompositionRoot dispatch (deferred from T021); call the matching CompositionRoot method instead".to_owned()))
    }

    fn signal_calc_spec_adr(
        &self,
        _spec_json_path: Option<PathBuf>,
        _workspace_root: Option<PathBuf>,
    ) -> CommandOutcome {
        // TODO(T021): resolve spec_json_path, then invoke SpecAdrSignalInteractor::calc_and_persist
        // and render_outcome("signal calc-spec-adr", &VerifyOutcome::pass()) here.
        // Mirrors cli_composition/src/signal/mod.rs SignalCompositionRoot::signal_calc_spec_adr.
        CommandOutcome::failure(Some("cli_driver Driver::handle is not yet wired — apps/cli still routes through cli_composition CompositionRoot dispatch (deferred from T021); call the matching CompositionRoot method instead".to_owned()))
    }

    fn signal_check_spec_adr(
        &self,
        _spec_json_path: Option<PathBuf>,
        _strict_override: bool,
        _gate: Option<SignalGateName>,
        _workspace_root: Option<PathBuf>,
    ) -> CommandOutcome {
        // TODO(T021): resolve strictness + spec_json_path + trusted_root, then invoke
        // infrastructure::verify::spec_states::verify_from_spec_json and render_outcome here.
        // Mirrors cli_composition/src/signal/mod.rs SignalCompositionRoot::signal_check_spec_adr.
        CommandOutcome::failure(Some("cli_driver Driver::handle is not yet wired — apps/cli still routes through cli_composition CompositionRoot dispatch (deferred from T021); call the matching CompositionRoot method instead".to_owned()))
    }

    fn signal_calc_catalog_spec(&self) -> CommandOutcome {
        // TODO(T021): discover git repo, load TDDD layer bindings, construct
        // BindingSignalLayerReader + FsCatalogueSpecSignalsStore, invoke
        // usecase::signal::calc_catalog_spec and render_outcome here.
        // Mirrors cli_composition/src/signal/mod.rs SignalCompositionRoot::signal_calc_catalog_spec.
        CommandOutcome::failure(Some("cli_driver Driver::handle is not yet wired — apps/cli still routes through cli_composition CompositionRoot dispatch (deferred from T021); call the matching CompositionRoot method instead".to_owned()))
    }

    fn signal_check_catalog_spec(
        &self,
        _strict_override: bool,
        _gate: Option<SignalGateName>,
        _workspace_root: Option<PathBuf>,
    ) -> CommandOutcome {
        // TODO(T021): delegate to signal_check_layer_chain with chain ② parameters.
        // Mirrors cli_composition/src/signal/mod.rs SignalCompositionRoot::signal_check_catalog_spec.
        CommandOutcome::failure(Some("cli_driver Driver::handle is not yet wired — apps/cli still routes through cli_composition CompositionRoot dispatch (deferred from T021); call the matching CompositionRoot method instead".to_owned()))
    }

    fn signal_calc_impl_catalog(&self) -> CommandOutcome {
        // TODO(T021): discover git repo, load layer bindings + TypeSignalsInteractor,
        // invoke usecase::signal::calc_impl_catalog and render_outcome here.
        // Mirrors cli_composition/src/signal/mod.rs SignalCompositionRoot::signal_calc_impl_catalog.
        CommandOutcome::failure(Some("cli_driver Driver::handle is not yet wired — apps/cli still routes through cli_composition CompositionRoot dispatch (deferred from T021); call the matching CompositionRoot method instead".to_owned()))
    }

    fn signal_check_impl_catalog(
        &self,
        _strict_override: bool,
        _gate: Option<SignalGateName>,
        _workspace_root: Option<PathBuf>,
    ) -> CommandOutcome {
        // TODO(T021): delegate to signal_check_layer_chain with chain ③ parameters.
        // Mirrors cli_composition/src/signal/mod.rs SignalCompositionRoot::signal_check_impl_catalog.
        CommandOutcome::failure(Some("cli_driver Driver::handle is not yet wired — apps/cli still routes through cli_composition CompositionRoot dispatch (deferred from T021); call the matching CompositionRoot method instead".to_owned()))
    }

    fn signal_check_gate(
        &self,
        _project_root: Option<PathBuf>,
        _spec_json_path: Option<PathBuf>,
        _gate: SignalGateName,
        _workspace_root: Option<PathBuf>,
    ) -> CommandOutcome {
        // TODO(T021): load gate matrix, build AdrChainAdapter / SpecAdrChainAdapter /
        // LayerChainAdapter, invoke SignalGateInteractor::run_gate,
        // then merge_outcomes(gate_label, chain_outcomes) here.
        // Mirrors cli_composition/src/signal/gate_check.rs SignalCompositionRoot::signal_check_gate.
        CommandOutcome::failure(Some("cli_driver Driver::handle is not yet wired — apps/cli still routes through cli_composition CompositionRoot dispatch (deferred from T021); call the matching CompositionRoot method instead".to_owned()))
    }
}

impl Default for SignalDriver {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Render helpers (duplicated from cli_composition/src/signal/mod.rs lines ~49;
// T021 removes the cli_composition copy and moves these to cli_driver::render).
// ---------------------------------------------------------------------------

/// Merge multiple `CommandOutcome`s into one, with a header/footer label.
///
/// Mirrors `cli_composition::signal::merge_outcomes` (lines ~49 of signal/mod.rs).
#[allow(dead_code)]
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
