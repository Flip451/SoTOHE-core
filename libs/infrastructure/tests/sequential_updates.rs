//! Sequential update tests for FsTrackStore.
//!
//! Verifies that sequential read-modify-write operations on metadata.json
//! produce correct results.

#![allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use domain::{
    PlanSection, PlanView, TaskId, TaskTransition, TrackId, TrackMetadata, TrackReader,
    TrackStatus, TrackTask, TrackWriter,
};
use infrastructure::track::fs_store::FsTrackStore;

fn make_track(num_tasks: usize) -> TrackMetadata {
    let tasks: Vec<TrackTask> = (1..=num_tasks)
        .map(|i| {
            let id = TaskId::new(format!("T{i}")).unwrap();
            TrackTask::new(id, format!("Task {i}")).unwrap()
        })
        .collect();

    let task_ids: Vec<TaskId> = tasks.iter().map(|t| t.id().clone()).collect();
    let section = PlanSection::new("S1", "All tasks", Vec::new(), task_ids).unwrap();
    let plan = PlanView::new(Vec::new(), vec![section]);

    TrackMetadata::new(
        TrackId::new("concurrency-test").unwrap(),
        "Concurrency Test Track",
        tasks,
        plan,
        None,
    )
    .unwrap()
}

#[test]
fn sequential_updates_are_applied_correctly() {
    let dir = tempfile::tempdir().unwrap();

    let store = FsTrackStore::new(dir.path());

    let num_tasks = 5;
    let track = make_track(num_tasks);
    let track_id = track.id().clone();

    store.save(&track).unwrap();

    // Sequentially transition each task to in_progress.
    for i in 1..=num_tasks {
        let task_id = TaskId::new(format!("T{i}")).unwrap();
        store
            .update(&track_id, |t| {
                t.transition_task(&task_id, TaskTransition::Start)?;
                Ok(())
            })
            .unwrap();
    }

    // Verify: all tasks should be in_progress.
    let final_track = store.find(&track_id).unwrap().unwrap();
    assert_eq!(final_track.status(), TrackStatus::InProgress);

    for task in final_track.tasks() {
        assert_eq!(
            task.status().kind(),
            domain::TaskStatusKind::InProgress,
            "task {} should be in_progress",
            task.id()
        );
    }
}

#[test]
fn sequential_updates_then_complete_all_results_in_done() {
    let dir = tempfile::tempdir().unwrap();

    let store = FsTrackStore::new(dir.path());

    let num_tasks = 3;
    let track = make_track(num_tasks);
    let track_id = track.id().clone();

    store.save(&track).unwrap();

    // Phase 1: Start all tasks sequentially.
    for i in 1..=num_tasks {
        let task_id = TaskId::new(format!("T{i}")).unwrap();
        store
            .update(&track_id, |t| {
                t.transition_task(&task_id, TaskTransition::Start)?;
                Ok(())
            })
            .unwrap();
    }

    // Phase 2: Complete all tasks sequentially.
    for i in 1..=num_tasks {
        let task_id = TaskId::new(format!("T{i}")).unwrap();
        store
            .update(&track_id, |t| {
                t.transition_task(&task_id, TaskTransition::Complete { commit_hash: None })?;
                Ok(())
            })
            .unwrap();
    }

    // Verify: track should be done (all tasks resolved).
    let final_track = store.find(&track_id).unwrap().unwrap();
    assert_eq!(final_track.status(), TrackStatus::Done);
}
