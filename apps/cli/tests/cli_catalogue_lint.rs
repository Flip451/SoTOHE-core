//! Integration tests for `sotp catalogue-lint check-active-track`.
//!
//! Process-level tests that exercise the full composition root:
//! `TrackCompositionRoot::catalogue_lint_check_active_track` in
//! `apps/cli-composition/src/track/tddd.rs`, which aggregates
//! `RunCatalogueLintInteractor` results across every `tddd.enabled` layer
//! declared in `architecture-rules.json` (CN-07: reuses the same
//! active-track-resolution mechanism as `sotp track lint`; no new
//! track-scoping logic).
//!
//! Config file format (schema_version 1):
//! ```json
//! { "schema_version": 1, "rules": [ ... ] }
//! ```
//!
//! Rules are loaded from `.harness/catalogue-lint/config.json` by default, or
//! from the path supplied via `--rules-file`. When no config is found the
//! command exits with code 1 and a user-facing "lint config not found"
//! message on stderr, mirroring `sotp track lint`'s fail-closed behavior.
//!
//! ADR `knowledge/adr/2026-07-01-0004-catalogue-primitive-obsession-guard.md`
//! §D5: blocking from day one, no warn→block staged migration.

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
/// (domain). Matches the format used by `FsCatalogueLoader`. A single
/// declared layer keeps the fixture minimal while still exercising the
/// multi-layer aggregation path (loop over one binding).
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

/// Lint config (schema_version 1) mirroring the two ADR D7 default
/// `ForbidPrimitiveInTypes` rules shipped in `.harness/catalogue-lint/config.json`
/// (T007), scoped to the single `domain` layer this fixture suite declares as
/// `tddd.enabled` (see [`RULES_JSON`]) rather than the shipped file's full
/// 6-layer list.
///
/// The shipped file's exact byte-for-byte content (all 6 layers, all 21
/// non-Dto/Command roles) is separately verified in
/// `libs/infrastructure/tests/catalogue_lint_shipped_config.rs`. This fixture
/// exists to exercise the *semantics* of the two rules end to end through the
/// real CLI, the real `FsLintConfigLoader`, and the real
/// `SynPrimitiveOccurrenceScanner` (ADR D7; CN-08/CN-09/AC-05) — a single
/// declared layer keeps the violation count deterministic (see the test
/// below for why a multi-layer `layers` list is deliberately avoided here).
const FORBID_PRIMITIVE_IN_TYPES_DEFAULT_RULES: &str = r#"{
  "schema_version": 1,
  "rules": [
    {
      "target_roles": [],
      "kind": {
        "ForbidPrimitiveInTypes": {
          "primitives": ["String"],
          "layers": ["domain"],
          "positions": ["result_err"]
        }
      }
    },
    {
      "target_roles": [
        "ValueObject", "Entity", "AggregateRoot", "DomainService", "Specification",
        "Factory", "UseCase", "Interactor", "Query", "ErrorType", "SecondaryAdapter",
        "EventPolicy", "DomainEvent", "CompositionRoot", "PrimaryAdapter",
        "SpecificationPort", "ApplicationService", "SecondaryPort", "Repository",
        "FreeFunction", "UseCaseFunction"
      ],
      "kind": {
        "ForbidPrimitiveInTypes": {
          "primitives": ["String"],
          "layers": ["domain"],
          "positions": ["named_field", "variant_field"]
        }
      }
    }
  ]
}"#;

/// A domain-types.json (v5) with two entries exercising the two default
/// `ForbidPrimitiveInTypes` rules' independence (CN-08):
///
/// - `ErrorTypeFixture` (role `ErrorType`, not `Dto`/`Command`, so CN-09 does
///   not exclude it): a bare `String` named field (`code`) — must violate the
///   named_field/variant_field rule.
/// - `DtoFixture` (role `Dto`, excluded from the named_field/variant_field
///   rule by CN-09): a bare `String` named field (`label`, must NOT violate)
///   and a `Result<i32, String>` named field (`detail`) whose `String` `Err`
///   slot is reclassified to `result_err` by the scanner regardless of the
///   field's own declared position, and must still violate — proving the
///   named_field rule's role exclusion does not reach the result_err rule.
const CATALOGUE_WITH_FORBID_PRIMITIVE_FIXTURES: &str = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {
    "ErrorTypeFixture": {
      "action": "add",
      "role": { "ErrorType": {} },
      "kind": {
        "kind": "struct",
        "shape": { "kind": "plain", "fields": [ { "name": "code", "ty": "String" } ] }
      },
      "methods": [],
      "module_path": "",
      "spec_refs": [],
      "informal_grounds": []
    },
    "DtoFixture": {
      "action": "add",
      "role": { "Dto": {} },
      "kind": {
        "kind": "struct",
        "shape": {
          "kind": "plain",
          "fields": [
            { "name": "label", "ty": "String" },
            { "name": "detail", "ty": "Result<i32, String>" }
          ]
        }
      },
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

/// Shared implementation for `sotp catalogue-lint check-active-track` invocations.
///
/// Builds the fixed argument set for the test workspace rooted at `root`, then
/// appends `--rules-file <path>` when `rules_file` is `Some`.
fn run_catalogue_lint_impl(root: &Path, rules_file: Option<&Path>) -> std::process::Output {
    let mut cmd = sotp_bin();
    cmd.args([
        "catalogue-lint",
        "check-active-track",
        "--track-id",
        "test-track",
        "--workspace-root",
        root.to_str().unwrap(),
    ]);
    if let Some(rf) = rules_file {
        cmd.args(["--rules-file", rf.to_str().unwrap()]);
    }
    cmd.output().unwrap()
}

/// Invoke `sotp catalogue-lint check-active-track` with the fixed args for the
/// test workspace rooted at `root`.
fn run_catalogue_lint(root: &Path) -> std::process::Output {
    run_catalogue_lint_impl(root, None)
}

// ---------------------------------------------------------------------------
// Test 1: Happy path — config present, rules load, no violations, exit 0
// ---------------------------------------------------------------------------

#[test]
fn test_catalogue_lint_check_active_track_no_violations_exits_zero() {
    let root_dir = tempfile::tempdir().unwrap();
    let root = root_dir.path();

    write(&root.join("architecture-rules.json"), RULES_JSON);
    write(&root.join(".harness/catalogue-lint/config.json"), LINT_CONFIG_WITH_INVARIANT_RULE);
    write(&root.join("track/items/test-track/domain-types.json"), CATALOGUE_WITH_INVARIANT);

    let output = run_catalogue_lint(root);

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "catalogue with invariant satisfies FieldNonEmpty rule: expected exit 0\n\
         stdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        stdout.trim().is_empty(),
        "stdout must be empty when there are no violations\nstdout: {stdout}"
    );
    assert!(
        stderr.contains("Found 0 violation(s) across 1 layer(s)"),
        "stderr must report zero violations across the one declared layer\nstderr: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// Test 2: Violation present — exit 1, violation detail on stdout
// ---------------------------------------------------------------------------

#[test]
fn test_catalogue_lint_check_active_track_violation_found_exits_one() {
    let root_dir = tempfile::tempdir().unwrap();
    let root = root_dir.path();

    write(&root.join("architecture-rules.json"), RULES_JSON);
    write(&root.join(".harness/catalogue-lint/config.json"), LINT_CONFIG_WITH_INVARIANT_RULE);
    write(&root.join("track/items/test-track/domain-types.json"), CATALOGUE_NO_INVARIANTS);

    let output = run_catalogue_lint(root);

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
        stderr.contains("Found 1 violation(s) across 1 layer(s)"),
        "stderr must report 1 violation across the one declared layer\nstderr: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// Test 3: Fail-closed — no config file → exit 1 with "lint config not found"
// ---------------------------------------------------------------------------

#[test]
fn test_catalogue_lint_check_active_track_missing_config_exits_one() {
    let root_dir = tempfile::tempdir().unwrap();
    let root = root_dir.path();

    write(&root.join("architecture-rules.json"), RULES_JSON);
    // No .harness/catalogue-lint/config.json written.
    write(&root.join("track/items/test-track/domain-types.json"), CATALOGUE_WITH_INVARIANT);

    let output = run_catalogue_lint(root);

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
}

// ---------------------------------------------------------------------------
// Test 4: Layer catalogue not yet created (Phase 2 in progress) — gate is
// skipped gracefully (exit 0), since `CatalogueLoader::load_all` requires
// every `tddd.enabled` layer's catalogue file to be present.
// ---------------------------------------------------------------------------

#[test]
fn test_catalogue_lint_check_active_track_missing_catalogue_skips_with_exit_zero() {
    let root_dir = tempfile::tempdir().unwrap();
    let root = root_dir.path();

    write(&root.join("architecture-rules.json"), RULES_JSON);
    write(&root.join(".harness/catalogue-lint/config.json"), LINT_CONFIG_WITH_INVARIANT_RULE);
    // No track/items/test-track/domain-types.json written — layer catalogue absent.

    let output = run_catalogue_lint(root);

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "absent catalogue file must skip the gate (exit 0), not fail\n\
         stdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        stderr.contains("catalogue-lint skipped"),
        "stderr must explain the gate was skipped due to an absent catalogue\nstderr: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// Test 5: --rules-file flag overrides the default config location
// ---------------------------------------------------------------------------

#[test]
fn test_catalogue_lint_check_active_track_rules_file_flag_overrides_default_config() {
    let root_dir = tempfile::tempdir().unwrap();
    let root = root_dir.path();

    write(&root.join("architecture-rules.json"), RULES_JSON);

    // Write the override config at a non-default location with a rule that
    // WOULD fire on CATALOGUE_NO_INVARIANTS, proving this path (not some
    // absent default) was used.
    let custom_config = root.join("custom-lint-config.json");
    write(&custom_config, LINT_CONFIG_WITH_INVARIANT_RULE);

    // No default config at .harness/catalogue-lint/config.json.
    write(&root.join("track/items/test-track/domain-types.json"), CATALOGUE_NO_INVARIANTS);

    let output = run_catalogue_lint_impl(root, Some(&custom_config));

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !output.status.success(),
        "--rules-file override must be used for linting (violation expected)\n\
         stdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        stderr.contains("Found 1 violation(s) across 1 layer(s)"),
        "stderr must report 1 violation sourced from the custom rules file\nstderr: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// Test 6: ADR D7 default ForbidPrimitiveInTypes rule semantics — the
// named_field/variant_field rule's Dto/Command role exclusion (CN-09) does
// not reach the result_err rule, which has no role exclusion at all (CN-08),
// exercised end to end through the real CLI (T007;
// knowledge/adr/2026-07-01-0004-catalogue-primitive-obsession-guard.md §D7;
// spec.json AC-05/CN-08/CN-09).
//
// This fixture deliberately declares a single `tddd.enabled` layer (see
// `RULES_JSON`) and trims `FORBID_PRIMITIVE_IN_TYPES_DEFAULT_RULES`'s own
// `layers` list to match, rather than reusing the shipped config's full
// 6-layer list. `catalogue_lint_check_active_track` calls
// `RunCatalogueLintInteractor::execute` once per `tddd.enabled` layer
// binding and sums `violations.len()` across calls, while
// `evaluate_forbid_primitive_in_types` (libs/domain/src/tddd/
// catalogue_linter_eval_primitives.rs) evaluates a rule against every layer
// in *the rule's own* `layers` list on every such call, independent of which
// binding triggered it. With a multi-layer `layers` list this would report
// each real violation once per `tddd.enabled` layer binding rather than
// once — a pre-existing behavior of the shared aggregation/evaluation path
// (not introduced by T007) that is out of scope for a config+test-only task
// to change. A single declared layer sidesteps it entirely: the outer loop
// only calls `execute` once, so no such duplication is possible here.
// ---------------------------------------------------------------------------

#[test]
fn test_catalogue_lint_check_active_track_forbid_primitive_in_types_default_rule_semantics() {
    let root_dir = tempfile::tempdir().unwrap();
    let root = root_dir.path();

    write(&root.join("architecture-rules.json"), RULES_JSON);
    write(
        &root.join("track/items/test-track/domain-types.json"),
        CATALOGUE_WITH_FORBID_PRIMITIVE_FIXTURES,
    );
    let rules_file = root.join("forbid-primitive-rules.json");
    write(&rules_file, FORBID_PRIMITIVE_IN_TYPES_DEFAULT_RULES);

    let output = run_catalogue_lint_impl(root, Some(&rules_file));

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !output.status.success(),
        "ErrorTypeFixture.code (named_field) and DtoFixture.detail (result_err) \
         must both violate\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        stdout.contains("ForbidPrimitiveInTypes on ErrorTypeFixture")
            && stdout.contains("NamedField"),
        "ErrorType (not Dto/Command, CN-09) bare String named field must violate \
         the named_field/variant_field rule\nstdout: {stdout}"
    );
    assert!(
        stdout.contains("ForbidPrimitiveInTypes on DtoFixture") && stdout.contains("ResultErr"),
        "Dto's Result<_, String> field must still violate via result_err even though \
         Dto is excluded from the named_field/variant_field rule (CN-08)\nstdout: {stdout}"
    );
    assert!(
        stderr.contains("Found 2 violation(s) across 1 layer(s)"),
        "expected exactly 2 violations: DtoFixture's bare String field ('label') must \
         NOT violate, since Dto is excluded from the named_field/variant_field rule\n\
         stdout: {stdout}\nstderr: {stderr}"
    );
}
