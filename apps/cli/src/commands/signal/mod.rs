//! `sotp signal` command namespace.
//!
//! Provides 8 orthogonal `calc-*` / `check-*` subcommands for the 4 SoT Chain
//! signal evaluation chains, plus an aggregate `check --gate commit|merge`
//! that evaluates all four chains and merges their outcomes.
//!
//! # Aggregate `signal check` behaviour
//!
//! `signal check` runs all four chains in declared order (ADR D1 / D5,
//! AC-01, AC-11) and merges their outcomes into a single pass/fail result.
//!
//! Chains ② and ③ require a valid SHA-256 hex digest to compare against the
//! signals file.  When `--catalog-spec-hash` or `--impl-catalog-hash` is
//! omitted, the corresponding chain reports a hash-parse failure as part of
//! the merged outcome (no panic, no clap rejection — but the overall result
//! reflects that those chains could not be evaluated).
//!
//! To obtain a fully-passing aggregate result, callers must supply:
//! - `--spec-json` (chain ①; falls back to `track/items/spec.json` when absent)
//! - `--catalog-spec-hash` (chain ②; chain reports hash error when absent)
//! - `--impl-catalog-hash` (chain ③; chain reports hash error when absent)
//!
//! The `sotp track-active-gate` / `cargo make track-active-gate` wrapper
//! supplies these arguments automatically from the active track context.
//!
//! # UI naming convention
//!
//! CLI surface uses `catalog` (US spelling). Internal Rust chain struct names
//! keep their existing `catalogue` form; only the CLI command names and
//! arguments use `catalog`.
//!
//! # Flag mutual exclusion: `--strict` vs `--gate`
//!
//! Each `check-*` accepts either:
//! - `--strict`: override strictness to `strict=true` (blocks Yellow), or
//! - `--gate commit|merge`: resolve strictness from `signal-gates.json`.
//!
//! These flags are **mutually exclusive**. The default is `--gate commit`.
//! Clap enforces the exclusion via `conflicts_with` at the parser level.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Subcommand, ValueEnum};
use cli_composition::{CompositionError, SignalCompositionRoot};
use cli_driver::signal::SignalGateName;

use super::outcome_to_exit;

// ── Gate value type ───────────────────────────────────────────────────────────

/// Selects the gate context for config-driven strictness resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum GateArg {
    /// CI commit gate (uses `commit_gate.*` cells from `signal-gates.json`).
    Commit,
    /// PR merge gate (uses `merge_gate.*` cells from `signal-gates.json`).
    Merge,
}

impl From<GateArg> for SignalGateName {
    fn from(arg: GateArg) -> Self {
        match arg {
            GateArg::Commit => SignalGateName::Commit,
            GateArg::Merge => SignalGateName::Merge,
        }
    }
}

// ── Common args ───────────────────────────────────────────────────────────────

/// Common flag set for all `check-*` subcommands.
///
/// `--strict` and `--gate` are mutually exclusive (enforced by `conflicts_with`).
/// When neither is given, defaults to `--gate commit`.
#[derive(Args, Debug)]
pub struct CheckFlags {
    /// Override strictness: treat Yellow signals as errors (blocks).
    /// Mutually exclusive with `--gate`.
    #[arg(long, conflicts_with = "gate")]
    pub strict: bool,

    /// Gate context: resolve strictness from `signal-gates.json`.
    /// Mutually exclusive with `--strict`.
    /// Default: `commit`.
    #[arg(long, value_enum, conflicts_with = "strict")]
    pub gate: Option<GateArg>,

    /// Path to workspace root (used to locate `signal-gates.json`).
    /// Defaults to git repository root discovered from CWD.
    #[arg(long)]
    pub workspace_root: Option<PathBuf>,
}

impl CheckFlags {
    /// Resolve the effective `SignalGateName`.
    ///
    /// - `--gate <arg>` → use the specified gate.
    /// - neither `--strict` nor `--gate` → default to `Commit`.
    pub fn gate_name(&self) -> Option<SignalGateName> {
        if self.strict {
            // strict override: no gate needed for config lookup
            None
        } else {
            Some(self.gate.unwrap_or(GateArg::Commit).into())
        }
    }
}

// ── Sub-command definitions ───────────────────────────────────────────────────

mod calc_adr_user;
mod calc_catalog_spec;
mod calc_impl_catalog;
mod calc_spec_adr;
mod check_adr_user;
mod check_catalog_spec;
mod check_impl_catalog;
mod check_spec_adr;

pub use calc_adr_user::CalcAdrUserArgs;
pub use calc_catalog_spec::CalcCatalogSpecArgs;
pub use calc_impl_catalog::CalcImplCatalogArgs;
pub use calc_spec_adr::CalcSpecAdrArgs;
pub use check_adr_user::CheckAdrUserArgs;
pub use check_catalog_spec::CheckCatalogSpecArgs;
pub use check_impl_catalog::CheckImplCatalogArgs;
pub use check_spec_adr::CheckSpecAdrArgs;

// ── Aggregate check args ──────────────────────────────────────────────────────

/// Arguments for the aggregate `signal check --gate commit|merge` command.
///
/// Chains ② and ③ are now argless (T020 / D8): active track and layer
/// enumeration are resolved from the current git branch and
/// `architecture-rules.json` via the usecase orchestrator.  The only user-
/// facing arguments are `--gate`, `--workspace-root`, `--project-root`, and
/// `--spec-json`.
#[derive(Args, Debug)]
pub struct SignalCheckArgs {
    /// Gate context: resolve strictness from `signal-gates.json` for all chains.
    #[arg(long, value_enum, default_value_t = GateArg::Commit)]
    pub gate: GateArg,

    /// Path to workspace root (used to locate `signal-gates.json`).
    /// Defaults to git repository root discovered from CWD.
    #[arg(long)]
    pub workspace_root: Option<PathBuf>,

    /// Project root for chain ⓪ (`knowledge/adr/` scan).
    /// When omitted, defaults to the resolved workspace / git-repo root so
    /// `sotp signal check` works from any subdirectory without scanning a
    /// stray `<subdir>/knowledge/adr/` tree.
    #[arg(long)]
    pub project_root: Option<PathBuf>,

    /// Path to `spec.json` for chain ① (`spec-adr`).
    /// Typically `track/items/<track-id>/spec.json` relative to the workspace root.
    /// When absent, falls back to `track/items/spec.json`; chain ① will report
    /// a file-not-found failure if that path does not exist.
    #[arg(long)]
    pub spec_json: Option<PathBuf>,
}

// ── Top-level enum ────────────────────────────────────────────────────────────

/// `sotp signal` subcommand routing.
#[derive(Subcommand, Debug)]
pub enum SignalCommand {
    /// Compute ADR grounding signals live (chain ⓪, no persistence).
    CalcAdrUser(CalcAdrUserArgs),
    /// Evaluate ADR→user gate (chain ⓪).
    CheckAdrUser(CheckAdrUserArgs),
    /// Compute and persist spec-adr signals to spec.json (chain ①).
    CalcSpecAdr(CalcSpecAdrArgs),
    /// Evaluate spec→ADR gate (chain ①).
    CheckSpecAdr(CheckSpecAdrArgs),
    /// Compute and persist catalog-spec signals (chain ②).
    CalcCatalogSpec(CalcCatalogSpecArgs),
    /// Evaluate catalog→spec gate (chain ②).
    CheckCatalogSpec(CheckCatalogSpecArgs),
    /// Compute and persist impl-catalog signals (chain ③).
    CalcImplCatalog(CalcImplCatalogArgs),
    /// Evaluate impl↔catalog gate (chain ③).
    CheckImplCatalog(CheckImplCatalogArgs),
    /// Aggregate gate check: run all 4 chains in declared order.
    Check(SignalCheckArgs),
}

/// Dispatch a [`SignalCommand`] and return an [`ExitCode`].
pub fn execute(cmd: SignalCommand) -> ExitCode {
    let driver = SignalCompositionRoot::new().signal_driver();
    match cmd {
        SignalCommand::CalcAdrUser(args) => outcome_to_exit(calc_adr_user::run(&driver, args)),
        SignalCommand::CheckAdrUser(args) => outcome_to_exit(check_adr_user::run(&driver, args)),
        SignalCommand::CalcSpecAdr(args) => outcome_to_exit(calc_spec_adr::run(&driver, args)),
        SignalCommand::CheckSpecAdr(args) => outcome_to_exit(check_spec_adr::run(&driver, args)),
        SignalCommand::CalcCatalogSpec(args) => {
            outcome_to_exit(calc_catalog_spec::run(&driver, args))
        }
        SignalCommand::CheckCatalogSpec(args) => {
            outcome_to_exit(check_catalog_spec::run(&driver, args))
        }
        SignalCommand::CalcImplCatalog(args) => {
            outcome_to_exit(calc_impl_catalog::run(&driver, args))
        }
        SignalCommand::CheckImplCatalog(args) => {
            outcome_to_exit(check_impl_catalog::run(&driver, args))
        }
        SignalCommand::Check(args) => outcome_to_exit(run_aggregate_check(&driver, args)),
    }
}

fn run_aggregate_check(
    driver: &cli_driver::signal::SignalDriver,
    args: SignalCheckArgs,
) -> Result<cli_composition::CommandOutcome, CompositionError> {
    let gate = match args.gate {
        GateArg::Commit => cli_driver::signal::SignalGateName::Commit,
        GateArg::Merge => cli_driver::signal::SignalGateName::Merge,
    };
    Ok(driver.handle(cli_driver::signal::SignalInput::CheckGate {
        project_root: args.project_root,
        spec_json_path: args.spec_json,
        gate,
        workspace_root: args.workspace_root,
    }))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    use clap::Parser;
    use cli_driver::signal::SignalDriver;
    use infrastructure::{signal::SystemSignalGateConfigAdapter, verify::VerifyOutcome};
    use usecase::signal_service::{
        ResolvedSignalChainCommand, SignalChainExecutionReport,
        SignalCommand as ServiceSignalCommand, SignalCommandInteractor, SignalCommandOutput,
        SignalCommandPort, SignalCommandPortError, SignalGateName as ServiceSignalGateName,
        SignalService,
    };

    use super::*;

    const RECORDED_STDOUT: &str = "recorded stdout";
    const RECORDED_STDERR: &str = "recorded stderr";
    const RECORDED_EXIT_CODE: u8 = 23;

    struct RecordingSignalService(Mutex<Vec<ServiceSignalCommand>>);

    impl RecordingSignalService {
        fn calls(&self) -> Vec<ServiceSignalCommand> {
            self.0.lock().unwrap().clone()
        }

        fn record(&self, command: ServiceSignalCommand) -> SignalCommandOutput {
            self.0.lock().unwrap().push(command);
            SignalCommandOutput {
                stdout: Some(RECORDED_STDOUT.to_owned()),
                stderr: Some(RECORDED_STDERR.to_owned()),
                exit_code: RECORDED_EXIT_CODE,
            }
        }
    }

    impl SignalService for RecordingSignalService {
        fn calc_adr_user(&self, project_root: PathBuf) -> SignalCommandOutput {
            self.record(ServiceSignalCommand::CalcAdrUser { project_root })
        }

        fn check_adr_user(
            &self,
            project_root: PathBuf,
            strict_override: bool,
            gate: Option<ServiceSignalGateName>,
            workspace_root: Option<PathBuf>,
        ) -> SignalCommandOutput {
            self.record(ServiceSignalCommand::CheckAdrUser {
                project_root,
                strict_override: if strict_override {
                    usecase::signal_service::SignalStrictOverride::ForceStrict
                } else {
                    usecase::signal_service::SignalStrictOverride::UseGateMatrix
                },
                gate,
                workspace_root,
            })
        }

        fn calc_spec_adr(
            &self,
            spec_json_path: Option<PathBuf>,
            workspace_root: Option<PathBuf>,
        ) -> SignalCommandOutput {
            self.record(ServiceSignalCommand::CalcSpecAdr { spec_json_path, workspace_root })
        }

        fn check_spec_adr(
            &self,
            spec_json_path: Option<PathBuf>,
            strict_override: bool,
            gate: Option<ServiceSignalGateName>,
            workspace_root: Option<PathBuf>,
        ) -> SignalCommandOutput {
            self.record(ServiceSignalCommand::CheckSpecAdr {
                spec_json_path,
                strict_override: if strict_override {
                    usecase::signal_service::SignalStrictOverride::ForceStrict
                } else {
                    usecase::signal_service::SignalStrictOverride::UseGateMatrix
                },
                gate,
                workspace_root,
            })
        }

        fn calc_catalog_spec(&self) -> SignalCommandOutput {
            self.record(ServiceSignalCommand::CalcCatalogSpec)
        }

        fn check_catalog_spec(
            &self,
            strict_override: bool,
            gate: Option<ServiceSignalGateName>,
            workspace_root: Option<PathBuf>,
        ) -> SignalCommandOutput {
            self.record(ServiceSignalCommand::CheckCatalogSpec {
                strict_override: if strict_override {
                    usecase::signal_service::SignalStrictOverride::ForceStrict
                } else {
                    usecase::signal_service::SignalStrictOverride::UseGateMatrix
                },
                gate,
                workspace_root,
            })
        }

        fn calc_impl_catalog(&self) -> SignalCommandOutput {
            self.record(ServiceSignalCommand::CalcImplCatalog)
        }

        fn check_impl_catalog(
            &self,
            strict_override: bool,
            gate: Option<ServiceSignalGateName>,
            workspace_root: Option<PathBuf>,
        ) -> SignalCommandOutput {
            self.record(ServiceSignalCommand::CheckImplCatalog {
                strict_override: if strict_override {
                    usecase::signal_service::SignalStrictOverride::ForceStrict
                } else {
                    usecase::signal_service::SignalStrictOverride::UseGateMatrix
                },
                gate,
                workspace_root,
            })
        }

        fn check_gate(
            &self,
            project_root: Option<PathBuf>,
            spec_json_path: Option<PathBuf>,
            gate: ServiceSignalGateName,
            workspace_root: Option<PathBuf>,
        ) -> SignalCommandOutput {
            self.record(ServiceSignalCommand::CheckGate {
                project_root,
                spec_json_path,
                gate,
                workspace_root,
            })
        }
    }

    fn recording_driver() -> (SignalDriver, Arc<RecordingSignalService>) {
        let service = Arc::new(RecordingSignalService(Mutex::new(Vec::new())));
        (SignalDriver::new(service.clone()), service)
    }

    fn assert_recorded_outcome(outcome: &cli_composition::CommandOutcome) {
        assert_eq!(outcome.stdout.as_deref(), Some(RECORDED_STDOUT));
        assert_eq!(outcome.stderr.as_deref(), Some(RECORDED_STDERR));
        assert_eq!(outcome.exit_code, RECORDED_EXIT_CODE);
    }

    struct RecordingSignalPort(Mutex<Vec<ResolvedSignalChainCommand>>);

    impl RecordingSignalPort {
        fn calls(&self) -> Vec<ResolvedSignalChainCommand> {
            self.0.lock().unwrap().clone()
        }
    }

    impl SignalCommandPort for RecordingSignalPort {
        fn execute(
            &self,
            command: ResolvedSignalChainCommand,
        ) -> Result<SignalChainExecutionReport, SignalCommandPortError> {
            self.0.lock().unwrap().push(command);
            Ok(SignalChainExecutionReport {
                outcome: VerifyOutcome::pass(),
                stdout: Some(RECORDED_STDOUT.to_owned()),
                stderr: Some(RECORDED_STDERR.to_owned()),
            })
        }
    }

    #[test]
    fn test_calc_adr_user_run_with_driver_dispatches_command_and_preserves_outcome() {
        let (driver, service) = recording_driver();
        let project_root = PathBuf::from("project");

        let outcome =
            calc_adr_user::run(&driver, CalcAdrUserArgs { project_root: project_root.clone() })
                .unwrap();

        assert_recorded_outcome(&outcome);
        assert_eq!(service.calls(), vec![ServiceSignalCommand::CalcAdrUser { project_root }]);
    }

    #[test]
    fn test_calc_adr_user_run_with_real_interactor_reaches_port_once_and_preserves_outcome() {
        let port = Arc::new(RecordingSignalPort(Mutex::new(Vec::new())));
        let adapter = Arc::new(infrastructure::signal::SystemSignalCommandAdapter::new());
        let interactor = Arc::new(SignalCommandInteractor::new(
            port.clone(),
            adapter.clone(),
            adapter,
            Arc::new(SystemSignalGateConfigAdapter::new()),
        ));
        let driver = SignalDriver::new(interactor);
        let project_root = PathBuf::from("project");

        let outcome =
            calc_adr_user::run(&driver, CalcAdrUserArgs { project_root: project_root.clone() })
                .unwrap();

        assert_eq!(outcome.stdout.as_deref(), Some(RECORDED_STDOUT));
        assert_eq!(outcome.stderr.as_deref(), Some(RECORDED_STDERR));
        assert_eq!(outcome.exit_code, 0);
        assert_eq!(port.calls(), vec![ResolvedSignalChainCommand::CalcAdrUser { project_root }]);
    }

    #[test]
    fn test_calc_catalog_spec_run_with_driver_dispatches_command_and_preserves_outcome() {
        let (driver, service) = recording_driver();

        let outcome = calc_catalog_spec::run(&driver, CalcCatalogSpecArgs {}).unwrap();

        assert_recorded_outcome(&outcome);
        assert_eq!(service.calls(), vec![ServiceSignalCommand::CalcCatalogSpec]);
    }

    #[test]
    fn test_calc_impl_catalog_run_with_driver_dispatches_command_and_preserves_outcome() {
        let (driver, service) = recording_driver();

        let outcome = calc_impl_catalog::run(&driver, CalcImplCatalogArgs {}).unwrap();

        assert_recorded_outcome(&outcome);
        assert_eq!(service.calls(), vec![ServiceSignalCommand::CalcImplCatalog]);
    }

    #[test]
    fn test_calc_spec_adr_run_with_driver_dispatches_command_and_preserves_outcome() {
        let (driver, service) = recording_driver();
        let spec_json_path = PathBuf::from("project/spec.json");
        let workspace_root = PathBuf::from("workspace");

        let outcome = calc_spec_adr::run(
            &driver,
            CalcSpecAdrArgs {
                spec_json: Some(spec_json_path.clone()),
                workspace_root: Some(workspace_root.clone()),
            },
        )
        .unwrap();

        assert_recorded_outcome(&outcome);
        assert_eq!(
            service.calls(),
            vec![ServiceSignalCommand::CalcSpecAdr {
                spec_json_path: Some(spec_json_path),
                workspace_root: Some(workspace_root),
            }]
        );
    }

    #[test]
    fn test_check_adr_user_run_with_driver_dispatches_command_and_preserves_outcome() {
        let (driver, service) = recording_driver();
        let project_root = PathBuf::from("project");
        let workspace_root = PathBuf::from("workspace");

        let outcome = check_adr_user::run(
            &driver,
            CheckAdrUserArgs {
                project_root: project_root.clone(),
                flags: CheckFlags {
                    strict: false,
                    gate: Some(GateArg::Merge),
                    workspace_root: Some(workspace_root.clone()),
                },
            },
        )
        .unwrap();

        assert_recorded_outcome(&outcome);
        assert_eq!(
            service.calls(),
            vec![ServiceSignalCommand::CheckAdrUser {
                project_root,
                strict_override: usecase::signal_service::SignalStrictOverride::UseGateMatrix,
                gate: Some(ServiceSignalGateName::Merge),
                workspace_root: Some(workspace_root),
            }]
        );
    }

    #[test]
    fn test_check_catalog_spec_run_with_driver_dispatches_command_and_preserves_outcome() {
        let (driver, service) = recording_driver();
        let workspace_root = PathBuf::from("workspace");

        let outcome = check_catalog_spec::run(
            &driver,
            CheckCatalogSpecArgs {
                flags: CheckFlags {
                    strict: true,
                    gate: None,
                    workspace_root: Some(workspace_root.clone()),
                },
            },
        )
        .unwrap();

        assert_recorded_outcome(&outcome);
        assert_eq!(
            service.calls(),
            vec![ServiceSignalCommand::CheckCatalogSpec {
                strict_override: usecase::signal_service::SignalStrictOverride::ForceStrict,
                gate: None,
                workspace_root: Some(workspace_root),
            }]
        );
    }

    #[test]
    fn test_check_impl_catalog_run_with_driver_dispatches_command_and_preserves_outcome() {
        let (driver, service) = recording_driver();
        let workspace_root = PathBuf::from("workspace");

        let outcome = check_impl_catalog::run(
            &driver,
            CheckImplCatalogArgs {
                flags: CheckFlags {
                    strict: false,
                    gate: Some(GateArg::Commit),
                    workspace_root: Some(workspace_root.clone()),
                },
            },
        )
        .unwrap();

        assert_recorded_outcome(&outcome);
        assert_eq!(
            service.calls(),
            vec![ServiceSignalCommand::CheckImplCatalog {
                strict_override: usecase::signal_service::SignalStrictOverride::UseGateMatrix,
                gate: Some(ServiceSignalGateName::Commit),
                workspace_root: Some(workspace_root),
            }]
        );
    }

    #[test]
    fn test_check_spec_adr_run_with_driver_dispatches_command_and_preserves_outcome() {
        let (driver, service) = recording_driver();
        let spec_json_path = PathBuf::from("project/spec.json");
        let workspace_root = PathBuf::from("workspace");

        let outcome = check_spec_adr::run(
            &driver,
            CheckSpecAdrArgs {
                spec_json: Some(spec_json_path.clone()),
                flags: CheckFlags {
                    strict: false,
                    gate: Some(GateArg::Merge),
                    workspace_root: Some(workspace_root.clone()),
                },
            },
        )
        .unwrap();

        assert_recorded_outcome(&outcome);
        assert_eq!(
            service.calls(),
            vec![ServiceSignalCommand::CheckSpecAdr {
                spec_json_path: Some(spec_json_path),
                strict_override: usecase::signal_service::SignalStrictOverride::UseGateMatrix,
                gate: Some(ServiceSignalGateName::Merge),
                workspace_root: Some(workspace_root),
            }]
        );
    }

    #[test]
    fn test_run_aggregate_check_with_driver_dispatches_gate_and_preserves_outcome() {
        let (driver, service) = recording_driver();
        let project_root = PathBuf::from("project");
        let spec_json_path = PathBuf::from("project/spec.json");
        let workspace_root = PathBuf::from("workspace");

        let outcome = run_aggregate_check(
            &driver,
            SignalCheckArgs {
                gate: GateArg::Merge,
                project_root: Some(project_root.clone()),
                spec_json: Some(spec_json_path.clone()),
                workspace_root: Some(workspace_root.clone()),
            },
        )
        .unwrap();

        assert_recorded_outcome(&outcome);
        assert_eq!(
            service.calls(),
            vec![ServiceSignalCommand::CheckGate {
                project_root: Some(project_root),
                spec_json_path: Some(spec_json_path),
                gate: ServiceSignalGateName::Merge,
                workspace_root: Some(workspace_root),
            }]
        );
    }

    // ── Parser wrapper ────────────────────────────────────────────────────────

    #[derive(Parser)]
    struct TestCli {
        #[command(subcommand)]
        cmd: SignalCommand,
    }

    // ── Command name surface (catalog, not catalogue) ─────────────────────────

    #[test]
    fn test_signal_calc_adr_user_parses_correctly() {
        let cli =
            TestCli::try_parse_from(["sotp", "calc-adr-user", "--project-root", "."]).unwrap();
        assert!(matches!(cli.cmd, SignalCommand::CalcAdrUser(_)));
    }

    #[test]
    fn test_signal_check_adr_user_parses_correctly() {
        let cli =
            TestCli::try_parse_from(["sotp", "check-adr-user", "--project-root", "."]).unwrap();
        assert!(matches!(cli.cmd, SignalCommand::CheckAdrUser(_)));
    }

    #[test]
    fn test_signal_calc_spec_adr_parses_correctly() {
        let cli = TestCli::try_parse_from(["sotp", "calc-spec-adr", "--spec-json", "/tmp/s.json"])
            .unwrap();
        assert!(matches!(cli.cmd, SignalCommand::CalcSpecAdr(_)));
    }

    #[test]
    fn test_signal_check_spec_adr_parses_correctly() {
        let cli = TestCli::try_parse_from(["sotp", "check-spec-adr", "--spec-json", "/tmp/s.json"])
            .unwrap();
        assert!(matches!(cli.cmd, SignalCommand::CheckSpecAdr(_)));
    }

    #[test]
    fn test_signal_calc_catalog_spec_name_uses_catalog_not_catalogue() {
        // Command name must be `calc-catalog-spec` (US spelling); argless in T020.
        let cli = TestCli::try_parse_from(["sotp", "calc-catalog-spec"]).unwrap();
        assert!(matches!(cli.cmd, SignalCommand::CalcCatalogSpec(_)));
    }

    #[test]
    fn test_signal_check_catalog_spec_name_uses_catalog_not_catalogue() {
        // Argless in T020: no --signals-path / --catalog-hash needed.
        let cli = TestCli::try_parse_from(["sotp", "check-catalog-spec"]).unwrap();
        assert!(matches!(cli.cmd, SignalCommand::CheckCatalogSpec(_)));
    }

    #[test]
    fn test_signal_calc_impl_catalog_parses_correctly() {
        // Argless in T020.
        let cli = TestCli::try_parse_from(["sotp", "calc-impl-catalog"]).unwrap();
        assert!(matches!(cli.cmd, SignalCommand::CalcImplCatalog(_)));
    }

    #[test]
    fn test_signal_check_impl_catalog_parses_correctly() {
        // Argless in T020.
        let cli = TestCli::try_parse_from(["sotp", "check-impl-catalog"]).unwrap();
        assert!(matches!(cli.cmd, SignalCommand::CheckImplCatalog(_)));
    }

    // ── Aggregate check ───────────────────────────────────────────────────────

    #[test]
    fn test_signal_check_gate_commit_parses_correctly() {
        // All optional args absent — gate defaults to commit; no required hash args.
        let cli = TestCli::try_parse_from(["sotp", "check", "--gate", "commit"]).unwrap();
        match cli.cmd {
            SignalCommand::Check(args) => {
                assert_eq!(args.gate, GateArg::Commit);
            }
            other => panic!("expected Check, got {other:?}"),
        }
    }

    #[test]
    fn test_signal_check_gate_merge_parses_correctly() {
        let cli = TestCli::try_parse_from(["sotp", "check", "--gate", "merge"]).unwrap();
        match cli.cmd {
            SignalCommand::Check(args) => {
                assert_eq!(args.gate, GateArg::Merge);
            }
            other => panic!("expected Check, got {other:?}"),
        }
    }

    #[test]
    fn test_signal_check_defaults_to_commit_gate() {
        // Bare invocation: all optional args absent, gate defaults to commit.
        let cli = TestCli::try_parse_from(["sotp", "check"]).unwrap();
        match cli.cmd {
            SignalCommand::Check(args) => {
                assert_eq!(args.gate, GateArg::Commit);
            }
            other => panic!("expected Check, got {other:?}"),
        }
    }

    #[test]
    fn test_signal_check_with_spec_json_parses_correctly() {
        // T020: hash/signals-path args removed; spec-json is still optional.
        let cli =
            TestCli::try_parse_from(["sotp", "check", "--spec-json", "/tmp/spec.json"]).unwrap();
        match cli.cmd {
            SignalCommand::Check(args) => {
                assert_eq!(args.spec_json, Some(PathBuf::from("/tmp/spec.json")));
                assert_eq!(args.gate, GateArg::Commit);
            }
            other => panic!("expected Check, got {other:?}"),
        }
    }

    // ── --strict / --gate mutual exclusion ───────────────────────────────────

    #[test]
    fn test_check_adr_user_strict_and_gate_together_is_parser_error() {
        let result =
            TestCli::try_parse_from(["sotp", "check-adr-user", "--strict", "--gate", "commit"]);
        assert!(result.is_err(), "--strict and --gate must be mutually exclusive");
    }

    #[test]
    fn test_check_spec_adr_strict_and_gate_together_is_parser_error() {
        let result = TestCli::try_parse_from([
            "sotp",
            "check-spec-adr",
            "--spec-json",
            "/tmp/s.json",
            "--strict",
            "--gate",
            "merge",
        ]);
        assert!(
            result.is_err(),
            "--strict and --gate must be mutually exclusive for check-spec-adr"
        );
    }

    #[test]
    fn test_check_catalog_spec_strict_flag_accepted() {
        // T020: argless command; --strict is the only required flag change.
        let cli = TestCli::try_parse_from(["sotp", "check-catalog-spec", "--strict"]).unwrap();
        match cli.cmd {
            SignalCommand::CheckCatalogSpec(args) => {
                assert!(args.flags.strict);
                assert!(args.flags.gate.is_none());
            }
            other => panic!("expected CheckCatalogSpec, got {other:?}"),
        }
    }

    #[test]
    fn test_check_catalog_spec_gate_flag_accepted() {
        // T020: argless command.
        let cli =
            TestCli::try_parse_from(["sotp", "check-catalog-spec", "--gate", "merge"]).unwrap();
        match cli.cmd {
            SignalCommand::CheckCatalogSpec(args) => {
                assert!(!args.flags.strict);
                assert_eq!(args.flags.gate, Some(GateArg::Merge));
            }
            other => panic!("expected CheckCatalogSpec, got {other:?}"),
        }
    }

    #[test]
    fn test_check_impl_catalog_strict_and_gate_together_is_parser_error() {
        // T020: argless command; only --strict and --gate matter.
        let result =
            TestCli::try_parse_from(["sotp", "check-impl-catalog", "--strict", "--gate", "commit"]);
        assert!(result.is_err(), "--strict and --gate must be mutually exclusive");
    }

    // ── gate_name resolution ─────────────────────────────────────────────────

    #[test]
    fn test_check_flags_gate_name_returns_none_when_strict_true() {
        let flags = CheckFlags { strict: true, gate: None, workspace_root: None };
        assert!(flags.gate_name().is_none());
    }

    #[test]
    fn test_check_flags_gate_name_returns_commit_by_default() {
        let flags = CheckFlags { strict: false, gate: None, workspace_root: None };
        assert_eq!(flags.gate_name(), Some(SignalGateName::Commit));
    }

    #[test]
    fn test_check_flags_gate_name_returns_merge_when_gate_is_merge() {
        let flags = CheckFlags { strict: false, gate: Some(GateArg::Merge), workspace_root: None };
        assert_eq!(flags.gate_name(), Some(SignalGateName::Merge));
    }

    // ── GateArg -> SignalGateName conversion ──────────────────────────────────

    #[test]
    fn test_gate_arg_commit_converts_to_signal_gate_name_commit() {
        assert_eq!(SignalGateName::from(GateArg::Commit), SignalGateName::Commit);
    }

    #[test]
    fn test_gate_arg_merge_converts_to_signal_gate_name_merge() {
        assert_eq!(SignalGateName::from(GateArg::Merge), SignalGateName::Merge);
    }
}
