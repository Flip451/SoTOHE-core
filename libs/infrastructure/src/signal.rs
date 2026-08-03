//! System secondary adapters for the typed Signal command boundary.

use std::path::{Path, PathBuf};

use domain::verify::{VerifyFinding, VerifyOutcome};
use usecase::signal::SignalLayerReader as _;
use usecase::signal_service::{
    ResolvedSignalChainCommand, SignalActiveTrackResolverPort, SignalChainExecutionReport,
    SignalCommandPort, SignalCommandPortError, SignalFailureReason, SignalGateConfigError,
    SignalGateConfigPort, SignalRootSelection, SignalSpecPathResolverPort,
};

use crate::git_cli::SystemGitRepo;
use crate::signal_layer_reader::LocalSignalLayerReaderAdapter;
use crate::verify::signal_gates_config::SignalGatesConfigError;

struct BindingSignalLayerReader {
    inner: LocalSignalLayerReaderAdapter,
    bindings: Vec<crate::verify::tddd_layers::TdddLayerBinding>,
}

impl usecase::signal::SignalLayerReader for BindingSignalLayerReader {
    fn active_track_id(&self) -> Result<domain::TrackId, usecase::signal::SignalLayerReaderError> {
        self.inner.active_track_id()
    }

    fn enabled_layers(
        &self,
        _track_id: domain::TrackId,
    ) -> Result<Vec<domain::tddd::LayerId>, usecase::signal::SignalLayerReaderError> {
        self.bindings
            .iter()
            .map(|binding| {
                domain::tddd::LayerId::try_new(binding.layer_id().to_owned())
                    .map_err(|_| usecase::signal::SignalLayerReaderError::Io)
            })
            .collect()
    }

    fn catalogue_bytes(
        &self,
        track_id: domain::TrackId,
        layer: domain::tddd::LayerId,
    ) -> Result<Option<Vec<u8>>, usecase::signal::SignalLayerReaderError> {
        self.inner.catalogue_bytes(track_id, layer)
    }
}

/// System adapter which performs resolved Signal operations.
pub struct SystemSignalCommandAdapter {
    #[cfg(feature = "test-helpers")]
    impl_catalog_test_context: Option<ImplCatalogTestContext>,
}

#[cfg(feature = "test-helpers")]
struct ImplCatalogTestContext {
    workspace_root: PathBuf,
    track_id: domain::TrackId,
    launch_observer: crate::tddd::type_signals_evaluator::RustdocLaunchObserver,
}

impl SystemSignalCommandAdapter {
    /// Creates a system-backed Signal execution adapter.
    #[must_use]
    pub fn new() -> Self {
        Self {
            #[cfg(feature = "test-helpers")]
            impl_catalog_test_context: None,
        }
    }

    /// Creates an adapter for an isolated impl-catalog test workspace.
    #[cfg(feature = "test-helpers")]
    #[must_use]
    pub fn with_test_context(
        workspace_root: PathBuf,
        track_id: domain::TrackId,
        launch_observer: crate::tddd::type_signals_evaluator::RustdocLaunchObserver,
    ) -> Self {
        Self {
            impl_catalog_test_context: Some(ImplCatalogTestContext {
                workspace_root,
                track_id,
                launch_observer,
            }),
        }
    }

    fn report(label: &str, outcome: VerifyOutcome) -> SignalChainExecutionReport {
        let mut lines = vec![format!("--- {label} ---")];
        if outcome.findings().is_empty() {
            lines.push("[OK] All checks passed.".to_owned());
        } else {
            lines.extend(outcome.findings().iter().map(ToString::to_string));
        }
        lines.push(format!(
            "--- {label} {} ---",
            if outcome.has_errors() { "FAILED" } else { "PASSED" }
        ));
        SignalChainExecutionReport { outcome, stdout: Some(lines.join("\n")), stderr: None }
    }

    fn root(workspace_root: Option<PathBuf>) -> Result<PathBuf, SignalCommandPortError> {
        match workspace_root {
            Some(root) => Ok(root),
            None => Self::discovered_root(SystemGitRepo::discover()),
        }
    }

    fn command_root(workspace_root: Option<PathBuf>) -> Result<PathBuf, SignalCommandPortError> {
        Self::root(workspace_root)
    }

    fn discovered_root<E>(
        repository: Result<SystemGitRepo, E>,
    ) -> Result<PathBuf, SignalCommandPortError>
    where
        E: std::fmt::Display,
    {
        repository.map(|repo| repo.root().to_path_buf()).map_err(Self::repository_discovery_error)
    }

    fn repository_discovery_error(error: impl std::fmt::Display) -> SignalCommandPortError {
        SignalCommandPortError::RepositoryDiscovery {
            reason: SignalFailureReason::new(error.to_string()),
        }
    }

    fn active_track_preflight_error(error: impl std::fmt::Display) -> SignalCommandPortError {
        SignalCommandPortError::Execution {
            reason: SignalFailureReason::new(format!("cannot resolve active track ID: {error}")),
        }
    }

    fn resolve_current_branch<E>(
        current_branch: Result<Option<String>, E>,
    ) -> Result<String, SignalCommandPortError>
    where
        E: std::fmt::Display,
    {
        current_branch
            .map_err(|error| SignalCommandPortError::BranchReadFailure {
                reason: SignalFailureReason::new(error.to_string()),
            })?
            .ok_or(SignalCommandPortError::BranchAbsent)
    }

    fn tddd_load_error(
        error: crate::verify::tddd_layers::LoadTdddLayersError,
    ) -> SignalCommandPortError {
        use crate::verify::tddd_layers::LoadTdddLayersError;

        let message = match error {
            LoadTdddLayersError::Io { path, source } => {
                format!("[ERROR] {}: {source}", path.display())
            }
            LoadTdddLayersError::Parse(error) => {
                format!("[ERROR] architecture-rules.json: {error}")
            }
        };
        SignalCommandPortError::Execution { reason: SignalFailureReason::new(message) }
    }

    fn spec_path(
        root: &Path,
        override_path: Option<PathBuf>,
    ) -> Result<PathBuf, SignalCommandPortError> {
        let reader = LocalSignalLayerReaderAdapter::new(root.to_path_buf());
        usecase::signal::resolve_spec_json_path(&reader, root, override_path).map_err(|error| {
            SignalCommandPortError::SpecPathResolution {
                reason: SignalFailureReason::new(error.to_string()),
            }
        })
    }

    fn resolve_spec_path(
        workspace_root: Option<&Path>,
        override_path: Option<&Path>,
    ) -> Result<PathBuf, SignalCommandPortError> {
        if let Some(path) = override_path {
            return Ok(path.to_path_buf());
        }
        let root = match workspace_root {
            Some(root) => root.to_path_buf(),
            None => Self::root(None)?,
        };
        Self::spec_path(&root, None)
    }

    fn run_catalogue_check(
        root: PathBuf,
        strictness: domain::Strictness,
        impl_catalog: bool,
    ) -> Result<VerifyOutcome, SignalCommandPortError> {
        use crate::verify::tddd_layers::load_tddd_layers_from_workspace;
        let bindings = load_tddd_layers_from_workspace(&root)
            .map_err(Self::tddd_load_error)?
            .into_iter()
            .filter(|binding| impl_catalog || binding.catalogue_spec_signal_enabled())
            .collect::<Vec<_>>();
        if bindings.is_empty() {
            return Ok(if impl_catalog {
                VerifyOutcome::from_findings(vec![VerifyFinding::error(
                    "[BLOCKED] signal check-impl-catalog: no TDDD-enabled layers for chain ③ \
                     check — set `tddd.enabled: true` for at least one layer in \
                     architecture-rules.json",
                )])
            } else {
                VerifyOutcome::pass()
            });
        }
        let reader = BindingSignalLayerReader {
            inner: LocalSignalLayerReaderAdapter::new(root.clone()),
            bindings: bindings.clone(),
        };
        let check = move |layer: domain::tddd::LayerId, hash_hex: &str, track_id: &str| {
            let Some(binding) =
                bindings.iter().find(|binding| binding.layer_id() == layer.as_ref())
            else {
                return VerifyOutcome::from_findings(vec![VerifyFinding::error(
                    "signal layer binding was not found",
                )]);
            };
            let path = if impl_catalog {
                crate::verify::tddd_layers::impl_catalog_signals_path(binding, &root, track_id)
            } else {
                crate::verify::tddd_layers::catalogue_spec_signals_path(binding, &root, track_id)
            };
            if impl_catalog {
                crate::verify::spec_states::check_impl_catalog_from_signals_file(
                    &path,
                    hash_hex,
                    strictness == domain::Strictness::Strict,
                )
            } else {
                crate::verify::catalogue_spec_signals::check_catalog_spec_from_signals_file(
                    &path,
                    hash_hex,
                    strictness == domain::Strictness::Strict,
                )
            }
        };
        Ok(if impl_catalog {
            usecase::signal::check_impl_catalog(&reader, check)
        } else {
            usecase::signal::check_catalog_spec(&reader, check)
        })
    }

    fn calc_catalogue(root: PathBuf) -> Result<VerifyOutcome, SignalCommandPortError> {
        use crate::tddd::fs_catalogue_spec_signals_store::FsCatalogueSpecSignalsStore;
        use crate::verify::tddd_layers::{TdddLayerBinding, load_tddd_layers_from_workspace};
        let items_dir = root.join("track/items");
        let bindings = load_tddd_layers_from_workspace(&root)
            .map_err(Self::tddd_load_error)?
            .into_iter()
            .filter(TdddLayerBinding::catalogue_spec_signal_enabled)
            .collect::<Vec<_>>();
        let reader = BindingSignalLayerReader {
            inner: LocalSignalLayerReaderAdapter::new(root.clone()),
            bindings: bindings.clone(),
        };
        let writer = FsCatalogueSpecSignalsStore::new(items_dir.clone());
        Ok(usecase::signal::calc_catalog_spec(&reader, move |layer, _hash_hex, track_id| {
            let Some(binding) =
                bindings.iter().find(|binding| binding.layer_id() == layer.as_ref())
            else {
                return VerifyOutcome::from_findings(vec![VerifyFinding::error(
                    "signal layer binding was not found",
                )]);
            };
            let track_dir = items_dir.join(track_id);
            match crate::tddd::catalogue_spec_signals_refresher::refresh_one_layer(
                &items_dir, &track_dir, track_id, binding, &writer,
            ) {
                Ok(()) => VerifyOutcome::pass(),
                Err(error) => VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                    "signal calc-catalog-spec: layer '{layer}': {error}"
                ))]),
            }
        }))
    }

    fn calc_impl(
        root: PathBuf,
        branch: String,
        executor: std::sync::Arc<dyn usecase::type_signals::TypeSignalsExecutorPort>,
    ) -> Result<VerifyOutcome, SignalCommandPortError> {
        use crate::tddd::feature_declaration_adapter::FsTdddFeatureDeclarationAdapter;
        use crate::tddd::tddd_layer_bindings_adapter::FsTdddLayerBindingsAdapter;
        use crate::verify::tddd_layers::load_tddd_layers_from_workspace;
        use usecase::type_signals::{
            TypeSignalsInteractor, TypeSignalsRequest, TypeSignalsService,
        };
        load_tddd_layers_from_workspace(&root).map_err(Self::tddd_load_error)?;
        let items_dir = root.join("track/items");
        let reader = LocalSignalLayerReaderAdapter::new(root.clone());
        let interactor = TypeSignalsInteractor::new(
            std::sync::Arc::new(FsTdddLayerBindingsAdapter::new()),
            executor,
            std::sync::Arc::new(FsTdddFeatureDeclarationAdapter::new()),
        );
        Ok(usecase::signal::calc_impl_catalog(&reader, move |layer, _hash_hex, track_id| {
            let track_id = match domain::TrackId::try_new(track_id) {
                Ok(value) => value,
                Err(_) => {
                    return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                        "signal calc-impl-catalog: layer '{layer}': invalid track id"
                    ))]);
                }
            };
            let branch = match domain::TrackBranch::try_new(branch.clone()) {
                Ok(value) => value,
                Err(_) => {
                    return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                        "signal calc-impl-catalog: layer '{layer}': invalid track branch"
                    ))]);
                }
            };
            match interactor.run(TypeSignalsRequest {
                items_dir: items_dir.clone(),
                track_id,
                branch,
                workspace_root: root.clone(),
                layer: Some(layer.clone()),
            }) {
                Ok(()) => VerifyOutcome::pass(),
                Err(error) => Self::impl_layer_execution_failure(&layer, error),
            }
        }))
    }

    fn impl_layer_execution_failure(
        layer: &domain::tddd::LayerId,
        error: impl std::fmt::Display,
    ) -> VerifyOutcome {
        VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
            "signal calc-impl-catalog: layer '{layer}': {error}"
        ))])
    }
}

impl Default for SystemSignalCommandAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl SignalActiveTrackResolverPort for SystemSignalCommandAdapter {
    fn resolve_active_track(
        &self,
        workspace_root: Option<&Path>,
    ) -> Result<domain::TrackId, SignalCommandPortError> {
        let root = match workspace_root {
            Some(root) => root.to_path_buf(),
            None => Self::root(None)?,
        };
        LocalSignalLayerReaderAdapter::new(root)
            .active_track_id()
            .map_err(Self::active_track_preflight_error)
    }
}

impl SignalSpecPathResolverPort for SystemSignalCommandAdapter {
    fn resolve_spec_path(
        &self,
        workspace_root: Option<&Path>,
        spec_json_path: Option<&Path>,
    ) -> Result<PathBuf, SignalCommandPortError> {
        Self::resolve_spec_path(workspace_root, spec_json_path)
    }
}

impl SignalCommandPort for SystemSignalCommandAdapter {
    fn execute(
        &self,
        command: ResolvedSignalChainCommand,
    ) -> Result<SignalChainExecutionReport, SignalCommandPortError> {
        match command {
            ResolvedSignalChainCommand::CalcAdrUser { project_root } => Ok(Self::report(
                "signal calc-adr-user",
                crate::verify::adr_signals::execute_verify_adr_signals_with_strict(
                    &project_root,
                    false,
                ),
            )),
            ResolvedSignalChainCommand::CheckAdrUser { project_root, strictness } => {
                let project_root = match project_root {
                    SignalRootSelection::Supplied(project_root) => project_root,
                    SignalRootSelection::Discover => Self::root(None)?,
                };
                Ok(Self::report(
                    "signal check-adr-user",
                    crate::verify::adr_signals::execute_verify_adr_signals_with_strict(
                        &project_root,
                        strictness == domain::Strictness::Strict,
                    ),
                ))
            }
            ResolvedSignalChainCommand::CalcSpecAdr { spec_json_path, workspace_root } => {
                let path =
                    Self::resolve_spec_path(workspace_root.as_deref(), spec_json_path.as_deref())?;
                let service = usecase::spec_adr_signal::SpecAdrSignalInteractor::new(
                    std::sync::Arc::new(crate::spec::FsSpecFileWriterAdapter::new()),
                );
                usecase::spec_adr_signal::SpecAdrSignalService::calc_and_persist(
                    &service,
                    usecase::spec_adr_signal::SpecAdrSignalCommand { spec_json_path: path },
                )
                .map_err(|error| SignalCommandPortError::Persistence {
                    reason: SignalFailureReason::new(format!("[ERROR] {error}")),
                })?;
                Ok(Self::report("signal calc-spec-adr", VerifyOutcome::pass()))
            }
            ResolvedSignalChainCommand::CheckSpecAdr {
                spec_json_path,
                strictness,
                workspace_root,
            } => {
                let path =
                    Self::resolve_spec_path(workspace_root.as_deref(), spec_json_path.as_deref())?;
                let outcome = match crate::verify::trusted_root::resolve_trusted_root(&path) {
                    Ok(trusted_root) => crate::verify::spec_states::verify_from_spec_json(
                        path,
                        strictness == domain::Strictness::Strict,
                        trusted_root,
                    ),
                    Err(error) => VerifyOutcome::from_findings(vec![VerifyFinding::error(
                        format!("cannot resolve trusted_root for {}: {error}", path.display()),
                    )]),
                };
                Ok(Self::report("signal check-spec-adr", outcome))
            }
            ResolvedSignalChainCommand::CalcCatalogSpec => {
                let root = Self::command_root(None)?;
                Ok(Self::report("signal calc-catalog-spec", Self::calc_catalogue(root)?))
            }
            ResolvedSignalChainCommand::CheckCatalogSpec { strictness, workspace_root } => {
                let root = Self::command_root(workspace_root)?;
                Ok(Self::report(
                    "signal check-catalog-spec",
                    Self::run_catalogue_check(root, strictness, false)?,
                ))
            }
            ResolvedSignalChainCommand::CalcImplCatalog => {
                #[cfg(feature = "test-helpers")]
                if let Some(context) = &self.impl_catalog_test_context {
                    use crate::tddd::type_signals_executor_adapter::TypeSignalsExecutorAdapter;

                    return Ok(Self::report(
                        "signal calc-impl-catalog",
                        Self::calc_impl(
                            context.workspace_root.clone(),
                            format!("track/{}", context.track_id),
                            std::sync::Arc::new(
                                TypeSignalsExecutorAdapter::with_rustdoc_launch_observer(
                                    context.launch_observer.clone(),
                                ),
                            ),
                        )?,
                    ));
                }
                let repo = SystemGitRepo::discover().map_err(Self::repository_discovery_error)?;
                let branch = Self::resolve_current_branch(repo.current_branch())?;
                Ok(Self::report(
                    "signal calc-impl-catalog",
                    Self::calc_impl(
                        repo.root().to_path_buf(),
                        branch,
                        std::sync::Arc::new(
                            crate::tddd::type_signals_executor_adapter::TypeSignalsExecutorAdapter::new(),
                        ),
                    )?,
                ))
            }
            ResolvedSignalChainCommand::CheckImplCatalog { strictness, workspace_root } => {
                let root = Self::command_root(workspace_root)?;
                Ok(Self::report(
                    "signal check-impl-catalog",
                    Self::run_catalogue_check(root, strictness, true)?,
                ))
            }
        }
    }
}

/// System adapter that loads the Signal gate matrix without resolving it.
pub struct SystemSignalGateConfigAdapter;

impl SystemSignalGateConfigAdapter {
    /// Creates a system-backed gate-configuration adapter.
    #[must_use]
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self
    }

    fn classify_load_error(
        config_path: PathBuf,
        error: SignalGatesConfigError,
    ) -> SignalGateConfigError {
        match error {
            SignalGatesConfigError::FileNotFound { .. } => {
                SignalGateConfigError::ConfigurationNotFound { path: config_path }
            }
            error => SignalGateConfigError::ConfigurationInvalid {
                path: config_path,
                reason: SignalFailureReason::new(error.to_string()),
            },
        }
    }
}

impl SignalGateConfigPort for SystemSignalGateConfigAdapter {
    fn load(
        &self,
        workspace_root: Option<&Path>,
    ) -> Result<domain::SignalGateMatrix, SignalGateConfigError> {
        let root = match workspace_root {
            Some(root) => root.to_path_buf(),
            None => SystemGitRepo::discover().map(|repo| repo.root().to_path_buf()).map_err(
                |error| SignalGateConfigError::RepositoryDiscovery {
                    reason: SignalFailureReason::new(error.to_string()),
                },
            )?,
        };
        let config_path = root.join(".harness/config/signal-gates.json");
        crate::verify::signal_gates_config::load_signal_gates_config(config_path.clone())
            .map_err(|error| Self::classify_load_error(config_path, error))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use domain::{ChainGateEntry, ChainId, GateKind, SignalGateMatrix, Strictness};
    use serde_json::Value;
    use std::sync::Arc;
    use usecase::signal_service::{SignalCommandInteractor, SignalService};

    const ONE_LAYER_ARCHITECTURE_RULES: &str = r#"{
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
      "tddd": {
        "enabled": true,
        "catalogue_file": "domain-types.json",
        "catalogue_spec_signal": { "enabled": true },
        "schema_export": { "method": "rustdoc", "targets": ["domain"] }
      }
    }
  ]
}"#;

    fn setup_track_workspace(track_id: &str) -> tempfile::TempDir {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path();
        let init = std::process::Command::new("git")
            .args(["init", "--quiet", &format!("--initial-branch=track/{track_id}")])
            .current_dir(root)
            .status()
            .unwrap();
        assert!(init.success(), "test workspace git init must succeed");
        std::fs::create_dir_all(root.join("track/items").join(track_id)).unwrap();
        std::fs::create_dir_all(root.join(".harness/config")).unwrap();
        std::fs::write(root.join("architecture-rules.json"), ONE_LAYER_ARCHITECTURE_RULES).unwrap();
        std::fs::write(
            root.join(".harness/config/signal-gates.json"),
            r#"{
  "$schema_version": 1,
  "commit_gate": {
    "adr_user": "strict", "spec_adr": "strict",
    "catalog_spec": "strict", "impl_catalog": "strict"
  },
  "merge_gate": {
    "adr_user": "strict", "spec_adr": "strict",
    "catalog_spec": "strict", "impl_catalog": "strict"
  }
}"#,
        )
        .unwrap();
        std::fs::write(
            root.join("track/items").join(track_id).join("domain-types.json"),
            r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {},
  "traits": {},
  "functions": {}
}"#,
        )
        .unwrap();
        let commit = std::process::Command::new("git")
            .args(["add", "."])
            .current_dir(root)
            .status()
            .unwrap();
        assert!(commit.success(), "test workspace files must be staged for the initial commit");
        let commit = std::process::Command::new("git")
            .env("GIT_AUTHOR_NAME", "test")
            .env("GIT_AUTHOR_EMAIL", "test@example.invalid")
            .env("GIT_COMMITTER_NAME", "test")
            .env("GIT_COMMITTER_EMAIL", "test@example.invalid")
            .args(["commit", "--quiet", "-m", "initial"])
            .current_dir(root)
            .status()
            .unwrap();
        assert!(commit.success(), "test workspace initial commit must succeed");
        workspace
    }

    #[test]
    fn test_system_signal_command_adapter_implements_typed_port() {
        let adapter = SystemSignalCommandAdapter::new();
        let _port: &dyn SignalCommandPort = &adapter;
        let _active_track_resolver: &dyn SignalActiveTrackResolverPort = &adapter;
        let _spec_path_resolver: &dyn SignalSpecPathResolverPort = &adapter;
    }

    #[test]
    fn test_system_signal_command_adapter_resolves_active_track_from_supplied_workspace() {
        let workspace = setup_track_workspace("supplied-workspace-track");

        let track_id = SystemSignalCommandAdapter::new()
            .resolve_active_track(Some(workspace.path()))
            .expect("a supplied track workspace must resolve without current-directory discovery");

        assert_eq!(track_id.to_string(), "supplied-workspace-track");
    }

    #[test]
    fn test_system_signal_gate_config_adapter_implements_typed_port() {
        let adapter = SystemSignalGateConfigAdapter::new();
        let _port: &dyn SignalGateConfigPort = &adapter;
    }

    #[test]
    fn test_system_signal_command_adapter_non_track_branch_preflight_preserves_aggregate_gate_stderr()
     {
        struct NonTrackBranchPort;

        impl SignalCommandPort for NonTrackBranchPort {
            fn execute(
                &self,
                _command: ResolvedSignalChainCommand,
            ) -> Result<SignalChainExecutionReport, SignalCommandPortError> {
                Ok(SystemSignalCommandAdapter::report("test command", VerifyOutcome::pass()))
            }
        }

        struct NonTrackBranchResolver;

        impl SignalActiveTrackResolverPort for NonTrackBranchResolver {
            fn resolve_active_track(
                &self,
                _workspace_root: Option<&Path>,
            ) -> Result<domain::TrackId, SignalCommandPortError> {
                Err(SystemSignalCommandAdapter::active_track_preflight_error(
                    "branch is not a track branch",
                ))
            }
        }

        struct StaticSpecPathResolver;

        impl SignalSpecPathResolverPort for StaticSpecPathResolver {
            fn resolve_spec_path(
                &self,
                _workspace_root: Option<&Path>,
                _spec_json_path: Option<&Path>,
            ) -> Result<PathBuf, SignalCommandPortError> {
                Ok(PathBuf::from("spec.json"))
            }
        }

        struct StaticGateConfig;

        impl SignalGateConfigPort for StaticGateConfig {
            fn load(
                &self,
                _workspace_root: Option<&Path>,
            ) -> Result<SignalGateMatrix, SignalGateConfigError> {
                let entry = ChainGateEntry {
                    commit_gate: Strictness::Interim,
                    merge_gate: Strictness::Strict,
                };
                Ok(SignalGateMatrix {
                    adr_user: entry.clone(),
                    spec_adr: entry.clone(),
                    catalog_spec: entry.clone(),
                    impl_catalog: entry,
                })
            }
        }

        let interactor = SignalCommandInteractor::new(
            Arc::new(NonTrackBranchPort),
            Arc::new(NonTrackBranchResolver),
            Arc::new(StaticSpecPathResolver),
            Arc::new(StaticGateConfig),
        );
        let output = interactor.check_gate(
            None,
            None,
            usecase::signal_service::SignalGateName::Commit,
            None,
        );

        assert_eq!(output.stdout, None);
        assert_eq!(
            output.stderr.as_deref(),
            Some(
                "[BLOCKED] signal check --gate Commit: cannot resolve active track ID: branch is not a track branch"
            )
        );
        assert_eq!(output.exit_code, 1);
    }

    #[test]
    fn test_system_signal_command_adapter_current_branch_outcomes_use_typed_variants() {
        let read_failure =
            SystemSignalCommandAdapter::resolve_current_branch(Result::<Option<String>, _>::Err(
                "branch read failed",
            ))
            .expect_err("a branch read error must retain its typed variant");
        assert!(matches!(
            read_failure,
            SignalCommandPortError::BranchReadFailure { reason }
                if reason.as_str() == "branch read failed"
        ));

        let branch_absent = SystemSignalCommandAdapter::resolve_current_branch(Result::<
            Option<String>,
            &str,
        >::Ok(None))
        .expect_err("a missing branch must retain its typed variant");
        assert!(matches!(branch_absent, SignalCommandPortError::BranchAbsent));
    }

    #[test]
    fn test_system_signal_command_adapter_repository_discovery_failure_retains_typed_variant() {
        let error = SystemSignalCommandAdapter::discovered_root(Result::<SystemGitRepo, _>::Err(
            "repository unavailable",
        ))
        .expect_err("repository discovery failure must retain its typed variant");

        assert!(matches!(
            error,
            SignalCommandPortError::RepositoryDiscovery { reason }
                if reason.as_str() == "repository unavailable"
        ));
    }

    #[test]
    fn test_system_signal_gate_config_adapter_loads_repository_matrix() {
        let repository_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("infrastructure crate must be nested under the repository root")
            .to_path_buf();
        let matrix = SystemSignalGateConfigAdapter::new()
            .load(Some(repository_root.as_path()))
            .expect("repository signal gate matrix must load");

        assert_eq!(matrix.resolve(ChainId::AdrUser, GateKind::Commit), Strictness::Interim);
        assert_eq!(matrix.resolve(ChainId::ImplCatalog, GateKind::Merge), Strictness::Strict);
    }

    #[test]
    fn test_system_signal_gate_config_adapter_classifies_missing_and_invalid_configuration() {
        let missing_path = PathBuf::from("workspace/.harness/config/signal-gates.json");
        let missing = SystemSignalGateConfigAdapter::classify_load_error(
            missing_path.clone(),
            SignalGatesConfigError::FileNotFound { path: missing_path.clone() },
        );
        assert!(matches!(
            missing,
            SignalGateConfigError::ConfigurationNotFound { path } if path == missing_path
        ));

        let invalid_path = PathBuf::from("workspace/.harness/config/signal-gates.json");
        let invalid = SystemSignalGateConfigAdapter::classify_load_error(
            invalid_path.clone(),
            SignalGatesConfigError::ParseFailed {
                path: invalid_path.clone(),
                reason: "unexpected end of JSON input".to_owned(),
            },
        );
        assert!(matches!(
            invalid,
            SignalGateConfigError::ConfigurationInvalid { path, reason }
                if path == invalid_path && reason.as_str().contains("unexpected end of JSON input")
        ));
    }

    #[test]
    fn test_system_signal_command_adapter_calc_then_check_preserves_persisted_spec_document() {
        let workspace = tempfile::tempdir().unwrap();
        let spec_path = workspace.path().join("spec.json");
        let mut document: Value = serde_json::from_str(include_str!(
            "../../../track/items/pr-signal-pure-di-2026-07-26/spec.json"
        ))
        .unwrap();
        document.as_object_mut().unwrap().remove("signals");
        std::fs::write(&spec_path, serde_json::to_string_pretty(&document).unwrap()).unwrap();

        let adapter = SystemSignalCommandAdapter::new();
        let calc = adapter
            .execute(ResolvedSignalChainCommand::CalcSpecAdr {
                spec_json_path: Some(spec_path.clone()),
                workspace_root: Some(workspace.path().to_path_buf()),
            })
            .unwrap();
        assert!(!calc.outcome.has_errors());

        let persisted_after_calc = std::fs::read_to_string(&spec_path).unwrap();
        let persisted_document: Value = serde_json::from_str(&persisted_after_calc).unwrap();
        assert!(persisted_document.get("signals").is_some());
        assert!(
            persisted_after_calc.starts_with("{\n  \"acceptance_criteria\":"),
            "signal calculation must persist canonical JSON keys: {persisted_after_calc}"
        );

        let repeated_calc = adapter
            .execute(ResolvedSignalChainCommand::CalcSpecAdr {
                spec_json_path: Some(spec_path.clone()),
                workspace_root: Some(workspace.path().to_path_buf()),
            })
            .unwrap();
        assert!(!repeated_calc.outcome.has_errors());
        assert_eq!(
            std::fs::read_to_string(&spec_path).unwrap(),
            persisted_after_calc,
            "repeated signal calculation must not churn JSON bytes"
        );

        let check = adapter
            .execute(ResolvedSignalChainCommand::CheckSpecAdr {
                spec_json_path: Some(spec_path.clone()),
                strictness: Strictness::Strict,
                workspace_root: Some(workspace.path().to_path_buf()),
            })
            .unwrap();
        assert!(
            !check.outcome.has_errors(),
            "the persisted calc document must produce a passing strict check: {check:?}"
        );
        assert!(
            check.stdout.as_deref().is_some_and(|stdout| stdout.contains("PASSED")),
            "the persisted calc document must determine the reported passing verdict"
        );
        assert_eq!(std::fs::read_to_string(&spec_path).unwrap(), persisted_after_calc);

        let mut missing_signals: Value = serde_json::from_str(&persisted_after_calc).unwrap();
        missing_signals.as_object_mut().unwrap().remove("signals");
        std::fs::write(&spec_path, serde_json::to_string_pretty(&missing_signals).unwrap())
            .unwrap();

        let missing_document_check = adapter
            .execute(ResolvedSignalChainCommand::CheckSpecAdr {
                spec_json_path: Some(spec_path),
                strictness: Strictness::Strict,
                workspace_root: Some(workspace.path().to_path_buf()),
            })
            .unwrap();
        assert!(
            missing_document_check.outcome.has_errors(),
            "a check must deny when the persisted signal document is absent"
        );
        assert!(
            missing_document_check
                .stdout
                .as_deref()
                .is_some_and(|stdout| stdout.contains("FAILED")),
            "the reported verdict must reflect the missing persisted signal document"
        );
    }

    #[test]
    fn test_system_signal_command_adapter_external_spec_override_skips_root_resolution() {
        let external = tempfile::tempdir().unwrap();
        let external_spec_path = external.path().join("spec.json");
        let mut document: Value = serde_json::from_str(include_str!(
            "../../../track/items/pr-signal-pure-di-2026-07-26/spec.json"
        ))
        .unwrap();
        document.as_object_mut().unwrap().remove("signals");
        std::fs::write(&external_spec_path, serde_json::to_string_pretty(&document).unwrap())
            .unwrap();

        let adapter = SystemSignalCommandAdapter::new();
        let calc = adapter
            .execute(ResolvedSignalChainCommand::CalcSpecAdr {
                spec_json_path: Some(external_spec_path.clone()),
                workspace_root: None,
            })
            .expect("an explicit spec path must bypass repository discovery for calc");
        assert!(!calc.outcome.has_errors());

        let check = adapter
            .execute(ResolvedSignalChainCommand::CheckSpecAdr {
                spec_json_path: Some(external_spec_path),
                strictness: Strictness::Strict,
                workspace_root: None,
            })
            .expect("an explicit spec path must bypass repository discovery for strict check");
        assert!(!check.outcome.has_errors());
    }

    #[test]
    fn test_system_signal_command_adapter_adr_user_calc_and_check_preserve_adr_documents() {
        let workspace = tempfile::tempdir().unwrap();
        let adr_dir = workspace.path().join("knowledge/adr");
        std::fs::create_dir_all(&adr_dir).unwrap();
        let adr_path = adr_dir.join("2026-01-01-signal.md");
        std::fs::write(
            &adr_path,
            "---\nadr_id: 2026-01-01-signal\ndecisions: []\n---\n\n# Signal fixture\n",
        )
        .unwrap();
        let persisted_before_calc = std::fs::read_to_string(&adr_path).unwrap();

        let adapter = SystemSignalCommandAdapter::new();
        let calc = adapter
            .execute(ResolvedSignalChainCommand::CalcAdrUser {
                project_root: workspace.path().to_path_buf(),
            })
            .unwrap();
        assert!(!calc.outcome.has_errors(), "ADR calc must accept the persisted document");

        let check = adapter
            .execute(ResolvedSignalChainCommand::CheckAdrUser {
                project_root: SignalRootSelection::Supplied(workspace.path().to_path_buf()),
                strictness: Strictness::Strict,
            })
            .unwrap();
        assert!(!check.outcome.has_errors(), "ADR check must read the persisted document");
        assert_eq!(std::fs::read_to_string(adr_path).unwrap(), persisted_before_calc);
    }

    #[test]
    fn test_system_signal_command_adapter_catalog_spec_calc_then_check_preserves_signals() {
        let track_id = "catalog-spec-signal";
        let workspace = setup_track_workspace(track_id);
        let root = workspace.path().to_path_buf();
        let signals_path =
            root.join("track/items").join(track_id).join("domain-catalogue-spec-signals.json");

        let calc = SystemSignalCommandAdapter::calc_catalogue(root.clone())
            .expect("catalog-spec calc must load the layer bindings");
        assert!(!calc.has_errors(), "catalog-spec calc must persist a signals document: {calc:?}");
        let persisted_after_calc = std::fs::read_to_string(&signals_path).unwrap();

        let check = SystemSignalCommandAdapter::new()
            .execute(ResolvedSignalChainCommand::CheckCatalogSpec {
                strictness: Strictness::Strict,
                workspace_root: Some(root),
            })
            .unwrap();
        assert!(
            !check.outcome.has_errors(),
            "catalog-spec check must read the persisted signals document: {check:?}"
        );
        assert_eq!(std::fs::read_to_string(signals_path).unwrap(), persisted_after_calc);
    }

    #[test]
    fn test_system_signal_command_adapter_catalog_spec_calc_skips_opted_out_layers() {
        let track_id = "catalog-spec-mixed-layers";
        let workspace = setup_track_workspace(track_id);
        let root = workspace.path().to_path_buf();
        std::fs::write(
            root.join("architecture-rules.json"),
            r#"{
  "version": 2,
  "module_limits": { "max_lines": 700, "warn_lines": 400, "exclude": [] },
  "canonical_modules": [],
  "extra_dirs": [],
  "layers": [
    {
      "crate": "domain", "path": "libs/domain", "may_depend_on": [], "deny_reason": "",
      "tddd": {
        "enabled": true, "catalogue_file": "domain-types.json",
        "catalogue_spec_signal": { "enabled": true },
        "schema_export": { "method": "rustdoc", "targets": ["domain"] }
      }
    },
    {
      "crate": "usecase", "path": "libs/usecase", "may_depend_on": ["domain"], "deny_reason": "",
      "tddd": {
        "enabled": true, "catalogue_file": "usecase-types.json",
        "catalogue_spec_signal": { "enabled": false },
        "schema_export": { "method": "rustdoc", "targets": ["usecase"] }
      }
    }
  ]
}"#,
        )
        .unwrap();
        std::fs::write(
            root.join("track/items").join(track_id).join("usecase-types.json"),
            r#"{
  "schema_version": 5,
  "crate_name": "usecase",
  "layer": "usecase",
  "types": {},
  "traits": {},
  "functions": {}
}"#,
        )
        .unwrap();

        let calc = SystemSignalCommandAdapter::calc_catalogue(root.clone())
            .expect("opted-out catalogues must not enter the catalogue-spec calculation");

        assert!(!calc.has_errors(), "only opted-in layers may affect the calculation: {calc:?}");
        assert!(
            root.join("track/items")
                .join(track_id)
                .join("domain-catalogue-spec-signals.json")
                .exists()
        );
        assert!(
            !root
                .join("track/items")
                .join(track_id)
                .join("usecase-catalogue-spec-signals.json")
                .exists()
        );
    }

    #[test]
    fn test_system_signal_command_adapter_catalog_spec_tddd_load_failure_is_execution_error() {
        let track_id = "catalog-spec-invalid-config";
        let workspace = setup_track_workspace(track_id);
        let root = workspace.path().to_path_buf();
        std::fs::write(root.join("architecture-rules.json"), "not valid JSON").unwrap();

        let error = SystemSignalCommandAdapter::calc_catalogue(root)
            .expect_err("a malformed TDDD configuration must stop command execution");

        assert!(matches!(
            error,
            SignalCommandPortError::Execution { reason }
                if reason.as_str().contains("[ERROR] architecture-rules.json")
                    && reason.as_str().contains("not valid JSON")
        ));
    }

    #[test]
    fn test_system_signal_command_adapter_impl_catalog_execution_error_quotes_layer() {
        let layer = domain::tddd::LayerId::try_new("infrastructure".to_owned()).unwrap();

        let outcome =
            SystemSignalCommandAdapter::impl_layer_execution_failure(&layer, "executor failed");

        assert!(outcome.findings().first().is_some_and(|finding| {
            finding
                .to_string()
                .contains("signal calc-impl-catalog: layer 'infrastructure': executor failed")
        }));
    }

    #[test]
    fn test_system_signal_command_adapter_impl_catalog_check_preserves_persisted_signals() {
        let track_id = "impl-catalog-signal";
        let workspace = setup_track_workspace(track_id);
        let root = workspace.path().to_path_buf();
        let track_dir = root.join("track/items").join(track_id);
        let source_track_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .unwrap()
            .join("track/items/pr-signal-pure-di-2026-07-26");
        std::fs::copy(
            source_track_dir.join("domain-type-signals.json"),
            track_dir.join("domain-type-signals.json"),
        )
        .unwrap();
        std::fs::copy(
            source_track_dir.join("domain-types.json"),
            track_dir.join("domain-types.json"),
        )
        .unwrap();
        let signals_path = track_dir.join("domain-type-signals.json");
        let mut persisted: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&signals_path).unwrap()).unwrap();
        let object = persisted.as_object_mut().unwrap();
        let declaration_hash = object.get("declaration_hash").cloned().unwrap();
        object.insert(
            "schema_version".to_owned(),
            serde_json::json!(domain::TYPE_SIGNALS_SCHEMA_VERSION),
        );
        object.insert("baseline_hash".to_owned(), declaration_hash);
        std::fs::write(&signals_path, serde_json::to_string_pretty(&persisted).unwrap()).unwrap();
        let persisted_before_check = std::fs::read_to_string(&signals_path).unwrap();

        let check = SystemSignalCommandAdapter::new()
            .execute(ResolvedSignalChainCommand::CheckImplCatalog {
                strictness: Strictness::Strict,
                workspace_root: Some(root),
            })
            .unwrap();
        assert!(
            !check.outcome.has_errors(),
            "impl-catalog check must read the persisted signals document: {check:?}"
        );
        assert_eq!(std::fs::read_to_string(signals_path).unwrap(), persisted_before_check);
    }
}
