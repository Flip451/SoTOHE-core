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

/// Build a minimal valid v6 identity-only metadata JSON for a track.
///
/// v6 has no `status`, `tasks`, or `plan` fields. Status is derived from
/// `impl-plan.json` + `status_override` at runtime. v6 adds the required
/// `branch_strategy_snapshot` field.
fn make_metadata_v6(id: &str, branch_json: &str, status_override_json: &str) -> String {
    make_metadata_v6_with_updated_at(
        id,
        branch_json,
        "2026-01-15T00:00:00+00:00",
        status_override_json,
    )
}

fn make_metadata_v6_with_updated_at(
    id: &str,
    branch_json: &str,
    updated_at: &str,
    status_override_json: &str,
) -> String {
    format!(
        r#"{{"schema_version":6,"id":"{id}","title":"Track {id}","created_at":"2026-01-01T00:00:00+00:00","updated_at":"{updated_at}","branch":{branch_json},"branch_strategy_snapshot":{{"base_branch":"main","merge_target":"main","merge_method":"squash"}}{status_override_json}}}"#
    )
}

/// Minimal valid `impl-plan.json` content for test fixtures that need an
/// activated (branched) track without caring about the plan contents.
const MINIMAL_IMPL_PLAN_JSON: &str = r#"{"schema_version":1,"plan":{"summary":[],"sections":[]}}"#;

const IN_PROGRESS_IMPL_PLAN_JSON: &str = r#"{
  "schema_version": 1,
  "tasks": [{"id": "T001", "description": "Build feature", "status": "in_progress"}],
  "plan": {"summary": [], "sections": [{"id": "S1", "title": "Build", "description": [], "task_ids": ["T001"]}]}
}"#;

fn setup_track(root: &Path, id: &str, branch: Option<&str>) {
    let dir = root.join(TRACK_ITEMS_DIR).join(id);
    fs::create_dir_all(&dir).unwrap();
    let branch_json = match branch {
        Some(b) => format!(r#""{b}""#),
        None => "null".to_owned(),
    };
    let meta = make_metadata_v6(id, &branch_json, "");
    fs::write(dir.join("metadata.json"), meta).unwrap();
}

fn setup_track_planned(root: &Path, id: &str) {
    // v6 track with no branch and no impl-plan.json → status derives to "planned".
    // Branchless tracks can no longer be created (plan-only lane removed); this
    // helper is retained for legacy-compatibility tests.
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
fn test_complete_v6_track_passes() {
    let tmp = TempDir::new().unwrap();
    // v6 track with branch (in-progress derived from impl-plan) and all artifacts.
    setup_complete_track(tmp.path(), "my-feature", Some("track/my-feature"));
    let outcome = verify(tmp.path());
    assert!(outcome.is_ok(), "complete v6 track should pass: {:#?}", outcome.findings());
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
        "v3 tracks must be skipped; no v6 track → pass: {:#?}",
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
fn test_selection_priority_matches_registry_planned_last_order() {
    // In-progress / blocked / cancelled sort ahead of planned, matching registry Current Focus.
    assert_eq!(selection_priority("planned"), 1);
    assert_eq!(selection_priority("in_progress"), 2);
    assert_eq!(selection_priority("blocked"), 2);
    assert_eq!(selection_priority("cancelled"), 2);
    // done / archived => priority 0
    assert_eq!(selection_priority("done"), 0);
    assert_eq!(selection_priority("archived"), 0);
    // Unrecognized status => priority 0 (treated as inactive)
    assert_eq!(selection_priority("unknown"), 0);
}

#[test]
fn test_latest_track_prefers_in_progress_over_newer_planned() {
    let tmp = TempDir::new().unwrap();

    let planned_dir = tmp.path().join(TRACK_ITEMS_DIR).join("newer-planned");
    fs::create_dir_all(&planned_dir).unwrap();
    let planned_meta = make_metadata_v6_with_updated_at(
        "newer-planned",
        r#""track/newer-planned""#,
        "2026-01-20T00:00:00+00:00",
        "",
    );
    fs::write(planned_dir.join("metadata.json"), planned_meta).unwrap();

    let in_progress_dir = tmp.path().join(TRACK_ITEMS_DIR).join("older-in-progress");
    fs::create_dir_all(&in_progress_dir).unwrap();
    let in_progress_meta = make_metadata_v6_with_updated_at(
        "older-in-progress",
        r#""track/older-in-progress""#,
        "2026-01-10T00:00:00+00:00",
        "",
    );
    fs::write(in_progress_dir.join("metadata.json"), in_progress_meta).unwrap();
    fs::write(in_progress_dir.join("impl-plan.json"), IN_PROGRESS_IMPL_PLAN_JSON).unwrap();

    let latest = latest_track_dir(tmp.path()).unwrap().unwrap();
    assert_eq!(latest.file_name().and_then(|name| name.to_str()), Some("older-in-progress"));
}

#[test]
fn test_latest_track_tie_breaks_same_timestamp_by_track_id_ascending() {
    let tmp = TempDir::new().unwrap();

    for id in ["track-b", "track-a"] {
        let track_dir = tmp.path().join(TRACK_ITEMS_DIR).join(id);
        fs::create_dir_all(&track_dir).unwrap();
        let metadata = make_metadata_v6_with_updated_at(
            id,
            &format!(r#""track/{id}""#),
            "2026-01-10T00:00:00+00:00",
            "",
        );
        fs::write(track_dir.join("metadata.json"), metadata).unwrap();
        fs::write(track_dir.join("impl-plan.json"), IN_PROGRESS_IMPL_PLAN_JSON).unwrap();
    }

    let latest = latest_track_dir(tmp.path()).unwrap().unwrap();
    assert_eq!(latest.file_name().and_then(|name| name.to_str()), Some("track-a"));
}

#[test]
fn test_v6_branchless_planned_valid() {
    let tmp = TempDir::new().unwrap();
    // v6 track with no branch (legacy state): no impl-plan.json → status derives to "planned".
    // Plan-only lane is removed; this test verifies that branchless data does not
    // break verification (impl-plan.json absent → pre-Phase-3 → artifact checks are skipped).
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
    assert!(outcome.is_ok(), "v6 branchless planned track should pass: {:#?}", outcome.findings());
}

#[test]
fn test_archived_track_in_archive_dir_skipped() {
    let tmp = TempDir::new().unwrap();
    // Track under track/archive/ is skipped by path, no markdown files needed.
    let archive_dir = tmp.path().join(TRACK_ARCHIVE_DIR).join("old-feat");
    fs::create_dir_all(&archive_dir).unwrap();
    // Even v6 metadata in the archive directory is skipped by path.
    let meta = make_metadata_v6("old-feat", r#""track/old-feat""#, "");
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
    let meta = make_metadata_v6("corrupt-track", r#""track/corrupt-track""#, "");
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
    // are skipped. A branch-materialized v6 track that only has metadata
    // must therefore pass even without spec.md / spec.json / plan.md.
    // This implements "file existence = phase status" from
    // knowledge/conventions/workflow-ceremony-minimization.md and
    // supersedes the prior Planned-fallback-with-artifact-validation
    // behavior introduced by T025.
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().join(TRACK_ITEMS_DIR).join("phase0-track");
    fs::create_dir_all(&dir).unwrap();
    let meta = make_metadata_v6("phase0-track", r#""track/phase0-track""#, "");
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
    // v6 processing so that errors are surfaced (fail-closed).
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().join(TRACK_ITEMS_DIR).join("no-version-track");
    fs::create_dir_all(&dir).unwrap();
    // metadata.json without schema_version — must not be silently skipped.
    let meta = r#"{"id":"no-version-track","branch":"track/no-version-track","title":"No Version","created_at":"2026-01-01T00:00:00Z","updated_at":"2026-01-15T00:00:00Z"}"#;
    fs::write(dir.join("metadata.json"), meta).unwrap();
    // No impl-plan.json — if the track were processed as v6, status derives
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
    // < 6 and would cause the track to be silently skipped as legacy).
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
#[cfg(unix)]
fn test_symlinked_track_directory_fails_closed() {
    let tmp = TempDir::new().unwrap();
    let items_dir = tmp.path().join(TRACK_ITEMS_DIR);
    fs::create_dir_all(&items_dir).unwrap();
    let outside = TempDir::new().unwrap();
    fs::write(
        outside.path().join("metadata.json"),
        make_metadata_v6("escape-track", r#""track/escape-track""#, ""),
    )
    .unwrap();
    std::os::unix::fs::symlink(outside.path(), items_dir.join("escape-track")).unwrap();

    let outcome = verify(tmp.path());
    assert!(outcome.has_errors(), "symlinked track directory must fail closed");
    let msgs: Vec<_> = outcome.findings().iter().map(|f| f.message()).collect();
    assert!(
        msgs.iter().any(|msg| msg.contains("track directory is a symlink")),
        "error should mention symlinked track directory, got: {msgs:?}"
    );
}

#[test]
#[cfg(unix)]
fn test_symlinked_metadata_file_fails_closed() {
    let tmp = TempDir::new().unwrap();
    let track_dir = tmp.path().join(TRACK_ITEMS_DIR).join("metadata-link-track");
    fs::create_dir_all(&track_dir).unwrap();
    let outside = TempDir::new().unwrap();
    let target = outside.path().join("metadata.json");
    fs::write(
        &target,
        make_metadata_v6("metadata-link-track", r#""track/metadata-link-track""#, ""),
    )
    .unwrap();
    std::os::unix::fs::symlink(&target, track_dir.join("metadata.json")).unwrap();

    let outcome = verify(tmp.path());
    assert!(outcome.has_errors(), "symlinked metadata.json must fail closed");
    let msgs: Vec<_> = outcome.findings().iter().map(|f| f.message()).collect();
    assert!(
        msgs.iter().any(|msg| msg.contains("metadata.json") && msg.contains("symlink")),
        "error should mention symlinked metadata.json, got: {msgs:?}"
    );
}

#[test]
#[cfg(unix)]
fn test_symlinked_impl_plan_file_fails_closed() {
    let tmp = TempDir::new().unwrap();
    let track_dir = tmp.path().join(TRACK_ITEMS_DIR).join("impl-plan-link-track");
    fs::create_dir_all(&track_dir).unwrap();
    fs::write(
        track_dir.join("metadata.json"),
        make_metadata_v6("impl-plan-link-track", r#""track/impl-plan-link-track""#, ""),
    )
    .unwrap();
    let outside = TempDir::new().unwrap();
    let target = outside.path().join("impl-plan.json");
    fs::write(&target, IN_PROGRESS_IMPL_PLAN_JSON).unwrap();
    std::os::unix::fs::symlink(&target, track_dir.join("impl-plan.json")).unwrap();

    let outcome = verify(tmp.path());
    assert!(outcome.has_errors(), "symlinked impl-plan.json must fail closed");
    let msgs: Vec<_> = outcome.findings().iter().map(|f| f.message()).collect();
    assert!(
        msgs.iter().any(|msg| msg.contains("impl-plan.json") && msg.contains("symlink")),
        "error should mention symlinked impl-plan.json, got: {msgs:?}"
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
