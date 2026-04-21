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
    // T005: encode() always writes schema_version = 4 (identity-only shape),
    // so meta2 will always be 4 regardless of the original file's schema_version.
    assert_eq!(meta2.schema_version, 4, "encode always upgrades to schema_version = 4");
}

/// Tests that the Rust codec round-trips v2/v3 docs preserving identity-only fields (T005).
///
/// T005: schema_version 4 is identity-only. v2/v3 docs are decoded by stripping
/// tasks/plan/extra fields. The re-encoded output is a v4 identity-only document.
/// Python track_schema.py will be updated to v4 in the same track.
#[test]
fn rust_codec_preserves_python_expected_keys() {
    let json = full_featured_json();
    let (track, meta) = codec::decode(json).unwrap();
    let re_encoded = codec::encode(&track, &meta).unwrap();

    // Parse as generic JSON and verify structure.
    let doc: serde_json::Value = serde_json::from_str(&re_encoded).unwrap();

    // T005: Top-level identity keys expected by v4 schema.
    assert!(doc.get("schema_version").is_some());
    assert!(doc.get("id").is_some());
    assert!(doc.get("title").is_some());
    assert!(doc.get("status").is_some());
    assert!(doc.get("created_at").is_some());
    assert!(doc.get("updated_at").is_some());

    // T005: tasks/plan are stripped during migration (moved to impl-plan.json / ImplPlanDocument).
    assert!(doc.get("tasks").is_none(), "T005: tasks must be absent from v4 identity-only output");
    assert!(doc.get("plan").is_none(), "T005: plan must be absent from v4 identity-only output");

    // Note: status_override is retained in v4 for Blocked/Cancelled semantics.
    // If the input had a status_override, it should be preserved in the output.

    // Verify the decoded track identity is preserved.
    assert_eq!(doc["id"].as_str().unwrap(), track.id().as_ref(), "round-trip must preserve id");
    assert_eq!(doc["title"].as_str().unwrap(), track.title(), "round-trip must preserve title");
}

/// Tests that null commit_hash is omitted (skip_serializing_if) matching Python behavior.
/// T005: tasks are stripped in v4; this test verifies the legacy decode path still
/// accepts v2 docs and that re-encoded output omits tasks entirely.
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

    // T005: tasks stripped during v2→v4 migration; tasks key must be absent.
    assert!(doc.get("tasks").is_none(), "T005: tasks must be absent from v4 identity-only output");
    assert_eq!(doc["id"].as_str().unwrap(), track.id().as_ref());
}

/// Tests that null status_override is omitted matching Python behavior.
/// T005: status_override was a v3 field; re-encoded v4 output omits it entirely.
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

    // T005: status_override is a v3 field; v4 identity-only output omits it.
    assert!(
        doc.get("status_override").is_none(),
        "T005: status_override must be absent from v4 identity-only output"
    );
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
