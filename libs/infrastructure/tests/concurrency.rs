//! Concurrency tests: parallel metadata.json write simulation via FsTrackStore.
//!
//! Uses real FsFileLockManager to verify lock-based serialization.

#![allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;
use std::thread;
use std::time::Duration;

use domain::{
    PlanSection, PlanView, TaskId, TaskTransition, TrackId, TrackMetadata, TrackReader,
    TrackStatus, TrackTask, TrackWriter,
};
use infrastructure::lock::FsFileLockManager;
use infrastructure::track::fs_store::FsTrackStore;

const LOCK_TIMEOUT: Duration = Duration::from_secs(10);

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
fn parallel_updates_are_serialized_by_lock_manager() {
    let dir = tempfile::tempdir().unwrap();
    let locks_dir = tempfile::tempdir().unwrap();

    let lock_manager = Arc::new(FsFileLockManager::new(locks_dir.path()).unwrap());
    let store = Arc::new(FsTrackStore::new(dir.path(), lock_manager, LOCK_TIMEOUT));

    let num_tasks = 5;
    let track = make_track(num_tasks);
    let track_id = track.id().clone();

    // Save initial track.
    store.save(&track).unwrap();

    // Spawn threads, each transitioning a different task to in_progress.
    let handles: Vec<_> = (1..=num_tasks)
        .map(|i| {
            let store = Arc::clone(&store);
            let track_id = track_id.clone();
            thread::spawn(move || {
                let task_id = TaskId::new(format!("T{i}")).unwrap();
                store
                    .update(&track_id, |t| {
                        t.transition_task(&task_id, TaskTransition::Start)?;
                        Ok(())
                    })
                    .unwrap();
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
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
fn parallel_updates_then_complete_all_results_in_done() {
    let dir = tempfile::tempdir().unwrap();
    let locks_dir = tempfile::tempdir().unwrap();

    let lock_manager = Arc::new(FsFileLockManager::new(locks_dir.path()).unwrap());
    let store = Arc::new(FsTrackStore::new(dir.path(), lock_manager, LOCK_TIMEOUT));

    let num_tasks = 3;
    let track = make_track(num_tasks);
    let track_id = track.id().clone();

    store.save(&track).unwrap();

    // Phase 1: Start all tasks in parallel.
    let handles: Vec<_> = (1..=num_tasks)
        .map(|i| {
            let store = Arc::clone(&store);
            let track_id = track_id.clone();
            thread::spawn(move || {
                let task_id = TaskId::new(format!("T{i}")).unwrap();
                store
                    .update(&track_id, |t| {
                        t.transition_task(&task_id, TaskTransition::Start)?;
                        Ok(())
                    })
                    .unwrap();
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    // Phase 2: Complete all tasks in parallel.
    let handles: Vec<_> = (1..=num_tasks)
        .map(|i| {
            let store = Arc::clone(&store);
            let track_id = track_id.clone();
            thread::spawn(move || {
                let task_id = TaskId::new(format!("T{i}")).unwrap();
                store
                    .update(&track_id, |t| {
                        t.transition_task(
                            &task_id,
                            TaskTransition::Complete { commit_hash: None },
                        )?;
                        Ok(())
                    })
                    .unwrap();
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    // Verify: track should be done (all tasks resolved).
    let final_track = store.find(&track_id).unwrap().unwrap();
    assert_eq!(final_track.status(), TrackStatus::Done);
}
