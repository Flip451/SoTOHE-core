//! Tests for [`latest_track`] (split out to keep the main module under the 200-400 line guideline).

use std::fs;

use tempfile::TempDir;

use super::*;

// ---- helpers ----

fn write_file(root: &Path, rel: &str, content: &str) {
    let path = root.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, content).unwrap();
}

/// Build a minimal valid v5 identity-only metadata JSON for a track.
///
/// v5 has no `status`, `tasks`, or `plan` fields. Status is derived from
/// `impl-plan.json` + `status_override` at runtime.
fn make_metadata_v5(id: &str, branch_json: &str, status_override_json: &str) -> String {
    format!(
        r#"{{"schema_version":5,"id":"{id}","title":"Track {id}","created_at":"2026-01-01T00:00:00+00:00","updated_at":"2026-01-15T00:00:00+00:00","branch":{branch_json}{status_override_json}}}"#
    )
}

/// Minimal valid `impl-plan.json` content for test fixtures that need an
/// activated (branched) track without caring about the plan contents.
const MINIMAL_IMPL_PLAN_JSON: &str = r#"{"schema_version":1,"plan":{"summary":[],"sections":[]}}"#;

fn setup_track(root: &Path, id: &str, branch: Option<&str>) {
    let dir = root.join(TRACK_ITEMS_DIR).join(id);
    fs::create_dir_all(&dir).unwrap();
    let branch_json = match branch {
        Some(b) => format!(r#""{b}""#),
        None => "null".to_owned(),
    };
    let meta = make_metadata_v5(id, &branch_json, "");
    fs::write(dir.join("metadata.json"), meta).unwrap();
}

fn setup_track_planned(root: &Path, id: &str) {
    // v5 planning-only: no branch, no impl-plan.json → status derives to "planned".
    setup_track(root, id, None);
}

fn setup_track_with_branch(root: &Path, id: &str) {
    let branch = format!("track/{id}");
    setup_track(root, id, Some(&branch));
    // Write a minimal impl-plan.json so tests that assert on the derived
    // track status (`derive_track_status`) exercise the normal activated
    // path rather than the Planned fallback used for pre-impl-plan state.
    let dir = root.join(TRACK_ITEMS_DIR).join(id);
    fs::write(dir.join("impl-plan.json"), MINIMAL_IMPL_PLAN_JSON).unwrap();
}

fn setup_complete_track(root: &Path, id: &str, branch: Option<&str>) {
    setup_track(root, id, branch);
    // Write a minimal impl-plan.json so the derived track status drives
    // the artifact-validation test path rather than the pre-impl-plan
    // Planned fallback.
    if branch.is_some() {
        let dir = root.join(TRACK_ITEMS_DIR).join(id);
        fs::write(dir.join("impl-plan.json"), MINIMAL_IMPL_PLAN_JSON).unwrap();
    }
    write_file(
        root,
        &format!("{TRACK_ITEMS_DIR}/{id}/spec.md"),
        "# Spec\n\nThis is a complete specification with real content.\n",
    );
    write_file(
        root,
        &format!("{TRACK_ITEMS_DIR}/{id}/plan.md"),
        "# Plan\n\n- [ ] Task one\n- [x] Task two done\n",
    );
}

// ---- test cases ----

#[test]
fn test_no_tracks_passes() {
    let tmp = TempDir::new().unwrap();
    let outcome = verify(tmp.path());
    assert!(outcome.is_ok(), "empty repo should pass: {outcome:?}");
}

#[test]
fn test_complete_v5_track_passes() {
    let tmp = TempDir::new().unwrap();
    // v5 track with branch (in-progress derived from impl-plan) and all artifacts.
    setup_complete_track(tmp.path(), "my-feature", Some("track/my-feature"));
    let outcome = verify(tmp.path());
    assert!(outcome.is_ok(), "complete v5 track should pass: {:#?}", outcome.findings());
}

#[test]
fn test_legacy_v3_track_is_skipped() {
    // v3 metadata must be skipped by latest_track.rs. With only a v3 track
    // in the repo, no track is selected and verify() returns pass.
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().join(TRACK_ITEMS_DIR).join("legacy-feat");
    fs::create_dir_all(&dir).unwrap();
    let meta = r#"{"schema_version":3,"id":"legacy-feat","title":"Legacy","status":"in_progress","created_at":"2026-01-01T00:00:00+00:00","updated_at":"2026-01-15T00:00:00+00:00","branch":"track/legacy-feat","tasks":[{"id":"t1","description":"Task","status":"todo"}],"plan":{"summary":[],"sections":[{"id":"s1","title":"S","description":[],"task_ids":["t1"]}]}}"#;
    fs::write(dir.join("metadata.json"), meta).unwrap();

    let outcome = verify(tmp.path());
    assert!(
        outcome.is_ok(),
        "v3 tracks must be skipped; no v5 track → pass: {:#?}",
        outcome.findings()
    );
}

#[test]
fn test_missing_spec_fails() {
    let tmp = TempDir::new().unwrap();
    setup_track_with_branch(tmp.path(), "feat-a");
    // plan.md present, spec.md absent
    write_file(
        tmp.path(),
        &format!("{TRACK_ITEMS_DIR}/feat-a/plan.md"),
        "# Plan\n\n- [ ] Task one\n",
    );

    let outcome = verify(tmp.path());
    assert!(outcome.has_errors(), "missing spec.md should fail");
    let msgs: Vec<_> = outcome.findings().iter().map(|f| f.message()).collect();
    assert!(
        msgs.iter().any(|m| m.contains("spec.md")),
        "error should mention spec.md, got: {msgs:?}"
    );
}

#[test]
fn test_placeholder_in_spec_fails() {
    let tmp = TempDir::new().unwrap();
    setup_track_with_branch(tmp.path(), "feat-b");
    write_file(
        tmp.path(),
        &format!("{TRACK_ITEMS_DIR}/feat-b/spec.md"),
        "# Spec\n\nTODO: fill in details\n",
    );
    write_file(
        tmp.path(),
        &format!("{TRACK_ITEMS_DIR}/feat-b/plan.md"),
        "# Plan\n\n- [ ] Task one\n",
    );

    let outcome = verify(tmp.path());
    assert!(outcome.has_errors(), "placeholder in spec should fail");
    let msgs: Vec<_> = outcome.findings().iter().map(|f| f.message()).collect();
    assert!(
        msgs.iter().any(|m| m.contains("placeholder")),
        "error should mention placeholder, got: {msgs:?}"
    );
}

#[test]
fn test_placeholder_in_fenced_block_ignored() {
    let tmp = TempDir::new().unwrap();
    setup_track_with_branch(tmp.path(), "feat-c");
    write_file(
        tmp.path(),
        &format!("{TRACK_ITEMS_DIR}/feat-c/spec.md"),
        "# Spec\n\nReal content here.\n\n```\nTODO: this is inside a code block\n```\n",
    );
    write_file(
        tmp.path(),
        &format!("{TRACK_ITEMS_DIR}/feat-c/plan.md"),
        "# Plan\n\n- [ ] Task one\n",
    );

    let outcome = verify(tmp.path());
    assert!(
        outcome.is_ok(),
        "TODO inside fenced block should be ignored: {:#?}",
        outcome.findings()
    );
}

#[test]
fn test_selection_priority_v5_active_branch_highest() {
    // v5 + branch + not-done => priority 2
    assert_eq!(selection_priority("in_progress", Some("track/feat"), 5), 2);
    // v5 + planned + no branch => priority 1
    assert_eq!(selection_priority("planned", None, 5), 1);
    // v5 + done + branch => priority 0
    assert_eq!(selection_priority("done", Some("track/feat"), 5), 0);
    // Active branch beats branchless planned
    assert!(
        selection_priority("in_progress", Some("track/feat"), 5)
            > selection_priority("planned", None, 5)
    );
}

#[test]
fn test_v5_branchless_planned_valid() {
    let tmp = TempDir::new().unwrap();
    // v5 planning-only: no branch, no impl-plan.json → status derives to "planned".
    setup_track_planned(tmp.path(), "planning-track");
    write_file(
        tmp.path(),
        &format!("{TRACK_ITEMS_DIR}/planning-track/spec.md"),
        "# Spec\n\nPlanning specification with real content.\n",
    );
    write_file(
        tmp.path(),
        &format!("{TRACK_ITEMS_DIR}/planning-track/plan.md"),
        "# Plan\n\n- [ ] Task one\n",
    );

    let outcome = verify(tmp.path());
    assert!(outcome.is_ok(), "v5 branchless planned track should pass: {:#?}", outcome.findings());
}

#[test]
fn test_archived_track_in_archive_dir_skipped() {
    let tmp = TempDir::new().unwrap();
    // Track under track/archive/ is skipped by path, no markdown files needed.
    let archive_dir = tmp.path().join(TRACK_ARCHIVE_DIR).join("old-feat");
    fs::create_dir_all(&archive_dir).unwrap();
    // Even v5 metadata in the archive directory is skipped by path.
    let meta = make_metadata_v5("old-feat", r#""track/old-feat""#, "");
    fs::write(archive_dir.join("metadata.json"), meta).unwrap();

    let outcome = verify(tmp.path());
    assert!(
        outcome.is_ok(),
        "archived track under archive dir should be skipped: {:#?}",
        outcome.findings()
    );
}

#[test]
fn test_corrupt_impl_plan_surfaces_error() {
    // A present but corrupt impl-plan.json must NOT silently be treated as
    // absent. The verifier should surface an error so that a broken track
    // is not silently selected as the latest track.
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().join(TRACK_ITEMS_DIR).join("corrupt-track");
    fs::create_dir_all(&dir).unwrap();
    let meta = make_metadata_v5("corrupt-track", r#""track/corrupt-track""#, "");
    fs::write(dir.join("metadata.json"), meta).unwrap();
    // Write invalid JSON to impl-plan.json.
    fs::write(dir.join("impl-plan.json"), "NOT VALID JSON").unwrap();

    let outcome = verify(tmp.path());
    assert!(
        outcome.has_errors(),
        "corrupt impl-plan.json must surface an error: {:#?}",
        outcome.findings()
    );
}

#[test]
fn test_phase0_track_with_no_artifacts_passes() {
    // T001 (phase-aware verify gates): when impl-plan.json is absent
    // (Phase 0 / 1 / 2 — pre-implementation), spec/plan existence checks
    // are skipped. A branch-materialized v5 track that only has metadata
    // must therefore pass even without spec.md / spec.json / plan.md.
    // This implements "file existence = phase status" from
    // knowledge/conventions/workflow-ceremony-minimization.md and
    // supersedes the prior Planned-fallback-with-artifact-validation
    // behavior introduced by T025.
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().join(TRACK_ITEMS_DIR).join("phase0-track");
    fs::create_dir_all(&dir).unwrap();
    let meta = make_metadata_v5("phase0-track", r#""track/phase0-track""#, "");
    fs::write(dir.join("metadata.json"), meta).unwrap();
    // Intentionally omit impl-plan.json, spec.json, spec.md, plan.md
    // (Phase 0 state — only metadata.json + branch exists).

    let outcome = verify(tmp.path());
    assert!(
        outcome.is_ok(),
        "Phase 0 track (impl-plan.json absent) must pass — spec/plan checks skipped: {:#?}",
        outcome.findings()
    );
    // Regression guard: activation-invariant error from T025 must not appear.
    let msgs: Vec<_> = outcome.findings().iter().map(|f| f.message()).collect();
    assert!(
        !msgs.iter().any(|m| m.contains("activation invariant")),
        "activation-invariant error must not fire, got: {msgs:?}"
    );
}

#[test]
fn test_missing_schema_version_is_not_silently_skipped() {
    // A metadata.json without `schema_version` must NOT be treated as a
    // legacy v2/v3 track and silently skipped. It should fall through to
    // v5 processing so that errors are surfaced (fail-closed).
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().join(TRACK_ITEMS_DIR).join("no-version-track");
    fs::create_dir_all(&dir).unwrap();
    // metadata.json without schema_version — must not be silently skipped.
    let meta = r#"{"id":"no-version-track","branch":"track/no-version-track","title":"No Version","created_at":"2026-01-01T00:00:00Z","updated_at":"2026-01-15T00:00:00Z"}"#;
    fs::write(dir.join("metadata.json"), meta).unwrap();
    // No impl-plan.json — if the track were processed as v5, status derives
    // to "planned" and the verifier proceeds to check artifacts. Without the
    // required spec.md / plan.md the outcome must be an error (not a silent pass).
    write_file(
        tmp.path(),
        &format!("{TRACK_ITEMS_DIR}/no-version-track/plan.md"),
        "# Plan\n\n- [ ] Task one\n",
    );
    // No spec.md — should produce a "missing spec" error (not pass silently).
    let outcome = verify(tmp.path());
    assert!(
        outcome.has_errors(),
        "track with missing schema_version must not be silently skipped: {:#?}",
        outcome.findings()
    );
}

#[test]
fn test_schema_version_overflow_u32_is_rejected() {
    // u32::MAX + 1 == 4294967296; as u32 would silently wrap to 0 (which is
    // < 5 and would cause the track to be silently skipped as legacy).
    // The fix using u32::try_from must surface an explicit error instead.
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().join(TRACK_ITEMS_DIR).join("overflow-track");
    fs::create_dir_all(&dir).unwrap();
    // schema_version = 4294967298 (u32::MAX + 3) — overflows u32, would wrap to 2
    // with `as u32` and be silently skipped as a legacy v2 track.
    let meta = r#"{"schema_version":4294967298,"id":"overflow-track","title":"Overflow Track","created_at":"2026-01-01T00:00:00Z","updated_at":"2026-01-15T00:00:00Z","branch":"track/overflow-track"}"#;
    fs::write(dir.join("metadata.json"), meta).unwrap();

    let outcome = verify(tmp.path());
    assert!(
        outcome.has_errors(),
        "schema_version overflow must produce an error (fail-closed): {:#?}",
        outcome.findings()
    );
    let msgs: Vec<_> = outcome.findings().iter().map(|f| f.message()).collect();
    assert!(
        msgs.iter().any(|m| m.contains("overflows u32")),
        "error should mention overflow, got: {msgs:?}"
    );
}

#[test]
fn test_parse_updated_at_z_suffix() {
    let secs_z = parse_updated_at("2026-01-15T00:00:00Z").unwrap();
    let secs_offset = parse_updated_at("2026-01-15T00:00:00+00:00").unwrap();
    assert_eq!(secs_z, secs_offset);
}

// ---- spec.json artifact tests ----

const VALID_SPEC_JSON: &str = r#"{
  "schema_version": 2,
  "version": "1.0",
  "title": "Feature",
  "scope": { "in_scope": [], "out_of_scope": [] }
}"#;

/// Helper: set up a track with spec.json instead of spec.md.
fn setup_complete_track_with_spec_json(root: &Path, id: &str) {
    setup_track_with_branch(root, id);
    write_file(root, &format!("{TRACK_ITEMS_DIR}/{id}/spec.json"), VALID_SPEC_JSON);
    write_file(root, &format!("{TRACK_ITEMS_DIR}/{id}/plan.md"), "# Plan\n\n- [ ] Task one\n");
}

#[test]
fn test_spec_json_instead_of_spec_md_passes() {
    let tmp = TempDir::new().unwrap();
    setup_complete_track_with_spec_json(tmp.path(), "feat-json");
    let outcome = verify(tmp.path());
    assert!(
        outcome.is_ok(),
        "track with valid spec.json and no spec.md should pass: {:#?}",
        outcome.findings()
    );
}

#[test]
fn test_spec_json_and_spec_md_both_present_uses_spec_json() {
    let tmp = TempDir::new().unwrap();
    setup_complete_track_with_spec_json(tmp.path(), "feat-both");
    // Also write a spec.md with placeholder content that would fail markdown checks
    write_file(
        tmp.path(),
        &format!("{TRACK_ITEMS_DIR}/feat-both/spec.md"),
        "TODO: placeholder only\n",
    );
    let outcome = verify(tmp.path());
    // spec.json is preferred; valid spec.json should pass regardless of spec.md content
    assert!(outcome.is_ok(), "spec.json takes priority over spec.md: {:#?}", outcome.findings());
}

#[test]
fn test_invalid_spec_json_fails() {
    let tmp = TempDir::new().unwrap();
    setup_track_with_branch(tmp.path(), "feat-bad-json");
    write_file(tmp.path(), &format!("{TRACK_ITEMS_DIR}/feat-bad-json/spec.json"), "not valid json");
    write_file(
        tmp.path(),
        &format!("{TRACK_ITEMS_DIR}/feat-bad-json/plan.md"),
        "# Plan\n\n- [ ] Task one\n",
    );
    let outcome = verify(tmp.path());
    assert!(outcome.has_errors(), "invalid spec.json should fail");
    let msgs: Vec<_> = outcome.findings().iter().map(|f| f.message()).collect();
    assert!(
        msgs.iter().any(|m| m.contains("spec.json")),
        "error should mention spec.json, got: {msgs:?}"
    );
}

#[test]
fn test_missing_spec_md_and_spec_json_fails() {
    let tmp = TempDir::new().unwrap();
    setup_track_with_branch(tmp.path(), "feat-no-spec");
    // Neither spec.md nor spec.json present
    write_file(
        tmp.path(),
        &format!("{TRACK_ITEMS_DIR}/feat-no-spec/plan.md"),
        "# Plan\n\n- [ ] Task one\n",
    );
    let outcome = verify(tmp.path());
    assert!(outcome.has_errors(), "missing both spec.md and spec.json should fail");
    let msgs: Vec<_> = outcome.findings().iter().map(|f| f.message()).collect();
    assert!(
        msgs.iter().any(|m| m.contains("spec.md")),
        "error should mention spec.md, got: {msgs:?}"
    );
}
