//! Tests for [`plan_artifact_refs`] (split out to keep the main module under the 200-400 line guideline).

use tempfile::TempDir;

use super::*;

// -----------------------------------------------------------------------
// Fixtures
// -----------------------------------------------------------------------

/// Minimal spec.json (v2) with no ref fields → all refs are empty.
const MINIMAL_SPEC: &str = r#"{
  "schema_version": 2,
  "version": "1.0",
  "title": "Test Track",
  "scope": {
    "in_scope": [
      {"id": "IN-01", "text": "requirement one"}
    ],
    "out_of_scope": []
  }
}"#;

/// Writes a file at `dir / relative_path`, creating parent dirs as needed.
fn write_file(dir: &Path, relative: &str, content: &str) {
    let path = dir.join(relative);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(&path, content).unwrap();
}

/// Build a fake repo layout rooted at `tmp`:
///   tmp/.git/               ← git root marker (makes `fallback_trusted_root` return `tmp`)
///   tmp/track/items/<id>/   ← track_dir
///   tmp/knowledge/adr/      ← referenced by adr_refs
fn setup_repo(tmp: &Path, track_id: &str) -> PathBuf {
    // Create a `.git` directory at the repo root so that `fallback_trusted_root`
    // (called inside `resolve_trusted_root` when the spec path is outside the
    // current working tree) returns `tmp` as the trusted_root.  This matches the
    // layout assumed by `resolve_path`, which now uses `trusted_root` directly as
    // the repo root instead of a hard-coded 3-parent walk.
    std::fs::create_dir_all(tmp.join(".git")).unwrap();
    let track_dir = tmp.join("track").join("items").join(track_id);
    std::fs::create_dir_all(&track_dir).unwrap();
    track_dir
}

fn write_spec_with_adr_ref(track_dir: &Path, adr_file: &str, anchor: &str) {
    let spec = format!(
        r#"{{
  "schema_version": 2,
  "version": "1.0",
  "title": "T",
  "scope": {{
    "in_scope": [
      {{
        "id": "IN-01",
        "text": "req",
        "adr_refs": [{{"file": "{adr_file}", "anchor": "{anchor}"}}]
      }}
    ],
    "out_of_scope": []
  }}
}}"#
    );
    write_file(track_dir, "spec.json", &spec);
}

fn write_domain_catalogue_with_spec_ref(track_dir: &Path, file: &str, anchor: &str) {
    let catalogue = format!(
        r#"{{
  "schema_version": 4,
  "crate_name": "domain",
  "layer": "domain",
  "types": {{
    "MyType": {{
      "action": "add",
      "role": {{ "ValueObject": {{}} }},
      "kind": {{ "kind": "struct", "shape": {{ "kind": "unit" }} }},
      "spec_refs": [
        {{
          "file": "{file}",
          "anchor": "{anchor}"
        }}
      ]
    }}
  }},
  "traits": {{}},
  "functions": {{}}
}}"#
    );
    write_file(track_dir, "domain-types.json", &catalogue);
}

// -----------------------------------------------------------------------
// Happy-path: all refs valid
// -----------------------------------------------------------------------

#[test]
fn test_no_spec_json_passes() {
    let tmp = TempDir::new().unwrap();
    let track_dir = setup_repo(tmp.path(), "test-track");
    // No spec.json at all → pass immediately.
    let outcome = verify(&track_dir);
    assert!(outcome.is_ok(), "absent spec.json must pass: {:?}", outcome);
}

#[test]
fn test_minimal_spec_with_no_refs_passes() {
    let tmp = TempDir::new().unwrap();
    let track_dir = setup_repo(tmp.path(), "test-track");
    write_file(&track_dir, "spec.json", MINIMAL_SPEC);
    let outcome = verify(&track_dir);
    assert!(outcome.is_ok(), "spec with no refs must pass: {:?}", outcome);
}

#[test]
fn test_spec_with_valid_adr_refs_pass() {
    for (track_id, adr_file, anchor, adr_body) in [
        ("test-track", "knowledge/adr/2026-04-19-1242.md", "D2.1", "# ADR\n## D2.1 Section\n"),
        ("t001-test", "knowledge/adr/test-adr.md", "D1", "# ADR\n"),
    ] {
        let tmp = TempDir::new().unwrap();
        let track_dir = setup_repo(tmp.path(), track_id);
        let adr = format!(
            "---\nadr_id: test-adr\ndecisions:\n  - id: {anchor}\n    status: proposed\n---\n{adr_body}",
        );
        write_file(tmp.path(), adr_file, &adr);

        write_spec_with_adr_ref(&track_dir, adr_file, anchor);
        let outcome = verify(&track_dir);
        assert!(outcome.is_ok(), "valid adr_ref with anchor '{anchor}' must pass: {:?}", outcome);
    }
}

#[test]
fn test_spec_with_missing_adr_file_reports_error() {
    let tmp = TempDir::new().unwrap();
    let track_dir = setup_repo(tmp.path(), "test-track");

    write_spec_with_adr_ref(&track_dir, "knowledge/adr/missing.md", "D2.1");
    let outcome = verify(&track_dir);
    assert!(outcome.has_errors(), "missing ADR file must produce error: {:?}", outcome);
    assert!(
        outcome.findings()[0].message().contains("missing.md"),
        "error must mention the missing file"
    );
}

#[test]
fn test_spec_with_missing_convention_file_reports_error() {
    let tmp = TempDir::new().unwrap();
    let track_dir = setup_repo(tmp.path(), "test-track");

    let spec = r#"{
  "schema_version": 2,
  "version": "1.0",
  "title": "T",
  "scope": {
    "in_scope": [
      {
        "id": "IN-01",
        "text": "req",
        "convention_refs": [{"file": "knowledge/conventions/missing.md", "anchor": "intro"}]
      }
    ],
    "out_of_scope": []
  }
}"#;
    write_file(&track_dir, "spec.json", spec);
    let outcome = verify(&track_dir);
    assert!(outcome.has_errors(), "missing convention file must produce error");
}

#[test]
fn test_spec_with_informal_grounds_only_passes() {
    // informal_grounds have no file path — no file existence check needed.
    let tmp = TempDir::new().unwrap();
    let track_dir = setup_repo(tmp.path(), "test-track");

    let spec = r#"{
  "schema_version": 2,
  "version": "1.0",
  "title": "T",
  "scope": {
    "in_scope": [
      {
        "id": "IN-01",
        "text": "req",
        "informal_grounds": [{"kind": "feedback", "summary": "user directive to defer"}]
      }
    ],
    "out_of_scope": []
  }
}"#;
    write_file(&track_dir, "spec.json", spec);
    let outcome = verify(&track_dir);
    assert!(outcome.is_ok(), "informal_grounds only must pass: {:?}", outcome);
}

#[test]
fn test_malformed_spec_json_reports_error() {
    let tmp = TempDir::new().unwrap();
    let track_dir = setup_repo(tmp.path(), "test-track");
    write_file(&track_dir, "spec.json", "not valid json");
    let outcome = verify(&track_dir);
    assert!(outcome.has_errors(), "malformed spec.json must produce error");
}

// -----------------------------------------------------------------------
// SpecRef anchor resolution
// -----------------------------------------------------------------------

#[test]
fn test_spec_ref_with_valid_anchor_passes() {
    let tmp = TempDir::new().unwrap();
    let track_dir = setup_repo(tmp.path(), "test-track");

    // Write spec.json with one in_scope element.
    write_file(&track_dir, "spec.json", MINIMAL_SPEC);

    write_domain_catalogue_with_spec_ref(&track_dir, "track/items/test-track/spec.json", "IN-01");

    let outcome = verify(&track_dir);
    assert!(outcome.is_ok(), "valid SpecRef must pass: {:?}", outcome);
}

#[test]
fn test_spec_ref_with_missing_spec_file_reports_error() {
    let tmp = TempDir::new().unwrap();
    let track_dir = setup_repo(tmp.path(), "test-track");
    write_file(&track_dir, "spec.json", MINIMAL_SPEC);

    write_domain_catalogue_with_spec_ref(&track_dir, "track/items/nonexistent/spec.json", "IN-01");

    let outcome = verify(&track_dir);
    assert!(outcome.has_errors(), "missing spec file must produce error: {:?}", outcome);
}

#[test]
fn test_spec_ref_with_unresolved_anchor_reports_error() {
    let tmp = TempDir::new().unwrap();
    let track_dir = setup_repo(tmp.path(), "test-track");
    write_file(&track_dir, "spec.json", MINIMAL_SPEC);

    write_domain_catalogue_with_spec_ref(&track_dir, "track/items/test-track/spec.json", "IN-99");

    let outcome = verify(&track_dir);
    assert!(outcome.has_errors(), "unresolved anchor must produce error: {:?}", outcome);
    assert!(
        outcome.findings()[0].message().contains("IN-99"),
        "error must mention the missing anchor"
    );
}

#[test]
fn test_empty_spec_refs_on_catalogue_entry_passes() {
    let tmp = TempDir::new().unwrap();
    let track_dir = setup_repo(tmp.path(), "test-track");
    write_file(&track_dir, "spec.json", MINIMAL_SPEC);

    // Catalogue entry with no spec_refs → nothing to validate.
    let catalogue = r#"{
  "schema_version": 4,
  "crate_name": "domain",
  "layer": "domain",
  "types": {
    "MyType": {
      "action": "add",
      "role": { "ValueObject": {} },
      "kind": { "kind": "struct", "shape": { "kind": "unit" } }
    }
  },
  "traits": {},
  "functions": {}
}"#;
    write_file(&track_dir, "domain-types.json", catalogue);

    let outcome = verify(&track_dir);
    assert!(outcome.is_ok(), "empty spec_refs must pass: {:?}", outcome);
}

#[test]
fn test_malformed_catalogue_json_reports_error() {
    let tmp = TempDir::new().unwrap();
    let track_dir = setup_repo(tmp.path(), "test-track");
    write_file(&track_dir, "spec.json", MINIMAL_SPEC);
    write_file(&track_dir, "domain-types.json", "not valid json");

    let outcome = verify(&track_dir);
    assert!(outcome.has_errors(), "malformed catalogue must produce error: {:?}", outcome);
}

// -----------------------------------------------------------------------
// canonical_json helper
// -----------------------------------------------------------------------

#[test]
fn test_canonical_json_sorts_object_keys() {
    let v: serde_json::Value = serde_json::from_str(r#"{"z": 1, "a": 2, "m": 3}"#).unwrap();
    let canonical = canonical_json(&v);
    // Keys must appear in alphabetical order: a, m, z
    let pos_a = canonical.find("\"a\"").unwrap();
    let pos_m = canonical.find("\"m\"").unwrap();
    let pos_z = canonical.find("\"z\"").unwrap();
    assert!(pos_a < pos_m, "a must come before m");
    assert!(pos_m < pos_z, "m must come before z");
}

#[test]
fn test_canonical_json_is_compact() {
    let v: serde_json::Value = serde_json::from_str(r#"{"key": "value"}"#).unwrap();
    let canonical = canonical_json(&v);
    assert!(!canonical.contains('\n'), "canonical form must not contain newlines");
    assert!(!canonical.contains("  "), "canonical form must not contain extra spaces");
}

#[test]
fn test_canonical_json_sha256_is_stable() {
    // Same input must produce the same hash every time.
    let json = r#"{"id":"IN-01","text":"requirement one"}"#;
    let h1 = canonical_json_sha256(json);
    let h2 = canonical_json_sha256(json);
    assert_eq!(h1, h2);
    assert_eq!(h1.len(), 64);
}

#[test]
fn test_canonical_json_sha256_differs_for_different_input() {
    let h1 = canonical_json_sha256(r#"{"id":"IN-01"}"#);
    let h2 = canonical_json_sha256(r#"{"id":"IN-02"}"#);
    assert_ne!(h1, h2);
}

// -----------------------------------------------------------------------
// Security: symlink guard and path-traversal rejection
// -----------------------------------------------------------------------

/// A ref path containing a `..` component must be rejected even when the
/// normalized result would remain inside the repo root.
///
/// Rationale: lexical normalization collapses `..` before the symlink guard
/// runs. A crafted ref like `dir/symlink/../target.md` normalizes to
/// `dir/target.md`, hiding the symlink component from `reject_symlinks_below`.
/// Rejecting `..` components in the input prevents this bypass.
#[test]
fn test_spec_with_adr_ref_containing_parent_dir_reports_error() {
    let tmp = TempDir::new().unwrap();
    let track_dir = setup_repo(tmp.path(), "test-track");

    // Create a real ADR file so the ref would pass if `..` were accepted.
    write_file(tmp.path(), "knowledge/adr/2026-04-19-1242.md", "# ADR\n");

    write_spec_with_adr_ref(&track_dir, "knowledge/../knowledge/adr/2026-04-19-1242.md", "D2.1");
    let outcome = verify(&track_dir);
    assert!(!outcome.is_ok(), "ref path with `..` must be rejected as invalid: {:?}", outcome);
}

/// A symlinked spec.json (symlink to a regular file) must be rejected by
/// the symlink guard rather than silently passing because `is_file()` returns
/// true for symlinks.
///
/// A symlink-to-directory would cause `is_file()` to return `false`, which
/// (before the fix) triggered an early `pass()` return, skipping all checks.
#[cfg(unix)]
#[test]
fn test_symlinked_spec_json_is_rejected() {
    let tmp = TempDir::new().unwrap();
    let track_dir = setup_repo(tmp.path(), "test-track");

    // Write a real spec.json somewhere outside the track dir.
    let real_spec = tmp.path().join("real_spec.json");
    std::fs::write(&real_spec, MINIMAL_SPEC).unwrap();

    // Create spec.json as a symlink pointing to the real file.
    let symlink_path = track_dir.join("spec.json");
    std::os::unix::fs::symlink(&real_spec, &symlink_path).unwrap();

    let outcome = verify(&track_dir);
    assert!(!outcome.is_ok(), "symlinked spec.json must be rejected: {:?}", outcome);
}

/// A symlinked spec.json pointing to a directory must also be rejected
/// (before the fix, `is_file()` → false would silently return `pass()`).
#[cfg(unix)]
#[test]
fn test_symlinked_spec_json_pointing_to_dir_is_rejected() {
    let tmp = TempDir::new().unwrap();
    let track_dir = setup_repo(tmp.path(), "test-track");

    // Create a directory target for the symlink.
    let dir_target = tmp.path().join("some_dir");
    std::fs::create_dir_all(&dir_target).unwrap();

    // spec.json → directory: `is_file()` returns false here, but the
    // symlink guard must still catch it.
    let symlink_path = track_dir.join("spec.json");
    std::os::unix::fs::symlink(&dir_target, &symlink_path).unwrap();

    let outcome = verify(&track_dir);
    assert!(!outcome.is_ok(), "symlink-to-directory spec.json must be rejected: {:?}", outcome);
}

// -----------------------------------------------------------------------
// T011: task-coverage enforcement tests
// -----------------------------------------------------------------------

/// When task-coverage.json is absent, emit a warning (not an error).
/// The outcome is still ok() because warnings don't fail CI in T011.
#[test]
fn test_absent_task_coverage_emits_warning_not_error() {
    let tmp = TempDir::new().unwrap();
    let track_dir = setup_repo(tmp.path(), "test-track");
    write_file(&track_dir, "spec.json", MINIMAL_SPEC);
    // No task-coverage.json
    let outcome = verify(&track_dir);
    assert!(outcome.is_ok(), "absent task-coverage.json must not fail CI: {:?}", outcome);
    let has_warning =
        outcome.findings().iter().any(|f| f.severity() == domain::verify::Severity::Warning);
    assert!(has_warning, "absent task-coverage.json must emit a warning: {:?}", outcome);
}

/// in_scope requirement without a task_ref entry in task-coverage.json → error.
#[test]
fn test_in_scope_missing_task_ref_reports_coverage_violation() {
    let tmp = TempDir::new().unwrap();
    let track_dir = setup_repo(tmp.path(), "test-track");
    write_file(&track_dir, "spec.json", MINIMAL_SPEC);
    // task-coverage.json with empty in_scope section
    write_file(
        &track_dir,
        "task-coverage.json",
        r#"{"schema_version": 1, "in_scope": {}, "out_of_scope": {}, "constraints": {}, "acceptance_criteria": {}}"#,
    );
    let outcome = verify(&track_dir);
    assert!(outcome.has_errors(), "uncovered in_scope must produce error: {:?}", outcome);
    let has_coverage_error = outcome
        .findings()
        .iter()
        .any(|f| f.message().contains("coverage violation") && f.message().contains("IN-01"));
    assert!(has_coverage_error, "error must mention IN-01: {:?}", outcome);
}

/// Stale element id in task-coverage (not in spec.json) → referential integrity error.
#[test]
fn test_stale_element_id_in_task_coverage_reports_integrity_error() {
    let tmp = TempDir::new().unwrap();
    let track_dir = setup_repo(tmp.path(), "test-track");
    write_file(&track_dir, "spec.json", MINIMAL_SPEC);
    // task-coverage.json references IN-99 which doesn't exist in spec.json
    write_file(
        &track_dir,
        "task-coverage.json",
        r#"{"schema_version": 1, "in_scope": {"IN-01": ["T001"], "IN-99": ["T001"]}, "out_of_scope": {}, "constraints": {}, "acceptance_criteria": {}}"#,
    );
    // impl-plan.json with T001
    write_file(
        &track_dir,
        "impl-plan.json",
        r#"{"schema_version": 1, "tasks": [{"id": "T001", "description": "task", "status": "todo"}], "plan": {"summary": [], "sections": [{"id": "S1", "title": "S", "description": [], "task_ids": ["T001"]}]}}"#,
    );
    let outcome = verify(&track_dir);
    assert!(outcome.has_errors(), "stale element id must produce error: {:?}", outcome);
    let has_integrity_error = outcome.findings().iter().any(|f| f.message().contains("IN-99"));
    assert!(has_integrity_error, "error must mention IN-99: {:?}", outcome);
}

/// Stale TaskId in task-coverage (not in impl-plan.json) → impl-plan integrity error.
#[test]
fn test_stale_task_id_in_task_coverage_reports_implplan_integrity_error() {
    let tmp = TempDir::new().unwrap();
    let track_dir = setup_repo(tmp.path(), "test-track");
    write_file(&track_dir, "spec.json", MINIMAL_SPEC);
    // task-coverage.json references T999 which doesn't exist in impl-plan.json
    write_file(
        &track_dir,
        "task-coverage.json",
        r#"{"schema_version": 1, "in_scope": {"IN-01": ["T999"]}, "out_of_scope": {}, "constraints": {}, "acceptance_criteria": {}}"#,
    );
    // impl-plan.json with T001 only (T999 absent)
    write_file(
        &track_dir,
        "impl-plan.json",
        r#"{"schema_version": 1, "tasks": [{"id": "T001", "description": "task", "status": "todo"}], "plan": {"summary": [], "sections": [{"id": "S1", "title": "S", "description": [], "task_ids": ["T001"]}]}}"#,
    );
    let outcome = verify(&track_dir);
    assert!(outcome.has_errors(), "stale TaskId must produce error: {:?}", outcome);
    let has_task_error = outcome
        .findings()
        .iter()
        .any(|f| f.message().contains("T999") && f.message().contains("impl-plan.json"));
    assert!(has_task_error, "error must mention T999 and impl-plan.json: {:?}", outcome);
}

/// task-coverage.json present but impl-plan.json absent → fail-closed error.
///
/// The verifier must not silently skip task_ref integrity when impl-plan.json is
/// missing. Dangling task references would otherwise pass undetected (e.g. after
/// an accidental delete or partial commit).
#[test]
fn test_task_coverage_present_impl_plan_absent_fails_closed() {
    let tmp = TempDir::new().unwrap();
    let track_dir = setup_repo(tmp.path(), "test-track");
    write_file(&track_dir, "spec.json", MINIMAL_SPEC);
    // task-coverage.json is present with a task ref
    write_file(
        &track_dir,
        "task-coverage.json",
        r#"{"schema_version": 1, "in_scope": {"IN-01": ["T001"]}, "out_of_scope": {}, "constraints": {}, "acceptance_criteria": {}}"#,
    );
    // impl-plan.json is intentionally absent
    let outcome = verify(&track_dir);
    assert!(
        outcome.has_errors(),
        "task-coverage present + impl-plan absent must produce error: {:?}",
        outcome
    );
    let has_fail_closed_error = outcome.findings().iter().any(|f| {
        f.message().contains("impl-plan.json is missing")
            || f.message().contains("impl-plan.json")
                && f.message().contains("task-coverage.json is present")
    });
    assert!(
        has_fail_closed_error,
        "error must mention both task-coverage.json and impl-plan.json absence: {:?}",
        outcome
    );
}

/// Fully covered track (IN-01 → T001 in both coverage and impl-plan) passes.
#[test]
fn test_fully_covered_track_passes() {
    let tmp = TempDir::new().unwrap();
    let track_dir = setup_repo(tmp.path(), "test-track");
    write_file(&track_dir, "spec.json", MINIMAL_SPEC);
    write_file(
        &track_dir,
        "task-coverage.json",
        r#"{"schema_version": 1, "in_scope": {"IN-01": ["T001"]}, "out_of_scope": {}, "constraints": {}, "acceptance_criteria": {}}"#,
    );
    write_file(
        &track_dir,
        "impl-plan.json",
        r#"{"schema_version": 1, "tasks": [{"id": "T001", "description": "task", "status": "todo"}], "plan": {"summary": [], "sections": [{"id": "S1", "title": "S", "description": [], "task_ids": ["T001"]}]}}"#,
    );
    let outcome = verify(&track_dir);
    assert!(outcome.is_ok(), "fully covered track must pass: {:?}", outcome);
}

// -----------------------------------------------------------------------
// T011: canonical-block suspicion detection tests
// -----------------------------------------------------------------------

/// A fenced code block with >10 lines and no example marker → warning.
#[test]
fn test_canonical_block_over_10_lines_emits_warning() {
    let tmp = TempDir::new().unwrap();
    let track_dir = setup_repo(tmp.path(), "test-track");
    write_file(&track_dir, "spec.json", MINIMAL_SPEC);

    // Create a plan.md with a long fenced code block (12 lines inside).
    let plan_md = "# Plan\n\n```rust\nline1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\nline11\nline12\n```\n";
    write_file(&track_dir, "plan.md", plan_md);

    let outcome = verify(&track_dir);
    let has_block_warning = outcome.findings().iter().any(|f| {
        f.severity() == domain::verify::Severity::Warning
            && f.message().contains("canonical-block suspicion")
    });
    assert!(has_block_warning, "long code block must emit canonical-block warning: {:?}", outcome);
    // Must NOT be an error (warning only).
    assert!(outcome.is_ok(), "canonical-block warning must not fail CI: {:?}", outcome);
}

/// A fenced code block with an example marker in the preceding line → no warning.
#[test]
fn test_canonical_block_with_preceding_example_marker_no_warning() {
    let tmp = TempDir::new().unwrap();
    let track_dir = setup_repo(tmp.path(), "test-track");
    write_file(&track_dir, "spec.json", MINIMAL_SPEC);

    // plan.md with example marker on the line before the fence
    let plan_md = "# Plan\n\n<!-- example: long block -->\n```rust\nline1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\nline11\nline12\n```\n";
    write_file(&track_dir, "plan.md", plan_md);

    let outcome = verify(&track_dir);
    let has_block_warning =
        outcome.findings().iter().any(|f| f.message().contains("canonical-block suspicion"));
    assert!(
        !has_block_warning,
        "example-marked block must not emit canonical-block warning: {:?}",
        outcome
    );
}

/// A fenced code block with an example marker only inside the body (no preceding-line or
/// fence-open marker) → warning IS emitted. Inner-line scanning is intentionally excluded
/// to avoid false negatives from block content that happens to contain `example:` text.
#[test]
fn test_canonical_block_with_inner_only_example_marker_still_warns() {
    let tmp = TempDir::new().unwrap();
    let track_dir = setup_repo(tmp.path(), "test-track");
    write_file(&track_dir, "spec.json", MINIMAL_SPEC);

    // plan.md with example: marker inside the fence but NOT in preceding line or info string.
    // Inner markers alone do not suppress the canonical-block warning.
    let plan_md = "# Plan\n\n```rust\n// example\nline1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\nline11\n```\n";
    write_file(&track_dir, "plan.md", plan_md);

    let outcome = verify(&track_dir);
    let has_block_warning =
        outcome.findings().iter().any(|f| f.message().contains("canonical-block suspicion"));
    assert!(
        has_block_warning,
        "inner-only example marker must not suppress canonical-block warning: {:?}",
        outcome
    );
}

/// A fenced code block with an example marker in the opening fence info string → no warning.
///
/// Example: "```example" or "```example my-block" — the info string contains "example".
#[test]
fn test_canonical_block_with_fence_open_example_marker_no_warning() {
    let tmp = TempDir::new().unwrap();
    let track_dir = setup_repo(tmp.path(), "test-track");
    write_file(&track_dir, "spec.json", MINIMAL_SPEC);

    // plan.md with example marker inline on the opening fence line (info string)
    let plan_md = "# Plan\n\n```example\nline1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\nline11\nline12\n```\n";
    write_file(&track_dir, "plan.md", plan_md);

    let outcome = verify(&track_dir);
    let has_block_warning =
        outcome.findings().iter().any(|f| f.message().contains("canonical-block suspicion"));
    assert!(
        !has_block_warning,
        "block with fence-open example marker must not emit warning: {:?}",
        outcome
    );
}

/// A fenced code block preceded by the canonical ADR Q3 marker
/// `<!-- illustrative, non-canonical -->` → no warning.
#[test]
fn test_canonical_block_with_adr_illustrative_marker_no_warning() {
    let tmp = TempDir::new().unwrap();
    let track_dir = setup_repo(tmp.path(), "test-track");
    write_file(&track_dir, "spec.json", MINIMAL_SPEC);

    // plan.md with the canonical ADR Q3 marker on the line before the fence.
    let plan_md = "# Plan\n\n<!-- illustrative, non-canonical -->\n```rust\nline1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\nline11\nline12\n```\n";
    write_file(&track_dir, "plan.md", plan_md);

    let outcome = verify(&track_dir);
    let has_block_warning =
        outcome.findings().iter().any(|f| f.message().contains("canonical-block suspicion"));
    assert!(
        !has_block_warning,
        "ADR Q3 illustrative marker must suppress canonical-block warning: {:?}",
        outcome
    );
}

/// A fenced code block with exactly 10 lines (not >10) → no warning.
#[test]
fn test_canonical_block_exactly_10_lines_no_warning() {
    let tmp = TempDir::new().unwrap();
    let track_dir = setup_repo(tmp.path(), "test-track");
    write_file(&track_dir, "spec.json", MINIMAL_SPEC);

    // plan.md with exactly 10 lines inside the fence (boundary: not >10)
    let plan_md = "# Plan\n\n```\nline1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\n```\n";
    write_file(&track_dir, "plan.md", plan_md);

    let outcome = verify(&track_dir);
    let has_block_warning =
        outcome.findings().iter().any(|f| f.message().contains("canonical-block suspicion"));
    assert!(!has_block_warning, "block with exactly 10 lines must not emit warning: {:?}", outcome);
}

// -----------------------------------------------------------------------
// Schema version rejection tests
// -----------------------------------------------------------------------

/// schema_version 3 catalogue — must now be rejected with an unsupported schema version error.
const V3_CATALOGUE_DOMAIN: &str = r#"{
  "schema_version": 3,
  "crate_name": "domain",
  "layer": "domain",
  "types": {
    "MyType": {
      "action": "add",
      "role": { "ValueObject": {} },
      "kind": { "kind": "struct", "shape": { "kind": "plain" } },
      "docs": "A simple value object."
    }
  },
  "traits": {},
  "functions": {}
}"#;

#[test]
fn test_v3_catalogue_produces_error_finding() {
    // schema_version 3 is no longer supported (bumped to 4 in T001).
    // The codec must reject it with an unsupported schema version error.
    let tmp = TempDir::new().unwrap();
    let track_dir = setup_repo(tmp.path(), "test-track");
    write_file(&track_dir, "spec.json", MINIMAL_SPEC);
    write_file(&track_dir, "domain-types.json", V3_CATALOGUE_DOMAIN);

    let outcome = verify(&track_dir);

    let has_error =
        outcome.findings().iter().any(|f| f.severity() == domain::verify::Severity::Error);
    assert!(
        has_error,
        "schema_version 3 catalogue must produce an error finding; findings: {:?}",
        outcome.findings()
    );
}

// -----------------------------------------------------------------------
// T001 (D3 / AC-05): AdrRef anchor existence in plan-artifact-refs verifier
// -----------------------------------------------------------------------

/// ADR with front-matter but the referenced anchor is absent from decisions → error.
#[test]
fn test_adr_ref_with_anchor_absent_from_decisions_reports_error() {
    let tmp = TempDir::new().unwrap();
    let track_dir = setup_repo(tmp.path(), "t001-test");

    // ADR front-matter has D1 only; spec references D99 which does not exist.
    write_file(
        tmp.path(),
        "knowledge/adr/test-adr.md",
        "---\nadr_id: test-adr\ndecisions:\n  - id: D1\n    status: proposed\n---\n# ADR\n",
    );

    write_spec_with_adr_ref(&track_dir, "knowledge/adr/test-adr.md", "D99");
    let outcome = verify(&track_dir);
    assert!(
        outcome.has_errors(),
        "anchor absent from decisions[] must produce error: {:?}",
        outcome
    );
    assert!(
        outcome.findings().iter().any(|f| f.message().contains("D99")),
        "error must mention the missing anchor 'D99': {:?}",
        outcome
    );
}

/// ADR file exists but has no YAML front-matter → fail-closed error.
#[test]
fn test_adr_ref_with_no_frontmatter_reports_error() {
    let tmp = TempDir::new().unwrap();
    let track_dir = setup_repo(tmp.path(), "t001-test");

    // ADR has no front-matter block.
    write_file(tmp.path(), "knowledge/adr/test-adr.md", "# ADR\n\n## D1\n\nSome content.\n");

    write_spec_with_adr_ref(&track_dir, "knowledge/adr/test-adr.md", "D1");
    let outcome = verify(&track_dir);
    assert!(
        outcome.has_errors(),
        "ADR without front-matter must produce error (fail-closed): {:?}",
        outcome
    );
    assert!(
        outcome.findings().iter().any(|f| f.message().contains("front-matter")),
        "error must mention front-matter: {:?}",
        outcome
    );
}

/// ADR with front-matter but empty decisions array → anchor not found → error.
#[test]
fn test_adr_ref_with_empty_decisions_array_reports_error() {
    let tmp = TempDir::new().unwrap();
    let track_dir = setup_repo(tmp.path(), "t001-test");

    // ADR front-matter has decisions: [] (empty).
    write_file(
        tmp.path(),
        "knowledge/adr/test-adr.md",
        "---\nadr_id: test-adr\ndecisions: []\n---\n# ADR\n",
    );

    write_spec_with_adr_ref(&track_dir, "knowledge/adr/test-adr.md", "D1");
    let outcome = verify(&track_dir);
    assert!(
        outcome.has_errors(),
        "empty decisions array must produce anchor-not-found error: {:?}",
        outcome
    );
}

/// ADR front-matter has `decisions` field missing entirely → typed-deserialization error.
#[test]
fn test_adr_ref_with_frontmatter_missing_decisions_field_reports_error() {
    let tmp = TempDir::new().unwrap();
    let track_dir = setup_repo(tmp.path(), "t001-test");

    // Front-matter is valid YAML but no `decisions` key.
    write_file(
        tmp.path(),
        "knowledge/adr/test-adr.md",
        "---\nadr_id: test-adr\ntitle: Some ADR\n---\n# ADR\n",
    );

    write_spec_with_adr_ref(&track_dir, "knowledge/adr/test-adr.md", "D1");
    let outcome = verify(&track_dir);
    assert!(
        outcome.has_errors(),
        "missing decisions field must produce typed-deserialization error: {:?}",
        outcome
    );
}

/// ADR front-matter `decisions` entries have invalid types → typed-deserialization error.
#[test]
fn test_adr_ref_with_invalid_decisions_entry_type_reports_typed_deserialization_error() {
    let tmp = TempDir::new().unwrap();
    let track_dir = setup_repo(tmp.path(), "t001-test");

    // decisions entries are bare strings, not maps with an `id` field.
    write_file(
        tmp.path(),
        "knowledge/adr/test-adr.md",
        "---\nadr_id: test-adr\ndecisions:\n  - \"not a map\"\n---\n# ADR\n",
    );

    write_spec_with_adr_ref(&track_dir, "knowledge/adr/test-adr.md", "D1");
    let outcome = verify(&track_dir);
    assert!(
        outcome.has_errors(),
        "type-mismatch in decisions entries must produce error: {:?}",
        outcome
    );
}
