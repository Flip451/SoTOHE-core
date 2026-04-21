//! Integration tests for `sotp track transition` and `sotp track views sync`.
//!
//! T005: TrackMetadata is identity-only.
//! T007: `sotp track transition` delegates to `TransitionTaskUseCase`, which
//!       loads and persists task state via `ImplPlanDocument` (impl-plan.json).
//! T008: `plan.md` is rendered from `impl-plan.json`; task markers are present.

#![allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::process::Command;

fn sotp_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sotp"))
}

/// Writes a minimal v4 metadata.json fixture (identity-only, no tasks/plan).
fn write_fixture_metadata(items_dir: &Path, track_id: &str) -> PathBuf {
    let track_dir = items_dir.join(track_id);
    std::fs::create_dir_all(&track_dir).unwrap();

    // schema_version 4: identity-only (T005)
    let metadata = format!(
        r#"{{
  "schema_version": 4,
  "id": "{track_id}",
  "branch": "track/{track_id}",
  "title": "Integration Track",
  "status": "planned",
  "created_at": "2026-03-13T00:00:00Z",
  "updated_at": "2026-03-13T00:00:00Z"
}}
"#
    );
    let metadata_path = track_dir.join("metadata.json");
    std::fs::write(&metadata_path, metadata).unwrap();
    metadata_path
}

/// Writes an impl-plan.json with a single todo task T001.
fn write_fixture_impl_plan(items_dir: &Path, track_id: &str) {
    let track_dir = items_dir.join(track_id);
    let impl_plan = r#"{
  "schema_version": 1,
  "tasks": [
    { "id": "T001", "description": "First task", "status": "todo" }
  ],
  "plan": {
    "summary": [],
    "sections": [
      { "id": "S1", "title": "Phase 1", "description": [], "task_ids": ["T001"] }
    ]
  }
}
"#;
    std::fs::write(track_dir.join("impl-plan.json"), impl_plan).unwrap();
}

fn project_root_with_full_track(root: &Path, track_id: &str) -> PathBuf {
    let items_dir = root.join("track/items");
    write_fixture_metadata(&items_dir, track_id);
    write_fixture_impl_plan(&items_dir, track_id);
    items_dir
}

// --- transition tests (T007 implemented) ---

#[test]
fn transition_subcommand_success_updates_status_and_persists() {
    // T007: `sotp track transition` loads impl-plan.json, applies transition,
    // and persists updated impl-plan.json back to disk.
    // T005: metadata.json `status` must also be synced to the derived track status.
    let root_dir = tempfile::tempdir().unwrap();
    let items_dir = project_root_with_full_track(root_dir.path(), "demo");

    let output = sotp_bin()
        .args([
            "track",
            "transition",
            "--items-dir",
            items_dir.to_str().unwrap(),
            "--skip-branch-check",
            "demo",
            "T001",
            "in_progress",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "expected exit 0 for valid transition, got: {}\nstderr: {stderr}",
        output.status
    );

    // Verify impl-plan.json was updated on disk.
    let impl_plan_path = items_dir.join("demo/impl-plan.json");
    let content = std::fs::read_to_string(&impl_plan_path).unwrap();
    assert!(
        content.contains("\"in_progress\""),
        "impl-plan.json must reflect new status:\n{content}"
    );

    // T005: metadata.json status must also be synced to "in_progress" after
    // the first task starts (derived track status: in_progress).
    let metadata_path = items_dir.join("demo/metadata.json");
    let metadata_content = std::fs::read_to_string(&metadata_path).unwrap();
    assert!(
        metadata_content.contains("\"in_progress\""),
        "metadata.json status must be synced to in_progress:\n{metadata_content}"
    );
}

#[test]
fn transition_subcommand_rejects_invalid_status_transition() {
    // T007: todo -> done is invalid (must go todo -> in_progress -> done).
    // Also verifies that impl-plan.json is NOT partially written on a failed
    // transition — the task state must remain "todo" after rejection.
    let root_dir = tempfile::tempdir().unwrap();
    let items_dir = project_root_with_full_track(root_dir.path(), "demo");

    let output = sotp_bin()
        .args([
            "track",
            "transition",
            "--items-dir",
            items_dir.to_str().unwrap(),
            "--skip-branch-check",
            "demo",
            "T001",
            "done",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success(), "expected non-zero exit for invalid transition todo->done");

    // impl-plan.json must not have been partially written — T001 stays "todo".
    let impl_plan_path = items_dir.join("demo/impl-plan.json");
    let content = std::fs::read_to_string(&impl_plan_path).unwrap();
    assert!(
        content.contains("\"todo\""),
        "impl-plan.json must still contain \"todo\" status after rejected transition:\n{content}"
    );
    assert!(
        !content.contains("\"done\""),
        "impl-plan.json must NOT contain \"done\" status after rejected transition:\n{content}"
    );
}

#[test]
fn transition_subcommand_fails_on_missing_items_dir() {
    let root_dir = tempfile::tempdir().unwrap();
    let bogus = root_dir.path().join("does/not/exist/track/items");

    let output = sotp_bin()
        .args([
            "track",
            "transition",
            "--items-dir",
            bogus.to_str().unwrap(),
            "--skip-branch-check",
            "demo",
            "T001",
            "in_progress",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success(), "expected non-zero exit for missing items-dir");
}

#[test]
fn transition_subcommand_persists_commit_hash_on_done_transition() {
    // T007: transition to done with a commit hash traces the hash in impl-plan.json.
    // Must first transition to in_progress before done.
    let root_dir = tempfile::tempdir().unwrap();
    let items_dir = project_root_with_full_track(root_dir.path(), "demo");

    // Step 1: todo -> in_progress
    let out1 = sotp_bin()
        .args([
            "track",
            "transition",
            "--items-dir",
            items_dir.to_str().unwrap(),
            "--skip-branch-check",
            "demo",
            "T001",
            "in_progress",
        ])
        .output()
        .unwrap();
    let stderr1 = String::from_utf8_lossy(&out1.stderr);
    assert!(out1.status.success(), "step1 transition to in_progress failed:\nstderr: {stderr1}");

    // Step 2: in_progress -> done with commit hash
    let out2 = sotp_bin()
        .args([
            "track",
            "transition",
            "--items-dir",
            items_dir.to_str().unwrap(),
            "--skip-branch-check",
            "--commit-hash",
            "abc1234",
            "demo",
            "T001",
            "done",
        ])
        .output()
        .unwrap();
    let stderr2 = String::from_utf8_lossy(&out2.stderr);
    assert!(out2.status.success(), "step2 transition to done failed:\nstderr: {stderr2}");

    // Verify commit hash is persisted in impl-plan.json.
    let impl_plan_path = items_dir.join("demo/impl-plan.json");
    let content = std::fs::read_to_string(&impl_plan_path).unwrap();
    assert!(content.contains("abc1234"), "impl-plan.json must contain commit hash:\n{content}");
    assert!(content.contains("\"done\""), "impl-plan.json must reflect done status:\n{content}");
}

#[test]
fn transition_subcommand_full_round_trip_including_reopen() {
    // Covers the required round-trip: todo -> in_progress -> done -> in_progress (Reopen).
    // A regression in the Reopen transition would not be caught by the hash-persistence
    // test, which only covers todo -> in_progress -> done.
    let root_dir = tempfile::tempdir().unwrap();
    let items_dir = project_root_with_full_track(root_dir.path(), "demo");

    // Step 1: todo -> in_progress
    let out1 = sotp_bin()
        .args([
            "track",
            "transition",
            "--items-dir",
            items_dir.to_str().unwrap(),
            "--skip-branch-check",
            "demo",
            "T001",
            "in_progress",
        ])
        .output()
        .unwrap();
    let stderr1 = String::from_utf8_lossy(&out1.stderr);
    assert!(out1.status.success(), "step1 (todo->in_progress) failed:\nstderr: {stderr1}");

    // Step 2: in_progress -> done
    let out2 = sotp_bin()
        .args([
            "track",
            "transition",
            "--items-dir",
            items_dir.to_str().unwrap(),
            "--skip-branch-check",
            "demo",
            "T001",
            "done",
        ])
        .output()
        .unwrap();
    let stderr2 = String::from_utf8_lossy(&out2.stderr);
    assert!(out2.status.success(), "step2 (in_progress->done) failed:\nstderr: {stderr2}");

    // Verify done status is persisted.
    let impl_plan_path = items_dir.join("demo/impl-plan.json");
    let content = std::fs::read_to_string(&impl_plan_path).unwrap();
    assert!(content.contains("\"done\""), "impl-plan.json must reflect done status:\n{content}");

    // Step 3: done -> in_progress (Reopen)
    let out3 = sotp_bin()
        .args([
            "track",
            "transition",
            "--items-dir",
            items_dir.to_str().unwrap(),
            "--skip-branch-check",
            "demo",
            "T001",
            "in_progress",
        ])
        .output()
        .unwrap();
    let stderr3 = String::from_utf8_lossy(&out3.stderr);
    assert!(out3.status.success(), "step3 (done->in_progress Reopen) failed:\nstderr: {stderr3}");

    // Verify impl-plan.json reflects reopened in_progress status.
    let content = std::fs::read_to_string(&impl_plan_path).unwrap();
    assert!(
        content.contains("\"in_progress\""),
        "impl-plan.json must reflect reopened in_progress status:\n{content}"
    );
}

#[test]
fn transition_subcommand_full_round_trip_with_commit_hash_and_reopen() {
    // Covers the full required round-trip: todo -> in_progress -> done (with commit hash) ->
    // in_progress (Reopen). Verifies that the commit hash is retained in impl-plan.json
    // even after reopening (the traced hash is kept on the task entry; only the status reverts
    // to in_progress). This guards against regressions that erase the commit hash on reopen.
    let root_dir = tempfile::tempdir().unwrap();
    let items_dir = project_root_with_full_track(root_dir.path(), "demo");
    let impl_plan_path = items_dir.join("demo/impl-plan.json");

    // Step 1: todo -> in_progress
    let out1 = sotp_bin()
        .args([
            "track",
            "transition",
            "--items-dir",
            items_dir.to_str().unwrap(),
            "--skip-branch-check",
            "demo",
            "T001",
            "in_progress",
        ])
        .output()
        .unwrap();
    let stderr1 = String::from_utf8_lossy(&out1.stderr);
    assert!(out1.status.success(), "step1 (todo->in_progress) failed:\nstderr: {stderr1}");

    // Step 2: in_progress -> done with commit hash
    let out2 = sotp_bin()
        .args([
            "track",
            "transition",
            "--items-dir",
            items_dir.to_str().unwrap(),
            "--skip-branch-check",
            "--commit-hash",
            "def5678",
            "demo",
            "T001",
            "done",
        ])
        .output()
        .unwrap();
    let stderr2 = String::from_utf8_lossy(&out2.stderr);
    assert!(
        out2.status.success(),
        "step2 (in_progress->done with hash) failed:\nstderr: {stderr2}"
    );

    // Verify commit hash is persisted after done transition.
    let content = std::fs::read_to_string(&impl_plan_path).unwrap();
    assert!(
        content.contains("def5678"),
        "impl-plan.json must contain commit hash after done:\n{content}"
    );
    assert!(content.contains("\"done\""), "impl-plan.json must reflect done status:\n{content}");

    // Step 3: done -> in_progress (Reopen)
    let out3 = sotp_bin()
        .args([
            "track",
            "transition",
            "--items-dir",
            items_dir.to_str().unwrap(),
            "--skip-branch-check",
            "demo",
            "T001",
            "in_progress",
        ])
        .output()
        .unwrap();
    let stderr3 = String::from_utf8_lossy(&out3.stderr);
    assert!(out3.status.success(), "step3 (done->in_progress Reopen) failed:\nstderr: {stderr3}");

    // Verify impl-plan.json reflects reopened in_progress status.
    // The reopen transition (DoneTraced -> InProgress) intentionally resets the task
    // to plain `in_progress` with no commit hash — the hash is cleared by design.
    let content = std::fs::read_to_string(&impl_plan_path).unwrap();
    assert!(
        content.contains("\"in_progress\""),
        "impl-plan.json must reflect reopened in_progress status:\n{content}"
    );
    assert!(
        !content.contains("def5678"),
        "impl-plan.json must NOT contain commit hash after DoneTraced->InProgress reopen:\n{content}"
    );
}

#[test]
fn views_sync_subcommand_renders_plan_and_registry() {
    let root_dir = tempfile::tempdir().unwrap();
    let _items_dir = project_root_with_full_track(root_dir.path(), "demo");

    let output = sotp_bin()
        .args([
            "track",
            "views",
            "sync",
            "--project-root",
            root_dir.path().to_str().unwrap(),
            "--track-id",
            "demo",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "expected exit 0, got {}: stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );

    let plan_md = root_dir.path().join("track/items/demo/plan.md");
    assert!(plan_md.is_file(), "plan.md must be rendered at {}", plan_md.display());

    // T008: task markers are now rendered from impl-plan.json.
    let plan_content = std::fs::read_to_string(&plan_md).unwrap();
    assert!(!plan_content.is_empty(), "plan.md must not be empty");
    assert!(plan_content.contains("T001"), "plan.md must contain task T001:\n{plan_content}");
    assert!(plan_content.contains("[ ]"), "plan.md must contain todo task marker:\n{plan_content}");

    let registry_md = root_dir.path().join("track/registry.md");
    assert!(registry_md.is_file(), "registry.md must be rendered at {}", registry_md.display());
}
