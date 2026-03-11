//! Schema compatibility tests: verify Rust TrackDocumentV2 and Python track_schema.py
//! produce identical metadata.json structure.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use infrastructure::track::codec;

/// Tests round-trip decoding of an actual track metadata.json from the repository.
#[test]
fn real_metadata_json_round_trips_through_rust_codec() {
    // Use a known metadata.json from the repo.
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../track/items/filelock-migration-2026-03-11/metadata.json");

    if !path.exists() {
        // Skip if the track doesn't exist (e.g., in CI without track artifacts).
        eprintln!("Skipping: {path:?} not found");
        return;
    }

    let json = std::fs::read_to_string(&path).unwrap();
    let (track, meta) = codec::decode(&json).unwrap();
    let re_encoded = codec::encode(&track, &meta).unwrap();
    let (track2, meta2) = codec::decode(&re_encoded).unwrap();

    assert_eq!(track, track2, "round-trip must preserve TrackMetadata");
    assert_eq!(meta.schema_version, meta2.schema_version);
}

/// Tests that the Rust codec preserves all Python-expected JSON keys.
#[test]
fn rust_codec_preserves_python_expected_keys() {
    let json = full_featured_json();
    let (track, meta) = codec::decode(json).unwrap();
    let re_encoded = codec::encode(&track, &meta).unwrap();

    // Parse as generic JSON and verify structure.
    let doc: serde_json::Value = serde_json::from_str(&re_encoded).unwrap();

    // Top-level keys expected by Python track_schema.py.
    assert!(doc.get("schema_version").is_some());
    assert!(doc.get("id").is_some());
    assert!(doc.get("title").is_some());
    assert!(doc.get("status").is_some());
    assert!(doc.get("created_at").is_some());
    assert!(doc.get("updated_at").is_some());
    assert!(doc.get("tasks").is_some());
    assert!(doc.get("plan").is_some());

    // Task keys.
    let tasks = doc["tasks"].as_array().unwrap();
    assert!(!tasks.is_empty());
    let task = &tasks[0];
    assert!(task.get("id").is_some());
    assert!(task.get("description").is_some());
    assert!(task.get("status").is_some());

    // Done task should have commit_hash.
    let done_task = tasks.iter().find(|t| t["status"] == "done").unwrap();
    assert!(done_task.get("commit_hash").is_some());

    // Plan keys.
    let plan = &doc["plan"];
    assert!(plan.get("summary").is_some());
    assert!(plan.get("sections").is_some());
    let section = &plan["sections"][0];
    assert!(section.get("id").is_some());
    assert!(section.get("title").is_some());
    assert!(section.get("description").is_some());
    assert!(section.get("task_ids").is_some());

    // Status override.
    assert!(doc.get("status_override").is_some());
    let override_ = doc["status_override"].as_object().unwrap();
    assert!(override_.get("status").is_some());
    assert!(override_.get("reason").is_some());
}

/// Tests that null commit_hash is omitted (skip_serializing_if) matching Python behavior.
#[test]
fn null_commit_hash_is_omitted_in_json() {
    let json = r#"{
  "schema_version": 2,
  "id": "test-track",
  "title": "Test",
  "status": "planned",
  "created_at": "2026-03-11T00:00:00Z",
  "updated_at": "2026-03-11T00:00:00Z",
  "tasks": [{"id": "T1", "description": "Task", "status": "todo"}],
  "plan": {"summary": [], "sections": [{"id": "S1", "title": "S", "description": [], "task_ids": ["T1"]}]}
}"#;

    let (track, meta) = codec::decode(json).unwrap();
    let re_encoded = codec::encode(&track, &meta).unwrap();
    let doc: serde_json::Value = serde_json::from_str(&re_encoded).unwrap();

    // todo task should NOT have commit_hash key (skip_serializing_if = "Option::is_none").
    let task = &doc["tasks"][0];
    assert!(task.get("commit_hash").is_none(), "todo task should omit commit_hash");
}

/// Tests that null status_override is omitted matching Python behavior.
#[test]
fn null_status_override_is_omitted_in_json() {
    let json = r#"{
  "schema_version": 2,
  "id": "test-track",
  "title": "Test",
  "status": "planned",
  "created_at": "2026-03-11T00:00:00Z",
  "updated_at": "2026-03-11T00:00:00Z",
  "tasks": [{"id": "T1", "description": "Task", "status": "todo"}],
  "plan": {"summary": [], "sections": [{"id": "S1", "title": "S", "description": [], "task_ids": ["T1"]}]}
}"#;

    let (track, meta) = codec::decode(json).unwrap();
    let re_encoded = codec::encode(&track, &meta).unwrap();
    let doc: serde_json::Value = serde_json::from_str(&re_encoded).unwrap();

    assert!(doc.get("status_override").is_none(), "no override should omit key");
}

fn full_featured_json() -> &'static str {
    r#"{
  "schema_version": 2,
  "id": "full-featured-track",
  "title": "Full Featured Track",
  "status": "blocked",
  "created_at": "2026-03-11T00:00:00Z",
  "updated_at": "2026-03-11T12:00:00Z",
  "tasks": [
    {"id": "T1", "description": "First task", "status": "done", "commit_hash": "abc1234"},
    {"id": "T2", "description": "Second task", "status": "in_progress"},
    {"id": "T3", "description": "Third task", "status": "todo"},
    {"id": "T4", "description": "Skipped task", "status": "skipped"}
  ],
  "plan": {
    "summary": ["Plan summary line 1", "Plan summary line 2"],
    "sections": [
      {"id": "S1", "title": "Phase 1", "description": ["Build core"], "task_ids": ["T1", "T2"]},
      {"id": "S2", "title": "Phase 2", "description": ["Integrate"], "task_ids": ["T3", "T4"]}
    ]
  },
  "status_override": {"status": "blocked", "reason": "waiting on dependency"}
}"#
}
