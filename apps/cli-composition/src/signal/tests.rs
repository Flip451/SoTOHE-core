//! Tests for the `signal` command family.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::*;
use crate::signal::SignalCompositionRoot;

#[cfg(feature = "test-support")]
use cli_driver::signal::SignalInput;
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

/// When all layers have `tddd.enabled: false`, `signal_check_impl_catalog`
/// (chain ③) must fail-closed with a `[BLOCKED]` message.
#[test]
fn test_signal_check_impl_catalog_empty_bindings_fail_closed() {
    let track_id = "T999";
    let dir = setup_workspace(track_id, ARCH_RULES_ALL_TDDD_DISABLED, SIGNAL_GATES_ALL_STRICT);

    let app = SignalCompositionRoot::new();
    let result = app.signal_check_impl_catalog(
        None,
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

    let app = SignalCompositionRoot::new();
    let result = app.signal_check_catalog_spec(
        None,
        Some(SignalGateName::Commit),
        Some(dir.path().to_path_buf()),
    );

    let outcome = result.expect("signal_check_catalog_spec should not return Err");
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
    assert!(initial_signals.get("implementation_input_hash").unwrap().is_string());

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
        0,
        "catalogue-only changes must reevaluate without rustdoc extraction"
    );
    let reevaluated_signals: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&signal_path).unwrap()).unwrap();
    assert_ne!(
        reevaluated_signals.get("declaration_hash").cloned().unwrap(),
        initial_declaration_hash,
        "the changed declaration must be recorded, proving signal evaluation reran"
    );
    assert!(reevaluated_signals.get("implementation_input_hash").unwrap().is_string());

    let mut incomplete_signals = reevaluated_signals;
    incomplete_signals.as_object_mut().unwrap().remove("implementation_input_hash");
    std::fs::write(&signal_path, serde_json::to_vec(&incomplete_signals).unwrap()).unwrap();
    let incomplete_observer = RustdocLaunchObserver::using_json_paths(rustdoc_json_paths.clone());
    let incomplete =
        calc_impl_catalog_with_observer(root, track_id.clone(), incomplete_observer.clone());
    assert_eq!(incomplete.exit_code, 0, "incomplete artifacts must be regenerated: {incomplete:?}");
    assert_eq!(
        incomplete_observer.launches(),
        1,
        "a recorded artifact without every required hash must not be reused"
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
        0,
        "the unchanged usecase layer must skip rustdoc extraction"
    );

    let lockfile_path = root.join("Cargo.lock");
    let changed_lockfile_content = [
        std::fs::read_to_string(&lockfile_path).unwrap(),
        "# changed lockfile fixture for freshness verification\n".to_owned(),
    ]
    .concat();
    std::fs::write(&lockfile_path, &changed_lockfile_content).unwrap();
    let changed_lockfile_observer =
        RustdocLaunchObserver::using_json_paths(rustdoc_json_paths.clone());
    let changed_lockfile =
        calc_impl_catalog_with_observer(root, track_id.clone(), changed_lockfile_observer.clone());
    assert_eq!(
        changed_lockfile.exit_code, 0,
        "changed lockfile contents must recalculate cleanly: {changed_lockfile:?}"
    );
    assert_eq!(
        changed_lockfile_observer.launches_for("domain"),
        1,
        "a lockfile change must re-extract the domain layer"
    );
    assert_eq!(
        changed_lockfile_observer.launches_for("usecase"),
        1,
        "a lockfile change must re-extract every affected layer"
    );

    std::fs::remove_file(&lockfile_path).unwrap();
    let restored_lockfile_path = lockfile_path.clone();
    let indeterminate_observer = RustdocLaunchObserver::using_json_path_with_before_export(
        rustdoc_json_paths.get("domain").cloned().unwrap(),
        std::sync::Arc::new(move || {
            std::fs::write(&restored_lockfile_path, &changed_lockfile_content).unwrap();
        }),
    );
    let indeterminate =
        calc_impl_catalog_with_observer(root, track_id, indeterminate_observer.clone());
    assert_eq!(
        indeterminate.exit_code, 0,
        "an indeterminate reuse hash must fall back to a successful fresh evaluation: {indeterminate:?}"
    );
    assert_eq!(
        indeterminate_observer.launches(),
        1,
        "an indeterminate implementation hash must re-extract rather than skip"
    );
}
