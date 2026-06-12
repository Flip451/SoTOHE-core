//! Integration tests for `sotp track transition` and `sotp track views sync`.
//!
//! TrackMetadata is identity-only. `sotp track transition` delegates to
//! `TransitionTaskUseCase`, which loads and persists task state via
//! `ImplPlanDocument` (impl-plan.json). `plan.md` is rendered from
//! `impl-plan.json` with task markers.

#![allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::process::Command;

fn sotp_bin() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_sotp"));
    // Disable telemetry in spawned binary so integration tests never write to
    // the real track/items/ tree (CN-06 / AC-07).  The #[cfg(test)] guard only
    // applies to in-process code; spawned binaries are full production processes.
    cmd.env("SOTP_TELEMETRY", "0");
    cmd
}

/// Creates a minimal git repository rooted at `root` with a track branch
/// `track/<track_id>` checked out. This lets integration tests control the
/// git branch that `SystemGitRepo::discover()` reads inside the sotp binary,
/// independent of the CI checkout state.
///
/// A commit is required so that `git rev-parse --abbrev-ref HEAD` resolves
/// to the branch name rather than "HEAD" (orphan branches without commits
/// show "HEAD" instead of the branch name).
fn init_git_repo_on_track_branch(root: &Path, track_id: &str) {
    let branch_name = format!("track/{track_id}");

    // git init
    let status = Command::new("git")
        .args(["init", "-q"])
        .current_dir(root)
        .status()
        .expect("git init failed");
    assert!(status.success(), "git init must succeed");

    // Configure minimal identity so git commit works.
    for (key, value) in
        [("user.email", "test@example.com"), ("user.name", "Test"), ("commit.gpgsign", "false")]
    {
        let status = Command::new("git")
            .args(["config", key, value])
            .current_dir(root)
            .status()
            .expect("git config failed");
        assert!(status.success(), "git config {key} must succeed");
    }

    // Create an empty initial commit on the target track branch.
    // Using --allow-empty so no file needs to be staged.
    let status = Command::new("git")
        .args(["commit", "--allow-empty", "-q", "-m", "init", "--no-gpg-sign"])
        .current_dir(root)
        .status()
        .expect("git commit failed");
    assert!(status.success(), "initial git commit must succeed");

    // Rename the default branch (usually main/master) to the track branch.
    let status = Command::new("git")
        .args(["branch", "-m", &branch_name])
        .current_dir(root)
        .status()
        .expect("git branch -m failed");
    assert!(status.success(), "git branch -m must succeed");
}

/// Writes a minimal v5 metadata.json fixture (identity-only, branchless).
///
/// Uses `"branch": null` so the in-usecase branch guard is a no-op: the guard
/// only fires for tracks that carry an expected branch name.  This lets the
/// integration tests exercise the full transition / domain logic without
/// requiring a real git repository in the temp directory.
///
/// Note: `--track-id` must NOT be passed to WRITE commands when running these
/// integration tests, because `resolve_track_id_for_write` would compare the
/// supplied id against the real git branch of the CI/dev environment and fail.
/// Instead the tests rely on `--track-id` being omitted, which delegates to the
/// branchless path inside the usecase layer via `None` branch_reader skip.
/// To supply the track id to the CLI without triggering the WRITE guard, the
/// test fixture uses `"branch": null` and passes `--track-id` on commands that
/// only need it for filesystem routing (the guard still runs but passes because
/// branchless tracks skip the branch comparison inside enforce_branch_guard).
fn write_fixture_metadata(items_dir: &Path, track_id: &str) -> PathBuf {
    let track_dir = items_dir.join(track_id);
    std::fs::create_dir_all(&track_dir).unwrap();

    // schema_version 5: branchless (branch: null) so the branch guard is no-op.
    let metadata = format!(
        r#"{{
  "schema_version": 5,
  "id": "{track_id}",
  "branch": null,
  "title": "Integration Track",
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

/// Writes a minimal architecture-rules.json fixture so render.rs can iterate
/// TDDD layers (fail-closed after T045 — synthetic fallback removed).
fn write_fixture_arch_rules(root: &Path) {
    let arch_rules = r#"{"layers":[{"crate":"domain","tddd":{"enabled":true,"catalogue_file":"domain-types.json"}}]}"#;
    std::fs::write(root.join("architecture-rules.json"), arch_rules).unwrap();
}

fn project_root_with_full_track(root: &Path, track_id: &str) -> PathBuf {
    let items_dir = root.join("track/items");
    write_fixture_metadata(&items_dir, track_id);
    write_fixture_impl_plan(&items_dir, track_id);
    write_fixture_arch_rules(root);
    // Bootstrap a git repo on the track branch so `resolve_track_id_for_write`
    // can discover it from the items_dir project root.
    init_git_repo_on_track_branch(root, track_id);
    items_dir
}

// --- transition tests ---

#[test]
fn transition_subcommand_success_updates_status_and_persists() {
    // `sotp track transition` loads impl-plan.json, applies transition, and
    // persists updated impl-plan.json back to disk. metadata.json is not
    // written; status is derived on demand from impl-plan.json.
    //
    // Uses a fixed synthetic track id and creates an isolated git repo on the
    // corresponding track branch via `project_root_with_full_track()`.
    // `resolve_track_id_for_write` discovers this isolated repo (anchored to the
    // items_dir project root), so the test runs unconditionally on any CI/dev
    // checkout branch (D7 / AC-18).
    let track_id = "synthetic-2026";

    let root_dir = tempfile::tempdir().unwrap();
    let items_dir = project_root_with_full_track(root_dir.path(), track_id);

    let output = sotp_bin()
        .args([
            "track",
            "transition",
            "--items-dir",
            items_dir.to_str().unwrap(),
            "--track-id",
            track_id,
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
    let impl_plan_path = items_dir.join(format!("{track_id}/impl-plan.json"));
    let content = std::fs::read_to_string(&impl_plan_path).unwrap();
    assert!(
        content.contains("\"in_progress\""),
        "impl-plan.json must reflect new status:\n{content}"
    );

    // metadata.json does NOT contain a status field — status is derived on
    // demand from impl-plan.json. Verify metadata.json has no "status" key.
    let metadata_path = items_dir.join(format!("{track_id}/metadata.json"));
    let metadata_content = std::fs::read_to_string(&metadata_path).unwrap();
    assert!(
        !metadata_content.contains("\"status\""),
        "metadata.json must NOT contain a status field (derived-status):\n{metadata_content}"
    );
}

#[test]
fn transition_subcommand_rejects_invalid_status_transition() {
    // todo -> done is invalid (must go todo -> in_progress -> done).
    // Also verifies that impl-plan.json is NOT partially written on a failed
    // transition — the task state must remain "todo" after rejection.
    //
    // Uses a fixed synthetic track id and an isolated git repo so the test runs
    // unconditionally on any CI/dev checkout branch (D7 / AC-18).
    let track_id = "synthetic-2026";

    let root_dir = tempfile::tempdir().unwrap();
    let items_dir = project_root_with_full_track(root_dir.path(), track_id);

    let output = sotp_bin()
        .args([
            "track",
            "transition",
            "--items-dir",
            items_dir.to_str().unwrap(),
            "--track-id",
            track_id,
            "T001",
            "done",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success(), "expected non-zero exit for invalid transition todo->done");

    // impl-plan.json must not have been partially written — T001 stays "todo".
    let impl_plan_path = items_dir.join(format!("{track_id}/impl-plan.json"));
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
    // An items-dir path that does not end in `track/items` is rejected by
    // `resolve_project_root` inside `resolve_track_id_for_write` before any git
    // discovery or branch guard fires. This gives a deterministic failure
    // regardless of which branch CI is checked out on.
    let root_dir = tempfile::tempdir().unwrap();
    // Path does NOT end in `track/items` → resolve_project_root returns Err.
    let bogus = root_dir.path().join("does/not/exist/wrong/path");

    let output = sotp_bin()
        .args([
            "track",
            "transition",
            "--items-dir",
            bogus.to_str().unwrap(),
            "--track-id",
            "synthetic-2026",
            "T001",
            "in_progress",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success(), "expected non-zero exit for malformed items-dir path");
}

#[test]
fn transition_subcommand_persists_commit_hash_on_done_transition() {
    // Transition to done with a commit hash traces the hash in impl-plan.json.
    // Must first transition to in_progress before done.
    //
    // Uses a fixed synthetic track id and an isolated git repo so the test runs
    // unconditionally on any CI/dev checkout branch (D7 / AC-18).
    let track_id = "synthetic-2026";

    let root_dir = tempfile::tempdir().unwrap();
    let items_dir = project_root_with_full_track(root_dir.path(), track_id);

    // Step 1: todo -> in_progress
    let out1 = sotp_bin()
        .args([
            "track",
            "transition",
            "--items-dir",
            items_dir.to_str().unwrap(),
            "--track-id",
            track_id,
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
            "--commit-hash",
            "abc1234",
            "--track-id",
            track_id,
            "T001",
            "done",
        ])
        .output()
        .unwrap();
    let stderr2 = String::from_utf8_lossy(&out2.stderr);
    assert!(out2.status.success(), "step2 transition to done failed:\nstderr: {stderr2}");

    // Verify commit hash is persisted in impl-plan.json.
    let impl_plan_path = items_dir.join(format!("{track_id}/impl-plan.json"));
    let content = std::fs::read_to_string(&impl_plan_path).unwrap();
    assert!(content.contains("abc1234"), "impl-plan.json must contain commit hash:\n{content}");
    assert!(content.contains("\"done\""), "impl-plan.json must reflect done status:\n{content}");
}

#[test]
fn transition_subcommand_full_round_trip_including_reopen() {
    // Covers the required round-trip: todo -> in_progress -> done -> in_progress (Reopen).
    // A regression in the Reopen transition would not be caught by the hash-persistence
    // test, which only covers todo -> in_progress -> done.
    //
    // Uses a fixed synthetic track id and an isolated git repo so the test runs
    // unconditionally on any CI/dev checkout branch (D7 / AC-18).
    let track_id = "synthetic-2026";

    let root_dir = tempfile::tempdir().unwrap();
    let items_dir = project_root_with_full_track(root_dir.path(), track_id);

    // Step 1: todo -> in_progress
    let out1 = sotp_bin()
        .args([
            "track",
            "transition",
            "--items-dir",
            items_dir.to_str().unwrap(),
            "--track-id",
            track_id,
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
            "--track-id",
            track_id,
            "T001",
            "done",
        ])
        .output()
        .unwrap();
    let stderr2 = String::from_utf8_lossy(&out2.stderr);
    assert!(out2.status.success(), "step2 (in_progress->done) failed:\nstderr: {stderr2}");

    // Verify done status is persisted.
    let impl_plan_path = items_dir.join(format!("{track_id}/impl-plan.json"));
    let content = std::fs::read_to_string(&impl_plan_path).unwrap();
    assert!(content.contains("\"done\""), "impl-plan.json must reflect done status:\n{content}");

    // Step 3: done -> in_progress (Reopen)
    let out3 = sotp_bin()
        .args([
            "track",
            "transition",
            "--items-dir",
            items_dir.to_str().unwrap(),
            "--track-id",
            track_id,
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
    //
    // Uses a fixed synthetic track id and an isolated git repo so the test runs
    // unconditionally on any CI/dev checkout branch (D7 / AC-18).
    let track_id = "synthetic-2026";

    let root_dir = tempfile::tempdir().unwrap();
    let items_dir = project_root_with_full_track(root_dir.path(), track_id);
    let impl_plan_path = items_dir.join(format!("{track_id}/impl-plan.json"));

    // Step 1: todo -> in_progress
    let out1 = sotp_bin()
        .args([
            "track",
            "transition",
            "--items-dir",
            items_dir.to_str().unwrap(),
            "--track-id",
            track_id,
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
            "--commit-hash",
            "def5678",
            "--track-id",
            track_id,
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
            "--track-id",
            track_id,
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

    // Task markers are rendered from impl-plan.json.
    let plan_content = std::fs::read_to_string(&plan_md).unwrap();
    assert!(!plan_content.is_empty(), "plan.md must not be empty");
    assert!(plan_content.contains("T001"), "plan.md must contain task T001:\n{plan_content}");
    assert!(plan_content.contains("[ ]"), "plan.md must contain todo task marker:\n{plan_content}");

    let registry_md = root_dir.path().join("track/registry.md");
    assert!(registry_md.is_file(), "registry.md must be rendered at {}", registry_md.display());
}

#[test]
fn transition_subcommand_write_guard_success_with_synthetic_track_branch() {
    // End-to-end test for the T015 WRITE guard SUCCESS path that runs
    // unconditionally on any branch (including detached HEAD and CI
    // default-branch checkouts).
    //
    // Creates an isolated git repository in a temp directory with branch
    // `track/<id>`. Since `resolve_track_id_for_write` now anchors git
    // discovery to the repository that owns `--items-dir` (derived via
    // `resolve_project_root`), the synthetic repo at `root_dir` is discovered
    // independently of the CI/dev checkout branch.
    //
    // The explicit `--track-id` matches the branch, so
    // `resolve_track_id_for_write(Some(id), items_dir)` returns `Ok(id)` and
    // the transition proceeds to the domain layer.
    let root_dir = tempfile::tempdir().unwrap();
    let track_id = "synthetic-2026";

    // Bootstrap a git repo at root_dir on the target track branch.
    // resolve_project_root(items_dir) == root_dir → git discovers this repo.
    init_git_repo_on_track_branch(root_dir.path(), track_id);

    // Create the track fixture inside the same root.
    let items_dir = root_dir.path().join("track/items");
    write_fixture_metadata(&items_dir, track_id);
    write_fixture_impl_plan(&items_dir, track_id);
    write_fixture_arch_rules(root_dir.path());

    let output = sotp_bin()
        .args([
            "track",
            "transition",
            "--items-dir",
            items_dir.to_str().unwrap(),
            "--track-id",
            track_id,
            "T001",
            "in_progress",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "expected exit 0: WRITE guard must accept matching --track-id on branch track/{track_id}\n\
         stderr: {stderr}"
    );

    // Verify impl-plan.json was mutated.
    let impl_plan_path = items_dir.join(format!("{track_id}/impl-plan.json"));
    let content = std::fs::read_to_string(&impl_plan_path).unwrap();
    assert!(
        content.contains("\"in_progress\""),
        "impl-plan.json must reflect in_progress after transition:\n{content}"
    );
}

#[test]
fn transition_subcommand_write_guard_rejects_mismatched_track_id() {
    // Verifies that the T015 WRITE guard (resolve_track_id_for_write) rejects a
    // `--track-id` value that does not match the git branch in the repository
    // that owns `--items-dir`. Runs unconditionally on all branches.
    //
    // Creates a synthetic git repo on branch `track/actual-branch-2026`, then
    // passes `--track-id mismatch-sentinel-999`. The guard compares the
    // branch-derived id (`actual-branch-2026`) against the explicit id
    // (`mismatch-sentinel-999`) and must reject with a non-zero exit.
    let root_dir = tempfile::tempdir().unwrap();
    let actual_branch = "actual-branch-2026";
    let sentinel_track = "mismatch-sentinel-999";

    // Bootstrap a git repo on a DIFFERENT track branch than the sentinel.
    // Note: do NOT call project_root_with_full_track here — that would init the
    // git repo on track/sentinel_track instead of track/actual_branch.
    init_git_repo_on_track_branch(root_dir.path(), actual_branch);

    // Create the track fixture for the sentinel track id (metadata + impl-plan).
    let items_dir = root_dir.path().join("track/items");
    write_fixture_metadata(&items_dir, sentinel_track);
    write_fixture_impl_plan(&items_dir, sentinel_track);
    write_fixture_arch_rules(root_dir.path());

    let output = sotp_bin()
        .args([
            "track",
            "transition",
            "--items-dir",
            items_dir.to_str().unwrap(),
            "--track-id",
            sentinel_track,
            "T001",
            "in_progress",
        ])
        .output()
        .unwrap();

    // The WRITE guard must reject because `actual-branch-2026` ≠ `mismatch-sentinel-999`.
    assert!(
        !output.status.success(),
        "expected non-zero exit: WRITE guard must reject mismatched --track-id, got success\n\
         stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("WRITE operation rejected") || stderr.contains("does not match"),
        "stderr must mention WRITE guard rejection:\n{stderr}"
    );
}
