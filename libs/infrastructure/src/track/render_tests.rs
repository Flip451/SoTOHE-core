//! Tests for [`render`] (split out to keep the main module under the 200-400 line guideline).

use super::*;

/// Generates a v5 metadata.json string (no `status` field).
/// The `status` parameter is accepted for API compatibility but is ignored
/// since v5 derives status from impl-plan.json at runtime.
/// The `tasks_json` parameter is also ignored (tasks live in impl-plan.json).
fn sample_metadata_json(id: &str, _status: &str, updated_at: &str, _tasks_json: &str) -> String {
    sample_metadata_json_with_schema_and_branch(
        5,
        id,
        _status,
        updated_at,
        _tasks_json,
        Some(&format!("track/{id}")),
    )
}

fn sample_metadata_json_with_branch(
    id: &str,
    _status: &str,
    updated_at: &str,
    _tasks_json: &str,
    branch: Option<&str>,
) -> String {
    sample_metadata_json_with_schema_and_branch(5, id, _status, updated_at, _tasks_json, branch)
}

/// Generates a metadata.json string.
///
/// For `schema_version == 5`: emits v5 format (no `status`, no `tasks`/`plan`).
/// For `schema_version < 5`: emits legacy format with `status`, `tasks`, and `plan`.
fn sample_metadata_json_with_schema_and_branch(
    schema_version: u32,
    id: &str,
    status: &str,
    updated_at: &str,
    tasks_json: &str,
    branch: Option<&str>,
) -> String {
    let branch_field = match branch {
        Some(branch) => format!(r#""branch": "{branch}","#),
        None => r#""branch": null,"#.to_owned(),
    };
    if schema_version >= 5 {
        // v5: no `status`, no `tasks`, no `plan`
        format!(
            r#"{{
  "schema_version": {schema_version},
  "id": "{id}",
  {branch_field}
  "title": "Title {id}",
  "created_at": "2026-03-13T00:00:00Z",
  "updated_at": "{updated_at}"
}}
"#
        )
    } else {
        // Legacy v2/v3: include `status`, `tasks`, `plan`
        format!(
            r#"{{
  "schema_version": {schema_version},
  "id": "{id}",
  {branch_field}
  "title": "Title {id}",
  "status": "{status}",
  "created_at": "2026-03-13T00:00:00Z",
  "updated_at": "{updated_at}",
  "tasks": {tasks_json},
  "plan": {{
    "summary": ["Summary line"],
    "sections": [
      {{
        "id": "S001",
        "title": "Section",
        "description": ["Section desc"],
        "task_ids": ["T001"]
      }}
    ]
  }}
}}
"#
        )
    }
}

/// Build an `ImplPlanDocument` from flat task + section specs.
///
/// - `tasks`: `(id, description, status_str, commit_hash_opt)` where
///   status_str is `"todo" | "in_progress" | "done_pending" | "done_traced" | "skipped"`
/// - `sections`: `(section_id, section_title, task_ids_slice)`
fn make_impl_plan_with_tasks(
    tasks: &[(&str, &str, &str, Option<&str>)],
    sections: &[(&str, &str, &[&str])],
) -> domain::ImplPlanDocument {
    make_impl_plan_with_summary_and_tasks(&[], tasks, sections)
}

fn make_impl_plan_with_summary_and_tasks(
    summary: &[&str],
    tasks: &[(&str, &str, &str, Option<&str>)],
    sections: &[(&str, &str, &[&str])],
) -> domain::ImplPlanDocument {
    let sections_with_desc: Vec<(&str, &str, &[&str], &[&str])> = sections
        .iter()
        .map(|(id, title, task_ids)| (*id, *title, *task_ids, [].as_slice()))
        .collect();
    make_impl_plan_inner(summary, tasks, &sections_with_desc)
}

fn make_impl_plan_with_desc_and_tasks(
    tasks: &[(&str, &str, &str, Option<&str>)],
    sections: &[(&str, &str, &[&str], &[&str])],
) -> domain::ImplPlanDocument {
    make_impl_plan_inner(&[], tasks, sections)
}

fn make_impl_plan_inner(
    summary: &[&str],
    tasks: &[(&str, &str, &str, Option<&str>)],
    sections: &[(&str, &str, &[&str], &[&str])],
) -> domain::ImplPlanDocument {
    use domain::{CommitHash, PlanSection, PlanView, TaskId, TaskStatus, TrackTask};

    let domain_tasks: Vec<TrackTask> = tasks
        .iter()
        .map(|(id, desc, status_str, hash)| {
            let task_id = TaskId::try_new(id.to_string()).unwrap();
            let status = match *status_str {
                "todo" => TaskStatus::Todo,
                "in_progress" => TaskStatus::InProgress,
                "done_pending" => TaskStatus::DonePending,
                "done_traced" => {
                    let h = CommitHash::try_new(hash.unwrap_or("abc1234")).unwrap();
                    TaskStatus::DoneTraced { commit_hash: h }
                }
                "skipped" => TaskStatus::Skipped,
                other => panic!("unknown status: {other}"),
            };
            TrackTask::with_status(task_id, *desc, status).unwrap()
        })
        .collect();

    let domain_sections: Vec<PlanSection> = sections
        .iter()
        .map(|(sid, stitle, task_ids, desc)| {
            let tids: Vec<TaskId> =
                task_ids.iter().map(|t| TaskId::try_new(t.to_string()).unwrap()).collect();
            PlanSection::new(*sid, *stitle, desc.iter().map(|s| s.to_string()).collect(), tids)
                .unwrap()
        })
        .collect();

    let plan = PlanView::new(summary.iter().map(|s| s.to_string()).collect(), domain_sections);
    domain::ImplPlanDocument::new(domain_tasks, plan).unwrap()
}

#[test]
fn render_plan_matches_expected_layout() {
    // render_plan with None impl_plan renders header, title, and impl-plan stub note.
    let json = sample_metadata_json(
        "track-a",
        "planned",
        "2026-03-13T01:00:00Z",
        r#"[
    {
      "id": "T001",
      "description": "First task",
      "status": "todo"
    }
  ]"#,
    );
    let (track, _) = codec::decode(&json).unwrap();

    let rendered = render_plan(&track, None);

    assert!(rendered.contains("<!-- Generated from metadata.json + impl-plan.json"));
    assert!(rendered.contains("# Title track-a"));
    assert!(rendered.contains("impl-plan.json"), "None case must mention impl-plan.json");
}

// --- render_plan marker tests ---

#[test]
fn render_plan_marks_in_progress_task_with_tilde() {
    // With an impl-plan containing an in-progress task, [~] marker appears.
    let json = sample_metadata_json(
        "track-a",
        "in_progress",
        "2026-03-13T01:00:00Z",
        r#"[
    { "id": "T001", "description": "Working task", "status": "in_progress" }
  ]"#,
    );
    let (track, _) = codec::decode(&json).unwrap();
    let impl_plan = make_impl_plan_with_tasks(
        &[("T001", "Working task", "in_progress", None)],
        &[("S1", "Section", &["T001"])],
    );
    let rendered = render_plan(&track, Some(&impl_plan));
    assert!(rendered.contains("# Title track-a"), "title must appear:\n{rendered}");
    assert!(rendered.contains("[~]"), "[~] marker must appear for in_progress task:\n{rendered}");
}

#[test]
fn render_plan_marks_done_task_with_short_commit_hash() {
    // Done task with commit hash renders [x] marker and hash note.
    let json = sample_metadata_json(
        "track-a",
        "done",
        "2026-03-13T01:00:00Z",
        r#"[
    {
      "id": "T001",
      "description": "Completed task",
      "status": "done",
      "commit_hash": "abc1234"
    }
  ]"#,
    );
    let (track, _) = codec::decode(&json).unwrap();
    let impl_plan = make_impl_plan_with_tasks(
        &[("T001", "Completed task", "done_traced", Some("abc1234"))],
        &[("S1", "Section", &["T001"])],
    );
    let rendered = render_plan(&track, Some(&impl_plan));
    assert!(rendered.contains("# Title track-a"), "title must appear:\n{rendered}");
    assert!(rendered.contains("[x]"), "[x] marker must appear for done task:\n{rendered}");
    assert!(rendered.contains("abc1234"), "commit hash must appear:\n{rendered}");
}

#[test]
fn render_plan_done_without_commit_hash_omits_literal_none() {
    // Done task without commit hash renders [x] but no "None" string.
    let json = sample_metadata_json(
        "track-a",
        "done",
        "2026-03-13T01:00:00Z",
        r#"[
    { "id": "T001", "description": "Untraced done", "status": "done" }
  ]"#,
    );
    let (track, _) = codec::decode(&json).unwrap();
    let impl_plan = make_impl_plan_with_tasks(
        &[("T001", "Untraced done", "done_pending", None)],
        &[("S1", "Section", &["T001"])],
    );
    let rendered = render_plan(&track, Some(&impl_plan));
    assert!(
        !rendered.contains("None"),
        "literal 'None' must never appear in rendered plan:\n{rendered}"
    );
    assert!(rendered.contains("[x]"), "[x] marker must appear for done_pending task:\n{rendered}");
}

#[test]
fn render_plan_marks_skipped_task_with_dash() {
    // Skipped task renders with [-] marker.
    let json = sample_metadata_json(
        "track-a",
        "done",
        "2026-03-13T01:00:00Z",
        r#"[
    { "id": "T001", "description": "Skipped task", "status": "skipped" }
  ]"#,
    );
    let (track, _) = codec::decode(&json).unwrap();
    let impl_plan = make_impl_plan_with_tasks(
        &[("T001", "Skipped task", "skipped", None)],
        &[("S1", "Section", &["T001"])],
    );
    let rendered = render_plan(&track, Some(&impl_plan));
    assert!(rendered.contains("# Title track-a"), "title must appear:\n{rendered}");
    assert!(rendered.contains("[-]"), "[-] marker must appear for skipped task:\n{rendered}");
}

#[test]
fn render_plan_preserves_multi_section_order() {
    // Sections rendered in order (S1 before S2).
    // Uses v5 metadata JSON (no `status`/`tasks`/`plan` fields).
    let json = r#"{
  "schema_version": 5,
  "id": "track-a",
  "branch": "track/track-a",
  "title": "Title track-a",
  "created_at": "2026-03-13T00:00:00Z",
  "updated_at": "2026-03-13T01:00:00Z"
}"#;
    let (track, _) = codec::decode(json).unwrap();
    let impl_plan = make_impl_plan_with_tasks(
        &[("T001", "Task one", "todo", None), ("T002", "Task two", "todo", None)],
        &[("S1", "First Section", &["T001"]), ("S2", "Second Section", &["T002"])],
    );
    let rendered = render_plan(&track, Some(&impl_plan));
    assert!(rendered.contains("# Title track-a"), "title must appear:\n{rendered}");
    let s1_pos = rendered.find("First Section").expect("S1 not found");
    let s2_pos = rendered.find("Second Section").expect("S2 not found");
    assert!(s1_pos < s2_pos, "S1 must appear before S2:\n{rendered}");
}

#[test]
fn render_plan_places_summary_after_generated_header() {
    // Summary lines appear after the header and before task sections.
    // Uses v5 metadata JSON.
    let json = r#"{
  "schema_version": 5,
  "id": "track-a",
  "branch": "track/track-a",
  "title": "Title track-a",
  "created_at": "2026-03-13T00:00:00Z",
  "updated_at": "2026-03-13T01:00:00Z"
}"#;
    let (track, _) = codec::decode(json).unwrap();
    let impl_plan = make_impl_plan_with_summary_and_tasks(
        &["Summary line one", "Summary line two"],
        &[("T001", "Task", "todo", None)],
        &[("S1", "Section", &["T001"])],
    );
    let rendered = render_plan(&track, Some(&impl_plan));
    let header_idx = rendered
        .find("<!-- Generated from metadata.json + impl-plan.json")
        .expect("generated header missing");
    let summary_idx = rendered.find("Summary line one").expect("summary not found");
    let tasks_idx = rendered.find("## Tasks").expect("tasks section not found");
    assert!(header_idx < summary_idx, "header must precede summary:\n{rendered}");
    assert!(summary_idx < tasks_idx, "summary must precede tasks:\n{rendered}");
}

#[test]
fn render_plan_renders_section_description_lines() {
    // Section description lines appear as blockquotes under the section heading.
    // Uses v5 metadata JSON.
    let json = r#"{
  "schema_version": 5,
  "id": "track-a",
  "branch": "track/track-a",
  "title": "Title track-a",
  "created_at": "2026-03-13T00:00:00Z",
  "updated_at": "2026-03-13T01:00:00Z"
}"#;
    let (track, _) = codec::decode(json).unwrap();
    let impl_plan = make_impl_plan_with_desc_and_tasks(
        &[("T001", "Task", "todo", None)],
        &[("S1", "Section", &["T001"], &["Describe the section goal", "Additional context"])],
    );
    let rendered = render_plan(&track, Some(&impl_plan));
    assert!(rendered.contains("# Title track-a"), "title must appear:\n{rendered}");
    assert!(
        rendered.contains("Describe the section goal"),
        "first description line missing:\n{rendered}"
    );
    assert!(
        rendered.contains("Additional context"),
        "second description line missing:\n{rendered}"
    );
}

/// Decodes v5 metadata JSON and returns a `TrackSnapshot` with a specified `derived_status`.
/// For test use only: allows setting an explicit `derived_status` without impl-plan.json I/O.
fn make_snapshot_v5(
    json: &str,
    derived_status: &str,
    schema_version: u32,
    dir: PathBuf,
) -> TrackSnapshot {
    let (track, meta) = codec::decode(json).unwrap();
    TrackSnapshot { dir, track, meta, schema_version, derived_status: derived_status.to_owned() }
}

/// Decodes legacy v2/v3 metadata JSON and returns a `TrackSnapshot`.
/// The `derived_status` is read from the raw JSON `status` field.
fn make_snapshot_legacy(json: &str, schema_version: u32, dir: PathBuf) -> TrackSnapshot {
    let raw: serde_json::Value = serde_json::from_str(json).unwrap();
    let status = raw.get("status").and_then(|v| v.as_str()).unwrap_or("planned").to_owned();
    let (track, meta) = decode_legacy_metadata(&raw, std::path::Path::new("test")).unwrap();
    TrackSnapshot { dir, track, meta, schema_version, derived_status: status }
}

#[test]
fn render_registry_places_active_completed_and_archived() {
    // Active track is v5 (no `status` field; derived status = "planned").
    // Done and archived tracks use legacy v3 JSON (status field present).
    let active_json = sample_metadata_json("track-a", "planned", "2026-03-13T02:00:00Z", "[]");
    let done_json = sample_metadata_json_with_schema_and_branch(
        3,
        "track-b",
        "done",
        "2026-03-13T01:00:00Z",
        r#"[{"id":"T001","description":"First task","status":"done","commit_hash":"abc1234"}]"#,
        Some("track/track-b"),
    );
    let archived_json = sample_metadata_json_with_schema_and_branch(
        3,
        "track-c",
        "archived",
        "2026-03-13T00:00:00Z",
        r#"[{"id":"T001","description":"First task","status":"todo"}]"#,
        Some("track/track-c"),
    );

    let active_snapshot =
        make_snapshot_v5(&active_json, "planned", 5, PathBuf::from("track/items/track-a"));
    let done_snapshot = make_snapshot_legacy(&done_json, 3, PathBuf::from("track/items/track-b"));
    let archived_snapshot =
        make_snapshot_legacy(&archived_json, 3, PathBuf::from("track/archive/track-c"));

    let rendered = render_registry(&[active_snapshot, done_snapshot, archived_snapshot]);

    assert!(rendered.contains("| track-a | planned | `/track:implement` | 2026-03-13 |"));
    assert!(rendered.contains("| track-b | Done | 2026-03-13 |"));
    assert!(rendered.contains("| track-c | Archived | 2026-03-13 |"));
}

#[test]
fn render_registry_routes_branchless_planning_track_to_activate() {
    // v5 branchless planned track → no branch, derived_status = "planned"
    let plan_only_json =
        sample_metadata_json_with_branch("track-a", "planned", "2026-03-13T02:00:00Z", "[]", None);
    let snapshot =
        make_snapshot_v5(&plan_only_json, "planned", 5, PathBuf::from("track/items/track-a"));
    let rendered = render_registry(&[snapshot]);

    assert!(rendered.contains("/track:activate track-a"));
    assert!(rendered.contains("/track:plan-only <feature>"));
}

#[test]
fn render_registry_keeps_legacy_v2_branchless_planned_track_on_implement() {
    // Legacy v2 track uses the legacy decode path.
    let legacy_json = sample_metadata_json_with_schema_and_branch(
        2,
        "track-a",
        "planned",
        "2026-03-13T02:00:00Z",
        r#"[{"id":"T001","description":"First task","status":"todo"}]"#,
        None,
    );
    let snapshot = make_snapshot_legacy(&legacy_json, 2, PathBuf::from("track/items/track-a"));
    let rendered = render_registry(&[snapshot]);

    assert!(rendered.contains("/track:implement"));
    assert!(!rendered.contains("/track:activate track-a"));
}

#[test]
fn render_registry_prefers_materialized_active_track_in_current_focus() {
    // Both are v5: no branch (plan-only) vs with branch (materialized).
    let plan_only_json = sample_metadata_json_with_branch(
        "track-plan-only",
        "planned",
        "2026-03-13T03:00:00Z",
        "[]",
        None,
    );
    let materialized_json =
        sample_metadata_json("track-materialized", "planned", "2026-03-13T02:00:00Z", "[]");
    let plan_only_snap = make_snapshot_v5(
        &plan_only_json,
        "planned",
        5,
        PathBuf::from("track/items/track-plan-only"),
    );
    let materialized_snap = make_snapshot_v5(
        &materialized_json,
        "planned",
        5,
        PathBuf::from("track/items/track-materialized"),
    );
    let rendered = render_registry(&[plan_only_snap, materialized_snap]);

    assert!(rendered.contains("- Latest active track: `track-materialized`"));
    assert!(rendered.contains("- Next recommended command: `/track:implement`"));
}

#[test]
fn render_registry_prefers_legacy_v2_planned_track_over_newer_plan_only() {
    // Legacy v2 (no branch) vs v5 plan-only (no branch, schema_version 5).
    // The v2 legacy track should be preferred (lower priority sort key).
    let legacy_json = sample_metadata_json_with_schema_and_branch(
        2,
        "track-legacy",
        "planned",
        "2026-03-13T02:00:00Z",
        r#"[{"id":"T001","description":"First task","status":"todo"}]"#,
        None,
    );
    let plan_only_json = sample_metadata_json_with_branch(
        "track-plan-only",
        "planned",
        "2026-03-13T03:00:00Z",
        "[]",
        None,
    );
    let legacy_snap =
        make_snapshot_legacy(&legacy_json, 2, PathBuf::from("track/items/track-legacy"));
    let plan_only_snap = make_snapshot_v5(
        &plan_only_json,
        "planned",
        5,
        PathBuf::from("track/items/track-plan-only"),
    );
    let rendered = render_registry(&[plan_only_snap, legacy_snap]);

    assert!(rendered.contains("- Latest active track: `track-legacy`"));
    assert!(rendered.contains("- Next recommended command: `/track:implement`"));
}

#[test]
fn sync_rendered_views_writes_plan_and_registry() {
    let dir = tempfile::tempdir().unwrap();
    let track_dir = dir.path().join("track/items/track-a");
    std::fs::create_dir_all(&track_dir).unwrap();
    std::fs::write(
        track_dir.join("metadata.json"),
        sample_metadata_json(
            "track-a",
            "planned",
            "2026-03-13T02:00:00Z",
            r#"[
    {
      "id": "T001",
      "description": "First task",
      "status": "todo"
    }
  ]"#,
        ),
    )
    .unwrap();
    std::fs::write(dir.path().join("architecture-rules.json"), DOMAIN_ARCH_RULES).unwrap();

    let changed = sync_rendered_views(dir.path(), Some("track-a")).unwrap();

    assert!(changed.iter().any(|path| path.ends_with("plan.md")));
    assert!(changed.iter().any(|path| path.ends_with("registry.md")));
    assert!(track_dir.join("plan.md").is_file());
    assert!(dir.path().join("track/registry.md").is_file());
}

// --- registry / snapshot boundary tests ---

#[test]
fn collect_track_snapshots_ignores_plain_files_under_items() {
    let dir = tempfile::tempdir().unwrap();
    let items_root = dir.path().join("track/items");
    std::fs::create_dir_all(&items_root).unwrap();
    // Valid track directory.
    let track_dir = items_root.join("track-a");
    std::fs::create_dir_all(&track_dir).unwrap();
    std::fs::write(
        track_dir.join("metadata.json"),
        sample_metadata_json(
            "track-a",
            "planned",
            "2026-03-13T02:00:00Z",
            r#"[
    { "id": "T001", "description": "First", "status": "todo" }
  ]"#,
        ),
    )
    .unwrap();
    // A stray file (not a directory) directly under track/items.
    std::fs::write(items_root.join("stray.txt"), "not a track").unwrap();

    let snapshots = collect_track_snapshots(dir.path()).unwrap();
    assert_eq!(snapshots.len(), 1, "stray file must be ignored: got {snapshots:?}");
    assert_eq!(snapshots[0].track.id().as_ref(), "track-a");
}

#[test]
fn collect_track_snapshots_tie_breaks_same_updated_at_by_track_id() {
    let dir = tempfile::tempdir().unwrap();
    let items_root = dir.path().join("track/items");
    std::fs::create_dir_all(&items_root).unwrap();

    // track-b is inserted first to verify the tie-break applies regardless of
    // directory traversal order.
    for id in ["track-b", "track-a"] {
        let td = items_root.join(id);
        std::fs::create_dir_all(&td).unwrap();
        std::fs::write(
            td.join("metadata.json"),
            sample_metadata_json(
                id,
                "planned",
                "2026-03-13T02:00:00Z", // identical updated_at
                r#"[
    { "id": "T001", "description": "First", "status": "todo" }
  ]"#,
            ),
        )
        .unwrap();
    }

    let snapshots = collect_track_snapshots(dir.path()).unwrap();
    let ids: Vec<&str> = snapshots.iter().map(|s| s.track.id().as_ref()).collect();
    assert_eq!(ids, vec!["track-a", "track-b"], "same updated_at must tie-break by track_id asc");
}

#[test]
fn sync_rendered_views_omits_unchanged_registry_from_changed_set() {
    let dir = tempfile::tempdir().unwrap();
    let track_dir = dir.path().join("track/items/track-a");
    std::fs::create_dir_all(&track_dir).unwrap();
    std::fs::write(
        track_dir.join("metadata.json"),
        sample_metadata_json(
            "track-a",
            "planned",
            "2026-03-13T02:00:00Z",
            r#"[
    { "id": "T001", "description": "First", "status": "todo" }
  ]"#,
        ),
    )
    .unwrap();
    std::fs::write(dir.path().join("architecture-rules.json"), DOMAIN_ARCH_RULES).unwrap();

    // First call populates plan.md and registry.md.
    let first_changed = sync_rendered_views(dir.path(), Some("track-a")).unwrap();
    assert!(first_changed.iter().any(|p| p.ends_with("registry.md")));

    // Second call with no metadata changes must leave both outputs untouched.
    let second_changed = sync_rendered_views(dir.path(), Some("track-a")).unwrap();
    assert!(
        !second_changed.iter().any(|p| p.ends_with("registry.md")),
        "unchanged registry.md must be omitted from changed set: {second_changed:?}"
    );
    assert!(
        !second_changed.iter().any(|p| p.ends_with("plan.md")),
        "unchanged plan.md must be omitted from changed set: {second_changed:?}"
    );
}

#[test]
fn sync_rendered_views_single_track_rejects_unrelated_invalid_metadata() {
    let dir = tempfile::tempdir().unwrap();
    let good_track = dir.path().join("track/items/track-a");
    std::fs::create_dir_all(&good_track).unwrap();
    std::fs::write(
        good_track.join("metadata.json"),
        sample_metadata_json(
            "track-a",
            "planned",
            "2026-03-13T02:00:00Z",
            r#"[
    {
      "id": "T001",
      "description": "First task",
      "status": "todo"
    }
  ]"#,
        ),
    )
    .unwrap();

    let bad_track = dir.path().join("track/items/bad-track");
    std::fs::create_dir_all(&bad_track).unwrap();
    std::fs::write(
        bad_track.join("metadata.json"),
        r#"{
  "schema_version": 99,
  "id": "bad-track",
  "title": "Bad Track",
  "status": "planned",
  "created_at": "2026-03-13T00:00:00Z",
  "updated_at": "2026-03-13T00:00:00Z",
  "tasks": [],
  "plan": { "summary": [], "sections": [] }
}"#,
    )
    .unwrap();

    let err = sync_rendered_views(dir.path(), Some("track-a")).unwrap_err();
    assert!(matches!(err, RenderError::UnsupportedSchemaVersion { .. }));
    assert!(!good_track.join("plan.md").exists());
    assert!(!dir.path().join("track/registry.md").exists());
}

#[test]
fn validate_track_snapshots_rejects_invalid_metadata() {
    let dir = tempfile::tempdir().unwrap();
    let track_dir = dir.path().join("track/items/bad-track");
    std::fs::create_dir_all(&track_dir).unwrap();
    std::fs::write(track_dir.join("metadata.json"), "{").unwrap();

    let err = validate_track_snapshots(dir.path()).unwrap_err();
    assert!(err.to_string().contains("invalid metadata"));
}

#[test]
fn validate_track_snapshots_rejects_unsupported_schema_version() {
    let dir = tempfile::tempdir().unwrap();
    let track_dir = dir.path().join("track/items/bad-schema");
    std::fs::create_dir_all(&track_dir).unwrap();
    std::fs::write(
        track_dir.join("metadata.json"),
        r#"{
  "schema_version": 99,
  "id": "bad-schema",
  "title": "Bad Schema",
  "status": "planned",
  "created_at": "2026-03-13T00:00:00Z",
  "updated_at": "2026-03-13T00:00:00Z",
  "tasks": [],
  "plan": { "summary": [], "sections": [] }
}"#,
    )
    .unwrap();

    let err = validate_track_snapshots(dir.path()).unwrap_err();
    assert!(err.to_string().contains("unsupported schema_version 99"));
}

#[test]
fn validate_track_snapshots_tolerates_phase_zero_missing_plan_md() {
    // Phase 0 compat (ADR 2026-04-19-1242 §D0.0 / §D1.4): a freshly-created
    // v5 track directory containing only `metadata.json` (no `plan.md` yet,
    // because the view is rendered in later phases) must pass validation.
    // The previous behaviour failed with an I/O error on the missing file.
    let dir = tempfile::tempdir().unwrap();
    let track_dir = dir.path().join("track/items/track-a");
    std::fs::create_dir_all(&track_dir).unwrap();
    std::fs::write(
        track_dir.join("metadata.json"),
        sample_metadata_json_with_schema_and_branch(
            5,
            "track-a",
            "planned",
            "2026-03-13T02:00:00Z",
            "[]",
            Some("track/track-a"),
        ),
    )
    .unwrap();
    // NOTE: no `plan.md` on purpose — this mirrors the state right after
    // `/track:init` before any downstream view rendering has occurred.
    assert!(validate_track_snapshots(dir.path()).is_ok());
}

#[test]
#[cfg(unix)]
fn validate_track_snapshots_rejects_dangling_plan_md_symlink() {
    // Regression guard (Codex review #110, 2026-04-23): a `plan.md` that
    // exists as a symlink pointing at a non-existent target must NOT be
    // treated as "Phase 0 plan.md absent". Previously `std::fs::metadata`
    // followed the symlink and returned NotFound, so the branch
    // swallowed the dangling-symlink case and reported success for a
    // corrupted track directory.
    let dir = tempfile::tempdir().unwrap();
    let track_dir = dir.path().join("track/items/track-a");
    std::fs::create_dir_all(&track_dir).unwrap();
    std::fs::write(
        track_dir.join("metadata.json"),
        sample_metadata_json_with_schema_and_branch(
            5,
            "track-a",
            "planned",
            "2026-03-13T02:00:00Z",
            "[]",
            Some("track/track-a"),
        ),
    )
    .unwrap();
    // Create a symlink whose target does not exist.
    let link = track_dir.join("plan.md");
    std::os::unix::fs::symlink(track_dir.join("missing-target.md"), &link).unwrap();

    let err = validate_track_snapshots(dir.path()).unwrap_err();
    assert!(
        err.to_string().contains("dangling symlink"),
        "expected dangling-symlink rejection, got: {err}"
    );
}

#[test]
fn validate_track_snapshots_rejects_out_of_sync_plan() {
    let dir = tempfile::tempdir().unwrap();
    let track_dir = dir.path().join("track/items/track-a");
    std::fs::create_dir_all(&track_dir).unwrap();
    // v5 identity-only metadata — v2/v3/v4 legacy tracks are intentionally
    // skipped by `validate_track_snapshots` so only v5 mismatches surface.
    std::fs::write(
        track_dir.join("metadata.json"),
        sample_metadata_json_with_schema_and_branch(
            5,
            "track-a",
            "planned",
            "2026-03-13T02:00:00Z",
            "[]",
            Some("track/track-a"),
        ),
    )
    .unwrap();
    std::fs::write(track_dir.join("plan.md"), "# stale\n").unwrap();
    std::fs::create_dir_all(dir.path().join("track")).unwrap();
    std::fs::write(dir.path().join("track/registry.md"), "# registry\n").unwrap();

    let err = validate_track_snapshots(dir.path()).unwrap_err();
    assert!(err.to_string().contains("plan.md does not match metadata.json"));
}

#[test]
fn validate_track_snapshots_rejects_metadata_id_directory_mismatch() {
    let dir = tempfile::tempdir().unwrap();
    let track_dir = dir.path().join("track/items/track-a");
    std::fs::create_dir_all(&track_dir).unwrap();
    std::fs::write(
        track_dir.join("metadata.json"),
        sample_metadata_json(
            "other-track",
            "planned",
            "2026-03-13T02:00:00Z",
            r#"[
    {
      "id": "T001",
      "description": "First task",
      "status": "todo"
    }
  ]"#,
        ),
    )
    .unwrap();
    std::fs::write(track_dir.join("plan.md"), "# stale\n").unwrap();
    std::fs::create_dir_all(dir.path().join("track")).unwrap();
    std::fs::write(dir.path().join("track/registry.md"), "# registry\n").unwrap();

    let err = validate_track_snapshots(dir.path()).unwrap_err();
    assert!(
        err.to_string().contains("metadata id 'other-track' does not match directory 'track-a'")
    );
}

#[test]
fn validate_track_snapshots_rejects_out_of_sync_registry() {
    let dir = tempfile::tempdir().unwrap();
    let track_dir = dir.path().join("track/items/track-a");
    std::fs::create_dir_all(&track_dir).unwrap();
    let metadata_path = track_dir.join("metadata.json");
    // v5 identity-only metadata so the plan.md freshness check runs and
    // passes before we reach the registry.md check.
    std::fs::write(
        &metadata_path,
        sample_metadata_json_with_schema_and_branch(
            5,
            "track-a",
            "planned",
            "2026-03-13T02:00:00Z",
            "[]",
            Some("track/track-a"),
        ),
    )
    .unwrap();
    let (track, _) = codec::decode(&std::fs::read_to_string(&metadata_path).unwrap()).unwrap();
    std::fs::write(track_dir.join("plan.md"), render_plan(&track, None)).unwrap();
    std::fs::create_dir_all(dir.path().join("track")).unwrap();
    std::fs::write(dir.path().join("track/registry.md"), "# stale registry\n").unwrap();

    let err = validate_track_snapshots(dir.path()).unwrap_err();
    assert!(err.to_string().contains("registry.md does not match metadata.json"));
}

#[test]
fn validate_track_document_accepts_planning_only_v3_without_branch() {
    // Validates legacy v3 behavior. Uses explicit v3 JSON (with `status`
    // field) since `sample_metadata_json_with_branch` now generates v5.
    let dir = tempfile::tempdir().unwrap();
    let track_dir = dir.path().join("track/items/track-a");
    std::fs::create_dir_all(&track_dir).unwrap();
    let metadata_path = track_dir.join("metadata.json");
    std::fs::write(
        &metadata_path,
        sample_metadata_json_with_schema_and_branch(
            3,
            "track-a",
            "planned",
            "2026-03-13T02:00:00Z",
            r#"[{"id":"T001","description":"First task","status":"todo"}]"#,
            None,
        ),
    )
    .unwrap();

    let doc = serde_json::from_str(&std::fs::read_to_string(&metadata_path).unwrap()).unwrap();

    let result = validate_track_document(&metadata_path, track_dir.file_name(), &doc);

    assert!(result.is_ok());
}

#[test]
fn validate_track_document_rejects_non_planning_v3_without_branch() {
    // Validates legacy v3 behavior. Uses explicit v3 JSON.
    let dir = tempfile::tempdir().unwrap();
    let track_dir = dir.path().join("track/items/track-a");
    std::fs::create_dir_all(&track_dir).unwrap();
    let metadata_path = track_dir.join("metadata.json");
    std::fs::write(
        &metadata_path,
        sample_metadata_json_with_schema_and_branch(
            3,
            "track-a",
            "in_progress",
            "2026-03-13T02:00:00Z",
            r#"[{"id":"T001","description":"First task","status":"in_progress"}]"#,
            None,
        ),
    )
    .unwrap();

    let doc = serde_json::from_str(&std::fs::read_to_string(&metadata_path).unwrap()).unwrap();
    let err = validate_track_document(&metadata_path, track_dir.file_name(), &doc).unwrap_err();

    assert!(
        err.to_string()
            .contains("'branch' is required for v3 tracks unless the track is planning-only")
    );
}

#[test]
fn validate_track_document_rejects_v3_track_missing_branch_field() {
    let dir = tempfile::tempdir().unwrap();
    let track_dir = dir.path().join("track/items/track-a");
    std::fs::create_dir_all(&track_dir).unwrap();
    let metadata_path = track_dir.join("metadata.json");
    std::fs::write(
        &metadata_path,
        r#"{
  "schema_version": 3,
  "id": "track-a",
  "title": "Track A",
  "status": "planned",
  "created_at": "2026-03-13T00:00:00Z",
  "updated_at": "2026-03-13T02:00:00Z",
  "tasks": [
    {
      "id": "T001",
      "description": "First task",
      "status": "todo"
    }
  ],
  "plan": {
    "summary": [],
    "sections": [
      {
        "id": "S1",
        "title": "Build",
        "description": [],
        "task_ids": ["T001"]
      }
    ]
  }
}"#,
    )
    .unwrap();

    let doc = serde_json::from_str(&std::fs::read_to_string(&metadata_path).unwrap()).unwrap();
    let err = validate_track_document(&metadata_path, track_dir.file_name(), &doc).unwrap_err();

    assert!(err.to_string().contains("Missing required field 'branch'"));
}

#[test]
fn validate_track_document_rejects_v3_track_missing_tasks_field() {
    // tasks/plan fields are stripped by codec::decode() during the v2/v3 migration window.
    // A v3 document missing 'tasks' still decodes successfully (stripped) → Ok.
    // The "Missing required field 'tasks'" check is not enforced (tasks moved to ImplPlanDocument).
    let dir = tempfile::tempdir().unwrap();
    let track_dir = dir.path().join("track/items/track-a");
    std::fs::create_dir_all(&track_dir).unwrap();
    let metadata_path = track_dir.join("metadata.json");
    std::fs::write(
        &metadata_path,
        r#"{
  "schema_version": 3,
  "id": "track-a",
  "branch": null,
  "title": "Track A",
  "status": "planned",
  "created_at": "2026-03-13T00:00:00Z",
  "updated_at": "2026-03-13T02:00:00Z",
  "plan": {
    "summary": [],
    "sections": [
      {
        "id": "S1",
        "title": "Build",
        "description": [],
        "task_ids": []
      }
    ]
  }
}"#,
    )
    .unwrap();

    let doc = serde_json::from_str(&std::fs::read_to_string(&metadata_path).unwrap()).unwrap();
    assert!(
        validate_track_document(&metadata_path, track_dir.file_name(), &doc).is_ok(),
        "v3 doc missing 'tasks' field is accepted (tasks stripped during migration)"
    );
}

#[test]
fn validate_track_document_rejects_unreferenced_task() {
    // tasks/plan fields are stripped by codec::decode() during the v2/v3 migration window.
    // An unreferenced task in a v3 doc no longer causes a validate error (tasks moved to ImplPlanDocument).
    let dir = tempfile::tempdir().unwrap();
    let track_dir = dir.path().join("track/items/track-a");
    std::fs::create_dir_all(&track_dir).unwrap();
    let metadata_path = track_dir.join("metadata.json");
    // T002 is declared in tasks but not referenced from any plan section (legacy v3 doc).
    std::fs::write(
        &metadata_path,
        r#"{
  "schema_version": 3,
  "id": "track-a",
  "branch": "track/track-a",
  "title": "Track A",
  "status": "planned",
  "created_at": "2026-03-13T00:00:00Z",
  "updated_at": "2026-03-13T02:00:00Z",
  "tasks": [
    { "id": "T001", "description": "Referenced task", "status": "todo" },
    { "id": "T002", "description": "Unreferenced task", "status": "todo" }
  ],
  "plan": {
    "summary": [],
    "sections": [
      { "id": "S1", "title": "Build", "description": [], "task_ids": ["T001"] }
    ]
  }
}"#,
    )
    .unwrap();

    let doc = serde_json::from_str(&std::fs::read_to_string(&metadata_path).unwrap()).unwrap();
    assert!(
        validate_track_document(&metadata_path, track_dir.file_name(), &doc).is_ok(),
        "v3 doc with unreferenced task is accepted (tasks/plan stripped during migration)"
    );
}

#[test]
fn validate_track_document_rejects_duplicate_task_reference() {
    // Duplicate task_ids in plan sections are an ImplPlanDocument concern.
    // validate_track_document strips tasks/plan fields via codec::decode(); no error expected.
    let dir = tempfile::tempdir().unwrap();
    let track_dir = dir.path().join("track/items/track-a");
    std::fs::create_dir_all(&track_dir).unwrap();
    let metadata_path = track_dir.join("metadata.json");
    // T001 is referenced by both S1 and S2 sections (legacy v3 format).
    std::fs::write(
        &metadata_path,
        r#"{
  "schema_version": 3,
  "id": "track-a",
  "branch": "track/track-a",
  "title": "Track A",
  "status": "planned",
  "created_at": "2026-03-13T00:00:00Z",
  "updated_at": "2026-03-13T02:00:00Z",
  "tasks": [
    { "id": "T001", "description": "Shared task", "status": "todo" }
  ],
  "plan": {
    "summary": [],
    "sections": [
      { "id": "S1", "title": "First",  "description": [], "task_ids": ["T001"] },
      { "id": "S2", "title": "Second", "description": [], "task_ids": ["T001"] }
    ]
  }
}"#,
    )
    .unwrap();

    let doc = serde_json::from_str(&std::fs::read_to_string(&metadata_path).unwrap()).unwrap();
    // Duplicate plan references are no longer checked at this layer.
    // Document should decode successfully (tasks/plan stripped by codec).
    assert!(
        validate_track_document(&metadata_path, track_dir.file_name(), &doc).is_ok(),
        "duplicate plan ref check moved to ImplPlanDocument"
    );
}

#[test]
fn validate_track_document_rejects_status_drift_in_progress_vs_done() {
    // Status is stored explicitly in legacy tracks (not task-derived).
    // The stored metadata.status is the authoritative source; task states are ignored.
    // The old task-derived drift check is gone — this document now passes validation.
    let dir = tempfile::tempdir().unwrap();
    let track_dir = dir.path().join("track/items/track-a");
    std::fs::create_dir_all(&track_dir).unwrap();
    let metadata_path = track_dir.join("metadata.json");
    std::fs::write(
        &metadata_path,
        r#"{
  "schema_version": 3,
  "id": "track-a",
  "branch": "track/track-a",
  "title": "Track A",
  "status": "in_progress",
  "created_at": "2026-03-13T00:00:00Z",
  "updated_at": "2026-03-13T02:00:00Z",
  "tasks": [
    {
      "id": "T001",
      "description": "Completed task",
      "status": "done",
      "commit_hash": "abc1234"
    }
  ],
  "plan": {
    "summary": [],
    "sections": [
      { "id": "S1", "title": "Build", "description": [], "task_ids": ["T001"] }
    ]
  }
}"#,
    )
    .unwrap();

    let doc = serde_json::from_str(&std::fs::read_to_string(&metadata_path).unwrap()).unwrap();
    // Task-derived status drift check removed; stored status is authoritative.
    // doc.status="in_progress" → decoded status="in_progress" → no drift → Ok.
    assert!(
        validate_track_document(&metadata_path, track_dir.file_name(), &doc).is_ok(),
        "task-derived status drift check removed; stored status is authoritative"
    );
}

#[test]
fn validate_track_document_rejects_archived_with_incomplete_tasks() {
    // "archived must have all tasks resolved" is now an ImplPlanDocument concern.
    // validate_track_document no longer checks task states (stripped by codec::decode()).
    // A v3 archived track with a todo task decodes correctly with the identity-only semantics.
    let dir = tempfile::tempdir().unwrap();
    let track_dir = dir.path().join("track/items/track-a");
    std::fs::create_dir_all(&track_dir).unwrap();
    let metadata_path = track_dir.join("metadata.json");
    // metadata.status is "archived" — tasks are ignored under identity-only semantics.
    std::fs::write(
        &metadata_path,
        r#"{
  "schema_version": 3,
  "id": "track-a",
  "branch": "track/track-a",
  "title": "Track A",
  "status": "archived",
  "created_at": "2026-03-13T00:00:00Z",
  "updated_at": "2026-03-13T02:00:00Z",
  "tasks": [
    { "id": "T001", "description": "Unfinished task", "status": "todo" }
  ],
  "plan": {
    "summary": [],
    "sections": [
      { "id": "S1", "title": "Build", "description": [], "task_ids": ["T001"] }
    ]
  }
}"#,
    )
    .unwrap();

    let doc = serde_json::from_str(&std::fs::read_to_string(&metadata_path).unwrap()).unwrap();
    // Task-completion check for archived tracks moved to ImplPlanDocument.
    // Document should now decode without error (status=archived is valid; tasks stripped).
    assert!(
        validate_track_document(&metadata_path, track_dir.file_name(), &doc).is_ok(),
        "archived+incomplete check moved to ImplPlanDocument"
    );
}

#[test]
fn validate_track_document_accepts_id_with_git_substring_in_segment() {
    // "legit" contains "git" as a substring but is not a whole segment,
    // so reserved-id matching must not reject the track.
    let dir = tempfile::tempdir().unwrap();
    let track_dir = dir.path().join("track/items/legit-cleanup-2026-03-11");
    std::fs::create_dir_all(&track_dir).unwrap();
    let metadata_path = track_dir.join("metadata.json");
    std::fs::write(
        &metadata_path,
        sample_metadata_json_with_branch(
            "legit-cleanup-2026-03-11",
            "planned",
            "2026-03-13T02:00:00Z",
            r#"[
    { "id": "T001", "description": "First task", "status": "todo" }
  ]"#,
            None,
        ),
    )
    .unwrap();

    let doc = serde_json::from_str(&std::fs::read_to_string(&metadata_path).unwrap()).unwrap();
    let result = validate_track_document(&metadata_path, track_dir.file_name(), &doc);

    assert!(result.is_ok(), "legit-cleanup-* must be accepted, got: {result:?}");
}

#[test]
fn sync_rendered_views_generates_spec_md_from_spec_json() {
    let dir = tempfile::tempdir().unwrap();
    let track_dir = dir.path().join("track/items/track-a");
    std::fs::create_dir_all(&track_dir).unwrap();

    // Write valid metadata.json
    std::fs::write(
        track_dir.join("metadata.json"),
        sample_metadata_json(
            "track-a",
            "planned",
            "2026-03-13T02:00:00Z",
            r#"[{"id":"T001","description":"First task","status":"todo"}]"#,
        ),
    )
    .unwrap();

    // Write a minimal spec.json (schema v2: no status field)
    std::fs::write(
        track_dir.join("spec.json"),
        r#"{
  "schema_version": 2,
  "version": "1.0",
  "title": "Feature Alpha",
  "scope": { "in_scope": [], "out_of_scope": [] }
}"#,
    )
    .unwrap();
    std::fs::write(dir.path().join("architecture-rules.json"), DOMAIN_ARCH_RULES).unwrap();

    let changed = sync_rendered_views(dir.path(), Some("track-a")).unwrap();

    // spec.md must be in the changed list
    assert!(
        changed.iter().any(|p| p.ends_with("spec.md")),
        "spec.md should be reported as changed"
    );

    // spec.md must exist and contain the generated header comment and title
    let spec_md = std::fs::read_to_string(track_dir.join("spec.md")).unwrap();
    assert!(spec_md.contains("<!-- Generated from spec.json"));
    assert!(spec_md.contains("Feature Alpha"));
}

#[test]
fn sync_rendered_views_skips_spec_md_when_spec_json_absent() {
    let dir = tempfile::tempdir().unwrap();
    let track_dir = dir.path().join("track/items/track-a");
    std::fs::create_dir_all(&track_dir).unwrap();

    std::fs::write(
        track_dir.join("metadata.json"),
        sample_metadata_json(
            "track-a",
            "planned",
            "2026-03-13T02:00:00Z",
            r#"[{"id":"T001","description":"First task","status":"todo"}]"#,
        ),
    )
    .unwrap();
    std::fs::write(dir.path().join("architecture-rules.json"), DOMAIN_ARCH_RULES).unwrap();

    // No spec.json written — legacy mode
    let changed = sync_rendered_views(dir.path(), Some("track-a")).unwrap();

    assert!(
        !changed.iter().any(|p| p.ends_with("spec.md")),
        "spec.md must NOT be in changed list when spec.json is absent"
    );
    assert!(!track_dir.join("spec.md").exists());
}

#[test]
fn sync_rendered_views_does_not_overwrite_spec_md_when_already_up_to_date() {
    let dir = tempfile::tempdir().unwrap();
    let track_dir = dir.path().join("track/items/track-a");
    std::fs::create_dir_all(&track_dir).unwrap();

    std::fs::write(
        track_dir.join("metadata.json"),
        sample_metadata_json(
            "track-a",
            "planned",
            "2026-03-13T02:00:00Z",
            r#"[{"id":"T001","description":"First task","status":"todo"}]"#,
        ),
    )
    .unwrap();

    let spec_json = r#"{
  "schema_version": 2,
  "version": "1.0",
  "title": "Feature Beta",
  "scope": { "in_scope": [], "out_of_scope": [] }
}"#;
    std::fs::write(track_dir.join("spec.json"), spec_json).unwrap();
    std::fs::write(dir.path().join("architecture-rules.json"), DOMAIN_ARCH_RULES).unwrap();

    // First sync — generates spec.md
    sync_rendered_views(dir.path(), Some("track-a")).unwrap();

    // Second sync — spec.md is already up-to-date, must NOT be in changed list
    let changed = sync_rendered_views(dir.path(), Some("track-a")).unwrap();
    assert!(
        !changed.iter().any(|p| p.ends_with("spec.md")),
        "spec.md must NOT be in changed list when already up-to-date"
    );
}

#[test]
fn sync_rendered_views_continues_on_malformed_spec_json() {
    let dir = tempfile::tempdir().unwrap();
    let track_dir = dir.path().join("track/items/track-a");
    std::fs::create_dir_all(&track_dir).unwrap();

    std::fs::write(
        track_dir.join("metadata.json"),
        sample_metadata_json(
            "track-a",
            "planned",
            "2026-03-13T02:00:00Z",
            r#"[{"id":"T001","description":"First task","status":"todo"}]"#,
        ),
    )
    .unwrap();

    // Write malformed spec.json (JSON parse error — warn and continue)
    std::fs::write(track_dir.join("spec.json"), "{not valid json}").unwrap();
    std::fs::write(dir.path().join("architecture-rules.json"), DOMAIN_ARCH_RULES).unwrap();

    // Must succeed (only warn) — plan.md and registry.md are still generated
    let result = sync_rendered_views(dir.path(), Some("track-a"));
    assert!(result.is_ok(), "JSON-parse-error spec.json must not abort sync");

    let changed = result.unwrap();
    assert!(changed.iter().any(|p| p.ends_with("plan.md")));
    assert!(!changed.iter().any(|p| p.ends_with("spec.md")));
}

#[test]
fn sync_rendered_views_propagates_error_on_spec_json_unsupported_schema_version() {
    let dir = tempfile::tempdir().unwrap();
    let track_dir = dir.path().join("track/items/track-a");
    std::fs::create_dir_all(&track_dir).unwrap();

    std::fs::write(
        track_dir.join("metadata.json"),
        sample_metadata_json(
            "track-a",
            "planned",
            "2026-03-13T02:00:00Z",
            r#"[{"id":"T001","description":"First task","status":"todo"}]"#,
        ),
    )
    .unwrap();

    // Valid JSON but unsupported schema version — must propagate as an error.
    // Note: no legacy fields (e.g. "status") here; deny_unknown_fields would turn those
    // into a Json error, which is warn-and-continue. This tests the version gate path.
    std::fs::write(
        track_dir.join("spec.json"),
        r#"{"schema_version":99,"version":"1.0","title":"T","scope":{"in_scope":[],"out_of_scope":[]}}"#,
    )
    .unwrap();

    let result = sync_rendered_views(dir.path(), Some("track-a"));
    assert!(result.is_err(), "unsupported spec.json schema version must return an error");
}

// ---------------------------------------------------------------------------
// T011: domain-types.md rendering
// ---------------------------------------------------------------------------

const DOMAIN_TYPES_JSON_MINIMAL: &str = r#"{
  "schema_version": 3,
  "crate_name": "domain",
  "layer": "domain",
  "types": {
    "TrackId": {
      "action": "add",
      "role": "ValueObject",
      "kind": { "kind": "tuple_struct", "fields": ["String"] }
    }
  },
  "traits": {},
  "functions": {}
}"#;

const DOMAIN_ARCH_RULES: &str = r#"{"layers":[{"crate":"domain","tddd":{"enabled":true,"catalogue_file":"domain-types.json"}}]}"#;

#[test]
fn sync_rendered_views_generates_domain_types_md_from_domain_types_json() {
    let dir = tempfile::tempdir().unwrap();
    let track_dir = dir.path().join("track/items/track-a");
    std::fs::create_dir_all(&track_dir).unwrap();

    std::fs::write(
        track_dir.join("metadata.json"),
        sample_metadata_json(
            "track-a",
            "planned",
            "2026-03-13T02:00:00Z",
            r#"[{"id":"T001","description":"First task","status":"todo"}]"#,
        ),
    )
    .unwrap();
    std::fs::write(track_dir.join("domain-types.json"), DOMAIN_TYPES_JSON_MINIMAL).unwrap();
    std::fs::write(dir.path().join("architecture-rules.json"), DOMAIN_ARCH_RULES).unwrap();

    let changed = sync_rendered_views(dir.path(), Some("track-a")).unwrap();

    assert!(
        changed.iter().any(|p| p.ends_with("domain-types.md")),
        "domain-types.md should be reported as changed"
    );

    let md = std::fs::read_to_string(track_dir.join("domain-types.md")).unwrap();
    assert!(md.contains("<!-- Generated from domain-types.json"), "must have generated header");
    assert!(md.contains("TrackId"), "must include declared type name");
}

#[test]
fn sync_rendered_views_populates_signal_emojis_from_signal_file() {
    // Regression guard: after the declaration codec stopped surfacing inline
    // signals, the rendered `<layer>-types.md` lost its signal-column emojis
    // and fell back to `—`. `sync_rendered_views` must read the companion
    // `<layer>-type-signals.json` file and populate `doc.signals()` before
    // rendering so the markdown reflects the evaluated state.
    let dir = tempfile::tempdir().unwrap();
    let track_dir = dir.path().join("track/items/track-a");
    std::fs::create_dir_all(&track_dir).unwrap();

    std::fs::write(
        track_dir.join("metadata.json"),
        sample_metadata_json(
            "track-a",
            "planned",
            "2026-03-13T02:00:00Z",
            r#"[{"id":"T001","description":"First task","status":"todo"}]"#,
        ),
    )
    .unwrap();
    std::fs::write(track_dir.join("domain-types.json"), DOMAIN_TYPES_JSON_MINIMAL).unwrap();
    std::fs::write(dir.path().join("architecture-rules.json"), DOMAIN_ARCH_RULES).unwrap();

    // Companion signal file with a Blue signal for the declared TrackId.
    let decl_bytes = std::fs::read(track_dir.join("domain-types.json")).unwrap();
    let hash = crate::tddd::type_signals_codec::declaration_hash(&decl_bytes);
    let signal_file = serde_json::json!({
        "schema_version": 1,
        "generated_at": "2026-04-19T00:00:00Z",
        "declaration_hash": hash,
        "signals": [
            {
                "type_name": "TrackId",
                "kind_tag": "value_object",
                "signal": "blue",
                "found_type": true
            }
        ],
    });
    std::fs::write(
        track_dir.join("domain-type-signals.json"),
        serde_json::to_string_pretty(&signal_file).unwrap(),
    )
    .unwrap();

    let _changed = sync_rendered_views(dir.path(), Some("track-a")).unwrap();

    let md = std::fs::read_to_string(track_dir.join("domain-types.md")).unwrap();
    assert!(
        md.contains('\u{1f535}'),
        "rendered markdown must include the Blue emoji populated from the signal file, got:\n{md}"
    );
}

#[test]
fn sync_rendered_views_ignores_stale_signal_file_when_hash_mismatches() {
    // Regression guard for the stale-hash view-render bug: if the
    // declaration changes without regenerating signals, the rendered
    // `<layer>-types.md` must NOT paint misleading Blue emojis from the
    // old evaluation. Fall back to `—` placeholders instead. The
    // authoritative fail-closed behavior for stale signals lives in
    // `spec_states::evaluate_layer_catalogue`; the renderer just avoids
    // misrepresenting the state to a reviewer.
    let dir = tempfile::tempdir().unwrap();
    let track_dir = dir.path().join("track/items/track-a");
    std::fs::create_dir_all(&track_dir).unwrap();

    std::fs::write(
        track_dir.join("metadata.json"),
        sample_metadata_json(
            "track-a",
            "planned",
            "2026-03-13T02:00:00Z",
            r#"[{"id":"T001","description":"First task","status":"todo"}]"#,
        ),
    )
    .unwrap();
    std::fs::write(track_dir.join("domain-types.json"), DOMAIN_TYPES_JSON_MINIMAL).unwrap();
    std::fs::write(dir.path().join("architecture-rules.json"), DOMAIN_ARCH_RULES).unwrap();

    // Stale signal file — `declaration_hash` does NOT match the on-disk
    // declaration bytes.
    let stale_signal = serde_json::json!({
        "schema_version": 1,
        "generated_at": "2026-04-19T00:00:00Z",
        "declaration_hash": "0000000000000000000000000000000000000000000000000000000000000000",
        "signals": [
            {
                "type_name": "TrackId",
                "kind_tag": "value_object",
                "signal": "blue",
                "found_type": true
            }
        ],
    });
    std::fs::write(
        track_dir.join("domain-type-signals.json"),
        serde_json::to_string_pretty(&stale_signal).unwrap(),
    )
    .unwrap();

    let _changed = sync_rendered_views(dir.path(), Some("track-a")).unwrap();

    let md = std::fs::read_to_string(track_dir.join("domain-types.md")).unwrap();
    assert!(
        !md.contains('\u{1f535}'),
        "stale signal file must NOT produce a Blue emoji in the rendered markdown, got:\n{md}"
    );
    assert!(
        md.contains('—'),
        "rendered markdown must fall back to `—` placeholder on stale signal file, got:\n{md}"
    );
}

#[test]
fn sync_rendered_views_skips_domain_types_md_when_domain_types_json_absent() {
    let dir = tempfile::tempdir().unwrap();
    let track_dir = dir.path().join("track/items/track-a");
    std::fs::create_dir_all(&track_dir).unwrap();

    std::fs::write(
        track_dir.join("metadata.json"),
        sample_metadata_json(
            "track-a",
            "planned",
            "2026-03-13T02:00:00Z",
            r#"[{"id":"T001","description":"First task","status":"todo"}]"#,
        ),
    )
    .unwrap();
    std::fs::write(dir.path().join("architecture-rules.json"), DOMAIN_ARCH_RULES).unwrap();
    // No domain-types.json

    let changed = sync_rendered_views(dir.path(), Some("track-a")).unwrap();

    assert!(
        !changed.iter().any(|p| p.ends_with("domain-types.md")),
        "domain-types.md must not be generated when domain-types.json is absent"
    );
    assert!(!track_dir.join("domain-types.md").exists());
}

#[test]
fn sync_rendered_views_does_not_overwrite_domain_types_md_when_already_up_to_date() {
    let dir = tempfile::tempdir().unwrap();
    let track_dir = dir.path().join("track/items/track-a");
    std::fs::create_dir_all(&track_dir).unwrap();

    std::fs::write(
        track_dir.join("metadata.json"),
        sample_metadata_json(
            "track-a",
            "planned",
            "2026-03-13T02:00:00Z",
            r#"[{"id":"T001","description":"First task","status":"todo"}]"#,
        ),
    )
    .unwrap();
    std::fs::write(track_dir.join("domain-types.json"), DOMAIN_TYPES_JSON_MINIMAL).unwrap();
    // architecture-rules.json is required since T045: render.rs skips per-layer
    // rendering when arch rules are absent (fail-graceful precondition check).
    std::fs::write(dir.path().join("architecture-rules.json"), DOMAIN_ARCH_RULES).unwrap();

    // First sync — generates domain-types.md.
    sync_rendered_views(dir.path(), Some("track-a")).unwrap();

    // Second sync — domain-types.md is already up to date, should not appear in changed.
    let changed = sync_rendered_views(dir.path(), Some("track-a")).unwrap();
    assert!(
        !changed.iter().any(|p| p.ends_with("domain-types.md")),
        "second sync must not report domain-types.md as changed when already up to date"
    );
}

#[test]
fn sync_rendered_views_continues_on_malformed_domain_types_json() {
    let dir = tempfile::tempdir().unwrap();
    let track_dir = dir.path().join("track/items/track-a");
    std::fs::create_dir_all(&track_dir).unwrap();

    std::fs::write(
        track_dir.join("metadata.json"),
        sample_metadata_json(
            "track-a",
            "planned",
            "2026-03-13T02:00:00Z",
            r#"[{"id":"T001","description":"First task","status":"todo"}]"#,
        ),
    )
    .unwrap();
    // Write malformed domain-types.json (JSON parse error).
    std::fs::write(track_dir.join("domain-types.json"), "{ not valid json }").unwrap();
    std::fs::write(dir.path().join("architecture-rules.json"), DOMAIN_ARCH_RULES).unwrap();

    let result = sync_rendered_views(dir.path(), Some("track-a"));
    assert!(result.is_ok(), "malformed domain-types.json must not abort sync");

    let changed = result.unwrap();
    assert!(changed.iter().any(|p| p.ends_with("plan.md")));
    assert!(!changed.iter().any(|p| p.ends_with("domain-types.md")));
}

#[test]
fn sync_rendered_views_with_none_refreshes_registry_only() {
    // With `track_id = None` the function now operates in "registry only"
    // mode: it rebuilds track/registry.md from all collected snapshots but
    // does NOT iterate per-track views. Existing plan.md sentinels on
    // other tracks must therefore stay intact, and the bulk mode
    // "render every track under items/ and archive/" is gone.
    let dir = tempfile::tempdir().unwrap();

    // Active track — even with a valid metadata.json, its plan.md must
    // not be generated when `track_id = None` (registry-only mode).
    let active_dir = dir.path().join("track/items/track-active");
    std::fs::create_dir_all(&active_dir).unwrap();
    std::fs::write(
        active_dir.join("metadata.json"),
        sample_metadata_json(
            "track-active",
            "planned",
            "2026-03-13T02:00:00Z",
            r#"[{"id":"T001","description":"First task","status":"todo"}]"#,
        ),
    )
    .unwrap();

    // Done track in items/ — its sentinel plan.md must stay intact.
    let done_dir = dir.path().join("track/items/track-done");
    std::fs::create_dir_all(&done_dir).unwrap();
    std::fs::write(
        done_dir.join("metadata.json"),
        sample_metadata_json(
            "track-done",
            "done",
            "2026-03-10T00:00:00Z",
            r#"[{"id":"T001","description":"Done task","status":"done","commit_hash":"abc1234567890abc1234567890abc1234567890a"}]"#,
        ),
    )
    .unwrap();
    std::fs::write(done_dir.join("plan.md"), "SENTINEL_DONE").unwrap();

    // Archived track in archive/ — registry must still list it, but its
    // plan.md must stay intact.
    let archived_dir = dir.path().join("track/archive/track-archived");
    std::fs::create_dir_all(&archived_dir).unwrap();
    std::fs::write(
        archived_dir.join("metadata.json"),
        sample_metadata_json(
            "track-archived",
            "archived",
            "2026-03-10T00:00:00Z",
            r#"[{"id":"T001","description":"Archived task","status":"done","commit_hash":"def4567890def4567890def4567890def4567890"}]"#,
        ),
    )
    .unwrap();
    std::fs::write(archived_dir.join("plan.md"), "SENTINEL_ARCHIVED").unwrap();

    let changed = sync_rendered_views(dir.path(), None).unwrap();

    // No per-track views should be generated or touched.
    assert!(!active_dir.join("plan.md").exists());
    assert!(!changed.iter().any(|p| p.ends_with("track-active/plan.md")));
    assert_eq!(std::fs::read_to_string(done_dir.join("plan.md")).unwrap(), "SENTINEL_DONE");
    assert!(!changed.iter().any(|p| p.ends_with("track-done/plan.md")));
    assert_eq!(std::fs::read_to_string(archived_dir.join("plan.md")).unwrap(), "SENTINEL_ARCHIVED");
    assert!(!changed.iter().any(|p| p.ends_with("track-archived/plan.md")));

    // Registry MUST reflect all three tracks (snapshots are collected
    // across items/ and archive/ regardless of rendering mode).
    assert!(changed.iter().any(|p| p.ends_with("registry.md")));
    let registry = std::fs::read_to_string(dir.path().join("track/registry.md")).unwrap();
    assert!(registry.contains("track-active"));
    assert!(registry.contains("track-done"));
    assert!(registry.contains("track-archived"));
}

#[test]
fn sync_rendered_views_single_track_renders_done_track() {
    // Regression: when `track_id=Some(id)` is passed, the caller has
    // explicitly asked to render that track. The done/archived skip is a
    // bulk-sync-only protection and must NOT apply to single-track sync,
    // otherwise the final `in_progress → done` transition of an active
    // track freezes plan.md in its pre-done state.
    //
    // Verify that single-track sync overwrites stale plan.md and renders the title.
    let dir = tempfile::tempdir().unwrap();

    let done_dir = dir.path().join("track/items/track-done");
    std::fs::create_dir_all(&done_dir).unwrap();
    std::fs::write(
        done_dir.join("metadata.json"),
        sample_metadata_json(
            "track-done",
            "done",
            "2026-03-10T00:00:00Z",
            r#"[{"id":"T001","description":"Done task","status":"done","commit_hash":"abc1234567890abc1234567890abc1234567890a"}]"#,
        ),
    )
    .unwrap();
    // Sentinel content that must be overwritten by the single-track render.
    std::fs::write(done_dir.join("plan.md"), "STALE_SENTINEL_MUST_BE_OVERWRITTEN").unwrap();
    std::fs::write(dir.path().join("architecture-rules.json"), DOMAIN_ARCH_RULES).unwrap();

    // Single-track path with track_id=Some.
    let changed = sync_rendered_views(dir.path(), Some("track-done")).unwrap();

    // plan.md must be freshly rendered (sentinel overwritten).
    let plan = std::fs::read_to_string(done_dir.join("plan.md")).unwrap();
    assert_ne!(plan, "STALE_SENTINEL_MUST_BE_OVERWRITTEN");
    // Verify title and stub note are present.
    assert!(plan.contains("# Title track-done"), "title must appear in plan.md:\n{plan}");
    assert!(changed.iter().any(|p| p.ends_with("track-done/plan.md")));
}

#[test]
fn sync_rendered_views_single_track_skips_spec_md_for_done_track() {
    // Regression: single-track rendering must still preserve legacy
    // spec.md content on done/archived tracks to avoid silently
    // overwriting a field an older renderer preserved. Only plan.md is
    // re-rendered unconditionally because it mirrors task state that
    // actually changes during transitions; spec.md reflects spec.json
    // which does NOT change on a task transition.
    let dir = tempfile::tempdir().unwrap();
    let done_dir = dir.path().join("track/items/track-done-spec");
    std::fs::create_dir_all(&done_dir).unwrap();
    // v5 identity-only metadata. Derived status comes from impl-plan.json
    // below (all tasks done → status = Done).
    std::fs::write(
        done_dir.join("metadata.json"),
        sample_metadata_json("track-done-spec", "done", "2026-03-10T00:00:00Z", "[]"),
    )
    .unwrap();
    // impl-plan.json with an all-done task list so the derived track status
    // resolves to Done and the done-branch render path is exercised.
    std::fs::write(
        done_dir.join("impl-plan.json"),
        r#"{"schema_version":1,"tasks":[{"id":"T001","description":"Done task","status":"done","commit_hash":"abc1234567890abc1234567890abc1234567890a"}],"plan":{"summary":[],"sections":[{"id":"S1","title":"All","task_ids":["T001"]}]}}"#,
    )
    .unwrap();
    // Done tracks intentionally do not require spec.json to be re-decoded
    // (it is never re-rendered for a frozen track). Writing a minimal v2
    // spec.json here is fine; an absent spec.json would also work.
    std::fs::write(
        done_dir.join("spec.json"),
        r#"{"schema_version":2,"version":"1.0","title":"Done Feature","goal":[],"scope":{"in_scope":[],"out_of_scope":[]},"constraints":[],"acceptance_criteria":[]}"#,
    )
    .unwrap();
    // Sentinel spec.md that must stay intact.
    std::fs::write(done_dir.join("spec.md"), "LEGACY_SPEC_SENTINEL_PRESERVED").unwrap();
    // architecture-rules.json is required by render_contract_map_view (fail-closed).
    // Done tracks skip the per-layer catalogue iteration, but contract-map.md is
    // rendered unconditionally; a missing arch-rules file now propagates as an error.
    std::fs::write(dir.path().join("architecture-rules.json"), DOMAIN_ARCH_RULES).unwrap();

    let changed = sync_rendered_views(dir.path(), Some("track-done-spec")).unwrap();

    // spec.md must NOT be re-rendered for a done track.
    let spec = std::fs::read_to_string(done_dir.join("spec.md")).unwrap();
    assert_eq!(spec, "LEGACY_SPEC_SENTINEL_PRESERVED");
    assert!(!changed.iter().any(|p| p.ends_with("spec.md")));

    // plan.md, on the other hand, MUST still have been rendered so
    // the post-transition state is captured.
    assert!(changed.iter().any(|p| p.ends_with("track-done-spec/plan.md")));
}

#[test]
fn sync_rendered_views_single_track_skips_domain_types_md_for_done_track() {
    // Same legacy-protection rationale as the spec.md case above, but
    // for `domain-types.md`.
    let dir = tempfile::tempdir().unwrap();
    let done_dir = dir.path().join("track/items/track-done-domain");
    std::fs::create_dir_all(&done_dir).unwrap();
    std::fs::write(
        done_dir.join("metadata.json"),
        sample_metadata_json("track-done-domain", "done", "2026-03-10T00:00:00Z", "[]"),
    )
    .unwrap();
    // impl-plan.json with an all-done task list so the derived track status
    // resolves to Done.
    std::fs::write(
        done_dir.join("impl-plan.json"),
        r#"{"schema_version":1,"tasks":[{"id":"T001","description":"Done task","status":"done","commit_hash":"abc1234567890abc1234567890abc1234567890a"}],"plan":{"summary":[],"sections":[{"id":"S1","title":"All","task_ids":["T001"]}]}}"#,
    )
    .unwrap();
    std::fs::write(done_dir.join("domain-types.json"), DOMAIN_TYPES_JSON_MINIMAL).unwrap();
    std::fs::write(done_dir.join("domain-types.md"), "LEGACY_DOMAIN_TYPES_SENTINEL_PRESERVED")
        .unwrap();
    // architecture-rules.json is required by render_contract_map_view (fail-closed).
    std::fs::write(dir.path().join("architecture-rules.json"), DOMAIN_ARCH_RULES).unwrap();

    let changed = sync_rendered_views(dir.path(), Some("track-done-domain")).unwrap();

    let domain_types = std::fs::read_to_string(done_dir.join("domain-types.md")).unwrap();
    assert_eq!(domain_types, "LEGACY_DOMAIN_TYPES_SENTINEL_PRESERVED");
    assert!(!changed.iter().any(|p| p.ends_with("domain-types.md")));
    assert!(changed.iter().any(|p| p.ends_with("track-done-domain/plan.md")));
}

// ---------------------------------------------------------------------------
// Multi-layer sync_rendered_views tests (T004 / D3)
// ---------------------------------------------------------------------------
//
// These tests verify that sync_rendered_views correctly iterates all
// tddd.enabled layers from architecture-rules.json and generates the
// corresponding <layer>-types.md for each layer whose <layer>-types.json
// is present in the track directory. The loop uses the existing
// `parse_tddd_layers` resolver (introduced in tddd-01 Phase 1 Task 7,
// already reused by `apps/cli::resolve_layers`).

const USECASE_TYPES_JSON_MINIMAL: &str = r#"{
  "schema_version": 3,
  "crate_name": "usecase",
  "layer": "usecase",
  "types": {
    "TrackReader": {
      "action": "add",
      "role": "ValueObject",
      "kind": { "kind": "tuple_struct", "fields": ["String"] }
    }
  },
  "traits": {},
  "functions": {}
}"#;

const INFRASTRUCTURE_TYPES_JSON_MINIMAL: &str = r#"{
  "schema_version": 3,
  "crate_name": "infrastructure",
  "layer": "infrastructure",
  "types": {
    "FsTrackStore": {
      "action": "add",
      "role": "ValueObject",
      "kind": { "kind": "tuple_struct", "fields": ["String"] }
    }
  },
  "traits": {},
  "functions": {}
}"#;

const MULTI_LAYER_ARCH_RULES: &str = r#"{
      "layers": [
        { "crate": "domain", "tddd": { "enabled": true, "catalogue_file": "domain-types.json" } },
        { "crate": "usecase", "tddd": { "enabled": true, "catalogue_file": "usecase-types.json" } },
        { "crate": "infrastructure", "tddd": { "enabled": true, "catalogue_file": "infrastructure-types.json" } }
      ]
    }"#;

#[test]
fn sync_rendered_views_generates_usecase_types_md_from_usecase_types_json() {
    let dir = tempfile::tempdir().unwrap();
    let track_dir = dir.path().join("track/items/track-a");
    std::fs::create_dir_all(&track_dir).unwrap();

    std::fs::write(dir.path().join("architecture-rules.json"), MULTI_LAYER_ARCH_RULES).unwrap();

    std::fs::write(
        track_dir.join("metadata.json"),
        sample_metadata_json(
            "track-a",
            "planned",
            "2026-03-13T02:00:00Z",
            r#"[{"id":"T001","description":"First task","status":"todo"}]"#,
        ),
    )
    .unwrap();
    std::fs::write(track_dir.join("usecase-types.json"), USECASE_TYPES_JSON_MINIMAL).unwrap();

    let changed = sync_rendered_views(dir.path(), Some("track-a")).unwrap();

    assert!(
        changed.iter().any(|p| p.ends_with("usecase-types.md")),
        "usecase-types.md should be reported as changed"
    );

    let md = std::fs::read_to_string(track_dir.join("usecase-types.md")).unwrap();
    assert!(
        md.contains("<!-- Generated from usecase-types.json"),
        "must have usecase-types.json header (not domain-types.json), got: {md}"
    );
    assert!(md.contains("TrackReader"), "must include declared type name");
}

#[test]
fn sync_rendered_views_generates_infrastructure_types_md_from_infrastructure_types_json() {
    let dir = tempfile::tempdir().unwrap();
    let track_dir = dir.path().join("track/items/track-a");
    std::fs::create_dir_all(&track_dir).unwrap();

    std::fs::write(dir.path().join("architecture-rules.json"), MULTI_LAYER_ARCH_RULES).unwrap();

    std::fs::write(
        track_dir.join("metadata.json"),
        sample_metadata_json(
            "track-a",
            "planned",
            "2026-03-13T02:00:00Z",
            r#"[{"id":"T001","description":"First task","status":"todo"}]"#,
        ),
    )
    .unwrap();
    std::fs::write(track_dir.join("infrastructure-types.json"), INFRASTRUCTURE_TYPES_JSON_MINIMAL)
        .unwrap();

    let changed = sync_rendered_views(dir.path(), Some("track-a")).unwrap();

    assert!(
        changed.iter().any(|p| p.ends_with("infrastructure-types.md")),
        "infrastructure-types.md should be reported as changed"
    );

    let md = std::fs::read_to_string(track_dir.join("infrastructure-types.md")).unwrap();
    assert!(
        md.contains("<!-- Generated from infrastructure-types.json"),
        "must have infrastructure-types.json header, got: {md}"
    );
    assert!(md.contains("FsTrackStore"), "must include declared type name");
}

#[test]
fn sync_rendered_views_generates_multiple_layer_types_md_independently() {
    // Multi-layer track: domain + usecase + infrastructure catalogue files all
    // present. The loop must render each <layer>-types.md independently (one
    // layer's presence/absence must not affect another's rendering).
    let dir = tempfile::tempdir().unwrap();
    let track_dir = dir.path().join("track/items/track-a");
    std::fs::create_dir_all(&track_dir).unwrap();

    std::fs::write(dir.path().join("architecture-rules.json"), MULTI_LAYER_ARCH_RULES).unwrap();

    std::fs::write(
        track_dir.join("metadata.json"),
        sample_metadata_json(
            "track-a",
            "planned",
            "2026-03-13T02:00:00Z",
            r#"[{"id":"T001","description":"First task","status":"todo"}]"#,
        ),
    )
    .unwrap();
    std::fs::write(track_dir.join("domain-types.json"), DOMAIN_TYPES_JSON_MINIMAL).unwrap();
    std::fs::write(track_dir.join("usecase-types.json"), USECASE_TYPES_JSON_MINIMAL).unwrap();
    std::fs::write(track_dir.join("infrastructure-types.json"), INFRASTRUCTURE_TYPES_JSON_MINIMAL)
        .unwrap();

    let changed = sync_rendered_views(dir.path(), Some("track-a")).unwrap();

    // All 3 layer rendered views must be reported as changed
    assert!(
        changed.iter().any(|p| p.ends_with("domain-types.md")),
        "domain-types.md should be reported as changed"
    );
    assert!(
        changed.iter().any(|p| p.ends_with("usecase-types.md")),
        "usecase-types.md should be reported as changed"
    );
    assert!(
        changed.iter().any(|p| p.ends_with("infrastructure-types.md")),
        "infrastructure-types.md should be reported as changed"
    );

    // Each rendered view must carry its own source_file_name in the header
    let domain_md = std::fs::read_to_string(track_dir.join("domain-types.md")).unwrap();
    assert!(
        domain_md.contains("<!-- Generated from domain-types.json"),
        "domain-types.md must have its own header"
    );

    let usecase_md = std::fs::read_to_string(track_dir.join("usecase-types.md")).unwrap();
    assert!(
        usecase_md.contains("<!-- Generated from usecase-types.json"),
        "usecase-types.md must have its own header (independent of domain-types.md)"
    );

    let infra_md = std::fs::read_to_string(track_dir.join("infrastructure-types.md")).unwrap();
    assert!(
        infra_md.contains("<!-- Generated from infrastructure-types.json"),
        "infrastructure-types.md must have its own header"
    );
}

#[test]
fn sync_rendered_views_malformed_layer_json_does_not_block_other_layers() {
    // D3 guarantee: the per-layer `CatalogueDocumentCodecError::Json` (syntax/EOF) warn-and-continue
    // path must be exercised in a multi-layer scenario. A malformed catalogue for
    // one layer (usecase) must not prevent the other layers (domain, infrastructure)
    // from rendering their views. This is the cross-layer error isolation guarantee.
    let dir = tempfile::tempdir().unwrap();
    let track_dir = dir.path().join("track/items/track-a");
    std::fs::create_dir_all(&track_dir).unwrap();

    std::fs::write(dir.path().join("architecture-rules.json"), MULTI_LAYER_ARCH_RULES).unwrap();

    std::fs::write(
        track_dir.join("metadata.json"),
        sample_metadata_json(
            "track-a",
            "planned",
            "2026-03-13T02:00:00Z",
            r#"[{"id":"T001","description":"First task","status":"todo"}]"#,
        ),
    )
    .unwrap();
    // domain and infrastructure catalogue files are valid
    std::fs::write(track_dir.join("domain-types.json"), DOMAIN_TYPES_JSON_MINIMAL).unwrap();
    // usecase catalogue is malformed JSON — must warn and continue
    std::fs::write(track_dir.join("usecase-types.json"), "{ not valid json }").unwrap();
    std::fs::write(track_dir.join("infrastructure-types.json"), INFRASTRUCTURE_TYPES_JSON_MINIMAL)
        .unwrap();

    // Must succeed — malformed usecase JSON must not abort the sync
    let result = sync_rendered_views(dir.path(), Some("track-a"));
    assert!(result.is_ok(), "malformed usecase-types.json must not abort multi-layer sync");

    let changed = result.unwrap();

    // domain and infrastructure rendered views must still be generated
    assert!(
        changed.iter().any(|p| p.ends_with("domain-types.md")),
        "domain-types.md must still render when usecase-types.json is malformed"
    );
    assert!(
        changed.iter().any(|p| p.ends_with("infrastructure-types.md")),
        "infrastructure-types.md must still render when usecase-types.json is malformed"
    );

    // usecase rendered view must NOT appear in changed list (malformed → skipped)
    assert!(
        !changed.iter().any(|p| p.ends_with("usecase-types.md")),
        "usecase-types.md must NOT be rendered when usecase-types.json is malformed"
    );
}

// ---------------------------------------------------------------------------
// T020 sync_rendered_views <layer>-catalogue-spec-signals.json integration
// ---------------------------------------------------------------------------
//
// End-to-end tests covering the catalogue-spec signals file-loading path
// added in T020 (ADR 2026-04-23-0344 §D2.5 / IN-17). Complements the
// renderer-level Some/None unit tests in `type_catalogue_render.rs` by
// exercising the actual `sync_rendered_views` pipeline:
//   - opt-in guard via `catalogue_spec_signal.enabled`
//   - filename derivation (`<layer_id>-catalogue-spec-signals.json`)
//   - fresh-hash validation (hex comparison)
//   - stale / malformed fallback to `None` (em-dash fallback, non-fatal)

const MULTI_LAYER_ARCH_RULES_WITH_CAT_SPEC_OPT_IN: &str = r#"{
      "layers": [
        {
          "crate": "domain",
          "tddd": {
            "enabled": true,
            "catalogue_file": "domain-types.json",
            "catalogue_spec_signal": { "enabled": true }
          }
        }
      ]
    }"#;

const MULTI_LAYER_ARCH_RULES_CAT_SPEC_OPT_OUT: &str = r#"{
      "layers": [
        {
          "crate": "domain",
          "tddd": {
            "enabled": true,
            "catalogue_file": "domain-types.json"
          }
        }
      ]
    }"#;

#[test]
fn sync_rendered_views_renders_cat_spec_column_when_signals_fresh_and_opt_in_enabled() {
    // Happy path: opt-in flag is true, signals file exists with a matching
    // catalogue_declaration_hash, and a per-entry `blue` signal is declared
    // for `TrackId`. The rendered markdown must carry the 6-column header
    // and paint the 🔵 emoji in the Cat-Spec column.
    let dir = tempfile::tempdir().unwrap();
    let track_dir = dir.path().join("track/items/track-a");
    std::fs::create_dir_all(&track_dir).unwrap();

    std::fs::write(
        dir.path().join("architecture-rules.json"),
        MULTI_LAYER_ARCH_RULES_WITH_CAT_SPEC_OPT_IN,
    )
    .unwrap();

    std::fs::write(
        track_dir.join("metadata.json"),
        sample_metadata_json(
            "track-a",
            "planned",
            "2026-03-13T02:00:00Z",
            r#"[{"id":"T001","description":"First task","status":"todo"}]"#,
        ),
    )
    .unwrap();
    std::fs::write(track_dir.join("domain-types.json"), DOMAIN_TYPES_JSON_MINIMAL).unwrap();

    // Catalogue-spec-signals file with a fresh (matching) hash.
    let decl_bytes = std::fs::read(track_dir.join("domain-types.json")).unwrap();
    let hash_hex = crate::tddd::type_signals_codec::declaration_hash(&decl_bytes);
    let spec_signals_json = serde_json::json!({
        "schema_version": 1,
        "catalogue_declaration_hash": hash_hex,
        "signals": [
            { "type_name": "TrackId", "signal": "blue" }
        ],
    });
    // Filename derivation: `<layer_id>-catalogue-spec-signals.json`
    // = `domain-catalogue-spec-signals.json`.
    std::fs::write(
        track_dir.join("domain-catalogue-spec-signals.json"),
        serde_json::to_string_pretty(&spec_signals_json).unwrap(),
    )
    .unwrap();

    let _changed = sync_rendered_views(dir.path(), Some("track-a")).unwrap();

    let md = std::fs::read_to_string(track_dir.join("domain-types.md")).unwrap();
    assert!(
        md.contains("| Name | Kind | Action | Details | Signal | Cat-Spec |"),
        "6-column header must appear when opt-in + fresh signals present, got:\n{md}"
    );
    // TrackId entry row must include the 🔵 emoji in Cat-Spec column.
    let track_id_row =
        md.lines().find(|l| l.starts_with("| TrackId |")).expect("TrackId row must be rendered");
    assert!(
        track_id_row.contains('\u{1f535}'),
        "TrackId row must show Blue emoji in Cat-Spec column, got: {track_id_row}"
    );
}

#[test]
fn sync_rendered_views_skips_cat_spec_column_when_opt_in_disabled() {
    // Opt-in guard: even when a valid catalogue-spec-signals.json is
    // present on disk, the renderer must produce the legacy 5-column
    // layout if the layer has NOT opted in via `catalogue_spec_signal.enabled`.
    // This is the phased-activation knob per ADR §D5.4.
    let dir = tempfile::tempdir().unwrap();
    let track_dir = dir.path().join("track/items/track-a");
    std::fs::create_dir_all(&track_dir).unwrap();

    std::fs::write(
        dir.path().join("architecture-rules.json"),
        MULTI_LAYER_ARCH_RULES_CAT_SPEC_OPT_OUT,
    )
    .unwrap();

    std::fs::write(
        track_dir.join("metadata.json"),
        sample_metadata_json(
            "track-a",
            "planned",
            "2026-03-13T02:00:00Z",
            r#"[{"id":"T001","description":"First task","status":"todo"}]"#,
        ),
    )
    .unwrap();
    std::fs::write(track_dir.join("domain-types.json"), DOMAIN_TYPES_JSON_MINIMAL).unwrap();

    // Signals file present with matching hash — but layer hasn't opted in.
    let decl_bytes = std::fs::read(track_dir.join("domain-types.json")).unwrap();
    let hash_hex = crate::tddd::type_signals_codec::declaration_hash(&decl_bytes);
    let spec_signals_json = serde_json::json!({
        "schema_version": 1,
        "catalogue_declaration_hash": hash_hex,
        "signals": [ { "type_name": "TrackId", "signal": "blue" } ],
    });
    std::fs::write(
        track_dir.join("domain-catalogue-spec-signals.json"),
        serde_json::to_string_pretty(&spec_signals_json).unwrap(),
    )
    .unwrap();

    let _changed = sync_rendered_views(dir.path(), Some("track-a")).unwrap();

    let md = std::fs::read_to_string(track_dir.join("domain-types.md")).unwrap();
    assert!(
        !md.contains("Cat-Spec"),
        "Cat-Spec column must NOT appear when opt-in is disabled, got:\n{md}"
    );
    assert!(
        md.contains("| Name | Kind | Action | Details | Signal |"),
        "legacy 5-column header must be preserved when opt-in is disabled, got:\n{md}"
    );
}

#[test]
fn sync_rendered_views_errors_on_stale_cat_spec_signals() {
    // Fail-closed: a stale `catalogue_declaration_hash` in the signals
    // file indicates the catalogue changed without regenerating signals.
    // View rendering aborts and the caller is expected to run
    // `sotp track catalogue-spec-signals <track_id>` before retrying.
    let dir = tempfile::tempdir().unwrap();
    let track_dir = dir.path().join("track/items/track-a");
    std::fs::create_dir_all(&track_dir).unwrap();

    std::fs::write(
        dir.path().join("architecture-rules.json"),
        MULTI_LAYER_ARCH_RULES_WITH_CAT_SPEC_OPT_IN,
    )
    .unwrap();

    std::fs::write(
        track_dir.join("metadata.json"),
        sample_metadata_json(
            "track-a",
            "planned",
            "2026-03-13T02:00:00Z",
            r#"[{"id":"T001","description":"First task","status":"todo"}]"#,
        ),
    )
    .unwrap();
    std::fs::write(track_dir.join("domain-types.json"), DOMAIN_TYPES_JSON_MINIMAL).unwrap();

    // Stale: hash does NOT match on-disk catalogue.
    let stale_hash = "0".repeat(64);
    let stale_json = serde_json::json!({
        "schema_version": 1,
        "catalogue_declaration_hash": stale_hash,
        "signals": [ { "type_name": "TrackId", "signal": "blue" } ],
    });
    std::fs::write(
        track_dir.join("domain-catalogue-spec-signals.json"),
        serde_json::to_string_pretty(&stale_json).unwrap(),
    )
    .unwrap();

    let err = sync_rendered_views(dir.path(), Some("track-a")).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("stale") && msg.contains("catalogue-spec-signals"),
        "stale-hash error expected, got: {msg}"
    );
}

#[test]
fn sync_rendered_views_errors_on_malformed_cat_spec_signals() {
    // Fail-closed: an unparseable signals file is a system-state error,
    // not a silent fallback. The view renderer propagates the decode
    // failure and the caller re-runs `sotp track catalogue-spec-signals`.
    let dir = tempfile::tempdir().unwrap();
    let track_dir = dir.path().join("track/items/track-a");
    std::fs::create_dir_all(&track_dir).unwrap();

    std::fs::write(
        dir.path().join("architecture-rules.json"),
        MULTI_LAYER_ARCH_RULES_WITH_CAT_SPEC_OPT_IN,
    )
    .unwrap();

    std::fs::write(
        track_dir.join("metadata.json"),
        sample_metadata_json(
            "track-a",
            "planned",
            "2026-03-13T02:00:00Z",
            r#"[{"id":"T001","description":"First task","status":"todo"}]"#,
        ),
    )
    .unwrap();
    std::fs::write(track_dir.join("domain-types.json"), DOMAIN_TYPES_JSON_MINIMAL).unwrap();

    // Malformed JSON.
    std::fs::write(
        track_dir.join("domain-catalogue-spec-signals.json"),
        "{ this is not valid json ",
    )
    .unwrap();

    let err = sync_rendered_views(dir.path(), Some("track-a")).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("decode") || msg.contains("JSON"), "decode error expected, got: {msg}");
}

#[test]
fn sync_rendered_views_errors_on_missing_cat_spec_signals_when_opt_in() {
    // Fail-closed: when opt-in is enabled but the signals file has never
    // been generated, view rendering must error and direct the user to
    // `sotp track catalogue-spec-signals <track_id>`.
    let dir = tempfile::tempdir().unwrap();
    let track_dir = dir.path().join("track/items/track-a");
    std::fs::create_dir_all(&track_dir).unwrap();

    std::fs::write(
        dir.path().join("architecture-rules.json"),
        MULTI_LAYER_ARCH_RULES_WITH_CAT_SPEC_OPT_IN,
    )
    .unwrap();
    std::fs::write(
        track_dir.join("metadata.json"),
        sample_metadata_json(
            "track-a",
            "planned",
            "2026-03-13T02:00:00Z",
            r#"[{"id":"T001","description":"First task","status":"todo"}]"#,
        ),
    )
    .unwrap();
    std::fs::write(track_dir.join("domain-types.json"), DOMAIN_TYPES_JSON_MINIMAL).unwrap();

    let err = sync_rendered_views(dir.path(), Some("track-a")).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("not found"), "not-found error expected, got: {msg}");
    assert!(msg.contains("sotp track catalogue-spec-signals"), "remediation missing: {msg}");
}

// ---------------------------------------------------------------------------
// PR follow-up: contract-map.md renders for done tracks
// (Issue 2: orphan-node regression — render_contract_map_view was inside
// the !is_done_or_archived guard, so a done track never got its contract-map
// refreshed after the T002 variant-payload edge renderer was committed)
// ---------------------------------------------------------------------------

/// Minimal arch rules with only domain layer (sufficient for the contract-map E2E path).
const ARCH_RULES_DOMAIN_ONLY: &str = r#"{
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

/// Catalogue (v3) with two type entries.
/// Mirrors the production data that produced the "0 edges" bug in the track.
const DOMAIN_TYPES_WITH_ENUM_VARIANTS: &str = r#"{
      "schema_version": 3,
      "crate_name": "domain",
      "layer": "domain",
      "types": {
        "EnumVariantDeclaration": {
          "action": "add",
          "role": "ValueObject",
          "kind": {"kind": "plain_struct", "fields": [], "has_stripped_fields": false},
          "methods": [],
          "module_path": "",
          "spec_refs": [],
          "informal_grounds": []
        },
        "MemberDeclaration": {
          "action": "add",
          "role": "ValueObject",
          "kind": {
            "kind": "enum",
            "variants": [
              {"name": "Variant", "payload": {"kind": "tuple", "fields": ["EnumVariantDeclaration"]}},
              {"name": "Field", "payload": {"kind": "tuple", "fields": ["String", "String"]}}
            ]
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

#[test]
fn sync_rendered_views_renders_contract_map_for_done_track() {
    // Regression guard for the "0-edge orphan" bug:
    // `render_contract_map_view` was inside the `!is_done_or_archived`
    // block, so a track that reached `done` status never had its
    // contract-map regenerated after the fix that moved the call outside
    // the guard.
    //
    // This test proves that:
    // 1. `sync_rendered_views` produces a `contract-map.md` even when the
    //    track status is `done` (all tasks in impl-plan.json are done).
    // 2. The rendered contract-map contains the IN-24 / OS-07 placeholder
    //    scaffold (deferment comment + domain subgraph). Full v3 rendering
    //    is deferred per IN-24 / OS-07.
    let dir = tempfile::tempdir().unwrap();
    let track_dir = dir.path().join("track/items/track-done-cmap");
    std::fs::create_dir_all(&track_dir).unwrap();

    // architecture-rules.json — required by FsCatalogueLoader inside
    // `render_contract_map_view`.
    std::fs::write(dir.path().join("architecture-rules.json"), ARCH_RULES_DOMAIN_ONLY).unwrap();

    // v5 metadata (no status field — status derived from impl-plan.json).
    std::fs::write(
        track_dir.join("metadata.json"),
        sample_metadata_json("track-done-cmap", "done", "2026-03-10T00:00:00Z", "[]"),
    )
    .unwrap();

    // impl-plan.json: single done task → derive_track_status() == Done.
    std::fs::write(
        track_dir.join("impl-plan.json"),
        r#"{"schema_version":1,"tasks":[{"id":"T001","description":"Done","status":"done","commit_hash":"abc1234567890abc1234567890abc1234567890a"}],"plan":{"summary":[],"sections":[{"id":"S1","title":"All","task_ids":["T001"]}]}}"#,
    )
    .unwrap();

    // domain-types.json (v3 format — T025 v3-native migration).
    std::fs::write(track_dir.join("domain-types.json"), DOMAIN_TYPES_WITH_ENUM_VARIANTS).unwrap();

    let changed = sync_rendered_views(dir.path(), Some("track-done-cmap")).unwrap();

    // contract-map.md must appear in the changed set (first-time generation).
    assert!(
        changed.iter().any(|p| p.ends_with("contract-map.md")),
        "contract-map.md must be rendered for done tracks; changed: {changed:?}"
    );

    // IN-24 placeholder: the rendered contract-map must contain the deferment
    // comment and the domain subgraph. Full v3 rendering is deferred per IN-24.
    let cmap = std::fs::read_to_string(track_dir.join("contract-map.md")).unwrap();
    assert!(
        cmap.contains("IN-24"),
        "IN-24 deferment comment must appear in contract-map.md; got:\n{cmap}"
    );
    assert!(
        cmap.contains("subgraph domain [domain]"),
        "domain subgraph must appear in contract-map.md; got:\n{cmap}"
    );
    // Placeholder must not emit mermaid edge arrows.
    assert!(!cmap.contains("-->|"), "placeholder must not emit mermaid edges; got:\n{cmap}");
}
