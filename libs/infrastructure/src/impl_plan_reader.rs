//! Filesystem secondary adapter for reading `impl-plan.json` task statuses (D7).
//!
//! [`FsImplPlanReader`] implements
//! [`usecase::pre_review_gate::ImplPlanReaderPort`]. It reads
//! `<items_dir>/<track_id>/impl-plan.json`, decodes it via
//! [`super::impl_plan_codec::decode`], and returns a
//! `HashMap<TaskId, TaskStatusKind>` for use by `PreReviewGateInteractor` to
//! filter task attributions by status before evaluating `impl_catalog` signals.

use std::collections::HashMap;
use std::path::PathBuf;

use domain::{TaskId, TaskStatusKind, TrackId};
use usecase::pre_review_gate::{ImplPlanReaderPort, PreReviewGateError};

use crate::impl_plan_codec;
use crate::track::symlink_guard::reject_symlinks_below;

const MAX_IMPL_PLAN_BYTES: u64 = 16 * 1024 * 1024;

/// Filesystem secondary adapter implementing
/// [`usecase::pre_review_gate::ImplPlanReaderPort`].
///
/// Reads `<items_dir>/<track_id>/impl-plan.json`, decodes it via
/// `impl_plan_codec::decode`, and returns a `HashMap<TaskId, TaskStatusKind>`.
///
/// - Missing file maps to [`PreReviewGateError::ImplPlanReadFailed`].
/// - I/O and codec errors map to [`PreReviewGateError::ImplPlanReadFailed`].
///
/// The `items_dir` is injected at construction time so callers do not need to
/// pass it on every [`read_task_statuses`](FsImplPlanReader::read_task_statuses) call.
#[derive(Debug)]
pub struct FsImplPlanReader {
    items_dir: PathBuf,
}

impl FsImplPlanReader {
    /// Construct a `FsImplPlanReader` with the given items directory root.
    #[must_use]
    pub fn new(items_dir: PathBuf) -> Self {
        Self { items_dir }
    }
}

impl ImplPlanReaderPort for FsImplPlanReader {
    fn read_task_statuses(
        &self,
        track_id: &TrackId,
    ) -> Result<HashMap<TaskId, TaskStatusKind>, PreReviewGateError> {
        let items_dir =
            crate::resolve_items_dir_under_current_repo(&self.items_dir).map_err(|e| {
                PreReviewGateError::ImplPlanReadFailed {
                    message: format!("items_dir rejected before reading impl-plan.json: {e}"),
                }
            })?;
        match std::fs::symlink_metadata(&items_dir) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(PreReviewGateError::ImplPlanReadFailed {
                    message: format!(
                        "symlink check failed for {}: refused symlink",
                        items_dir.display()
                    ),
                });
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(PreReviewGateError::ImplPlanReadFailed {
                    message: format!("metadata error reading {}: {e}", items_dir.display()),
                });
            }
        }
        let path = items_dir.join(track_id.as_ref()).join("impl-plan.json");

        match reject_symlinks_below(&path, &items_dir) {
            Ok(true) => {}
            Ok(false) => {
                return Err(PreReviewGateError::ImplPlanReadFailed {
                    message: format!(
                        "impl-plan.json not found for track '{}': {}",
                        track_id.as_ref(),
                        path.display()
                    ),
                });
            }
            Err(e) => {
                return Err(PreReviewGateError::ImplPlanReadFailed {
                    message: format!("symlink check failed for {}: {e}", path.display()),
                });
            }
        }

        let metadata = match std::fs::symlink_metadata(&path) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(PreReviewGateError::ImplPlanReadFailed {
                    message: format!(
                        "symlink check failed for {}: refused symlink",
                        path.display()
                    ),
                });
            }
            Ok(meta) => meta,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(PreReviewGateError::ImplPlanReadFailed {
                    message: format!(
                        "impl-plan.json not found for track '{}': {}",
                        track_id.as_ref(),
                        path.display()
                    ),
                });
            }
            Err(e) => {
                return Err(PreReviewGateError::ImplPlanReadFailed {
                    message: format!("metadata error reading {}: {e}", path.display()),
                });
            }
        };
        if metadata.len() > MAX_IMPL_PLAN_BYTES {
            return Err(PreReviewGateError::ImplPlanReadFailed {
                message: format!(
                    "impl-plan.json exceeds maximum size of {MAX_IMPL_PLAN_BYTES} bytes: {} bytes",
                    metadata.len()
                ),
            });
        }

        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(PreReviewGateError::ImplPlanReadFailed {
                    message: format!(
                        "impl-plan.json not found for track '{}': {}",
                        track_id.as_ref(),
                        path.display()
                    ),
                });
            }
            Err(e) => {
                return Err(PreReviewGateError::ImplPlanReadFailed {
                    message: format!("I/O error reading {}: {e}", path.display()),
                });
            }
        };

        let contents =
            std::str::from_utf8(&bytes).map_err(|e| PreReviewGateError::ImplPlanReadFailed {
                message: format!("UTF-8 error in {}: {e}", path.display()),
            })?;

        let doc = impl_plan_codec::decode(contents).map_err(|e| {
            PreReviewGateError::ImplPlanReadFailed {
                message: format!(
                    "codec error reading impl-plan.json for track '{}': {e}",
                    track_id.as_ref()
                ),
            }
        })?;

        let statuses =
            doc.tasks().iter().map(|task| (task.id().clone(), task.status().kind())).collect();

        Ok(statuses)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use std::fs;

    use domain::{TaskId, TaskStatusKind, TrackId};
    use usecase::pre_review_gate::PreReviewGateError;

    use super::*;

    fn track_id(s: &str) -> TrackId {
        TrackId::try_new(s).unwrap()
    }

    fn temp_items_dir() -> tempfile::TempDir {
        tempfile::Builder::new()
            .prefix("impl-plan-reader-")
            .tempdir_in(std::env::current_dir().unwrap())
            .unwrap()
    }

    const SAMPLE_JSON: &str = r#"{
  "schema_version": 1,
  "tasks": [
    {"id": "T001", "description": "First task", "status": "todo"},
    {"id": "T002", "description": "Second task", "status": "in_progress"},
    {"id": "T003", "description": "Third task", "status": "done"}
  ],
  "plan": {
    "summary": [],
    "sections": [
      {
        "id": "S1",
        "title": "Section",
        "description": [],
        "task_ids": ["T001", "T002", "T003"]
      }
    ]
  }
}"#;

    #[test]
    fn read_task_statuses_returns_map_for_existing_file() {
        let dir = temp_items_dir();
        let track_dir = dir.path().join("my-track");
        fs::create_dir_all(&track_dir).unwrap();
        fs::write(track_dir.join("impl-plan.json"), SAMPLE_JSON).unwrap();

        let reader = FsImplPlanReader::new(dir.path().to_path_buf());
        let statuses = reader.read_task_statuses(&track_id("my-track")).unwrap();

        let t1: TaskId = TaskId::try_new("T001").unwrap();
        let t2: TaskId = TaskId::try_new("T002").unwrap();
        let t3: TaskId = TaskId::try_new("T003").unwrap();

        assert_eq!(statuses[&t1], TaskStatusKind::Todo);
        assert_eq!(statuses[&t2], TaskStatusKind::InProgress);
        assert_eq!(statuses[&t3], TaskStatusKind::Done);
    }

    #[test]
    fn read_task_statuses_returns_impl_plan_read_failed_for_missing_file() {
        let dir = temp_items_dir();
        let reader = FsImplPlanReader::new(dir.path().to_path_buf());
        let err = reader.read_task_statuses(&track_id("nonexistent-track")).unwrap_err();
        assert!(
            matches!(err, PreReviewGateError::ImplPlanReadFailed { .. }),
            "expected ImplPlanReadFailed, got: {err}"
        );
    }

    #[test]
    fn read_task_statuses_returns_impl_plan_read_failed_for_malformed_json() {
        let dir = temp_items_dir();
        let track_dir = dir.path().join("my-track");
        fs::create_dir_all(&track_dir).unwrap();
        fs::write(track_dir.join("impl-plan.json"), b"not json").unwrap();

        let reader = FsImplPlanReader::new(dir.path().to_path_buf());
        let err = reader.read_task_statuses(&track_id("my-track")).unwrap_err();
        assert!(
            matches!(err, PreReviewGateError::ImplPlanReadFailed { .. }),
            "expected ImplPlanReadFailed, got: {err}"
        );
    }

    #[test]
    fn read_task_statuses_returns_impl_plan_read_failed_for_oversized_file() {
        let dir = temp_items_dir();
        let track_dir = dir.path().join("my-track");
        fs::create_dir_all(&track_dir).unwrap();
        let file = fs::File::create(track_dir.join("impl-plan.json")).unwrap();
        file.set_len(MAX_IMPL_PLAN_BYTES + 1).unwrap();

        let reader = FsImplPlanReader::new(dir.path().to_path_buf());
        let err = reader.read_task_statuses(&track_id("my-track")).unwrap_err();
        assert!(
            matches!(err, PreReviewGateError::ImplPlanReadFailed { .. }),
            "expected ImplPlanReadFailed, got: {err}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn read_task_statuses_returns_impl_plan_read_failed_for_symlinked_impl_plan_file() {
        let dir = temp_items_dir();
        let track_dir = dir.path().join("my-track");
        fs::create_dir_all(&track_dir).unwrap();
        let real = track_dir.join("real-impl-plan.json");
        fs::write(&real, SAMPLE_JSON).unwrap();
        std::os::unix::fs::symlink(&real, track_dir.join("impl-plan.json")).unwrap();

        let reader = FsImplPlanReader::new(dir.path().to_path_buf());
        let err = reader.read_task_statuses(&track_id("my-track")).unwrap_err();
        assert!(
            matches!(err, PreReviewGateError::ImplPlanReadFailed { .. }),
            "expected ImplPlanReadFailed, got: {err}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn read_task_statuses_returns_impl_plan_read_failed_for_symlinked_track_dir() {
        let dir = temp_items_dir();
        let real_track_dir = dir.path().join("real-track");
        fs::create_dir_all(&real_track_dir).unwrap();
        fs::write(real_track_dir.join("impl-plan.json"), SAMPLE_JSON).unwrap();
        std::os::unix::fs::symlink(&real_track_dir, dir.path().join("my-track")).unwrap();

        let reader = FsImplPlanReader::new(dir.path().to_path_buf());
        let err = reader.read_task_statuses(&track_id("my-track")).unwrap_err();
        assert!(
            matches!(err, PreReviewGateError::ImplPlanReadFailed { .. }),
            "expected ImplPlanReadFailed, got: {err}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn read_task_statuses_returns_impl_plan_read_failed_for_symlinked_items_dir() {
        let dir = temp_items_dir();
        let real_items_dir = dir.path().join("real-items");
        let track_dir = real_items_dir.join("my-track");
        fs::create_dir_all(&track_dir).unwrap();
        fs::write(track_dir.join("impl-plan.json"), SAMPLE_JSON).unwrap();
        let link_items_dir = dir.path().join("items");
        std::os::unix::fs::symlink(&real_items_dir, &link_items_dir).unwrap();

        let reader = FsImplPlanReader::new(link_items_dir);
        let err = reader.read_task_statuses(&track_id("my-track")).unwrap_err();
        assert!(
            matches!(err, PreReviewGateError::ImplPlanReadFailed { .. }),
            "expected ImplPlanReadFailed, got: {err}"
        );
    }

    #[test]
    fn read_task_statuses_returns_impl_plan_read_failed_for_items_dir_outside_current_repo() {
        let dir = tempfile::tempdir().unwrap();
        let reader = FsImplPlanReader::new(dir.path().to_path_buf());
        let err = reader.read_task_statuses(&track_id("my-track")).unwrap_err();
        assert!(
            matches!(err, PreReviewGateError::ImplPlanReadFailed { .. }),
            "expected ImplPlanReadFailed, got: {err}"
        );
    }
}
