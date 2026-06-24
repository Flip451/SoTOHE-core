//! Tests for the `signal` command family.

#![cfg(test)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::*;
use crate::signal::SignalCompositionRoot;

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
        false,
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
        false,
        Some(SignalGateName::Commit),
        Some(dir.path().to_path_buf()),
    );

    let outcome = result.expect("signal_check_catalog_spec should not return Err");
    assert_eq!(
        outcome.exit_code, 0,
        "chain ② with empty enabled-layer set should pass vacuously: {outcome:?}"
    );
}
