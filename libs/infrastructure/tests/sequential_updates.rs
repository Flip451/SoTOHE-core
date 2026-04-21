//! Sequential update tests for FsTrackStore.
//!
//! Verifies that sequential read-modify-write operations on metadata.json
//! produce correct results.
//!
//! T005: TrackMetadata is now identity-only. Task-level sequential update tests
//! are deferred to T007 (impl-plan.json task transitions).

#![allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use domain::{TrackId, TrackMetadata, TrackReader, TrackStatus, TrackWriter};
use infrastructure::track::fs_store::FsTrackStore;

fn make_track() -> TrackMetadata {
    // T005: identity-only; no tasks/plan fields.
    TrackMetadata::new(
        TrackId::try_new("concurrency-test").unwrap(),
        "Concurrency Test Track",
        TrackStatus::Planned,
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

    // T007 pending: task-level transitions will use impl-plan.json.
    // For now, verify that sequential status updates are persisted correctly.
    store
        .update(&track_id, |t| {
            t.set_status(TrackStatus::InProgress);
            Ok(())
        })
        .unwrap();

    let final_track = store.find(&track_id).unwrap().unwrap();
    assert_eq!(final_track.status(), TrackStatus::InProgress);
}

#[test]
fn sequential_updates_then_complete_all_results_in_done() {
    let dir = tempfile::tempdir().unwrap();
    let store = FsTrackStore::new(dir.path());

    let track = make_track();
    let track_id = track.id().clone();

    store.save(&track).unwrap();

    // T007 pending: task-level transitions will use impl-plan.json.
    // Verify that setting status to Done is persisted correctly.
    store
        .update(&track_id, |t| {
            t.set_status(TrackStatus::Done);
            Ok(())
        })
        .unwrap();

    let final_track = store.find(&track_id).unwrap().unwrap();
    assert_eq!(final_track.status(), TrackStatus::Done);
}
