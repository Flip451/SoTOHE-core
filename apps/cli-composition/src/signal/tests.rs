//! Tests for the `signal` command family.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use crate::signal::SignalCompositionRoot;
use cli_driver::signal::{SignalGateName as DriverSignalGateName, SignalInput};
use cli_driver::signal_report::{
    SignalReportChainFilter, SignalReportInput, SignalReportLevelFilter,
};
use domain::review_v2::types::FilePath;
use domain::{ChainGateEntry, NonEmptyString, SignalGateMatrix, Strictness, verify::VerifyOutcome};
use std::sync::{Arc, Mutex};
use usecase::signal_report::{
    SignalReportChain, SignalReportEntryId, SignalReportError, SignalReportLevel,
    SignalReportLocation, SignalReportOccurrence, SignalReportReason, SignalReportReference,
    SignalReportSourcePort,
};
use usecase::signal_service::{
    ResolvedSignalChainCommand, SignalActiveTrackResolverPort, SignalChainExecutionReport,
    SignalCommandPort, SignalCommandPortError, SignalGateConfigError, SignalGateConfigPort,
    SignalRootSelection, SignalSpecPathResolverPort,
};

#[cfg(feature = "test-support")]
use domain::TrackId;
#[cfg(feature = "test-support")]
use infrastructure::tddd::type_signals_evaluator::RustdocLaunchObserver;

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

#[test]
fn test_signal_composition_root_constructs_wired_driver() {
    let root = SignalCompositionRoot::new();
    let workspace = tempfile::tempdir().unwrap();
    let outcome = root.signal_driver().handle(SignalInput::CheckGate {
        project_root: None,
        spec_json_path: None,
        gate: DriverSignalGateName::Commit,
        workspace_root: Some(workspace.path().to_path_buf()),
    });

    assert_eq!(outcome.stdout, None);
    let config_path = workspace.path().join(".harness/config/signal-gates.json");
    let expected_prefix =
        format!("[ERROR] failed to load signal-gates config from {}", config_path.display());
    assert!(
        outcome.stderr.as_deref().is_some_and(|stderr| stderr.starts_with(&expected_prefix)),
        "expected config-load diagnostic for {config_path:?}, got: {:?}",
        outcome.stderr
    );
    assert_eq!(outcome.exit_code, 1);
}

#[test]
fn test_signal_composition_root_constructs_signal_report_driver_without_dispatch() {
    let root = SignalCompositionRoot::new();

    let _driver = root.signal_report_driver();
}

struct StubSignalReportSource {
    occurrences: Vec<SignalReportOccurrence>,
}

impl SignalReportSourcePort for StubSignalReportSource {
    fn load(
        &self,
        chain: SignalReportChain,
    ) -> Result<Vec<SignalReportOccurrence>, SignalReportError> {
        Ok(self
            .occurrences
            .iter()
            .filter(|occurrence| occurrence.chain == chain)
            .cloned()
            .collect())
    }
}

fn signal_report_occurrence(
    chain: SignalReportChain,
    level: SignalReportLevel,
    entry_id: &str,
    reference: &str,
    reason: &str,
    location: &str,
) -> SignalReportOccurrence {
    SignalReportOccurrence {
        chain,
        level,
        entry_id: SignalReportEntryId::new(NonEmptyString::try_new(entry_id).unwrap()),
        reference: SignalReportReference::new(NonEmptyString::try_new(reference).unwrap()),
        reason: SignalReportReason::new(NonEmptyString::try_new(reason).unwrap()),
        location: SignalReportLocation::new(FilePath::new(location).unwrap()),
    }
}

#[test]
fn test_signal_composition_root_signal_report_driver_renders_yellow_and_red_block_causes() {
    let root = SignalCompositionRoot::new();
    let driver = root.signal_report_driver_with_source(Arc::new(StubSignalReportSource {
        occurrences: vec![
            signal_report_occurrence(
                SignalReportChain::AdrUser,
                SignalReportLevel::Yellow,
                "D1",
                "chat_segment:signal-report",
                "user evidence remains unresolved",
                "knowledge/adr/2026-07-29-0839-signal-report-command.md",
            ),
            signal_report_occurrence(
                SignalReportChain::ImplCatalog,
                SignalReportLevel::Red,
                "SignalCompositionRoot",
                "cli_composition-types.json#GO-01",
                "missing implementation reference blocks the gate",
                "apps/cli-composition/src/signal/mod.rs",
            ),
        ],
    }));

    let outcome = driver.handle(SignalReportInput {
        chain: SignalReportChainFilter::All,
        levels: SignalReportLevelFilter::YellowAndRed,
    });

    assert_eq!(outcome.exit_code, 0);
    assert_eq!(outcome.stderr, None);
    let output = outcome.stdout.as_deref().expect("report output must be rendered");
    assert!(output.contains("chain=adr_user level=yellow entry_id=D1"));
    assert!(output.contains("reason=user evidence remains unresolved"));
    assert!(output.contains("chain=impl_catalog level=red entry_id=SignalCompositionRoot"));
    assert!(output.contains("reason=missing implementation reference blocks the gate"));
}

#[test]
fn test_signal_report_driver_factory_is_wiring_only() {
    let source = include_str!("mod.rs");

    for wired_component in [
        "SystemSignalReportSourceAdapter::new()",
        "SignalReportInteractor::new(source)",
        "SignalReportDriver::new(service)",
    ] {
        assert!(
            source.contains(wired_component),
            "signal-report factory must wire {wired_component}"
        );
    }
    assert!(!source.contains(".handle("), "signal composition must not dispatch a driver");
}

#[test]
fn test_signal_composition_root_signal_driver_dispatches_adr_user_to_system_port() {
    let root = SignalCompositionRoot::new();
    let workspace = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(workspace.path().join("knowledge/adr")).unwrap();

    let outcome = root
        .signal_driver()
        .handle(SignalInput::CalcAdrUser { project_root: workspace.path().to_path_buf() });

    assert_eq!(outcome.exit_code, 0);
    assert_eq!(outcome.stderr, None);
    assert!(
        outcome
            .stdout
            .as_deref()
            .is_some_and(|stdout| stdout.contains("signal calc-adr-user PASSED"))
    );
}

#[test]
fn test_signal_driver_calc_and_check_spec_adr_preserve_command_and_persistence_parity() {
    let workspace = tempfile::tempdir().unwrap();
    let spec_path = workspace.path().join("spec.json");
    let mut document: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../track/items/pr-signal-pure-di-2026-07-26/spec.json"
    )))
    .unwrap();
    document.as_object_mut().unwrap().remove("signals");
    std::fs::write(&spec_path, serde_json::to_string_pretty(&document).unwrap()).unwrap();

    let persisted_before_calc = std::fs::read_to_string(&spec_path).unwrap();
    assert!(
        serde_json::from_str::<serde_json::Value>(&persisted_before_calc)
            .unwrap()
            .get("signals")
            .is_none(),
        "the fixture must begin without persisted signal results"
    );

    let driver = SignalCompositionRoot::new().signal_driver();
    let calc = driver.handle(SignalInput::CalcSpecAdr {
        spec_json_path: Some(spec_path.clone()),
        workspace_root: Some(workspace.path().to_path_buf()),
    });

    assert_eq!(calc.exit_code, 0);
    assert_eq!(calc.stderr, None);
    assert!(
        calc.stdout.as_deref().is_some_and(|stdout| stdout.contains("signal calc-spec-adr PASSED"))
    );

    let persisted_after_calc = std::fs::read_to_string(&spec_path).unwrap();
    assert!(
        serde_json::from_str::<serde_json::Value>(&persisted_after_calc)
            .unwrap()
            .get("signals")
            .is_some(),
        "calc must persist the signal result"
    );

    let check = driver.handle(SignalInput::CheckSpecAdr {
        spec_json_path: Some(spec_path.clone()),
        strict_override: true,
        gate: Some(DriverSignalGateName::Commit),
        workspace_root: Some(workspace.path().to_path_buf()),
    });

    assert_eq!(check.exit_code, 0);
    assert_eq!(check.stderr, None);
    assert!(
        check
            .stdout
            .as_deref()
            .is_some_and(|stdout| stdout.contains("signal check-spec-adr PASSED"))
    );
    assert_eq!(std::fs::read_to_string(spec_path).unwrap(), persisted_after_calc);
}

struct RecordingSignalCommandPort(Mutex<Vec<ResolvedSignalChainCommand>>);

impl SignalCommandPort for RecordingSignalCommandPort {
    fn execute(
        &self,
        command: ResolvedSignalChainCommand,
    ) -> Result<SignalChainExecutionReport, SignalCommandPortError> {
        self.0.lock().unwrap().push(command);
        Ok(SignalChainExecutionReport {
            outcome: VerifyOutcome::pass(),
            stdout: Some("recorded port output".to_owned()),
            stderr: None,
        })
    }
}

struct FixedActiveTrackResolver;

impl SignalActiveTrackResolverPort for FixedActiveTrackResolver {
    fn resolve_active_track(
        &self,
        _workspace_root: Option<&std::path::Path>,
    ) -> Result<domain::TrackId, SignalCommandPortError> {
        domain::TrackId::try_new("test-track".to_owned()).map_err(|error| {
            SignalCommandPortError::Execution {
                reason: usecase::signal_service::SignalFailureReason::new(error.to_string()),
            }
        })
    }
}

struct DefaultSpecPathResolver;

impl SignalSpecPathResolverPort for DefaultSpecPathResolver {
    fn resolve_spec_path(
        &self,
        _workspace_root: Option<&std::path::Path>,
        spec_json_path: Option<&std::path::Path>,
    ) -> Result<std::path::PathBuf, SignalCommandPortError> {
        Ok(spec_json_path
            .map(std::path::Path::to_path_buf)
            .unwrap_or_else(|| std::path::PathBuf::from("workspace/spec.json")))
    }
}

struct RecordingSignalGateConfig;

impl SignalGateConfigPort for RecordingSignalGateConfig {
    fn load(
        &self,
        _workspace_root: Option<&std::path::Path>,
    ) -> Result<SignalGateMatrix, SignalGateConfigError> {
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

#[test]
fn test_signal_composition_root_all_commands_follow_single_path_and_preserve_outcomes() {
    let root = SignalCompositionRoot::new();
    let port = Arc::new(RecordingSignalCommandPort(Mutex::new(Vec::new())));
    let driver = root.signal_driver_with_ports(
        port.clone(),
        Arc::new(FixedActiveTrackResolver),
        Arc::new(DefaultSpecPathResolver),
        Arc::new(RecordingSignalGateConfig),
    );
    let project_root = std::path::PathBuf::from("project");
    let spec_json_path = std::path::PathBuf::from("project/spec.json");
    let workspace_root = std::path::PathBuf::from("workspace");

    let outcomes = [
        driver.handle(SignalInput::CalcAdrUser { project_root: project_root.clone() }),
        driver.handle(SignalInput::CheckAdrUser {
            project_root: project_root.clone(),
            strict_override: false,
            gate: Some(DriverSignalGateName::Commit),
            workspace_root: Some(workspace_root.clone()),
        }),
        driver.handle(SignalInput::CalcSpecAdr {
            spec_json_path: Some(spec_json_path.clone()),
            workspace_root: Some(workspace_root.clone()),
        }),
        driver.handle(SignalInput::CheckSpecAdr {
            spec_json_path: Some(spec_json_path.clone()),
            strict_override: true,
            gate: Some(DriverSignalGateName::Merge),
            workspace_root: Some(workspace_root.clone()),
        }),
        driver.handle(SignalInput::CalcCatalogSpec),
        driver.handle(SignalInput::CheckCatalogSpec {
            strict_override: false,
            gate: Some(DriverSignalGateName::Merge),
            workspace_root: Some(workspace_root.clone()),
        }),
        driver.handle(SignalInput::CalcImplCatalog),
        driver.handle(SignalInput::CheckImplCatalog {
            strict_override: true,
            gate: Some(DriverSignalGateName::Commit),
            workspace_root: Some(workspace_root.clone()),
        }),
        driver.handle(SignalInput::CheckGate {
            project_root: Some(project_root.clone()),
            spec_json_path: Some(spec_json_path.clone()),
            gate: DriverSignalGateName::Merge,
            workspace_root: Some(workspace_root.clone()),
        }),
    ];

    assert!(outcomes.iter().all(|outcome| {
        outcome.stdout.as_deref() == Some("recorded port output")
            || (outcome.exit_code == 0
                && outcome.stderr.is_none()
                && outcome.stdout.as_deref().is_some_and(|output| output.contains("PASSED")))
    }));
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
                strictness: Strictness::Strict,
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

#[test]
fn test_signal_execution_sources_do_not_reference_legacy_service_or_shim() {
    let composition_source = include_str!("mod.rs");
    let execution_sources = [
        ("apps/cli-composition/src/signal/mod.rs", composition_source),
        (
            "apps/cli/src/commands/signal/mod.rs",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../apps/cli/src/commands/signal/mod.rs"
            )),
        ),
        (
            "libs/usecase/src/signal_service.rs",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../libs/usecase/src/signal_service.rs"
            )),
        ),
        (
            "libs/infrastructure/src/signal.rs",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../libs/infrastructure/src/signal.rs"
            )),
        ),
    ];

    for (path, source) in execution_sources {
        assert!(!source.contains("SignalServiceImpl"), "{path} retains the legacy service");
        assert!(!source.contains("signal::shim"), "{path} retains the signal shim");
    }
}

#[test]
fn test_system_signal_command_adapter_with_live_execution_sources_has_only_driver_interactor_port_route()
 {
    let composition_source = include_str!("mod.rs");
    let driver_source =
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/cli-driver/src/signal.rs"));
    let interactor_source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../libs/usecase/src/signal_service.rs"
    ));
    let adapter_source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../libs/infrastructure/src/signal.rs"
    ));
    let composition_public_api_source =
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/lib.rs"));

    assert!(
        composition_source.contains(
            "let adapter = Arc::new(infrastructure::signal::SystemSignalCommandAdapter::new());"
        ),
        "the composition root must be the live adapter construction point"
    );
    assert!(
        composition_source.contains(
            "self.signal_driver_with_ports(adapter.clone(), adapter.clone(), adapter, gate_config)"
        ),
        "the constructed adapter must supply all segregated Signal ports"
    );
    assert!(
        composition_source.contains("port: Arc<dyn usecase::signal_service::SignalCommandPort>"),
        "the wiring helper must accept the typed execution port"
    );
    assert!(
        composition_source.contains(
            "active_track_resolver: Arc<dyn usecase::signal_service::SignalActiveTrackResolverPort>"
        ) && composition_source.contains(
            "spec_path_resolver: Arc<dyn usecase::signal_service::SignalSpecPathResolverPort>"
        ),
        "the wiring helper must accept the two resolver ports independently"
    );
    assert!(
        composition_source
            .contains("usecase::signal_service::SignalCommandInteractor::new(\n            port,"),
        "the segregated ports must be owned by the interactor"
    );
    assert!(
        composition_source.contains("cli_driver::signal::SignalDriver::new(service)"),
        "the interactor must be the driver's sole execution dependency"
    );

    assert!(
        driver_source.contains("service: Arc<dyn SignalService>"),
        "the driver must depend only on the usecase application service"
    );
    assert!(
        !driver_source.contains("SystemSignalCommandAdapter"),
        "the driver must not construct or re-enter the infrastructure adapter"
    );
    assert!(
        !driver_source.contains("SignalCommandPort"),
        "the driver must not bypass the interactor through the port"
    );

    assert!(
        interactor_source.contains("port: Arc<dyn SignalCommandPort>"),
        "the interactor must own the typed execution port"
    );
    assert!(
        interactor_source.contains("active_track_resolver: Arc<dyn SignalActiveTrackResolverPort>")
            && interactor_source
                .contains("spec_path_resolver: Arc<dyn SignalSpecPathResolverPort>"),
        "the interactor must own independent resolver ports"
    );
    assert!(
        interactor_source.contains("impl SignalService for SignalCommandInteractor"),
        "the interactor must implement the driver's application-service boundary"
    );
    assert!(
        !interactor_source.contains("SystemSignalCommandAdapter"),
        "the interactor must not re-enter a concrete infrastructure adapter"
    );

    assert!(
        adapter_source.contains("impl SignalCommandPort for SystemSignalCommandAdapter"),
        "the adapter must participate only as the typed execution port"
    );
    assert!(
        adapter_source
            .contains("impl SignalActiveTrackResolverPort for SystemSignalCommandAdapter")
            && adapter_source
                .contains("impl SignalSpecPathResolverPort for SystemSignalCommandAdapter"),
        "the system adapter must implement both resolver ports"
    );
    for forbidden_dependency in
        ["cli_composition", "cli_driver", "SignalServiceImpl", "signal::shim"]
    {
        assert!(
            !adapter_source.contains(forbidden_dependency),
            "the adapter must not reach {forbidden_dependency} through a compatibility path"
        );
    }
    assert!(
        !composition_public_api_source
            .contains("pub use infrastructure::signal::SystemSignalCommandAdapter"),
        "the composition crate must not re-export the adapter as a compatibility facade"
    );
}

#[test]
fn test_signal_composition_root_public_api_exposes_only_driver_factory() {
    let public_methods = include_str!("mod.rs")
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("pub ") && line.contains(" fn "))
        .collect::<Vec<_>>();

    assert_eq!(
        public_methods,
        vec![
            "pub fn new() -> Self {",
            "pub fn signal_driver(&self) -> cli_driver::signal::SignalDriver {",
            "pub fn signal_report_driver(&self) -> cli_driver::signal_report::SignalReportDriver {",
        ]
    );

    let source = include_str!("mod.rs");
    for execution_method in ["signal_calc_", "signal_check_", "signal_check_gate"] {
        assert!(
            !source.contains(&format!("fn {execution_method}")),
            "SignalCompositionRoot retains execution method {execution_method}"
        );
    }
}

/// Set up a minimal workspace directory containing `architecture-rules.json`,
/// `.harness/config/signal-gates.json`, and the `track/items/<track_id>/` tree.
///
/// Initialises a git repo so `SystemGitRepo::discover()` succeeds, and sets
/// the current branch to `track/<track_id>` so `active_track_id()` resolves.
fn setup_workspace(track_id: &str, arch_rules: &str, signal_gates: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

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

    std::fs::write(root.join("architecture-rules.json"), arch_rules).unwrap();
    std::fs::create_dir_all(root.join(".harness/config")).unwrap();
    std::fs::write(root.join(".harness/config/signal-gates.json"), signal_gates).unwrap();

    std::fs::create_dir_all(root.join("track/items").join(track_id)).unwrap();

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

/// When all layers have `tddd.enabled: false`, the driver must fail closed for
/// chain ③ with a `[BLOCKED]` message.
#[test]
fn test_signal_check_impl_catalog_empty_bindings_fail_closed() {
    let track_id = "T999";
    let dir = setup_workspace(track_id, ARCH_RULES_ALL_TDDD_DISABLED, SIGNAL_GATES_ALL_STRICT);

    let outcome =
        SignalCompositionRoot::new().signal_driver().handle(SignalInput::CheckImplCatalog {
            strict_override: false,
            gate: Some(DriverSignalGateName::Commit),
            workspace_root: Some(dir.path().to_path_buf()),
        });
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

/// Chain ② with all layers disabled passes without error — it does not enforce
/// the empty-set contract.
#[test]
fn test_signal_check_catalog_spec_empty_bindings_passes() {
    let track_id = "T999";
    let dir = setup_workspace(track_id, ARCH_RULES_ALL_TDDD_DISABLED, SIGNAL_GATES_ALL_STRICT);

    let outcome =
        SignalCompositionRoot::new().signal_driver().handle(SignalInput::CheckCatalogSpec {
            strict_override: false,
            gate: Some(DriverSignalGateName::Commit),
            workspace_root: Some(dir.path().to_path_buf()),
        });
    assert_eq!(
        outcome.exit_code, 0,
        "chain ② with empty enabled-layer set should pass vacuously: {outcome:?}"
    );
}

#[cfg(feature = "test-support")]
const ARCH_RULES_TWO_TDDD_LAYERS: &str = r#"{
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
        "schema_export": { "method": "rustdoc", "targets": ["domain"] }
      }
    },
    {
      "crate": "usecase",
      "path": "libs/usecase",
      "may_depend_on": ["domain"],
      "deny_reason": "",
      "tddd": {
        "enabled": true,
        "catalogue_file": "usecase-types.json",
        "schema_export": { "method": "rustdoc", "targets": ["usecase"] }
      }
    }
  ]
}"#;

#[cfg(feature = "test-support")]
fn minimal_rustdoc_json() -> String {
    format!(
        r#"{{"root":0,"crate_version":null,"includes_private":false,"index":{{}},"paths":{{}},"external_crates":{{}},"format_version":{},"target":{{"triple":"","target_features":[]}}}}"#,
        rustdoc_types::FORMAT_VERSION
    )
}

/// A real root → driver → service → interactor → adapter fixture. The observer
/// replaces only rustdoc process launch; all freshness and signal paths remain
/// production code.
#[cfg(feature = "test-support")]
fn setup_type_signal_workspace()
-> (tempfile::TempDir, TrackId, std::collections::BTreeMap<String, std::path::PathBuf>) {
    let track_id = "freshness-composition";
    let workspace = setup_workspace(track_id, ARCH_RULES_TWO_TDDD_LAYERS, SIGNAL_GATES_ALL_STRICT);
    let root = workspace.path();
    let track_dir = root.join("track/items").join(track_id);
    std::fs::create_dir_all(root.join("libs/domain/src")).unwrap();
    std::fs::create_dir_all(root.join("libs/usecase/src")).unwrap();
    std::fs::create_dir_all(root.join("target/doc")).unwrap();
    std::fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = [\"libs/domain\", \"libs/usecase\"]\nresolver = \"2\"\n",
    )
    .unwrap();
    std::fs::write(
        root.join("Cargo.lock"),
        "version = 4\n\n[[package]]\nname = \"domain\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    std::fs::write(
        root.join("libs/domain/Cargo.toml"),
        "[package]\nname = \"domain\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    std::fs::write(
        root.join("libs/usecase/Cargo.toml"),
        "[package]\nname = \"usecase\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    std::fs::write(root.join("libs/domain/src/lib.rs"), "pub struct Fixture;\n").unwrap();
    std::fs::write(root.join("libs/usecase/src/lib.rs"), "pub struct Fixture;\n").unwrap();
    std::fs::write(
        track_dir.join("domain-types.json"),
        "{\n  \"schema_version\": 5,\n  \"crate_name\": \"domain\",\n  \"layer\": \"domain\",\n  \"types\": {},\n  \"traits\": {},\n  \"functions\": {}\n}\n",
    )
    .unwrap();
    std::fs::write(
        track_dir.join("usecase-types.json"),
        "{\n  \"schema_version\": 5,\n  \"crate_name\": \"usecase\",\n  \"layer\": \"usecase\",\n  \"types\": {},\n  \"traits\": {},\n  \"functions\": {}\n}\n",
    )
    .unwrap();
    let domain_rustdoc_json_path = root.join("target/doc/domain.json");
    let usecase_rustdoc_json_path = root.join("target/doc/usecase.json");
    let rustdoc_json = minimal_rustdoc_json();
    std::fs::write(&domain_rustdoc_json_path, &rustdoc_json).unwrap();
    std::fs::write(&usecase_rustdoc_json_path, &rustdoc_json).unwrap();
    std::fs::write(track_dir.join("domain-types-baseline.json"), &rustdoc_json).unwrap();
    std::fs::write(track_dir.join("usecase-types-baseline.json"), rustdoc_json).unwrap();
    let feature_declaration = r#"{
  "schema_version": 1,
  "layers": {
    "domain": [],
    "usecase": []
  }
}"#;
    std::fs::write(track_dir.join("tddd-features.json"), feature_declaration).unwrap();
    std::fs::write(track_dir.join("tddd-features-baseline.json"), feature_declaration).unwrap();
    std::fs::write(
        root.join(".gitignore"),
        "track/items/freshness-composition/*-type-signals.json\n",
    )
    .unwrap();
    std::process::Command::new("git").args(["add", "."]).current_dir(root).status().ok();
    std::process::Command::new("git")
        .env("GIT_AUTHOR_NAME", "test")
        .env("GIT_AUTHOR_EMAIL", "test@test.com")
        .env("GIT_COMMITTER_NAME", "test")
        .env("GIT_COMMITTER_EMAIL", "test@test.com")
        .args(["commit", "--quiet", "-m", "fixture inputs"])
        .current_dir(root)
        .status()
        .ok();

    let rustdoc_json_paths = std::collections::BTreeMap::from([
        ("domain".to_owned(), domain_rustdoc_json_path),
        ("usecase".to_owned(), usecase_rustdoc_json_path),
    ]);
    (workspace, TrackId::try_new(track_id).unwrap(), rustdoc_json_paths)
}

/// The actual-capture declaration must be verified before the canonical signal
/// path reaches its rustdoc executor.
#[cfg(feature = "test-support")]
#[test]
fn test_signal_calc_impl_catalog_absent_feature_declaration_stops_before_rustdoc() {
    let (workspace, track_id, rustdoc_json_paths) = setup_type_signal_workspace();
    let declaration_path =
        workspace.path().join("track/items/freshness-composition/tddd-features.json");
    std::fs::remove_file(declaration_path).unwrap();
    let observer = RustdocLaunchObserver::using_json_paths(rustdoc_json_paths);

    let outcome = calc_impl_catalog_with_observer(workspace.path(), track_id, observer.clone());

    assert_ne!(outcome.exit_code, 0, "an absent declaration must fail the canonical path");
    assert_eq!(observer.launches(), 0, "the declaration failure must occur before rustdoc launch");
}

#[cfg(feature = "test-support")]
fn calc_impl_catalog_with_observer(
    root: &std::path::Path,
    track_id: TrackId,
    observer: RustdocLaunchObserver,
) -> crate::CommandOutcome {
    SignalCompositionRoot::new()
        .signal_driver_for_test_workspace(root.to_path_buf(), track_id, observer)
        .handle(SignalInput::CalcImplCatalog)
}

#[cfg(feature = "test-support")]
#[test]
fn test_signal_driver_calc_impl_catalog_persists_each_signal_document() {
    if !nightly_toolchain_available() {
        eprintln!("skipping impl-catalog persistence lane: nightly toolchain is unavailable");
        return;
    }

    let (workspace, track_id, rustdoc_json_paths) = setup_type_signal_workspace();
    let outcome = calc_impl_catalog_with_observer(
        workspace.path(),
        track_id,
        RustdocLaunchObserver::using_json_paths(rustdoc_json_paths),
    );

    assert_eq!(outcome.exit_code, 0, "calc must persist every enabled layer: {outcome:?}");
    for layer in ["domain", "usecase"] {
        let signal_path = workspace
            .path()
            .join("track/items/freshness-composition")
            .join(format!("{layer}-type-signals.json"));
        let persisted: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(signal_path).unwrap()).unwrap();
        assert!(persisted.get("head_commit").is_some());
    }
}

#[cfg(feature = "test-support")]
fn nightly_toolchain_available() -> bool {
    std::process::Command::new("rustup")
        .args(["run", "nightly", "rustc", "-Vv"])
        .status()
        .is_ok_and(|status| status.success())
}

/// The injected seam must preserve the production wiring while distinguishing
/// verified skip from conservative re-extraction.
#[cfg(feature = "test-support")]
#[test]
fn test_signal_composition_freshness_skip_and_conservative_reextract_use_real_adapter() {
    if !nightly_toolchain_available() {
        eprintln!("skipping freshness composition lane: nightly toolchain is unavailable");
        return;
    }

    let (workspace, track_id, rustdoc_json_paths) = setup_type_signal_workspace();
    let root = workspace.path();

    let initial_observer = RustdocLaunchObserver::using_json_paths(rustdoc_json_paths.clone());
    let initial = calc_impl_catalog_with_observer(root, track_id.clone(), initial_observer.clone());
    assert_eq!(initial.exit_code, 0, "initial calculation must succeed: {initial:?}");
    assert_eq!(initial_observer.launches_for("domain"), 1, "domain must export rustdoc initially");
    assert_eq!(
        initial_observer.launches_for("usecase"),
        1,
        "usecase must export rustdoc initially"
    );

    let signal_path = root.join("track/items/freshness-composition/domain-type-signals.json");
    let initial_signals: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&signal_path).unwrap()).unwrap();
    let initial_declaration_hash = initial_signals.get("declaration_hash").cloned().unwrap();
    assert!(initial_signals.get("head_commit").unwrap().is_string());

    let skip_observer = RustdocLaunchObserver::using_json_paths(rustdoc_json_paths.clone());
    let skip = calc_impl_catalog_with_observer(root, track_id.clone(), skip_observer.clone());
    assert_eq!(skip.exit_code, 0, "verified inputs must skip cleanly: {skip:?}");
    assert_eq!(skip_observer.launches(), 0, "verified inputs must not launch rustdoc");

    let catalogue_path = root.join("track/items/freshness-composition/domain-types.json");
    let catalogue = std::fs::read_to_string(&catalogue_path).unwrap();
    std::fs::write(&catalogue_path, format!("{catalogue}\n")).unwrap();
    let catalogue_only_observer =
        RustdocLaunchObserver::using_json_paths(rustdoc_json_paths.clone());
    let catalogue_only =
        calc_impl_catalog_with_observer(root, track_id.clone(), catalogue_only_observer.clone());
    assert_eq!(
        catalogue_only.exit_code, 0,
        "catalogue-only changes must reevaluate cleanly: {catalogue_only:?}"
    );
    assert_eq!(
        catalogue_only_observer.launches(),
        2,
        "a dirty worktree must recalculate every layer with fresh rustdoc extraction"
    );
    let reevaluated_signals: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&signal_path).unwrap()).unwrap();
    assert_ne!(
        reevaluated_signals.get("declaration_hash").cloned().unwrap(),
        initial_declaration_hash,
        "the changed declaration must be recorded, proving signal evaluation reran"
    );
    assert!(reevaluated_signals.get("head_commit").unwrap().is_string());

    let mut incomplete_signals = reevaluated_signals;
    incomplete_signals.as_object_mut().unwrap().remove("head_commit");
    std::fs::write(&signal_path, serde_json::to_vec(&incomplete_signals).unwrap()).unwrap();
    let incomplete_observer = RustdocLaunchObserver::using_json_paths(rustdoc_json_paths.clone());
    let incomplete =
        calc_impl_catalog_with_observer(root, track_id.clone(), incomplete_observer.clone());
    assert_eq!(incomplete.exit_code, 0, "incomplete artifacts must be regenerated: {incomplete:?}");
    assert_eq!(
        incomplete_observer.launches(),
        2,
        "a recorded artifact without every required identity must not be reused"
    );

    std::fs::write(root.join("libs/domain/src/lib.rs"), "pub struct ChangedFixture;\n").unwrap();
    let changed_source_observer =
        RustdocLaunchObserver::using_json_paths(rustdoc_json_paths.clone());
    let changed_source =
        calc_impl_catalog_with_observer(root, track_id.clone(), changed_source_observer.clone());
    assert_eq!(
        changed_source.exit_code, 0,
        "changed source contents must recalculate cleanly: {changed_source:?}"
    );
    assert_eq!(
        changed_source_observer.launches_for("domain"),
        1,
        "the changed domain layer must re-extract through the real adapter"
    );
    assert_eq!(
        changed_source_observer.launches_for("usecase"),
        1,
        "a dirty worktree must recalculate the unchanged layer too"
    );
}
