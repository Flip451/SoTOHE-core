//! Integration tests for `sotp track lint`.
//!
//! Process-level tests that exercise the full composition root:
//! `FsCatalogueLoader` + `InMemoryCatalogueLinter` +
//! `RunCatalogueLintInteractor` wired in `apps/cli/src/commands/track/tddd/lint.rs`.
//!
//! ADR `tddd-struct-kind-uniformization-and-catalogue-linter` §S3 / IN-05 / AC-05.

#![allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::Path;
use std::process::Command;

fn sotp_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sotp"))
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

/// A minimal domain-types.json with one `value_object` entry whose
/// `expected_methods` is empty — satisfies the demo `FieldEmpty` rule
/// (no violation expected).
const CATALOGUE_EMPTY_METHODS: &str = r#"{
  "schema_version": 2,
  "type_definitions": [
    {
      "name": "MyValueObject",
      "description": "A value object with no methods",
      "approved": true,
      "action": "add",
      "kind": "value_object",
      "expected_members": [],
      "expected_methods": []
    }
  ]
}"#;

/// A domain-types.json with one `value_object` entry whose
/// `expected_methods` is non-empty — fires the demo `FieldEmpty` rule
/// (violation expected).
const CATALOGUE_WITH_METHODS: &str = r#"{
  "schema_version": 2,
  "type_definitions": [
    {
      "name": "MethodfulObject",
      "description": "A value object with behavioral methods",
      "approved": true,
      "action": "add",
      "kind": "value_object",
      "expected_methods": [
        {"name": "validate", "receiver": "&self", "params": [], "returns": "bool", "is_async": false}
      ]
    }
  ]
}"#;

/// Write a file, creating all parent directories as needed.
fn write(path: &Path, content: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
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

    let output = sotp_bin()
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
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "expected exit 0 for catalogue with no violations\nstdout: {stdout}\nstderr: {stderr}"
    );
    // No violation lines on stdout.
    assert!(
        stdout.trim().is_empty(),
        "stdout must be empty when there are no violations\nstdout: {stdout}"
    );
    // Summary line on stderr.
    assert!(
        stderr.contains("Found 0 violation(s)"),
        "stderr must contain 'Found 0 violation(s)'\nstderr: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// Test 2: Violation detected — exit code 1, entry name in stdout
// ---------------------------------------------------------------------------

#[test]
fn test_track_lint_with_violation_exits_one_and_prints_entry_name() {
    let root_dir = tempfile::tempdir().unwrap();
    let root = root_dir.path();

    write(&root.join("architecture-rules.json"), RULES_JSON);

    // Write a domain-types.json whose value_object has a non-empty
    // expected_methods → fires the FieldEmpty demo rule.
    write(&root.join("track/items/test-track/domain-types.json"), CATALOGUE_WITH_METHODS);

    let output = sotp_bin()
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
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "expected exit 1 when violations exist\nstdout: {stdout}\nstderr: {stderr}"
    );
    // Entry name must appear in stdout.
    assert!(
        stdout.contains("MethodfulObject"),
        "stdout must contain the violating entry name\nstdout: {stdout}"
    );
    // Summary line must mention at least 1 violation.
    assert!(
        stderr.contains("Found 1 violation(s)"),
        "stderr must contain 'Found 1 violation(s)'\nstderr: {stderr}"
    );
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
