//! Integration tests for `sotp track lint`.
//!
//! Process-level tests that exercise the full composition root:
//! `FsCatalogueLoader` + `RunCatalogueLintInteractor` +
//! `evaluate_catalogue_lint` (domain pure function) wired in
//! `apps/cli/src/commands/track/tddd/lint.rs`.
//!
//! Note: per-rule evaluation logic is deferred to T014. Until T014, the
//! `evaluate_catalogue_lint` skeleton always returns an empty violation list
//! regardless of the catalogue contents. Tests that relied on specific
//! violations firing are updated to reflect this skeleton behavior.
//!
//! ADR `knowledge/adr/2026-05-25-0000-tddd-pattern-semantics-extension.md`
//! §D15 / D17.

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
/// (domain). Matches the format used by `FsCatalogueLoader` tests in
/// `libs/infrastructure/src/tddd/contract_map_adapter.rs`.
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

/// A minimal domain-types.json (v3) with one `value_object` entry whose
/// `methods` list is empty — satisfies the demo `FieldEmpty` rule
/// (no violation expected).
const CATALOGUE_EMPTY_METHODS: &str = r#"{
  "schema_version": 4,
  "crate_name": "domain",
  "layer": "domain",
  "types": {
    "MyValueObject": {
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

/// A domain-types.json (v3) with one `value_object` entry whose
/// `methods` list is non-empty — fires the demo `FieldEmpty` rule
/// (violation expected).
const CATALOGUE_WITH_METHODS: &str = r#"{
  "schema_version": 4,
  "crate_name": "domain",
  "layer": "domain",
  "types": {
    "MethodfulObject": {
      "action": "add",
      "role": { "ValueObject": {} },
      "kind": {"kind": "struct", "shape": {"kind": "plain"}},
      "methods": [
        {"name": "validate", "receiver": "&self", "params": [], "returns": "()", "is_async": false}
      ],
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

/// Invoke `sotp track lint` with the fixed args for the test workspace rooted at `root`.
///
/// Returns the raw `Output` so each test can assert its scenario-specific expectations.
fn run_track_lint(root: &Path) -> std::process::Output {
    sotp_bin()
        .args([
            "track",
            "lint",
            "--track-id",
            "test-track",
            "--layer-id",
            "domain",
            "--workspace-root",
            root.to_str().unwrap(),
        ])
        .output()
        .unwrap()
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
// Test 1: Happy path — no violations, exit code 0
// ---------------------------------------------------------------------------

#[test]
fn test_track_lint_no_violations_exits_zero() {
    let root_dir = tempfile::tempdir().unwrap();
    let root = root_dir.path();

    // Write architecture-rules.json at workspace root.
    write(&root.join("architecture-rules.json"), RULES_JSON);

    // Write a domain-types.json whose value_object has empty expected_methods
    // → no FieldEmpty violation.
    write(&root.join("track/items/test-track/domain-types.json"), CATALOGUE_EMPTY_METHODS);

    let output = run_track_lint(root);
    assert_lint_zero_violations(&output, "catalogue with no violations");
}

// ---------------------------------------------------------------------------
// Test 2: Catalogue with non-empty methods — skeleton returns no violations
//
// NOTE(T014): The evaluate_catalogue_lint function is a skeleton in Stage 3.
// Per-rule evaluation logic (including FieldEmpty) is deferred to T014.
// Until T014, even a catalogue that would fire a rule returns no violations.
// This test verifies the skeleton path exits 0 with a valid catalogue.
// ---------------------------------------------------------------------------

#[test]
fn test_track_lint_skeleton_with_methods_catalogue_exits_zero() {
    let root_dir = tempfile::tempdir().unwrap();
    let root = root_dir.path();

    write(&root.join("architecture-rules.json"), RULES_JSON);

    // Write a domain-types.json whose value_object has a non-empty methods
    // list. In T014 this would fire FieldEmpty; in the T008 skeleton it does
    // not, so we expect exit 0 and no violations.
    write(&root.join("track/items/test-track/domain-types.json"), CATALOGUE_WITH_METHODS);

    let output = run_track_lint(root);
    assert_lint_zero_violations(&output, "skeleton evaluate_catalogue_lint (no violations yet)");
}

// ---------------------------------------------------------------------------
// Test 3: Invalid layer — exit code 1, error message mentions the layer
// ---------------------------------------------------------------------------

#[test]
fn test_track_lint_invalid_layer_exits_one_with_error_message() {
    let root_dir = tempfile::tempdir().unwrap();
    let root = root_dir.path();

    write(&root.join("architecture-rules.json"), RULES_JSON);

    // Write a valid catalogue so the loader can find the track directory.
    write(&root.join("track/items/test-track/domain-types.json"), CATALOGUE_EMPTY_METHODS);

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
    // Error message must mention the unknown layer name.
    assert!(
        stderr.contains("nonexistent-layer"),
        "stderr must mention the unknown layer name\nstderr: {stderr}"
    );
}
