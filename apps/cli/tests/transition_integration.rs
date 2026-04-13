//! Integration tests for `sotp track transition` and `sotp track views sync`.
//!
//! These cover the CLI-level regressions previously owned by the deleted Python
//! `scripts/test_track_state_machine.py::TestCLI` module:
//! transition success / invalid transition / missing directory / `--commit-hash`
//! persistence / `views sync` rendering.

#![allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::process::Command;

fn sotp_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sotp"))
}

/// Writes a minimal v3 metadata.json fixture.
///
/// `task_status` and optional `commit_hash` control the initial task state so
/// the caller can exercise individual transitions without chaining commands.
fn write_fixture_metadata(
    items_dir: &Path,
    track_id: &str,
    task_status: &str,
    commit_hash: Option<&str>,
) -> PathBuf {
    let track_dir = items_dir.join(track_id);
    std::fs::create_dir_all(&track_dir).unwrap();

    let commit_hash_field = match commit_hash {
        Some(hash) => format!(r#", "commit_hash": "{hash}""#),
        None => String::new(),
    };

    let metadata = format!(
        r#"{{
  "schema_version": 3,
  "id": "{track_id}",
  "branch": "track/{track_id}",
  "title": "Integration Track",
  "status": "planned",
  "created_at": "2026-03-13T00:00:00Z",
  "updated_at": "2026-03-13T00:00:00Z",
  "tasks": [
    {{ "id": "T001", "description": "First task", "status": "{task_status}"{commit_hash_field} }}
  ],
  "plan": {{
    "summary": [],
    "sections": [
      {{ "id": "S1", "title": "Build", "description": [], "task_ids": ["T001"] }}
    ]
  }}
}}
"#
    );
    let metadata_path = track_dir.join("metadata.json");
    std::fs::write(&metadata_path, metadata).unwrap();
    metadata_path
}

fn project_root_with_track(
    root: &Path,
    track_id: &str,
    task_status: &str,
    commit_hash: Option<&str>,
) -> PathBuf {
    let items_dir = root.join("track/items");
    write_fixture_metadata(&items_dir, track_id, task_status, commit_hash);
    items_dir
}

fn read_metadata_json(items_dir: &Path, track_id: &str) -> serde_json::Value {
    let path = items_dir.join(track_id).join("metadata.json");
    let content = std::fs::read_to_string(&path).unwrap();
    serde_json::from_str(&content).unwrap()
}

#[test]
fn transition_subcommand_success_updates_status_and_persists() {
    let root_dir = tempfile::tempdir().unwrap();
    let items_dir = project_root_with_track(root_dir.path(), "demo", "todo", None);

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

    assert!(
        output.status.success(),
        "expected exit 0, got {}: stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );

    let data = read_metadata_json(&items_dir, "demo");
    assert_eq!(data["tasks"][0]["status"], "in_progress");
}

#[test]
fn transition_subcommand_rejects_invalid_status_transition() {
    let root_dir = tempfile::tempdir().unwrap();
    let items_dir = project_root_with_track(root_dir.path(), "demo", "todo", None);

    // Going directly from `todo` to `done` is not a valid transition (must first
    // enter `in_progress`).
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

    assert!(!output.status.success(), "expected non-zero exit for invalid transition, got success");

    let data = read_metadata_json(&items_dir, "demo");
    assert_eq!(
        data["tasks"][0]["status"], "todo",
        "task status must be unchanged after rejected transition"
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
    let root_dir = tempfile::tempdir().unwrap();
    let items_dir = project_root_with_track(root_dir.path(), "demo", "in_progress", None);

    let output = sotp_bin()
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

    assert!(
        output.status.success(),
        "expected exit 0, got {}: stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );

    let data = read_metadata_json(&items_dir, "demo");
    assert_eq!(data["tasks"][0]["status"], "done");
    assert_eq!(data["tasks"][0]["commit_hash"], "abc1234");
}

#[test]
fn views_sync_subcommand_renders_plan_and_registry() {
    let root_dir = tempfile::tempdir().unwrap();
    let _items_dir = project_root_with_track(root_dir.path(), "demo", "todo", None);

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
    let plan_content = std::fs::read_to_string(&plan_md).unwrap();
    assert!(plan_content.contains("- [ ] First task"), "plan.md missing task marker");

    let registry_md = root_dir.path().join("track/registry.md");
    assert!(registry_md.is_file(), "registry.md must be rendered at {}", registry_md.display());
}
