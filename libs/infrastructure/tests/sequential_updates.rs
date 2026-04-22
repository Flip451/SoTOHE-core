//! Sequential update tests for FsTrackStore.
//!
//! Verifies that sequential read-modify-write operations on metadata.json
//! produce correct results.
//!
//! TrackMetadata is identity-only. Task-level sequential update tests cover
//! impl-plan.json task transitions. The `status` field is removed from
//! metadata.json; track status is derived on demand. Tests here verify
//! `status_override` persistence.

#![allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use domain::{
    StatusOverride, TrackId, TrackMetadata, TrackReader, TrackWriter, derive_track_status,
};
use infrastructure::track::fs_store::FsTrackStore;

fn make_track() -> TrackMetadata {
    // Identity-only; no tasks/plan/status fields.
    TrackMetadata::new(
        TrackId::try_new("concurrency-test").unwrap(),
        "Concurrency Test Track",
        None,
    )
    .unwrap()
}

#[test]
fn sequential_updates_are_applied_correctly() {
    let dir = tempfile::tempdir().unwrap();
    let store = FsTrackStore::new(dir.path());

    let track = make_track();
    let track_id = track.id().clone();

    store.save(&track).unwrap();

    // Status is derived on demand; test that status_override is persisted correctly.
    store
        .update(&track_id, |t| {
            t.set_status_override(Some(StatusOverride::blocked("testing sequential").unwrap()));
            Ok(())
        })
        .unwrap();

    let final_track = store.find(&track_id).unwrap().unwrap();
    assert!(
        final_track.status_override().is_some(),
        "status_override must be persisted after sequential update"
    );
    assert_eq!(derive_track_status(None, final_track.status_override()).to_string(), "blocked");
}

#[test]
fn sequential_updates_then_clear_override_derives_planned() {
    let dir = tempfile::tempdir().unwrap();
    let store = FsTrackStore::new(dir.path());

    let track = make_track();
    let track_id = track.id().clone();

    store.save(&track).unwrap();

    // Set and then clear override; derived status should return to Planned.
    store
        .update(&track_id, |t| {
            t.set_status_override(Some(StatusOverride::blocked("temporary block").unwrap()));
            Ok(())
        })
        .unwrap();
    store
        .update(&track_id, |t| {
            t.set_status_override(None);
            Ok(())
        })
        .unwrap();

    let final_track = store.find(&track_id).unwrap().unwrap();
    assert!(final_track.status_override().is_none(), "override must be cleared");
    assert_eq!(
        derive_track_status(None, final_track.status_override()).to_string(),
        "planned",
        "no override + no impl-plan → Planned"
    );
}
