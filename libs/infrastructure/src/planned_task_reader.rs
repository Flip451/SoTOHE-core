//! Filesystem secondary adapter for the planned tasks (IN-07, AC-07).
//!
//! [`FsPlannedTaskReader`] implements
//! [`usecase::batch_plan::PlannedTaskReaderPort`] over
//! `<items_dir>/<track-id>/impl-plan.json`, returning the tasks in declaration
//! order with their status and the dependencies they declare — the edges the
//! batch-order check reads. An absent plan is the port's `NotFound` state
//! rather than an empty task list.

use std::path::Path;

use domain::{FreeText, TrackId, TrackTask};
use usecase::batch_plan::{PlanArtifactReadError, PlannedTaskReaderPort};

use crate::impl_plan_codec;
use crate::track_artifact::{TrackArtifactReadError, read_track_artifact};

const IMPL_PLAN_FILE: &str = "impl-plan.json";
const MAX_IMPL_PLAN_BYTES: u64 = 16 * 1024 * 1024;

/// Reads a track's planned tasks from the filesystem.
///
/// Constructed with no arguments so composition roots stay zero-argument
/// wiring accessors; the items directory arrives with each call.
#[derive(Debug, Default)]
pub struct FsPlannedTaskReader;

impl FsPlannedTaskReader {
    /// Creates the adapter.
    #[must_use]
    pub fn new() -> FsPlannedTaskReader {
        FsPlannedTaskReader
    }
}

impl PlannedTaskReaderPort for FsPlannedTaskReader {
    fn read_planned_tasks(
        &self,
        items_dir: &Path,
        track_id: &TrackId,
    ) -> Result<Vec<TrackTask>, PlanArtifactReadError> {
        let contents =
            read_track_artifact(items_dir, track_id, IMPL_PLAN_FILE, MAX_IMPL_PLAN_BYTES).map_err(
                |error| match error {
                    TrackArtifactReadError::NotFound => PlanArtifactReadError::NotFound,
                    TrackArtifactReadError::Failed(message) => {
                        PlanArtifactReadError::ReadFailed { message: FreeText::new(message) }
                    }
                },
            )?;

        let document = impl_plan_codec::decode(&contents).map_err(|error| {
            PlanArtifactReadError::ReadFailed {
                message: FreeText::new(format!(
                    "codec error reading {IMPL_PLAN_FILE} for track '{}': {error}",
                    track_id.as_ref()
                )),
            }
        })?;

        Ok(document.tasks().to_vec())
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::fs;

    use domain::TaskId;

    use super::*;

    /// Two tasks in declaration order; the second declares a dependency.
    const SAMPLE_JSON: &str = r#"{
      "schema_version": 2,
      "tasks": [
        {"id": "T001", "description": "First", "status": "done", "commit_hash": "a1b2c3d"},
        {"id": "T002", "description": "Second", "status": "todo", "depends_on": ["T001"]}
      ],
      "plan": {"summary": [], "sections": [
        {"id": "S1", "title": "Impl", "description": [], "task_ids": ["T001", "T002"]}
      ]}
    }"#;

    fn track_id(value: &str) -> TrackId {
        TrackId::try_new(value).unwrap()
    }

    fn temp_items_dir() -> tempfile::TempDir {
        tempfile::Builder::new()
            .prefix("planned-task-reader-")
            .tempdir_in(std::env::current_dir().unwrap())
            .unwrap()
    }

    #[test]
    fn test_the_reader_returns_the_tasks_in_declaration_order_with_what_they_declare() {
        let dir = temp_items_dir();
        let track_dir = dir.path().join("my-track");
        fs::create_dir_all(&track_dir).unwrap();
        fs::write(track_dir.join("impl-plan.json"), SAMPLE_JSON).unwrap();

        let tasks = FsPlannedTaskReader::new()
            .read_planned_tasks(dir.path(), &track_id("my-track"))
            .unwrap();

        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].id().as_ref(), "T001");
        assert_eq!(tasks[1].id().as_ref(), "T002");
        assert!(tasks[0].depends_on().is_empty());
        assert_eq!(tasks[1].depends_on(), &[TaskId::try_new("T001").unwrap()]);
        assert_eq!(tasks[1].status(), &domain::TaskStatus::Todo);
    }

    #[test]
    fn test_an_absent_implementation_plan_is_reported_as_not_found_rather_than_no_tasks() {
        let dir = temp_items_dir();

        let error = FsPlannedTaskReader::new()
            .read_planned_tasks(dir.path(), &track_id("no-such-track"))
            .unwrap_err();

        assert!(matches!(error, PlanArtifactReadError::NotFound), "unexpected error: {error}");
    }

    #[test]
    fn test_an_undecodable_implementation_plan_is_reported_as_a_read_failure() {
        let dir = temp_items_dir();
        let track_dir = dir.path().join("my-track");
        fs::create_dir_all(&track_dir).unwrap();
        fs::write(track_dir.join("impl-plan.json"), b"{ not json").unwrap();

        let error = FsPlannedTaskReader::new()
            .read_planned_tasks(dir.path(), &track_id("my-track"))
            .unwrap_err();

        assert!(
            matches!(error, PlanArtifactReadError::ReadFailed { .. }),
            "unexpected error: {error}"
        );
    }
}
