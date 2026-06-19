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

/// Run a catalogue-hash calc command.
///
/// Parses the hex hash, builds a `VerifyOutcome` via the supplied closure
/// (which runs `Chain::calc`), then renders the outcome.  Each calc command
/// only supplies its `command_label` and the chain-specific `build_and_calc`
/// closure; all shared parsing/error/render logic lives here.
fn run_catalogue_calc(
    command_label: &str,
    catalogue_hash: &str,
    build_and_calc: impl FnOnce(&domain::ContentHash) -> infrastructure::verify::VerifyOutcome,
) -> CommandOutcome {
    let hash = match parse_catalogue_hash(command_label, catalogue_hash) {
        Ok(h) => h,
        Err(outcome) => return outcome,
    };
    let outcome = build_and_calc(&hash);
    render_outcome(command_label, &outcome)
}

/// Run a catalogue-hash check command.
///
/// Resolves strictness, parses the hex hash, runs the supplied check closure,
/// then renders the outcome.  Each check command only supplies its label,
/// `ChainId`, override/gate flags, and the chain-specific `build_and_check`
/// closure; all shared resolve/parse/render logic lives here.
fn run_catalogue_check(
    command_label: &str,
    chain_id: domain::ChainId,
    strict_override: bool,
    gate: Option<SignalGateName>,
    workspace_root: Option<&std::path::Path>,
    catalogue_hash: &str,
    build_and_check: impl FnOnce(&domain::ContentHash, bool) -> infrastructure::verify::VerifyOutcome,
) -> CommandOutcome {
    let strict = match resolve_strict(strict_override, gate, chain_id, workspace_root) {
        Ok(s) => s,
        Err(outcome) => return outcome,
    };
    let hash = match parse_catalogue_hash(command_label, catalogue_hash) {
        Ok(h) => h,
        Err(outcome) => return outcome,
    };
    let outcome = build_and_check(&hash, strict);
    render_outcome(command_label, &outcome)
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

// ── CliApp impl ───────────────────────────────────────────────────────────────

impl CliApp {
    // ── calc-adr-user ─────────────────────────────────────────────────────────

    /// Compute ADR signal grounding live from `project_root/knowledge/adr/`.
    ///
    /// Delegates to `AdrUserChain::calc_live` via `LiveSoTChain`.
    /// No persistence — result is displayed only.
    ///
    /// # Errors
    ///
    /// Returns `Err` when the live calculation fails (e.g. port not yet wired).
    pub fn signal_calc_adr_user(&self, project_root: PathBuf) -> Result<CommandOutcome, String> {
        use domain::LiveSoTChain as _;
        use usecase::chain::AdrUserChain;

        match AdrUserChain::calc_live(&project_root.as_path()) {
            Ok(report) => {
                let msg = format!(
                    "--- signal calc-adr-user ---\n\
                     ADR grounding report: blue={}, yellow={}, red={}, grandfathered={}\n\
                     --- signal calc-adr-user DONE ---",
                    report.blue_count(),
                    report.yellow_count(),
                    report.red_count(),
                    report.grandfathered_count(),
                );
                Ok(CommandOutcome::success(Some(msg)))
            }
            Err(e) => Ok(CommandOutcome::failure(Some(format!(
                "--- signal calc-adr-user ---\n\
                 [ERROR] {e}\n\
                 --- signal calc-adr-user FAILED ---"
            )))),
        }
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
        use domain::SoTChain as _;
        use usecase::chain::AdrUserChain;

        let strict = match resolve_strict(
            strict_override,
            gate,
            domain::ChainId::AdrUser,
            workspace_root.as_deref(),
        ) {
            Ok(s) => s,
            Err(outcome) => return Ok(outcome),
        };

        let outcome = AdrUserChain::check(&project_root.as_path(), strict);
        Ok(render_outcome("signal check-adr-user", &outcome))
    }

    // ── calc-spec-adr ─────────────────────────────────────────────────────────

    /// Compute and persist chain ① (spec→ADR) signals to `spec.json`.
    ///
    /// Delegates to `SpecAdrChain::calc` via `PersistedSoTChain`.
    ///
    /// # Errors
    ///
    /// Returns `Err` when the calculation fails (I/O not yet wired in T007).
    pub fn signal_calc_spec_adr(&self, spec_json_path: PathBuf) -> Result<CommandOutcome, String> {
        use domain::PersistedSoTChain as _;
        use usecase::chain::{SpecAdrChain, SpecAdrInput};

        let input = SpecAdrInput::new(&spec_json_path);
        let outcome = match SpecAdrChain::calc(&input) {
            Ok(_doc) => infrastructure::verify::VerifyOutcome::pass(),
            Err(e) => infrastructure::verify::VerifyOutcome::from_findings(vec![
                infrastructure::verify::VerifyFinding::error(format!("{e}")),
            ]),
        };
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
        use domain::SoTChain as _;
        use usecase::chain::{SpecAdrChain, SpecAdrInput};

        let strict = match resolve_strict(
            strict_override,
            gate,
            domain::ChainId::SpecAdr,
            workspace_root.as_deref(),
        ) {
            Ok(s) => s,
            Err(outcome) => return Ok(outcome),
        };

        let input = SpecAdrInput::new(&spec_json_path);
        let outcome = SpecAdrChain::check(&input, strict);
        Ok(render_outcome("signal check-spec-adr", &outcome))
    }

    // ── calc-catalog-spec ─────────────────────────────────────────────────────

    /// Compute and persist chain ② (catalog→spec) signals.
    ///
    /// Delegates to `CatalogSpecChain::calc` via `PersistedSoTChain`.
    ///
    /// # Errors
    ///
    /// Returns `Err` when the calculation fails (I/O not yet wired in T008).
    pub fn signal_calc_catalog_spec(
        &self,
        signals_path: PathBuf,
        catalogue_hash: String,
    ) -> Result<CommandOutcome, String> {
        use domain::PersistedSoTChain as _;
        use usecase::chain::{CatalogSpecChain, CatalogSpecInput};

        Ok(run_catalogue_calc("signal calc-catalog-spec", &catalogue_hash, |hash| {
            match CatalogSpecChain::calc(&CatalogSpecInput::new(&signals_path, hash, &[])) {
                Ok(_doc) => infrastructure::verify::VerifyOutcome::pass(),
                Err(e) => infrastructure::verify::VerifyOutcome::from_findings(vec![
                    infrastructure::verify::VerifyFinding::error(format!("{e}")),
                ]),
            }
        }))
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
        use domain::SoTChain as _;
        use usecase::chain::{CatalogSpecChain, CatalogSpecInput};

        Ok(run_catalogue_check(
            "signal check-catalog-spec",
            domain::ChainId::CatalogSpec,
            strict_override,
            gate,
            workspace_root.as_deref(),
            &catalogue_hash,
            |hash, strict| {
                CatalogSpecChain::check(&CatalogSpecInput::new(&signals_path, hash, &[]), strict)
            },
        ))
    }

    // ── calc-impl-catalog ─────────────────────────────────────────────────────

    /// Compute and persist chain ③ (impl↔catalog) signals.
    ///
    /// Delegates to `ImplCatalogChain::calc` via `PersistedSoTChain`.
    ///
    /// # Errors
    ///
    /// Returns `Err` when the calculation fails (I/O not yet wired in T007).
    pub fn signal_calc_impl_catalog(
        &self,
        signals_path: PathBuf,
        catalogue_hash: String,
    ) -> Result<CommandOutcome, String> {
        use domain::PersistedSoTChain as _;
        use usecase::chain::{ImplCatalogChain, ImplCatalogInput};

        Ok(run_catalogue_calc("signal calc-impl-catalog", &catalogue_hash, |hash| {
            match ImplCatalogChain::calc(&ImplCatalogInput::new(&signals_path, hash)) {
                Ok(_doc) => infrastructure::verify::VerifyOutcome::pass(),
                Err(e) => infrastructure::verify::VerifyOutcome::from_findings(vec![
                    infrastructure::verify::VerifyFinding::error(format!("{e}")),
                ]),
            }
        }))
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
        use domain::SoTChain as _;
        use usecase::chain::{ImplCatalogChain, ImplCatalogInput};

        Ok(run_catalogue_check(
            "signal check-impl-catalog",
            domain::ChainId::ImplCatalog,
            strict_override,
            gate,
            workspace_root.as_deref(),
            &catalogue_hash,
            |hash, strict| {
                ImplCatalogChain::check(&ImplCatalogInput::new(&signals_path, hash), strict)
            },
        ))
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

        // All four chains use the already-loaded matrix.  Call chain-specific logic
        // directly (bypassing the per-chain public helpers that would re-invoke
        // `resolve_strict` / `load_gate_matrix` a second time, dropping the caller's
        // `workspace_root` and `gate` context when the resolved bool is `false`).
        use domain::SoTChain as _;
        use usecase::chain::{
            AdrUserChain, CatalogSpecChain, CatalogSpecInput, ImplCatalogChain, ImplCatalogInput,
            SpecAdrChain, SpecAdrInput,
        };

        // Chain ⓪: adr-user
        let adr_strict =
            matrix.resolve(domain::ChainId::AdrUser, gate_kind) == domain::Strictness::Strict;
        let chain0 = {
            let outcome = AdrUserChain::check(&project_root.as_path(), adr_strict);
            render_outcome("signal check-adr-user", &outcome)
        };

        // Chain ①: spec-adr
        let spec_strict =
            matrix.resolve(domain::ChainId::SpecAdr, gate_kind) == domain::Strictness::Strict;
        let chain1 = {
            let input = SpecAdrInput::new(&spec_json_path);
            let outcome = SpecAdrChain::check(&input, spec_strict);
            render_outcome("signal check-spec-adr", &outcome)
        };

        // Chain ②: catalog-spec
        let catalog_strict =
            matrix.resolve(domain::ChainId::CatalogSpec, gate_kind) == domain::Strictness::Strict;
        let chain2 = match parse_catalogue_hash("signal check-catalog-spec", &catalog_spec_hash) {
            Err(err_outcome) => err_outcome,
            Ok(hash) => {
                let input = CatalogSpecInput::new(&catalog_spec_signals_path, &hash, &[]);
                let outcome = CatalogSpecChain::check(&input, catalog_strict);
                render_outcome("signal check-catalog-spec", &outcome)
            }
        };

        // Chain ③: impl-catalog
        let impl_strict =
            matrix.resolve(domain::ChainId::ImplCatalog, gate_kind) == domain::Strictness::Strict;
        let chain3 = match parse_catalogue_hash("signal check-impl-catalog", &impl_catalog_hash) {
            Err(err_outcome) => err_outcome,
            Ok(hash) => {
                let input = ImplCatalogInput::new(&impl_catalog_signals_path, &hash);
                let outcome = ImplCatalogChain::check(&input, impl_strict);
                render_outcome("signal check-impl-catalog", &outcome)
            }
        };

        Ok(merge_outcomes(gate_label, vec![chain0, chain1, chain2, chain3]))
    }
}
