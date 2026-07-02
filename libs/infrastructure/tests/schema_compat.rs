//! Schema compatibility tests for v6 identity-only metadata.json.
//!
//! metadata.json is schema_version 6 identity-only. Track status is derived on
//! demand from impl-plan.json — it no longer lives on metadata.json. Legacy
//! v2 / v3 / v4 / v5 metadata is not accepted and is not covered by these tests.
//! Earlier schema-compat tests that round-tripped v2/v5 through the codec were
//! deleted alongside the status-field removal and branch_strategy_snapshot addition.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use infrastructure::track::codec;

const V6_MINIMAL_JSON: &str = r#"{
  "schema_version": 6,
  "id": "test-track",
  "title": "Test",
  "created_at": "2026-03-11T00:00:00Z",
  "updated_at": "2026-03-11T00:00:00Z",
  "branch_strategy_snapshot": {
    "base_branch": "main",
    "merge_target": "main",
    "merge_method": "squash"
  }
}"#;

const V6_WITH_BRANCH_AND_OVERRIDE_JSON: &str = r#"{
  "schema_version": 6,
  "id": "full-featured-track",
  "branch": "track/full-featured-track",
  "title": "Full Featured Track",
  "created_at": "2026-03-11T00:00:00Z",
  "updated_at": "2026-03-11T12:00:00Z",
  "branch_strategy_snapshot": {
    "base_branch": "main",
    "merge_target": "main",
    "merge_method": "squash"
  },
  "status_override": {"status": "blocked", "reason": "waiting on dependency"}
}"#;

/// v6 round-trip preserves identity fields and re-encodes at schema_version 6.
#[test]
fn v6_identity_only_round_trips_through_rust_codec() {
    let (track, meta) = codec::decode(V6_MINIMAL_JSON).unwrap();
    let re_encoded = codec::encode(&track, &meta).unwrap();
    let (track2, meta2) = codec::decode(&re_encoded).unwrap();

    assert_eq!(track, track2, "round-trip must preserve TrackMetadata");
    assert_eq!(meta2.schema_version, 6, "encode always writes schema_version = 6");
}

/// Rust codec emits only the v6 identity-only keys — no `status`, no `tasks`, no `plan`.
#[test]
fn rust_codec_v6_emits_identity_only_keys() {
    let (track, meta) = codec::decode(V6_WITH_BRANCH_AND_OVERRIDE_JSON).unwrap();
    let re_encoded = codec::encode(&track, &meta).unwrap();
    let doc: serde_json::Value = serde_json::from_str(&re_encoded).unwrap();

    // Required identity keys.
    assert_eq!(doc["schema_version"].as_u64().unwrap(), 6);
    assert!(doc.get("id").is_some());
    assert!(doc.get("title").is_some());
    assert!(doc.get("created_at").is_some());
    assert!(doc.get("updated_at").is_some());
    assert!(doc.get("branch_strategy_snapshot").is_some(), "v6 must emit branch_strategy_snapshot");

    // Forbidden (retired) keys.
    assert!(doc.get("status").is_none(), "v6 must not emit `status` — derived on demand");
    assert!(doc.get("tasks").is_none(), "tasks live in impl-plan.json");
    assert!(doc.get("plan").is_none(), "plan lives in impl-plan.json");

    // Optional keys preserved when present.
    assert_eq!(doc["branch"].as_str().unwrap(), "track/full-featured-track");
    assert_eq!(doc["status_override"]["status"].as_str().unwrap(), "blocked");

    // branch_strategy_snapshot preserved.
    assert_eq!(doc["branch_strategy_snapshot"]["base_branch"].as_str().unwrap(), "main");
    assert_eq!(doc["branch_strategy_snapshot"]["merge_method"].as_str().unwrap(), "squash");

    // Identity preservation.
    assert_eq!(doc["id"].as_str().unwrap(), track.id().as_ref());
    assert_eq!(doc["title"].as_str().unwrap(), track.title());
}

/// Minimal v6 doc without an override serialises without a `status_override` key.
#[test]
fn null_status_override_is_omitted_in_v6_json() {
    let (track, meta) = codec::decode(V6_MINIMAL_JSON).unwrap();
    let re_encoded = codec::encode(&track, &meta).unwrap();
    let doc: serde_json::Value = serde_json::from_str(&re_encoded).unwrap();

    assert!(doc.get("status_override").is_none(), "absent status_override must not be emitted");
    assert_eq!(doc["id"].as_str().unwrap(), track.id().as_ref());
}
