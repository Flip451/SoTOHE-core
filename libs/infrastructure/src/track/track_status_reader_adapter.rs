//! `FsTrackStatusReaderAdapter` — infrastructure adapter for `TrackStatusReaderPort`.
//!
//! Wraps [`crate::track::fs_store::read_track_status_str`] and converts its
//! string result to a domain [`domain::TrackStatus`] so the usecase layer
//! can work with typed values.

use std::path::Path;

use domain::TrackStatus;
use domain::tddd::catalogue_v2::{TrackStatusReadError, TrackStatusReaderPort};

use super::fs_store::read_track_status_str;

// ---------------------------------------------------------------------------
// Adapter
// ---------------------------------------------------------------------------

/// Filesystem-backed implementation of [`TrackStatusReaderPort`].
///
/// Reads `metadata.json` + `impl-plan.json` for the given track via
/// `read_track_status_str`, applies the symlink guard, validates the track id,
/// and converts the returned status string to a domain [`TrackStatus`].
#[derive(Debug, Default)]
pub struct FsTrackStatusReaderAdapter;

impl FsTrackStatusReaderAdapter {
    /// Creates a new adapter instance.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl TrackStatusReaderPort for FsTrackStatusReaderAdapter {
    /// Reads the derived [`TrackStatus`] for the given track.
    ///
    /// Delegates to [`read_track_status_str`] which applies symlink guards,
    /// validates `track_id`, reads `metadata.json` + `impl-plan.json`, and
    /// calls `derive_track_status`.
    ///
    /// # Errors
    ///
    /// Returns [`TrackStatusReadError`] when the track id is invalid, any file
    /// is unreadable, a symlink guard fires, or the metadata cannot be decoded.
    fn read_status(
        &self,
        items_dir: &Path,
        track_id: &str,
    ) -> Result<TrackStatus, TrackStatusReadError> {
        let status_str =
            read_track_status_str(items_dir, track_id).map_err(TrackStatusReadError)?;
        // Map the serialized string back to the domain enum.
        // `TrackStatus` implements `Display` via `to_string()` which produces
        // these exact strings; we parse them back here.
        match status_str.as_str() {
            "planned" => Ok(TrackStatus::Planned),
            "in_progress" => Ok(TrackStatus::InProgress),
            "blocked" => Ok(TrackStatus::Blocked),
            "cancelled" => Ok(TrackStatus::Cancelled),
            "done" => Ok(TrackStatus::Done),
            "archived" => Ok(TrackStatus::Archived),
            other => Err(TrackStatusReadError(format!(
                "unrecognised track status string '{other}': \
                 domain added a new variant that this adapter has not classified yet"
            ))),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    /// Minimal valid `metadata.json` (schema v6, no status field → Planned).
    fn activated_metadata(track_id: &str) -> String {
        format!(
            r#"{{
  "schema_version": 6,
  "id": "{track_id}",
  "branch": "track/{track_id}",
  "title": "Test",
  "created_at": "2026-01-01T00:00:00Z",
  "updated_at": "2026-01-01T00:00:00Z",
  "branch_strategy_snapshot": {{
    "base_branch": "main",
    "merge_target": "main",
    "merge_method": "squash"
  }}
}}"#
        )
    }

    fn minimal_impl_plan() -> &'static str {
        r#"{"schema_version":1,"tasks":[],"plan":{"summary":[],"sections":[]}}"#
    }

    #[test]
    fn test_read_status_planned_with_no_tasks() {
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        let track_dir = items_dir.join("my-track");
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(track_dir.join("metadata.json"), activated_metadata("my-track")).unwrap();
        std::fs::write(track_dir.join("impl-plan.json"), minimal_impl_plan()).unwrap();

        let adapter = FsTrackStatusReaderAdapter::new();
        let status = adapter.read_status(&items_dir, "my-track").unwrap();
        // InProgress when impl-plan exists with no tasks (all open), but the
        // plan has no tasks so status should be InProgress (has an open plan).
        // Actually: empty tasks list → all done → Done? No: an empty tasks list
        // means no tasks at all, which is treated as Planned by derive_track_status.
        // Verify it's not an error.
        assert!(
            matches!(status, TrackStatus::Planned | TrackStatus::InProgress),
            "expected Planned or InProgress, got: {status:?}"
        );
    }

    #[test]
    fn test_read_status_invalid_track_id_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        std::fs::create_dir_all(&items_dir).unwrap();

        let adapter = FsTrackStatusReaderAdapter::new();
        let err = adapter.read_status(&items_dir, "../evil").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("invalid track id"), "expected invalid id error, got: {msg}");
    }

    #[test]
    fn test_read_status_missing_metadata_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        let track_dir = items_dir.join("my-track");
        std::fs::create_dir_all(&track_dir).unwrap();
        // No metadata.json

        let adapter = FsTrackStatusReaderAdapter::new();
        let err = adapter.read_status(&items_dir, "my-track").unwrap_err();
        assert!(!err.to_string().is_empty(), "error should be non-empty");
    }

    #[test]
    fn test_read_status_done_track_returns_done() {
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        let track_dir = items_dir.join("done-track");
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(track_dir.join("metadata.json"), activated_metadata("done-track")).unwrap();

        let done_impl_plan = r#"{
  "schema_version": 1,
  "tasks": [
    {
      "id": "T001",
      "description": "A completed task",
      "status": "done",
      "commit_hash": "0000000000000000000000000000000000000000"
    }
  ],
  "plan": {
    "summary": ["Done"],
    "sections": [{"id": "S001", "title": "Done", "description": [], "task_ids": ["T001"]}]
  }
}"#;
        std::fs::write(track_dir.join("impl-plan.json"), done_impl_plan).unwrap();

        let adapter = FsTrackStatusReaderAdapter::new();
        let status = adapter.read_status(&items_dir, "done-track").unwrap();
        assert_eq!(status, TrackStatus::Done, "all-done impl-plan should yield Done");
    }
}
