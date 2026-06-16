//! Integration tests for `sotp track lint`.
//!
//! Process-level tests that exercise the full composition root:
//! `FsCatalogueLoader` + `FsLintConfigLoader` + `RunCatalogueLintInteractor` +
//! `evaluate_catalogue_lint` (domain pure function) wired in
//! `apps/cli-composition/src/track/tddd.rs`.
//!
//! Config file format (schema_version 1):
//! ```json
//! { "schema_version": 1, "rules": [ ... ] }
//! ```
//!
//! Rules are loaded from `.harness/catalogue-lint/config.json` by default, or
//! from the path supplied via `--rules-file`. When no config is found the
//! command exits with code 1 and a user-facing "lint config not found" message
//! on stderr (D19 fail-closed).
//!
//! ADR `knowledge/adr/2026-05-25-0000-tddd-pattern-semantics-extension.md`
//! §D15 / D17 / D19.

#![allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::Path;
use std::process::Command;

fn sotp_bin() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_sotp"));
    // Disable telemetry in spawned binary so integration tests never write to
    // the real track/items/ tree (CN-06 / AC-07).  The #[cfg(test)] guard only
    // applies to in-process code; spawned binaries are full production processes.
    cmd.env("SOTP_TELEMETRY", "0");
    cmd
}

// ---------------------------------------------------------------------------
// Fixture helpers
// ---------------------------------------------------------------------------

/// Minimal `architecture-rules.json` with a single `tddd.enabled` layer
/// (domain). Matches the format used by `FsCatalogueLoader`.
const RULES_JSON: &str = r#"{
  "version": 2,
  "layers": [
    {
      "crate": "domain",
      "path": "libs/domain",
      "may_depend_on": [],
      "deny_reason": "no reverse dep",
      "tddd": {
        "enabled": true,
        "catalogue_file": "domain-types.json",
        "schema_export": {"method": "rustdoc", "targets": ["domain"]}
      }
    }
  ]
}"#;

/// Lint config (schema_version 1) with a `FieldNonEmpty { "invariants" }` rule
/// for `ValueObject` entries.
const LINT_CONFIG_WITH_INVARIANT_RULE: &str = r#"{
  "schema_version": 1,
  "rules": [
    {
      "target_roles": ["ValueObject"],
      "kind": { "FieldNonEmpty": { "target_field": "invariants" } }
    }
  ]
}"#;

/// Lint config (schema_version 1) with no rules — always produces zero violations.
const LINT_CONFIG_EMPTY_RULES: &str = r#"{
  "schema_version": 1,
  "rules": []
}"#;

/// A minimal domain-types.json (v5) with one `value_object` entry that has an
/// invariant declared — satisfies `FieldNonEmpty { "invariants" }` (no violation).
const CATALOGUE_WITH_INVARIANT: &str = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {
    "MyValueObject": {
      "action": "add",
      "role": {
        "ValueObject": {
          "invariants": [
            { "name": "is_valid", "predicate": { "SelfMethod": "is_valid" } }
          ]
        }
      },
      "kind": {"kind": "struct", "shape": {"kind": "plain"}},
      "methods": [
        {"name": "is_valid", "receiver": "&self", "params": [], "returns": "bool"}
      ],
      "module_path": "",
      "spec_refs": [],
      "informal_grounds": []
    }
  },
  "traits": {},
  "functions": {}
}"#;

/// A domain-types.json (v5) with one `value_object` entry that has no invariants —
/// fires `FieldNonEmpty { "invariants" }` (violation expected).
const CATALOGUE_NO_INVARIANTS: &str = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {
    "BareValueObject": {
      "action": "add",
      "role": { "ValueObject": {} },
      "kind": {"kind": "struct", "shape": {"kind": "plain"}},
      "methods": [],
      "module_path": "",
      "spec_refs": [],
      "informal_grounds": []
    }
  },
  "traits": {},
  "functions": {}
}"#;

/// Write a file, creating all parent directories as needed.
fn write(path: &Path, content: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

/// Shared implementation for `sotp track lint` invocations.
///
/// Builds the fixed argument set for the test workspace rooted at `root`, then
/// appends `--rules-file <path>` when `rules_file` is `Some`.
fn run_track_lint_impl(root: &Path, rules_file: Option<&Path>) -> std::process::Output {
    let mut cmd = sotp_bin();
    cmd.args([
        "track",
        "lint",
        "--track-id",
        "test-track",
        "--layer-id",
        "domain",
        "--workspace-root",
        root.to_str().unwrap(),
    ]);
    if let Some(rf) = rules_file {
        cmd.args(["--rules-file", rf.to_str().unwrap()]);
    }
    cmd.output().unwrap()
}

/// Invoke `sotp track lint` with the fixed args for the test workspace rooted at `root`.
///
/// Returns the raw `Output` so each test can assert its scenario-specific expectations.
fn run_track_lint(root: &Path) -> std::process::Output {
    run_track_lint_impl(root, None)
}

/// Invoke `sotp track lint` with an explicit `--rules-file` override.
fn run_track_lint_with_rules_file(root: &Path, rules_file: &Path) -> std::process::Output {
    run_track_lint_impl(root, Some(rules_file))
}

/// Assert the common zero-violation exit contract: exit 0, empty stdout, and the
/// "Found 0 violation(s)" summary line on stderr.
fn assert_lint_zero_violations(output: &std::process::Output, context_msg: &str) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "{context_msg}: expected exit 0\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        stdout.trim().is_empty(),
        "{context_msg}: stdout must be empty when there are no violations\nstdout: {stdout}"
    );
    assert!(
        stderr.contains("Found 0 violation(s)"),
        "{context_msg}: stderr must contain 'Found 0 violation(s)'\nstderr: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// Test 1: Happy path — config present, rules load, no violations, exit 0
// ---------------------------------------------------------------------------

#[test]
fn test_track_lint_no_violations_exits_zero() {
    let root_dir = tempfile::tempdir().unwrap();
    let root = root_dir.path();

    // Write architecture-rules.json at workspace root.
    write(&root.join("architecture-rules.json"), RULES_JSON);

    // Write the default lint config at the expected location.
    write(&root.join(".harness/catalogue-lint/config.json"), LINT_CONFIG_WITH_INVARIANT_RULE);

    // Write a catalogue whose ValueObject has invariants → no violation.
    write(&root.join("track/items/test-track/domain-types.json"), CATALOGUE_WITH_INVARIANT);

    let output = run_track_lint(root);
    assert_lint_zero_violations(&output, "catalogue with invariant satisfies FieldNonEmpty rule");
}

// ---------------------------------------------------------------------------
// Test 2: Fail-closed — no config file → exit 1 with "lint config not found"
// ---------------------------------------------------------------------------

#[test]
fn test_track_lint_missing_config_exits_one_with_config_missing_message() {
    let root_dir = tempfile::tempdir().unwrap();
    let root = root_dir.path();

    write(&root.join("architecture-rules.json"), RULES_JSON);
    // No .harness/catalogue-lint/config.json written.
    write(&root.join("track/items/test-track/domain-types.json"), CATALOGUE_WITH_INVARIANT);

    let output = run_track_lint(root);

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !output.status.success(),
        "missing config must exit 1\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        stderr.contains("lint config not found"),
        "stderr must contain 'lint config not found'\nstderr: {stderr}"
    );
    // The error message must include the path so the user knows where to put the file.
    assert!(
        stderr.contains(".harness/catalogue-lint/config.json"),
        "stderr must mention the config path\nstderr: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// Test 3: --rules-file flag overrides the default config location
// ---------------------------------------------------------------------------

#[test]
fn test_track_lint_rules_file_flag_overrides_default_config() {
    let root_dir = tempfile::tempdir().unwrap();
    let root = root_dir.path();

    write(&root.join("architecture-rules.json"), RULES_JSON);

    // Write the override config at a non-default location with empty rules.
    let custom_config = root.join("custom-lint-config.json");
    write(&custom_config, LINT_CONFIG_EMPTY_RULES);

    // No default config at .harness/catalogue-lint/config.json.
    write(&root.join("track/items/test-track/domain-types.json"), CATALOGUE_NO_INVARIANTS);

    // With empty rules the catalogue that would otherwise fire FieldNonEmpty produces
    // zero violations — confirming the custom config was used, not the (absent) default.
    let output = run_track_lint_with_rules_file(root, &custom_config);
    assert_lint_zero_violations(
        &output,
        "--rules-file with empty rules produces zero violations regardless of catalogue",
    );
}

// ---------------------------------------------------------------------------
// Test 4: Invalid JSON in config — exit 1, error mentions parse failure
// ---------------------------------------------------------------------------

#[test]
fn test_track_lint_invalid_json_config_exits_one_with_error() {
    let root_dir = tempfile::tempdir().unwrap();
    let root = root_dir.path();

    write(&root.join("architecture-rules.json"), RULES_JSON);

    // Write malformed JSON as the lint config.
    write(
        &root.join(".harness/catalogue-lint/config.json"),
        r#"{ "schema_version": 1, "rules": [ INVALID JSON }"#,
    );

    write(&root.join("track/items/test-track/domain-types.json"), CATALOGUE_WITH_INVARIANT);

    let output = run_track_lint(root);

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !output.status.success(),
        "invalid JSON config must exit non-zero\nstdout: {stdout}\nstderr: {stderr}"
    );
    // The error message should indicate a parse / lint failure.
    assert!(
        stderr.contains("catalogue lint failed")
            || stderr.contains("parse")
            || stderr.contains("failed"),
        "stderr must describe the parse failure\nstderr: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// Test 5: schema_version mismatch — exit 1, error mentions version
// ---------------------------------------------------------------------------

#[test]
fn test_track_lint_schema_version_mismatch_exits_one_with_error() {
    let root_dir = tempfile::tempdir().unwrap();
    let root = root_dir.path();

    write(&root.join("architecture-rules.json"), RULES_JSON);

    // Write a config with unsupported schema_version.
    write(
        &root.join(".harness/catalogue-lint/config.json"),
        r#"{ "schema_version": 99, "rules": [] }"#,
    );

    write(&root.join("track/items/test-track/domain-types.json"), CATALOGUE_WITH_INVARIANT);

    let output = run_track_lint(root);

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !output.status.success(),
        "schema_version mismatch must exit non-zero\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        stderr.contains("catalogue lint failed")
            || stderr.contains("schema_version")
            || stderr.contains("mismatch"),
        "stderr must describe the version mismatch\nstderr: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// Test 6: Config present and violations found — exit 1, violation on stdout
// ---------------------------------------------------------------------------

#[test]
fn test_track_lint_value_object_without_invariants_fires_violation() {
    let root_dir = tempfile::tempdir().unwrap();
    let root = root_dir.path();

    write(&root.join("architecture-rules.json"), RULES_JSON);

    // Config with FieldNonEmpty rule for invariants.
    write(&root.join(".harness/catalogue-lint/config.json"), LINT_CONFIG_WITH_INVARIANT_RULE);

    // Catalogue with ValueObject that has no invariants → fires the rule.
    write(&root.join("track/items/test-track/domain-types.json"), CATALOGUE_NO_INVARIANTS);

    let output = run_track_lint(root);

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !output.status.success(),
        "ValueObject without invariants must exit 1\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        stdout.contains("FieldNonEmpty") && stdout.contains("BareValueObject"),
        "stdout must contain violation for FieldNonEmpty on BareValueObject\nstdout: {stdout}"
    );
    assert!(
        stderr.contains("Found 1 violation(s)"),
        "stderr must report 1 violation\nstderr: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// Test 7: Invalid layer — exit 1, error message mentions the layer
// ---------------------------------------------------------------------------

#[test]
fn test_track_lint_invalid_layer_exits_one_with_error_message() {
    let root_dir = tempfile::tempdir().unwrap();
    let root = root_dir.path();

    write(&root.join("architecture-rules.json"), RULES_JSON);

    // Config must be present so we don't fail on ConfigMissing before InvalidLayer.
    write(&root.join(".harness/catalogue-lint/config.json"), LINT_CONFIG_EMPTY_RULES);

    write(&root.join("track/items/test-track/domain-types.json"), CATALOGUE_WITH_INVARIANT);

    let output = sotp_bin()
        .args([
            "track",
            "lint",
            "--track-id",
            "test-track",
            "--layer-id",
            "nonexistent-layer",
            "--workspace-root",
            root.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "expected exit 1 for unknown layer\nstderr: {stderr}");
    assert!(
        stderr.contains("nonexistent-layer"),
        "stderr must mention the unknown layer name\nstderr: {stderr}"
    );
}
