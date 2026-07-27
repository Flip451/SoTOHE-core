//! `SignalService` — unified application-service facade for all `signal`
//! subcommands.
//!
//! Defines the primary port trait [`SignalService`] and the shared output DTO
//! [`SignalCommandOutput`] that the `cli_driver::signal::SignalDriver` consumes.
//! The composition root (`apps/cli-composition`) implements the trait by wiring
//! the appropriate infrastructure adapters and usecase interactors for each
//! subcommand.
//!
//! # Design rationale
//!
//! The `signal` family has nine subcommands that each require different
//! infrastructure setup (git discovery, ADR scan, spec.json resolution, TDDD
//! layer enumeration, type-signals executor, …).  Defining one wide service
//! trait lets the `SignalDriver` stay a simple dispatcher with a single
//! `Arc<dyn SignalService>` dependency, while the composition root retains
//! full control over wiring without leaking infrastructure types into
//! `cli_driver`.
//!
//! The output type [`SignalCommandOutput`] mirrors `cli_driver::CommandOutcome`
//! field-for-field so the driver can convert it in one expression, without
//! `usecase` needing to import `cli_driver`.

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use domain::verify::VerifyOutcome;
use domain::{ChainId, GateKind, Strictness};

mod error_presentation;

// ── Output DTO ────────────────────────────────────────────────────────────────

/// Unified output DTO for all `signal` subcommands.
///
/// Mirrors `cli_driver::render::CommandOutcome` field-for-field.  Defined here
/// (in the usecase layer) so that the `SignalService` trait does not import
/// `cli_driver`, preserving hexagonal layer order.
///
/// `cli_driver::signal` converts this to `CommandOutcome` in one expression.
#[derive(Debug, Clone)]
pub struct SignalCommandOutput {
    /// Optional text written to stdout.
    pub stdout: Option<String>,
    /// Optional text written to stderr.
    pub stderr: Option<String>,
    /// Process exit code (0 = success, non-zero = failure).
    pub exit_code: u8,
}

impl SignalCommandOutput {
    /// Construct a successful output with optional stdout text.
    pub fn success(stdout: Option<String>) -> Self {
        Self { stdout, stderr: None, exit_code: 0 }
    }

    /// Construct a failure output with optional stderr text.
    pub fn failure(stderr: Option<String>) -> Self {
        Self { stdout: None, stderr, exit_code: 1 }
    }
}

// ── Gate name ─────────────────────────────────────────────────────────────────

/// Selects the gate context when resolving strictness from `signal-gates.json`.
///
/// Re-defined here so that `cli_driver` can pass the value through the
/// `SignalService` port without importing `domain` directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalGateName {
    /// CI commit gate — uses `commit_gate.*` cells.
    Commit,
    /// PR merge gate — uses `merge_gate.*` cells.
    Merge,
}

/// Preserves the two meanings of the CLI `--strict` flag at the application boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalStrictOverride {
    /// Resolve the selected gate-matrix cell.
    UseGateMatrix,
    /// Always use strict verification.
    ForceStrict,
}

/// Selects whether a Signal root was supplied by the caller or must be discovered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignalRootSelection {
    /// Use the caller-supplied path verbatim, including an empty path.
    Supplied(PathBuf),
    /// Discover the repository root through the execution port.
    Discover,
}

/// A fully resolved operation that the Signal execution adapter may perform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedSignalChainCommand {
    CalcAdrUser {
        project_root: PathBuf,
    },
    CheckAdrUser {
        project_root: SignalRootSelection,
        strictness: Strictness,
    },
    CalcSpecAdr {
        spec_json_path: Option<PathBuf>,
        workspace_root: Option<PathBuf>,
    },
    CheckSpecAdr {
        spec_json_path: Option<PathBuf>,
        strictness: Strictness,
        workspace_root: Option<PathBuf>,
    },
    CalcCatalogSpec,
    CheckCatalogSpec {
        strictness: Strictness,
        workspace_root: Option<PathBuf>,
    },
    CalcImplCatalog,
    CheckImplCatalog {
        strictness: Strictness,
        workspace_root: Option<PathBuf>,
    },
}

/// Driver-facing command for the Signal family.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignalCommand {
    CalcAdrUser {
        project_root: PathBuf,
    },
    CheckAdrUser {
        project_root: PathBuf,
        strict_override: SignalStrictOverride,
        gate: Option<SignalGateName>,
        workspace_root: Option<PathBuf>,
    },
    CalcSpecAdr {
        spec_json_path: Option<PathBuf>,
        workspace_root: Option<PathBuf>,
    },
    CheckSpecAdr {
        spec_json_path: Option<PathBuf>,
        strict_override: SignalStrictOverride,
        gate: Option<SignalGateName>,
        workspace_root: Option<PathBuf>,
    },
    CalcCatalogSpec,
    CheckCatalogSpec {
        strict_override: SignalStrictOverride,
        gate: Option<SignalGateName>,
        workspace_root: Option<PathBuf>,
    },
    CalcImplCatalog,
    CheckImplCatalog {
        strict_override: SignalStrictOverride,
        gate: Option<SignalGateName>,
        workspace_root: Option<PathBuf>,
    },
    CheckGate {
        project_root: Option<PathBuf>,
        spec_json_path: Option<PathBuf>,
        gate: SignalGateName,
        workspace_root: Option<PathBuf>,
    },
}

/// Result of a resolved Signal operation.
#[derive(Debug, Clone)]
pub struct SignalChainExecutionReport {
    /// Structured verification result used for fail-closed output construction.
    pub outcome: VerifyOutcome,
    /// Compatibility-preserving standard output.
    pub stdout: Option<String>,
    /// Compatibility-preserving standard error.
    pub stderr: Option<String>,
}

/// Adapter-sanitized diagnostic text that may cross the Signal port boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalFailureReason(String);

impl SignalFailureReason {
    /// Wraps adapter-sanitized diagnostic text for the Signal port boundary.
    #[must_use]
    pub fn new(reason: String) -> Self {
        Self(reason)
    }

    /// Returns the adapter-sanitized diagnostic text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SignalFailureReason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Failure identity for a Signal execution adapter.
#[derive(Debug)]
pub enum SignalCommandPortError {
    /// Git repository discovery failed before command dispatch.
    RepositoryDiscovery { reason: SignalFailureReason },
    /// The repository has no current branch.
    BranchAbsent,
    /// Reading the current branch failed.
    BranchReadFailure { reason: SignalFailureReason },
    /// The active track's spec document could not be resolved.
    SpecPathResolution { reason: SignalFailureReason },
    /// A command's persistence operation failed.
    Persistence { reason: SignalFailureReason },
    /// The adapter could not execute the resolved operation.
    Execution { reason: SignalFailureReason },
}

impl fmt::Display for SignalCommandPortError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BranchAbsent => formatter.write_str("no current branch"),
            Self::RepositoryDiscovery { reason }
            | Self::BranchReadFailure { reason }
            | Self::SpecPathResolution { reason }
            | Self::Persistence { reason }
            | Self::Execution { reason } => reason.fmt(formatter),
        }
    }
}

impl std::error::Error for SignalCommandPortError {}

/// Failure identity for gate-matrix loading.
#[derive(Debug)]
pub enum SignalGateConfigError {
    /// Git repository discovery failed while locating the configuration.
    RepositoryDiscovery { reason: SignalFailureReason },
    /// The gate configuration file does not exist.
    ConfigurationNotFound { path: PathBuf },
    /// The gate configuration could not be parsed or validated.
    ConfigurationInvalid { path: PathBuf, reason: SignalFailureReason },
}

impl fmt::Display for SignalGateConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RepositoryDiscovery { reason } => reason.fmt(formatter),
            Self::ConfigurationNotFound { path } => {
                write!(formatter, "signal-gates.json not found at {}", path.display())
            }
            Self::ConfigurationInvalid { reason, .. } => reason.fmt(formatter),
        }
    }
}

impl std::error::Error for SignalGateConfigError {}

/// Synchronous driven port for a resolved Signal chain operation.
pub trait SignalCommandPort: Send + Sync {
    /// Execute one operation after policy has been resolved.
    fn execute(
        &self,
        command: ResolvedSignalChainCommand,
    ) -> Result<SignalChainExecutionReport, SignalCommandPortError>;
}

/// Resolves the active track against the selected workspace root.
pub trait SignalActiveTrackResolverPort: Send + Sync {
    /// Resolve the active track against the selected workspace root.
    fn resolve_active_track(
        &self,
        workspace_root: Option<&Path>,
    ) -> Result<domain::TrackId, SignalCommandPortError>;
}

/// Resolves a supplied Signal spec path or locates the active track's `spec.json`.
pub trait SignalSpecPathResolverPort: Send + Sync {
    /// Resolve a supplied spec path or locate the active track's `spec.json`.
    fn resolve_spec_path(
        &self,
        workspace_root: Option<&Path>,
        spec_json_path: Option<&Path>,
    ) -> Result<PathBuf, SignalCommandPortError>;
}

/// Loads the validated Signal gate matrix.
pub trait SignalGateConfigPort: Send + Sync {
    /// Load the matrix for an optional workspace root.
    fn load(
        &self,
        workspace_root: Option<&Path>,
    ) -> Result<domain::SignalGateMatrix, SignalGateConfigError>;
}

// ── Primary port ──────────────────────────────────────────────────────────────

/// Primary port for the `signal` command family.
///
/// Each method corresponds to one `sotp signal <subcommand>` invocation.
/// Return value is [`SignalCommandOutput`]; the driver converts it to
/// `CommandOutcome`.
pub trait SignalService: Send + Sync {
    /// `signal calc-adr-user` — compute ADR signal grounding from
    /// `project_root/knowledge/adr/`.
    fn calc_adr_user(&self, project_root: PathBuf) -> SignalCommandOutput;

    /// `signal check-adr-user` — evaluate chain ⓪ (ADR→user) gate.
    fn check_adr_user(
        &self,
        project_root: PathBuf,
        strict_override: bool,
        gate: Option<SignalGateName>,
        workspace_root: Option<PathBuf>,
    ) -> SignalCommandOutput;

    /// `signal calc-spec-adr` — compute and persist chain ① signals to
    /// `spec.json`.
    fn calc_spec_adr(
        &self,
        spec_json_path: Option<PathBuf>,
        workspace_root: Option<PathBuf>,
    ) -> SignalCommandOutput;

    /// `signal check-spec-adr` — evaluate chain ① (spec→ADR) gate.
    fn check_spec_adr(
        &self,
        spec_json_path: Option<PathBuf>,
        strict_override: bool,
        gate: Option<SignalGateName>,
        workspace_root: Option<PathBuf>,
    ) -> SignalCommandOutput;

    /// `signal calc-catalog-spec` — compute and persist chain ② signals for
    /// all TDDD-enabled layers.
    fn calc_catalog_spec(&self) -> SignalCommandOutput;

    /// `signal check-catalog-spec` — evaluate chain ② (catalog→spec) gate.
    fn check_catalog_spec(
        &self,
        strict_override: bool,
        gate: Option<SignalGateName>,
        workspace_root: Option<PathBuf>,
    ) -> SignalCommandOutput;

    /// `signal calc-impl-catalog` — compute and persist chain ③ signals for
    /// all TDDD-enabled layers.
    fn calc_impl_catalog(&self) -> SignalCommandOutput;

    /// `signal check-impl-catalog` — evaluate chain ③ (impl↔catalog) gate.
    fn check_impl_catalog(
        &self,
        strict_override: bool,
        gate: Option<SignalGateName>,
        workspace_root: Option<PathBuf>,
    ) -> SignalCommandOutput;

    /// `signal check --gate` — evaluate commit/merge gate (chains ⓪①②③).
    fn check_gate(
        &self,
        project_root: Option<PathBuf>,
        spec_json_path: Option<PathBuf>,
        gate: SignalGateName,
        workspace_root: Option<PathBuf>,
    ) -> SignalCommandOutput;
}

/// The sole application boundary that resolves Signal policy and makes gate decisions.
pub struct SignalCommandInteractor {
    port: Arc<dyn SignalCommandPort>,
    active_track_resolver: Arc<dyn SignalActiveTrackResolverPort>,
    spec_path_resolver: Arc<dyn SignalSpecPathResolverPort>,
    gate_config: Arc<dyn SignalGateConfigPort>,
}

impl SignalCommandInteractor {
    /// Creates an interactor with its segregated execution and resolver ports.
    #[must_use]
    pub fn new(
        port: Arc<dyn SignalCommandPort>,
        active_track_resolver: Arc<dyn SignalActiveTrackResolverPort>,
        spec_path_resolver: Arc<dyn SignalSpecPathResolverPort>,
        gate_config: Arc<dyn SignalGateConfigPort>,
    ) -> Self {
        Self { port, active_track_resolver, spec_path_resolver, gate_config }
    }

    fn gate_kind(gate: SignalGateName) -> GateKind {
        match gate {
            SignalGateName::Commit => GateKind::Commit,
            SignalGateName::Merge => GateKind::Merge,
        }
    }

    fn resolve(
        &self,
        strict_override: SignalStrictOverride,
        gate: Option<SignalGateName>,
        chain: ChainId,
        workspace_root: Option<PathBuf>,
    ) -> Result<Strictness, SignalCommandOutput> {
        if strict_override == SignalStrictOverride::ForceStrict {
            return Ok(Strictness::Strict);
        }
        let matrix =
            self.gate_config.load(workspace_root.as_deref()).map_err(Self::gate_config_error)?;
        Ok(matrix.resolve(chain, Self::gate_kind(gate.unwrap_or(SignalGateName::Commit))))
    }

    fn output(report: SignalChainExecutionReport) -> SignalCommandOutput {
        SignalCommandOutput {
            stdout: report.stdout,
            stderr: report.stderr,
            exit_code: u8::from(report.outcome.has_errors()),
        }
    }

    fn execute(&self, command: ResolvedSignalChainCommand) -> SignalCommandOutput {
        let command_label = match &command {
            ResolvedSignalChainCommand::CalcCatalogSpec => Some("signal calc-catalog-spec"),
            ResolvedSignalChainCommand::CheckCatalogSpec { .. } => {
                Some("signal check-catalog-spec")
            }
            ResolvedSignalChainCommand::CalcImplCatalog => Some("signal calc-impl-catalog"),
            ResolvedSignalChainCommand::CheckImplCatalog { .. } => {
                Some("signal check-impl-catalog")
            }
            ResolvedSignalChainCommand::CalcAdrUser { .. }
            | ResolvedSignalChainCommand::CheckAdrUser { .. }
            | ResolvedSignalChainCommand::CalcSpecAdr { .. }
            | ResolvedSignalChainCommand::CheckSpecAdr { .. } => None,
        };

        self.port
            .execute(command)
            .map(Self::output)
            .unwrap_or_else(|error| Self::catalogue_command_error(error, command_label))
    }

    fn check_gate(
        &self,
        project_root: Option<PathBuf>,
        spec_json_path: Option<PathBuf>,
        gate: SignalGateName,
        workspace_root: Option<PathBuf>,
    ) -> SignalCommandOutput {
        let matrix = match self.gate_config.load(workspace_root.as_deref()) {
            Ok(matrix) => matrix,
            Err(error) => return Self::gate_config_error(error),
        };
        let gate_kind = Self::gate_kind(gate);
        let adr_user_strictness = matrix.resolve(ChainId::AdrUser, gate_kind);
        let spec_adr_strictness = matrix.resolve(ChainId::SpecAdr, gate_kind);
        let catalog_spec_strictness = matrix.resolve(ChainId::CatalogSpec, gate_kind);
        let impl_catalog_strictness = matrix.resolve(ChainId::ImplCatalog, gate_kind);
        let root = project_root
            .or_else(|| workspace_root.clone())
            .map(SignalRootSelection::Supplied)
            .unwrap_or(SignalRootSelection::Discover);

        // Preserve the baseline failure order: resolve the spec path before the
        // active-track preflight, without executing chain ① during preflight.
        let resolved_spec_path = match self
            .spec_path_resolver
            .resolve_spec_path(workspace_root.as_deref(), spec_json_path.as_deref())
        {
            Ok(path) => path,
            Err(error) => return Self::command_error(error),
        };

        if let Err(error) =
            self.active_track_resolver.resolve_active_track(workspace_root.as_deref())
        {
            return Self::gate_preflight_error(gate, error);
        }
        let reports = [
            match self.port.execute(ResolvedSignalChainCommand::CheckAdrUser {
                project_root: root,
                strictness: adr_user_strictness,
            }) {
                Ok(report) => Self::output(report),
                Err(error) => return Self::command_error(error),
            },
            match self.port.execute(ResolvedSignalChainCommand::CheckSpecAdr {
                spec_json_path: Some(resolved_spec_path),
                strictness: spec_adr_strictness,
                workspace_root: workspace_root.clone(),
            }) {
                Ok(report) => Self::output(report),
                Err(error) => return Self::command_error(error),
            },
            match self.port.execute(ResolvedSignalChainCommand::CheckCatalogSpec {
                strictness: catalog_spec_strictness,
                workspace_root: workspace_root.clone(),
            }) {
                Ok(report) => Self::output(report),
                Err(error) => return Self::command_error(error),
            },
            match self.port.execute(ResolvedSignalChainCommand::CheckImplCatalog {
                strictness: impl_catalog_strictness,
                workspace_root,
            }) {
                Ok(report) => Self::output(report),
                Err(error) => return Self::command_error(error),
            },
        ];
        let label = match gate {
            SignalGateName::Commit => "signal check --gate commit",
            SignalGateName::Merge => "signal check --gate merge",
        };
        let failed = reports.iter().any(|output| output.exit_code != 0);
        let mut lines = vec![format!("--- {label} ---")];
        for output in reports {
            if let Some(stdout) = output.stdout {
                lines.push(stdout);
            }
            if let Some(stderr) = output.stderr {
                lines.push(stderr);
            }
        }
        lines.push(format!("--- {label} {} ---", if failed { "FAILED" } else { "PASSED" }));
        SignalCommandOutput {
            stdout: Some(lines.join("\n")),
            stderr: None,
            exit_code: u8::from(failed),
        }
    }
}

impl SignalService for SignalCommandInteractor {
    fn calc_adr_user(&self, project_root: PathBuf) -> SignalCommandOutput {
        self.execute(ResolvedSignalChainCommand::CalcAdrUser { project_root })
    }
    fn check_adr_user(
        &self,
        project_root: PathBuf,
        strict_override: bool,
        gate: Option<SignalGateName>,
        workspace_root: Option<PathBuf>,
    ) -> SignalCommandOutput {
        let strictness = match self.resolve(
            if strict_override {
                SignalStrictOverride::ForceStrict
            } else {
                SignalStrictOverride::UseGateMatrix
            },
            gate,
            ChainId::AdrUser,
            workspace_root.clone(),
        ) {
            Ok(value) => value,
            Err(output) => return output,
        };
        self.execute(ResolvedSignalChainCommand::CheckAdrUser {
            project_root: SignalRootSelection::Supplied(project_root),
            strictness,
        })
    }
    fn calc_spec_adr(
        &self,
        spec_json_path: Option<PathBuf>,
        workspace_root: Option<PathBuf>,
    ) -> SignalCommandOutput {
        self.execute(ResolvedSignalChainCommand::CalcSpecAdr { spec_json_path, workspace_root })
    }
    fn check_spec_adr(
        &self,
        spec_json_path: Option<PathBuf>,
        strict_override: bool,
        gate: Option<SignalGateName>,
        workspace_root: Option<PathBuf>,
    ) -> SignalCommandOutput {
        let strictness = match self.resolve(
            if strict_override {
                SignalStrictOverride::ForceStrict
            } else {
                SignalStrictOverride::UseGateMatrix
            },
            gate,
            ChainId::SpecAdr,
            workspace_root.clone(),
        ) {
            Ok(value) => value,
            Err(output) => return output,
        };
        self.execute(ResolvedSignalChainCommand::CheckSpecAdr {
            spec_json_path,
            strictness,
            workspace_root,
        })
    }
    fn calc_catalog_spec(&self) -> SignalCommandOutput {
        self.execute(ResolvedSignalChainCommand::CalcCatalogSpec)
    }
    fn check_catalog_spec(
        &self,
        strict_override: bool,
        gate: Option<SignalGateName>,
        workspace_root: Option<PathBuf>,
    ) -> SignalCommandOutput {
        let strictness = match self.resolve(
            if strict_override {
                SignalStrictOverride::ForceStrict
            } else {
                SignalStrictOverride::UseGateMatrix
            },
            gate,
            ChainId::CatalogSpec,
            workspace_root.clone(),
        ) {
            Ok(value) => value,
            Err(output) => return output,
        };
        self.execute(ResolvedSignalChainCommand::CheckCatalogSpec { strictness, workspace_root })
    }
    fn calc_impl_catalog(&self) -> SignalCommandOutput {
        self.execute(ResolvedSignalChainCommand::CalcImplCatalog)
    }
    fn check_impl_catalog(
        &self,
        strict_override: bool,
        gate: Option<SignalGateName>,
        workspace_root: Option<PathBuf>,
    ) -> SignalCommandOutput {
        let strictness = match self.resolve(
            if strict_override {
                SignalStrictOverride::ForceStrict
            } else {
                SignalStrictOverride::UseGateMatrix
            },
            gate,
            ChainId::ImplCatalog,
            workspace_root.clone(),
        ) {
            Ok(value) => value,
            Err(output) => return output,
        };
        self.execute(ResolvedSignalChainCommand::CheckImplCatalog { strictness, workspace_root })
    }
    fn check_gate(
        &self,
        project_root: Option<PathBuf>,
        spec_json_path: Option<PathBuf>,
        gate: SignalGateName,
        workspace_root: Option<PathBuf>,
    ) -> SignalCommandOutput {
        self.check_gate(project_root, spec_json_path, gate, workspace_root)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    use domain::{ChainGateEntry, SignalGateMatrix, verify::VerifyFinding};

    use super::*;

    fn test_track_id() -> domain::TrackId {
        domain::TrackId::try_new("test-track".to_owned()).unwrap()
    }

    fn resolved_spec_path(spec_json_path: Option<&Path>) -> PathBuf {
        spec_json_path
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("workspace/spec.json"))
    }

    struct FixedActiveTrackResolver;

    impl SignalActiveTrackResolverPort for FixedActiveTrackResolver {
        fn resolve_active_track(
            &self,
            _workspace_root: Option<&Path>,
        ) -> Result<domain::TrackId, SignalCommandPortError> {
            Ok(test_track_id())
        }
    }

    struct DefaultSpecPathResolver;

    impl SignalSpecPathResolverPort for DefaultSpecPathResolver {
        fn resolve_spec_path(
            &self,
            _workspace_root: Option<&Path>,
            spec_json_path: Option<&Path>,
        ) -> Result<PathBuf, SignalCommandPortError> {
            Ok(resolved_spec_path(spec_json_path))
        }
    }

    fn interactor(
        port: Arc<dyn SignalCommandPort>,
        gate_config: Arc<dyn SignalGateConfigPort>,
    ) -> SignalCommandInteractor {
        SignalCommandInteractor::new(
            port,
            Arc::new(FixedActiveTrackResolver),
            Arc::new(DefaultSpecPathResolver),
            gate_config,
        )
    }

    struct RecordingPort(Mutex<Vec<ResolvedSignalChainCommand>>);

    impl SignalCommandPort for RecordingPort {
        fn execute(
            &self,
            command: ResolvedSignalChainCommand,
        ) -> Result<SignalChainExecutionReport, SignalCommandPortError> {
            self.0.lock().unwrap().push(command);
            Ok(SignalChainExecutionReport {
                outcome: VerifyOutcome::pass(),
                stdout: Some("port output".to_owned()),
                stderr: None,
            })
        }
    }

    struct MatrixConfig;

    impl SignalGateConfigPort for MatrixConfig {
        fn load(
            &self,
            _workspace_root: Option<&Path>,
        ) -> Result<domain::SignalGateMatrix, SignalGateConfigError> {
            let interim =
                ChainGateEntry { commit_gate: Strictness::Interim, merge_gate: Strictness::Strict };
            Ok(SignalGateMatrix {
                adr_user: interim.clone(),
                spec_adr: interim.clone(),
                catalog_spec: interim.clone(),
                impl_catalog: interim,
            })
        }
    }

    struct CountingMatrixConfig {
        loads: AtomicUsize,
    }

    impl SignalGateConfigPort for CountingMatrixConfig {
        fn load(
            &self,
            workspace_root: Option<&Path>,
        ) -> Result<domain::SignalGateMatrix, SignalGateConfigError> {
            self.loads.fetch_add(1, Ordering::SeqCst);
            MatrixConfig.load(workspace_root)
        }
    }

    struct MissingInputReportPort(Mutex<Vec<ResolvedSignalChainCommand>>);

    impl SignalCommandPort for MissingInputReportPort {
        fn execute(
            &self,
            command: ResolvedSignalChainCommand,
        ) -> Result<SignalChainExecutionReport, SignalCommandPortError> {
            self.0.lock().unwrap().push(command.clone());
            let outcome = match command {
                ResolvedSignalChainCommand::CheckSpecAdr { .. } => VerifyOutcome::from_findings(
                    vec![VerifyFinding::error("required spec signal document is unavailable")],
                ),
                _ => VerifyOutcome::pass(),
            };
            Ok(SignalChainExecutionReport {
                outcome,
                stdout: Some("chain report".to_owned()),
                stderr: None,
            })
        }
    }

    #[test]
    fn test_signal_command_interactor_check_adr_user_resolves_gate_then_delegates() {
        let port = Arc::new(RecordingPort(Mutex::new(Vec::new())));
        let interactor = interactor(port.clone(), Arc::new(MatrixConfig));
        let project_root = PathBuf::from("workspace");
        let output = interactor.check_adr_user(
            project_root.clone(),
            false,
            Some(SignalGateName::Commit),
            None,
        );
        assert_eq!(output.stdout.as_deref(), Some("port output"));
        assert_eq!(output.stderr, None);
        assert_eq!(output.exit_code, 0);
        assert_eq!(
            port.0.lock().unwrap().as_slice(),
            [ResolvedSignalChainCommand::CheckAdrUser {
                project_root: SignalRootSelection::Supplied(project_root),
                strictness: Strictness::Interim,
            }]
        );
    }

    #[test]
    fn test_signal_command_interactor_force_strict_bypasses_matrix_resolution() {
        let port = Arc::new(RecordingPort(Mutex::new(Vec::new())));
        let interactor = interactor(port.clone(), Arc::new(MatrixConfig));
        let _ = interactor.check_catalog_spec(true, None, None);
        assert_eq!(
            port.0.lock().unwrap().as_slice(),
            [ResolvedSignalChainCommand::CheckCatalogSpec {
                strictness: Strictness::Strict,
                workspace_root: None,
            }]
        );
    }

    #[test]
    fn test_signal_command_closed_variants_preserve_every_payload() {
        let project_root = PathBuf::from("project");
        let spec_json_path = PathBuf::from("project/spec.json");
        let workspace_root = PathBuf::from("workspace");
        let commands = [
            SignalCommand::CalcAdrUser { project_root: project_root.clone() },
            SignalCommand::CheckAdrUser {
                project_root: project_root.clone(),
                strict_override: SignalStrictOverride::UseGateMatrix,
                gate: Some(SignalGateName::Commit),
                workspace_root: Some(workspace_root.clone()),
            },
            SignalCommand::CalcSpecAdr {
                spec_json_path: Some(spec_json_path.clone()),
                workspace_root: Some(workspace_root.clone()),
            },
            SignalCommand::CheckSpecAdr {
                spec_json_path: Some(spec_json_path.clone()),
                strict_override: SignalStrictOverride::ForceStrict,
                gate: Some(SignalGateName::Merge),
                workspace_root: Some(workspace_root.clone()),
            },
            SignalCommand::CalcCatalogSpec,
            SignalCommand::CheckCatalogSpec {
                strict_override: SignalStrictOverride::UseGateMatrix,
                gate: None,
                workspace_root: Some(workspace_root.clone()),
            },
            SignalCommand::CalcImplCatalog,
            SignalCommand::CheckImplCatalog {
                strict_override: SignalStrictOverride::ForceStrict,
                gate: Some(SignalGateName::Commit),
                workspace_root: Some(workspace_root.clone()),
            },
            SignalCommand::CheckGate {
                project_root: Some(project_root.clone()),
                spec_json_path: Some(spec_json_path.clone()),
                gate: SignalGateName::Merge,
                workspace_root: Some(workspace_root.clone()),
            },
        ];

        assert!(matches!(
            &commands[0],
            SignalCommand::CalcAdrUser { project_root: value } if value == &project_root
        ));
        assert!(matches!(
            &commands[1],
            SignalCommand::CheckAdrUser {
                project_root: value,
                strict_override: SignalStrictOverride::UseGateMatrix,
                gate: Some(SignalGateName::Commit),
                workspace_root: Some(root),
            } if value == &project_root && root == &workspace_root
        ));
        assert!(matches!(
            &commands[2],
            SignalCommand::CalcSpecAdr {
                spec_json_path: Some(path),
                workspace_root: Some(root),
            } if path == &spec_json_path && root == &workspace_root
        ));
        assert!(matches!(
            &commands[3],
            SignalCommand::CheckSpecAdr {
                spec_json_path: Some(path),
                strict_override: SignalStrictOverride::ForceStrict,
                gate: Some(SignalGateName::Merge),
                workspace_root: Some(root),
            } if path == &spec_json_path && root == &workspace_root
        ));
        assert!(matches!(&commands[4], SignalCommand::CalcCatalogSpec));
        assert!(matches!(
            &commands[5],
            SignalCommand::CheckCatalogSpec {
                strict_override: SignalStrictOverride::UseGateMatrix,
                gate: None,
                workspace_root: Some(root),
            } if root == &workspace_root
        ));
        assert!(matches!(&commands[6], SignalCommand::CalcImplCatalog));
        assert!(matches!(
            &commands[7],
            SignalCommand::CheckImplCatalog {
                strict_override: SignalStrictOverride::ForceStrict,
                gate: Some(SignalGateName::Commit),
                workspace_root: Some(root),
            } if root == &workspace_root
        ));
        assert!(matches!(
            &commands[8],
            SignalCommand::CheckGate {
                project_root: Some(project),
                spec_json_path: Some(spec),
                gate: SignalGateName::Merge,
                workspace_root: Some(root),
            } if project == &project_root && spec == &spec_json_path && root == &workspace_root
        ));
    }

    #[test]
    fn test_signal_command_interactor_all_operations_delegate_resolved_payloads() {
        let port = Arc::new(RecordingPort(Mutex::new(Vec::new())));
        let interactor = interactor(port.clone(), Arc::new(MatrixConfig));
        let project_root = PathBuf::from("project");
        let spec_json_path = PathBuf::from("project/spec.json");
        let workspace_root = PathBuf::from("workspace");

        let _ = interactor.calc_adr_user(project_root.clone());
        let _ = interactor.check_adr_user(
            project_root.clone(),
            false,
            Some(SignalGateName::Commit),
            Some(workspace_root.clone()),
        );
        let _ =
            interactor.calc_spec_adr(Some(spec_json_path.clone()), Some(workspace_root.clone()));
        let _ = interactor.check_spec_adr(
            Some(spec_json_path.clone()),
            true,
            Some(SignalGateName::Merge),
            Some(workspace_root.clone()),
        );
        let _ = interactor.calc_catalog_spec();
        let _ = interactor.check_catalog_spec(
            false,
            Some(SignalGateName::Merge),
            Some(workspace_root.clone()),
        );
        let _ = interactor.calc_impl_catalog();
        let _ = interactor.check_impl_catalog(
            false,
            Some(SignalGateName::Commit),
            Some(workspace_root.clone()),
        );
        let gate_output = interactor.check_gate(
            Some(project_root.clone()),
            Some(spec_json_path.clone()),
            SignalGateName::Merge,
            Some(workspace_root.clone()),
        );

        assert_eq!(gate_output.stderr, None);
        assert_eq!(gate_output.exit_code, 0);
        assert_eq!(
            gate_output.stdout.as_deref(),
            Some(
                "--- signal check --gate merge ---\nport output\nport output\nport output\nport output\n--- signal check --gate merge PASSED ---"
            )
        );
        assert_eq!(
            *port.0.lock().unwrap(),
            vec![
                ResolvedSignalChainCommand::CalcAdrUser { project_root: project_root.clone() },
                ResolvedSignalChainCommand::CheckAdrUser {
                    project_root: SignalRootSelection::Supplied(project_root.clone()),
                    strictness: Strictness::Interim,
                },
                ResolvedSignalChainCommand::CalcSpecAdr {
                    spec_json_path: Some(spec_json_path.clone()),
                    workspace_root: Some(workspace_root.clone()),
                },
                ResolvedSignalChainCommand::CheckSpecAdr {
                    spec_json_path: Some(spec_json_path.clone()),
                    strictness: Strictness::Strict,
                    workspace_root: Some(workspace_root.clone()),
                },
                ResolvedSignalChainCommand::CalcCatalogSpec,
                ResolvedSignalChainCommand::CheckCatalogSpec {
                    strictness: Strictness::Strict,
                    workspace_root: Some(workspace_root.clone()),
                },
                ResolvedSignalChainCommand::CalcImplCatalog,
                ResolvedSignalChainCommand::CheckImplCatalog {
                    strictness: Strictness::Interim,
                    workspace_root: Some(workspace_root.clone()),
                },
                ResolvedSignalChainCommand::CheckAdrUser {
                    project_root: SignalRootSelection::Supplied(project_root),
                    strictness: Strictness::Strict,
                },
                ResolvedSignalChainCommand::CheckSpecAdr {
                    spec_json_path: Some(spec_json_path),
                    strictness: Strictness::Strict,
                    workspace_root: Some(workspace_root.clone()),
                },
                ResolvedSignalChainCommand::CheckCatalogSpec {
                    strictness: Strictness::Strict,
                    workspace_root: Some(workspace_root.clone()),
                },
                ResolvedSignalChainCommand::CheckImplCatalog {
                    strictness: Strictness::Strict,
                    workspace_root: Some(workspace_root),
                },
            ]
        );
    }

    struct FailingPort;

    impl SignalCommandPort for FailingPort {
        fn execute(
            &self,
            _command: ResolvedSignalChainCommand,
        ) -> Result<SignalChainExecutionReport, SignalCommandPortError> {
            Err(SignalCommandPortError::SpecPathResolution {
                reason: SignalFailureReason::new("active track is unavailable".to_owned()),
            })
        }
    }

    struct RepositoryDiscoveryFailingPort;

    impl SignalCommandPort for RepositoryDiscoveryFailingPort {
        fn execute(
            &self,
            _command: ResolvedSignalChainCommand,
        ) -> Result<SignalChainExecutionReport, SignalCommandPortError> {
            Err(SignalCommandPortError::RepositoryDiscovery {
                reason: SignalFailureReason::new("not a work tree".to_owned()),
            })
        }
    }

    struct FailingReportPort;

    impl SignalCommandPort for FailingReportPort {
        fn execute(
            &self,
            _command: ResolvedSignalChainCommand,
        ) -> Result<SignalChainExecutionReport, SignalCommandPortError> {
            Ok(SignalChainExecutionReport {
                outcome: VerifyOutcome::from_findings(vec![VerifyFinding::error("chain blocked")]),
                stdout: Some("chain report".to_owned()),
                stderr: None,
            })
        }
    }

    struct FailingMatrixConfig;

    impl SignalGateConfigPort for FailingMatrixConfig {
        fn load(
            &self,
            _workspace_root: Option<&Path>,
        ) -> Result<domain::SignalGateMatrix, SignalGateConfigError> {
            Err(SignalGateConfigError::ConfigurationInvalid {
                path: PathBuf::from("workspace/.harness/config/signal-gates.json"),
                reason: SignalFailureReason::new("invalid strictness value".to_owned()),
            })
        }
    }

    struct RepositoryDiscoveryFailingMatrixConfig;

    impl SignalGateConfigPort for RepositoryDiscoveryFailingMatrixConfig {
        fn load(
            &self,
            _workspace_root: Option<&Path>,
        ) -> Result<domain::SignalGateMatrix, SignalGateConfigError> {
            Err(SignalGateConfigError::RepositoryDiscovery {
                reason: SignalFailureReason::new("not a work tree".to_owned()),
            })
        }
    }

    #[test]
    fn test_signal_failure_reason_with_text_preserves_value() {
        let reason = SignalFailureReason::new("adapter diagnostic".to_owned());

        assert_eq!(reason.as_str(), "adapter diagnostic");
        assert_eq!(reason.to_string(), "adapter diagnostic");
    }

    #[test]
    fn test_signal_command_interactor_error_categories_preserve_actionable_diagnostics() {
        let port_failure = interactor(Arc::new(FailingPort), Arc::new(MatrixConfig))
            .calc_adr_user(PathBuf::from("project"));
        assert_eq!(port_failure.stdout, None);
        assert_eq!(
            port_failure.stderr.as_deref(),
            Some(
                "[BLOCKED] cannot resolve spec.json from active track: active track is unavailable; pass --workspace-root or --spec-json explicitly"
            )
        );
        assert_eq!(port_failure.exit_code, 1);

        let repository_failure =
            SignalCommandInteractor::command_error(SignalCommandPortError::RepositoryDiscovery {
                reason: SignalFailureReason::new("not a work tree".to_owned()),
            });
        assert_eq!(
            repository_failure.stderr.as_deref(),
            Some(
                "[BLOCKED] cannot discover git repository: not a work tree; pass --workspace-root or --spec-json explicitly"
            )
        );

        let branch_failure =
            SignalCommandInteractor::command_error(SignalCommandPortError::BranchReadFailure {
                reason: SignalFailureReason::new("detached HEAD".to_owned()),
            });
        assert_eq!(
            branch_failure.stderr.as_deref(),
            Some("[ERROR] signal calc-impl-catalog: cannot read current branch: detached HEAD")
        );

        let missing_branch_failure =
            SignalCommandInteractor::command_error(SignalCommandPortError::BranchAbsent);
        assert_eq!(
            missing_branch_failure.stderr.as_deref(),
            Some("[ERROR] signal calc-impl-catalog: cannot read current branch")
        );

        let persistence_failure =
            SignalCommandInteractor::command_error(SignalCommandPortError::Persistence {
                reason: SignalFailureReason::new("write denied".to_owned()),
            });
        assert_eq!(persistence_failure.stderr.as_deref(), Some("write denied"));

        let execution_failure =
            SignalCommandInteractor::command_error(SignalCommandPortError::Execution {
                reason: SignalFailureReason::new(
                    "signal check-catalog-spec: cannot discover git repository: not a work tree"
                        .to_owned(),
                ),
            });
        assert_eq!(
            execution_failure.stderr.as_deref(),
            Some("signal check-catalog-spec: cannot discover git repository: not a work tree")
        );

        let config_failure = interactor(
            Arc::new(RecordingPort(Mutex::new(Vec::new()))),
            Arc::new(FailingMatrixConfig),
        )
        .check_adr_user(PathBuf::from("project"), false, None, None);
        assert_eq!(config_failure.stdout, None);
        assert_eq!(
            config_failure.stderr.as_deref(),
            Some(
                "[ERROR] failed to load signal-gates config from workspace/.harness/config/signal-gates.json: invalid strictness value"
            )
        );
        assert_eq!(config_failure.exit_code, 1);

        let gate_config_failure = interactor(
            Arc::new(RecordingPort(Mutex::new(Vec::new()))),
            Arc::new(FailingMatrixConfig),
        )
        .check_gate(None, None, SignalGateName::Commit, None);
        assert_eq!(gate_config_failure.stdout, None);
        assert_eq!(
            gate_config_failure.stderr.as_deref(),
            Some(
                "[ERROR] failed to load signal-gates config from workspace/.harness/config/signal-gates.json: invalid strictness value"
            )
        );
        assert_eq!(gate_config_failure.exit_code, 1);

        let repository_gate_config_failure = interactor(
            Arc::new(RecordingPort(Mutex::new(Vec::new()))),
            Arc::new(RepositoryDiscoveryFailingMatrixConfig),
        )
        .check_gate(None, None, SignalGateName::Commit, None);
        assert_eq!(repository_gate_config_failure.stdout, None);
        assert_eq!(
            repository_gate_config_failure.stderr.as_deref(),
            Some("cannot discover git repository: not a work tree")
        );
        assert_eq!(repository_gate_config_failure.exit_code, 1);
    }

    #[test]
    fn test_signal_command_interactor_command_root_discovery_preserves_catalogue_command_output() {
        let calc_catalogue =
            interactor(Arc::new(RepositoryDiscoveryFailingPort), Arc::new(MatrixConfig))
                .calc_catalog_spec();
        assert_eq!(calc_catalogue.stdout, None);
        assert_eq!(
            calc_catalogue.stderr.as_deref(),
            Some("[ERROR] signal calc-catalog-spec: cannot discover git repo: not a work tree")
        );
        assert_eq!(calc_catalogue.exit_code, 1);

        let calc_impl =
            interactor(Arc::new(RepositoryDiscoveryFailingPort), Arc::new(MatrixConfig))
                .calc_impl_catalog();
        assert_eq!(calc_impl.stdout, None);
        assert_eq!(
            calc_impl.stderr.as_deref(),
            Some("[ERROR] signal calc-impl-catalog: cannot discover git repo: not a work tree")
        );
        assert_eq!(calc_impl.exit_code, 1);

        let check_catalogue =
            interactor(Arc::new(RepositoryDiscoveryFailingPort), Arc::new(MatrixConfig))
                .check_catalog_spec(true, None, None);
        assert_eq!(check_catalogue.stdout, None);
        assert_eq!(
            check_catalogue.stderr.as_deref(),
            Some("[ERROR] signal check-catalog-spec: cannot discover git repo: not a work tree")
        );
        assert_eq!(check_catalogue.exit_code, 1);

        let check_impl =
            interactor(Arc::new(RepositoryDiscoveryFailingPort), Arc::new(MatrixConfig))
                .check_impl_catalog(true, None, None);
        assert_eq!(check_impl.stdout, None);
        assert_eq!(
            check_impl.stderr.as_deref(),
            Some("[ERROR] signal check-impl-catalog: cannot discover git repo: not a work tree")
        );
        assert_eq!(check_impl.exit_code, 1);
    }

    struct ErrorOnSecondChainPort {
        commands: Mutex<Vec<ResolvedSignalChainCommand>>,
    }

    struct PreflightFailingPort {
        commands: Mutex<Vec<ResolvedSignalChainCommand>>,
    }

    impl SignalCommandPort for PreflightFailingPort {
        fn execute(
            &self,
            command: ResolvedSignalChainCommand,
        ) -> Result<SignalChainExecutionReport, SignalCommandPortError> {
            self.commands.lock().unwrap().push(command.clone());
            Ok(SignalChainExecutionReport {
                outcome: VerifyOutcome::pass(),
                stdout: Some("unexpected chain execution".to_owned()),
                stderr: None,
            })
        }
    }

    struct PreflightFailingActiveTrackResolver {
        active_track_roots: Arc<Mutex<Vec<Option<PathBuf>>>>,
    }

    impl SignalActiveTrackResolverPort for PreflightFailingActiveTrackResolver {
        fn resolve_active_track(
            &self,
            workspace_root: Option<&Path>,
        ) -> Result<domain::TrackId, SignalCommandPortError> {
            self.active_track_roots.lock().unwrap().push(workspace_root.map(Path::to_path_buf));
            Err(SignalCommandPortError::Execution {
                reason: SignalFailureReason::new(
                    "cannot resolve active track ID: branch is not a track branch".to_owned(),
                ),
            })
        }
    }

    #[test]
    fn test_signal_command_interactor_gate_preflight_uses_supplied_workspace_and_short_circuits() {
        let port = Arc::new(PreflightFailingPort { commands: Mutex::new(Vec::new()) });
        let active_track_roots = Arc::new(Mutex::new(Vec::new()));
        let output = SignalCommandInteractor::new(
            port.clone(),
            Arc::new(PreflightFailingActiveTrackResolver {
                active_track_roots: active_track_roots.clone(),
            }),
            Arc::new(DefaultSpecPathResolver),
            Arc::new(MatrixConfig),
        )
        .check_gate(
            Some(PathBuf::from("explicit/project")),
            Some(PathBuf::from("explicit/spec.json")),
            SignalGateName::Commit,
            Some(PathBuf::from("explicit/workspace")),
        );

        assert_eq!(output.stdout, None);
        assert_eq!(
            output.stderr.as_deref(),
            Some(
                "[BLOCKED] signal check --gate Commit: cannot resolve active track ID: branch is not a track branch"
            )
        );
        assert_eq!(output.exit_code, 1);
        assert!(port.commands.lock().unwrap().is_empty());
        assert_eq!(
            active_track_roots.lock().unwrap().as_slice(),
            [Some(PathBuf::from("explicit/workspace"))]
        );
    }

    struct SpecResolutionFailingPort {}

    impl SignalCommandPort for SpecResolutionFailingPort {
        fn execute(
            &self,
            _command: ResolvedSignalChainCommand,
        ) -> Result<SignalChainExecutionReport, SignalCommandPortError> {
            Err(SignalCommandPortError::Execution {
                reason: SignalFailureReason::new("unexpected chain".to_owned()),
            })
        }
    }

    struct SpecResolutionFailingResolver {
        events: Arc<Mutex<Vec<&'static str>>>,
    }

    impl SignalSpecPathResolverPort for SpecResolutionFailingResolver {
        fn resolve_spec_path(
            &self,
            _workspace_root: Option<&Path>,
            _spec_json_path: Option<&Path>,
        ) -> Result<PathBuf, SignalCommandPortError> {
            self.events.lock().unwrap().push("spec-path");
            Err(SignalCommandPortError::SpecPathResolution {
                reason: SignalFailureReason::new("branch is not a track branch".to_owned()),
            })
        }
    }

    #[test]
    fn test_signal_command_interactor_gate_missing_spec_path_precedes_active_track_preflight() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let output = SignalCommandInteractor::new(
            Arc::new(SpecResolutionFailingPort {}),
            Arc::new(FixedActiveTrackResolver),
            Arc::new(SpecResolutionFailingResolver { events: events.clone() }),
            Arc::new(MatrixConfig),
        )
        .check_gate(None, None, SignalGateName::Commit, Some(PathBuf::from("workspace")));

        assert_eq!(output.stdout, None);
        assert_eq!(
            output.stderr.as_deref(),
            Some(
                "[BLOCKED] cannot resolve spec.json from active track: branch is not a track branch; pass --workspace-root or --spec-json explicitly"
            )
        );
        assert_eq!(output.exit_code, 1);
        assert_eq!(events.lock().unwrap().as_slice(), ["spec-path"]);
    }

    impl SignalCommandPort for ErrorOnSecondChainPort {
        fn execute(
            &self,
            command: ResolvedSignalChainCommand,
        ) -> Result<SignalChainExecutionReport, SignalCommandPortError> {
            let mut commands = self.commands.lock().unwrap();
            commands.push(command);
            if commands.len() == 2 {
                return Err(SignalCommandPortError::Execution {
                    reason: SignalFailureReason::new("chain executor unavailable".to_owned()),
                });
            }
            Ok(SignalChainExecutionReport {
                outcome: VerifyOutcome::pass(),
                stdout: Some("completed chain".to_owned()),
                stderr: None,
            })
        }
    }

    #[test]
    fn test_signal_command_interactor_check_gate_dependency_error_stops_later_chains() {
        let port = Arc::new(ErrorOnSecondChainPort { commands: Mutex::new(Vec::new()) });
        let output = interactor(port.clone(), Arc::new(MatrixConfig)).check_gate(
            Some(PathBuf::from("project")),
            Some(PathBuf::from("project/spec.json")),
            SignalGateName::Commit,
            Some(PathBuf::from("workspace")),
        );

        assert_eq!(output.stdout, None);
        assert_eq!(output.stderr.as_deref(), Some("chain executor unavailable"));
        assert_eq!(output.exit_code, 1);
        assert_eq!(port.commands.lock().unwrap().len(), 2);
    }

    #[test]
    fn test_signal_command_interactor_check_gate_with_failing_chain_fails_closed() {
        let output = interactor(Arc::new(FailingReportPort), Arc::new(MatrixConfig)).check_gate(
            Some(PathBuf::from("project")),
            Some(PathBuf::from("project/spec.json")),
            SignalGateName::Merge,
            Some(PathBuf::from("workspace")),
        );

        assert_eq!(output.stderr, None);
        assert_eq!(output.exit_code, 1);
        assert_eq!(
            output.stdout.as_deref(),
            Some(
                "--- signal check --gate merge ---\nchain report\nchain report\nchain report\nchain report\n--- signal check --gate merge FAILED ---"
            )
        );
    }

    #[test]
    fn test_signal_command_interactor_check_gate_with_failing_spec_chain_fails_closed() {
        let port = Arc::new(MissingInputReportPort(Mutex::new(Vec::new())));
        let output = interactor(port.clone(), Arc::new(MatrixConfig)).check_gate(
            Some(PathBuf::from("project")),
            None,
            SignalGateName::Commit,
            Some(PathBuf::from("workspace")),
        );

        assert_eq!(output.exit_code, 1);
        assert_eq!(output.stderr, None);
        assert_eq!(
            output.stdout.as_deref(),
            Some(
                "--- signal check --gate commit ---\nchain report\nchain report\nchain report\nchain report\n--- signal check --gate commit FAILED ---"
            )
        );
        let commands = port.0.lock().unwrap();
        assert!(matches!(
            commands.as_slice(),
            [
                ResolvedSignalChainCommand::CheckAdrUser { .. },
                ResolvedSignalChainCommand::CheckSpecAdr { spec_json_path: Some(_), .. },
                ResolvedSignalChainCommand::CheckCatalogSpec { .. },
                ResolvedSignalChainCommand::CheckImplCatalog { .. },
            ]
        ));
    }

    #[test]
    fn test_signal_command_interactor_check_gate_loads_mode_once_then_dispatches_all_chains() {
        let port = Arc::new(RecordingPort(Mutex::new(Vec::new())));
        let config = Arc::new(CountingMatrixConfig { loads: AtomicUsize::new(0) });
        let interactor = interactor(port.clone(), config.clone());

        let output = interactor.check_gate(
            Some(PathBuf::from("project")),
            Some(PathBuf::from("project/spec.json")),
            SignalGateName::Commit,
            Some(PathBuf::from("workspace")),
        );

        assert_eq!(output.exit_code, 0);
        assert_eq!(config.loads.load(Ordering::SeqCst), 1);
        let commands = port.0.lock().unwrap();
        assert_eq!(commands.len(), 4);
        assert!(matches!(
            commands.as_slice(),
            [
                ResolvedSignalChainCommand::CheckAdrUser { strictness: Strictness::Interim, .. },
                ResolvedSignalChainCommand::CheckSpecAdr { .. },
                ResolvedSignalChainCommand::CheckCatalogSpec { .. },
                ResolvedSignalChainCommand::CheckImplCatalog { .. },
            ]
        ));
    }
}
