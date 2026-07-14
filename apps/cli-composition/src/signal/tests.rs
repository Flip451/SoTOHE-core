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
const ARCH_RULES_ONE_TDDD_LAYER: &str = r#"{
  "version": 2,
  "module_limits": { "max_lines": 700, "warn_lines": 400, "exclude": [] },
  "canonical_modules": [],
  "extra_dirs": [],
  "layers": [
    {
      "crate": "domain",
      "path": "crates/domain",
      "may_depend_on": [],
      "deny_reason": "",
      "tddd": {
        "enabled": true,
        "catalogue_file": "domain-types.json",
        "schema_export": { "method": "rustdoc", "targets": ["domain"] }
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
fn setup_type_signal_workspace() -> (tempfile::TempDir, TrackId, std::path::PathBuf) {
    let track_id = "freshness-composition";
    let workspace = setup_workspace(track_id, ARCH_RULES_ONE_TDDD_LAYER, SIGNAL_GATES_ALL_STRICT);
    let root = workspace.path();
    let track_dir = root.join("track/items").join(track_id);
    std::fs::create_dir_all(root.join("crates/domain/src")).unwrap();
    std::fs::create_dir_all(root.join("target/doc")).unwrap();
    std::fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = [\"crates/domain\"]\nresolver = \"2\"\n",
    )
    .unwrap();
    std::fs::write(
        root.join("Cargo.lock"),
        "version = 4\n\n[[package]]\nname = \"domain\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    std::fs::write(
        root.join("crates/domain/Cargo.toml"),
        "[package]\nname = \"domain\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    std::fs::write(root.join("crates/domain/src/lib.rs"), "pub struct Fixture;\n").unwrap();
    std::fs::write(
        track_dir.join("domain-types.json"),
        "{\n  \"schema_version\": 5,\n  \"crate_name\": \"domain\",\n  \"layer\": \"domain\",\n  \"types\": {},\n  \"traits\": {},\n  \"functions\": {}\n}\n",
    )
    .unwrap();
    let snapshot_path = root.join("target/doc/domain.json");
    let snapshot = minimal_rustdoc_json();
    std::fs::write(&snapshot_path, &snapshot).unwrap();
    std::fs::write(track_dir.join("domain-types-baseline.json"), snapshot).unwrap();

    (workspace, TrackId::try_new(track_id).unwrap(), snapshot_path)
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

    let (workspace, track_id, snapshot_path) = setup_type_signal_workspace();
    let root = workspace.path();

    let initial_observer = RustdocLaunchObserver::using_snapshot(snapshot_path.clone());
    let initial = calc_impl_catalog_with_observer(root, track_id.clone(), initial_observer.clone());
    assert_eq!(initial.exit_code, 0, "initial calculation must succeed: {initial:?}");
    assert_eq!(initial_observer.launches(), 1, "initial calculation must export rustdoc");

    let skip_observer = RustdocLaunchObserver::using_snapshot(snapshot_path.clone());
    let skip = calc_impl_catalog_with_observer(root, track_id.clone(), skip_observer.clone());
    assert_eq!(skip.exit_code, 0, "verified inputs must skip cleanly: {skip:?}");
    assert_eq!(skip_observer.launches(), 0, "verified inputs must not launch rustdoc");

    std::fs::write(
        root.join("Cargo.lock"),
        "# changed lockfile\nversion = 4\n\n[[package]]\nname = \"domain\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    let conservative_observer = RustdocLaunchObserver::using_snapshot(snapshot_path);
    let conservative =
        calc_impl_catalog_with_observer(root, track_id, conservative_observer.clone());
    assert_eq!(
        conservative.exit_code, 0,
        "changed implementation inputs must recalculate cleanly: {conservative:?}"
    );
    assert_eq!(
        conservative_observer.launches(),
        1,
        "changed implementation inputs must re-extract through the real adapter"
    );
}
