//! `signal` command family — primary adapter driver.
//!
//! `SignalDriver` holds an injected [`usecase::signal_service::SignalService`]
//! and exposes `handle(input) -> CommandOutcome`.

use std::path::PathBuf;
use std::sync::Arc;

use usecase::signal_service::{
    SignalCommandOutput, SignalGateName as ServiceGateName, SignalService,
};

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
// Gate name conversion
// ---------------------------------------------------------------------------

fn to_service_gate(gate: SignalGateName) -> ServiceGateName {
    match gate {
        SignalGateName::Commit => ServiceGateName::Commit,
        SignalGateName::Merge => ServiceGateName::Merge,
    }
}

fn service_output_to_outcome(output: SignalCommandOutput) -> CommandOutcome {
    CommandOutcome { stdout: output.stdout, stderr: output.stderr, exit_code: output.exit_code }
}

// ---------------------------------------------------------------------------
// Driver
// ---------------------------------------------------------------------------

/// Primary adapter driver for the `signal` command family.
///
/// Holds an injected [`SignalService`]; exposes `handle(input) -> CommandOutcome`.
pub struct SignalDriver {
    service: Arc<dyn SignalService>,
}

impl SignalDriver {
    /// Create a new `SignalDriver` with the given service.
    pub fn new(service: Arc<dyn SignalService>) -> Self {
        Self { service }
    }

    /// Handle a signal command.
    pub fn handle(&self, input: SignalInput) -> CommandOutcome {
        match input {
            SignalInput::CalcAdrUser { project_root } => {
                service_output_to_outcome(self.service.calc_adr_user(project_root))
            }
            SignalInput::CheckAdrUser { project_root, strict_override, gate, workspace_root } => {
                service_output_to_outcome(self.service.check_adr_user(
                    project_root,
                    strict_override,
                    gate.map(to_service_gate),
                    workspace_root,
                ))
            }
            SignalInput::CalcSpecAdr { spec_json_path, workspace_root } => {
                service_output_to_outcome(
                    self.service.calc_spec_adr(spec_json_path, workspace_root),
                )
            }
            SignalInput::CheckSpecAdr { spec_json_path, strict_override, gate, workspace_root } => {
                service_output_to_outcome(self.service.check_spec_adr(
                    spec_json_path,
                    strict_override,
                    gate.map(to_service_gate),
                    workspace_root,
                ))
            }
            SignalInput::CalcCatalogSpec => {
                service_output_to_outcome(self.service.calc_catalog_spec())
            }
            SignalInput::CheckCatalogSpec { strict_override, gate, workspace_root } => {
                service_output_to_outcome(self.service.check_catalog_spec(
                    strict_override,
                    gate.map(to_service_gate),
                    workspace_root,
                ))
            }
            SignalInput::CalcImplCatalog => {
                service_output_to_outcome(self.service.calc_impl_catalog())
            }
            SignalInput::CheckImplCatalog { strict_override, gate, workspace_root } => {
                service_output_to_outcome(self.service.check_impl_catalog(
                    strict_override,
                    gate.map(to_service_gate),
                    workspace_root,
                ))
            }
            SignalInput::CheckGate { project_root, spec_json_path, gate, workspace_root } => {
                service_output_to_outcome(self.service.check_gate(
                    project_root,
                    spec_json_path,
                    to_service_gate(gate),
                    workspace_root,
                ))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Render helpers (now unused — previously duplicated from cli_composition)
// ---------------------------------------------------------------------------

/// Merge multiple `CommandOutcome`s into one, with a header/footer label.
///
/// Mirrors `cli_composition::signal::merge_outcomes` (lines ~49 of signal/mod.rs).
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

// Keep merge_outcomes in scope — used transitionally.
const _: fn() = || {
    let _ = merge_outcomes;
};
