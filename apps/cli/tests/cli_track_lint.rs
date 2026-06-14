//! Integration tests for `sotp track lint`.
//!
//! Process-level tests that exercise the full composition root:
//! `FsCatalogueLoader` + `RunCatalogueLintInteractor` +
//! `evaluate_catalogue_lint` (domain pure function) wired in
//! `apps/cli/src/commands/track/tddd/lint.rs`.
//!
//! The demo lint rule set uses `FieldNonEmpty { target_field: "invariants" }` for
//! `ValueObject` entries. A `ValueObject` with no `invariants` in its role payload
//! fires a violation. Fixtures that should produce zero violations must include at
//! least one invariant declaration.
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

/// A minimal domain-types.json (v5) with one `value_object` entry that has an
/// invariant declared — satisfies the demo `FieldNonEmpty { "invariants" }` rule
/// (no violation expected).
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
/// fires the demo `FieldNonEmpty { "invariants" }` rule (violation expected).
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

    // Write a domain-types.json whose value_object has an invariant declared
    // → satisfies the FieldNonEmpty "invariants" demo rule → no violation.
    write(&root.join("track/items/test-track/domain-types.json"), CATALOGUE_WITH_INVARIANT);

    let output = run_track_lint(root);
    assert_lint_zero_violations(&output, "catalogue with invariant satisfies FieldNonEmpty rule");
}

// ---------------------------------------------------------------------------
// Test 2: ValueObject without invariants — fires FieldNonEmpty "invariants" violation
// ---------------------------------------------------------------------------

#[test]
fn test_track_lint_value_object_without_invariants_fires_violation() {
    let root_dir = tempfile::tempdir().unwrap();
    let root = root_dir.path();

    write(&root.join("architecture-rules.json"), RULES_JSON);

    // A value_object with no invariants fires the demo FieldNonEmpty "invariants" rule.
    write(&root.join("track/items/test-track/domain-types.json"), CATALOGUE_NO_INVARIANTS);

    let output = run_track_lint(root);

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Violation found → exit code 1.
    assert!(
        !output.status.success(),
        "ValueObject without invariants must exit 1\nstdout: {stdout}\nstderr: {stderr}"
    );
    // Violation line must mention FieldNonEmpty and the entry name.
    assert!(
        stdout.contains("FieldNonEmpty") && stdout.contains("BareValueObject"),
        "stdout must contain violation for FieldNonEmpty on BareValueObject\nstdout: {stdout}"
    );
    // Summary line on stderr must show 1 violation.
    assert!(
        stderr.contains("Found 1 violation(s)"),
        "stderr must report 1 violation\nstderr: {stderr}"
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
    // Error message must mention the unknown layer name.
    assert!(
        stderr.contains("nonexistent-layer"),
        "stderr must mention the unknown layer name\nstderr: {stderr}"
    );
}
