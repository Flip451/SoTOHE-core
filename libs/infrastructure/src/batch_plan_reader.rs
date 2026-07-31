//! Filesystem secondary adapter for `batch-plan.json` (IN-01, AC-05, CN-09).
//!
//! [`FsBatchPlanReader`] implements
//! [`usecase::batch_plan::BatchPlanReaderPort`] over
//! `<items_dir>/<track-id>/batch-plan.json`. An absent file is the port's
//! `NotFound` state, not an empty plan: on a track this gate applies to, the
//! artifact is required.

use std::path::Path;

use domain::batch_plan::BatchPlanDocument;
use domain::{FreeText, TrackId};
use usecase::batch_plan::{BatchPlanReaderPort, PlanArtifactReadError};

use crate::batch_plan_codec;
use crate::track_artifact::{TrackArtifactReadError, read_track_artifact};

const BATCH_PLAN_FILE: &str = "batch-plan.json";
const MAX_BATCH_PLAN_BYTES: u64 = 4 * 1024 * 1024;

/// Reads a track's declared batch plan from the filesystem.
///
/// Constructed with no arguments so composition roots stay zero-argument
/// wiring accessors; the items directory arrives with each call.
#[derive(Debug, Default)]
pub struct FsBatchPlanReader;

impl FsBatchPlanReader {
    /// Creates the adapter.
    #[must_use]
    pub fn new() -> FsBatchPlanReader {
        FsBatchPlanReader
    }
}

impl BatchPlanReaderPort for FsBatchPlanReader {
    fn read(
        &self,
        items_dir: &Path,
        track_id: &TrackId,
    ) -> Result<BatchPlanDocument, PlanArtifactReadError> {
        let contents =
            read_track_artifact(items_dir, track_id, BATCH_PLAN_FILE, MAX_BATCH_PLAN_BYTES)
                .map_err(|error| match error {
                    TrackArtifactReadError::NotFound => PlanArtifactReadError::NotFound,
                    // The reader states what was wrong and nothing about where it
                    // looked; the artifact is identified by the well-known name
                    // this adapter owns and by the track it was asked for.
                    TrackArtifactReadError::Failed(classification) => {
                        PlanArtifactReadError::ReadFailed {
                            message: FreeText::new(format!(
                                "read {BATCH_PLAN_FILE} for track '{}': {classification}",
                                track_id.as_ref()
                            )),
                        }
                    }
                })?;

        let plan = batch_plan_codec::decode(&contents).map_err(|error| {
            PlanArtifactReadError::ReadFailed {
                message: FreeText::new(format!(
                    "codec error reading {BATCH_PLAN_FILE} for track '{}': {error}",
                    track_id.as_ref()
                )),
            }
        })?;

        if plan.track_id() != track_id {
            return Err(PlanArtifactReadError::ReadFailed {
                message: FreeText::new(format!(
                    "{BATCH_PLAN_FILE} track_id '{}' does not match requested track '{}'",
                    plan.track_id().as_ref(),
                    track_id.as_ref()
                )),
            });
        }

        Ok(plan)
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::fs;

    use super::*;

    const SAMPLE_JSON: &str = r#"{
      "schema_version": 1,
      "track_id": "my-track",
      "task_estimates": [
        {
          "task_id": "T001",
          "scope_estimates": [
            {"scope": "domain", "production_lines": 100, "test_lines": 50}
          ],
          "oversize_justification": null
        }
      ],
      "batches": [{"id": "B1", "task_ids": ["T001"]}]
    }"#;

    fn track_id(value: &str) -> TrackId {
        TrackId::try_new(value).unwrap()
    }

    fn temp_items_dir() -> tempfile::TempDir {
        tempfile::Builder::new()
            .prefix("batch-plan-reader-")
            .tempdir_in(std::env::current_dir().unwrap())
            .unwrap()
    }

    #[test]
    fn test_the_reader_returns_the_declared_plan_of_the_track_it_was_asked_for() {
        let dir = temp_items_dir();
        let track_dir = dir.path().join("my-track");
        fs::create_dir_all(&track_dir).unwrap();
        fs::write(track_dir.join("batch-plan.json"), SAMPLE_JSON).unwrap();

        let plan = FsBatchPlanReader::new().read(dir.path(), &track_id("my-track")).unwrap();

        assert_eq!(plan.track_id().as_ref(), "my-track");
        assert_eq!(plan.batches().len(), 1);
        assert_eq!(plan.batches()[0].id().as_str(), "B1");
    }

    #[test]
    fn test_an_absent_batch_plan_is_reported_as_not_found_rather_than_an_empty_plan() {
        let dir = temp_items_dir();

        let error =
            FsBatchPlanReader::new().read(dir.path(), &track_id("no-such-track")).unwrap_err();

        assert!(matches!(error, PlanArtifactReadError::NotFound), "unexpected error: {error}");
    }

    #[test]
    fn test_a_track_that_exists_without_a_batch_plan_is_still_reported_as_not_found() {
        // The track directory is there and holds its other planning artifact;
        // only `batch-plan.json` is missing, which is the shape an active track
        // has before the plan is authored.
        let dir = temp_items_dir();
        let track_dir = dir.path().join("my-track");
        fs::create_dir_all(&track_dir).unwrap();
        fs::write(track_dir.join("impl-plan.json"), "{}").unwrap();

        let error = FsBatchPlanReader::new().read(dir.path(), &track_id("my-track")).unwrap_err();

        assert!(matches!(error, PlanArtifactReadError::NotFound), "unexpected error: {error}");
    }

    #[test]
    fn test_an_undecodable_batch_plan_is_reported_as_a_read_failure() {
        let dir = temp_items_dir();
        let track_dir = dir.path().join("my-track");
        fs::create_dir_all(&track_dir).unwrap();
        fs::write(track_dir.join("batch-plan.json"), b"{ not json").unwrap();

        let error = FsBatchPlanReader::new().read(dir.path(), &track_id("my-track")).unwrap_err();

        assert!(
            matches!(error, PlanArtifactReadError::ReadFailed { .. }),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn test_a_read_failure_names_the_artifact_and_never_the_directory_it_was_read_from() {
        // A directory where the plan belongs is refused by the guarded read, and
        // that refusal is what the operator sees rendered at the gate: it names
        // the well-known file, the track asked for, and what was wrong — not the
        // absolute location the process was pointed at.
        let dir = temp_items_dir();
        let track_dir = dir.path().join("my-track");
        fs::create_dir_all(track_dir.join("batch-plan.json")).unwrap();

        let error = FsBatchPlanReader::new().read(dir.path(), &track_id("my-track")).unwrap_err();

        let rendered = error.to_string();
        assert!(
            rendered.contains("batch-plan.json") && rendered.contains("my-track"),
            "the artifact stays identified: {rendered}"
        );
        assert!(rendered.contains("not a regular file"), "the cause is stated: {rendered}");
        for spelling in [dir.path().to_path_buf(), dir.path().canonicalize().unwrap()] {
            assert!(
                !rendered.contains(&spelling.display().to_string()),
                "no host path reaches the port error: {rendered}"
            );
        }
    }

    #[test]
    fn test_a_batch_plan_for_a_different_track_is_reported_as_a_read_failure() {
        let dir = temp_items_dir();
        let track_dir = dir.path().join("requested-track");
        fs::create_dir_all(&track_dir).unwrap();
        fs::write(track_dir.join("batch-plan.json"), SAMPLE_JSON).unwrap();

        let error =
            FsBatchPlanReader::new().read(dir.path(), &track_id("requested-track")).unwrap_err();

        assert!(
            matches!(error, PlanArtifactReadError::ReadFailed { .. }),
            "unexpected error: {error}"
        );
    }
}
